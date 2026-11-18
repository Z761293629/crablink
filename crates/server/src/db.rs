//! The Postgres database.

use sqlx::PgPool;

/// Connects to the database at `url` and runs migrations.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

/// The id of the Default Domain `host`, creating it if missing. A changed
/// `CRABLINK_BASE_URL` makes its host the Default Domain instead.
pub(crate) async fn default_domain(pool: &PgPool, host: &str) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE domains SET is_default = FALSE WHERE is_default AND host <> $1")
        .bind(host)
        .execute(&mut *tx)
        .await?;
    let id = sqlx::query_scalar(
        "INSERT INTO domains (host, is_default) VALUES ($1, TRUE) \
         ON CONFLICT (host) DO UPDATE SET is_default = TRUE RETURNING id",
    )
    .bind(host)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}
