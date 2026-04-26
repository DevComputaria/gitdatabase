use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub mod metadata;

/// Create a connection pool to PostgreSQL and run migrations.
pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;

    tracing::info!("connected to PostgreSQL");

    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("migrations applied");

    Ok(pool)
}

/// Simple health check – runs `SELECT 1`.
pub async fn health_check(pool: &PgPool) -> Result<()> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await?;
    Ok(())
}
