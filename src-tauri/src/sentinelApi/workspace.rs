impl SentinelManager {
    fn handle_project_changed(&self, app: &AppHandle, project_path: Option<PathBuf>) -> Result<(), String> {
        let (needs_close, old_workspace, old_project_root) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (
                inner.ide.record.is_some(),
                inner.ide.workspace_path.clone(),
                inner.ide.workspace_project_root.clone(),
            )
        };

        if needs_close {
            self.close_ide_terminal(app)?;
        }

        let project_changed = old_project_root != project_path;
        if project_changed {
            if let Some(workspace_path) = old_workspace {
                let _ = fs::remove_dir_all(&workspace_path);
            }
            let state = IdeTerminalState::idle();
            {
                let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.ide = IdeRuntime::default();
            }
            emit_event(app, EVENT_IDE_STATE, &state);
        }

        Ok(())
    }

    fn refresh_project_snapshot(&self, app: &AppHandle) -> Result<ProjectState, String> {
        let (active_workspace_id, current_project) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (inner.active_workspace_id.clone(), inner.project.clone())
        };

        let Some(project_path) = current_project.path.as_deref() else {
            self.emit_project_state(app);
            return Ok(current_project);
        };

        let next_project = inspect_project(Path::new(project_path))?;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project = next_project.clone();
            sync_active_project_to_workspace(&mut inner);
            update_workspace_summary(&mut inner);
        }

        if let Some(active_workspace_id) = active_workspace_id {
            self.emit_workspace_updated(app, &active_workspace_id);
        }
        self.emit_project_state(app);
        self.emit_workspace_state(app);
        Ok(next_project)
    }

    fn close_ide_terminal(&self, app: &AppHandle) -> Result<(), String> {
        let (pid, killer) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(record) = inner.ide.record.as_mut() else {
                return Ok(());
            };

            if !record.close_requested {
                record.close_requested = true;
                if matches!(record.state.status, IdeStatus::Starting | IdeStatus::Ready | IdeStatus::Error) {
                    record.state.status = IdeStatus::Closing;
                    record.state.error = None;
                }
            }

            (record.state.pid, record.killer.clone())
        };

        self.emit_ide_state(app);
        let _ = kill_with_killer(&killer);
        let _ = terminate_process_id(pid);

        let start = now_millis();
        loop {
            let done = {
                let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.ide.record.as_ref().map(|record| record.finalized).unwrap_or(true)
            };
            if done {
                break;
            }
            if now_millis() - start >= CLOSE_TIMEOUT_MS as i64 {
                self.finalize_ide_terminal(
                    app.clone(),
                    None,
                    Some("Sentinel forced the IDE terminal to close after the shell stopped responding.".to_string()),
                );
                break;
            }
            thread::sleep(Duration::from_millis(80));
        }

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.ide.record = None;
        }
        Ok(())
    }

    fn create_session_workspace(
        &self,
        app: &AppHandle,
        project: &ProjectState,
        label: &str,
        workspace_strategy: SessionWorkspaceStrategy,
    ) -> Result<SessionWorkspaceResult, String> {
        if workspace_strategy == SessionWorkspaceStrategy::GitWorktree {
            return self.create_worktree(app, project, label);
        }

        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Project root is unavailable.".to_string())?;
        let project_name = sanitize_segment(project.name.as_deref().unwrap_or("project"));
        let session_stamp = create_timestamp();
        let token = create_token();
        let temp_root = std::env::temp_dir().join("sentinel-sandboxes").join(project_name);
        let workspace_path = temp_root.join(format!("{}-{}-{}", sanitize_segment(label), session_stamp, token));

        fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Create sandbox workspace",
            path_to_string(&workspace_path),
            None,
        );

        match create_sandbox_workspace(Path::new(&project_path), &workspace_path) {
            Ok(sandbox_state) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Create sandbox workspace",
                    path_to_string(&workspace_path),
                    None,
                );
                Ok(SessionWorkspaceResult {
                    workspace_path,
                    branch_name: None,
                    sandbox_state: Some(sandbox_state),
                })
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Create sandbox workspace",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    fn create_worktree(
        &self,
        app: &AppHandle,
        project: &ProjectState,
        label: &str,
    ) -> Result<SessionWorkspaceResult, String> {
        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Project root is unavailable.".to_string())?;
        let project_name = sanitize_segment(project.name.as_deref().unwrap_or("repo"));
        let session_stamp = create_timestamp();
        let token = create_token();
        let branch_name = format!(
            "sentinel/{}-{}-{}-{}",
            project_name,
            sanitize_segment(label),
            session_stamp,
            token
        );
        let temp_root = std::env::temp_dir().join("sentinel-worktrees").join(project_name);
        let workspace_path = temp_root.join(format!("{}-{}-{}", sanitize_segment(label), session_stamp, token));

        fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
        run_git_command(
            Some(app),
            Path::new(&project_path),
            [
                "worktree",
                "add",
                "-b",
                &branch_name,
                &path_to_string(&workspace_path),
                "HEAD",
            ],
        )?;

        Ok(SessionWorkspaceResult {
            workspace_path,
            branch_name: Some(branch_name),
            sandbox_state: None,
        })
    }

    fn cleanup_detached_session_workspace(
        &self,
        app: &AppHandle,
        project_root: Option<PathBuf>,
        workspace_path: &Path,
        branch_name: Option<&str>,
    ) -> Result<(), String> {
        if let Some(branch_name) = branch_name {
            if let Some(project_root) = project_root.as_ref() {
                let _ = run_git_command(Some(app), project_root, ["worktree", "remove", "--force", &path_to_string(workspace_path)]);
                let _ = run_git_command(Some(app), project_root, ["branch", "-D", branch_name]);
            }
        }

        fs::remove_dir_all(workspace_path)
            .map_err(|error| error.to_string())
            .or(Ok(()))
    }

    fn ensure_ide_workspace(
        &self,
        app: &AppHandle,
        project: &ProjectState,
    ) -> Result<(PathBuf, Vec<String>), String> {
        let project_root = project
            .path
            .clone()
            .ok_or_else(|| "Open a project folder before using IDE mode.".to_string())?;
        let project_root_path = PathBuf::from(project_root.clone());

        {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let (Some(workspace_path), Some(workspace_project_root), Some(_sandbox_state)) = (
                inner.ide.workspace_path.as_ref(),
                inner.ide.workspace_project_root.as_ref(),
                inner.ide.sandbox_state.as_ref(),
            ) {
                if workspace_project_root == &project_root_path {
                    let modified_paths = inner
                        .ide
                        .record
                        .as_ref()
                        .map(|record| record.state.modified_paths.clone())
                        .unwrap_or_default();
                    return Ok((workspace_path.clone(), modified_paths));
                }
            }
        }

        let old_workspace = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.ide.workspace_path.clone()
        };
        if let Some(old_workspace) = old_workspace {
            let _ = fs::remove_dir_all(old_workspace);
        }

        let project_name = sanitize_segment(project.name.as_deref().unwrap_or("project"));
        let session_stamp = create_timestamp();
        let token = create_token();
        let temp_root = std::env::temp_dir().join("sentinel-ide").join(project_name);
        let workspace_path = temp_root.join(format!("ide-{}-{}", session_stamp, token));

        fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Create IDE workspace",
            path_to_string(&workspace_path),
            None,
        );

        match create_sandbox_workspace(Path::new(&project_root), &workspace_path) {
            Ok(sandbox_state) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Create IDE workspace",
                    path_to_string(&workspace_path),
                    None,
                );
                {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.ide.workspace_path = Some(workspace_path.clone());
                    inner.ide.workspace_project_root = Some(project_root_path);
                    inner.ide.sandbox_state = Some(sandbox_state);
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.cwd = Some(path_to_string(&workspace_path));
                        record.state.workspace_path = Some(path_to_string(&workspace_path));
                        record.state.modified_paths.clear();
                    }
                }
                Ok((workspace_path, Vec::new()))
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Create IDE workspace",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }
}
