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
use crablink_api::ErrorKind;
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
        .route("/links/{code}/qr", get(link_qr))
        .fallback(|| async { ApiError::NotFound });
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
        ApiError::Unauthorized.into_response()
    }
}

/// A management API error: JSON `{ "kind": "...", "error": "..." }`, where
/// `error` is the `Display` message.
#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("missing or invalid token")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    /// A Code with no live Link (missing or deleted).
    #[error("link not found")]
    LinkNotFound,
    /// A Link whose Expiry has passed, where only a live one will do.
    #[error("link has expired")]
    LinkExpired,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    InvalidTarget(&'static str),
    #[error("code must be 3-64 characters of A-Z, a-z, 0-9, _ or -")]
    InvalidCode,
    #[error("code {0:?} is reserved")]
    CodeReserved(String),
    #[error("code already taken")]
    CodeTaken,
    #[error("could not generate a free code, try again")]
    CodesExhausted,
    /// Details are logged, never sent.
    #[error("internal error")]
    Internal,
}

impl ApiError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Unauthorized => ErrorKind::Unauthorized,
            Self::NotFound => ErrorKind::NotFound,
            Self::LinkNotFound => ErrorKind::LinkNotFound,
            Self::LinkExpired => ErrorKind::LinkExpired,
            Self::BadRequest(_) => ErrorKind::BadRequest,
            Self::InvalidTarget(_) => ErrorKind::InvalidTarget,
            Self::InvalidCode => ErrorKind::InvalidCode,
            Self::CodeReserved(_) => ErrorKind::CodeReserved,
            Self::CodeTaken => ErrorKind::CodeTaken,
            Self::CodesExhausted | Self::Internal => ErrorKind::Internal,
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound | Self::LinkNotFound => StatusCode::NOT_FOUND,
            Self::LinkExpired => StatusCode::GONE,
            Self::BadRequest(_)
            | Self::InvalidTarget(_)
            | Self::InvalidCode
            | Self::CodeReserved(_) => StatusCode::BAD_REQUEST,
            Self::CodeTaken => StatusCode::CONFLICT,
            Self::CodesExhausted | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = crablink_api::ErrorBody {
            kind: self.kind(),
            error: self.to_string(),
        };
        (self.status(), Json(body)).into_response()
    }
}

/// A body that doesn't match the DTO (e.g. a malformed `expires_at`) is a 400, not axum's 422.
impl From<JsonRejection> for ApiError {
    fn from(e: JsonRejection) -> Self {
        ApiError::BadRequest(e.body_text())
    }
}

/// A malformed query string (e.g. `limit=x`) is a JSON 400 like any other API error.
impl From<QueryRejection> for ApiError {
    fn from(e: QueryRejection) -> Self {
        ApiError::BadRequest(e.body_text())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        eprintln!("database error: {e}");
        ApiError::Internal
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
    let bad = |msg| Err(ApiError::InvalidTarget(msg));
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
        return Err(ApiError::InvalidCode);
    }
    if RESERVED_CODES.iter().any(|r| r.eq_ignore_ascii_case(code)) {
        return Err(ApiError::CodeReserved(code.into()));
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
                return Err(ApiError::CodeTaken);
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError::CodesExhausted)
}

/// `GET /api/links/{code}`: one Link; 404 if missing or deleted.
async fn get_link(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<crablink_api::Link>, ApiError> {
    fetch_link(&s, &code).await.map(Json)
}

/// The live or expired Link with this Code; 404 if missing or deleted.
async fn fetch_link(s: &AppState, code: &str) -> Result<crablink_api::Link, ApiError> {
    sqlx::query_as::<_, LinkRow>(concat!(
        "SELECT ",
        link_columns!(),
        " FROM links WHERE code = ? AND deleted_at IS NULL"
    ))
    .bind(code)
    .fetch_optional(&s.pool)
    .await?
    .map(|row| row.into_dto(&s.base_url))
    .ok_or(ApiError::LinkNotFound)
}

/// `GET /api/links/{code}/qr?format=svg|png`: the QR code of the Link's Short URL.
async fn link_qr(
    State(s): State<AppState>,
    Path(code): Path<String>,
    query: Result<Query<crablink_api::QrQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(q) = query?;
    let link = fetch_link(&s, &code).await?;
    // Its Short URL no longer redirects, so a QR code of it would only mislead.
    if link.is_expired(Utc::now()) {
        return Err(ApiError::LinkExpired);
    }
    let format = q.format.unwrap_or(crablink_api::QrFormat::Svg);
    let internal = |what: &str, e: &dyn std::fmt::Display| {
        eprintln!("cannot {what} the QR code of {:?}: {e}", link.short_url);
        ApiError::Internal
    };
    let qr = qrcode::QrCode::new(&link.short_url).map_err(|e| internal("encode", &e))?;
    let body = match format {
        crablink_api::QrFormat::Svg => qr.render::<qrcode::render::svg::Color>().build().into(),
        crablink_api::QrFormat::Png => {
            let img = qr.render::<image::Luma<u8>>().build();
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| internal("write", &e))?;
            png.into_inner()
        }
    };
    Ok(([(header::CONTENT_TYPE, format.content_type())], body).into_response())
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
    .ok_or(ApiError::LinkNotFound)
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
        return Err(ApiError::BadRequest(format!(
            "limit must be between 1 and {MAX_PAGE}"
        )));
    }
    let before = match q.cursor {
        None => i64::MAX,
        Some(c) => c
            .parse()
            .map_err(|_| ApiError::BadRequest("invalid cursor".into()))?,
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
        return Err(ApiError::LinkNotFound);
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
        .unwrap_or_else(|e| (e.status(), e.to_string()).into_response())
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
