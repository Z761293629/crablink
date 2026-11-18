//! The SQLite database: opening it and keeping its WAL in check.

use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

/// Opens (creating if missing) the database in WAL mode and runs migrations.
pub async fn connect(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    // An in-memory database lives inside one connection, so it must be the only one.
    let max = if url.contains(":memory:") { 1 } else { 8 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(opts)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

/// Copies the WAL into the database and truncates it to zero bytes. SQLite's own
/// automatic checkpoints can only restart the WAL when no connection is reading it,
/// which under steady redirect traffic is almost never, so without this the WAL file
/// grows by a page per click and never shrinks. Briefly holds back writers; readers
/// carry on.
pub async fn checkpoint(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;
    Ok(())
}
