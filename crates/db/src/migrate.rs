use sqlx::SqlitePool;
use anyhow::Result;

/// Runs migrations from a directory of SQL files.
/// Each file is executed inside a transaction.
/// NOTE: For production use, consider using sqlx::migrate!() which tracks
/// applied migrations and ensures idempotency.
pub async fn run_migrations(pool: &SqlitePool, migrations_dir: &str) -> Result<()> {
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

        let sql = std::fs::read_to_string(&path)?;

        tracing::info!("Running migration: {}", path.display());

        // Execute each migration inside a transaction
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok(())
}
