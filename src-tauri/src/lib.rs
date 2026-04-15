mod database;
mod models;
#[path = "sentinelApi/mod.rs"]
mod sentinel;

use database::Database;
use std::sync::Arc;

use models::{
    BootstrapPayload, CommandHistoryEntry, CreateSessionInput, FileChangeEntry, IdeTerminalState,
    ProjectState, SessionApplyResult, SessionCommitResult, SessionSummary,
    SessionWorkspaceStrategy, SnapshotSummary, TabSummary, WorkspaceAnalytics, WorkspaceContext,
    WorkspaceMode, WorkspacePreferences, ChangesManagerState,
};
use sentinel::SentinelManager;
use tauri::{AppHandle, Manager, RunEvent, State};

#[tauri::command]
fn bootstrap(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<BootstrapPayload, String> {
    state.bootstrap(&app)
}

#[tauri::command]
fn load_project(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    candidate_path: String,
) -> Result<ProjectState, String> {
    state.load_project(&app, candidate_path)
}

#[tauri::command]
fn create_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    candidate_path: String,
    name: Option<String>,
    mode: Option<WorkspaceMode>,
) -> Result<WorkspaceContext, String> {
    state.create_workspace(&app, candidate_path, name, mode)
}

#[tauri::command]
fn list_workspaces(
    state: State<'_, Arc<SentinelManager>>,
) -> Result<Vec<WorkspaceContext>, String> {
    Ok(state.list_workspaces())
}

#[tauri::command]
fn switch_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<WorkspaceContext, String> {
    state.switch_workspace(&app, &workspace_id)
}

#[tauri::command]
fn close_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    close_sessions: bool,
) -> Result<(), String> {
    state.close_workspace(&app, &workspace_id, close_sessions)
}

#[tauri::command]
fn stop_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<(), String> {
    state.stop_workspace(&app, &workspace_id)
}

#[tauri::command]
fn pause_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<(), String> {
    state.pause_workspace(&app, &workspace_id)
}

#[tauri::command]
fn get_active_workspace(
    state: State<'_, Arc<SentinelManager>>,
) -> Result<Option<WorkspaceContext>, String> {
    Ok(state.get_active_workspace())
}

#[tauri::command]
fn refresh_project(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<ProjectState, String> {
    state.refresh_project(&app)
}

#[tauri::command]
fn set_default_session_strategy(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    strategy: SessionWorkspaceStrategy,
) -> Result<WorkspacePreferences, String> {
    Ok(state.set_default_session_strategy(&app, strategy))
}

#[tauri::command]
async fn create_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    input: Option<CreateSessionInput>,
) -> Result<SessionSummary, String> {
    let manager = state.inner().clone();
    let input = input.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || manager.create_session(&app, input))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn close_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<(), String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.close_session(&app, &session_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn pause_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<(), String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.pause_session(&app, &session_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn resume_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<SessionSummary, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.resume_session(&app, &session_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn delete_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<(), String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.delete_session(&app, &session_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn resize_session(
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize_session(&session_id, cols, rows)
}

#[tauri::command]
fn send_input(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    state.send_input(&app, &session_id, &data)
}

#[tauri::command]
fn ensure_ide_terminal(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<IdeTerminalState, String> {
    state.ensure_ide_terminal(&app)
}

#[tauri::command]
fn resize_ide_terminal(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize_ide_terminal(&app, cols, rows)
}

#[tauri::command]
fn send_ide_terminal_input(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    data: String,
) -> Result<(), String> {
    state.send_ide_terminal_input(&app, &data)
}

#[tauri::command]
async fn create_standalone_terminal(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    cwd: Option<String>,
    label: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<TabSummary, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.create_standalone_terminal(&app, cwd, label, cols, rows)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn close_tab(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    tab_id: String,
) -> Result<(), String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.close_tab(&app, &tab_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn resize_tab(
    state: State<'_, Arc<SentinelManager>>,
    tab_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize_tab(&tab_id, cols, rows)
}

#[tauri::command]
fn send_tab_input(
    state: State<'_, Arc<SentinelManager>>,
    tab_id: String,
    data: String,
) -> Result<(), String> {
    state.send_tab_input(&tab_id, &data)
}

#[tauri::command]
fn write_ide_file(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    state.write_ide_file(&app, &relative_path, &content)
}

#[tauri::command]
async fn apply_ide_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<SessionApplyResult, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.apply_ide_workspace(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn discard_ide_workspace_changes(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<(), String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.discard_ide_workspace_changes(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn read_file(state: State<'_, Arc<SentinelManager>>, file_path: String) -> Result<String, String> {
    Ok(state.read_file(&file_path))
}

#[tauri::command]
fn read_file_diff(
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    file_path: String,
) -> Result<String, String> {
    Ok(state.read_file_diff(&session_id, &file_path))
}

#[tauri::command]
fn write_session_file(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    state.write_session_file(&app, &session_id, &relative_path, &content)
}

#[tauri::command]
async fn apply_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<SessionApplyResult, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.apply_session(&app, &session_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn commit_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    message: String,
) -> Result<SessionCommitResult, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.commit_session(&app, &session_id, &message))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn discard_session_changes(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<(), String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.discard_session_changes(&app, &session_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn reveal_in_file_explorer(
    state: State<'_, Arc<SentinelManager>>,
    file_path: String,
) -> Result<(), String> {
    state.reveal_in_file_explorer(&file_path)
}

#[tauri::command]
fn open_in_system_editor(
    state: State<'_, Arc<SentinelManager>>,
    file_path: String,
) -> Result<(), String> {
    state.open_in_system_editor(&file_path)
}

#[tauri::command]
fn search_command_history(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<CommandHistoryEntry>, String> {
    state.search_command_history(&app, &workspace_id, &query, limit)
}

#[tauri::command]
fn get_file_change_timeline(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    file_path: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<FileChangeEntry>, String> {
    state.get_file_change_timeline(&app, &workspace_id, file_path.as_deref(), limit)
}

#[tauri::command]
fn get_workspace_analytics(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<WorkspaceAnalytics, String> {
    state.get_workspace_analytics(&app, &workspace_id)
}

#[tauri::command]
fn export_audit_log(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    start_timestamp: Option<i64>,
    end_timestamp: Option<i64>,
    format: Option<String>,
) -> Result<String, String> {
    state.export_audit_log(
        &app,
        &workspace_id,
        start_timestamp,
        end_timestamp,
        format.as_deref(),
    )
}

#[tauri::command]
fn create_workspace_snapshot(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    name: String,
    description: Option<String>,
) -> Result<SnapshotSummary, String> {
    state.create_workspace_snapshot(&app, &workspace_id, &name, description)
}

#[tauri::command]
fn restore_workspace_snapshot(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    snapshot_id: String,
) -> Result<WorkspaceContext, String> {
    state.restore_workspace_snapshot(&app, &snapshot_id)
}

#[tauri::command]
fn list_workspace_snapshots(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<Vec<SnapshotSummary>, String> {
    state.list_workspace_snapshots(&app, &workspace_id)
}

#[tauri::command]
fn get_changes_manager_state(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<ChangesManagerState, String> {
    state.changes_manager.get_changes_state(&app, &workspace_id)
}

#[tauri::command]
fn scan_agent_changes(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    agent_id: String,
) -> Result<(), String> {
    state.changes_manager.scan_sandbox_changes(&app, &workspace_id, &agent_id)?;
    Ok(())
}

#[tauri::command]
fn push_unified_sandbox(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
) -> Result<Vec<String>, String> {
    state.push_unified_sandbox(&app, &workspace_id)
}

#[tauri::command]
fn discard_changes(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    agent_id: Option<String>,
) -> Result<(), String> {
    state.changes_manager.discard_changes(&app, &workspace_id, agent_id.as_deref())
}

#[tauri::command]
fn resolve_file_conflict(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    workspace_id: String,
    file_path: String,
    winning_agent_id: String,
) -> Result<(), String> {
    state.changes_manager.resolve_conflict(&app, &workspace_id, &file_path, &winning_agent_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(SentinelManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(manager.clone())
        .setup({
            let manager = manager.clone();
            move |app| {
                // Initialize the SQLite database
                let app_data_dir = app
                    .path()
                    .app_data_dir()
                    .expect("Failed to resolve app data directory");
                let db = tauri::async_runtime::block_on(Database::init(&app_data_dir))
                    .expect("Failed to initialize SQLite database");
                let db = Arc::new(db);
                app.manage(db);
                manager
                    .hydrate_from_database(&app.handle())
                    .expect("Failed to hydrate SQLite state");

                manager.start_refresh_loop(app.handle().clone());
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            load_project,
            create_workspace,
            list_workspaces,
            switch_workspace,
            close_workspace,
            stop_workspace,
            pause_workspace,
            get_active_workspace,
            refresh_project,
            set_default_session_strategy,
            create_session,
            close_session,
            pause_session,
            resume_session,
            delete_session,
            resize_session,
            send_input,
            ensure_ide_terminal,
            resize_ide_terminal,
            send_ide_terminal_input,
            write_ide_file,
            apply_ide_workspace,
            discard_ide_workspace_changes,
            read_file,
            read_file_diff,
            write_session_file,
            apply_session,
            commit_session,
            discard_session_changes,
            reveal_in_file_explorer,
            open_in_system_editor,
            create_standalone_terminal,
            close_tab,
            resize_tab,
            send_tab_input,
            search_command_history,
            get_file_change_timeline,
            get_workspace_analytics,
            export_audit_log,
            create_workspace_snapshot,
            restore_workspace_snapshot,
            list_workspace_snapshots,
            get_changes_manager_state,
            scan_agent_changes,
            push_unified_sandbox,
            discard_changes,
            resolve_file_conflict
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app_handle, event| {
            if let RunEvent::Exit = event {
                manager.dispose(app_handle);
            }
        });
}
