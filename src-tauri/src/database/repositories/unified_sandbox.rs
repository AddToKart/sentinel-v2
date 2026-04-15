use crate::database::db_models::UnifiedSandboxStateRow;
use sqlx::SqlitePool;

pub struct UnifiedSandboxRepository;

impl UnifiedSandboxRepository {
    pub async fn upsert(
        pool: &SqlitePool,
        id: &str,
        workspace_id: &str,
        file_path: &str,
        source_agent_id: &str,
        conflict_agent_ids: Option<&str>,
        status: &str,
        last_updated_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO unified_sandbox_state
                (id, workspace_id, file_path, source_agent_id, conflict_agent_ids, status, last_updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                source_agent_id = excluded.source_agent_id,
                conflict_agent_ids = excluded.conflict_agent_ids,
                status = excluded.status,
                last_updated_at = excluded.last_updated_at
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(file_path)
        .bind(source_agent_id)
        .bind(conflict_agent_ids)
        .bind(status)
        .bind(last_updated_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<UnifiedSandboxStateRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, UnifiedSandboxStateRow>(
            "SELECT * FROM unified_sandbox_state WHERE workspace_id = ?1 ORDER BY file_path ASC",
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_conflicted_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<UnifiedSandboxStateRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, UnifiedSandboxStateRow>(
            r#"
            SELECT * FROM unified_sandbox_state
            WHERE workspace_id = ?1 AND status = 'conflicted'
            ORDER BY file_path ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_status(
        pool: &SqlitePool,
        workspace_id: &str,
        file_path: &str,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE unified_sandbox_state SET status = ?1 WHERE workspace_id = ?2 AND file_path = ?3",
        )
        .bind(status)
        .bind(workspace_id)
        .bind(file_path)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM unified_sandbox_state WHERE workspace_id = ?1")
            .bind(workspace_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_file(
        pool: &SqlitePool,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM unified_sandbox_state WHERE workspace_id = ?1 AND file_path = ?2",
        )
        .bind(workspace_id)
        .bind(file_path)
        .execute(pool)
        .await?;
        Ok(())
    }
}
