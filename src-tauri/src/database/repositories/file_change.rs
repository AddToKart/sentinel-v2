use crate::database::db_models::FileChangeRow;
use sqlx::SqlitePool;

pub struct FileChangeRepository;

impl FileChangeRepository {
    pub async fn insert(
        pool: &SqlitePool,
        session_id: &str,
        workspace_id: &str,
        file_path: &str,
        change_type: &str,
        before_hash: Option<&str>,
        after_hash: Option<&str>,
        timestamp: i64,
        file_size: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO file_changes
                (session_id, workspace_id, file_path, change_type,
                 before_hash, after_hash, timestamp, file_size)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(session_id)
        .bind(workspace_id)
        .bind(file_path)
        .bind(change_type)
        .bind(before_hash)
        .bind(after_hash)
        .bind(timestamp)
        .bind(file_size)
        .execute(pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn find_by_session(
        pool: &SqlitePool,
        session_id: &str,
    ) -> Result<Vec<FileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FileChangeRow>(
            "SELECT * FROM file_changes WHERE session_id = ?1 ORDER BY timestamp DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<FileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FileChangeRow>(
            "SELECT * FROM file_changes WHERE workspace_id = ?1 ORDER BY timestamp DESC",
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_file(
        pool: &SqlitePool,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<Vec<FileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FileChangeRow>(
            r#"
            SELECT * FROM file_changes
            WHERE workspace_id = ?1 AND file_path = ?2
            ORDER BY timestamp DESC
            "#,
        )
        .bind(workspace_id)
        .bind(file_path)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_date_range(
        pool: &SqlitePool,
        workspace_id: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<FileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FileChangeRow>(
            r#"
            SELECT * FROM file_changes
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
