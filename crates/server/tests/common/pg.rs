//! One embedded Postgres per test binary, started on first use and stopped at exit;
//! `database_url` hands each caller a fresh, empty database on it. No Docker needed.
//! Shared by the server and CLI test harnesses.

use std::sync::atomic::{AtomicUsize, Ordering};

use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};
use tokio::sync::Mutex;

/// Production's major version: keep in step with the `postgres` image in `compose.yaml`.
const MAJOR: &str = "=18";

static PG: Mutex<Option<PostgreSQL>> = Mutex::const_new(None);
static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" {
    safe fn atexit(f: extern "C" fn()) -> std::ffi::c_int;
}

/// Dropping the `PostgreSQL` stops the server and removes its data directory; a
/// static is never dropped, so this runs at process exit instead.
extern "C" fn stop_postgres() {
    if let Ok(mut pg) = PG.try_lock() {
        pg.take();
    }
}

/// The URL of a new, empty database, starting the shared server if needed.
pub async fn database_url() -> String {
    let mut pg = PG.lock().await;
    if pg.is_none() {
        let settings = SettingsBuilder::new()
            .version(VersionReq::parse(MAJOR).unwrap())
            .build();
        let mut server = PostgreSQL::new(settings);
        server.setup().await.expect("installing embedded Postgres");
        server.start().await.expect("starting embedded Postgres");
        *pg = Some(server);
        atexit(stop_postgres);
    }
    let pg = pg.as_ref().unwrap();
    let name = format!("test_{}", NEXT_DB.fetch_add(1, Ordering::Relaxed));
    pg.create_database(&name).await.unwrap();
    pg.settings().url(&name)
}
