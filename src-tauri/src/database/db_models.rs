#![allow(dead_code)]

/// Database row structs — separate from the API-facing models in `models.rs`.
/// These derive `sqlx::FromRow` for direct mapping from query results.

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    pub project_path: Option<String>,
    pub project_name: Option<String>,
    pub is_git_repo: i64,
    pub git_branch: Option<String>,
    pub default_session_strategy: String,
    pub created_at: i64,
    pub last_active_at: i64,
    pub is_active: i64,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkspaceMemberRow {
    pub id: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub workspace_id: String,
    pub label: String,
    pub project_root: String,
    pub cwd: String,
    pub workspace_path: String,
    pub workspace_strategy: String,
    pub branch_name: Option<String>,
    pub status: String,
    pub cleanup_state: String,
    pub shell: String,
    pub process_id: Option<i64>,
    pub created_at: i64,
    pub startup_command: Option<String>,
    pub exit_code: Option<i64>,
    pub error_message: Option<String>,
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub thread_count: i64,
    pub handle_count: i64,
    pub process_count: i64,
    pub last_metrics_update: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TabRow {
    pub id: String,
    pub workspace_id: String,
    pub tab_type: String,
    pub label: String,
    pub status: String,
    pub cwd: String,
    pub shell: String,
    pub process_id: Option<i64>,
    pub created_at: i64,
    pub exit_code: Option<i64>,
    pub error_message: Option<String>,
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub thread_count: i64,
    pub handle_count: i64,
    pub process_count: i64,
    pub last_metrics_update: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IdeTerminalRow {
    pub id: String,
    pub workspace_id: String,
    pub status: String,
    pub cwd: Option<String>,
    pub workspace_path: Option<String>,
    pub shell: String,
    pub process_id: Option<i64>,
    pub created_at: i64,
    pub exit_code: Option<i64>,
    pub error_message: Option<String>,
    pub modified_paths: String, // JSON array
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommandHistoryRow {
    pub id: i64,
    pub session_id: String,
    pub workspace_id: String,
    pub command_text: String,
    pub timestamp: i64,
    pub source: String,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FileChangeRow {
    pub id: i64,
    pub session_id: String,
    pub workspace_id: String,
    pub file_path: String,
    pub change_type: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub timestamp: i64,
    pub file_size: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActivityLogRow {
    pub id: String,
    pub workspace_id: String,
    pub session_id: Option<String>,
    pub timestamp: i64,
    pub scope: String,
    pub status: String,
    pub command: String,
    pub cwd: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: i64,
    pub workspace_id: Option<String>,
    pub session_id: Option<String>,
    pub tab_id: Option<String>,
    pub timestamp: i64,
    pub action_type: String,
    pub resource_type: String,
    pub resource_id: String,
    pub details: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PreferenceRow {
    pub id: i64,
    pub workspace_id: Option<String>,
    pub category: String,
    pub key: String,
    pub value: String,
    pub is_sensitive: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkspaceSnapshotRow {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub snapshot_data: String,
    pub file_count: i64,
    pub session_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentFileChangeRow {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub sandbox_id: String,
    pub file_path: String,
    pub operation: String,
    pub diff_content: Option<String>,
    pub additions: i64,
    pub deletions: i64,
    pub timestamp: i64,
    pub unified_status: String,
    pub file_size: Option<i64>,
    pub is_binary: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UnifiedSandboxStateRow {
    pub id: String,
    pub workspace_id: String,
    pub file_path: String,
    pub source_agent_id: String,
    pub conflict_agent_ids: Option<String>,
    pub status: String,
    pub last_updated_at: i64,
}
