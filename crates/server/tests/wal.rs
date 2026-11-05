//! The WAL can't outgrow what `checkpoint` reclaims, even when steady traffic keeps
//! some connection reading and SQLite's automatic checkpoints never restart it.

use sqlx::{Connection, SqliteConnection};

#[tokio::test]
async fn checkpoint_truncates_a_wal_that_readers_kept_growing() {
    let dir = std::env::temp_dir().join(format!("crablink-wal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crablink.db");
    let url = format!("sqlite:{}", db.display());
    let wal = || std::fs::metadata(dir.join("crablink.db-wal")).map_or(0, |m| m.len());

    let pool = crablink_server::connect(&url).await.unwrap();
    sqlx::query(
        "INSERT INTO links (code, target, created_at, updated_at) \
         VALUES ('hot', 'https://example.com', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // A reader holding a snapshot, like one of many in-flight redirects under load.
    let mut reader = SqliteConnection::connect(&url).await.unwrap();
    sqlx::query("BEGIN").execute(&mut reader).await.unwrap();
    sqlx::query("SELECT count(*) FROM links")
        .execute(&mut reader)
        .await
        .unwrap();

    for _ in 0..2000 {
        sqlx::query("UPDATE links SET click_count = click_count + 1 WHERE code = 'hot'")
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("COMMIT").execute(&mut reader).await.unwrap();
    // Past SQLite's 1000-page automatic checkpoint: it couldn't restart the WAL.
    assert!(wal() > 4_000_000, "WAL is {} bytes", wal());

    crablink_server::checkpoint(&pool).await.unwrap();

    assert_eq!(wal(), 0);
    let clicks: i64 = sqlx::query_scalar("SELECT click_count FROM links WHERE code = 'hot'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(clicks, 2000);
    drop(reader);
    pool.close().await;
    std::fs::remove_dir_all(&dir).unwrap();
}
