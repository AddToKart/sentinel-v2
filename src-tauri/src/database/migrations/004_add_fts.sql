-- Migration 004: Full-text search for command history

CREATE VIRTUAL TABLE IF NOT EXISTS command_history_fts USING fts5(
    command_text,
    cwd,
    content='command_history',
    content_rowid='id'
);

-- Triggers to keep FTS index in sync with the base table
CREATE TRIGGER IF NOT EXISTS command_history_fts_insert
    AFTER INSERT ON command_history
BEGIN
    INSERT INTO command_history_fts(rowid, command_text, cwd)
    VALUES (new.id, new.command_text, new.cwd);
END;

CREATE TRIGGER IF NOT EXISTS command_history_fts_delete
    AFTER DELETE ON command_history
BEGIN
    INSERT INTO command_history_fts(command_history_fts, rowid, command_text, cwd)
    VALUES ('delete', old.id, old.command_text, old.cwd);
END;

CREATE TRIGGER IF NOT EXISTS command_history_fts_update
    AFTER UPDATE ON command_history
BEGIN
    INSERT INTO command_history_fts(command_history_fts, rowid, command_text, cwd)
    VALUES ('delete', old.id, old.command_text, old.cwd);
    INSERT INTO command_history_fts(rowid, command_text, cwd)
    VALUES (new.id, new.command_text, new.cwd);
END;
