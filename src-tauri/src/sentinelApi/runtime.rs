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
            if let Err(error) = self.persist_session_status(app, &summary) {
                log_persistence_error("persist ready session state", &error);
            }
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
        let mut workspace_id = None;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(record) = inner.ide.record.as_mut() {
                if record.state.status == IdeStatus::Starting {
                    record.state.status = IdeStatus::Ready;
                    emit_state = Some(record.state.clone());
                    workspace_id = inner.active_workspace_id.clone();
                }
            }
        }

        if let Some(state) = emit_state {
            if let Some(workspace_id) = workspace_id.as_deref() {
                if let Err(error) = self.persist_ide_state(app, workspace_id, &state) {
                    log_persistence_error("persist ready IDE terminal state", &error);
                }
            }
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

        let final_summary = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let mut final_summary = None;
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
                final_summary = Some(record.summary.clone());
            }
            update_workspace_summary(&mut inner);
            final_summary
        };

        if let Some(summary) = final_summary.as_ref() {
            if let Err(error) = self.persist_session_status(&app, summary) {
                log_persistence_error("persist finalized session state", &error);
            }
            self.persist_audit_event(
                &app,
                Some(&summary.workspace_id),
                Some(&summary.id),
                None,
                "session-finalized",
                "session",
                &summary.id,
                Some(serde_json::json!({
                    "status": summary.status,
                    "cleanupState": summary.cleanup_state,
                    "exitCode": summary.exit_code,
                    "error": summary.error,
                })),
            );
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
        let (state, workspace_id) = {
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
            (record.state.clone(), inner.active_workspace_id.clone())
        };
        if let Some(workspace_id) = workspace_id.as_deref() {
            if let Err(error) = self.persist_ide_state(&app, workspace_id, &state) {
                log_persistence_error("persist finalized IDE terminal state", &error);
            }
        }
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

            let summary = {
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
                                        let elapsed =
                                            (sampled_at - previous_sampled_at) as f64 / 1000.0;
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
                    Some(record.summary.clone())
                } else {
                    None
                }
            };
            if let Some(summary) = summary.as_ref() {
                if let Err(error) = self.persist_session_metrics(app, summary, sampled_at) {
                    log_persistence_error("persist session metrics", &error);
                }
            }
            self.emit_session_metrics(app, session_id);
        }

        let session_updates = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let mut updates = Vec::new();
            for session_id in &active_session_ids {
                if let Some(record) = inner.sessions.get_mut(session_id) {
                    if matches!(record.summary.status, SessionStatus::Closing) {
                        continue;
                    }

                    let should_scan_diff = record
                        .last_diff_scanned_at
                        .map(|last_scanned_at| sampled_at - last_scanned_at >= DIFF_INTERVAL_MS)
                        .unwrap_or(true);
                    if !should_scan_diff {
                        continue;
                    }

                    record.last_diff_scanned_at = Some(sampled_at);
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
        let workspace_path = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.ide.workspace_path.clone()
        };
        let Some(workspace_path) = workspace_path else {
            return Ok(());
        };

        // Get mutable access to sandbox_state for lazy loading
        let (modified_paths, _next_cache) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let sandbox_state = inner
                .ide
                .sandbox_state
                .as_mut()
                .ok_or("Sandbox state unavailable")?;
            refresh_sandbox_workspace_diffs(&workspace_path, sandbox_state)?
        };

        let persisted_state = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace_id = inner.active_workspace_id.clone();
            if let Some(record) = inner.ide.record.as_mut() {
                if record.state.modified_paths != modified_paths {
                    record.state.modified_paths = modified_paths;
                    workspace_id.map(|workspace_id| (workspace_id, record.state.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some((workspace_id, state)) = persisted_state.as_ref() {
            if let Err(error) = self.persist_ide_state(app, workspace_id, state) {
                log_persistence_error("persist IDE terminal diff state", &error);
            }
        }
        if persisted_state.is_some() {
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
                    workspace_id: record.summary.workspace_id.clone(),
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
                    workspace_id: record.summary.workspace_id.clone(),
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
                    workspace_id: record.summary.workspace_id.clone(),
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
        let (entry, workspace_id) = {
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
            (entry, inner.active_workspace_id.clone())
        };

        if let Some(workspace_id) = workspace_id.as_deref() {
            self.persist_activity_entry(app, workspace_id, &entry, None);
        }
        emit_event(app, EVENT_ACTIVITY_LOG, &entry);
    }
}
