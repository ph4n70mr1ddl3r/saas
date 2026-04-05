use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub async fn create_test_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create test pool");
    pool
}
