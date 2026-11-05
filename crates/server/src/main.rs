use std::process::exit;
use std::sync::Arc;

fn required(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("crablink-server: {name} must be set and non-empty");
            exit(2);
        }
    }
}

#[tokio::main]
async fn main() {
    let base_url = required("CRABLINK_BASE_URL");
    let token = required("CRABLINK_TOKEN");
    let db_url =
        std::env::var("CRABLINK_DATABASE_URL").unwrap_or_else(|_| "sqlite:crablink.db".into());
    let listen = std::env::var("CRABLINK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());

    let pool = crablink_server::connect(&db_url).await.unwrap_or_else(|e| {
        eprintln!("crablink-server: cannot open database {db_url}: {e}");
        exit(1);
    });
    // ponytail: fixed 10 s; at the ~7k clicks/s a laptop sustains that caps the WAL near 300 MB.
    let checkpoints = pool.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            every.tick().await;
            if let Err(e) = crablink_server::checkpoint(&checkpoints).await {
                eprintln!("crablink-server: WAL checkpoint failed: {e}");
            }
        }
    });
    let app = crablink_server::app(
        pool,
        Arc::new(crablink_server::random_code),
        &token,
        &base_url,
    );
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|e| {
            eprintln!("crablink-server: cannot listen on {listen}: {e}");
            exit(1);
        });
    eprintln!("crablink-server listening on {listen}");
    axum::serve(listener, app).await.unwrap();
}
