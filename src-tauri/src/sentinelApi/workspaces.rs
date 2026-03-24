fn normalized_workspace_name(name: Option<&str>, project: &ProjectState) -> String {
    name.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| project.name.clone())
        .or_else(|| {
            project.path.as_ref().and_then(|path| {
                Path::new(path)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| "Workspace".to_string())
}

fn sorted_workspaces(inner: &SentinelState) -> Vec<WorkspaceContext> {
    let mut workspaces = inner.workspaces.values().cloned().collect::<Vec<_>>();
    workspaces.sort_by(|left, right| {
        right
            .last_active_at
            .cmp(&left.last_active_at)
            .then_with(|| left.name.cmp(&right.name))
    });
    workspaces
}

fn next_workspace_id(inner: &SentinelState, excluding_workspace_id: &str) -> Option<String> {
    sorted_workspaces(inner)
        .into_iter()
        .find(|workspace| workspace.id != excluding_workspace_id)
        .map(|workspace| workspace.id)
}

fn active_workspace_clone(inner: &SentinelState) -> Option<WorkspaceContext> {
    inner
        .active_workspace_id
        .as_ref()
        .and_then(|workspace_id| inner.workspaces.get(workspace_id))
        .cloned()
}

fn sync_active_project_to_workspace(inner: &mut SentinelState) {
    let Some(active_workspace_id) = inner.active_workspace_id.clone() else {
        return;
    };
    let active_project = inner.project.clone();
    let active_project_name = active_project.name.clone();

    let Some(workspace) = inner.workspaces.get_mut(&active_workspace_id) else {
        return;
    };

    workspace.project = active_project;
    if let Some(project_name) = active_project_name {
        workspace.name = project_name;
    }
    workspace.last_active_at = now_millis();
}

fn set_active_workspace_locked(
    inner: &mut SentinelState,
    workspace_id: &str,
) -> Result<WorkspaceContext, String> {
    let now = now_millis();
    let workspace = inner
        .workspaces
        .get_mut(workspace_id)
        .ok_or_else(|| "Workspace not found.".to_string())?;

    workspace.last_active_at = now;
    let workspace = workspace.clone();
    inner.project = workspace.project.clone();
    inner.active_workspace_id = Some(workspace_id.to_string());
    inner.preferences.last_workspace_id = Some(workspace_id.to_string());
    inner.preferences.default_session_strategy = workspace.default_session_strategy;
    update_workspace_summary(inner);
    Ok(workspace)
}

fn remove_session_from_workspace(inner: &mut SentinelState, workspace_id: &str, session_id: &str) {
    if let Some(workspace) = inner.workspaces.get_mut(workspace_id) {
        workspace.session_ids.retain(|existing_id| existing_id != session_id);
        workspace.last_active_at = now_millis();
    }
}

fn remove_tab_from_workspace(inner: &mut SentinelState, workspace_id: &str, tab_id: &str) {
    if let Some(workspace) = inner.workspaces.get_mut(workspace_id) {
        workspace.tab_ids.retain(|existing_id| existing_id != tab_id);
        workspace.last_active_at = now_millis();
    }
}

impl SentinelManager {
    pub fn create_workspace(
        &self,
        app: &AppHandle,
        candidate_path: String,
        name: Option<String>,
    ) -> Result<WorkspaceContext, String> {
        let next_project = inspect_project(Path::new(&candidate_path))?;
        let next_project_path = next_project.path.clone().map(PathBuf::from);

        self.handle_project_changed(app, next_project_path)?;

        let (workspace, created) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let existing_workspace_id = next_project.path.as_deref().and_then(|project_path| {
                inner
                    .workspaces
                    .values()
                    .find(|workspace| workspace.project.path.as_deref() == Some(project_path))
                    .map(|workspace| workspace.id.clone())
            });
            let created = existing_workspace_id.is_none();

            let workspace_id = if let Some(existing_workspace_id) = existing_workspace_id.clone() {
                if let Some(workspace) = inner.workspaces.get_mut(&existing_workspace_id) {
                    workspace.project = next_project.clone();
                    workspace.name = normalized_workspace_name(name.as_deref(), &next_project);
                }
                existing_workspace_id
            } else {
                let workspace_id = generate_id();
                let now = now_millis();
                let workspace = WorkspaceContext {
                    id: workspace_id.clone(),
                    name: normalized_workspace_name(name.as_deref(), &next_project),
                    project: next_project.clone(),
                    session_ids: Vec::new(),
                    tab_ids: Vec::new(),
                    created_at: now,
                    last_active_at: now,
                    default_session_strategy: inner.preferences.default_session_strategy,
                };
                inner.workspaces.insert(workspace_id.clone(), workspace);
                workspace_id
            };

            inner.project = next_project.clone();
            let workspace = set_active_workspace_locked(&mut inner, &workspace_id)?;
            (workspace, created)
        };

        if created {
            self.emit_workspace_created(app, &workspace);
        } else {
            self.emit_workspace_updated(app, &workspace.id);
        }
        self.emit_workspace_switched(app, &workspace);
        self.emit_project_state(app);
        self.emit_workspace_state(app);

        Ok(workspace)
    }

    pub fn list_workspaces(&self) -> Vec<WorkspaceContext> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        sorted_workspaces(&inner)
    }

    pub fn get_active_workspace(&self) -> Option<WorkspaceContext> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        active_workspace_clone(&inner)
    }

    pub fn switch_workspace(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<WorkspaceContext, String> {
        let next_project_path = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .workspaces
                .get(workspace_id)
                .and_then(|workspace| workspace.project.path.clone())
                .map(PathBuf::from)
                .ok_or_else(|| "Workspace not found.".to_string())?
        };

        self.handle_project_changed(app, Some(next_project_path))?;

        let workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            set_active_workspace_locked(&mut inner, workspace_id)?
        };

        self.emit_workspace_switched(app, &workspace);
        self.emit_project_state(app);
        self.emit_workspace_state(app);
        Ok(workspace)
    }

    pub fn close_workspace(
        self: &Arc<Self>,
        app: &AppHandle,
        workspace_id: &str,
        close_sessions: bool,
    ) -> Result<(), String> {
        let (session_ids, tab_ids, is_active, next_active_project_path) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace = inner
                .workspaces
                .get(workspace_id)
                .ok_or_else(|| "Workspace not found.".to_string())?;

            if !close_sessions && (!workspace.session_ids.is_empty() || !workspace.tab_ids.is_empty()) {
                return Err(
                    "Workspace still has running sessions or terminals. Close them first or close the workspace with session cleanup enabled."
                        .to_string(),
                );
            }

            let is_active = inner.active_workspace_id.as_deref() == Some(workspace_id);
            let next_active_project_path = if is_active {
                next_workspace_id(&inner, workspace_id).and_then(|next_workspace_id| {
                    inner
                        .workspaces
                        .get(&next_workspace_id)
                        .and_then(|workspace| workspace.project.path.clone())
                        .map(PathBuf::from)
                })
            } else {
                None
            };

            (
                workspace.session_ids.clone(),
                workspace.tab_ids.clone(),
                is_active,
                next_active_project_path,
            )
        };

        if is_active {
            self.handle_project_changed(app, next_active_project_path)?;
        }

        if close_sessions {
            for tab_id in &tab_ids {
                let _ = self.close_tab(app, tab_id);
            }
            for session_id in &session_ids {
                let _ = self.close_session(app, session_id);
            }
        }

        let next_active_workspace = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .workspaces
                .remove(workspace_id)
                .ok_or_else(|| "Workspace not found.".to_string())?;

            if inner.active_workspace_id.as_deref() == Some(workspace_id) {
                if let Some(next_workspace_id) = next_workspace_id(&inner, workspace_id) {
                    Some(set_active_workspace_locked(&mut inner, &next_workspace_id)?)
                } else {
                    inner.active_workspace_id = None;
                    inner.preferences.last_workspace_id = None;
                    inner.project = ProjectState::default();
                    update_workspace_summary(&mut inner);
                    None
                }
            } else {
                update_workspace_summary(&mut inner);
                active_workspace_clone(&inner)
            }
        };

        emit_event(
            app,
            EVENT_WORKSPACE_REMOVED,
            &WorkspaceRemovedEvent {
                workspace_id: workspace_id.to_string(),
            },
        );

        if !is_active {
            self.emit_workspace_state(app);
            return Ok(());
        }

        if let Some(workspace) = next_active_workspace.as_ref() {
            self.emit_workspace_switched(app, workspace);
        }

        self.emit_project_state(app);
        self.emit_workspace_state(app);
        Ok(())
    }

    fn emit_workspace_created(&self, app: &AppHandle, workspace: &WorkspaceContext) {
        emit_event(app, EVENT_WORKSPACE_CREATED, workspace);
    }

    fn emit_workspace_updated(&self, app: &AppHandle, workspace_id: &str) {
        let workspace = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.workspaces.get(workspace_id).cloned()
        };

        if let Some(workspace) = workspace {
            emit_event(app, EVENT_WORKSPACE_UPDATED, &workspace);
        }
    }

    fn emit_workspace_switched(&self, app: &AppHandle, workspace: &WorkspaceContext) {
        emit_event(app, EVENT_WORKSPACE_SWITCHED, workspace);
    }

    pub fn stop_workspace(
        self: &Arc<Self>,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<(), String> {
        let (session_ids, tab_ids) = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let workspace = inner
                .workspaces
                .get(workspace_id)
                .ok_or_else(|| "Workspace not found.".to_string())?;

            (workspace.session_ids.clone(), workspace.tab_ids.clone())
        };

        for tab_id in &tab_ids {
            let _ = self.close_tab(app, tab_id);
        }
        for session_id in &session_ids {
            let _ = self.close_session(app, session_id);
        }

        self.emit_workspace_updated(app, workspace_id);
        self.emit_workspace_state(app);

        Ok(())
    }

    pub fn pause_workspace(
        self: &Arc<Self>,
        _app: &AppHandle,
        _workspace_id: &str,
    ) -> Result<(), String> {
        // Soft pause implementation will go here.
        println!("Soft pause triggered for workspace: {}", _workspace_id);
        Ok(())
    }
}
