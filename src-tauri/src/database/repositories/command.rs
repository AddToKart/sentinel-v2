use crate::database::db_models::CommandHistoryRow;
use sqlx::SqlitePool;

pub struct CommandRepository;

impl CommandRepository {
    /// Insert a single command history entry.
    pub async fn insert(
        pool: &SqlitePool,
        session_id: &str,
        workspace_id: &str,
        command_text: &str,
        timestamp: i64,
        source: &str,
        cwd: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO command_history
                (session_id, workspace_id, command_text, timestamp, source, cwd)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(session_id)
        .bind(workspace_id)
        .bind(command_text)
        .bind(timestamp)
        .bind(source)
        .bind(cwd)
        .execute(pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    /// Find all commands for a session, newest first.
    pub async fn find_by_session(
        pool: &SqlitePool,
        session_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<CommandHistoryRow>, sqlx::Error> {
        let limit = limit.unwrap_or(500);
        let rows = sqlx::query_as::<_, CommandHistoryRow>(
            r#"
            SELECT * FROM command_history
            WHERE session_id = ?1
            ORDER BY timestamp DESC
            LIMIT ?2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Find all commands for a workspace, newest first.
    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<CommandHistoryRow>, sqlx::Error> {
        let limit = limit.unwrap_or(1000);
        let rows = sqlx::query_as::<_, CommandHistoryRow>(
            r#"
            SELECT * FROM command_history
            WHERE workspace_id = ?1
            ORDER BY timestamp DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Full-text search over command_text using FTS5.
    pub async fn search(
        pool: &SqlitePool,
        workspace_id: &str,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<CommandHistoryRow>, sqlx::Error> {
        let limit = limit.unwrap_or(100);
        // FTS5 query: join back to base table to get all fields
        let rows = sqlx::query_as::<_, CommandHistoryRow>(
            r#"
            SELECT ch.* FROM command_history ch
            INNER JOIN command_history_fts fts ON fts.rowid = ch.id
            WHERE ch.workspace_id = ?1
              AND command_history_fts MATCH ?2
            ORDER BY bm25(fts)
            LIMIT ?3
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
