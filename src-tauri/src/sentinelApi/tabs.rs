impl SentinelManager {
    pub fn create_standalone_terminal(
        self: &Arc<Self>,
        app: &AppHandle,
        cols: u16,
        rows: u16,
    ) -> Result<TabSummary, String> {
        let tab_id = generate_id();

        // Spawn terminal at root directory
        let root_dir = if cfg!(windows) {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };

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
            tab_type: TabType::Terminal,
            label: format!("Terminal {}", self.get_next_terminal_number()),
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
        };

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.tabs.insert(tab_id.clone(), record);
        }

        emit_event(
            app,
            EVENT_TAB_STATE,
            &TabStateUpdate {
                tab_id: tab_id.clone(),
                status: TabStatus::Starting,
                pid: handles.pid,
                exit_code: None,
                error: None,
            },
        );

        Ok(summary)
    }

    pub fn close_tab(self: &Arc<Self>, app: &AppHandle, tab_id: &str) -> Result<(), String> {
        let (pid, killer, should_wait_for_shutdown) = {
            let mut inner = self.inner.lock().expect("state poisoned");
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

            (record.summary.pid, record.killer.clone(), true)
        };

        let _ = kill_process_tree(&killer);
        let _ = terminate_process_id(pid);

        emit_event(
            app,
            EVENT_TAB_STATE,
            &TabStateUpdate {
                tab_id: tab_id.to_string(),
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
                        let inner = manager.inner.lock().expect("state poisoned");
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
            let mut inner = self.inner.lock().expect("state poisoned");
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
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .tabs
                .get(tab_id)
                .ok_or_else(|| "Tab not found".to_string())?;
            record.writer.clone()
        };

        write_terminal(&writer, data.as_bytes())
    }

    fn get_next_terminal_number(&self) -> usize {
        let inner = self.inner.lock().expect("state poisoned");
        inner
            .tabs
            .values()
            .filter(|r| r.summary.tab_type == TabType::Terminal)
            .count()
            + 1
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
                        Err(_) => break,
                    }
                }
            });
        }

        // Exit handler thread
        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let exit_code = child.wait().ok().map(|status| status.exit_code() as i32);
                manager.finalize_tab(app_handle, tab_id, exit_code, None);
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
        let mut inner = self.inner.lock().expect("state poisoned");

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

        let should_emit = !record.close_requested;

        let final_status = record.summary.status.clone();

        let state_update = TabStateUpdate {
            tab_id: tab_id.clone(),
            status: final_status,
            pid: record.summary.pid,
            exit_code,
            error: record.summary.error.clone(),
        };

        // Remove the tab while still holding the lock
        inner.tabs.remove(&tab_id);

        // Drop the lock explicitly before emitting to avoid holding lock during I/O
        drop(inner);

        if should_emit {
            emit_event(&app, EVENT_TAB_STATE, &state_update);
        }
    }

    pub fn refresh_tab_metrics(&self, app: &AppHandle) {
        let tab_ids: Vec<String> = {
            let inner = self.inner.lock().expect("state poisoned");
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
        let (pid, last_cpu, last_sampled) = {
            let inner = self.inner.lock().expect("state poisoned");
            let Some(record) = inner.tabs.get(tab_id) else {
                return;
            };

            let pid = match record.summary.pid {
                Some(p) => p,
                None => return,
            };

            (pid, record.last_cpu_total_seconds, record.last_sampled_at)
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

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            let Some(record) = inner.tabs.get_mut(tab_id) else {
                return;
            };

            record.summary.metrics = metrics.clone();
            record.tracked_process_ids = snapshot.process_ids.clone();
            record.last_cpu_total_seconds = Some(snapshot.cpu_total_seconds);
            record.last_sampled_at = Some(now);
        }

        emit_event(
            app,
            EVENT_TAB_METRICS,
            &TabMetricsUpdate {
                tab_id: tab_id.to_string(),
                pid: Some(pid),
                process_ids: snapshot.process_ids,
                metrics,
                sampled_at: now,
            },
        );
    }
}
