use crate::database::db_models::{
    ActivityLogRow, CommandHistoryRow as DbCommandHistoryRow, FileChangeRow as DbFileChangeRow,
    IdeTerminalRow, PreferenceRow, SessionRow, TabRow, WorkspaceRow,
};
use crate::database::repositories::{
    ActivityRepository, AuditRepository, CommandRepository, FileChangeRepository,
    IdeTerminalRepository, PreferenceRepository, SessionRepository, TabRepository,
    WorkspaceRepository,
};
use crate::database::Database;
use sqlx::SqlitePool;

fn database_pool(app: &AppHandle) -> SqlitePool {
    app.state::<Arc<Database>>().pool().clone()
}

fn session_strategy_to_db(strategy: SessionWorkspaceStrategy) -> &'static str {
    match strategy {
        SessionWorkspaceStrategy::SandboxCopy => "sandbox-copy",
        SessionWorkspaceStrategy::GitWorktree => "git-worktree",
    }
}

fn session_strategy_from_db(value: &str) -> SessionWorkspaceStrategy {
    match value {
        "git-worktree" => SessionWorkspaceStrategy::GitWorktree,
        _ => SessionWorkspaceStrategy::SandboxCopy,
    }
}

fn workspace_project_from_row(row: &WorkspaceRow, refresh_tree: bool) -> ProjectState {
    if refresh_tree {
        if let Some(project_path) = row.project_path.as_deref() {
            let path = PathBuf::from(project_path);
            if path.exists() {
                if let Ok(project) = inspect_project(&path) {
                    return project;
                }
            }
        }
    }

    ProjectState {
        path: row.project_path.clone(),
        name: row.project_name.clone().or_else(|| Some(row.name.clone())),
        branch: row.git_branch.clone(),
        is_git_repo: row.is_git_repo != 0,
        tree: Vec::new(),
    }
}

fn workspace_from_row(row: WorkspaceRow, refresh_tree: bool) -> WorkspaceContext {
    let project = workspace_project_from_row(&row, refresh_tree);
    WorkspaceContext {
        id: row.id.clone(),
        name: row.name,
        project,
        session_ids: Vec::new(),
        tab_ids: Vec::new(),
        created_at: row.created_at,
        last_active_at: row.last_active_at,
        default_session_strategy: session_strategy_from_db(&row.default_session_strategy),
    }
}

fn activity_entry_from_row(row: ActivityLogRow) -> ActivityLogEntry {
    ActivityLogEntry {
        id: row.id,
        workspace_id: Some(row.workspace_id),
        timestamp: row.timestamp,
        scope: row.scope,
        status: row.status,
        command: row.command,
        cwd: row.cwd,
        detail: row.detail,
    }
}

fn session_status_from_db(value: &str) -> SessionStatus {
    match value {
        "ready" => SessionStatus::Ready,
        "closing" => SessionStatus::Closing,
        "paused" => SessionStatus::Paused,
        "closed" => SessionStatus::Closed,
        "error" => SessionStatus::Error,
        _ => SessionStatus::Starting,
    }
}

fn cleanup_state_from_db(value: &str) -> CleanupState {
    match value {
        "removed" => CleanupState::Removed,
        "preserved" => CleanupState::Preserved,
        "failed" => CleanupState::Failed,
        _ => CleanupState::Active,
    }
}

fn tab_status_from_db(value: &str) -> TabStatus {
    match value {
        "ready" => TabStatus::Ready,
        "closing" => TabStatus::Closing,
        "closed" => TabStatus::Closed,
        "error" => TabStatus::Error,
        _ => TabStatus::Starting,
    }
}

fn ide_status_from_db(value: &str) -> IdeStatus {
    match value {
        "starting" => IdeStatus::Starting,
        "ready" => IdeStatus::Ready,
        "closing" => IdeStatus::Closing,
        "closed" => IdeStatus::Closed,
        "error" => IdeStatus::Error,
        _ => IdeStatus::Idle,
    }
}

fn session_summary_from_row(row: SessionRow) -> SessionSummary {
    SessionSummary {
        id: row.id,
        workspace_id: row.workspace_id,
        label: row.label,
        project_root: row.project_root,
        cwd: row.cwd,
        workspace_path: row.workspace_path,
        workspace_strategy: session_strategy_from_db(&row.workspace_strategy),
        branch_name: row.branch_name,
        status: session_status_from_db(&row.status),
        cleanup_state: cleanup_state_from_db(&row.cleanup_state),
        shell: row.shell,
        pid: row.process_id.map(|value| value as u32),
        created_at: row.created_at,
        startup_command: row.startup_command,
        exit_code: row.exit_code.map(|value| value as i32),
        error: row.error_message,
        metrics: ProcessMetrics {
            cpu_percent: row.cpu_percent,
            memory_mb: row.memory_mb,
            thread_count: row.thread_count.max(0) as u32,
            handle_count: row.handle_count.max(0) as u32,
            process_count: row.process_count.max(0) as u32,
        },
    }
}

fn tab_type_from_db(value: &str) -> TabType {
    match value {
        "dashboard" => TabType::Dashboard,
        _ => TabType::Terminal,
    }
}

fn tab_summary_from_row(row: TabRow) -> TabSummary {
    TabSummary {
        id: row.id,
        workspace_id: row.workspace_id,
        tab_type: tab_type_from_db(&row.tab_type),
        label: row.label,
        status: tab_status_from_db(&row.status),
        cwd: row.cwd,
        shell: row.shell,
        pid: row.process_id.map(|value| value as u32),
        created_at: row.created_at,
        exit_code: row.exit_code.map(|value| value as i32),
        error: row.error_message,
        metrics: ProcessMetrics {
            cpu_percent: row.cpu_percent,
            memory_mb: row.memory_mb,
            thread_count: row.thread_count.max(0) as u32,
            handle_count: row.handle_count.max(0) as u32,
            process_count: row.process_count.max(0) as u32,
        },
    }
}

fn ide_state_from_row(row: IdeTerminalRow) -> IdeTerminalState {
    IdeTerminalState {
        status: ide_status_from_db(&row.status),
        cwd: row.cwd,
        workspace_path: row.workspace_path,
        shell: row.shell,
        pid: row.process_id.map(|value| value as u32),
        created_at: Some(row.created_at),
        exit_code: row.exit_code.map(|value| value as i32),
        error: row.error_message,
        modified_paths: serde_json::from_str(&row.modified_paths).unwrap_or_default(),
    }
}

fn session_history_entries_from_rows(
    mut rows: Vec<DbCommandHistoryRow>,
) -> Vec<SessionCommandEntry> {
    rows.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.id.cmp(&right.id))
    });
    rows.into_iter()
        .map(|row| SessionCommandEntry {
            id: row.id.to_string(),
            command: row.command_text,
            timestamp: row.timestamp,
            source: row.source,
        })
        .collect()
}

fn session_diff_snapshot_from_rows(
    session_id: &str,
    workspace_id: &str,
    rows: Vec<DbFileChangeRow>,
    fallback_timestamp: i64,
) -> SessionDiffUpdate {
    let mut latest_by_file = HashMap::<String, (i64, String)>::new();
    let mut updated_at = fallback_timestamp;

    for row in rows {
        updated_at = updated_at.max(row.timestamp);
        let entry = latest_by_file
            .entry(row.file_path)
            .or_insert((row.timestamp, row.change_type.clone()));
        if row.timestamp >= entry.0 {
            *entry = (row.timestamp, row.change_type);
        }
    }

    let mut modified_paths = latest_by_file
        .into_iter()
        .filter_map(|(file_path, (_timestamp, change_type))| {
            if change_type == "deleted" {
                None
            } else {
                Some(file_path)
            }
        })
        .collect::<Vec<_>>();
    modified_paths.sort();

    SessionDiffUpdate {
        session_id: session_id.to_string(),
        workspace_id: workspace_id.to_string(),
        modified_paths,
        updated_at,
    }
}

fn preferences_from_rows(
    rows: &[PreferenceRow],
    active_workspace_id: Option<&str>,
    active_workspace_strategy: Option<SessionWorkspaceStrategy>,
) -> WorkspacePreferences {
    let mut preferences = WorkspacePreferences::default();
    if let Some(strategy) = active_workspace_strategy {
        preferences.default_session_strategy = strategy;
    }

    for row in rows {
        match row.key.as_str() {
            "default_session_strategy" => {
                preferences.default_session_strategy = session_strategy_from_db(&row.value);
            }
            "last_workspace_id" => {
                preferences.last_workspace_id = if row.value.trim().is_empty() {
                    None
                } else {
                    Some(row.value.clone())
                };
            }
            _ => {}
        }
    }

    if preferences.last_workspace_id.is_none() {
        preferences.last_workspace_id = active_workspace_id.map(str::to_string);
    }

    preferences
}

fn log_persistence_error(context: &str, error: &str) {
    eprintln!("[sentinel][sqlite] {context}: {error}");
}

impl SentinelManager {
    pub fn hydrate_from_database(&self, app: &AppHandle) -> Result<(), String> {
        let pool = database_pool(app);
        let (workspace_rows, session_rows, tab_rows, activity_rows, preference_rows) =
            tauri::async_runtime::block_on(async {
                SessionRepository::mark_stale_as_paused(
                    &pool,
                    "Session paused because Sentinel exited before this agent finished.",
                )
                .await?;
                TabRepository::mark_stale_as_closed(
                    &pool,
                    "Terminal closed because Sentinel exited before it shut down cleanly.",
                )
                .await?;
                IdeTerminalRepository::mark_stale_as_closed(
                    &pool,
                    "IDE terminal closed because Sentinel exited before it shut down cleanly.",
                )
                .await?;

                tokio::try_join!(
                    WorkspaceRepository::find_all(&pool),
                    SessionRepository::find_workspace_memberships(&pool),
                    TabRepository::find_workspace_memberships(&pool),
                    ActivityRepository::find_recent(&pool, Some(120)),
                    PreferenceRepository::find_global_by_category(&pool, "workspace"),
                )
            })
            .map_err(|error| format!("Failed to hydrate persisted runtime state: {error}"))?;

        let database_active_workspace_id = workspace_rows
            .iter()
            .find(|row| row.is_active != 0)
            .map(|row| row.id.clone());
        let active_workspace_strategy = database_active_workspace_id.as_ref().and_then(|id| {
            workspace_rows
                .iter()
                .find(|row| &row.id == id)
                .map(|row| session_strategy_from_db(&row.default_session_strategy))
        });
        let preferences = preferences_from_rows(
            &preference_rows,
            database_active_workspace_id.as_deref(),
            active_workspace_strategy,
        );
        let active_workspace_id = database_active_workspace_id.clone().or_else(|| {
            preferences
                .last_workspace_id
                .clone()
                .filter(|id| workspace_rows.iter().any(|row| row.id == id.as_str()))
        });

        if database_active_workspace_id.as_deref() != active_workspace_id.as_deref() {
            tauri::async_runtime::block_on(WorkspaceRepository::set_active(
                &pool,
                active_workspace_id.as_deref().unwrap_or(""),
                active_workspace_id.is_some(),
            ))
            .map_err(|error| format!("Failed to synchronize active workspace state: {error}"))?;
        }

        let mut workspaces = workspace_rows
            .into_iter()
            .map(|row| {
                let refresh_tree = active_workspace_id.as_deref() == Some(row.id.as_str());
                let workspace = workspace_from_row(row, refresh_tree);
                (workspace.id.clone(), workspace)
            })
            .collect::<HashMap<_, _>>();
        for row in session_rows {
            if let Some(workspace) = workspaces.get_mut(&row.workspace_id) {
                workspace.session_ids.push(row.id);
            }
        }
        for row in tab_rows {
            if let Some(workspace) = workspaces.get_mut(&row.workspace_id) {
                workspace.tab_ids.push(row.id);
            }
        }
        let project = active_workspace_id
            .as_ref()
            .and_then(|workspace_id| workspaces.get(workspace_id))
            .map(|workspace| workspace.project.clone())
            .unwrap_or_default();
        let activity_log = activity_rows
            .into_iter()
            .map(activity_entry_from_row)
            .collect::<Vec<_>>();

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.workspaces = workspaces;
            inner.active_workspace_id = active_workspace_id;
            inner.project = project;
            inner.preferences = preferences;
            inner.activity_log = activity_log;
            update_workspace_summary(&mut inner);
        }

        Ok(())
    }

    fn persist_workspace(&self, app: &AppHandle, workspace: &WorkspaceContext) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(WorkspaceRepository::upsert(&pool, workspace))
        .map_err(|error| format!("Failed to persist workspace {}: {error}", workspace.id))
    }

    fn persist_active_workspace_selection(
        &self,
        app: &AppHandle,
        workspace_id: Option<&str>,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(WorkspaceRepository::set_active(
            &pool,
            workspace_id.unwrap_or(""),
            workspace_id.is_some(),
        ))
        .map_err(|error| format!("Failed to update active workspace selection: {error}"))
    }

    fn delete_workspace_from_database(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(WorkspaceRepository::delete(&pool, workspace_id))
            .map_err(|error| format!("Failed to remove workspace {workspace_id}: {error}"))
    }

    fn persist_preferences(&self, app: &AppHandle) -> Result<(), String> {
        let preferences = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.preferences.clone()
        };
        let pool = database_pool(app);
        let default_strategy = session_strategy_to_db(preferences.default_session_strategy);
        let last_workspace_id = preferences.last_workspace_id.clone().unwrap_or_default();

        tauri::async_runtime::block_on(async {
            PreferenceRepository::upsert_global(
                &pool,
                "workspace",
                "default_session_strategy",
                default_strategy,
                false,
                now_millis(),
            )
            .await?;
            PreferenceRepository::upsert_global(
                &pool,
                "workspace",
                "last_workspace_id",
                &last_workspace_id,
                false,
                now_millis(),
            )
            .await?;
            Ok::<_, sqlx::Error>(())
        })
        .map_err(|error| format!("Failed to persist workspace preferences: {error}"))
    }

    fn persist_session_created(
        &self,
        app: &AppHandle,
        summary: &SessionSummary,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(SessionRepository::create(&pool, summary))
            .map_err(|error| format!("Failed to persist session {}: {error}", summary.id))
    }

    fn persist_session_status(
        &self,
        app: &AppHandle,
        summary: &SessionSummary,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(SessionRepository::update_status(
            &pool,
            &summary.id,
            summary.status,
            summary.cleanup_state,
            summary.exit_code,
            summary.error.as_deref(),
        ))
        .map_err(|error| format!("Failed to update session {} state: {error}", summary.id))
    }

    fn persist_session_metrics(
        &self,
        app: &AppHandle,
        summary: &SessionSummary,
        sampled_at: i64,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(SessionRepository::update_metrics(
            &pool,
            &summary.id,
            summary.metrics.cpu_percent,
            summary.metrics.memory_mb,
            summary.metrics.thread_count as i64,
            summary.metrics.handle_count as i64,
            summary.metrics.process_count as i64,
            sampled_at,
        ))
        .map_err(|error| format!("Failed to update session {} metrics: {error}", summary.id))
    }

    fn persist_tab_created(&self, app: &AppHandle, summary: &TabSummary) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(TabRepository::create(&pool, summary))
            .map_err(|error| format!("Failed to persist tab {}: {error}", summary.id))
    }

    fn persist_tab_status(&self, app: &AppHandle, summary: &TabSummary) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(TabRepository::update_status(
            &pool,
            &summary.id,
            summary.status,
            summary.exit_code,
            summary.error.as_deref(),
        ))
        .map_err(|error| format!("Failed to update tab {} state: {error}", summary.id))
    }

    fn persist_tab_metrics(
        &self,
        app: &AppHandle,
        summary: &TabSummary,
        sampled_at: i64,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(TabRepository::update_metrics(
            &pool,
            &summary.id,
            summary.metrics.cpu_percent,
            summary.metrics.memory_mb,
            summary.metrics.thread_count as i64,
            summary.metrics.handle_count as i64,
            summary.metrics.process_count as i64,
            sampled_at,
        ))
        .map_err(|error| format!("Failed to update tab {} metrics: {error}", summary.id))
    }

    fn persist_ide_state(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        state: &IdeTerminalState,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(IdeTerminalRepository::upsert(&pool, workspace_id, state))
            .map_err(|error| {
                format!(
                    "Failed to persist IDE terminal state for workspace {workspace_id}: {error}"
                )
            })
    }

    fn persist_command_entry(
        &self,
        app: &AppHandle,
        summary: &SessionSummary,
        entry: &SessionCommandEntry,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        tauri::async_runtime::block_on(CommandRepository::insert(
            &pool,
            &summary.id,
            &summary.workspace_id,
            &entry.command,
            entry.timestamp,
            &entry.source,
            Some(&summary.cwd),
        ))
        .map(|_| ())
        .map_err(|error| format!("Failed to persist session command history: {error}"))
    }

    fn persist_file_change(
        &self,
        app: &AppHandle,
        session_id: &str,
        workspace_id: &str,
        file_path: &str,
        before_hash: Option<String>,
        after_hash: Option<String>,
        file_size: Option<i64>,
    ) {
        let change_type = match (before_hash.as_ref(), after_hash.as_ref()) {
            (None, Some(_)) => "created",
            (Some(_), Some(_)) => "modified",
            (Some(_), None) => "deleted",
            (None, None) => return,
        };

        let pool = database_pool(app);
        if let Err(error) = tauri::async_runtime::block_on(FileChangeRepository::insert(
            &pool,
            session_id,
            workspace_id,
            file_path,
            change_type,
            before_hash.as_deref(),
            after_hash.as_deref(),
            now_millis(),
            file_size,
        )) {
            log_persistence_error("persist file change", &error.to_string());
        }
    }

    fn persist_activity_entry(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        entry: &ActivityLogEntry,
        session_id: Option<&str>,
    ) {
        let pool = database_pool(app);
        if let Err(error) = tauri::async_runtime::block_on(ActivityRepository::insert(
            &pool,
            workspace_id,
            entry,
            session_id,
        )) {
            log_persistence_error("persist activity log", &error.to_string());
        }
    }

    fn persist_audit_event(
        &self,
        app: &AppHandle,
        workspace_id: Option<&str>,
        session_id: Option<&str>,
        tab_id: Option<&str>,
        action_type: &str,
        resource_type: &str,
        resource_id: &str,
        details: Option<serde_json::Value>,
    ) {
        let serialized = details.map(|value| value.to_string());
        let pool = database_pool(app);
        if let Err(error) = tauri::async_runtime::block_on(AuditRepository::insert(
            &pool,
            workspace_id,
            session_id,
            tab_id,
            now_millis(),
            action_type,
            resource_type,
            resource_id,
            serialized.as_deref(),
        )) {
            log_persistence_error("persist audit log", &error.to_string());
        }
    }
}
