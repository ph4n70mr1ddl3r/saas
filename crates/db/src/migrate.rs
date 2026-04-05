use sqlx::SqlitePool;
use anyhow::Result;

/// Runs migrations from a directory of SQL files.
/// Tracks applied migrations in a `_migrations` table to ensure idempotency.
/// Each file is executed inside a transaction.
pub async fn run_migrations(pool: &SqlitePool, migrations_dir: &str) -> Result<()> {
    // Ensure the tracking table exists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _migrations (filename TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
    )
    .execute(pool)
    .await?;

    let entries = std::fs::read_dir(migrations_dir)?;
    let mut sql_files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "sql"))
        .collect();
    sql_files.sort_by_key(|e| e.file_name());

    let canonical_dir = std::fs::canonicalize(migrations_dir)?;

    for entry in sql_files {
        let path = entry.path();

        // Validate the path stays within the migrations directory (prevent path traversal)
        let canonical = std::fs::canonicalize(&path)?;
        if !canonical.starts_with(&canonical_dir) {
            anyhow::bail!("Migration file '{}' is outside of migrations directory", path.display());
        }

        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Check if already applied
        let already_applied: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM _migrations WHERE filename = ?",
        )
        .bind(&filename)
        .fetch_one(pool)
        .await?
        > 0;

        if already_applied {
            tracing::debug!("Migration already applied, skipping: {}", filename);
            continue;
        }

        let sql = std::fs::read_to_string(&path)?;
        tracing::info!("Running migration: {}", filename);

        // Execute each migration inside a transaction
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;

        // Record the migration as applied
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO _migrations (filename, applied_at) VALUES (?, ?)")
            .bind(&filename)
            .bind(&now)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
    }
    Ok(())
}
