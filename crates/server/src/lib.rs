//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use subtle::ConstantTimeEq;

/// Produces a candidate Generated Code. Injected so tests can script collisions.
pub type CodeGenerator = Arc<dyn Fn() -> String + Send + Sync>;

const CODE_LEN: usize = 7;
const MAX_CODE_ATTEMPTS: usize = 5;
const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// 7 random base62 characters (ADR-0003).
pub fn random_code() -> String {
    (0..CODE_LEN)
        .map(|_| BASE62[rand::random_range(0..BASE62.len())] as char)
        .collect()
}

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

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    codes: CodeGenerator,
    token: Arc<str>,
    base_url: Arc<str>,
}

/// Builds the full HTTP app.
pub fn app(pool: SqlitePool, codes: CodeGenerator, token: &str, base_url: &str) -> Router {
    let state = AppState {
        pool,
        codes,
        token: token.into(),
        base_url: base_url.trim_end_matches('/').into(),
    };
    let api = Router::new()
        .route("/links", post(create_link))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));
    Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/{code}", get(redirect))
        .with_state(state)
}

/// Every `/api` route needs `Authorization: Bearer <token>`, compared in constant time.
async fn require_token(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !s.token.is_empty() && bool::from(given.as_bytes().ct_eq(s.token.as_bytes())) {
        next.run(req).await
    } else {
        ApiError(StatusCode::UNAUTHORIZED, "missing or invalid token".into()).into_response()
    }
}

/// A management API error: JSON `{ "error": "..." }`.
struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(crablink_api::Error { error: self.1 })).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        eprintln!("database error: {e}");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
    }
}

#[derive(sqlx::FromRow)]
struct LinkRow {
    code: String,
    target: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    click_count: i64,
}

impl LinkRow {
    fn into_dto(self, base_url: &str) -> crablink_api::Link {
        crablink_api::Link {
            short_url: format!("{base_url}/{}", self.code),
            code: self.code,
            target: self.target,
            created_at: self.created_at,
            updated_at: self.updated_at,
            expires_at: self.expires_at,
            click_count: self.click_count,
        }
    }
}

/// Columns selected into a `LinkRow`; use with `concat!` (sqlx only accepts static SQL).
macro_rules! link_columns {
    () => {
        "code, target, created_at, updated_at, expires_at, click_count"
    };
}

async fn create_link(
    State(s): State<AppState>,
    Json(req): Json<crablink_api::CreateLink>,
) -> Result<(StatusCode, Json<crablink_api::Link>), ApiError> {
    let now = Utc::now();
    // Insert and let the unique constraint reject a taken Code; never check first (ADR-0003).
    for _ in 0..MAX_CODE_ATTEMPTS {
        let inserted = sqlx::query_as::<_, LinkRow>(concat!(
            "INSERT INTO links (code, target, created_at, updated_at) VALUES (?, ?, ?, ?) RETURNING ",
            link_columns!()
        ))
        .bind((s.codes)())
        .bind(&req.target)
        .bind(now)
        .bind(now)
        .fetch_one(&s.pool)
        .await;
        match inserted {
            Ok(row) => return Ok((StatusCode::CREATED, Json(row.into_dto(&s.base_url)))),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "could not generate a free code, try again".into(),
    ))
}

/// `GET /{code}`: 302 to the Target. Never 301/308, which browsers cache forever.
async fn redirect(State(s): State<AppState>, Path(code): Path<String>) -> Response {
    let target: Option<String> = match sqlx::query_scalar("SELECT target FROM links WHERE code = ?")
        .bind(&code)
        .fetch_optional(&s.pool)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("database error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    match target {
        Some(target) => (
            StatusCode::FOUND,
            [
                (header::LOCATION, target),
                (header::CACHE_CONTROL, "private, max-age=0".into()),
            ],
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
