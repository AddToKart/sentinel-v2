impl SentinelManager {
    pub fn new() -> Self {
        let mut summary = WorkspaceSummary::default();
        summary.last_updated = now_millis();

        Self {
            inner: Mutex::new(SentinelState {
                sessions: HashMap::new(),
                tabs: HashMap::new(),
                ide: IdeRuntime::default(),
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
                entries: record.history.clone(),
            })
            .collect::<Vec<_>>();
        histories.sort_by(|left, right| left.session_id.cmp(&right.session_id));

        let mut diffs = inner
            .sessions
            .values()
            .map(|record| SessionDiffUpdate {
                session_id: record.summary.id.clone(),
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
                pid: record.summary.pid,
                process_ids: record.tracked_process_ids.clone(),
                metrics: record.summary.metrics.clone(),
                sampled_at: inner.workspace_summary.last_updated,
            })
            .collect::<Vec<_>>();

        BootstrapPayload {
            project: inner.project.clone(),
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
            update_workspace_summary(&mut inner);
            inner.preferences.clone()
        };
        self.emit_workspace_state(app);
        preferences
    }

    pub fn load_project(
        &self,
        app: &AppHandle,
        candidate_path: String,
    ) -> Result<ProjectState, String> {
        let next_project = inspect_project(Path::new(&candidate_path))?;
        self.handle_project_changed(app, next_project.path.as_ref().map(PathBuf::from))?;

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project = next_project.clone();
            update_workspace_summary(&mut inner);
        }

        self.emit_workspace_state(app);
        self.emit_project_state(app);
        Ok(next_project)
    }

    pub fn refresh_project(&self, app: &AppHandle) -> Result<ProjectState, String> {
        let project_path = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project.path.clone()
        };

        match project_path {
            Some(path) => self.load_project(app, path),
            None => Ok(self.bootstrap().project),
        }
    }
}
