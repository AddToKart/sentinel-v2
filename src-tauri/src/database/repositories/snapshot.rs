use crate::database::db_models::WorkspaceSnapshotRow;
use crate::models::SnapshotSummary;
use sqlx::SqlitePool;

pub struct WorkspaceSnapshotRepository;

impl WorkspaceSnapshotRepository {
    pub async fn create(
        pool: &SqlitePool,
        summary: &SnapshotSummary,
        snapshot_data: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO workspace_snapshots
                (id, workspace_id, name, description, created_at, snapshot_data, file_count, session_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&summary.id)
        .bind(&summary.workspace_id)
        .bind(&summary.name)
        .bind(&summary.description)
        .bind(summary.created_at)
        .bind(snapshot_data)
        .bind(summary.file_count)
        .bind(summary.session_count)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        snapshot_id: &str,
    ) -> Result<Option<WorkspaceSnapshotRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, WorkspaceSnapshotRow>(
            "SELECT * FROM workspace_snapshots WHERE id = ?1",
        )
        .bind(snapshot_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<WorkspaceSnapshotRow>, sqlx::Error> {
        let limit = limit.unwrap_or(100);
        let rows = sqlx::query_as::<_, WorkspaceSnapshotRow>(
            r#"
            SELECT * FROM workspace_snapshots
            WHERE workspace_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
