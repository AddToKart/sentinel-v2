use crate::database::db_models::{TabRow, WorkspaceMemberRow};
use crate::models::{TabStatus, TabSummary};
use sqlx::SqlitePool;

pub struct TabRepository;

impl TabRepository {
    pub async fn create(pool: &SqlitePool, t: &TabSummary) -> Result<(), sqlx::Error> {
        let status = tab_status_to_str(t.status);
        let tab_type = format!("{:?}", t.tab_type).to_lowercase();
        let pid = t.pid.map(|p| p as i64);

        sqlx::query(
            r#"
            INSERT INTO tabs
                (id, workspace_id, tab_type, label, status, cwd, shell, process_id,
                 created_at, exit_code, error_message,
                 cpu_percent, memory_mb, thread_count, handle_count, process_count)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(&t.id)
        .bind(&t.workspace_id)
        .bind(&tab_type)
        .bind(&t.label)
        .bind(status)
        .bind(&t.cwd)
        .bind(&t.shell)
        .bind(pid)
        .bind(t.created_at)
        .bind(&t.exit_code)
        .bind(&t.error)
        .bind(t.metrics.cpu_percent)
        .bind(t.metrics.memory_mb)
        .bind(t.metrics.thread_count as i64)
        .bind(t.metrics.handle_count as i64)
        .bind(t.metrics.process_count as i64)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        status: TabStatus,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let status_str = tab_status_to_str(status);
        sqlx::query(
            r#"
            UPDATE tabs SET
                status = ?2,
                exit_code = ?3,
                error_message = ?4,
                process_id = CASE WHEN ?2 IN ('closed', 'error') THEN NULL ELSE process_id END,
                cpu_percent = CASE WHEN ?2 IN ('closed', 'error') THEN 0.0 ELSE cpu_percent END,
                memory_mb = CASE WHEN ?2 IN ('closed', 'error') THEN 0.0 ELSE memory_mb END,
                thread_count = CASE WHEN ?2 IN ('closed', 'error') THEN 0 ELSE thread_count END,
                handle_count = CASE WHEN ?2 IN ('closed', 'error') THEN 0 ELSE handle_count END,
                process_count = CASE WHEN ?2 IN ('closed', 'error') THEN 0 ELSE process_count END,
                last_metrics_update = CASE WHEN ?2 IN ('closed', 'error') THEN NULL ELSE last_metrics_update END
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(status_str)
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
            UPDATE tabs SET
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

    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<TabRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, TabRow>("SELECT * FROM tabs WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: &str,
    ) -> Result<Vec<TabRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, TabRow>(
            "SELECT * FROM tabs WHERE workspace_id = ?1 ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_active(pool: &SqlitePool) -> Result<Vec<TabRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, TabRow>(
            "SELECT * FROM tabs WHERE status NOT IN ('closed', 'error') ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<TabRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, TabRow>("SELECT * FROM tabs ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn find_workspace_memberships(
        pool: &SqlitePool,
    ) -> Result<Vec<WorkspaceMemberRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, WorkspaceMemberRow>(
            "SELECT id, workspace_id FROM tabs WHERE status NOT IN ('closed', 'error')",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn mark_stale_as_closed(
        pool: &SqlitePool,
        close_message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE tabs
            SET
                status = 'closed',
                error_message = COALESCE(error_message, ?1),
                process_id = NULL,
                cpu_percent = 0.0,
                memory_mb = 0.0,
                thread_count = 0,
                handle_count = 0,
                process_count = 0,
                last_metrics_update = NULL
            WHERE status NOT IN ('closed', 'error')
            "#,
        )
        .bind(close_message)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tabs WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

fn tab_status_to_str(s: TabStatus) -> &'static str {
    match s {
        TabStatus::Starting => "starting",
        TabStatus::Ready => "ready",
        TabStatus::Closing => "closing",
        TabStatus::Closed => "closed",
        TabStatus::Error => "error",
    }
}
