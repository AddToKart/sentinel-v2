impl SentinelManager {
    pub fn ensure_ide_terminal(self: &Arc<Self>, app: &AppHandle) -> Result<IdeTerminalState, String> {
        let project = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project.clone()
        };

        let project_path = match project.path.clone() {
            Some(path) => PathBuf::from(path),
            None => {
                let state = IdeTerminalState::idle();
                {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.ide.record = None;
                }
                emit_event(app, EVENT_IDE_STATE, &state);
                return Ok(state);
            }
        };

        let (workspace_path, modified_paths) = self.ensure_ide_workspace(app, &project)?;
        let workspace_path_string = path_to_string(&workspace_path);
        let should_reuse = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(record) = inner.ide.record.as_ref() {
                record.state.workspace_path.as_deref() == Some(workspace_path_string.as_str())
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

        self.close_ide_terminal(app)?;

        let handles = self.spawn_ide_terminal(
            app.clone(),
            workspace_path.clone(),
            120,
            28,
            project_path,
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

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.ide.record = Some(IdeRecord {
                state: state.clone(),
                master: handles.master,
                writer: handles.writer,
                killer: handles.killer,
                terminal_size: TerminalSize { cols: 120, rows: 28 },
                close_requested: false,
                finalized: false,
            });
        }

        emit_event(app, EVENT_IDE_STATE, &state);
        Ok(state)
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
                result.remaining_paths = applied.modified_paths.clone();
                {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.ide.sandbox_state = Some(applied.sandbox_state.clone());
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.modified_paths = applied.modified_paths.clone();
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
                {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.ide.sandbox_state = Some(discarded.sandbox_state);
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.modified_paths = discarded.modified_paths;
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
