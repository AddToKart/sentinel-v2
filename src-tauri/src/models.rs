use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStatus {
    Starting,
    Ready,
    Closing,
    Closed,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupState {
    Active,
    Removed,
    Preserved,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SessionWorkspaceStrategy {
    SandboxCopy,
    GitWorktree,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum IdeStatus {
    Idle,
    Starting,
    Ready,
    Closing,
    Closed,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TabType {
    Dashboard,
    Terminal,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TabStatus {
    Starting,
    Ready,
    Closing,
    Closed,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMetrics {
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub thread_count: u32,
    pub handle_count: u32,
    pub process_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetricsUpdate {
    pub session_id: String,
    pub pid: Option<u32>,
    pub process_ids: Vec<u32>,
    pub metrics: ProcessMetrics,
    pub sampled_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectNode {
    pub name: String,
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ProjectNode>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub is_git_repo: bool,
    pub tree: Vec<ProjectNode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub label: String,
    pub project_root: String,
    pub cwd: String,
    pub workspace_path: String,
    pub workspace_strategy: SessionWorkspaceStrategy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    pub status: SessionStatus,
    pub cleanup_state: CleanupState,
    pub shell: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub metrics: ProcessMetrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub active_sessions: usize,
    pub total_cpu_percent: f64,
    pub total_memory_mb: f64,
    pub total_processes: u32,
    pub last_updated: i64,
    pub default_session_strategy: SessionWorkspaceStrategy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityLogEntry {
    pub id: String,
    pub timestamp: i64,
    pub scope: String,
    pub status: String,
    pub command: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCommandEntry {
    pub id: String,
    pub command: String,
    pub timestamp: i64,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryUpdate {
    pub session_id: String,
    pub entries: Vec<SessionCommandEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDiffUpdate {
    pub session_id: String,
    pub modified_paths: Vec<String>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSyncConflict {
    pub path: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionApplyResult {
    pub session_id: String,
    pub workspace_strategy: SessionWorkspaceStrategy,
    pub applied_paths: Vec<String>,
    pub remaining_paths: Vec<String>,
    pub conflicts: Vec<SessionSyncConflict>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCommitResult {
    pub session_id: String,
    pub workspace_strategy: SessionWorkspaceStrategy,
    pub applied_paths: Vec<String>,
    pub committed_paths: Vec<String>,
    pub remaining_paths: Vec<String>,
    pub conflicts: Vec<SessionSyncConflict>,
    pub created_commit: bool,
    pub commit_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreferences {
    pub default_session_strategy: SessionWorkspaceStrategy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeTerminalState {
    pub status: IdeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    pub shell: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub modified_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub project: ProjectState,
    pub sessions: Vec<SessionSummary>,
    pub tabs: Vec<TabSummary>,
    pub summary: WorkspaceSummary,
    pub activity_log: Vec<ActivityLogEntry>,
    pub metrics: Vec<SessionMetricsUpdate>,
    pub tab_metrics: Vec<TabMetricsUpdate>,
    pub histories: Vec<SessionHistoryUpdate>,
    pub diffs: Vec<SessionDiffUpdate>,
    pub preferences: WorkspacePreferences,
    pub ide_terminal: IdeTerminalState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_build_number: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_strategy: Option<SessionWorkspaceStrategy>,
}

impl IdeTerminalState {
    pub fn idle() -> Self {
        Self {
            status: IdeStatus::Idle,
            cwd: None,
            workspace_path: None,
            shell: "powershell.exe".to_string(),
            pid: None,
            created_at: None,
            exit_code: None,
            error: None,
            modified_paths: Vec::new(),
        }
    }
}

impl Default for WorkspacePreferences {
    fn default() -> Self {
        Self {
            default_session_strategy: SessionWorkspaceStrategy::SandboxCopy,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSummary {
    pub id: String,
    pub tab_type: TabType,
    pub label: String,
    pub status: TabStatus,
    pub cwd: String,
    pub shell: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub metrics: ProcessMetrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabMetricsUpdate {
    pub tab_id: String,
    pub pid: Option<u32>,
    pub process_ids: Vec<u32>,
    pub metrics: ProcessMetrics,
    pub sampled_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabOutputEvent {
    pub tab_id: String,
    pub data: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabStateUpdate {
    pub tab_id: String,
    pub status: TabStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for WorkspaceSummary {
    fn default() -> Self {
        Self {
            active_sessions: 0,
            total_cpu_percent: 0.0,
            total_memory_mb: 0.0,
            total_processes: 0,
            last_updated: 0,
            default_session_strategy: SessionWorkspaceStrategy::SandboxCopy,
            project_path: None,
            project_name: None,
            branch: None,
        }
    }
}
