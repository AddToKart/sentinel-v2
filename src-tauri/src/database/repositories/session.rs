use crate::database::db_models::SessionRow;
use crate::models::{CleanupState, SessionStatus, SessionSummary};
use sqlx::SqlitePool;

pub struct SessionRepository;

impl SessionRepository {
    pub async fn create(pool: &SqlitePool, s: &SessionSummary) -> Result<(), sqlx::Error> {
        let status = session_status_to_str(s.status);
        let cleanup = cleanup_state_to_str(s.cleanup_state);
        let strategy = format!("{:?}", s.workspace_strategy)
            .to_lowercase()
            .replace("sandboxcopy", "sandbox-copy")
            .replace("gitworktree", "git-worktree");
        let pid = s.pid.map(|p| p as i64);

        sqlx::query(
            r#"
            INSERT INTO sessions
                (id, workspace_id, label, project_root, cwd, workspace_path, workspace_strategy,
                 branch_name, status, cleanup_state, shell, process_id, created_at,
                 startup_command, exit_code, error_message,
                 cpu_percent, memory_mb, thread_count, handle_count, process_count)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                 ?17, ?18, ?19, ?20, ?21)
            ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(&s.id)
        .bind(&s.workspace_id)
        .bind(&s.label)
        .bind(&s.project_root)
        .bind(&s.cwd)
        .bind(&s.workspace_path)
        .bind(&strategy)
        .bind(&s.branch_name)
        .bind(status)
        .bind(cleanup)
        .bind(&s.shell)
        .bind(pid)
        .bind(s.created_at)
        .bind(&s.startup_command)
        .bind(&s.exit_code)
        .bind(&s.error)
        .bind(s.metrics.cpu_percent)
        .bind(s.metrics.memory_mb)
        .bind(s.metrics.thread_count as i64)
        .bind(s.metrics.handle_count as i64)
        .bind(s.metrics.process_count as i64)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        status: SessionStatus,
        cleanup: CleanupState,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let status_str = session_status_to_str(status);
        let cleanup_str = cleanup_state_to_str(cleanup);

        sqlx::query(
            r#"
            UPDATE sessions SET
                status = ?2, cleanup_state = ?3, exit_code = ?4, error_message = ?5
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(status_str)
        .bind(cleanup_str)
        .bind(exit_code)
        .bind(error)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_metrics(
        pool: &SqlitePool,
        id: &str,
        cpu_percent: f64,
        memory_mb: f64,
        thread_count: i64,
        handle_count: i64,
        process_count: i64,
        sampled_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE sessions SET
                cpu_percent = ?2, memory_mb = ?3, thread_count = ?4,
                handle_count = ?5, process_count = ?6, last_metrics_update = ?7
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(cpu_percent)
        .bind(memory_mb)
        .bind(thread_count)
        .bind(handle_count)
        .bind(process_count)
        .bind(sampled_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<SessionRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<SessionRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT * FROM sessions WHERE workspace_id = ?1 ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_active(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT * FROM sessions WHERE status NOT IN ('closed', 'error') ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT * FROM sessions ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn mark_stale_as_error(
        pool: &SqlitePool,
        error_message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET
                status = 'error',
                cleanup_state = CASE
                    WHEN cleanup_state = 'active' THEN 'preserved'
                    ELSE cleanup_state
                END,
                error_message = COALESCE(error_message, ?1)
            WHERE status NOT IN ('closed', 'error')
            "#,
        )
        .bind(error_message)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

fn session_status_to_str(s: SessionStatus) -> &'static str {
    match s {
        SessionStatus::Starting => "starting",
        SessionStatus::Ready => "ready",
        SessionStatus::Closing => "closing",
        SessionStatus::Closed => "closed",
        SessionStatus::Error => "error",
    }
}

fn cleanup_state_to_str(s: CleanupState) -> &'static str {
    match s {
        CleanupState::Active => "active",
        CleanupState::Removed => "removed",
        CleanupState::Preserved => "preserved",
        CleanupState::Failed => "failed",
    }
}
