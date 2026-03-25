use crate::database::db_models::PreferenceRow;
use sqlx::SqlitePool;

pub struct PreferenceRepository;

impl PreferenceRepository {
    pub async fn upsert_global(
        pool: &SqlitePool,
        category: &str,
        key: &str,
        value: &str,
        is_sensitive: bool,
        updated_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            DELETE FROM preferences
            WHERE workspace_id IS NULL AND category = ?1 AND key = ?2
            "#,
        )
        .bind(category)
        .bind(key)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO preferences
                (workspace_id, category, key, value, is_sensitive, updated_at)
            VALUES (NULL, ?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(category)
        .bind(key)
        .bind(value)
        .bind(is_sensitive as i64)
        .bind(updated_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_global_by_category(
        pool: &SqlitePool,
        category: &str,
    ) -> Result<Vec<PreferenceRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, PreferenceRow>(
            r#"
            SELECT * FROM preferences
            WHERE workspace_id IS NULL AND category = ?1
            ORDER BY key ASC
            "#,
        )
        .bind(category)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
