//! The Postgres database.

use sqlx::PgPool;

/// Connects to the database at `url` and runs migrations.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

/// Makes `host` the Default Domain, creating it if missing. A changed
/// `CRABLINK_BASE_URL` makes its host the Default Domain instead, taking it from any
/// Workspace that had it as a custom Domain.
pub(crate) async fn default_domain(pool: &PgPool, host: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE domains SET is_default = FALSE WHERE is_default AND host <> $1")
        .bind(host)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO domains (host, is_default, status) VALUES ($1, TRUE, 'approved') \
         ON CONFLICT (host) DO UPDATE \
         SET is_default = TRUE, status = 'approved', workspace_id = NULL",
    )
    .bind(host)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
