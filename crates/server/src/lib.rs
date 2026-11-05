//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::str::FromStr;
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
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
const MAX_TARGET_LEN: usize = 2048;
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
        .route("/links/{code}", get(get_link))
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

/// A body that doesn't match the DTO (e.g. a malformed `expires_at`) is a 400, not axum's 422.
impl From<JsonRejection> for ApiError {
    fn from(e: JsonRejection) -> Self {
        ApiError(StatusCode::BAD_REQUEST, e.body_text())
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

/// A Target must be an absolute `http(s)` URL not on our own host (that would
/// loop). It is never fetched. Shared by create and update.
fn validate_target(target: &str, base_url: &str) -> Result<(), ApiError> {
    let bad = |msg: &str| Err(ApiError(StatusCode::BAD_REQUEST, msg.into()));
    if target.chars().count() > MAX_TARGET_LEN {
        return bad("target must be at most 2048 characters");
    }
    let Ok(url) = url::Url::parse(target) else {
        return bad("target must be an absolute http(s) URL");
    };
    if !matches!(url.scheme(), "http" | "https") {
        return bad("target must be an absolute http(s) URL");
    }
    let base_host = url::Url::parse(base_url).ok();
    let base_host = base_host.as_ref().and_then(|u| u.host_str());
    if let (Some(host), Some(base)) = (url.host_str(), base_host)
        && host.eq_ignore_ascii_case(base)
    {
        return bad("target must not point at this service's own host");
    }
    Ok(())
}

/// Custom Codes that would shadow the service's own routes (compared case-insensitively).
const RESERVED_CODES: &[&str] = &["api", "admin", "health"];

/// A Custom Code matches `[A-Za-z0-9_-]{3,64}` and is not reserved.
fn validate_custom_code(code: &str) -> Result<(), ApiError> {
    let well_formed = (3..=64).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !well_formed {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "code must be 3-64 characters of A-Z, a-z, 0-9, _ or -".into(),
        ));
    }
    if RESERVED_CODES.iter().any(|r| r.eq_ignore_ascii_case(code)) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("code {code:?} is reserved"),
        ));
    }
    Ok(())
}

async fn create_link(
    State(s): State<AppState>,
    req: Result<Json<crablink_api::CreateLink>, JsonRejection>,
) -> Result<(StatusCode, Json<crablink_api::Link>), ApiError> {
    let Json(req) = req?;
    validate_target(&req.target, &s.base_url)?;
    if let Some(code) = &req.code {
        validate_custom_code(code)?;
    }
    let now = Utc::now();
    // Insert and let the unique constraint reject a taken Code; never check first (ADR-0003).
    for _ in 0..MAX_CODE_ATTEMPTS {
        let inserted = sqlx::query_as::<_, LinkRow>(concat!(
            "INSERT INTO links (code, target, created_at, updated_at, expires_at) VALUES (?, ?, ?, ?, ?) RETURNING ",
            link_columns!()
        ))
        .bind(req.code.clone().unwrap_or_else(|| (s.codes)()))
        .bind(&req.target)
        .bind(now)
        .bind(now)
        .bind(req.expires_at)
        .fetch_one(&s.pool)
        .await;
        match inserted {
            Ok(row) => return Ok((StatusCode::CREATED, Json(row.into_dto(&s.base_url)))),
            // A taken Custom Code is the caller's to fix: never overwrite, never retry.
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() && req.code.is_some() => {
                return Err(ApiError(StatusCode::CONFLICT, "code already taken".into()));
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "could not generate a free code, try again".into(),
    ))
}

/// `GET /api/links/{code}`: one Link; 404 if missing or deleted.
async fn get_link(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<crablink_api::Link>, ApiError> {
    sqlx::query_as::<_, LinkRow>(concat!(
        "SELECT ",
        link_columns!(),
        " FROM links WHERE code = ? AND deleted_at IS NULL"
    ))
    .bind(&code)
    .fetch_optional(&s.pool)
    .await?
    .map(|row| Json(row.into_dto(&s.base_url)))
    .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "link not found".into()))
}

/// `GET /{code}`: 302 to the Target. Never 301/308, which browsers cache forever.
/// Checks liveness and counts the click in one statement; only when that misses
/// does a lookup tell a Link that never existed (404) from a dead one (410).
async fn redirect(State(s): State<AppState>, Path(code): Path<String>) -> Response {
    match follow(&s.pool, &code).await {
        Ok(Some(target)) => (
            StatusCode::FOUND,
            [
                (header::LOCATION, target),
                (header::CACHE_CONTROL, "private, max-age=0".into()),
            ],
        )
            .into_response(),
        Ok(None) => match sqlx::query("SELECT 1 FROM links WHERE code = ?")
            .bind(&code)
            .fetch_optional(&s.pool)
            .await
        {
            Ok(Some(_)) => (StatusCode::GONE, "gone").into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, "not found").into_response(),
            Err(e) => db_error(e),
        },
        Err(e) => db_error(e),
    }
}

/// The Target of a live (not deleted, not expired) Link, counting the click.
async fn follow(pool: &SqlitePool, code: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE links SET click_count = click_count + 1 \
         WHERE code = ? AND deleted_at IS NULL AND (expires_at IS NULL OR expires_at > ?) \
         RETURNING target",
    )
    .bind(code)
    .bind(Utc::now())
    .fetch_optional(pool)
    .await
}

fn db_error(e: sqlx::Error) -> Response {
    eprintln!("database error: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}
