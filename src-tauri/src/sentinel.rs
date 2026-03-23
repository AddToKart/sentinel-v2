use crate::models::{
    ActivityLogEntry, BootstrapPayload, CleanupState, CreateSessionInput, IdeStatus,
    IdeTerminalState, ProcessMetrics, ProjectNode, ProjectState, SessionApplyResult,
    SessionCommandEntry, SessionDiffUpdate, SessionHistoryUpdate, SessionMetricsUpdate,
    SessionStatus, SessionSummary, SessionSyncConflict, SessionWorkspaceStrategy,
    WorkspacePreferences, WorkspaceSummary,
};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const TREE_DEPTH: usize = 3;
const TREE_ENTRY_LIMIT: usize = 28;
const METRIC_INTERVAL_MS: u64 = 1_000;
const CLOSE_TIMEOUT_MS: u64 = 4_000;

const EVENT_SESSION_OUTPUT: &str = "sentinel:session-output";
const EVENT_SESSION_STATE: &str = "sentinel:session-state";
const EVENT_IDE_OUTPUT: &str = "sentinel:ide-terminal-output";
const EVENT_IDE_STATE: &str = "sentinel:ide-terminal-state";
const EVENT_SESSION_METRICS: &str = "sentinel:session-metrics";
const EVENT_SESSION_HISTORY: &str = "sentinel:session-history";
const EVENT_SESSION_DIFF: &str = "sentinel:session-diff";
const EVENT_WORKSPACE_STATE: &str = "sentinel:workspace-state";
const EVENT_ACTIVITY_LOG: &str = "sentinel:activity-log";

static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct FileFingerprint {
    signature: String,
    hash: String,
}

#[derive(Clone)]
struct SandboxWorkspaceState {
    baseline_hashes: BTreeMap<String, String>,
    scan_cache: BTreeMap<String, FileFingerprint>,
}

type SharedMaster = Arc<Mutex<Box<dyn MasterPty + Send>>>;
type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;
type SharedKiller = Arc<Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>>;

struct TerminalHandles {
    master: SharedMaster,
    writer: SharedWriter,
    killer: SharedKiller,
    pid: Option<u32>,
}

#[derive(Clone, Copy)]
struct TerminalSize {
    cols: u16,
    rows: u16,
}

struct SessionRecord {
    summary: SessionSummary,
    master: SharedMaster,
    writer: SharedWriter,
    killer: SharedKiller,
    terminal_size: TerminalSize,
    close_requested: bool,
    finalized: bool,
    command_buffer: String,
    history: Vec<SessionCommandEntry>,
    modified_paths: Vec<String>,
    sandbox_state: Option<SandboxWorkspaceState>,
    tracked_process_ids: Vec<u32>,
    last_cpu_total_seconds: Option<f64>,
    last_sampled_at: Option<i64>,
}

struct IdeRecord {
    state: IdeTerminalState,
    master: SharedMaster,
    writer: SharedWriter,
    killer: SharedKiller,
    terminal_size: TerminalSize,
    close_requested: bool,
    finalized: bool,
}

#[derive(Default)]
struct IdeRuntime {
    record: Option<IdeRecord>,
    workspace_path: Option<PathBuf>,
    workspace_project_root: Option<PathBuf>,
    sandbox_state: Option<SandboxWorkspaceState>,
}

struct SentinelState {
    sessions: HashMap<String, SessionRecord>,
    ide: IdeRuntime,
    project: ProjectState,
    preferences: WorkspacePreferences,
    workspace_summary: WorkspaceSummary,
    activity_log: Vec<ActivityLogEntry>,
    windows_build_number: Option<u32>,
}

pub struct SentinelManager {
    inner: Mutex<SentinelState>,
}

#[derive(Deserialize)]
struct RawProcessTreeSnapshot {
    #[serde(rename = "RootId")]
    root_id: u32,
    #[serde(rename = "CpuTotalSeconds")]
    cpu_total_seconds: f64,
    #[serde(rename = "WorkingSetBytes")]
    working_set_bytes: u64,
    #[serde(rename = "HandleCount")]
    handle_count: u32,
    #[serde(rename = "ThreadCount")]
    thread_count: u32,
    #[serde(rename = "ProcessCount")]
    process_count: u32,
    #[serde(rename = "ProcessIds")]
    process_ids: Vec<u32>,
}

#[derive(Clone)]
struct ProcessTreeSnapshot {
    cpu_total_seconds: f64,
    working_set_bytes: u64,
    handle_count: u32,
    thread_count: u32,
    process_count: u32,
    process_ids: Vec<u32>,
}

struct SessionWorkspaceResult {
    workspace_path: PathBuf,
    branch_name: Option<String>,
    sandbox_state: Option<SandboxWorkspaceState>,
}

struct ApplySandboxOutcome {
    result: SessionApplyResult,
    next_baseline_hashes: BTreeMap<String, String>,
    next_cache: BTreeMap<String, FileFingerprint>,
}

struct IdeApplyOutcome {
    result: SessionApplyResult,
    sandbox_state: SandboxWorkspaceState,
    modified_paths: Vec<String>,
}

struct IdeDiscardOutcome {
    sandbox_state: SandboxWorkspaceState,
    modified_paths: Vec<String>,
}

pub fn parse_windows_build_number() -> Option<u32> {
    #[cfg(windows)]
    {
        let version = Command::new("cmd").args(["/C", "ver"]).output().ok()?;
        let text = String::from_utf8_lossy(&version.stdout).to_string();
        let digits: Vec<u32> = text
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter(|part| part.contains('.'))
            .flat_map(|part| part.split('.').filter_map(|piece| piece.parse::<u32>().ok()))
            .collect();
        digits.last().copied()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

impl SentinelManager {
    pub fn new() -> Self {
        let mut summary = WorkspaceSummary::default();
        summary.last_updated = now_millis();

        Self {
            inner: Mutex::new(SentinelState {
                sessions: HashMap::new(),
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
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(METRIC_INTERVAL_MS));
            manager.refresh_runtime_state(&app);
        });
    }

    pub fn bootstrap(&self) -> BootstrapPayload {
        let inner = self.inner.lock().expect("state poisoned");

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

        BootstrapPayload {
            project: inner.project.clone(),
            sessions,
            summary: inner.workspace_summary.clone(),
            activity_log: inner.activity_log.clone(),
            metrics,
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
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.preferences.default_session_strategy = strategy;
            update_workspace_summary(&mut inner);
            inner.preferences.clone()
        };
        self.emit_workspace_state(app);
        preferences
    }

    pub fn load_project(&self, app: &AppHandle, candidate_path: String) -> Result<ProjectState, String> {
        let next_project = inspect_project(Path::new(&candidate_path))?;
        self.handle_project_changed(app, next_project.path.as_ref().map(PathBuf::from))?;

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.project = next_project.clone();
            update_workspace_summary(&mut inner);
        }

        self.emit_workspace_state(app);
        Ok(next_project)
    }

    pub fn refresh_project(&self, app: &AppHandle) -> Result<ProjectState, String> {
        let project_path = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.project.path.clone()
        };

        match project_path {
            Some(path) => self.load_project(app, path),
            None => Ok(self.bootstrap().project),
        }
    }

    pub fn create_session(self: &Arc<Self>, app: &AppHandle, input: CreateSessionInput) -> Result<SessionSummary, String> {
        let (project, session_count, preferences) = {
            let inner = self.inner.lock().expect("state poisoned");
            (inner.project.clone(), inner.sessions.len(), inner.preferences.clone())
        };

        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Open a project folder before starting an agent session.".to_string())?;

        let workspace_strategy = input
            .workspace_strategy
            .unwrap_or(preferences.default_session_strategy);
        if workspace_strategy == SessionWorkspaceStrategy::GitWorktree && !project.is_git_repo {
            return Err(
                "Git Worktree mode requires a Git repository. Use Sandbox Copy mode for plain folders."
                    .to_string(),
            );
        }

        let label = input
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Agent {:02}", session_count + 1));

        let session_id = format!("{}-{}", create_timestamp(), create_token());
        let workspace = self.create_session_workspace(app, &project, &label, workspace_strategy)?;

        let cols = input.cols.unwrap_or(120);
        let rows = input.rows.unwrap_or(32);
        let shell = "powershell.exe".to_string();

        let handles = match self.spawn_session_terminal(
            app.clone(),
            session_id.clone(),
            workspace.workspace_path.clone(),
            cols,
            rows,
            workspace_strategy,
            workspace.branch_name.clone(),
        ) {
            Ok(handles) => handles,
            Err(error) => {
                let _ = self.cleanup_detached_session_workspace(
                    app,
                    Some(PathBuf::from(&project_path)),
                    &workspace.workspace_path,
                    workspace.branch_name.as_deref(),
                );
                return Err(error);
            }
        };

        let summary = SessionSummary {
            id: session_id.clone(),
            label,
            project_root: project_path,
            cwd: path_to_string(&workspace.workspace_path),
            workspace_path: path_to_string(&workspace.workspace_path),
            workspace_strategy,
            branch_name: workspace.branch_name.clone(),
            status: SessionStatus::Starting,
            cleanup_state: CleanupState::Active,
            shell,
            pid: handles.pid,
            created_at: now_millis(),
            startup_command: input.startup_command.clone(),
            exit_code: None,
            error: None,
            metrics: ProcessMetrics::default(),
        };

        let mut record = SessionRecord {
            summary: summary.clone(),
            master: handles.master,
            writer: handles.writer,
            killer: handles.killer,
            terminal_size: TerminalSize { cols, rows },
            close_requested: false,
            finalized: false,
            command_buffer: String::new(),
            history: Vec::new(),
            modified_paths: Vec::new(),
            sandbox_state: workspace.sandbox_state,
            tracked_process_ids: handles.pid.into_iter().collect(),
            last_cpu_total_seconds: None,
            last_sampled_at: None,
        };

        if let Some(command) = input
            .startup_command
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            append_history_entry(&mut record.history, command, "startup");
            write_terminal(&record.writer, format!("{command}\r").as_bytes())?;
        }

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.sessions.insert(session_id, record);
            update_workspace_summary(&mut inner);
        }

        emit_event(app, EVENT_SESSION_STATE, &summary);
        self.emit_session_metrics(app, &summary.id);
        self.emit_session_history(app, &summary.id);
        self.emit_session_diff(app, &summary.id);
        self.emit_workspace_state(app);
        Ok(summary)
    }

    pub fn send_input(&self, session_id: &str, data: &str) -> Result<(), String> {
        let writer = {
            let mut inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            track_command_input(&mut record.command_buffer, &mut record.history, data);
            record.writer.clone()
        };
        write_terminal(&writer, data.as_bytes())
    }

    pub fn resize_session(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }

        let master = {
            let mut inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            if record.terminal_size.cols == cols && record.terminal_size.rows == rows {
                return Ok(());
            }
            record.terminal_size = TerminalSize { cols, rows };
            record.master.clone()
        };

        resize_terminal(&master, cols, rows)
    }

    pub fn close_session(self: &Arc<Self>, app: &AppHandle, session_id: &str) -> Result<(), String> {
        let (pid, killer, should_emit) = {
            let mut inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;

            if !record.close_requested {
                record.close_requested = true;
                if matches!(
                    record.summary.status,
                    SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Error
                ) {
                    record.summary.status = SessionStatus::Closing;
                    record.summary.error = None;
                    record.modified_paths.clear();
                }
            }

            (record.summary.pid, record.killer.clone(), true)
        };

        if should_emit {
            self.emit_session_diff(app, session_id);
            self.emit_session_state(app, session_id);
            self.emit_workspace_state(app);
        }

        let _ = kill_with_killer(&killer);
        let _ = terminate_process_id(pid);

        let start = now_millis();
        loop {
            let done = {
                let inner = self.inner.lock().expect("state poisoned");
                inner
                    .sessions
                    .get(session_id)
                    .map(|record| record.finalized)
                    .unwrap_or(true)
            };
            if done {
                break;
            }
            if now_millis() - start >= CLOSE_TIMEOUT_MS as i64 {
                self.finalize_session(
                    app.clone(),
                    session_id.to_string(),
                    None,
                    Some("Sentinel forced this session to close after the shell stopped responding.".to_string()),
                );
                break;
            }
            thread::sleep(Duration::from_millis(80));
        }

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.sessions.remove(session_id);
            update_workspace_summary(&mut inner);
        }
        self.emit_workspace_state(app);
        Ok(())
    }

    pub fn ensure_ide_terminal(self: &Arc<Self>, app: &AppHandle) -> Result<IdeTerminalState, String> {
        let project = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.project.clone()
        };

        let project_path = match project.path.clone() {
            Some(path) => PathBuf::from(path),
            None => {
                let state = IdeTerminalState::idle();
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    inner.ide.record = None;
                }
                emit_event(app, EVENT_IDE_STATE, &state);
                return Ok(state);
            }
        };

        let (workspace_path, modified_paths) = self.ensure_ide_workspace(app, &project)?;
        let workspace_path_string = path_to_string(&workspace_path);
        let should_reuse = {
            let inner = self.inner.lock().expect("state poisoned");
            if let Some(record) = inner.ide.record.as_ref() {
                record.state.workspace_path.as_deref() == Some(workspace_path_string.as_str())
                    && !matches!(record.state.status, IdeStatus::Closed | IdeStatus::Error)
            } else {
                false
            }
        };

        if should_reuse {
            let inner = self.inner.lock().expect("state poisoned");
            return Ok(
                inner
                    .ide
                    .record
                    .as_ref()
                    .map(|record| record.state.clone())
                    .unwrap_or_else(IdeTerminalState::idle),
            );
        }

        self.close_ide_terminal(app)?;

        let handles = self.spawn_ide_terminal(
            app.clone(),
            workspace_path.clone(),
            120,
            28,
            project_path,
        )?;
        let state = IdeTerminalState {
            status: IdeStatus::Starting,
            cwd: Some(workspace_path_string.clone()),
            workspace_path: Some(workspace_path_string),
            shell: "powershell.exe".to_string(),
            pid: handles.pid,
            created_at: Some(now_millis()),
            exit_code: None,
            error: None,
            modified_paths,
        };

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.ide.record = Some(IdeRecord {
                state: state.clone(),
                master: handles.master,
                writer: handles.writer,
                killer: handles.killer,
                terminal_size: TerminalSize { cols: 120, rows: 28 },
                close_requested: false,
                finalized: false,
            });
        }

        emit_event(app, EVENT_IDE_STATE, &state);
        Ok(state)
    }

    pub fn send_ide_terminal_input(self: &Arc<Self>, app: &AppHandle, data: &str) -> Result<(), String> {
        let writer = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.ide.record.as_ref().map(|record| record.writer.clone())
        };

        match writer {
            Some(writer) => write_terminal(&writer, data.as_bytes()),
            None => {
                let _ = self.ensure_ide_terminal(app)?;
                let writer = {
                    let inner = self.inner.lock().expect("state poisoned");
                    inner
                        .ide
                        .record
                        .as_ref()
                        .map(|record| record.writer.clone())
                        .ok_or_else(|| "IDE terminal is unavailable.".to_string())?
                };
                write_terminal(&writer, data.as_bytes())
            }
        }
    }

    pub fn resize_ide_terminal(self: &Arc<Self>, app: &AppHandle, cols: u16, rows: u16) -> Result<(), String> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }

        let master = {
            let mut inner = self.inner.lock().expect("state poisoned");
            if inner.ide.record.is_none() {
                drop(inner);
                let _ = self.ensure_ide_terminal(app)?;
                inner = self.inner.lock().expect("state poisoned");
            }

            let record = inner
                .ide
                .record
                .as_mut()
                .ok_or_else(|| "IDE terminal is unavailable.".to_string())?;
            if record.terminal_size.cols == cols && record.terminal_size.rows == rows {
                return Ok(());
            }
            record.terminal_size = TerminalSize { cols, rows };
            record.master.clone()
        };

        resize_terminal(&master, cols, rows)
    }

    pub fn write_ide_file(self: &Arc<Self>, app: &AppHandle, relative_path: &str, content: &str) -> Result<(), String> {
        let project = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.project.clone()
        };
        let _ = self.ensure_ide_workspace(app, &project)?;
        let workspace_path = {
            let inner = self.inner.lock().expect("state poisoned");
            inner
                .ide
                .workspace_path
                .clone()
                .ok_or_else(|| "IDE workspace is unavailable.".to_string())?
        };
        write_workspace_file(&workspace_path, relative_path, content)?;
        self.refresh_runtime_state(app);
        Ok(())
    }

    pub fn apply_ide_workspace(self: &Arc<Self>, app: &AppHandle) -> Result<SessionApplyResult, String> {
        let (project_root, workspace_path, sandbox_state) = {
            let inner = self.inner.lock().expect("state poisoned");
            (
                inner.project.path.clone(),
                inner.ide.workspace_path.clone(),
                inner.ide.sandbox_state.clone(),
            )
        };

        let project_root = project_root
            .map(PathBuf::from)
            .ok_or_else(|| "IDE workspace is unavailable.".to_string())?;
        let workspace_path = workspace_path.ok_or_else(|| "IDE workspace is unavailable.".to_string())?;
        let sandbox_state = sandbox_state.ok_or_else(|| "IDE workspace is unavailable.".to_string())?;

        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Apply IDE workspace changes to project",
            path_to_string(&workspace_path),
            None,
        );

        match apply_ide_workspace_impl(&project_root, &workspace_path, sandbox_state) {
            Ok(applied) => {
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    inner.ide.sandbox_state = Some(applied.sandbox_state.clone());
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.modified_paths = applied.modified_paths.clone();
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    if applied.result.conflicts.is_empty() { "completed" } else { "failed" },
                    "Apply IDE workspace changes to project",
                    path_to_string(&workspace_path),
                    Some(if applied.result.conflicts.is_empty() {
                        format!("{} file(s) applied", applied.result.applied_paths.len())
                    } else {
                        format!(
                            "{} applied, {} conflicts",
                            applied.result.applied_paths.len(),
                            applied.result.conflicts.len()
                        )
                    }),
                );
                self.refresh_runtime_state(app);
                self.emit_ide_state(app);
                Ok(applied.result)
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Apply IDE workspace changes to project",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    pub fn discard_ide_workspace_changes(self: &Arc<Self>, app: &AppHandle) -> Result<(), String> {
        let (project_root, workspace_path) = {
            let inner = self.inner.lock().expect("state poisoned");
            (inner.project.path.clone(), inner.ide.workspace_path.clone())
        };

        let project_root = project_root
            .map(PathBuf::from)
            .ok_or_else(|| "IDE workspace is unavailable.".to_string())?;
        let workspace_path = workspace_path.ok_or_else(|| "IDE workspace is unavailable.".to_string())?;

        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Discard IDE workspace changes",
            path_to_string(&workspace_path),
            None,
        );

        match discard_ide_workspace_impl(&project_root, &workspace_path) {
            Ok(discarded) => {
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    inner.ide.sandbox_state = Some(discarded.sandbox_state);
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.modified_paths = discarded.modified_paths;
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Discard IDE workspace changes",
                    path_to_string(&workspace_path),
                    None,
                );
                self.emit_ide_state(app);
                Ok(())
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Discard IDE workspace changes",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    pub fn read_file(&self, file_path: &str) -> String {
        fs::read_to_string(file_path).unwrap_or_default()
    }

    pub fn read_file_diff(&self, session_id: &str, file_path: &str) -> String {
        let summary = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.sessions.get(session_id).map(|record| record.summary.clone())
        };
        let Some(summary) = summary else {
            return String::new();
        };
        if summary.workspace_strategy != SessionWorkspaceStrategy::GitWorktree {
            return String::new();
        }
        run_git_command(None, Path::new(&summary.workspace_path), ["diff", "HEAD", "--", file_path])
            .unwrap_or_default()
    }

    pub fn write_session_file(&self, session_id: &str, relative_path: &str, content: &str) -> Result<(), String> {
        let workspace_path = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            PathBuf::from(&record.summary.workspace_path)
        };
        write_workspace_file(&workspace_path, relative_path, content)
    }

    pub fn apply_session(&self, app: &AppHandle, session_id: &str) -> Result<SessionApplyResult, String> {
        let session_info = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session or project not found.".to_string())?;
            (
                record.summary.clone(),
                record.modified_paths.clone(),
                record.sandbox_state.clone(),
            )
        };

        let (summary, modified_paths, sandbox_state) = session_info;

        if summary.workspace_strategy == SessionWorkspaceStrategy::GitWorktree {
            run_git_command(
                Some(app),
                Path::new(&summary.project_root),
                ["merge", summary.branch_name.as_deref().unwrap_or("")],
            )?;
            return Ok(SessionApplyResult {
                session_id: summary.id,
                workspace_strategy: SessionWorkspaceStrategy::GitWorktree,
                applied_paths: modified_paths,
                conflicts: Vec::new(),
            });
        }

        let sandbox_state = sandbox_state.ok_or_else(|| "Sandbox session state is unavailable.".to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Apply sandbox changes to project",
            summary.workspace_path.clone(),
            None,
        );

        match apply_sandbox_workspace(
            &summary.id,
            Path::new(&summary.project_root),
            Path::new(&summary.workspace_path),
            sandbox_state,
        ) {
            Ok(applied) => {
                let refreshed = refresh_sandbox_workspace_diffs(
                    Path::new(&summary.workspace_path),
                    &SandboxWorkspaceState {
                        baseline_hashes: applied.next_baseline_hashes.clone(),
                        scan_cache: applied.next_cache.clone(),
                    },
                )?;
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    if let Some(record) = inner.sessions.get_mut(session_id) {
                        record.sandbox_state = Some(SandboxWorkspaceState {
                            baseline_hashes: applied.next_baseline_hashes.clone(),
                            scan_cache: refreshed.1.clone(),
                        });
                        record.modified_paths = refreshed.0.clone();
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    if applied.result.conflicts.is_empty() { "completed" } else { "failed" },
                    "Apply sandbox changes to project",
                    summary.workspace_path.clone(),
                    Some(if applied.result.conflicts.is_empty() {
                        format!("{} file(s) applied", applied.result.applied_paths.len())
                    } else {
                        format!(
                            "{} applied, {} conflicts",
                            applied.result.applied_paths.len(),
                            applied.result.conflicts.len()
                        )
                    }),
                );
                self.emit_session_diff(app, session_id);
                Ok(applied.result)
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Apply sandbox changes to project",
                    summary.workspace_path,
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    pub fn commit_session(&self, app: &AppHandle, session_id: &str, message: &str) -> Result<(), String> {
        let summary = {
            let inner = self.inner.lock().expect("state poisoned");
            inner
                .sessions
                .get(session_id)
                .map(|record| record.summary.clone())
                .ok_or_else(|| "Session or project not found.".to_string())?
        };
        if summary.workspace_strategy != SessionWorkspaceStrategy::GitWorktree {
            return Err("Commit is only available for Git Worktree sessions.".to_string());
        }
        run_git_command(Some(app), Path::new(&summary.workspace_path), ["add", "."])?;
        run_git_command(
            Some(app),
            Path::new(&summary.workspace_path),
            ["commit", "-m", message],
        )?;
        Ok(())
    }

    pub fn discard_session_changes(&self, app: &AppHandle, session_id: &str) -> Result<(), String> {
        let session_info = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session or project not found.".to_string())?;
            (record.summary.clone(), record.sandbox_state.clone())
        };

        let (summary, sandbox_state) = session_info;
        if summary.workspace_strategy == SessionWorkspaceStrategy::GitWorktree {
            run_git_command(Some(app), Path::new(&summary.workspace_path), ["reset", "--hard"])?;
            run_git_command(Some(app), Path::new(&summary.workspace_path), ["clean", "-fd"])?;
            return Ok(());
        }

        let _ = sandbox_state.ok_or_else(|| "Sandbox session state is unavailable.".to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Discard sandbox changes",
            summary.workspace_path.clone(),
            None,
        );

        match discard_sandbox_workspace(
            Path::new(&summary.project_root),
            Path::new(&summary.workspace_path),
        ) {
            Ok(next_state) => {
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    if let Some(record) = inner.sessions.get_mut(session_id) {
                        record.sandbox_state = Some(next_state);
                        record.modified_paths.clear();
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Discard sandbox changes",
                    summary.workspace_path,
                    None,
                );
                self.emit_session_diff(app, session_id);
                Ok(())
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Discard sandbox changes",
                    summary.workspace_path,
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    pub fn reveal_in_file_explorer(&self, file_path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let status = Command::new("explorer.exe")
                .args(["/select,", file_path])
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to reveal file in File Explorer.".to_string())
            }
        }
        #[cfg(not(windows))]
        {
            let path = Path::new(file_path);
            let parent = path.parent().unwrap_or(path);
            self.open_in_system_editor(parent.to_string_lossy().as_ref())
        }
    }

    pub fn open_in_system_editor(&self, file_path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let status = Command::new("cmd")
                .args(["/C", "start", "", file_path])
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
        #[cfg(target_os = "macos")]
        {
            let status = Command::new("open")
                .arg(file_path)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let status = Command::new("xdg-open")
                .arg(file_path)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
    }

    pub fn dispose(&self, _app: &AppHandle) {
        let (session_pids, ide_pid, ide_workspace) = {
            let inner = self.inner.lock().expect("state poisoned");
            (
                inner
                    .sessions
                    .values()
                    .map(|record| record.summary.pid)
                    .collect::<Vec<_>>(),
                inner.ide.record.as_ref().and_then(|record| record.state.pid),
                inner.ide.workspace_path.clone(),
            )
        };

        for pid in session_pids {
            let _ = terminate_process_id(pid);
        }
        let _ = terminate_process_id(ide_pid);
        if let Some(workspace_path) = ide_workspace {
            let _ = fs::remove_dir_all(workspace_path);
        }
    }
}

fn emit_event<T: serde::Serialize>(app: &AppHandle, event: &str, payload: &T) {
    let _ = app.emit(event, payload);
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn create_token() -> String {
    format!("{:06x}", TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed) & 0x00ff_ffff)
}

fn create_timestamp() -> String {
    now_millis().to_string()
}

fn round(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

fn path_to_string(path: &Path) -> String {
    let value = path.to_string_lossy().to_string();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}

fn sanitize_segment(input: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for character in input.trim().to_lowercase().chars() {
        let keep = character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
        if keep {
            output.push(character);
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed
    }
}

fn normalize_relative_path(relative_path: &str) -> Result<String, String> {
    let replaced = relative_path.trim().replace('/', std::path::MAIN_SEPARATOR_STR);
    let path = Path::new(&replaced);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "Refusing to access a path outside the workspace: {relative_path}"
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "Refusing to access a path outside the workspace: {relative_path}"
                ));
            }
        }
    }

    Ok(path_to_string(&normalized))
}

fn resolve_workspace_target(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let normalized_relative = normalize_relative_path(relative_path)?;
    let resolved = root.join(&normalized_relative);
    if !resolved.starts_with(root) {
        return Err(format!(
            "Refusing to access a path outside the workspace: {relative_path}"
        ));
    }
    Ok(resolved)
}

fn should_skip_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".next"
            | ".turbo"
            | ".venv"
            | "node_modules"
            | "dist"
            | "out"
            | "build"
            | "coverage"
            | "__pycache__"
    )
}

fn should_link_directory(name: &str) -> bool {
    matches!(name, "node_modules" | ".venv" | "venv" | ".tox" | ".yarn" | ".pnpm-store")
}

fn inspect_project(candidate_path: &Path) -> Result<ProjectState, String> {
    let requested_path = candidate_path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut project_root = requested_path.clone();
    let mut branch = None;
    let mut is_git_repo = false;

    if let Ok(root) = run_git_command(None, &requested_path, ["rev-parse", "--show-toplevel"]) {
        project_root = PathBuf::from(root);
        branch = run_git_command(None, &project_root, ["branch", "--show-current"]).ok();
        is_git_repo = true;
    }

    Ok(ProjectState {
        path: Some(path_to_string(&project_root)),
        name: project_root
            .file_name()
            .and_then(OsStr::to_str)
            .map(str::to_string),
        branch: branch.filter(|value| !value.is_empty()),
        is_git_repo,
        tree: build_project_tree(&project_root, TREE_DEPTH)?,
    })
}

fn build_project_tree(root_path: &Path, depth: usize) -> Result<Vec<ProjectNode>, String> {
    let mut entries = fs::read_dir(root_path)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    entries.retain(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        !should_skip_directory(&name)
    });

    entries.sort_by(|left, right| {
        let left_is_dir = left.file_type().map(|value| value.is_dir()).unwrap_or(false);
        let right_is_dir = right.file_type().map(|value| value.is_dir()).unwrap_or(false);
        match (left_is_dir, right_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => left.file_name().cmp(&right.file_name()),
        }
    });
    entries.truncate(TREE_ENTRY_LIMIT);

    let mut nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|value| value.is_dir()).unwrap_or(false);
        let children = if is_dir && depth > 0 {
            build_project_tree(&path, depth - 1).ok()
        } else {
            None
        };

        nodes.push(ProjectNode {
            name,
            path: path_to_string(&path),
            kind: if is_dir { "directory" } else { "file" }.to_string(),
            children,
        });
    }

    Ok(nodes)
}

fn run_command(file: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = Command::new(file);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.stdin(std::process::Stdio::null());
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if !stderr.is_empty() { stderr } else { stdout })
    }
}

fn run_powershell(script: &str) -> Result<String, String> {
    run_command(
        "powershell.exe",
        &["-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
        None,
    )
}

fn run_git_command<I, S>(app: Option<&AppHandle>, cwd: &Path, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args_vec = args.into_iter().map(|value| value.as_ref().to_string()).collect::<Vec<_>>();
    if let Some(app) = app {
        emit_event(
            app,
            EVENT_ACTIVITY_LOG,
            &ActivityLogEntry {
                id: format!("{}-{}", create_timestamp(), create_token()),
                timestamp: now_millis(),
                scope: "git".to_string(),
                status: "started".to_string(),
                command: format!("git -C {} {}", path_to_string(cwd), args_vec.join(" ")),
                cwd: path_to_string(cwd),
                detail: None,
            },
        );
    }
    let borrowed = args_vec.iter().map(|value| value.as_str()).collect::<Vec<_>>();
    run_command("git", &borrowed, Some(cwd))
}

fn hash_file(file_path: &Path) -> Result<String, String> {
    let content = fs::read(file_path).map_err(|error| error.to_string())?;
    let mut hasher = Sha1::new();
    hasher.update(content);
    Ok(format!("{:x}", hasher.finalize()))
}

fn create_signature(metadata: &fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis())
        .unwrap_or_default();
    format!("{}:{}", metadata.len(), modified)
}

fn copy_project_tree(project_root: &Path, workspace_path: &Path, relative_root: Option<&Path>) -> Result<(), String> {
    let source_root = relative_root.map(|value| project_root.join(value)).unwrap_or_else(|| project_root.to_path_buf());
    let target_root = relative_root.map(|value| workspace_path.join(value)).unwrap_or_else(|| workspace_path.to_path_buf());
    fs::create_dir_all(&target_root).map_err(|error| error.to_string())?;

    for entry in fs::read_dir(&source_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let entry_path = entry.path();
        let entry_name = entry.file_name();
        let name = entry_name.to_string_lossy().to_string();
        let relative_path = relative_root
            .map(|root| root.join(&entry_name))
            .unwrap_or_else(|| PathBuf::from(&entry_name));
        let target_path = workspace_path.join(&relative_path);
        let file_type = entry.file_type().map_err(|error| error.to_string())?;

        if file_type.is_dir() {
            if should_skip_directory(&name) {
                continue;
            }
            copy_project_tree(project_root, workspace_path, Some(&relative_path))?;
            continue;
        }

        if file_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(&entry_path, &target_path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn list_tracked_files(root_path: &Path) -> Result<Vec<String>, String> {
    if !root_path.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    list_tracked_files_recursive(root_path, root_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn list_tracked_files_recursive(root_path: &Path, current_path: &Path, files: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(current_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;

        if file_type.is_dir() {
            if should_skip_directory(&name) || should_link_directory(&name) {
                continue;
            }
            list_tracked_files_recursive(root_path, &path, files)?;
            continue;
        }

        if file_type.is_file() {
            let relative_path = path.strip_prefix(root_path).map_err(|error| error.to_string())?;
            files.push(path_to_string(relative_path));
        }
    }
    Ok(())
}

fn snapshot_project_hashes(root_path: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut hashes = BTreeMap::new();
    for relative_path in list_tracked_files(root_path)? {
        hashes.insert(
            relative_path.clone(),
            hash_file(&resolve_workspace_target(root_path, &relative_path)?)?,
        );
    }
    Ok(hashes)
}

fn snapshot_workspace_files(
    workspace_path: &Path,
    previous_cache: Option<&BTreeMap<String, FileFingerprint>>,
) -> Result<BTreeMap<String, FileFingerprint>, String> {
    let mut snapshots = BTreeMap::new();
    for relative_path in list_tracked_files(workspace_path)? {
        let absolute_path = resolve_workspace_target(workspace_path, &relative_path)?;
        let metadata = fs::metadata(&absolute_path).map_err(|error| error.to_string())?;
        let signature = create_signature(&metadata);
        let previous = previous_cache.and_then(|cache| cache.get(&relative_path));
        let hash = match previous {
            Some(previous) if previous.signature == signature => previous.hash.clone(),
            _ => hash_file(&absolute_path)?,
        };
        snapshots.insert(relative_path, FileFingerprint { signature, hash });
    }
    Ok(snapshots)
}

fn initialize_sandbox_repository(workspace_path: &Path) -> Result<(), String> {
    let _ = run_command("git", &["init", "-b", "sentinel-sandbox"], Some(workspace_path))
        .or_else(|_| run_command("git", &["init"], Some(workspace_path)))
        .and_then(|_| run_command("git", &["checkout", "-B", "sentinel-sandbox"], Some(workspace_path)));

    let exclude_path = workspace_path.join(".git").join("info").join("exclude");
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = [
        ".git", ".next", ".turbo", ".venv", "node_modules", "dist", "out", "build", "coverage",
        "__pycache__", "venv", ".tox", ".yarn", ".pnpm-store",
    ]
    .iter()
    .map(|entry| format!("{entry}/"))
    .collect::<Vec<_>>()
    .join("\n");
    fs::write(&exclude_path, format!("{content}\n")).map_err(|error| error.to_string())?;

    let _ = run_command("git", &["config", "user.name", "Sentinel"], Some(workspace_path));
    let _ = run_command("git", &["config", "user.email", "sentinel@local.invalid"], Some(workspace_path));
    let _ = run_command("git", &["add", "-A"], Some(workspace_path));
    let _ = run_command("git", &["commit", "-m", "Sentinel sandbox baseline"], Some(workspace_path))
        .or_else(|_| run_command("git", &["commit", "--allow-empty", "-m", "Sentinel sandbox baseline"], Some(workspace_path)));
    Ok(())
}

fn ensure_shared_directories(project_root: &Path, workspace_path: &Path) -> Result<(), String> {
    for directory_name in ["node_modules", ".venv", "venv", ".tox", ".yarn", ".pnpm-store"] {
        let source_path = project_root.join(directory_name);
        let destination_path = workspace_path.join(directory_name);
        if !source_path.exists() {
            continue;
        }
        let _ = fs::remove_dir_all(&destination_path);
        create_directory_link(&source_path, &destination_path)?;
    }
    Ok(())
}

fn create_directory_link(source: &Path, destination: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &path_to_string(destination),
                &path_to_string(source),
            ])
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to create directory junction.".to_string())
        }
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())
    }
}

fn collect_modified_paths(
    baseline_hashes: &BTreeMap<String, String>,
    workspace_snapshot: &BTreeMap<String, FileFingerprint>,
) -> Vec<String> {
    let all_paths = baseline_hashes
        .keys()
        .chain(workspace_snapshot.keys())
        .cloned()
        .collect::<HashSet<_>>();

    let mut modified = all_paths
        .into_iter()
        .filter(|relative_path| {
            baseline_hashes.get(relative_path)
                != workspace_snapshot.get(relative_path).map(|fingerprint| &fingerprint.hash)
        })
        .collect::<Vec<_>>();
    modified.sort();
    modified
}

fn create_sandbox_workspace(project_root: &Path, workspace_path: &Path) -> Result<SandboxWorkspaceState, String> {
    let baseline_hashes = snapshot_project_hashes(project_root)?;
    let _ = fs::remove_dir_all(workspace_path);
    fs::create_dir_all(workspace_path).map_err(|error| error.to_string())?;
    copy_project_tree(project_root, workspace_path, None)?;
    let _ = initialize_sandbox_repository(workspace_path);
    let _ = ensure_shared_directories(project_root, workspace_path);
    let scan_cache = snapshot_workspace_files(workspace_path, None)?;
    Ok(SandboxWorkspaceState {
        baseline_hashes,
        scan_cache,
    })
}

fn refresh_sandbox_workspace_diffs(
    workspace_path: &Path,
    sandbox_state: &SandboxWorkspaceState,
) -> Result<(Vec<String>, BTreeMap<String, FileFingerprint>), String> {
    let next_cache = snapshot_workspace_files(workspace_path, Some(&sandbox_state.scan_cache))?;
    Ok((collect_modified_paths(&sandbox_state.baseline_hashes, &next_cache), next_cache))
}

fn apply_sandbox_workspace(
    session_id: &str,
    project_root: &Path,
    workspace_path: &Path,
    sandbox_state: SandboxWorkspaceState,
) -> Result<ApplySandboxOutcome, String> {
    let workspace_snapshot = snapshot_workspace_files(workspace_path, Some(&sandbox_state.scan_cache))?;
    let modified_paths = collect_modified_paths(&sandbox_state.baseline_hashes, &workspace_snapshot);
    let mut conflicts = Vec::new();
    let mut applied_paths = Vec::new();
    let mut next_baseline_hashes = sandbox_state.baseline_hashes.clone();

    for relative_path in modified_paths {
        let project_file_path = resolve_workspace_target(project_root, &relative_path)?;
        let workspace_file_path = resolve_workspace_target(workspace_path, &relative_path)?;
        let baseline_hash = sandbox_state.baseline_hashes.get(&relative_path).cloned();
        let workspace_hash = workspace_snapshot
            .get(&relative_path)
            .map(|fingerprint| fingerprint.hash.clone());
        let current_project_hash = if project_file_path.exists() {
            Some(hash_file(&project_file_path)?)
        } else {
            None
        };

        if current_project_hash != baseline_hash {
            conflicts.push(SessionSyncConflict {
                path: relative_path,
                reason: "project-changed".to_string(),
                detail: Some("The file changed in the main project after this sandbox session started.".to_string()),
            });
            continue;
        }

        if let Some(workspace_hash) = workspace_hash {
            if let Some(parent) = project_file_path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(&workspace_file_path, &project_file_path).map_err(|error| error.to_string())?;
            next_baseline_hashes.insert(relative_path.clone(), workspace_hash);
        } else {
            let _ = fs::remove_file(&project_file_path);
            next_baseline_hashes.remove(&relative_path);
        }
        applied_paths.push(relative_path);
    }

    let refreshed_cache = snapshot_workspace_files(workspace_path, Some(&workspace_snapshot))?;
    Ok(ApplySandboxOutcome {
        result: SessionApplyResult {
            session_id: session_id.to_string(),
            workspace_strategy: SessionWorkspaceStrategy::SandboxCopy,
            applied_paths,
            conflicts,
        },
        next_baseline_hashes,
        next_cache: refreshed_cache,
    })
}

fn apply_ide_workspace_impl(
    project_root: &Path,
    workspace_path: &Path,
    sandbox_state: SandboxWorkspaceState,
) -> Result<IdeApplyOutcome, String> {
    let applied = apply_sandbox_workspace("ide-workspace", project_root, workspace_path, sandbox_state)?;
    let refreshed = refresh_sandbox_workspace_diffs(
        workspace_path,
        &SandboxWorkspaceState {
            baseline_hashes: applied.next_baseline_hashes.clone(),
            scan_cache: applied.next_cache.clone(),
        },
    )?;
    Ok(IdeApplyOutcome {
        result: applied.result,
        sandbox_state: SandboxWorkspaceState {
            baseline_hashes: applied.next_baseline_hashes,
            scan_cache: refreshed.1.clone(),
        },
        modified_paths: refreshed.0,
    })
}

fn discard_sandbox_workspace(project_root: &Path, workspace_path: &Path) -> Result<SandboxWorkspaceState, String> {
    create_sandbox_workspace(project_root, workspace_path)
}

fn discard_ide_workspace_impl(project_root: &Path, workspace_path: &Path) -> Result<IdeDiscardOutcome, String> {
    Ok(IdeDiscardOutcome {
        sandbox_state: discard_sandbox_workspace(project_root, workspace_path)?,
        modified_paths: Vec::new(),
    })
}

fn write_workspace_file(workspace_path: &Path, relative_path: &str, content: &str) -> Result<(), String> {
    let absolute_path = resolve_workspace_target(workspace_path, relative_path)?;
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(absolute_path, content).map_err(|error| error.to_string())
}

fn parse_git_status_output(raw: &str) -> Vec<String> {
    let entries = raw.split('\0').filter(|entry| !entry.is_empty()).collect::<Vec<_>>();
    let mut modified_paths = Vec::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = entries[index];
        if entry.len() < 4 {
            index += 1;
            continue;
        }
        let status = &entry[0..2];
        let primary_path = entry[3..].trim();
        if primary_path.is_empty() {
            index += 1;
            continue;
        }
        if status.contains('R') || status.contains('C') {
            if let Some(next_entry) = entries.get(index + 1) {
                let renamed = next_entry.trim();
                if !renamed.is_empty() {
                    modified_paths.push(renamed.to_string());
                    index += 2;
                    continue;
                }
            }
        }
        modified_paths.push(primary_path.to_string());
        index += 1;
    }
    modified_paths.sort();
    modified_paths.dedup();
    modified_paths
}

fn collect_workspace_diffs_for_record(record: &mut SessionRecord) -> Result<Vec<String>, String> {
    let workspace_path = PathBuf::from(&record.summary.workspace_path);
    if !workspace_path.exists() {
        return Ok(Vec::new());
    }

    if record.summary.workspace_strategy == SessionWorkspaceStrategy::SandboxCopy {
        let sandbox_state = record
            .sandbox_state
            .clone()
            .ok_or_else(|| "Sandbox state is unavailable.".to_string())?;
        let (modified_paths, next_cache) = refresh_sandbox_workspace_diffs(&workspace_path, &sandbox_state)?;
        record.sandbox_state = Some(SandboxWorkspaceState {
            baseline_hashes: sandbox_state.baseline_hashes,
            scan_cache: next_cache,
        });
        return Ok(modified_paths);
    }

    let raw = run_command(
        "git",
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        Some(&workspace_path),
    )?;
    Ok(parse_git_status_output(&raw))
}

fn collect_process_tree_snapshots(root_ids: &[u32]) -> Result<HashMap<u32, ProcessTreeSnapshot>, String> {
    if root_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let root_values = root_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         $rootIds=@({root_values}); \
         $children=@{{}}; \
         Get-CimInstance Win32_Process | ForEach-Object {{ \
           $parent=[string]$_.ParentProcessId; \
           if (-not $children.ContainsKey($parent)) {{ $children[$parent]=New-Object System.Collections.Generic.List[int] }}; \
           $children[$parent].Add([int]$_.ProcessId) | Out-Null; \
         }}; \
         $result=@(); \
         foreach ($rootId in $rootIds) {{ \
           $queue=New-Object 'System.Collections.Generic.Queue[int]'; \
           $seen=New-Object 'System.Collections.Generic.HashSet[int]'; \
           $queue.Enqueue([int]$rootId); \
           while ($queue.Count -gt 0) {{ \
             $current=$queue.Dequeue(); \
             if ($seen.Add($current)) {{ \
               $key=[string]$current; \
               if ($children.ContainsKey($key)) {{ \
                 foreach ($child in $children[$key]) {{ $queue.Enqueue([int]$child) }} \
               }} \
             }} \
           }}; \
           $ids=@($seen); \
           $stats=@(); \
           if ($ids.Count -gt 0) {{ $stats=Get-Process -Id $ids -ErrorAction SilentlyContinue }}; \
           $cpu=0.0; $workingSet=0; $handles=0; $threads=0; \
           foreach ($proc in $stats) {{ \
             if ($null -ne $proc.CPU) {{ $cpu += [double]$proc.CPU }}; \
             if ($null -ne $proc.WorkingSet64) {{ $workingSet += [int64]$proc.WorkingSet64 }}; \
             if ($null -ne $proc.HandleCount) {{ $handles += [int]$proc.HandleCount }}; \
             if ($null -ne $proc.Threads) {{ $threads += $proc.Threads.Count }}; \
           }}; \
           $result += [pscustomobject]@{{ \
             RootId=[int]$rootId; \
             CpuTotalSeconds=[double]$cpu; \
             WorkingSetBytes=[int64]$workingSet; \
             HandleCount=[int]$handles; \
             ThreadCount=[int]$threads; \
             ProcessCount=[int]$ids.Count; \
             ProcessIds=@($ids); \
           }}; \
         }}; \
         $result | ConvertTo-Json -Compress"
    );

    let raw = run_powershell(&script)?;
    if raw.trim().is_empty() {
        return Ok(HashMap::new());
    }

    let parsed = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| error.to_string())?;
    let snapshots = if parsed.is_array() {
        serde_json::from_value::<Vec<RawProcessTreeSnapshot>>(parsed).map_err(|error| error.to_string())?
    } else {
        vec![serde_json::from_value::<RawProcessTreeSnapshot>(parsed).map_err(|error| error.to_string())?]
    };

    Ok(snapshots
        .into_iter()
        .map(|snapshot| {
            (
                snapshot.root_id,
                ProcessTreeSnapshot {
                    cpu_total_seconds: snapshot.cpu_total_seconds,
                    working_set_bytes: snapshot.working_set_bytes,
                    handle_count: snapshot.handle_count,
                    thread_count: snapshot.thread_count,
                    process_count: snapshot.process_count,
                    process_ids: snapshot.process_ids,
                },
            )
        })
        .collect())
}

fn append_history_entry(history: &mut Vec<SessionCommandEntry>, command: &str, source: &str) {
    let normalized = command.trim();
    if normalized.is_empty() {
        return;
    }
    history.insert(
        0,
        SessionCommandEntry {
            id: format!("{}-{}", create_timestamp(), create_token()),
            command: normalized.to_string(),
            timestamp: now_millis(),
            source: source.to_string(),
        },
    );
    if history.len() > 250 {
        history.truncate(250);
    }
}

fn track_command_input(command_buffer: &mut String, history: &mut Vec<SessionCommandEntry>, data: &str) {
    for character in data.chars() {
        match character {
            '\r' | '\n' => {
                append_history_entry(history, command_buffer, "interactive");
                command_buffer.clear();
            }
            '\u{0003}' | '\u{0015}' => {
                command_buffer.clear();
            }
            '\u{0008}' | '\u{007f}' => {
                command_buffer.pop();
            }
            '\t' => command_buffer.push(character),
            value if value >= ' ' => command_buffer.push(value),
            _ => {}
        }
    }
}

fn resize_terminal(master: &SharedMaster, cols: u16, rows: u16) -> Result<(), String> {
    let master = master.lock().expect("pty poisoned");
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())
}

fn write_terminal(writer: &SharedWriter, data: &[u8]) -> Result<(), String> {
    let mut writer = writer.lock().expect("writer poisoned");
    writer.write_all(data).map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn kill_with_killer(killer: &SharedKiller) -> Result<(), String> {
    let mut killer = killer.lock().expect("killer poisoned");
    killer.kill().map_err(|error| error.to_string())
}

fn terminate_process_id(pid: Option<u32>) -> Result<(), String> {
    let Some(pid) = pid else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        let _ = run_command("taskkill", &["/PID", &pid.to_string(), "/T", "/F"], None);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to terminate process.".to_string())
        }
    }
}

fn cleanup_session_workspace(
    app: &AppHandle,
    project_root: &Path,
    workspace_path: &Path,
    workspace_strategy: SessionWorkspaceStrategy,
    branch_name: Option<&str>,
) -> Result<CleanupState, String> {
    match workspace_strategy {
        SessionWorkspaceStrategy::SandboxCopy => {
            if !workspace_path.exists() {
                return Ok(CleanupState::Removed);
            }
            fs::remove_dir_all(workspace_path)
                .map(|_| CleanupState::Removed)
                .map_err(|error| error.to_string())
        }
        SessionWorkspaceStrategy::GitWorktree => {
            let mut errors = Vec::new();
            if workspace_path.exists() {
                if let Err(error) =
                    run_git_command(Some(app), project_root, ["worktree", "remove", "--force", &path_to_string(workspace_path)])
                {
                    errors.push(error);
                }
            }
            if let Some(branch_name) = branch_name {
                if let Err(error) = run_git_command(Some(app), project_root, ["branch", "-D", branch_name]) {
                    errors.push(error);
                }
            }
            if workspace_path.exists() {
                if let Err(error) = fs::remove_dir_all(workspace_path) {
                    errors.push(error.to_string());
                }
            }
            if errors.is_empty() || !workspace_path.exists() {
                Ok(CleanupState::Removed)
            } else {
                Err(errors.join(" "))
            }
        }
    }
}

fn update_workspace_summary(inner: &mut SentinelState) {
    let active_sessions = inner
        .sessions
        .values()
        .filter(|record| {
            matches!(
                record.summary.status,
                SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Closing
            )
        })
        .collect::<Vec<_>>();

    inner.workspace_summary = WorkspaceSummary {
        active_sessions: active_sessions.len(),
        total_cpu_percent: round(
            active_sessions
                .iter()
                .map(|record| record.summary.metrics.cpu_percent)
                .sum::<f64>(),
            1,
        ),
        total_memory_mb: round(
            active_sessions
                .iter()
                .map(|record| record.summary.metrics.memory_mb)
                .sum::<f64>(),
            1,
        ),
        total_processes: active_sessions
            .iter()
            .map(|record| record.summary.metrics.process_count)
            .sum(),
        last_updated: now_millis(),
        default_session_strategy: inner.preferences.default_session_strategy,
        project_path: inner.project.path.clone(),
        project_name: inner.project.name.clone(),
        branch: inner.project.branch.clone(),
    };
}

impl SentinelManager {
    fn handle_project_changed(&self, app: &AppHandle, project_path: Option<PathBuf>) -> Result<(), String> {
        let (needs_close, old_workspace, old_project_root) = {
            let inner = self.inner.lock().expect("state poisoned");
            (
                inner.ide.record.is_some(),
                inner.ide.workspace_path.clone(),
                inner.ide.workspace_project_root.clone(),
            )
        };

        if needs_close {
            self.close_ide_terminal(app)?;
        }

        let project_changed = old_project_root != project_path;
        if project_changed {
            if let Some(workspace_path) = old_workspace {
                let _ = fs::remove_dir_all(&workspace_path);
            }
            let state = IdeTerminalState::idle();
            {
                let mut inner = self.inner.lock().expect("state poisoned");
                inner.ide = IdeRuntime::default();
            }
            emit_event(app, EVENT_IDE_STATE, &state);
        }

        Ok(())
    }

    fn close_ide_terminal(&self, app: &AppHandle) -> Result<(), String> {
        let (pid, killer) = {
            let mut inner = self.inner.lock().expect("state poisoned");
            let Some(record) = inner.ide.record.as_mut() else {
                return Ok(());
            };

            if !record.close_requested {
                record.close_requested = true;
                if matches!(record.state.status, IdeStatus::Starting | IdeStatus::Ready | IdeStatus::Error) {
                    record.state.status = IdeStatus::Closing;
                    record.state.error = None;
                }
            }

            (record.state.pid, record.killer.clone())
        };

        self.emit_ide_state(app);
        let _ = kill_with_killer(&killer);
        let _ = terminate_process_id(pid);

        let start = now_millis();
        loop {
            let done = {
                let inner = self.inner.lock().expect("state poisoned");
                inner.ide.record.as_ref().map(|record| record.finalized).unwrap_or(true)
            };
            if done {
                break;
            }
            if now_millis() - start >= CLOSE_TIMEOUT_MS as i64 {
                self.finalize_ide_terminal(
                    app.clone(),
                    None,
                    Some("Sentinel forced the IDE terminal to close after the shell stopped responding.".to_string()),
                );
                break;
            }
            thread::sleep(Duration::from_millis(80));
        }

        {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.ide.record = None;
        }
        Ok(())
    }

    fn create_session_workspace(
        &self,
        app: &AppHandle,
        project: &ProjectState,
        label: &str,
        workspace_strategy: SessionWorkspaceStrategy,
    ) -> Result<SessionWorkspaceResult, String> {
        if workspace_strategy == SessionWorkspaceStrategy::GitWorktree {
            return self.create_worktree(app, project, label);
        }

        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Project root is unavailable.".to_string())?;
        let project_name = sanitize_segment(project.name.as_deref().unwrap_or("project"));
        let session_stamp = create_timestamp();
        let token = create_token();
        let temp_root = std::env::temp_dir().join("sentinel-sandboxes").join(project_name);
        let workspace_path = temp_root.join(format!("{}-{}-{}", sanitize_segment(label), session_stamp, token));

        fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Create sandbox workspace",
            path_to_string(&workspace_path),
            None,
        );

        match create_sandbox_workspace(Path::new(&project_path), &workspace_path) {
            Ok(sandbox_state) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Create sandbox workspace",
                    path_to_string(&workspace_path),
                    None,
                );
                Ok(SessionWorkspaceResult {
                    workspace_path,
                    branch_name: None,
                    sandbox_state: Some(sandbox_state),
                })
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Create sandbox workspace",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    fn create_worktree(
        &self,
        app: &AppHandle,
        project: &ProjectState,
        label: &str,
    ) -> Result<SessionWorkspaceResult, String> {
        let project_path = project
            .path
            .clone()
            .ok_or_else(|| "Project root is unavailable.".to_string())?;
        let project_name = sanitize_segment(project.name.as_deref().unwrap_or("repo"));
        let session_stamp = create_timestamp();
        let token = create_token();
        let branch_name = format!(
            "sentinel/{}-{}-{}-{}",
            project_name,
            sanitize_segment(label),
            session_stamp,
            token
        );
        let temp_root = std::env::temp_dir().join("sentinel-worktrees").join(project_name);
        let workspace_path = temp_root.join(format!("{}-{}-{}", sanitize_segment(label), session_stamp, token));

        fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
        run_git_command(
            Some(app),
            Path::new(&project_path),
            [
                "worktree",
                "add",
                "-b",
                &branch_name,
                &path_to_string(&workspace_path),
                "HEAD",
            ],
        )?;

        Ok(SessionWorkspaceResult {
            workspace_path,
            branch_name: Some(branch_name),
            sandbox_state: None,
        })
    }

    fn cleanup_detached_session_workspace(
        &self,
        app: &AppHandle,
        project_root: Option<PathBuf>,
        workspace_path: &Path,
        branch_name: Option<&str>,
    ) -> Result<(), String> {
        if let Some(branch_name) = branch_name {
            if let Some(project_root) = project_root.as_ref() {
                let _ = run_git_command(Some(app), project_root, ["worktree", "remove", "--force", &path_to_string(workspace_path)]);
                let _ = run_git_command(Some(app), project_root, ["branch", "-D", branch_name]);
            }
        }

        fs::remove_dir_all(workspace_path)
            .map_err(|error| error.to_string())
            .or(Ok(()))
    }

    fn ensure_ide_workspace(
        &self,
        app: &AppHandle,
        project: &ProjectState,
    ) -> Result<(PathBuf, Vec<String>), String> {
        let project_root = project
            .path
            .clone()
            .ok_or_else(|| "Open a project folder before using IDE mode.".to_string())?;
        let project_root_path = PathBuf::from(project_root.clone());

        {
            let inner = self.inner.lock().expect("state poisoned");
            if let (Some(workspace_path), Some(workspace_project_root), Some(_sandbox_state)) = (
                inner.ide.workspace_path.as_ref(),
                inner.ide.workspace_project_root.as_ref(),
                inner.ide.sandbox_state.as_ref(),
            ) {
                if workspace_project_root == &project_root_path {
                    let modified_paths = inner
                        .ide
                        .record
                        .as_ref()
                        .map(|record| record.state.modified_paths.clone())
                        .unwrap_or_default();
                    return Ok((workspace_path.clone(), modified_paths));
                }
            }
        }

        let old_workspace = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.ide.workspace_path.clone()
        };
        if let Some(old_workspace) = old_workspace {
            let _ = fs::remove_dir_all(old_workspace);
        }

        let project_name = sanitize_segment(project.name.as_deref().unwrap_or("project"));
        let session_stamp = create_timestamp();
        let token = create_token();
        let temp_root = std::env::temp_dir().join("sentinel-ide").join(project_name);
        let workspace_path = temp_root.join(format!("ide-{}-{}", session_stamp, token));

        fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Create IDE workspace",
            path_to_string(&workspace_path),
            None,
        );

        match create_sandbox_workspace(Path::new(&project_root), &workspace_path) {
            Ok(sandbox_state) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Create IDE workspace",
                    path_to_string(&workspace_path),
                    None,
                );
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    inner.ide.workspace_path = Some(workspace_path.clone());
                    inner.ide.workspace_project_root = Some(project_root_path);
                    inner.ide.sandbox_state = Some(sandbox_state);
                    if let Some(record) = inner.ide.record.as_mut() {
                        record.state.cwd = Some(path_to_string(&workspace_path));
                        record.state.workspace_path = Some(path_to_string(&workspace_path));
                        record.state.modified_paths.clear();
                    }
                }
                Ok((workspace_path, Vec::new()))
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Create IDE workspace",
                    path_to_string(&workspace_path),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    fn spawn_session_terminal(
        self: &Arc<Self>,
        app: AppHandle,
        session_id: String,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        workspace_strategy: SessionWorkspaceStrategy,
        branch_name: Option<String>,
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
        cmd.env("SENTINEL_SESSION_ID", &session_id);
        cmd.env("SENTINEL_WORKSPACE_PATH", path_to_string(&cwd));
        cmd.env(
            "SENTINEL_WORKSPACE_MODE",
            match workspace_strategy {
                SessionWorkspaceStrategy::SandboxCopy => "sandbox-copy",
                SessionWorkspaceStrategy::GitWorktree => "git-worktree",
            },
        );
        cmd.env("SENTINEL_BRANCH", branch_name.unwrap_or_default());

        let mut child = pair.slave.spawn_command(cmd).map_err(|error| error.to_string())?;
        let pid = child.process_id();
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(|error| error.to_string())?;
        let writer = pair.master.take_writer().map_err(|error| error.to_string())?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let killer = Arc::new(Mutex::new(killer));

        {
            let manager = self.clone();
            let app_handle = app.clone();
            let event_session_id = session_id.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let chunk = String::from_utf8_lossy(&buffer[..size]).to_string();
                            manager.handle_session_output(&app_handle, &event_session_id, chunk);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let exit_code = child.wait().ok().map(|status| status.exit_code() as i32);
                manager.finalize_session(app_handle, session_id, exit_code, None);
            });
        }

        Ok(TerminalHandles {
            master,
            writer,
            killer,
            pid,
        })
    }

    fn spawn_ide_terminal(
        self: &Arc<Self>,
        app: AppHandle,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        project_root: PathBuf,
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
        cmd.env("SENTINEL_IDE_TERMINAL", "1");
        cmd.env("SENTINEL_PROJECT_ROOT", path_to_string(&project_root));
        cmd.env("SENTINEL_WORKSPACE_PATH", path_to_string(&cwd));
        cmd.env("SENTINEL_WORKSPACE_MODE", "sandbox-copy");

        let mut child = pair.slave.spawn_command(cmd).map_err(|error| error.to_string())?;
        let pid = child.process_id();
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(|error| error.to_string())?;
        let writer = pair.master.take_writer().map_err(|error| error.to_string())?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let killer = Arc::new(Mutex::new(killer));

        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let chunk = String::from_utf8_lossy(&buffer[..size]).to_string();
                            manager.handle_ide_output(&app_handle, chunk);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let exit_code = child.wait().ok().map(|status| status.exit_code() as i32);
                manager.finalize_ide_terminal(app_handle, exit_code, None);
            });
        }

        Ok(TerminalHandles {
            master,
            writer,
            killer,
            pid,
        })
    }
}

impl SentinelManager {
    fn handle_session_output(&self, app: &AppHandle, session_id: &str, data: String) {
        let mut emit_state = None;
        {
            let mut inner = self.inner.lock().expect("state poisoned");
            if let Some(record) = inner.sessions.get_mut(session_id) {
                if record.summary.status == SessionStatus::Starting {
                    record.summary.status = SessionStatus::Ready;
                    emit_state = Some(record.summary.clone());
                }
            }
        }

        if let Some(summary) = emit_state {
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
        {
            let mut inner = self.inner.lock().expect("state poisoned");
            if let Some(record) = inner.ide.record.as_mut() {
                if record.state.status == IdeStatus::Starting {
                    record.state.status = IdeStatus::Ready;
                    emit_state = Some(record.state.clone());
                }
            }
        }

        if let Some(state) = emit_state {
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
            let mut inner = self.inner.lock().expect("state poisoned");
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

        {
            let mut inner = self.inner.lock().expect("state poisoned");
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
                update_workspace_summary(&mut inner);
            }
        }

        self.emit_session_metrics(&app, &session_id);
        self.emit_session_diff(&app, &session_id);
        self.emit_session_state(&app, &session_id);
        self.emit_workspace_state(&app);
    }

    fn finalize_ide_terminal(&self, app: AppHandle, exit_code: Option<i32>, forced_error: Option<String>) {
        let state = {
            let mut inner = self.inner.lock().expect("state poisoned");
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
            record.state.clone()
        };
        emit_event(&app, EVENT_IDE_STATE, &state);
    }

    fn refresh_runtime_state(&self, app: &AppHandle) {
        let active_session_ids = {
            let inner = self.inner.lock().expect("state poisoned");
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
            let inner = self.inner.lock().expect("state poisoned");
            active_session_ids
                .iter()
                .filter_map(|session_id| inner.sessions.get(session_id).and_then(|record| record.summary.pid))
                .collect::<Vec<_>>()
        };

        let snapshot_map = collect_process_tree_snapshots(&root_ids).unwrap_or_default();
        let sampled_at = now_millis();

        for session_id in &active_session_ids {
            let maybe_snapshot = {
                let inner = self.inner.lock().expect("state poisoned");
                inner
                    .sessions
                    .get(session_id)
                    .and_then(|record| record.summary.pid)
                    .and_then(|pid| snapshot_map.get(&pid).cloned())
            };

            {
                let mut inner = self.inner.lock().expect("state poisoned");
                if let Some(record) = inner.sessions.get_mut(session_id) {
                    if let Some(snapshot) = maybe_snapshot {
                        let cpu_percent = match (record.last_cpu_total_seconds, record.last_sampled_at) {
                            (Some(previous_cpu), Some(previous_sampled_at))
                                if sampled_at > previous_sampled_at =>
                            {
                                let cpu_delta = snapshot.cpu_total_seconds - previous_cpu;
                                let elapsed = (sampled_at - previous_sampled_at) as f64 / 1000.0;
                                if elapsed > 0.0 {
                                    round(cpu_delta.max(0.0) / elapsed * 100.0, 1)
                                } else {
                                    0.0
                                }
                            }
                            _ => 0.0,
                        };

                        record.tracked_process_ids = snapshot.process_ids.clone();
                        record.summary.metrics = ProcessMetrics {
                            cpu_percent,
                            memory_mb: round(snapshot.working_set_bytes as f64 / 1024.0 / 1024.0, 1),
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
                }
            }
            self.emit_session_metrics(app, session_id);
        }

        let session_updates = {
            let mut inner = self.inner.lock().expect("state poisoned");
            let mut updates = Vec::new();
            for session_id in &active_session_ids {
                if let Some(record) = inner.sessions.get_mut(session_id) {
                    let next_modified_paths = collect_workspace_diffs_for_record(record).unwrap_or_default();
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
        let (workspace_path, sandbox_state) = {
            let inner = self.inner.lock().expect("state poisoned");
            (inner.ide.workspace_path.clone(), inner.ide.sandbox_state.clone())
        };
        let (Some(workspace_path), Some(sandbox_state)) = (workspace_path, sandbox_state) else {
            return Ok(());
        };

        let (modified_paths, next_cache) = refresh_sandbox_workspace_diffs(&workspace_path, &sandbox_state)?;
        let should_emit = {
            let mut inner = self.inner.lock().expect("state poisoned");
            inner.ide.sandbox_state = Some(SandboxWorkspaceState {
                baseline_hashes: sandbox_state.baseline_hashes,
                scan_cache: next_cache,
            });
            if let Some(record) = inner.ide.record.as_mut() {
                if record.state.modified_paths != modified_paths {
                    record.state.modified_paths = modified_paths;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if should_emit {
            self.emit_ide_state(app);
        }
        Ok(())
    }

    fn emit_session_state(&self, app: &AppHandle, session_id: &str) {
        let payload = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.sessions.get(session_id).map(|record| record.summary.clone())
        };
        if let Some(payload) = payload {
            emit_event(app, EVENT_SESSION_STATE, &payload);
        }
    }

    fn emit_ide_state(&self, app: &AppHandle) {
        let payload = {
            let inner = self.inner.lock().expect("state poisoned");
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
            let inner = self.inner.lock().expect("state poisoned");
            inner.sessions.get(session_id).map(|record| SessionMetricsUpdate {
                session_id: record.summary.id.clone(),
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
            let inner = self.inner.lock().expect("state poisoned");
            inner.sessions.get(session_id).map(|record| SessionHistoryUpdate {
                session_id: record.summary.id.clone(),
                entries: record.history.clone(),
            })
        };
        if let Some(payload) = payload {
            emit_event(app, EVENT_SESSION_HISTORY, &payload);
        }
    }

    fn emit_session_diff(&self, app: &AppHandle, session_id: &str) {
        let payload = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.sessions.get(session_id).map(|record| SessionDiffUpdate {
                session_id: record.summary.id.clone(),
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
            let mut inner = self.inner.lock().expect("state poisoned");
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
        let entry = {
            let mut inner = self.inner.lock().expect("state poisoned");
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
            entry
        };
        emit_event(app, EVENT_ACTIVITY_LOG, &entry);
    }
}
