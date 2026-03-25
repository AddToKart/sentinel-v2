use crate::models::{
    ActivityLogEntry, AuditLogEntry, BootstrapPayload, CleanupState, CommandHistoryEntry,
    CreateSessionInput, FileChangeEntry, IdeStatus, IdeTerminalState, ProcessMetrics, ProjectNode,
    ProjectState, SessionApplyResult, SessionCommandEntry, SessionCommitResult, SessionDiffUpdate,
    SessionHistoryUpdate, SessionMetricsUpdate, SessionStatus, SessionSummary, SessionSyncConflict,
    SessionWorkspaceStrategy, SnapshotSummary, TabMetricsUpdate, TabOutputEvent, TabStateUpdate,
    TabStatus, TabSummary, TabType, WorkspaceAnalytics, WorkspaceContext, WorkspacePreferences,
    WorkspaceRemovedEvent, WorkspaceSummary,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
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
use tauri::{AppHandle, Emitter, Manager};

const TREE_DEPTH: usize = 3;
const TREE_ENTRY_LIMIT: usize = 28;
const METRIC_INTERVAL_MS: u64 = 1_000;
const DIFF_INTERVAL_MS: i64 = 2_000;
const METRIC_PERSIST_INTERVAL_MS: i64 = 3_000;
const METRIC_PERSIST_CPU_DELTA: f64 = 1.0;
const METRIC_PERSIST_MEMORY_DELTA_MB: f64 = 4.0;
const CLOSE_TIMEOUT_MS: u64 = 4_000;

const EVENT_SESSION_OUTPUT: &str = "sentinel:session-output";
const EVENT_SESSION_STATE: &str = "sentinel:session-state";
const EVENT_PROJECT_STATE: &str = "sentinel:project-state";
const EVENT_IDE_OUTPUT: &str = "sentinel:ide-terminal-output";
const EVENT_IDE_STATE: &str = "sentinel:ide-terminal-state";
const EVENT_SESSION_METRICS: &str = "sentinel:session-metrics";
const EVENT_SESSION_HISTORY: &str = "sentinel:session-history";
const EVENT_SESSION_DIFF: &str = "sentinel:session-diff";
const EVENT_WORKSPACE_STATE: &str = "sentinel:workspace-state";
const EVENT_WORKSPACE_CREATED: &str = "sentinel:workspace-created";
const EVENT_WORKSPACE_UPDATED: &str = "sentinel:workspace-updated";
const EVENT_WORKSPACE_SWITCHED: &str = "sentinel:workspace-switched";
const EVENT_WORKSPACE_REMOVED: &str = "sentinel:workspace-removed";
const EVENT_ACTIVITY_LOG: &str = "sentinel:activity-log";
const EVENT_TAB_OUTPUT: &str = "sentinel:tab-output";
const EVENT_TAB_STATE: &str = "sentinel:tab-state";
const EVENT_TAB_METRICS: &str = "sentinel:tab-metrics";

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
    project_root: Option<String>,
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum SessionShutdownMode {
    Stop,
    Pause,
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
    last_persisted_metrics: ProcessMetrics,
    last_persisted_metrics_at: Option<i64>,
    last_diff_scanned_at: Option<i64>,
    shutdown_mode: SessionShutdownMode,
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

struct TabRecord {
    summary: TabSummary,
    master: SharedMaster,
    writer: SharedWriter,
    killer: SharedKiller,
    terminal_size: TerminalSize,
    close_requested: bool,
    finalized: bool,
    tracked_process_ids: Vec<u32>,
    last_cpu_total_seconds: Option<f64>,
    last_sampled_at: Option<i64>,
    last_persisted_metrics: ProcessMetrics,
    last_persisted_metrics_at: Option<i64>,
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
    tabs: HashMap<String, TabRecord>,
    ide: IdeRuntime,
    workspaces: HashMap<String, WorkspaceContext>,
    active_workspace_id: Option<String>,
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

fn metrics_changed_significantly(previous: &ProcessMetrics, next: &ProcessMetrics) -> bool {
    (previous.cpu_percent - next.cpu_percent).abs() >= METRIC_PERSIST_CPU_DELTA
        || (previous.memory_mb - next.memory_mb).abs() >= METRIC_PERSIST_MEMORY_DELTA_MB
        || previous.thread_count != next.thread_count
        || previous.handle_count != next.handle_count
        || previous.process_count != next.process_count
}

fn should_persist_metrics(
    previous: &ProcessMetrics,
    next: &ProcessMetrics,
    last_persisted_at: Option<i64>,
    sampled_at: i64,
) -> bool {
    if last_persisted_at.is_none() {
        return true;
    }

    metrics_changed_significantly(previous, next)
        || sampled_at - last_persisted_at.unwrap_or_default() >= METRIC_PERSIST_INTERVAL_MS
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
            .flat_map(|part| {
                part.split('.')
                    .filter_map(|piece| piece.parse::<u32>().ok())
            })
            .collect();
        digits.last().copied()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

include!("app.rs");
include!("persistence.rs");
include!("sqlite_queries.rs");
include!("sessions.rs");
include!("ide.rs");
include!("files.rs");
include!("sync.rs");
include!("shell_integration.rs");
include!("utilities.rs");
include!("sandbox.rs");
include!("tracking.rs");
include!("cleanup.rs");
include!("workspaces.rs");
include!("workspace.rs");
include!("terminals.rs");
include!("tabs.rs");
include!("runtime.rs");
