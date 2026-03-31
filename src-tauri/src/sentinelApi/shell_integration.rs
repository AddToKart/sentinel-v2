impl SentinelManager {
    pub fn reveal_in_file_explorer(&self, file_path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let status = Command::new("explorer.exe")
                .args(["/select,", file_path])
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to reveal file in File Explorer.".to_string())
            }
        }
        #[cfg(not(windows))]
        {
            let path = Path::new(file_path);
            let parent = path.parent().unwrap_or(path);
            self.open_in_system_editor(parent.to_string_lossy().as_ref())
        }
    }

    pub fn open_in_system_editor(&self, file_path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let status = Command::new("cmd")
                .args(["/C", "start", "", file_path])
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
        #[cfg(target_os = "macos")]
        {
            let status = Command::new("open")
                .arg(file_path)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let status = Command::new("xdg-open")
                .arg(file_path)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
    }

    pub fn dispose(&self, app: &AppHandle) {
        let (session_summaries, tab_summaries, ide_snapshot, session_pids, tab_pids, ide_pid, ide_workspace) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            for record in inner.sessions.values_mut() {
                if matches!(
                    record.summary.status,
                    SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Closing
                ) {
                    record.close_requested = true;
                    record.shutdown_mode = SessionShutdownMode::Pause;
                    record.summary.status = SessionStatus::Closing;
                    record.summary.error = Some(
                        "Session paused because Sentinel exited while this agent was still running."
                            .to_string(),
                    );
                }
            }
            for record in inner.tabs.values_mut() {
                if matches!(
                    record.summary.status,
                    TabStatus::Starting | TabStatus::Ready | TabStatus::Closing
                ) {
                    record.close_requested = true;
                    record.summary.status = TabStatus::Closing;
                    record.summary.error = Some(
                        "Terminal closed because Sentinel exited before it shut down cleanly."
                            .to_string(),
                    );
                }
            }
            if let Some(record) = inner.ide.record.as_mut() {
                if matches!(record.state.status, IdeStatus::Starting | IdeStatus::Ready | IdeStatus::Closing) {
                    record.close_requested = true;
                    record.state.status = IdeStatus::Closing;
                    record.state.error = Some(
                        "IDE terminal closed because Sentinel exited before it shut down cleanly."
                            .to_string(),
                    );
                }
            }
            (
                inner
                    .sessions
                    .values()
                    .map(|record| record.summary.clone())
                    .collect::<Vec<_>>(),
                inner
                    .tabs
                    .values()
                    .map(|record| record.summary.clone())
                    .collect::<Vec<_>>(),
                inner
                    .active_workspace_id
                    .clone()
                    .zip(inner.ide.record.as_ref().map(|record| record.state.clone())),
                inner
                    .sessions
                    .values()
                    .map(|record| record.summary.pid)
                    .collect::<Vec<_>>(),
                inner
                    .tabs
                    .values()
                    .map(|record| record.summary.pid)
                    .collect::<Vec<_>>(),
                inner.ide.record.as_ref().and_then(|record| record.state.pid),
                inner.ide.workspace_path.clone(),
            )
        };

        let pool = database_pool(app);
        for mut summary in session_summaries {
            if matches!(
                summary.status,
                SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Closing
            ) {
                summary.status = SessionStatus::Paused;
                if summary.cleanup_state == CleanupState::Active {
                    summary.cleanup_state = CleanupState::Preserved;
                }
                summary.pid = None;
                summary.exit_code = None;
                summary.error = Some(
                    "Session paused because Sentinel exited while this agent was still running."
                        .to_string(),
                );
                summary.metrics = ProcessMetrics::default();
                if let Err(error) = tauri::async_runtime::block_on(SessionRepository::update_status(
                    &pool,
                    &summary.id,
                    summary.status,
                    summary.cleanup_state,
                    summary.exit_code,
                    summary.error.as_deref(),
                )) {
                    log_persistence_error("persist paused session during dispose", &error.to_string());
                }
            }
        }

        for mut summary in tab_summaries {
            if matches!(summary.status, TabStatus::Starting | TabStatus::Ready | TabStatus::Closing) {
                summary.status = TabStatus::Closed;
                summary.pid = None;
                summary.exit_code = None;
                summary.error = Some(
                    "Terminal closed because Sentinel exited before it shut down cleanly."
                        .to_string(),
                );
                summary.metrics = ProcessMetrics::default();
                if let Err(error) = tauri::async_runtime::block_on(TabRepository::update_status(
                    &pool,
                    &summary.id,
                    summary.status,
                    summary.exit_code,
                    summary.error.as_deref(),
                )) {
                    log_persistence_error("persist closed tab during dispose", &error.to_string());
                }
            }
        }

        if let Some((workspace_id, mut state)) = ide_snapshot {
            if matches!(state.status, IdeStatus::Starting | IdeStatus::Ready | IdeStatus::Closing) {
                state.status = IdeStatus::Closed;
                state.pid = None;
                state.exit_code = None;
                state.error = Some(
                    "IDE terminal closed because Sentinel exited before it shut down cleanly."
                        .to_string(),
                );
                if let Err(error) = tauri::async_runtime::block_on(IdeTerminalRepository::upsert(
                    &pool,
                    &workspace_id,
                    &state,
                )) {
                    log_persistence_error("persist closed IDE terminal during dispose", &error.to_string());
                }
            }
        }

        for pid in session_pids {
            let _ = terminate_process_id(pid);
        }
        for pid in tab_pids {
            let _ = terminate_process_id(pid);
        }
        let _ = terminate_process_id(ide_pid);
        if let Some(workspace_path) = ide_workspace {
            let _ = fs::remove_dir_all(workspace_path);
        }
    }
}
