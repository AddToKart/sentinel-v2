-- Migration 005: runtime and swarm-readiness indexes

CREATE INDEX IF NOT EXISTS idx_workspaces_last_active_at
    ON workspaces(last_active_at DESC);

CREATE INDEX IF NOT EXISTS idx_sessions_workspace_created_at
    ON sessions(workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_sessions_status_created_at
    ON sessions(status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_tabs_workspace_created_at
    ON tabs(workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_tabs_status_created_at
    ON tabs(status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_command_history_workspace_timestamp_desc
    ON command_history(workspace_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_command_history_session_timestamp_desc
    ON command_history(session_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_file_changes_workspace_timestamp_desc
    ON file_changes(workspace_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_file_changes_session_timestamp_desc
    ON file_changes(session_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_activity_log_workspace_timestamp_desc
    ON activity_log(workspace_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_workspace_timestamp_desc
    ON audit_log(workspace_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_preferences_global_lookup
    ON preferences(category, key)
    WHERE workspace_id IS NULL;
