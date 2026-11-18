//! The Postgres database.

use sqlx::PgPool;

/// Connects to the database at `url` and runs migrations.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}
