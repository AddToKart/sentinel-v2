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
        }
    }

    pub fn start_refresh_loop(self: &Arc<Self>, app: AppHandle) {
        let manager = self.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(METRIC_INTERVAL_MS));
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    manager.refresh_runtime_state(&app);
                })).unwrap_or_else(|e| {
                    eprintln!("[sentinel] Panic in refresh loop: {:?}", e);
                });
            }
        });
    }

    pub fn bootstrap(&self) -> BootstrapPayload {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let workspaces = sorted_workspaces(&inner);

        let mut sessions = inner
            .sessions
            .values()
            .map(|record| record.summary.clone())
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let mut metrics = inner
            .sessions
            .values()
            .map(|record| SessionMetricsUpdate {
                session_id: record.summary.id.clone(),
                workspace_id: record.summary.workspace_id.clone(),
                pid: record.summary.pid,
                process_ids: record.tracked_process_ids.clone(),
                metrics: record.summary.metrics.clone(),
                sampled_at: inner.workspace_summary.last_updated,
            })
            .collect::<Vec<_>>();
        metrics.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let mut histories = inner
            .sessions
            .values()
            .map(|record| SessionHistoryUpdate {
                session_id: record.summary.id.clone(),
                workspace_id: record.summary.workspace_id.clone(),
                entries: record.history.clone(),
            })
            .collect::<Vec<_>>();
        histories.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let mut diffs = inner
            .sessions
            .values()
            .map(|record| SessionDiffUpdate {
                session_id: record.summary.id.clone(),
                workspace_id: record.summary.workspace_id.clone(),
                modified_paths: record.modified_paths.clone(),
                updated_at: record.summary.created_at,
            })
            .collect::<Vec<_>>();
        diffs.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let tabs = inner
            .tabs
            .values()
            .map(|record| record.summary.clone())
            .collect::<Vec<_>>();

        let tab_metrics = inner
            .tabs
            .values()
            .map(|record| TabMetricsUpdate {
                tab_id: record.summary.id.clone(),
                workspace_id: record.summary.workspace_id.clone(),
                pid: record.summary.pid,
                process_ids: record.tracked_process_ids.clone(),
                metrics: record.summary.metrics.clone(),
                sampled_at: inner.workspace_summary.last_updated,
            })
            .collect::<Vec<_>>();

        BootstrapPayload {
            project: inner.project.clone(),
            workspaces,
            active_workspace_id: inner.active_workspace_id.clone(),
            sessions,
            tabs,
            summary: inner.workspace_summary.clone(),
            activity_log: inner.activity_log.clone(),
            metrics,
            tab_metrics,
            histories,
            diffs,
            preferences: inner.preferences.clone(),
            ide_terminal: inner
                .ide
                .record
                .as_ref()
                .map(|record| record.state.clone())
                .unwrap_or_else(IdeTerminalState::idle),
            windows_build_number: inner.windows_build_number,
        }
    }

    pub fn set_default_session_strategy(
        &self,
        app: &AppHandle,
        strategy: SessionWorkspaceStrategy,
    ) -> WorkspacePreferences {
        let preferences = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.preferences.default_session_strategy = strategy;
            if let Some(active_workspace_id) = inner.active_workspace_id.clone() {
                if let Some(workspace) = inner.workspaces.get_mut(&active_workspace_id) {
                    workspace.default_session_strategy = strategy;
                    workspace.last_active_at = now_millis();
                }
            }
            update_workspace_summary(&mut inner);
            inner.preferences.clone()
        };
        if let Some(workspace) = self.get_active_workspace() {
            self.emit_workspace_updated(app, &workspace.id);
        }
        self.emit_workspace_state(app);
        preferences
    }

    pub fn load_project(
        &self,
        app: &AppHandle,
        candidate_path: String,
    ) -> Result<ProjectState, String> {
        Ok(self.create_workspace(app, candidate_path, None)?.project)
    }

    pub fn refresh_project(&self, app: &AppHandle) -> Result<ProjectState, String> {
        let (active_workspace_id, project_path) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (inner.active_workspace_id.clone(), inner.project.path.clone())
        };

        match project_path {
            Some(path) => {
                let next_project = inspect_project(Path::new(&path))?;
                let active_workspace_id = active_workspace_id
                    .ok_or_else(|| "Workspace not found.".to_string())?;

                {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.project = next_project.clone();
                    sync_active_project_to_workspace(&mut inner);
                    update_workspace_summary(&mut inner);
                }

                self.emit_workspace_updated(app, &active_workspace_id);
                self.emit_project_state(app);
                self.emit_workspace_state(app);
                Ok(next_project)
            }
            None => Ok(self.bootstrap().project),
        }
    }
}
