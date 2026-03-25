-- Migration 003: Indexes for performance

-- workspaces
CREATE INDEX IF NOT EXISTS idx_workspaces_is_active ON workspaces(is_active);
CREATE INDEX IF NOT EXISTS idx_workspaces_project_path ON workspaces(project_path);

-- sessions
CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id ON sessions(workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);

-- tabs
CREATE INDEX IF NOT EXISTS idx_tabs_workspace_id ON tabs(workspace_id);
CREATE INDEX IF NOT EXISTS idx_tabs_status ON tabs(status);

-- command_history
CREATE INDEX IF NOT EXISTS idx_command_history_session_id ON command_history(session_id);
CREATE INDEX IF NOT EXISTS idx_command_history_workspace_id ON command_history(workspace_id);
CREATE INDEX IF NOT EXISTS idx_command_history_timestamp ON command_history(timestamp);

-- file_changes
CREATE INDEX IF NOT EXISTS idx_file_changes_session_id ON file_changes(session_id);
CREATE INDEX IF NOT EXISTS idx_file_changes_workspace_id ON file_changes(workspace_id);
CREATE INDEX IF NOT EXISTS idx_file_changes_file_path ON file_changes(file_path);
CREATE INDEX IF NOT EXISTS idx_file_changes_timestamp ON file_changes(timestamp);

-- activity_log
CREATE INDEX IF NOT EXISTS idx_activity_log_workspace_id ON activity_log(workspace_id);
CREATE INDEX IF NOT EXISTS idx_activity_log_timestamp ON activity_log(timestamp);

-- audit_log
CREATE INDEX IF NOT EXISTS idx_audit_log_workspace_id ON audit_log(workspace_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_action_type ON audit_log(action_type);

-- preferences
CREATE INDEX IF NOT EXISTS idx_preferences_workspace_id ON preferences(workspace_id);
CREATE INDEX IF NOT EXISTS idx_preferences_category ON preferences(category);

-- workspace_snapshots
CREATE INDEX IF NOT EXISTS idx_workspace_snapshots_workspace_id ON workspace_snapshots(workspace_id);
CREATE INDEX IF NOT EXISTS idx_workspace_snapshots_created_at ON workspace_snapshots(created_at);
