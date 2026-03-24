mod models;
#[path = "sentinelApi/mod.rs"]
mod sentinel;

use std::sync::Arc;

use models::{
    BootstrapPayload, CreateSessionInput, IdeTerminalState, ProjectState, SessionApplyResult,
    SessionCommitResult, SessionSummary, SessionWorkspaceStrategy, TabSummary,
    WorkspacePreferences,
};
use sentinel::SentinelManager;
use tauri::{AppHandle, RunEvent, State};

#[tauri::command]
fn bootstrap(state: State<'_, Arc<SentinelManager>>) -> Result<BootstrapPayload, String> {
    Ok(state.bootstrap())
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
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    state.send_input(&session_id, &data)
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
    cols: u16,
    rows: u16,
) -> Result<TabSummary, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.create_standalone_terminal(&app, cwd, cols, rows)
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
fn apply_ide_workspace(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<SessionApplyResult, String> {
    state.apply_ide_workspace(&app)
}

#[tauri::command]
fn discard_ide_workspace_changes(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
) -> Result<(), String> {
    state.discard_ide_workspace_changes(&app)
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
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    state.write_session_file(&session_id, &relative_path, &content)
}

#[tauri::command]
fn apply_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<SessionApplyResult, String> {
    state.apply_session(&app, &session_id)
}

#[tauri::command]
fn commit_session(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
    message: String,
) -> Result<SessionCommitResult, String> {
    state.commit_session(&app, &session_id, &message)
}

#[tauri::command]
fn discard_session_changes(
    app: AppHandle,
    state: State<'_, Arc<SentinelManager>>,
    session_id: String,
) -> Result<(), String> {
    state.discard_session_changes(&app, &session_id)
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
                manager.start_refresh_loop(app.handle().clone());
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            load_project,
            refresh_project,
            set_default_session_strategy,
            create_session,
            close_session,
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
            send_tab_input
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app_handle, event| {
            if let RunEvent::Exit = event {
                manager.dispose(app_handle);
            }
        });
}
