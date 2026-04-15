-- Migration 006: AI Changes Manager tables

CREATE TABLE IF NOT EXISTS agent_file_changes (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    agent_id                TEXT    NOT NULL,
    sandbox_id              TEXT    NOT NULL,
    file_path               TEXT    NOT NULL,
    operation               TEXT    NOT NULL,
    diff_content            TEXT,
    additions               INTEGER NOT NULL DEFAULT 0,
    deletions               INTEGER NOT NULL DEFAULT 0,
    timestamp               INTEGER NOT NULL,
    unified_status          TEXT    NOT NULL DEFAULT 'pending',
    file_size               INTEGER,
    is_binary               INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_agent_file_changes_workspace ON agent_file_changes(workspace_id);
CREATE INDEX IF NOT EXISTS idx_agent_file_changes_agent ON agent_file_changes(agent_id, workspace_id);
CREATE INDEX IF NOT EXISTS idx_agent_file_changes_status ON agent_file_changes(unified_status, workspace_id);
CREATE INDEX IF NOT EXISTS idx_agent_file_changes_file ON agent_file_changes(workspace_id, file_path);

CREATE TABLE IF NOT EXISTS unified_sandbox_state (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    file_path               TEXT    NOT NULL,
    source_agent_id         TEXT    NOT NULL,
    conflict_agent_ids      TEXT,
    status                  TEXT    NOT NULL DEFAULT 'clean',
    last_updated_at         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_unified_sandbox_workspace ON unified_sandbox_state(workspace_id);
CREATE INDEX IF NOT EXISTS idx_unified_sandbox_status ON unified_sandbox_state(workspace_id, status);
