use crate::database::db_models::ActivityLogRow;
use crate::models::ActivityLogEntry;
use sqlx::SqlitePool;

pub struct ActivityRepository;

impl ActivityRepository {
    pub async fn insert(
        pool: &SqlitePool,
        workspace_id: &str,
        entry: &ActivityLogEntry,
        session_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO activity_log
                (id, workspace_id, session_id, timestamp, scope, status, command, cwd, detail)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(&entry.id)
        .bind(workspace_id)
        .bind(session_id)
        .bind(entry.timestamp)
        .bind(&entry.scope)
        .bind(&entry.status)
        .bind(&entry.command)
        .bind(&entry.cwd)
        .bind(&entry.detail)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ActivityLogRow>, sqlx::Error> {
        let limit = limit.unwrap_or(500);
        let rows = sqlx::query_as::<_, ActivityLogRow>(
            r#"
            SELECT * FROM activity_log
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

    pub async fn find_recent(
        pool: &SqlitePool,
        limit: Option<i64>,
    ) -> Result<Vec<ActivityLogRow>, sqlx::Error> {
        let limit = limit.unwrap_or(500);
        let rows = sqlx::query_as::<_, ActivityLogRow>(
            r#"
            SELECT * FROM activity_log
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_date_range(
        pool: &SqlitePool,
        workspace_id: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<ActivityLogRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ActivityLogRow>(
            r#"
            SELECT * FROM activity_log
            WHERE workspace_id = ?1 AND timestamp BETWEEN ?2 AND ?3
            ORDER BY timestamp DESC
            "#,
        )
        .bind(workspace_id)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
