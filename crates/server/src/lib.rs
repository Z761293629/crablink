//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::str::FromStr;
use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, Request, State};
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
        .route("/links", post(create_link).get(list_links))
        .route(
            "/links/{code}",
            get(get_link).patch(update_link).delete(delete_link),
        )
        .fallback(|| async { ApiError(StatusCode::NOT_FOUND, "not found".into()) });
    Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/{code}", get(redirect))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        // On the outer router, so unknown `/api` paths and methods need the token too.
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

/// Every `/api` request needs `Authorization: Bearer <token>`, compared in constant time.
async fn require_token(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path != "/api" && !path.starts_with("/api/") {
        return next.run(req).await;
    }
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
        (self.0, Json(crablink_api::ErrorBody { error: self.1 })).into_response()
    }
}

/// The 404 for a Code with no live Link (missing or deleted).
fn link_not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "link not found".into())
}

/// A body that doesn't match the DTO (e.g. a malformed `expires_at`) is a 400, not axum's 422.
impl From<JsonRejection> for ApiError {
    fn from(e: JsonRejection) -> Self {
        ApiError(StatusCode::BAD_REQUEST, e.body_text())
    }
}

/// A malformed query string (e.g. `limit=x`) is a JSON 400 like any other API error.
impl From<QueryRejection> for ApiError {
    fn from(e: QueryRejection) -> Self {
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
    // The URL parser silently strips these; stored raw they'd break the `Location` header.
    if target.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return bad("target must not contain whitespace or control characters");
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
    .ok_or_else(link_not_found)
}

/// `PATCH /api/links/{code}`: changes Target and/or Expiry, never the Code;
/// 404 if missing or deleted.
async fn update_link(
    State(s): State<AppState>,
    Path(code): Path<String>,
    req: Result<Json<crablink_api::UpdateLink>, JsonRejection>,
) -> Result<Json<crablink_api::Link>, ApiError> {
    let Json(req) = req?;
    if let Some(target) = &req.target {
        validate_target(target, &s.base_url)?;
    }
    sqlx::query_as::<_, LinkRow>(concat!(
        "UPDATE links SET target = COALESCE(?, target), \
         expires_at = CASE WHEN ? THEN ? ELSE expires_at END, updated_at = ? \
         WHERE code = ? AND deleted_at IS NULL RETURNING ",
        link_columns!()
    ))
    .bind(&req.target)
    .bind(req.expires_at.is_some())
    .bind(req.expires_at.flatten())
    .bind(Utc::now())
    .bind(&code)
    .fetch_optional(&s.pool)
    .await?
    .map(|row| Json(row.into_dto(&s.base_url)))
    .ok_or_else(link_not_found)
}

const DEFAULT_PAGE: u32 = 50;
const MAX_PAGE: u32 = 200;

#[derive(sqlx::FromRow)]
struct ListRow {
    id: i64,
    #[sqlx(flatten)]
    link: LinkRow,
}

/// `GET /api/links`: live and expired Links, newest (highest id) first. The cursor
/// is the id of the previous page's last Link, so each page is an index range scan.
async fn list_links(
    State(s): State<AppState>,
    query: Result<Query<crablink_api::ListLinks>, QueryRejection>,
) -> Result<Json<crablink_api::LinkPage>, ApiError> {
    let Query(q) = query?;
    let limit = q.limit.unwrap_or(DEFAULT_PAGE);
    if !(1..=MAX_PAGE).contains(&limit) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("limit must be between 1 and {MAX_PAGE}"),
        ));
    }
    let before = match q.cursor {
        None => i64::MAX,
        Some(c) => c
            .parse()
            .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "invalid cursor".into()))?,
    };
    // Fetch one extra row to learn whether another page follows.
    let mut rows = sqlx::query_as::<_, ListRow>(concat!(
        "SELECT id, ",
        link_columns!(),
        " FROM links WHERE deleted_at IS NULL AND id < ? ORDER BY id DESC LIMIT ?"
    ))
    .bind(before)
    .bind(limit + 1)
    .fetch_all(&s.pool)
    .await?;
    let next_cursor = if rows.len() > limit as usize {
        rows.truncate(limit as usize);
        rows.last().map(|r| r.id.to_string())
    } else {
        None
    };
    Ok(Json(crablink_api::LinkPage {
        links: rows
            .into_iter()
            .map(|r| r.link.into_dto(&s.base_url))
            .collect(),
        next_cursor,
    }))
}

/// `DELETE /api/links/{code}`: marks the Link deleted (ADR-0002). The row stays so
/// its Code is never reused; 404 if missing or already deleted.
async fn delete_link(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<StatusCode, ApiError> {
    let done = sqlx::query("UPDATE links SET deleted_at = ? WHERE code = ? AND deleted_at IS NULL")
        .bind(Utc::now())
        .bind(&code)
        .execute(&s.pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(link_not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /{code}`: 302 to the Target. Never 301/308, which browsers cache forever.
/// Checks liveness and counts the click in one statement; only when that misses
/// does a lookup tell a Link that never existed (404) from a dead one (410).
async fn redirect(State(s): State<AppState>, Path(code): Path<String>) -> Response {
    // Unlike the management API, redirect error bodies are plain text.
    follow(&s.pool, &code)
        .await
        .unwrap_or_else(|ApiError(status, msg)| (status, msg).into_response())
}

/// Redirects to the Target of a live (not deleted, not expired) Link, counting the click.
async fn follow(pool: &SqlitePool, code: &str) -> Result<Response, ApiError> {
    let target: Option<String> = sqlx::query_scalar(
        "UPDATE links SET click_count = click_count + 1 \
         WHERE code = ? AND deleted_at IS NULL AND (expires_at IS NULL OR expires_at > ?) \
         RETURNING target",
    )
    .bind(code)
    .bind(Utc::now())
    .fetch_optional(pool)
    .await?;
    if let Some(target) = target {
        let headers = [
            (header::LOCATION, target),
            (header::CACHE_CONTROL, "private, max-age=0".into()),
        ];
        return Ok((StatusCode::FOUND, headers).into_response());
    }
    let exists = sqlx::query("SELECT 1 FROM links WHERE code = ?")
        .bind(code)
        .fetch_optional(pool)
        .await?
        .is_some();
    Ok(if exists {
        (StatusCode::GONE, "gone").into_response()
    } else {
        (StatusCode::NOT_FOUND, "not found").into_response()
    })
}
