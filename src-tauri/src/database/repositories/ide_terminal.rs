use crate::database::db_models::IdeTerminalRow;
use crate::models::{IdeStatus, IdeTerminalState};
use sqlx::SqlitePool;

pub struct IdeTerminalRepository;

impl IdeTerminalRepository {
    pub async fn upsert(
        pool: &SqlitePool,
        workspace_id: &str,
        state: &IdeTerminalState,
    ) -> Result<(), sqlx::Error> {
        let id = format!("ide-{workspace_id}");
        let status = ide_status_to_str(state.status);
        let process_id = state.pid.map(|value| value as i64);
        let modified_paths =
            serde_json::to_string(&state.modified_paths).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO ide_terminal
                (id, workspace_id, status, cwd, workspace_path, shell, process_id,
                 created_at, exit_code, error_message, modified_paths)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(workspace_id) DO UPDATE SET
                id = excluded.id,
                status = excluded.status,
                cwd = excluded.cwd,
                workspace_path = excluded.workspace_path,
                shell = excluded.shell,
                process_id = excluded.process_id,
                created_at = excluded.created_at,
                exit_code = excluded.exit_code,
                error_message = excluded.error_message,
                modified_paths = excluded.modified_paths
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(status)
        .bind(&state.cwd)
        .bind(&state.workspace_path)
        .bind(&state.shell)
        .bind(process_id)
        .bind(state.created_at.unwrap_or_default())
        .bind(state.exit_code)
        .bind(&state.error)
        .bind(modified_paths)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Option<IdeTerminalRow>, sqlx::Error> {
        let row =
            sqlx::query_as::<_, IdeTerminalRow>("SELECT * FROM ide_terminal WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_optional(pool)
                .await?;
        Ok(row)
    }

    pub async fn mark_stale_as_error(
        pool: &SqlitePool,
        error_message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE ide_terminal
            SET
                status = 'error',
                error_message = COALESCE(error_message, ?1)
            WHERE status NOT IN ('idle', 'closed', 'error')
            "#,
        )
        .bind(error_message)
        .execute(pool)
        .await?;
        Ok(())
    }
}

fn ide_status_to_str(status: IdeStatus) -> &'static str {
    match status {
        IdeStatus::Idle => "idle",
        IdeStatus::Starting => "starting",
        IdeStatus::Ready => "ready",
        IdeStatus::Closing => "closing",
        IdeStatus::Closed => "closed",
        IdeStatus::Error => "error",
    }
}
