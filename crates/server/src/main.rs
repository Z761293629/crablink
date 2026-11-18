use std::io::IsTerminal;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => serve().await,
        ["admin", "create", "--email", email] => admin_create(email).await,
        _ => {
            eprintln!("usage: crablink-server [admin create --email <email>]");
            exit(2);
        }
    }
}

/// `admin create`: creates an Instance Admin, reading the password from a prompt,
/// or from stdin's first line when it isn't a terminal.
async fn admin_create(email: &str) {
    let db_url = required("CRABLINK_DATABASE_URL");
    let password = if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Password: ")
    } else {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).map(|_| line)
    };
    let password = password.unwrap_or_else(|e| {
        eprintln!("crablink-server: cannot read the password: {e}");
        exit(1);
    });
    let pool = open_database(&db_url).await;
    let password = password.trim_end_matches(['\r', '\n']);
    if let Err(e) = crablink_server::create_user(&pool, email, password, true).await {
        eprintln!("crablink-server: {e}");
        exit(1);
    }
    eprintln!("crablink-server: created Instance Admin {email}");
}

async fn open_database(db_url: &str) -> sqlx::PgPool {
    crablink_server::connect(db_url).await.unwrap_or_else(|e| {
        // Not the URL itself: it usually carries the database password.
        eprintln!("crablink-server: cannot open the database: {e}");
        exit(1);
    })
}

async fn serve() {
    let base_url = required("CRABLINK_BASE_URL");
    let db_url = required("CRABLINK_DATABASE_URL");
    let listen = std::env::var("CRABLINK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());

    if std::env::var_os("CRABLINK_TOKEN").is_some() {
        eprintln!(
            "crablink-server: CRABLINK_TOKEN is no longer used: programs sign in with an API Key"
        );
    }

    let pool = open_database(&db_url).await;
    let app = crablink_server::app(
        pool.clone(),
        Arc::new(crablink_server::random_code),
        Arc::new(chrono::Utc::now),
        &base_url,
    )
    .await
    .unwrap_or_else(|e| {
        eprintln!("crablink-server: cannot set up the Default Domain from CRABLINK_BASE_URL: {e}");
        exit(1);
    });
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|e| {
            eprintln!("crablink-server: cannot listen on {listen}: {e}");
            exit(1);
        });
    eprintln!("crablink-server listening on {listen}");
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("crablink-server: server error: {e}");
        exit(1);
    }
    pool.close().await;
    eprintln!("crablink-server stopped");
}

/// Resolves on Ctrl-C, or on SIGTERM (what Docker and systemd send) on Unix.
/// In-flight requests then finish before `serve` returns.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("installing a SIGTERM handler");
        tokio::select! {
            // An `Err` here would otherwise read as "Ctrl-C pressed" and stop the server at once.
            r = tokio::signal::ctrl_c() => r.expect("installing a Ctrl-C handler"),
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("installing a Ctrl-C handler");
}
