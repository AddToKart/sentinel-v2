impl SentinelManager {
    pub fn ensure_ide_terminal(self: &Arc<Self>, app: &AppHandle) -> Result<IdeTerminalState, String> {
        let project = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project.clone()
        };

        // If no project is open, return idle immediately
        if project.path.is_none() {
            let state = IdeTerminalState::idle();
            {
                let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.ide.record = None;
            }
            emit_event(app, EVENT_IDE_STATE, &state);
            return Ok(state);
        }

        // Check if we can reuse the existing terminal quickly without copying anything
        let project_root_str = project.path.clone().unwrap();
        let project_root_path = PathBuf::from(&project_root_str);
        let should_reuse = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let (Some(record), Some(workspace_project_root), Some(_sandbox_state)) = (
                inner.ide.record.as_ref(),
                inner.ide.workspace_project_root.as_ref(),
                inner.ide.sandbox_state.as_ref(),
            ) {
                workspace_project_root == &project_root_path
                    && !matches!(record.state.status, IdeStatus::Closed | IdeStatus::Error)
            } else {
                false
            }
        };

        if should_reuse {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            return Ok(
                inner
                    .ide
                    .record
                    .as_ref()
                    .map(|record| record.state.clone())
                    .unwrap_or_else(IdeTerminalState::idle),
            );
        }

        // Return a Staring state immediately to unblock the frontend IPC call.
        // The expensive workspace copy + PTY spawn happens in a background thread.
        let starting_state = IdeTerminalState {
            status: IdeStatus::Starting,
            cwd: Some(project_root_str.clone()),
            workspace_path: None,
            shell: "powershell.exe".to_string(),
            pid: None,
            created_at: Some(now_millis()),
            exit_code: None,
            error: None,
            modified_paths: Vec::new(),
        };
        emit_event(app, EVENT_IDE_STATE, &starting_state);

        let manager = self.clone();
        let app_handle = app.clone();
        thread::spawn(move || {
            if let Err(err) = manager.start_ide_terminal_background(&app_handle, project) {
                eprintln!("[sentinel] IDE terminal background init error: {}", err);
                let error_state = IdeTerminalState {
                    status: IdeStatus::Error,
                    cwd: Some(project_root_str),
                    workspace_path: None,
                    shell: "powershell.exe".to_string(),
                    pid: None,
                    created_at: Some(now_millis()),
                    exit_code: None,
                    error: Some(err),
                    modified_paths: Vec::new(),
                };
                emit_event(&app_handle, EVENT_IDE_STATE, &error_state);
            }
        });

        Ok(starting_state)
    }

    fn start_ide_terminal_background(self: &Arc<Self>, app: &AppHandle, project: ProjectState) -> Result<(), String> {
        let project_path = project.path.clone()
            .ok_or_else(|| "Project root is unavailable.".to_string())?;
        let project_root = PathBuf::from(&project_path);

        self.close_ide_terminal(app)?;

        let (workspace_path, modified_paths) = self.ensure_ide_workspace(app, &project)?;
        let workspace_path_string = path_to_string(&workspace_path);

        let handles = self.spawn_ide_terminal(
            app.clone(),
            workspace_path.clone(),
            120,
            28,
            project_root,
        )?;
        let state = IdeTerminalState {
            status: IdeStatus::Starting,
            cwd: Some(workspace_path_string.clone()),
            workspace_path: Some(workspace_path_string),
            shell: "powershell.exe".to_string(),
            pid: handles.pid,
            created_at: Some(now_millis()),
            exit_code: None,
            error: None,
            modified_paths,
        };

        let workspace_id = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace_id = inner.active_workspace_id.clone();
            inner.ide.record = Some(IdeRecord {
                state: state.clone(),
                master: handles.master,
                writer: handles.writer,
                killer: handles.killer,
                terminal_size: TerminalSize { cols: 120, rows: 28 },
                close_requested: false,
                finalized: false,
            });
            workspace_id
        };

        if let Some(workspace_id) = workspace_id.as_deref() {
            self.persist_ide_state(app, workspace_id, &state)?;
        }

        emit_event(app, EVENT_IDE_STATE, &state);
        Ok(())
    }

    pub fn send_ide_terminal_input(self: &Arc<Self>, app: &AppHandle, data: &str) -> Result<(), String> {
        let writer = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.ide.record.as_ref().map(|record| record.writer.clone())
        };

        match writer {
            Some(writer) => {
                match write_terminal(&writer, data.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        eprintln!("[sentinel] Failed to send input to IDE terminal: {}", e);
                        Err(e)
                    }
                }
            }
            None => {
                let _ = self.ensure_ide_terminal(app)?;
                let writer = {
                    let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner
                        .ide
                        .record
                        .as_ref()
                        .map(|record| record.writer.clone())
                        .ok_or_else(|| "IDE terminal is unavailable.".to_string())?
                };
                match write_terminal(&writer, data.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        eprintln!("[sentinel] Failed to send input to IDE terminal (after ensure): {}", e);
                        Err(e)
                    }
                }
            }
        }
    }

    pub fn resize_ide_terminal(self: &Arc<Self>, app: &AppHandle, cols: u16, rows: u16) -> Result<(), String> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }

        let master = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inner.ide.record.is_none() {
                drop(inner);
                let _ = self.ensure_ide_terminal(app)?;
                inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            }

            let record = inner
                .ide
                .record
                .as_mut()
                .ok_or_else(|| "IDE terminal is unavailable.".to_string())?;
            if record.terminal_size.cols == cols && record.terminal_size.rows == rows {
                return Ok(());
            }
            record.terminal_size = TerminalSize { cols, rows };
            record.master.clone()
        };

        resize_terminal(&master, cols, rows)
    }

    pub fn write_ide_file(self: &Arc<Self>, app: &AppHandle, relative_path: &str, content: &str) -> Result<(), String> {
        let project = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project.clone()
        };
        let _ = self.ensure_ide_workspace(app, &project)?;
        let workspace_path = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .ide
                .workspace_path
                .clone()
                .ok_or_else(|| "IDE workspace is unavailable.".to_string())?
        };
        write_workspace_file(&workspace_path, relative_path, content)?;
        self.refresh_runtime_state(app);
        Ok(())
    }

    pub fn apply_ide_workspace(self: &Arc<Self>, app: &AppHandle) -> Result<SessionApplyResult, String> {
        let (project_root, workspace_path, sandbox_state) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (
                inner.project.path.clone(),
                inner.ide.workspace_path.clone(),
                inner.ide.sandbox_state.clone(),
            )
        };

        let project_root = project_root
            .map(PathBuf::from)
            .ok_or_else(|| "IDE workspace is unavailable.".to_string())?;
        let workspace_path = workspace_path.ok_or_else(|| "IDE workspace is unavailable.".to_string())?;
        let sandbox_state = sandbox_state.ok_or_else(|| "IDE workspace is unavailable.".to_string())?;

        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Sync IDE workspace changes to main project files",
            path_to_string(&workspace_path),
            None,
        );

        match apply_ide_workspace_impl(&project_root, &workspace_path, sandbox_state) {
            Ok(applied) => {
                let mut result = applied.result;
                let persisted_state = {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    let workspace_id = inner.active_workspace_id.clone();
                    result.remaining_paths = applied.modified_paths.clone();
                    inner.ide.sandbox_state = Some(applied.sandbox_state.clone());
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.modified_paths = applied.modified_paths.clone();
                        workspace_id.map(|workspace_id| (workspace_id, record.state.clone()))
                    } else {
                        None
                    }
                };
                if let Some((workspace_id, state)) = persisted_state {
                    if let Err(error) = self.persist_ide_state(app, &workspace_id, &state) {
                        log_persistence_error("persist IDE terminal state after apply", &error);
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    if result.conflicts.is_empty() { "completed" } else { "failed" },
                    "Sync IDE workspace changes to main project files",
                    path_to_string(&workspace_path),
                    Some(if result.conflicts.is_empty() {
                        format!(
                            "{} file(s) synced, {} remaining",
                            result.applied_paths.len(),
                            result.remaining_paths.len()
                        )
                    } else {
                        format!(
                            "{} synced, {} conflicts, {} remaining",
                            result.applied_paths.len(),
                            result.conflicts.len(),
                            result.remaining_paths.len()
                        )
                    }),
                );
                if !result.applied_paths.is_empty() {
                    let _ = self.refresh_project_snapshot(app);
                }
                self.refresh_runtime_state(app);
                self.emit_ide_state(app);
                Ok(result)
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Sync IDE workspace changes to main project files",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    pub fn discard_ide_workspace_changes(self: &Arc<Self>, app: &AppHandle) -> Result<(), String> {
        let (project_root, workspace_path) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (inner.project.path.clone(), inner.ide.workspace_path.clone())
        };

        let project_root = project_root
            .map(PathBuf::from)
            .ok_or_else(|| "IDE workspace is unavailable.".to_string())?;
        let workspace_path = workspace_path.ok_or_else(|| "IDE workspace is unavailable.".to_string())?;

        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Discard IDE workspace changes",
            path_to_string(&workspace_path),
            None,
        );

        match discard_ide_workspace_impl(&project_root, &workspace_path) {
            Ok(discarded) => {
                let persisted_state = {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    let workspace_id = inner.active_workspace_id.clone();
                    inner.ide.sandbox_state = Some(discarded.sandbox_state);
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.modified_paths = discarded.modified_paths;
                        workspace_id.map(|workspace_id| (workspace_id, record.state.clone()))
                    } else {
                        None
                    }
                };
                if let Some((workspace_id, state)) = persisted_state {
                    if let Err(error) = self.persist_ide_state(app, &workspace_id, &state) {
                        log_persistence_error("persist IDE terminal state after discard", &error);
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Discard IDE workspace changes",
                    path_to_string(&workspace_path),
                    None,
                );
                self.emit_ide_state(app);
                Ok(())
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Discard IDE workspace changes",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }
}
