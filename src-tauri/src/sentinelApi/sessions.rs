impl SentinelManager {
    pub fn create_session(self: &Arc<Self>, app: &AppHandle, input: CreateSessionInput) -> Result<SessionSummary, String> {
        let (project, session_count, preferences) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (inner.project.clone(), inner.sessions.len(), inner.preferences.clone())
        };

        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Open a project folder before starting an agent session.".to_string())?;

        let workspace_strategy = input
            .workspace_strategy
            .unwrap_or(preferences.default_session_strategy);
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
        };

        if let Some(command) = input
            .startup_command
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            append_history_entry(&mut record.history, command, "startup");
            write_terminal(&record.writer, format!("{command}\r").as_bytes())?;
        }

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.sessions.insert(session_id, record);
            update_workspace_summary(&mut inner);
        }

        emit_event(app, EVENT_SESSION_STATE, &summary);
        self.emit_session_metrics(app, &summary.id);
        self.emit_session_history(app, &summary.id);
        self.emit_session_diff(app, &summary.id);
        self.emit_workspace_state(app);
        Ok(summary)
    }

    pub fn send_input(&self, session_id: &str, data: &str) -> Result<(), String> {
        let writer = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            track_command_input(&mut record.command_buffer, &mut record.history, data);
            record.writer.clone()
        };
        write_terminal(&writer, data.as_bytes())
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
        let (pid, killer, should_emit, should_wait_for_shutdown) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;

            let should_wait_for_shutdown = !record.close_requested;
            if !record.close_requested {
                record.close_requested = true;
                if matches!(
                    record.summary.status,
                    SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Error
                ) {
                    record.summary.status = SessionStatus::Closing;
                    record.summary.error = None;
                    record.modified_paths.clear();
                }
            }

            (
                record.summary.pid,
                record.killer.clone(),
                should_wait_for_shutdown,
                should_wait_for_shutdown,
            )
        };

        if should_emit {
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
                self.finalize_session(
                    app.clone(),
                    session_id.clone(),
                    None,
                    Some("Sentinel forced this session to close after the shell stopped responding.".to_string()),
                );
                break;
            }
            thread::sleep(Duration::from_millis(sleep_duration_ms));
            // Exponential backoff: increase sleep duration up to max_sleep_ms
            sleep_duration_ms = (sleep_duration_ms * 2).min(max_sleep_ms);
        }

        let removed = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let removed = inner.sessions.remove(&session_id).is_some();
            if removed {
                update_workspace_summary(&mut inner);
            }
            removed
        };
        if removed {
            self.emit_workspace_state(&app);
        }
    }
}
