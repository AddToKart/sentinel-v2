use crate::database::db_models::AuditLogRow;
use sqlx::SqlitePool;

pub struct AuditRepository;

impl AuditRepository {
    pub async fn insert(
        pool: &SqlitePool,
        workspace_id: Option<&str>,
        session_id: Option<&str>,
        tab_id: Option<&str>,
        timestamp: i64,
        action_type: &str,
        resource_type: &str,
        resource_id: &str,
        details: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO audit_log
                (workspace_id, session_id, tab_id, timestamp, action_type,
                 resource_type, resource_id, details)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(workspace_id)
        .bind(session_id)
        .bind(tab_id)
        .bind(timestamp)
        .bind(action_type)
        .bind(resource_type)
        .bind(resource_id)
        .bind(details)
        .execute(pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<AuditLogRow>, sqlx::Error> {
        let limit = limit.unwrap_or(1000);
        let rows = sqlx::query_as::<_, AuditLogRow>(
            r#"
            SELECT * FROM audit_log
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

    pub async fn find_by_date_range(
        pool: &SqlitePool,
        workspace_id: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<AuditLogRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, AuditLogRow>(
            r#"
            SELECT * FROM audit_log
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

    pub async fn export_json(
        pool: &SqlitePool,
        workspace_id: &str,
        start: i64,
        end: i64,
    ) -> Result<String, sqlx::Error> {
        let rows = Self::find_by_date_range(pool, workspace_id, start, end).await?;
        let json = serde_json::to_string(&rows.iter().map(|r| {
            serde_json::json!({
                "id": r.id,
                "workspace_id": r.workspace_id,
                "session_id": r.session_id,
                "tab_id": r.tab_id,
                "timestamp": r.timestamp,
                "action_type": r.action_type,
                "resource_type": r.resource_type,
                "resource_id": r.resource_id,
                "details": r.details,
            })
        }).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string());
        Ok(json)
    }
}
