impl SentinelManager {
    fn handle_session_output(&self, app: &AppHandle, session_id: &str, data: String) {
        let mut emit_state = None;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(record) = inner.sessions.get_mut(session_id) {
                if record.summary.status == SessionStatus::Starting {
                    record.summary.status = SessionStatus::Ready;
                    emit_state = Some(record.summary.clone());
                }
            }
        }

        if let Some(summary) = emit_state {
            emit_event(app, EVENT_SESSION_STATE, &summary);
            self.emit_workspace_state(app);
        }
        emit_event(
            app,
            EVENT_SESSION_OUTPUT,
            &serde_json::json!({ "sessionId": session_id, "data": data }),
        );
    }

    fn handle_ide_output(&self, app: &AppHandle, data: String) {
        let mut emit_state = None;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(record) = inner.ide.record.as_mut() {
                if record.state.status == IdeStatus::Starting {
                    record.state.status = IdeStatus::Ready;
                    emit_state = Some(record.state.clone());
                }
            }
        }

        if let Some(state) = emit_state {
            emit_event(app, EVENT_IDE_STATE, &state);
        }
        emit_event(app, EVENT_IDE_OUTPUT, &serde_json::json!({ "data": data }));
    }

    fn finalize_session(
        &self,
        app: AppHandle,
        session_id: String,
        exit_code: Option<i32>,
        forced_error: Option<String>,
    ) {
        let cleanup_input = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(record) = inner.sessions.get_mut(&session_id) else {
                return;
            };
            if record.finalized {
                return;
            }
            record.finalized = true;
            record.tracked_process_ids.clear();
            record.summary.exit_code = exit_code;
            record.summary.metrics = ProcessMetrics::default();
            record.modified_paths.clear();
            let closed_cleanly = exit_code.unwrap_or(0) == 0;
            record.summary.status = if record.close_requested || closed_cleanly {
                SessionStatus::Closed
            } else {
                SessionStatus::Error
            };
            record.summary.error = forced_error.clone().or_else(|| {
                if !record.close_requested && !closed_cleanly {
                    Some(format!(
                        "PowerShell exited unexpectedly with code {}.",
                        exit_code.unwrap_or_default()
                    ))
                } else {
                    None
                }
            });
            (
                record.summary.project_root.clone(),
                record.summary.workspace_path.clone(),
                record.summary.workspace_strategy,
                record.summary.branch_name.clone(),
            )
        };

        let cleanup_result = cleanup_session_workspace(
            &app,
            Path::new(&cleanup_input.0),
            Path::new(&cleanup_input.1),
            cleanup_input.2,
            cleanup_input.3.as_deref(),
        );

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(record) = inner.sessions.get_mut(&session_id) {
                match cleanup_result {
                    Ok(cleanup_state) => {
                        record.summary.cleanup_state = cleanup_state;
                    }
                    Err(error) => {
                        record.summary.cleanup_state = CleanupState::Failed;
                        if record.summary.error.is_none() {
                            record.summary.error = Some(error);
                        }
                    }
                }
                update_workspace_summary(&mut inner);
            }
        }

        self.emit_session_metrics(&app, &session_id);
        self.emit_session_diff(&app, &session_id);
        self.emit_session_state(&app, &session_id);
        self.emit_workspace_state(&app);
    }

    fn finalize_ide_terminal(
        &self,
        app: AppHandle,
        exit_code: Option<i32>,
        forced_error: Option<String>,
    ) {
        let state = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(record) = inner.ide.record.as_mut() else {
                return;
            };
            if record.finalized {
                return;
            }
            record.finalized = true;
            let closed_cleanly = exit_code.unwrap_or(0) == 0;
            record.state.exit_code = exit_code;
            record.state.status = if record.close_requested || closed_cleanly {
                IdeStatus::Closed
            } else {
                IdeStatus::Error
            };
            record.state.error = forced_error.or_else(|| {
                if !record.close_requested && !closed_cleanly {
                    Some(format!(
                        "PowerShell exited unexpectedly with code {}.",
                        exit_code.unwrap_or_default()
                    ))
                } else {
                    None
                }
            });
            record.state.clone()
        };
        emit_event(&app, EVENT_IDE_STATE, &state);
    }

    fn refresh_runtime_state(&self, app: &AppHandle) {
        // Refresh tab metrics
        self.refresh_tab_metrics(app);

        let active_session_ids = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .sessions
                .values()
                .filter(|record| {
                    matches!(
                        record.summary.status,
                        SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Closing
                    )
                })
                .map(|record| record.summary.id.clone())
                .collect::<Vec<_>>()
        };

        if active_session_ids.is_empty() {
            let _ = self.refresh_ide_workspace_diffs(app);
            self.emit_workspace_state(app);
            return;
        }

        let root_ids = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            active_session_ids
                .iter()
                .filter_map(|session_id| {
                    inner
                        .sessions
                        .get(session_id)
                        .and_then(|record| record.summary.pid)
                })
                .collect::<Vec<_>>()
        };

        let snapshot_map = collect_process_tree_snapshots(&root_ids).unwrap_or_default();
        let sampled_at = now_millis();

        for session_id in &active_session_ids {
            let maybe_snapshot = {
                let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner
                    .sessions
                    .get(session_id)
                    .and_then(|record| record.summary.pid)
                    .and_then(|pid| snapshot_map.get(&pid).cloned())
            };

            {
                let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(record) = inner.sessions.get_mut(session_id) {
                    if let Some(snapshot) = maybe_snapshot {
                        let cpu_percent =
                            match (record.last_cpu_total_seconds, record.last_sampled_at) {
                                (Some(previous_cpu), Some(previous_sampled_at)) => {
                                    // Guard against clock skew (sampled_at < previous_sampled_at on Windows)
                                    if sampled_at <= previous_sampled_at {
                                        0.0
                                    } else {
                                        let cpu_delta = snapshot.cpu_total_seconds - previous_cpu;
                                        let elapsed = (sampled_at - previous_sampled_at) as f64 / 1000.0;
                                        // Ensure elapsed is positive and non-zero
                                        if elapsed > 0.001 {
                                            round(cpu_delta.max(0.0) / elapsed * 100.0, 1)
                                        } else {
                                            0.0
                                        }
                                    }
                                }
                                _ => 0.0,
                            };

                        record.tracked_process_ids = snapshot.process_ids.clone();
                        record.summary.metrics = ProcessMetrics {
                            cpu_percent,
                            memory_mb: round(
                                snapshot.working_set_bytes as f64 / 1024.0 / 1024.0,
                                1,
                            ),
                            thread_count: snapshot.thread_count,
                            handle_count: snapshot.handle_count,
                            process_count: snapshot.process_count,
                        };
                        record.last_cpu_total_seconds = Some(snapshot.cpu_total_seconds);
                        record.last_sampled_at = Some(sampled_at);
                    } else {
                        record.summary.metrics = ProcessMetrics::default();
                        record.tracked_process_ids.clear();
                    }
                }
            }
            self.emit_session_metrics(app, session_id);
        }

        let session_updates = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let mut updates = Vec::new();
            for session_id in &active_session_ids {
                if let Some(record) = inner.sessions.get_mut(session_id) {
                    let next_modified_paths =
                        collect_workspace_diffs_for_record(record).unwrap_or_default();
                    if next_modified_paths != record.modified_paths {
                        record.modified_paths = next_modified_paths.clone();
                        updates.push(session_id.clone());
                    }
                }
            }
            update_workspace_summary(&mut inner);
            updates
        };

        for session_id in session_updates {
            self.emit_session_diff(app, &session_id);
        }
        let _ = self.refresh_ide_workspace_diffs(app);
        self.emit_workspace_state(app);
    }

    fn refresh_ide_workspace_diffs(&self, app: &AppHandle) -> Result<(), String> {
        let (workspace_path, sandbox_state) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (
                inner.ide.workspace_path.clone(),
                inner.ide.sandbox_state.clone(),
            )
        };
        let (Some(workspace_path), Some(sandbox_state)) = (workspace_path, sandbox_state) else {
            return Ok(());
        };

        let (modified_paths, next_cache) =
            refresh_sandbox_workspace_diffs(&workspace_path, &sandbox_state)?;
        let should_emit = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.ide.sandbox_state = Some(SandboxWorkspaceState {
                baseline_hashes: sandbox_state.baseline_hashes,
                scan_cache: next_cache,
            });
            if let Some(record) = inner.ide.record.as_mut() {
                if record.state.modified_paths != modified_paths {
                    record.state.modified_paths = modified_paths;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if should_emit {
            self.emit_ide_state(app);
        }
        Ok(())
    }

    fn emit_session_state(&self, app: &AppHandle, session_id: &str) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .sessions
                .get(session_id)
                .map(|record| record.summary.clone())
        };
        if let Some(payload) = payload {
            emit_event(app, EVENT_SESSION_STATE, &payload);
        }
    }

    fn emit_project_state(&self, app: &AppHandle) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project.clone()
        };
        emit_event(app, EVENT_PROJECT_STATE, &payload);
    }

    fn emit_ide_state(&self, app: &AppHandle) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .ide
                .record
                .as_ref()
                .map(|record| record.state.clone())
                .unwrap_or_else(IdeTerminalState::idle)
        };
        emit_event(app, EVENT_IDE_STATE, &payload);
    }

    fn emit_session_metrics(&self, app: &AppHandle, session_id: &str) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .sessions
                .get(session_id)
                .map(|record| SessionMetricsUpdate {
                    session_id: record.summary.id.clone(),
                    pid: record.summary.pid,
                    process_ids: record.tracked_process_ids.clone(),
                    metrics: record.summary.metrics.clone(),
                    sampled_at: now_millis(),
                })
        };
        if let Some(payload) = payload {
            emit_event(app, EVENT_SESSION_METRICS, &payload);
        }
    }

    fn emit_session_history(&self, app: &AppHandle, session_id: &str) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .sessions
                .get(session_id)
                .map(|record| SessionHistoryUpdate {
                    session_id: record.summary.id.clone(),
                    entries: record.history.clone(),
                })
        };
        if let Some(payload) = payload {
            emit_event(app, EVENT_SESSION_HISTORY, &payload);
        }
    }

    fn emit_session_diff(&self, app: &AppHandle, session_id: &str) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .sessions
                .get(session_id)
                .map(|record| SessionDiffUpdate {
                    session_id: record.summary.id.clone(),
                    modified_paths: record.modified_paths.clone(),
                    updated_at: now_millis(),
                })
        };
        if let Some(payload) = payload {
            emit_event(app, EVENT_SESSION_DIFF, &payload);
        }
    }

    fn emit_workspace_state(&self, app: &AppHandle) {
        let summary = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            update_workspace_summary(&mut inner);
            inner.workspace_summary.clone()
        };
        emit_event(app, EVENT_WORKSPACE_STATE, &summary);
    }

    fn push_activity_log(
        &self,
        app: &AppHandle,
        scope: &str,
        status: &str,
        command: &str,
        cwd: String,
        detail: Option<String>,
    ) {
        let entry = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let entry = ActivityLogEntry {
                id: format!("{}-{}", create_timestamp(), create_token()),
                timestamp: now_millis(),
                scope: scope.to_string(),
                status: status.to_string(),
                command: command.to_string(),
                cwd,
                detail,
            };
            inner.activity_log.insert(0, entry.clone());
            if inner.activity_log.len() > 120 {
                inner.activity_log.truncate(120);
            }
            entry
        };
        emit_event(app, EVENT_ACTIVITY_LOG, &entry);
    }
}
