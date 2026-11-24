//! The server the web UI's Playwright tests run against (`web/playwright.config.ts`):
//! starts an embedded Postgres, creates the Instance Admin named by
//! `CRABLINK_E2E_ADMIN_EMAIL` and `CRABLINK_E2E_ADMIN_PASSWORD`, then runs the real
//! `crablink-server` binary (built beforehand) on it with this process's environment,
//! plus the fake Identity Provider of the tests set up as Identity Provider `fake`,
//! which creates Users on `idp.example.com`.
//! On SIGTERM or Ctrl-C it stops the server, then Postgres and its data.

use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};
use tokio::signal::unix::{SignalKind, signal};

#[path = "../tests/common/fake_identity_provider.rs"]
mod fake_identity_provider;

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
    let fake = fake_identity_provider::FakeIdentityProvider::start().await;
    let mut child = tokio::process::Command::new(&server)
        .env("CRABLINK_DATABASE_URL", &db_url)
        .env("CRABLINK_IDENTITY_PROVIDER_FAKE_NAME", "Fake ID")
        .env("CRABLINK_IDENTITY_PROVIDER_FAKE_ISSUER", &fake.issuer)
        .env(
            "CRABLINK_IDENTITY_PROVIDER_FAKE_CLIENT_ID",
            fake_identity_provider::CLIENT_ID,
        )
        .env(
            "CRABLINK_IDENTITY_PROVIDER_FAKE_CLIENT_SECRET",
            fake_identity_provider::CLIENT_SECRET,
        )
        .env(
            "CRABLINK_IDENTITY_PROVIDER_FAKE_ALLOWED_EMAIL_DOMAINS",
            "idp.example.com",
        )
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
