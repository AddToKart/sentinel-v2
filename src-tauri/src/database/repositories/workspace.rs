use crate::database::db_models::WorkspaceRow;
use crate::models::WorkspaceContext;
use sqlx::SqlitePool;

pub struct WorkspaceRepository;

impl WorkspaceRepository {
    pub async fn create(pool: &SqlitePool, ws: &WorkspaceContext) -> Result<(), sqlx::Error> {
        let is_git_repo = ws.project.is_git_repo as i64;
        let strategy = format!("{:?}", ws.default_session_strategy)
            .to_lowercase()
            .replace("sandboxcopy", "sandbox-copy")
            .replace("gitworktree", "git-worktree");

        sqlx::query(
            r#"
            INSERT INTO workspaces
                (id, name, project_path, project_name, is_git_repo, git_branch,
                 default_session_strategy, created_at, last_active_at, is_active)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)
            ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(&ws.id)
        .bind(&ws.name)
        .bind(&ws.project.path)
        .bind(&ws.project.name)
        .bind(is_git_repo)
        .bind(&ws.project.branch)
        .bind(&strategy)
        .bind(ws.created_at)
        .bind(ws.last_active_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update(pool: &SqlitePool, ws: &WorkspaceContext) -> Result<(), sqlx::Error> {
        let is_git_repo = ws.project.is_git_repo as i64;
        let strategy = format!("{:?}", ws.default_session_strategy)
            .to_lowercase()
            .replace("sandboxcopy", "sandbox-copy")
            .replace("gitworktree", "git-worktree");

        sqlx::query(
            r#"
            UPDATE workspaces SET
                name = ?2,
                project_path = ?3,
                project_name = ?4,
                is_git_repo = ?5,
                git_branch = ?6,
                default_session_strategy = ?7,
                last_active_at = ?8
            WHERE id = ?1
            "#,
        )
        .bind(&ws.id)
        .bind(&ws.name)
        .bind(&ws.project.path)
        .bind(&ws.project.name)
        .bind(is_git_repo)
        .bind(&ws.project.branch)
        .bind(&strategy)
        .bind(ws.last_active_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn set_active(
        pool: &SqlitePool,
        workspace_id: &str,
        active: bool,
    ) -> Result<(), sqlx::Error> {
        let active_val = active as i64;
        // Clear all active flags first, then set the one we want
        sqlx::query("UPDATE workspaces SET is_active = 0")
            .execute(pool)
            .await?;
        if active {
            sqlx::query("UPDATE workspaces SET is_active = ?2 WHERE id = ?1")
            .bind(workspace_id)
            .bind(active_val)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<WorkspaceRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, WorkspaceRow>("SELECT * FROM workspaces WHERE id = ?1")
            .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<WorkspaceRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT * FROM workspaces ORDER BY last_active_at DESC",
        )
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn find_active(pool: &SqlitePool) -> Result<Option<WorkspaceRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT * FROM workspaces WHERE is_active = 1 LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM workspaces WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
