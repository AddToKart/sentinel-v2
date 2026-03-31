use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

pub mod db_models;
pub mod repositories;

static MIGRATOR: Migrator = sqlx::migrate!("./src/database/migrations");

pub struct Database {
    pool: SqlitePool,
}

fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn backup_query_for(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    format!("VACUUM INTO '{}'", escaped)
}

async fn create_backup(pool: &SqlitePool, backup_path: &Path) -> Result<(), sqlx::Error> {
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            sqlx::Error::Configuration(
                format!("Failed to create backup directory: {}", error).into(),
            )
        })?;
    }

    if backup_path.exists() {
        fs::remove_file(backup_path).map_err(|error| {
            sqlx::Error::Configuration(format!("Failed to replace backup file: {}", error).into())
        })?;
    }

    sqlx::query(&backup_query_for(backup_path))
        .execute(pool)
        .await?;
    Ok(())
}

fn prune_backups_by_prefix(
    backups_dir: &Path,
    prefix: &str,
    retain: usize,
) -> Result<(), sqlx::Error> {
    if !backups_dir.exists() {
        return Ok(());
    }

    let mut backups = fs::read_dir(backups_dir)
        .map_err(|error| {
            sqlx::Error::Configuration(format!("Failed to read backup directory: {}", error).into())
        })?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().is_file() && entry.file_name().to_string_lossy().starts_with(prefix)
        })
        .collect::<Vec<_>>();

    backups.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    let remove_count = backups.len().saturating_sub(retain);
    for entry in backups.into_iter().take(remove_count) {
        fs::remove_file(entry.path()).map_err(|error| {
            sqlx::Error::Configuration(format!("Failed to prune backup file: {}", error).into())
        })?;
    }

    Ok(())
}

async fn ensure_periodic_backup(
    pool: &SqlitePool,
    backups_dir: &Path,
    prefix: &str,
    bucket: u64,
) -> Result<(), sqlx::Error> {
    let backup_path = backups_dir.join(format!("{prefix}-{bucket}.db"));
    if !backup_path.exists() {
        create_backup(pool, &backup_path).await?;
    }
    Ok(())
}

async fn has_pending_migrations(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    if MIGRATOR.migrations.is_empty() {
        return Ok(false);
    }

    let migrations_table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations' LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .is_some();

    if !migrations_table_exists {
        return Ok(true);
    }

    let applied_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await?;

    Ok((applied_count as usize) < MIGRATOR.migrations.len())
}

impl Database {
    /// Initialize the database: create directory, open/create the DB file,
    /// configure PRAGMA settings, and run all pending migrations.
    pub async fn init(app_data_dir: &Path) -> Result<Self, sqlx::Error> {
        // Ensure the sentinel data directory exists
        if !app_data_dir.exists() {
            std::fs::create_dir_all(app_data_dir).map_err(|e| {
                sqlx::Error::Configuration(format!("Failed to create app data dir: {}", e).into())
            })?;
        }

        let db_path = app_data_dir.join("sentinel.db");
        let backups_dir = app_data_dir.join("backups");
        let database_previously_existed = db_path.exists();
        let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));

        let connect_options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(30))
            .pragma("foreign_keys", "ON")
            .pragma("cache_size", "-65536") // 64 MB page cache
            .pragma("temp_store", "MEMORY")
            .pragma("mmap_size", "268435456")
            .pragma("wal_autocheckpoint", "1000")
            .pragma("journal_size_limit", "67108864");

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .connect_with(connect_options)
            .await?;

        let pending_migrations = if database_previously_existed {
            has_pending_migrations(&pool).await?
        } else {
            !MIGRATOR.migrations.is_empty()
        };

        if database_previously_existed && pending_migrations {
            let timestamp = now_unix_seconds();
            create_backup(
                &pool,
                &backups_dir.join(format!("pre-migration-{}.db", timestamp)),
            )
            .await?;
        }

        // Run migrations embedded from the migrations directory
        MIGRATOR.run(&pool).await?;
        sqlx::query("PRAGMA optimize").execute(&pool).await?;

        if database_previously_existed {
            let timestamp = now_unix_seconds();
            let day_bucket = timestamp / 86_400;
            let week_bucket = day_bucket / 7;
            ensure_periodic_backup(&pool, &backups_dir, "daily", day_bucket).await?;
            ensure_periodic_backup(&pool, &backups_dir, "weekly", week_bucket).await?;
            prune_backups_by_prefix(&backups_dir, "daily-", 10)?;
            prune_backups_by_prefix(&backups_dir, "weekly-", 4)?;
            prune_backups_by_prefix(&backups_dir, "pre-migration-", 10)?;
        }

        Ok(Self { pool })
    }

    /// Returns a reference to the connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Runs VACUUM and ANALYZE for database maintenance.
    #[allow(dead_code)]
    pub async fn vacuum(&self) -> Result<(), sqlx::Error> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        sqlx::query("ANALYZE").execute(&self.pool).await?;
        Ok(())
    }

    /// Runs a quick integrity check.
    #[allow(dead_code)]
    pub async fn integrity_check(&self) -> Result<bool, sqlx::Error> {
        let result: (String,) = sqlx::query_as("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;
        Ok(result.0 == "ok")
    }
}
