//! The server the web UI's Playwright tests run against (`web/playwright.config.ts`):
//! starts an embedded Postgres, creates the Instance Admin named by
//! `CRABLINK_E2E_ADMIN_EMAIL` and `CRABLINK_E2E_ADMIN_PASSWORD`, then runs the real
//! `crablink-server` binary (built beforehand) on it with this process's environment.
//! With `CRABLINK_E2E_MAIL_DIR` set, the server sends email to an SMTP sink here that
//! writes each one, raw, into that directory, for tests to follow the links in them.
//! On SIGTERM or Ctrl-C it stops the server, then Postgres and its data.

use std::path::PathBuf;

use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::{SignalKind, signal};

/// Accepts every email over SMTP and writes it to `dir` as `<connection>-<n>.eml`.
async fn smtp_sink(listener: TcpListener, dir: PathBuf) {
    let mut connection = 0;
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        connection += 1;
        let dir = dir.clone();
        tokio::spawn(async move {
            if let Err(e) = smtp_session(stream, &dir, connection).await {
                eprintln!("e2e: SMTP sink: {e}");
            }
        });
    }
}

/// Just enough SMTP for lettre: every command but `DATA` and `QUIT` gets a plain OK.
async fn smtp_session(
    stream: TcpStream,
    dir: &std::path::Path,
    connection: u64,
) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    write.write_all(b"220 e2e SMTP sink\r\n").await?;
    let mut emails = 0;
    while let Some(line) = lines.next_line().await? {
        let reply = match line.get(..4).map(str::to_ascii_uppercase).as_deref() {
            Some("DATA") => {
                write.write_all(b"354 end with .\r\n").await?;
                let mut email = String::new();
                while let Some(line) = lines.next_line().await? {
                    if line == "." {
                        break;
                    }
                    // Undo dot-stuffing.
                    email.push_str(line.strip_prefix('.').unwrap_or(&line));
                    email.push('\n');
                }
                emails += 1;
                std::fs::write(dir.join(format!("{connection}-{emails}.eml")), email)?;
                "250 kept"
            }
            Some("QUIT") => {
                write.write_all(b"221 bye\r\n").await?;
                return Ok(());
            }
            _ => "250 ok",
        };
        write.write_all(format!("{reply}\r\n").as_bytes()).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let email = std::env::var("CRABLINK_E2E_ADMIN_EMAIL").expect("CRABLINK_E2E_ADMIN_EMAIL");
    let password =
        std::env::var("CRABLINK_E2E_ADMIN_PASSWORD").expect("CRABLINK_E2E_ADMIN_PASSWORD");

    // Production's major version, as in tests/common/pg.rs.
    let settings = SettingsBuilder::new()
        .version(VersionReq::parse("=18").unwrap())
        .build();
    let mut pg = PostgreSQL::new(settings);
    pg.setup().await.expect("installing embedded Postgres");
    pg.start().await.expect("starting embedded Postgres");
    pg.create_database("crablink").await.unwrap();
    let db_url = pg.settings().url("crablink");

    let pool = crablink_server::connect(&db_url).await.unwrap();
    crablink_server::create_user(&pool, &email, &password, true)
        .await
        .unwrap();
    pool.close().await;

    // target/<profile>/examples/e2e -> target/<profile>/crablink-server
    let exe = std::env::current_exe().unwrap();
    let server = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crablink-server");
    let mut command = tokio::process::Command::new(&server);
    if let Ok(dir) = std::env::var("CRABLINK_E2E_MAIL_DIR") {
        let dir = PathBuf::from(dir);
        // Emails of an earlier run would be found by tests looking for the latest.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(smtp_sink(listener, dir));
        command
            .env("CRABLINK_SMTP_URL", format!("smtp://127.0.0.1:{port}"))
            .env("CRABLINK_SMTP_FROM", "crablink <links@example.com>");
    }
    let mut child = command
        .env("CRABLINK_DATABASE_URL", &db_url)
        .kill_on_drop(true)
        .spawn()
        .unwrap_or_else(|e| panic!("running {}: {e}", server.display()));

    let mut term = signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        status = child.wait() => eprintln!("e2e: crablink-server exited: {status:?}"),
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    let _ = child.kill().await;
    pg.stop().await.expect("stopping embedded Postgres");
}
