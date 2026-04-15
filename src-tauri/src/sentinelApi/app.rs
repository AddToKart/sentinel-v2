impl SentinelManager {
    pub fn new() -> Self {
        let mut summary = WorkspaceSummary::default();
        summary.last_updated = now_millis();

        Self {
            inner: Mutex::new(SentinelState {
                sessions: HashMap::new(),
                tabs: HashMap::new(),
                ide: IdeRuntime::default(),
                workspaces: HashMap::new(),
                active_workspace_id: None,
                project: ProjectState::default(),
                preferences: WorkspacePreferences::default(),
                workspace_summary: summary,
                activity_log: Vec::new(),
                windows_build_number: parse_windows_build_number(),
            }),
            changes_manager: Arc::new(ChangesManager::new()),
        }
    }

    pub fn start_refresh_loop(self: &Arc<Self>, app: AppHandle) {
        let manager = self.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(METRIC_INTERVAL_MS));
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                manager.refresh_runtime_state(&app);
            }))
            .unwrap_or_else(|e| {
                eprintln!("[sentinel] Panic in refresh loop: {:?}", e);
            });
        });
    }

    pub fn bootstrap(&self, app: &AppHandle) -> Result<BootstrapPayload, String> {
        let (
            workspaces,
            active_workspace_id,
            project,
            summary,
            activity_log,
            preferences,
            windows_build_number,
            live_sessions,
            live_metrics,
            live_histories,
            live_diffs,
            live_tabs,
            live_tab_metrics,
            live_ide_terminal,
        ) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspaces = sorted_workspaces(&inner);
            let live_sessions = inner
                .sessions
                .values()
                .map(|record| (record.summary.id.clone(), record.summary.clone()))
                .collect::<HashMap<_, _>>();
            let live_metrics = inner
                .sessions
                .values()
                .map(|record| {
                    (
                        record.summary.id.clone(),
                        SessionMetricsUpdate {
                            session_id: record.summary.id.clone(),
                            workspace_id: record.summary.workspace_id.clone(),
                            pid: record.summary.pid,
                            process_ids: record.tracked_process_ids.clone(),
                            metrics: record.summary.metrics.clone(),
                            sampled_at: inner.workspace_summary.last_updated,
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            let live_histories = inner
                .sessions
                .values()
                .map(|record| {
                    (
                        record.summary.id.clone(),
                        SessionHistoryUpdate {
                            session_id: record.summary.id.clone(),
                            workspace_id: record.summary.workspace_id.clone(),
                            entries: record.history.clone(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            let live_diffs = inner
                .sessions
                .values()
                .map(|record| {
                    (
                        record.summary.id.clone(),
                        SessionDiffUpdate {
                            session_id: record.summary.id.clone(),
                            workspace_id: record.summary.workspace_id.clone(),
                            modified_paths: record.modified_paths.clone(),
                            updated_at: record.summary.created_at,
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            let live_tabs = inner
                .tabs
                .values()
                .map(|record| (record.summary.id.clone(), record.summary.clone()))
                .collect::<HashMap<_, _>>();
            let live_tab_metrics = inner
                .tabs
                .values()
                .map(|record| {
                    (
                        record.summary.id.clone(),
                        TabMetricsUpdate {
                            tab_id: record.summary.id.clone(),
                            workspace_id: record.summary.workspace_id.clone(),
                            pid: record.summary.pid,
                            process_ids: record.tracked_process_ids.clone(),
                            metrics: record.summary.metrics.clone(),
                            sampled_at: inner.workspace_summary.last_updated,
                        },
                    )
                })
                .collect::<HashMap<_, _>>();

            (
                workspaces,
                inner.active_workspace_id.clone(),
                inner.project.clone(),
                inner.workspace_summary.clone(),
                inner.activity_log.clone(),
                inner.preferences.clone(),
                inner.windows_build_number,
                live_sessions,
                live_metrics,
                live_histories,
                live_diffs,
                live_tabs,
                live_tab_metrics,
                inner.ide.record.as_ref().map(|record| record.state.clone()),
            )
        };

        let workspace_ids = workspaces
            .iter()
            .map(|workspace| workspace.id.clone())
            .collect::<HashSet<_>>();
        let workspace_modes = workspaces
            .iter()
            .map(|workspace| (workspace.id.clone(), workspace.mode))
            .collect::<HashMap<_, _>>();

        let pool = database_pool(app);
        let session_rows = tauri::async_runtime::block_on(
            crate::database::repositories::SessionRepository::find_all(&pool),
        )
        .map_err(|error| format!("Failed to load persisted sessions: {error}"))?;
        let tab_rows = tauri::async_runtime::block_on(
            crate::database::repositories::TabRepository::find_all(&pool),
        )
        .map_err(|error| format!("Failed to load persisted tabs: {error}"))?;

        let mut sessions_by_id = HashMap::<String, SessionSummary>::new();
        let mut metrics_by_id = HashMap::<String, SessionMetricsUpdate>::new();
        let mut histories_by_id = HashMap::<String, SessionHistoryUpdate>::new();
        let mut diffs_by_id = HashMap::<String, SessionDiffUpdate>::new();

        for row in session_rows
            .into_iter()
            .filter(|row| workspace_ids.contains(&row.workspace_id))
        {
            let workspace_mode = workspace_modes
                .get(&row.workspace_id)
                .copied()
                .unwrap_or(WorkspaceMode::Local);
            let summary = session_summary_from_row(row, workspace_mode);
            let session_id = summary.id.clone();
            let workspace_id = summary.workspace_id.clone();
            let command_rows = tauri::async_runtime::block_on(
                crate::database::repositories::CommandRepository::find_by_session(
                    &pool,
                    &session_id,
                    Some(250),
                ),
            )
            .map_err(|error| {
                format!(
                    "Failed to load persisted command history for session {session_id}: {error}"
                )
            })?;
            let file_change_rows = tauri::async_runtime::block_on(
                crate::database::repositories::FileChangeRepository::find_by_session(
                    &pool,
                    &session_id,
                ),
            )
            .map_err(|error| {
                format!("Failed to load persisted file changes for session {session_id}: {error}")
            })?;

            metrics_by_id.insert(
                session_id.clone(),
                SessionMetricsUpdate {
                    session_id: session_id.clone(),
                    workspace_id: workspace_id.clone(),
                    pid: summary.pid,
                    process_ids: Vec::new(),
                    metrics: summary.metrics.clone(),
                    sampled_at: summary.created_at,
                },
            );
            histories_by_id.insert(
                session_id.clone(),
                SessionHistoryUpdate {
                    session_id: session_id.clone(),
                    workspace_id: workspace_id.clone(),
                    entries: session_history_entries_from_rows(command_rows),
                },
            );
            diffs_by_id.insert(
                session_id.clone(),
                if summary.cleanup_state == CleanupState::Removed {
                    SessionDiffUpdate {
                        session_id: session_id.clone(),
                        workspace_id: workspace_id.clone(),
                        modified_paths: Vec::new(),
                        updated_at: summary.created_at,
                    }
                } else {
                    session_diff_snapshot_from_rows(
                        &session_id,
                        &workspace_id,
                        file_change_rows,
                        summary.created_at,
                    )
                },
            );
            sessions_by_id.insert(session_id, summary);
        }

        let mut tabs_by_id = HashMap::<String, TabSummary>::new();
        let mut tab_metrics_by_id = HashMap::<String, TabMetricsUpdate>::new();
        for row in tab_rows
            .into_iter()
            .filter(|row| workspace_ids.contains(&row.workspace_id))
        {
            let summary = tab_summary_from_row(row);
            if matches!(summary.status, TabStatus::Closed | TabStatus::Error) {
                continue;
            }
            let tab_id = summary.id.clone();
            let workspace_id = summary.workspace_id.clone();
            let pid = summary.pid;
            let metrics = summary.metrics.clone();
            let created_at = summary.created_at;

            tabs_by_id.insert(tab_id.clone(), summary);
            tab_metrics_by_id.insert(
                tab_id.clone(),
                TabMetricsUpdate {
                    tab_id,
                    workspace_id,
                    pid,
                    process_ids: Vec::new(),
                    metrics,
                    sampled_at: created_at,
                },
            );
        }

        for (session_id, summary) in live_sessions {
            sessions_by_id.insert(session_id, summary);
        }
        for (session_id, metric) in live_metrics {
            metrics_by_id.insert(session_id, metric);
        }
        for (session_id, history) in live_histories {
            histories_by_id.insert(session_id, history);
        }
        for (session_id, diff) in live_diffs {
            diffs_by_id.insert(session_id, diff);
        }
        for (tab_id, summary) in live_tabs {
            tabs_by_id.insert(tab_id, summary);
        }
        for (tab_id, metric) in live_tab_metrics {
            tab_metrics_by_id.insert(tab_id, metric);
        }

        let persisted_ide_terminal = if let Some(workspace_id) = active_workspace_id.as_deref() {
            tauri::async_runtime::block_on(
                crate::database::repositories::IdeTerminalRepository::find_by_workspace(
                    &pool,
                    workspace_id,
                ),
            )
            .map_err(|error| format!("Failed to load IDE terminal state: {error}"))?
            .map(ide_state_from_row)
            .and_then(|state| {
                if matches!(
                    state.status,
                    IdeStatus::Ready | IdeStatus::Starting | IdeStatus::Closing
                ) {
                    Some(state)
                } else {
                    None
                }
            })
        } else {
            None
        };

        let mut sessions = sessions_by_id.into_values().collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let mut metrics = metrics_by_id.into_values().collect::<Vec<_>>();
        metrics.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let mut histories = histories_by_id.into_values().collect::<Vec<_>>();
        histories.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let mut diffs = diffs_by_id.into_values().collect::<Vec<_>>();
        diffs.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let mut tabs = tabs_by_id.into_values().collect::<Vec<_>>();
        tabs.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let mut tab_metrics = tab_metrics_by_id.into_values().collect::<Vec<_>>();
        tab_metrics.sort_by(|left, right| left.tab_id.cmp(&right.tab_id));

        Ok(BootstrapPayload {
            project,
            workspaces,
            active_workspace_id,
            sessions,
            tabs,
            summary,
            activity_log,
            metrics,
            tab_metrics,
            histories,
            diffs,
            preferences,
            ide_terminal: live_ide_terminal
                .or(persisted_ide_terminal)
                .unwrap_or_else(IdeTerminalState::idle),
            windows_build_number,
            cloud_config: CloudConfig {
                url: "http://127.0.0.1:42069".to_string(),
                enabled: true,
            },
        })
    }

    pub fn set_default_session_strategy(
        &self,
        app: &AppHandle,
        strategy: SessionWorkspaceStrategy,
    ) -> WorkspacePreferences {
        let (preferences, active_workspace) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.preferences.default_session_strategy = strategy;
            if let Some(active_workspace_id) = inner.active_workspace_id.clone() {
                if let Some(workspace) = inner.workspaces.get_mut(&active_workspace_id) {
                    workspace.default_session_strategy = strategy;
                    workspace.last_active_at = now_millis();
                }
            }
            update_workspace_summary(&mut inner);
            (
                inner.preferences.clone(),
                inner
                    .active_workspace_id
                    .as_ref()
                    .and_then(|workspace_id| inner.workspaces.get(workspace_id))
                    .cloned(),
            )
        };

        if let Some(workspace) = active_workspace.as_ref() {
            let _ = self.persist_workspace(app, workspace);
            self.emit_workspace_updated(app, &workspace.id);
        }
        let _ = self.persist_preferences(app);
        self.emit_workspace_state(app);
        preferences
    }

    pub fn load_project(
        &self,
        app: &AppHandle,
        candidate_path: String,
    ) -> Result<ProjectState, String> {
        Ok(self
            .create_workspace(app, candidate_path, None, Some(WorkspaceMode::Local))?
            .project)
    }

    pub fn refresh_project(&self, app: &AppHandle) -> Result<ProjectState, String> {
        let (active_workspace_id, project_path) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (
                inner.active_workspace_id.clone(),
                inner.project.path.clone(),
            )
        };

        match project_path {
            Some(path) => {
                let next_project = inspect_project(Path::new(&path))?;
                let active_workspace_id =
                    active_workspace_id.ok_or_else(|| "Workspace not found.".to_string())?;

                let workspace = {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.project = next_project.clone();
                    sync_active_project_to_workspace(&mut inner);
                    update_workspace_summary(&mut inner);
                    inner.workspaces.get(&active_workspace_id).cloned()
                };

                if let Some(workspace) = workspace.as_ref() {
                    self.persist_workspace(app, workspace)?;
                }

                self.emit_workspace_updated(app, &active_workspace_id);
                self.emit_project_state(app);
                self.emit_workspace_state(app);
                Ok(next_project)
            }
            None => {
                let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                Ok(inner.project.clone())
            }
        }
    }
}
