use crate::database::db_models::AgentFileChangeRow;
use sqlx::SqlitePool;

pub struct AgentFileChangeRepository;

impl AgentFileChangeRepository {
    pub async fn insert(
        pool: &SqlitePool,
        id: &str,
        workspace_id: &str,
        agent_id: &str,
        sandbox_id: &str,
        file_path: &str,
        operation: &str,
        diff_content: Option<&str>,
        additions: i64,
        deletions: i64,
        timestamp: i64,
        unified_status: &str,
        file_size: Option<i64>,
        is_binary: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO agent_file_changes
                (id, workspace_id, agent_id, sandbox_id, file_path, operation,
                 diff_content, additions, deletions, timestamp, unified_status,
                 file_size, is_binary)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(agent_id)
        .bind(sandbox_id)
        .bind(file_path)
        .bind(operation)
        .bind(diff_content)
        .bind(additions)
        .bind(deletions)
        .bind(timestamp)
        .bind(unified_status)
        .bind(file_size)
        .bind(is_binary)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn upsert(
        pool: &SqlitePool,
        id: &str,
        workspace_id: &str,
        agent_id: &str,
        sandbox_id: &str,
        file_path: &str,
        operation: &str,
        diff_content: Option<&str>,
        additions: i64,
        deletions: i64,
        timestamp: i64,
        unified_status: &str,
        file_size: Option<i64>,
        is_binary: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO agent_file_changes
                (id, workspace_id, agent_id, sandbox_id, file_path, operation,
                 diff_content, additions, deletions, timestamp, unified_status,
                 file_size, is_binary)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                operation = excluded.operation,
                diff_content = excluded.diff_content,
                additions = excluded.additions,
                deletions = excluded.deletions,
                timestamp = excluded.timestamp,
                unified_status = excluded.unified_status,
                file_size = excluded.file_size,
                is_binary = excluded.is_binary
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(agent_id)
        .bind(sandbox_id)
        .bind(file_path)
        .bind(operation)
        .bind(diff_content)
        .bind(additions)
        .bind(deletions)
        .bind(timestamp)
        .bind(unified_status)
        .bind(file_size)
        .bind(is_binary)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<AgentFileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AgentFileChangeRow>(
            "SELECT * FROM agent_file_changes WHERE workspace_id = ?1 ORDER BY timestamp DESC",
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_agent(
        pool: &SqlitePool,
        workspace_id: &str,
        agent_id: &str,
    ) -> Result<Vec<AgentFileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AgentFileChangeRow>(
            r#"
            SELECT * FROM agent_file_changes
            WHERE workspace_id = ?1 AND agent_id = ?2
            ORDER BY timestamp DESC
            "#,
        )
        .bind(workspace_id)
        .bind(agent_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_pending_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<AgentFileChangeRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AgentFileChangeRow>(
            r#"
            SELECT * FROM agent_file_changes
            WHERE workspace_id = ?1 AND unified_status = 'pending'
            ORDER BY timestamp DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_unified_status(
        pool: &SqlitePool,
        id: &str,
        unified_status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE agent_file_changes SET unified_status = ?1 WHERE id = ?2",
        )
        .bind(unified_status)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn update_status_by_agent_and_file(
        pool: &SqlitePool,
        workspace_id: &str,
        agent_id: &str,
        file_path: &str,
        unified_status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE agent_file_changes
            SET unified_status = ?1
            WHERE workspace_id = ?2 AND agent_id = ?3 AND file_path = ?4
            "#,
        )
        .bind(unified_status)
        .bind(workspace_id)
        .bind(agent_id)
        .bind(file_path)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM agent_file_changes WHERE workspace_id = ?1")
            .bind(workspace_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_agent(
        pool: &SqlitePool,
        workspace_id: &str,
        agent_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM agent_file_changes WHERE workspace_id = ?1 AND agent_id = ?2",
        )
        .bind(workspace_id)
        .bind(agent_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM agent_file_changes WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
