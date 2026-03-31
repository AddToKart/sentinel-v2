-- Migration 002: History and audit tables

CREATE TABLE IF NOT EXISTS command_history (
    id                      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    session_id              TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    command_text            TEXT    NOT NULL,
    timestamp               INTEGER NOT NULL,
    source                  TEXT    NOT NULL DEFAULT 'interactive',
    exit_code               INTEGER,
    duration_ms             INTEGER,
    cwd                     TEXT
);

CREATE TABLE IF NOT EXISTS file_changes (
    id                      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    session_id              TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    file_path               TEXT    NOT NULL,
    change_type             TEXT    NOT NULL,
    before_hash             TEXT,
    after_hash              TEXT,
    timestamp               INTEGER NOT NULL,
    file_size               INTEGER
);

CREATE TABLE IF NOT EXISTS activity_log (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    session_id              TEXT    REFERENCES sessions(id) ON DELETE SET NULL,
    timestamp               INTEGER NOT NULL,
    scope                   TEXT    NOT NULL,
    status                  TEXT    NOT NULL,
    command                 TEXT    NOT NULL,
    cwd                     TEXT    NOT NULL,
    detail                  TEXT
);

CREATE TABLE IF NOT EXISTS audit_log (
    id                      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    workspace_id            TEXT    REFERENCES workspaces(id) ON DELETE SET NULL,
    session_id              TEXT    REFERENCES sessions(id) ON DELETE SET NULL,
    tab_id                  TEXT    REFERENCES tabs(id) ON DELETE SET NULL,
    timestamp               INTEGER NOT NULL,
    action_type             TEXT    NOT NULL,
    resource_type           TEXT    NOT NULL,
    resource_id             TEXT    NOT NULL,
    details                 TEXT,
    user_id                 TEXT
);

CREATE TABLE IF NOT EXISTS preferences (
    id                      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    workspace_id            TEXT    REFERENCES workspaces(id) ON DELETE CASCADE,
    category                TEXT    NOT NULL,
    key                     TEXT    NOT NULL,
    value                   TEXT    NOT NULL,
    is_sensitive            INTEGER NOT NULL DEFAULT 0,
    updated_at              INTEGER NOT NULL,
    UNIQUE(workspace_id, category, key)
);

CREATE TABLE IF NOT EXISTS workspace_snapshots (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name                    TEXT    NOT NULL,
    description             TEXT,
    created_at              INTEGER NOT NULL,
    snapshot_data           TEXT    NOT NULL,
    file_count              INTEGER NOT NULL DEFAULT 0,
    session_count           INTEGER NOT NULL DEFAULT 0
);
