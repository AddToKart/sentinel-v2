use crate::database::db_models::WorkspaceRow;
use crate::models::{SessionWorkspaceStrategy, WorkspaceContext};
use sqlx::SqlitePool;

pub struct WorkspaceRepository;

fn workspace_strategy_to_str(strategy: SessionWorkspaceStrategy) -> &'static str {
    match strategy {
        SessionWorkspaceStrategy::SandboxCopy => "sandbox-copy",
        SessionWorkspaceStrategy::GitWorktree => "git-worktree",
    }
}

fn workspace_metadata_json(ws: &WorkspaceContext) -> String {
    serde_json::json!({
        "mode": ws.mode,
        "repoUrl": ws.repo_url,
    })
    .to_string()
}

impl WorkspaceRepository {
    pub async fn create(pool: &SqlitePool, ws: &WorkspaceContext) -> Result<(), sqlx::Error> {
        let is_git_repo = ws.project.is_git_repo as i64;
        let strategy = workspace_strategy_to_str(ws.default_session_strategy);
        let metadata = workspace_metadata_json(ws);

        sqlx::query(
            r#"
            INSERT INTO workspaces
                (id, name, project_path, project_name, is_git_repo, git_branch,
                 default_session_strategy, created_at, last_active_at, is_active, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)
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
        .bind(metadata)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update(pool: &SqlitePool, ws: &WorkspaceContext) -> Result<(), sqlx::Error> {
        let is_git_repo = ws.project.is_git_repo as i64;
        let strategy = workspace_strategy_to_str(ws.default_session_strategy);
        let metadata = workspace_metadata_json(ws);

        sqlx::query(
            r#"
            UPDATE workspaces SET
                name = ?2,
                project_path = ?3,
                project_name = ?4,
                is_git_repo = ?5,
                git_branch = ?6,
                default_session_strategy = ?7,
                last_active_at = ?8,
                metadata = ?9
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
        .bind(metadata)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn upsert(pool: &SqlitePool, ws: &WorkspaceContext) -> Result<(), sqlx::Error> {
        let is_git_repo = ws.project.is_git_repo as i64;
        let strategy = workspace_strategy_to_str(ws.default_session_strategy);
        let metadata = workspace_metadata_json(ws);

        sqlx::query(
            r#"
            INSERT INTO workspaces
                (id, name, project_path, project_name, is_git_repo, git_branch,
                 default_session_strategy, created_at, last_active_at, is_active, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                project_path = excluded.project_path,
                project_name = excluded.project_name,
                is_git_repo = excluded.is_git_repo,
                git_branch = excluded.git_branch,
                default_session_strategy = excluded.default_session_strategy,
                last_active_at = excluded.last_active_at,
                metadata = excluded.metadata
            "#,
        )
        .bind(&ws.id)
        .bind(&ws.name)
        .bind(&ws.project.path)
        .bind(&ws.project.name)
        .bind(is_git_repo)
        .bind(&ws.project.branch)
        .bind(strategy)
        .bind(ws.created_at)
        .bind(ws.last_active_at)
        .bind(metadata)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn set_active(
        pool: &SqlitePool,
        workspace_id: &str,
        active: bool,
    ) -> Result<(), sqlx::Error> {
        if active {
            sqlx::query(
                r#"
                UPDATE workspaces
                SET is_active = CASE WHEN id = ?1 THEN 1 ELSE 0 END
                WHERE is_active != CASE WHEN id = ?1 THEN 1 ELSE 0 END
                "#,
            )
            .bind(workspace_id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query("UPDATE workspaces SET is_active = 0 WHERE is_active != 0")
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
