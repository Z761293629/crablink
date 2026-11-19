//! `/api/workspaces/{slug}/links`: Link management handlers, Target and Custom Code
//! validation, and QR codes. A Link is addressed by its Code and `?domain=`, the
//! Default Domain when absent.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::AppState;
use crate::error::ApiError;
use crate::workspaces::Caller;

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

#[derive(sqlx::FromRow)]
struct LinkRow {
    code: String,
    target: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    click_count: i64,
    domain: String,
    tags: Vec<String>,
}

impl LinkRow {
    /// The DTO, its Short URL built from the Link's Domain.
    fn into_dto(self, scheme: &str) -> crablink_api::Link {
        crablink_api::Link {
            short_url: format!("{scheme}://{}/{}", self.domain, self.code),
            domain: self.domain,
            code: self.code,
            target: self.target,
            created_at: self.created_at,
            updated_at: self.updated_at,
            expires_at: self.expires_at,
            click_count: self.click_count,
            tags: self.tags,
        }
    }
}

/// Columns selected into a `LinkRow` from `links l` joined with its `domains d`; use
/// with `concat!` (sqlx only accepts static SQL).
macro_rules! link_columns {
    () => {
        "l.code, l.target, l.created_at, l.updated_at, l.expires_at, l.click_count, \
         d.host AS domain, ARRAY(SELECT t.name FROM link_tags lt JOIN tags t ON t.id = lt.tag_id \
         WHERE lt.link_id = l.id ORDER BY t.name) AS tags"
    };
}

/// The Link with this id, deleted or not.
async fn link_by_id(conn: &mut PgConnection, id: i64) -> sqlx::Result<LinkRow> {
    sqlx::query_as(concat!(
        "SELECT ",
        link_columns!(),
        " FROM links l JOIN domains d ON d.id = l.domain_id WHERE l.id = $1"
    ))
    .bind(id)
    .fetch_one(conn)
    .await
}

/// Trimmed Tag names, each 1-64 characters with no comma (CSV joins a Link's Tags
/// with commas) or control character.
fn tag_names(tags: &[String]) -> Result<Vec<String>, ApiError> {
    tags.iter()
        .map(|t| {
            let t = t.trim();
            let ok = (1..=64).contains(&t.chars().count())
                && !t.chars().any(|c| c == ',' || c.is_control());
            ok.then(|| t.to_owned()).ok_or(ApiError::InvalidTag)
        })
        .collect()
}

/// Replaces the Link's Tags with `names`, creating those new to the Workspace.
async fn set_tags(
    conn: &mut PgConnection,
    workspace_id: i64,
    link_id: i64,
    names: &[String],
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM link_tags WHERE link_id = $1")
        .bind(link_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO tags (workspace_id, name) SELECT $1, unnest($2::text[]) \
         ON CONFLICT (workspace_id, name) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(names)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO link_tags (link_id, tag_id) \
         SELECT $1, id FROM tags WHERE workspace_id = $2 AND name = ANY($3)",
    )
    .bind(link_id)
    .bind(workspace_id)
    .bind(names)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// A Target must be an absolute `http(s)` URL not on `default_host`, the Default
/// Domain's host (that would loop). It is never fetched. Shared by create and update.
fn validate_target(target: &str, default_host: &str) -> Result<(), ApiError> {
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
    if url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case(default_host))
    {
        return bad("target must not point at this service's own host");
    }
    Ok(())
}

/// Custom Codes that would shadow the service's own routes (compared case-insensitively).
const RESERVED_CODES: &[&str] = &["api", "admin", "app", "health"];

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

/// `POST /api/workspaces/{slug}/links`: a Link on the Default Domain or an approved
/// Domain of the Workspace.
pub(crate) async fn create_link(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    req: Result<Json<crablink_api::CreateLink>, JsonRejection>,
) -> Result<(StatusCode, Json<crablink_api::Link>), ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Json(req) = req?;
    validate_target(&req.target, &s.default_host)?;
    if let Some(code) = &req.code {
        validate_custom_code(code)?;
    }
    let tags = tag_names(&req.tags)?;
    let host = req.domain.as_deref().unwrap_or(&s.default_domain);
    let domain_id = crate::domains::usable(&s.pool, workspace_id, host)
        .await?
        .ok_or(ApiError::DomainNotApproved)?;
    let now = (s.clock)();
    let mut tx = s.pool.begin().await?;
    // Insert and let the unique constraint reject a taken Code; never check first (ADR-0003).
    let mut id = None;
    for _ in 0..MAX_CODE_ATTEMPTS {
        id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO links \
             (code, target, created_at, updated_at, expires_at, domain_id, workspace_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (domain_id, code) DO NOTHING RETURNING id",
        )
        .bind(req.code.clone().unwrap_or_else(|| (s.codes)()))
        .bind(&req.target)
        .bind(now)
        .bind(now)
        .bind(req.expires_at)
        .bind(domain_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        // A taken Custom Code is the caller's to fix: never overwrite, never retry.
        if id.is_some() || req.code.is_some() {
            break;
        }
    }
    let Some(id) = id else {
        return Err(match req.code {
            Some(_) => ApiError::CodeTaken,
            None => ApiError::CodesExhausted,
        });
    };
    if !tags.is_empty() {
        set_tags(&mut tx, workspace_id, id, &tags).await?;
    }
    let link = link_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(link.into_dto(&s.scheme))))
}

/// The host of the Domain `?domain=` names, the Default Domain when absent.
fn host<'a>(s: &'a AppState, domain: &'a Option<String>) -> &'a str {
    domain.as_deref().unwrap_or(&s.default_domain)
}

/// `GET /api/workspaces/{slug}/links/{code}?domain=`: one Link; 404 if missing or deleted.
pub(crate) async fn get_link(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, code)): Path<(String, String)>,
    query: Result<Query<crablink_api::LinkDomain>, QueryRejection>,
) -> Result<Json<crablink_api::Link>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Query(q) = query?;
    fetch_link(&s, workspace_id, host(&s, &q.domain), &code)
        .await
        .map(Json)
}

/// The Workspace's live or expired Link with this Code on Domain `host`; 404 if
/// missing or deleted.
async fn fetch_link(
    s: &AppState,
    workspace_id: i64,
    host: &str,
    code: &str,
) -> Result<crablink_api::Link, ApiError> {
    sqlx::query_as::<_, LinkRow>(concat!(
        "SELECT ",
        link_columns!(),
        " FROM links l JOIN domains d ON d.id = l.domain_id \
         WHERE l.workspace_id = $1 AND d.host = lower($2) AND l.code = $3 \
         AND l.deleted_at IS NULL"
    ))
    .bind(workspace_id)
    .bind(host)
    .bind(code)
    .fetch_optional(&s.pool)
    .await?
    .map(|row| row.into_dto(&s.scheme))
    .ok_or(ApiError::LinkNotFound)
}

/// `GET /api/workspaces/{slug}/links/{code}/qr?format=svg|png`: the QR code of the
/// Link's Short URL.
pub(crate) async fn link_qr(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, code)): Path<(String, String)>,
    query: Result<Query<crablink_api::QrQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Query(q) = query?;
    let link = fetch_link(&s, workspace_id, host(&s, &q.domain), &code).await?;
    // Its Short URL no longer redirects, so a QR code of it would only mislead.
    if link.is_expired((s.clock)()) {
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

/// `PATCH /api/workspaces/{slug}/links/{code}?domain=`: changes Target and/or Expiry,
/// never the Code or Domain; 404 if missing or deleted.
pub(crate) async fn update_link(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, code)): Path<(String, String)>,
    query: Result<Query<crablink_api::LinkDomain>, QueryRejection>,
    req: Result<Json<crablink_api::UpdateLink>, JsonRejection>,
) -> Result<Json<crablink_api::Link>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Query(q) = query?;
    let Json(req) = req?;
    if let Some(target) = &req.target {
        validate_target(target, &s.default_host)?;
    }
    let tags = req.tags.as_deref().map(tag_names).transpose()?;
    let mut tx = s.pool.begin().await?;
    let id = sqlx::query_scalar::<_, i64>(
        "UPDATE links l SET target = COALESCE($1, l.target), \
         expires_at = CASE WHEN $2 THEN $3 ELSE l.expires_at END, updated_at = $4 \
         FROM domains d WHERE d.id = l.domain_id \
         AND l.workspace_id = $5 AND d.host = lower($6) AND l.code = $7 \
         AND l.deleted_at IS NULL \
         RETURNING l.id",
    )
    .bind(&req.target)
    .bind(req.expires_at.is_some())
    .bind(req.expires_at.flatten())
    .bind((s.clock)())
    .bind(workspace_id)
    .bind(host(&s, &q.domain))
    .bind(&code)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::LinkNotFound)?;
    if let Some(tags) = &tags {
        set_tags(&mut tx, workspace_id, id, tags).await?;
    }
    let link = link_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(link.into_dto(&s.scheme)))
}

const DEFAULT_PAGE: u32 = 50;
const MAX_PAGE: u32 = 200;

#[derive(sqlx::FromRow)]
struct ListRow {
    id: i64,
    #[sqlx(flatten)]
    link: LinkRow,
}

/// `GET /api/workspaces/{slug}/links`: the Workspace's live and expired Links on the
/// Default Domain and its own custom Domains (not a former Default Domain), newest
/// (highest id) first, only those with Tag `tag` and whose Code or Target contains
/// `q` (ignoring case) when given. The cursor is the id of the previous
/// page's last Link, so each page is an index range scan.
pub(crate) async fn list_links(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    query: Result<Query<crablink_api::ListLinks>, QueryRejection>,
) -> Result<Json<crablink_api::LinkPage>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
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
        "SELECT l.id, ",
        link_columns!(),
        " FROM links l JOIN domains d ON d.id = l.domain_id \
         WHERE l.workspace_id = $3 AND l.deleted_at IS NULL AND l.id < $1 \
         AND (d.is_default OR d.workspace_id = $3) \
         AND ($4::text IS NULL OR EXISTS (SELECT 1 FROM link_tags lt \
              JOIN tags t ON t.id = lt.tag_id WHERE lt.link_id = l.id AND t.name = $4)) \
         AND ($5::text IS NULL OR strpos(lower(l.code), lower($5)) > 0 \
              OR strpos(lower(l.target), lower($5)) > 0) \
         ORDER BY l.id DESC LIMIT $2"
    ))
    .bind(before)
    .bind(i64::from(limit) + 1)
    .bind(workspace_id)
    .bind(&q.tag)
    .bind(&q.q)
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
            .map(|r| r.link.into_dto(&s.scheme))
            .collect(),
        next_cursor,
    }))
}

/// `GET /api/workspaces/{slug}/tags`: the names of the Workspace's Tags on at least
/// one live Link, sorted.
pub(crate) async fn list_tags(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
) -> Result<Json<Vec<String>>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let tags = sqlx::query_scalar(
        "SELECT t.name FROM tags t WHERE t.workspace_id = $1 AND EXISTS \
         (SELECT 1 FROM link_tags lt JOIN links l ON l.id = lt.link_id \
          WHERE lt.tag_id = t.id AND l.deleted_at IS NULL) \
         ORDER BY t.name",
    )
    .bind(workspace_id)
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(tags))
}

/// `DELETE /api/workspaces/{slug}/links/{code}?domain=`: marks the Link deleted
/// (ADR-0002). The row stays so its Code is never reused on its Domain; 404 if missing
/// or already deleted.
pub(crate) async fn delete_link(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, code)): Path<(String, String)>,
    query: Result<Query<crablink_api::LinkDomain>, QueryRejection>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Query(q) = query?;
    let done = sqlx::query(
        "UPDATE links SET deleted_at = $1 FROM domains d WHERE d.id = links.domain_id \
         AND links.workspace_id = $2 AND d.host = lower($3) AND links.code = $4 \
         AND links.deleted_at IS NULL",
    )
    .bind((s.clock)())
    .bind(workspace_id)
    .bind(host(&s, &q.domain))
    .bind(&code)
    .execute(&s.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::LinkNotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
