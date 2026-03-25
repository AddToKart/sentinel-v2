-- Migration 001: Core tables

CREATE TABLE IF NOT EXISTS workspaces (
    id                      TEXT    NOT NULL PRIMARY KEY,
    name                    TEXT    NOT NULL,
    project_path            TEXT,
    project_name            TEXT,
    is_git_repo             INTEGER NOT NULL DEFAULT 0,
    git_branch              TEXT,
    default_session_strategy TEXT   NOT NULL DEFAULT 'sandbox-copy',
    created_at              INTEGER NOT NULL,
    last_active_at          INTEGER NOT NULL,
    is_active               INTEGER NOT NULL DEFAULT 0,
    metadata                TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    label                   TEXT    NOT NULL,
    project_root            TEXT    NOT NULL,
    cwd                     TEXT    NOT NULL,
    workspace_path          TEXT    NOT NULL,
    workspace_strategy      TEXT    NOT NULL,
    branch_name             TEXT,
    status                  TEXT    NOT NULL DEFAULT 'starting',
    cleanup_state           TEXT    NOT NULL DEFAULT 'active',
    shell                   TEXT    NOT NULL,
    process_id              INTEGER,
    created_at              INTEGER NOT NULL,
    startup_command         TEXT,
    exit_code               INTEGER,
    error_message           TEXT,
    cpu_percent             REAL    NOT NULL DEFAULT 0.0,
    memory_mb               REAL    NOT NULL DEFAULT 0.0,
    thread_count            INTEGER NOT NULL DEFAULT 0,
    handle_count            INTEGER NOT NULL DEFAULT 0,
    process_count           INTEGER NOT NULL DEFAULT 0,
    last_metrics_update     INTEGER
);

CREATE TABLE IF NOT EXISTS tabs (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    tab_type                TEXT    NOT NULL DEFAULT 'terminal',
    label                   TEXT    NOT NULL,
    status                  TEXT    NOT NULL DEFAULT 'starting',
    cwd                     TEXT    NOT NULL,
    shell                   TEXT    NOT NULL,
    process_id              INTEGER,
    created_at              INTEGER NOT NULL,
    exit_code               INTEGER,
    error_message           TEXT,
    cpu_percent             REAL    NOT NULL DEFAULT 0.0,
    memory_mb               REAL    NOT NULL DEFAULT 0.0,
    thread_count            INTEGER NOT NULL DEFAULT 0,
    handle_count            INTEGER NOT NULL DEFAULT 0,
    process_count           INTEGER NOT NULL DEFAULT 0,
    last_metrics_update     INTEGER
);

CREATE TABLE IF NOT EXISTS ide_terminal (
    id                      TEXT    NOT NULL PRIMARY KEY,
    workspace_id            TEXT    NOT NULL UNIQUE REFERENCES workspaces(id) ON DELETE CASCADE,
    status                  TEXT    NOT NULL DEFAULT 'idle',
    cwd                     TEXT,
    workspace_path          TEXT,
    shell                   TEXT    NOT NULL,
    process_id              INTEGER,
    created_at              INTEGER NOT NULL,
    exit_code               INTEGER,
    error_message           TEXT,
    modified_paths          TEXT    NOT NULL DEFAULT '[]'
);
