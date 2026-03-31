impl SentinelManager {
    pub fn create_standalone_terminal(
        self: &Arc<Self>,
        app: &AppHandle,
        cwd: Option<String>,
        label: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<TabSummary, String> {
        let (workspace_id, default_cwd, tab_count) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace = active_workspace_clone(&inner)
                .ok_or_else(|| "Open a project workspace before starting a terminal.".to_string())?;
            let default_cwd = workspace
                .project
                .path
                .clone()
                .ok_or_else(|| "Workspace project root is unavailable.".to_string())?;
            (workspace.id, default_cwd, workspace.tab_ids.len())
        };
        let tab_id = generate_id();

        let root_dir = PathBuf::from(cwd.unwrap_or(default_cwd));

        let handles = self.spawn_standalone_terminal(
            app.clone(),
            tab_id.clone(),
            root_dir.clone(),
            cols,
            rows,
        )?;

        let now = now_millis();
        let summary = TabSummary {
            id: tab_id.clone(),
            workspace_id: workspace_id.clone(),
            tab_type: TabType::Terminal,
            label: label.unwrap_or_else(|| format!("Terminal {}", tab_count + 1)),
            status: TabStatus::Starting,
            cwd: path_to_string(&root_dir),
            shell: "powershell.exe".to_string(),
            pid: handles.pid,
            created_at: now,
            exit_code: None,
            error: None,
            metrics: ProcessMetrics::default(),
        };

        let record = TabRecord {
            summary: summary.clone(),
            master: handles.master,
            writer: handles.writer,
            killer: handles.killer,
            terminal_size: TerminalSize { cols, rows },
            close_requested: false,
            finalized: false,
            tracked_process_ids: Vec::new(),
            last_cpu_total_seconds: None,
            last_sampled_at: None,
            last_persisted_metrics: summary.metrics.clone(),
            last_persisted_metrics_at: Some(summary.created_at),
        };

        let workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(workspace) = inner.workspaces.get_mut(&workspace_id) {
                workspace.tab_ids.push(summary.id.clone());
                workspace.last_active_at = now_millis();
            }
            inner.tabs.insert(tab_id.clone(), record);
            update_workspace_summary(&mut inner);
            inner.workspaces.get(&workspace_id).cloned()
        };

        self.persist_tab_created(app, &summary)?;
        if let Some(workspace) = workspace.as_ref() {
            self.persist_workspace(app, workspace)?;
        }
        self.persist_audit_event(
            app,
            Some(&summary.workspace_id),
            None,
            Some(&summary.id),
            "tab-created",
            "tab",
            &summary.id,
            Some(serde_json::json!({
                "label": summary.label.clone(),
                "cwd": summary.cwd.clone(),
            })),
        );

        emit_event(
            app,
            EVENT_TAB_STATE,
            &TabStateUpdate {
                tab_id: tab_id.clone(),
                workspace_id: workspace_id.clone(),
                status: TabStatus::Starting,
                pid: handles.pid,
                exit_code: None,
                error: None,
            },
        );
        self.emit_workspace_updated(app, &workspace_id);
        self.emit_workspace_state(app);

        Ok(summary)
    }

    pub fn close_tab(self: &Arc<Self>, app: &AppHandle, tab_id: &str) -> Result<(), String> {
        let (pid, killer, should_wait_for_shutdown, workspace_id, summary) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .tabs
                .get_mut(tab_id)
                .ok_or_else(|| "Tab not found".to_string())?;

            if record.close_requested || record.finalized {
                return Ok(());
            }

            record.close_requested = true;
            record.summary.status = TabStatus::Closing;
            record.summary.error = None;

            (
                record.summary.pid,
                record.killer.clone(),
                true,
                record.summary.workspace_id.clone(),
                record.summary.clone(),
            )
        };

        self.persist_tab_status(app, &summary)?;
        self.persist_audit_event(
            app,
            Some(&summary.workspace_id),
            None,
            Some(&summary.id),
            "tab-close-requested",
            "tab",
            &summary.id,
            None,
        );

        let _ = kill_process_tree(&killer);
        let _ = terminate_process_id(pid);

        emit_event(
            app,
            EVENT_TAB_STATE,
            &TabStateUpdate {
                tab_id: tab_id.to_string(),
                workspace_id,
                status: TabStatus::Closing,
                pid,
                exit_code: None,
                error: None,
            },
        );

        if should_wait_for_shutdown {
            let manager = self.clone();
            let app_handle = app.clone();
            let tab_id_owned = tab_id.to_string();

            thread::spawn(move || {
                let start = now_millis();
                loop {
                    let done = {
                        let inner = manager.inner.lock().unwrap_or_else(|e| e.into_inner());
                        inner
                            .tabs
                            .get(&tab_id_owned)
                            .map(|record| record.finalized)
                            .unwrap_or(true)
                    };

                    if done {
                        break;
                    }

                    if now_millis() - start >= CLOSE_TIMEOUT_MS as i64 {
                        let _ = kill_process_tree(&killer);
                        manager.finalize_tab(
                            app_handle,
                            tab_id_owned,
                            None,
                            Some("Sentinel forced the tab terminal to close after the shell stopped responding.".to_string()),
                        );
                        break;
                    }

                    thread::sleep(Duration::from_millis(80));
                }
            });
        }

        Ok(())
    }

    pub fn resize_tab(self: &Arc<Self>, tab_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }

        let master = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .tabs
                .get_mut(tab_id)
                .ok_or_else(|| "Tab not found".to_string())?;

            if record.terminal_size.cols == cols && record.terminal_size.rows == rows {
                return Ok(());
            }

            record.terminal_size = TerminalSize { cols, rows };
            record.master.clone()
        };

        resize_terminal(&master, cols, rows)
    }

    pub fn send_tab_input(self: &Arc<Self>, tab_id: &str, data: &str) -> Result<(), String> {
        let writer = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let record = inner
                .tabs
                .get(tab_id)
                .ok_or_else(|| "Tab not found".to_string())?;
            record.writer.clone()
        };

        match write_terminal(&writer, data.as_bytes()) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("[sentinel] Failed to send input to tab {}: {}", tab_id, e);
                Err(e)
            }
        }
    }

    fn spawn_standalone_terminal(
        self: &Arc<Self>,
        app: AppHandle,
        tab_id: String,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalHandles, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;

        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.arg("-NoLogo");
        cmd.cwd(&cwd);
        cmd.env("FORCE_COLOR", "1");
        cmd.env("SENTINEL_TAB_ID", &tab_id);
        cmd.env("SENTINEL_TAB_TYPE", "standalone-terminal");

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|error| error.to_string())?;
        let pid = child.process_id();
        let killer = child.clone_killer();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let killer = Arc::new(Mutex::new(killer));

        // Output reader thread
        {
            let manager = self.clone();
            let app_handle = app.clone();
            let event_tab_id = tab_id.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let chunk = String::from_utf8_lossy(&buffer[..size]).to_string();
                            manager.handle_tab_output(&app_handle, &event_tab_id, chunk);
                        }
                        Err(e) => {
                            eprintln!("[sentinel] Tab {} I/O error: {}", event_tab_id, e);
                            break;
                        }
                    }
                }
            });
        }

        // Exit handler thread
        {
            let manager = self.clone();
            let app_handle = app.clone();
            let event_tab_id = tab_id.clone();
            thread::spawn(move || {
                match child.wait() {
                    Ok(status) => {
                        let exit_code = Some(status.exit_code() as i32);
                        manager.finalize_tab(app_handle, event_tab_id, exit_code, None);
                    }
                    Err(e) => {
                        eprintln!("[sentinel] Tab {} wait error: {}", event_tab_id, e);
                        // Still finalize the tab even if we can't get exit code
                        manager.finalize_tab(app_handle, event_tab_id, None, Some(e.to_string()));
                    }
                }
            });
        }

        Ok(TerminalHandles {
            master,
            writer,
            killer,
            pid,
        })
    }

    fn handle_tab_output(&self, app: &AppHandle, tab_id: &str, chunk: String) {
        let ready_summary = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(record) = inner.tabs.get_mut(tab_id) else {
                return;
            };

            if record.summary.status == TabStatus::Starting {
                record.summary.status = TabStatus::Ready;
                Some(record.summary.clone())
            } else {
                None
            }
        };

        if let Some(summary) = ready_summary.as_ref() {
            if let Err(error) = self.persist_tab_status(app, summary) {
                log_persistence_error("persist ready tab state", &error);
            }
            emit_event(
                app,
                EVENT_TAB_STATE,
                &TabStateUpdate {
                    tab_id: summary.id.clone(),
                    workspace_id: summary.workspace_id.clone(),
                    status: summary.status,
                    pid: summary.pid,
                    exit_code: summary.exit_code,
                    error: summary.error.clone(),
                },
            );
            self.emit_workspace_state(app);
        }

        emit_event(
            app,
            EVENT_TAB_OUTPUT,
            &TabOutputEvent {
                tab_id: tab_id.to_string(),
                data: chunk,
            },
        );
    }

    fn finalize_tab(
        self: &Arc<Self>,
        app: AppHandle,
        tab_id: String,
        exit_code: Option<i32>,
        forced_error: Option<String>,
    ) {
        // Use a single lock for the entire operation
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        let Some(record) = inner.tabs.get_mut(&tab_id) else {
            return;
        };

        if record.finalized {
            return;
        }

        record.finalized = true;
        record.tracked_process_ids.clear();
        record.summary.metrics = ProcessMetrics::default();
        let closed_cleanly = exit_code.unwrap_or(0) == 0;
        record.summary.status = if record.close_requested || closed_cleanly {
            TabStatus::Closed
        } else {
            TabStatus::Error
        };
        record.summary.exit_code = exit_code;
        record.summary.error = forced_error.or_else(|| {
            if !record.close_requested && !closed_cleanly {
                Some(format!(
                    "PowerShell exited unexpectedly with code {}.",
                    exit_code.unwrap_or_default()
                ))
            } else {
                None
            }
        });

        let workspace_id = record.summary.workspace_id.clone();
        let should_emit = !record.close_requested;

        let final_status = record.summary.status.clone();
        let final_summary = record.summary.clone();

        let state_update = TabStateUpdate {
            tab_id: tab_id.clone(),
            workspace_id: workspace_id.clone(),
            status: final_status,
            pid: record.summary.pid,
            exit_code,
            error: record.summary.error.clone(),
        };

        inner.tabs.remove(&tab_id);
        if let Some(workspace) = inner.workspaces.get_mut(&workspace_id) {
            workspace.last_active_at = now_millis();
        }
        update_workspace_summary(&mut inner);
        let workspace = inner.workspaces.get(&workspace_id).cloned();

        drop(inner);

        if let Err(error) = self.persist_tab_status(&app, &final_summary) {
            log_persistence_error("persist finalized tab state", &error);
        }
        if let Some(workspace) = workspace.as_ref() {
            if let Err(error) = self.persist_workspace(&app, workspace) {
                log_persistence_error("persist workspace after tab removal", &error);
            }
        }
        self.persist_audit_event(
            &app,
            Some(&final_summary.workspace_id),
            None,
            Some(&final_summary.id),
            "tab-finalized",
            "tab",
            &final_summary.id,
            Some(serde_json::json!({
                "status": final_summary.status,
                "exitCode": final_summary.exit_code,
                "error": final_summary.error,
            })),
        );

        if should_emit {
            emit_event(&app, EVENT_TAB_STATE, &state_update);
        }
        self.emit_workspace_updated(&app, &workspace_id);
        self.emit_workspace_state(&app);
    }

    pub fn refresh_tab_metrics(&self, app: &AppHandle) {
        let tab_ids: Vec<String> = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .tabs
                .iter()
                .filter(|(_, record)| !record.finalized && record.summary.pid.is_some())
                .map(|(id, _)| id.clone())
                .collect()
        };

        for tab_id in tab_ids {
            self.sample_tab_metrics(app, &tab_id);
        }
    }

    fn sample_tab_metrics(&self, app: &AppHandle, tab_id: &str) {
        let (pid, workspace_id, last_cpu, last_sampled) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(record) = inner.tabs.get(tab_id) else {
                return;
            };

            let pid = match record.summary.pid {
                Some(p) => p,
                None => return,
            };

            (
                pid,
                record.summary.workspace_id.clone(),
                record.last_cpu_total_seconds,
                record.last_sampled_at,
            )
        };

        let snapshot = match capture_process_tree_snapshot(pid) {
            Some(s) => s,
            None => return,
        };

        let now = now_millis();
        let cpu_percent = if let (Some(last_cpu), Some(last_at)) = (last_cpu, last_sampled) {
            let elapsed = (now - last_at) as f64 / 1000.0;
            if elapsed > 0.0 {
                ((snapshot.cpu_total_seconds - last_cpu) / elapsed * 100.0).max(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let memory_mb = snapshot.working_set_bytes as f64 / 1_048_576.0;

        let metrics = ProcessMetrics {
            cpu_percent,
            memory_mb,
            thread_count: snapshot.thread_count,
            handle_count: snapshot.handle_count,
            process_count: snapshot.process_count,
        };

        let (summary, should_persist_metrics_now) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(record) = inner.tabs.get_mut(tab_id) else {
                return;
            };

            record.summary.metrics = metrics.clone();
            record.tracked_process_ids = snapshot.process_ids.clone();
            record.last_cpu_total_seconds = Some(snapshot.cpu_total_seconds);
            record.last_sampled_at = Some(now);
            let should_persist_metrics_now = should_persist_metrics(
                &record.last_persisted_metrics,
                &record.summary.metrics,
                record.last_persisted_metrics_at,
                now,
            );
            if should_persist_metrics_now {
                record.last_persisted_metrics = record.summary.metrics.clone();
                record.last_persisted_metrics_at = Some(now);
            }
            (Some(record.summary.clone()), should_persist_metrics_now)
        };

        if should_persist_metrics_now {
            if let Some(summary) = summary.as_ref() {
                if let Err(error) = self.persist_tab_metrics(app, summary, now) {
                    log_persistence_error("persist tab metrics", &error);
                }
            }
        }

        emit_event(
            app,
            EVENT_TAB_METRICS,
            &TabMetricsUpdate {
                tab_id: tab_id.to_string(),
                workspace_id,
                pid: Some(pid),
                process_ids: snapshot.process_ids,
                metrics,
                sampled_at: now,
            },
        );
    }
}
