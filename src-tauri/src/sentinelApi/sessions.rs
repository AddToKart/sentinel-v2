impl SentinelManager {
    pub fn create_session(self: &Arc<Self>, app: &AppHandle, input: CreateSessionInput) -> Result<SessionSummary, String> {
        let (workspace_id, project, session_count, default_workspace_strategy, workspace_mode) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace = active_workspace_clone(&inner)
                .ok_or_else(|| "Open a project workspace before starting an agent session.".to_string())?;
            (
                workspace.id,
                workspace.project,
                workspace.session_ids.len(),
                workspace.default_session_strategy,
                workspace.mode,
            )
        };

        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Open a project folder before starting an agent session.".to_string())?;

        let workspace_strategy = input
            .workspace_strategy
            .unwrap_or(default_workspace_strategy);
        if workspace_strategy == SessionWorkspaceStrategy::GitWorktree && !project.is_git_repo {
            return Err(
                "Git Worktree mode requires a Git repository. Use Sandbox Copy mode for plain folders."
                    .to_string(),
            );
        }

        let label = input
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Agent {:02}", session_count + 1));

        let session_id = format!("{}-{}", create_timestamp(), create_token());
        let workspace = self.create_session_workspace(app, &project, &label, workspace_strategy)?;

        let cols = input.cols.unwrap_or(120);
        let rows = input.rows.unwrap_or(32);
        let shell = "powershell.exe".to_string();

        let handles = match self.spawn_session_terminal(
            app.clone(),
            session_id.clone(),
            workspace.workspace_path.clone(),
            cols,
            rows,
            workspace_strategy,
            workspace.branch_name.clone(),
        ) {
            Ok(handles) => handles,
            Err(error) => {
                let _ = self.cleanup_detached_session_workspace(
                    app,
                    Some(PathBuf::from(&project_path)),
                    &workspace.workspace_path,
                    workspace.branch_name.as_deref(),
                );
                return Err(error);
            }
        };

        let summary = SessionSummary {
            id: session_id.clone(),
            workspace_id: workspace_id.clone(),
            label,
            project_root: project_path,
            cwd: path_to_string(&workspace.workspace_path),
            workspace_path: path_to_string(&workspace.workspace_path),
            workspace_strategy,
            branch_name: workspace.branch_name.clone(),
            status: SessionStatus::Starting,
            cleanup_state: CleanupState::Active,
            shell,
            pid: handles.pid,
            created_at: now_millis(),
            startup_command: input.startup_command.clone(),
            exit_code: None,
            error: None,
            metrics: ProcessMetrics::default(),
            mode: workspace_mode,
        };

        let mut record = SessionRecord {
            summary: summary.clone(),
            master: handles.master,
            writer: handles.writer,
            killer: handles.killer,
            terminal_size: TerminalSize { cols, rows },
            close_requested: false,
            finalized: false,
            command_buffer: String::new(),
            history: Vec::new(),
            modified_paths: Vec::new(),
            sandbox_state: workspace.sandbox_state,
            tracked_process_ids: handles.pid.into_iter().collect(),
            last_cpu_total_seconds: None,
            last_sampled_at: None,
            last_persisted_metrics: summary.metrics.clone(),
            last_persisted_metrics_at: Some(summary.created_at),
            last_diff_scanned_at: None,
            shutdown_mode: SessionShutdownMode::Stop,
        };

        let mut startup_history_entry = None;
        if let Some(command) = input
            .startup_command
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            startup_history_entry = append_history_entry(&mut record.history, command, "startup");
            write_terminal(&record.writer, format!("{command}\r").as_bytes())?;
        }

        let workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(workspace) = inner.workspaces.get_mut(&workspace_id) {
                workspace.session_ids.push(summary.id.clone());
                workspace.last_active_at = now_millis();
            }
            inner.sessions.insert(session_id, record);
            update_workspace_summary(&mut inner);
            inner.workspaces.get(&workspace_id).cloned()
        };

        self.persist_session_created(app, &summary)?;
        if let Some(workspace) = workspace.as_ref() {
            self.persist_workspace(app, workspace)?;
        }
        if let Some(entry) = startup_history_entry.as_ref() {
            self.persist_command_entry(app, &summary, entry)?;
        }
        self.persist_audit_event(
            app,
            Some(&summary.workspace_id),
            Some(&summary.id),
            None,
            "session-created",
            "session",
            &summary.id,
            Some(serde_json::json!({
                "label": summary.label.clone(),
                "workspaceStrategy": summary.workspace_strategy,
                "workspacePath": summary.workspace_path.clone(),
            })),
        );

        emit_event(app, EVENT_SESSION_STATE, &summary);
        self.emit_session_metrics(app, &summary.id);
        self.emit_session_history(app, &summary.id);
        self.emit_session_diff(app, &summary.id);
        self.emit_workspace_updated(app, &workspace_id);
        self.emit_workspace_state(app);
        Ok(summary)
    }

    pub fn send_input(&self, app: &AppHandle, session_id: &str, data: &str) -> Result<(), String> {
        let (writer, summary, entries) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            let entries = track_command_input(&mut record.command_buffer, &mut record.history, data);
            (record.writer.clone(), record.summary.clone(), entries)
        };

        match write_terminal(&writer, data.as_bytes()) {
            Ok(()) => {
                for entry in &entries {
                    if let Err(error) = self.persist_command_entry(app, &summary, entry) {
                        log_persistence_error("persist session command", &error);
                    }
                }
                if !entries.is_empty() {
                    self.emit_session_history(app, session_id);
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("[sentinel] Failed to send input to session {}: {}", session_id, e);
                Err(e)
            }
        }
    }

    pub fn resize_session(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }

        let master = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            if record.terminal_size.cols == cols && record.terminal_size.rows == rows {
                return Ok(());
            }
            record.terminal_size = TerminalSize { cols, rows };
            record.master.clone()
        };

        resize_terminal(&master, cols, rows)
    }

    pub fn close_session(self: &Arc<Self>, app: &AppHandle, session_id: &str) -> Result<(), String> {
        self.request_session_shutdown(app, session_id, SessionShutdownMode::Stop)
    }

    pub fn pause_session(self: &Arc<Self>, app: &AppHandle, session_id: &str) -> Result<(), String> {
        self.request_session_shutdown(app, session_id, SessionShutdownMode::Pause)
    }

    pub fn resume_session(
        self: &Arc<Self>,
        app: &AppHandle,
        session_id: &str,
    ) -> Result<SessionSummary, String> {
        if let Some(summary) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.sessions.get(session_id).map(|record| record.summary.clone())
        } {
            return Ok(summary);
        }

        let pool = database_pool(app);
        let row = tauri::async_runtime::block_on(SessionRepository::find_by_id(&pool, session_id))
            .map_err(|error| format!("Failed to load session {session_id}: {error}"))?
            .ok_or_else(|| "Session not found.".to_string())?;
        let mut summary = session_summary_from_row(row, WorkspaceMode::Local);

        if summary.status != SessionStatus::Paused {
            return Err("Only paused sessions can be resumed.".to_string());
        }

        let workspace_path = PathBuf::from(&summary.workspace_path);
        if !workspace_path.exists() {
            summary.status = SessionStatus::Error;
            summary.cleanup_state = CleanupState::Failed;
            summary.error =
                Some("The preserved session workspace could not be found.".to_string());
            self.persist_session_status(app, &summary)?;
            emit_event(app, EVENT_SESSION_STATE, &summary);
            return Err(
                "The preserved session workspace could not be found. Delete this session and start a new one."
                    .to_string(),
            );
        }

        let (command_rows, file_change_rows) = tauri::async_runtime::block_on(async {
            tokio::try_join!(
                CommandRepository::find_by_session(&pool, session_id, Some(250)),
                FileChangeRepository::find_by_session(&pool, session_id),
            )
        })
        .map_err(|error| format!("Failed to restore saved session state: {error}"))?;

        let cols = 120;
        let rows = 32;
        let handles = self.spawn_session_terminal(
            app.clone(),
            summary.id.clone(),
            workspace_path.clone(),
            cols,
            rows,
            summary.workspace_strategy,
            summary.branch_name.clone(),
        )?;

        let history = session_history_entries_from_rows(command_rows);
        let (modified_paths, sandbox_state) =
            if summary.workspace_strategy == SessionWorkspaceStrategy::SandboxCopy {
                let mut sandbox_state = restore_sandbox_workspace_state(
                    Path::new(&summary.project_root),
                    &workspace_path,
                )?;
                let (modified_paths, next_cache) =
                    refresh_sandbox_workspace_diffs(&workspace_path, &mut sandbox_state)?;
                sandbox_state.scan_cache = next_cache;
                (modified_paths, Some(sandbox_state))
            } else {
                let modified_paths = run_command(
                    "git",
                    &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
                    Some(&workspace_path),
                )
                .map(|raw| parse_git_status_output(&raw))
                .unwrap_or_else(|_| {
                    session_diff_snapshot_from_rows(
                        &summary.id,
                        &summary.workspace_id,
                        file_change_rows,
                        summary.created_at,
                    )
                    .modified_paths
                });
                (modified_paths, None)
            };

        summary.status = SessionStatus::Starting;
        summary.pid = handles.pid;
        summary.exit_code = None;
        summary.error = None;
        summary.metrics = ProcessMetrics::default();

        let record = SessionRecord {
            summary: summary.clone(),
            master: handles.master,
            writer: handles.writer,
            killer: handles.killer,
            terminal_size: TerminalSize { cols, rows },
            close_requested: false,
            finalized: false,
            command_buffer: String::new(),
            history,
            modified_paths,
            sandbox_state,
            tracked_process_ids: handles.pid.into_iter().collect(),
            last_cpu_total_seconds: None,
            last_sampled_at: None,
            last_persisted_metrics: ProcessMetrics::default(),
            last_persisted_metrics_at: Some(now_millis()),
            last_diff_scanned_at: None,
            shutdown_mode: SessionShutdownMode::Stop,
        };

        let workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(workspace) = inner.workspaces.get_mut(&summary.workspace_id) {
                if !workspace.session_ids.iter().any(|id| id == session_id) {
                    workspace.session_ids.push(session_id.to_string());
                }
                workspace.last_active_at = now_millis();
            }
            inner.sessions.insert(session_id.to_string(), record);
            update_workspace_summary(&mut inner);
            inner.workspaces.get(&summary.workspace_id).cloned()
        };

        self.persist_session_status(app, &summary)?;
        self.persist_audit_event(
            app,
            Some(&summary.workspace_id),
            Some(&summary.id),
            None,
            "session-resumed",
            "session",
            &summary.id,
            Some(serde_json::json!({
                "workspacePath": summary.workspace_path.clone(),
                "workspaceStrategy": summary.workspace_strategy,
            })),
        );
        if let Some(workspace) = workspace.as_ref() {
            self.persist_workspace(app, workspace)?;
            self.emit_workspace_updated(app, &workspace.id);
        }
        emit_event(app, EVENT_SESSION_STATE, &summary);
        self.emit_session_metrics(app, &summary.id);
        self.emit_session_history(app, &summary.id);
        self.emit_session_diff(app, &summary.id);
        self.emit_workspace_state(app);
        Ok(summary)
    }

    pub fn delete_session(&self, app: &AppHandle, session_id: &str) -> Result<(), String> {
        {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inner.sessions.contains_key(session_id) {
                return Err("Pause or stop the session before deleting it.".to_string());
            }
        }

        let pool = database_pool(app);
        let row = tauri::async_runtime::block_on(SessionRepository::find_by_id(&pool, session_id))
            .map_err(|error| format!("Failed to load session {session_id}: {error}"))?
            .ok_or_else(|| "Session not found.".to_string())?;
        let mut summary = session_summary_from_row(row, WorkspaceMode::Local);

        if summary.cleanup_state != CleanupState::Removed {
            summary.error = None;
            match cleanup_session_workspace(
                app,
                Path::new(&summary.project_root),
                Path::new(&summary.workspace_path),
                summary.workspace_strategy,
                summary.branch_name.as_deref(),
            ) {
                Ok(cleanup_state) => {
                    summary.cleanup_state = cleanup_state;
                }
                Err(error) => {
                    summary.cleanup_state = CleanupState::Failed;
                    summary.error = Some(error);
                    self.persist_session_status(app, &summary)?;
                    emit_event(
                        app,
                        EVENT_SESSION_STATE,
                        &summary,
                    );
                    return Err(summary
                        .error
                        .clone()
                        .unwrap_or_else(|| "Failed to clean up the session workspace.".to_string()));
                }
            }
        }

        tauri::async_runtime::block_on(SessionRepository::delete(&pool, session_id))
            .map_err(|error| format!("Failed to delete session {session_id}: {error}"))?;

        let workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace = inner.workspaces.get_mut(&summary.workspace_id);
            if let Some(workspace) = workspace {
                workspace.session_ids.retain(|id| id != session_id);
                workspace.last_active_at = now_millis();
            }
            update_workspace_summary(&mut inner);
            inner.workspaces.get(&summary.workspace_id).cloned()
        };

        if let Some(workspace) = workspace.as_ref() {
            self.persist_workspace(app, workspace)?;
            self.emit_workspace_updated(app, &workspace.id);
        }
        self.persist_audit_event(
            app,
            Some(&summary.workspace_id),
            Some(session_id),
            None,
            "session-deleted",
            "session",
            session_id,
            Some(serde_json::json!({
                "cleanupState": summary.cleanup_state,
            })),
        );
        self.emit_workspace_state(app);
        Ok(())
    }

    fn request_session_shutdown(
        self: &Arc<Self>,
        app: &AppHandle,
        session_id: &str,
        shutdown_mode: SessionShutdownMode,
    ) -> Result<(), String> {
        let live_shutdown = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(record) = inner.sessions.get_mut(session_id) {
                if record.finalized {
                    return Ok(());
                }

                let should_wait_for_shutdown = !record.close_requested;
                record.shutdown_mode = shutdown_mode;
                if !record.close_requested {
                    record.close_requested = true;
                    if matches!(
                        record.summary.status,
                        SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Error | SessionStatus::Paused
                    ) {
                        record.summary.status = SessionStatus::Closing;
                        record.summary.error = None;
                    }
                }

                Some((
                    record.summary.pid,
                    record.killer.clone(),
                    should_wait_for_shutdown,
                    record.summary.clone(),
                ))
            } else {
                None
            }
        };

        if let Some((pid, killer, should_wait_for_shutdown, summary)) = live_shutdown {
            self.persist_session_status(app, &summary)?;
            self.persist_audit_event(
                app,
                Some(&summary.workspace_id),
                Some(&summary.id),
                None,
                match shutdown_mode {
                    SessionShutdownMode::Stop => "session-stop-requested",
                    SessionShutdownMode::Pause => "session-pause-requested",
                },
                "session",
                &summary.id,
                None,
            );

            if should_wait_for_shutdown {
                self.emit_session_diff(app, session_id);
                self.emit_session_state(app, session_id);
                self.emit_workspace_state(app);
            }

            let _ = kill_with_killer(&killer);
            let _ = terminate_process_id(pid);

            if should_wait_for_shutdown {
                let manager = self.clone();
                let app_handle = app.clone();
                let session_id = session_id.to_string();
                thread::spawn(move || {
                    manager.finish_closing_session(app_handle, session_id);
                });
            }

            return Ok(());
        }

        self.shutdown_persisted_session(app, session_id, shutdown_mode)
    }

    fn shutdown_persisted_session(
        &self,
        app: &AppHandle,
        session_id: &str,
        shutdown_mode: SessionShutdownMode,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        let row = tauri::async_runtime::block_on(SessionRepository::find_by_id(&pool, session_id))
            .map_err(|error| format!("Failed to load session {session_id}: {error}"))?
            .ok_or_else(|| "Session not found.".to_string())?;
        let mut summary = session_summary_from_row(row, WorkspaceMode::Local);

        match shutdown_mode {
            SessionShutdownMode::Pause => {
                if summary.status == SessionStatus::Paused {
                    return Ok(());
                }
                if matches!(summary.status, SessionStatus::Closed | SessionStatus::Error) {
                    return Err("Only preserved sessions can be paused after restart.".to_string());
                }
                summary.status = SessionStatus::Paused;
                summary.cleanup_state = match summary.cleanup_state {
                    CleanupState::Active => CleanupState::Preserved,
                    other => other,
                };
                summary.pid = None;
                summary.exit_code = None;
                summary.error = None;
                summary.metrics = ProcessMetrics::default();
            }
            SessionShutdownMode::Stop => {
                summary.status = SessionStatus::Closed;
                summary.pid = None;
                summary.exit_code = None;
                summary.error = None;
                summary.metrics = ProcessMetrics::default();

                if summary.cleanup_state != CleanupState::Removed {
                    match cleanup_session_workspace(
                        app,
                        Path::new(&summary.project_root),
                        Path::new(&summary.workspace_path),
                        summary.workspace_strategy,
                        summary.branch_name.as_deref(),
                    ) {
                        Ok(cleanup_state) => {
                            summary.cleanup_state = cleanup_state;
                        }
                        Err(error) => {
                            summary.cleanup_state = CleanupState::Failed;
                            summary.error = Some(error);
                        }
                    }
                }
            }
        }

        self.persist_session_status(app, &summary)?;
        self.persist_audit_event(
            app,
            Some(&summary.workspace_id),
            Some(&summary.id),
            None,
            match shutdown_mode {
                SessionShutdownMode::Stop => "session-stopped",
                SessionShutdownMode::Pause => "session-paused",
            },
            "session",
            &summary.id,
            Some(serde_json::json!({
                "status": summary.status,
                "cleanupState": summary.cleanup_state,
                "error": summary.error,
            })),
        );

        let workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(workspace) = inner.workspaces.get_mut(&summary.workspace_id) {
                workspace.last_active_at = now_millis();
            }
            update_workspace_summary(&mut inner);
            inner.workspaces.get(&summary.workspace_id).cloned()
        };

        if let Some(workspace) = workspace.as_ref() {
            self.persist_workspace(app, workspace)?;
            self.emit_workspace_updated(app, &workspace.id);
        }

        emit_event(app, EVENT_SESSION_STATE, &summary);
        if summary.cleanup_state == CleanupState::Removed {
            emit_event(
                app,
                EVENT_SESSION_DIFF,
                &SessionDiffUpdate {
                    session_id: summary.id.clone(),
                    workspace_id: summary.workspace_id.clone(),
                    modified_paths: Vec::new(),
                    updated_at: now_millis(),
                },
            );
        }
        self.emit_workspace_state(app);
        Ok(())
    }

    fn finish_closing_session(self: Arc<Self>, app: AppHandle, session_id: String) {
        let start = now_millis();
        let mut sleep_duration_ms: u64 = 20;
        let max_sleep_ms: u64 = 500;

        loop {
            let done = {
                let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner
                    .sessions
                    .get(&session_id)
                    .map(|record| record.finalized)
                    .unwrap_or(true)
            };
            if done {
                break;
            }
            if now_millis() - start >= CLOSE_TIMEOUT_MS as i64 {
                let forced_message = {
                    let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.sessions.get(&session_id).map(|record| match record.shutdown_mode {
                        SessionShutdownMode::Pause => {
                            "Sentinel paused this session after the shell stopped responding."
                                .to_string()
                        }
                        SessionShutdownMode::Stop => {
                            "Sentinel forced this session to close after the shell stopped responding."
                                .to_string()
                        }
                    })
                }
                .unwrap_or_else(|| {
                    "Sentinel forced this session to close after the shell stopped responding."
                        .to_string()
                });
                self.finalize_session(
                    app.clone(),
                    session_id.clone(),
                    None,
                    Some(forced_message),
                );
                break;
            }
            thread::sleep(Duration::from_millis(sleep_duration_ms));
            // Exponential backoff: increase sleep duration up to max_sleep_ms
            sleep_duration_ms = (sleep_duration_ms * 2).min(max_sleep_ms);
        }

        let updated_workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace_id = inner
                .sessions
                .remove(&session_id)
                .map(|record| record.summary.workspace_id);
            if let Some(workspace_id) = workspace_id.as_deref() {
                if let Some(workspace) = inner.workspaces.get_mut(workspace_id) {
                    workspace.last_active_at = now_millis();
                }
                update_workspace_summary(&mut inner);
            }
            workspace_id.and_then(|workspace_id| inner.workspaces.get(&workspace_id).cloned())
        };
        if let Some(workspace) = updated_workspace.as_ref() {
            if let Err(error) = self.persist_workspace(&app, workspace) {
                log_persistence_error("persist workspace after session removal", &error);
            }
            self.emit_workspace_updated(&app, &workspace.id);
            self.emit_workspace_state(&app);
        }
    }
}
