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
    blocked: bool,
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
            blocked: self.blocked,
        }
    }
}

/// Columns selected into a `LinkRow` from `links l` joined with its `domains d`; use
/// with `concat!` (sqlx only accepts static SQL).
macro_rules! link_columns {
    () => {
        "l.code, l.target, l.created_at, l.updated_at, l.expires_at, l.click_count, \
         d.host AS domain, ARRAY(SELECT t.name FROM link_tags lt JOIN tags t ON t.id = lt.tag_id \
         WHERE lt.link_id = l.id ORDER BY t.name) AS tags, l.blocked_at IS NOT NULL AS blocked"
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

/// Refuses a Target the injected check calls malicious; a failed check lets it through,
/// so an outage at Safe Browsing never stops Link creation. Call after the cheap checks.
async fn check_target(s: &AppState, target: &str) -> Result<(), ApiError> {
    match (s.target_check)(target.to_owned()).await {
        Ok(true) => Err(ApiError::TargetUnsafe),
        Ok(false) => Ok(()),
        Err(e) => {
            eprintln!("Safe Browsing check failed, letting the Target through: {e}");
            Ok(())
        }
    }
}

/// Custom Codes that would shadow the service's own routes (compared case-insensitively).
const RESERVED_CODES: &[&str] = &["api", "admin", "app", "health", "report"];

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
    let link = insert_link(&s, workspace_id, req).await?;
    Ok((StatusCode::CREATED, Json(link)))
}

/// Creates a Link in Workspace `workspace_id`; shared by create and CSV import, so
/// every attempt counts against the Workspace's rate limit.
async fn insert_link(
    s: &AppState,
    workspace_id: i64,
    req: crablink_api::CreateLink,
) -> Result<crablink_api::Link, ApiError> {
    s.limiters.links.hit(workspace_id, (s.clock)())?;
    validate_target(&req.target, &s.default_host)?;
    if let Some(code) = &req.code {
        validate_custom_code(code)?;
    }
    let tags = tag_names(&req.tags)?;
    let domain = s.domain_or_default(req.domain.as_deref());
    if let Some(code) = &req.code
        && s.reserved_codes.contains(code)
        && domain.eq_ignore_ascii_case(&s.default_domain)
    {
        return Err(ApiError::CodeReserved(code.clone()));
    }
    let domain_id = crate::domains::usable(&s.pool, workspace_id, domain)
        .await?
        .ok_or(ApiError::DomainNotApproved)?;
    check_target(s, &req.target).await?;
    let now = (s.clock)();
    let mut tx = s.pool.begin().await?;
    // Insert and let the unique constraint reject a taken Code; never check first (ADR-0003).
    let mut id = None;
    for _ in 0..MAX_CODE_ATTEMPTS {
        let code = req.code.clone().unwrap_or_else(|| (s.codes)());
        // A Generated Code is never a Reserved Code, on any Domain.
        if req.code.is_none() && s.reserved_codes.contains(&code) {
            continue;
        }
        id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO links \
             (code, target, created_at, updated_at, expires_at, domain_id, workspace_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (domain_id, code) DO NOTHING RETURNING id",
        )
        .bind(code)
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
    Ok(link.into_dto(&s.scheme))
}

/// The columns of `links.csv`, in `CsvRow`'s field order.
const CSV_COLUMNS: [&str; 6] = ["Code", "Domain", "Target", "Expiry", "Tags", "Click Count"];

/// A row of `links.csv`, its fields in `CSV_COLUMNS` order. Export fills every column;
/// on import empty or missing cells are `None` (a Generated Code, the Default Domain,
/// no Expiry) and Click Count is ignored.
#[derive(serde::Serialize, serde::Deserialize)]
struct CsvRow {
    code: Option<String>,
    domain: Option<String>,
    target: String,
    expiry: Option<DateTime<Utc>>,
    /// Tag names joined with commas, which Tags never contain.
    tags: String,
    #[serde(skip_deserializing)]
    click_count: i64,
}

fn csv_error(e: impl std::fmt::Display) -> ApiError {
    eprintln!("cannot write links.csv: {e}");
    ApiError::Internal
}

/// `GET /api/workspaces/{slug}/links.csv`: every Link `list_links` would list, oldest
/// first, as CSV.
pub(crate) async fn export_links(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let rows = sqlx::query_as::<_, LinkRow>(concat!(
        "SELECT ",
        link_columns!(),
        " FROM links l JOIN domains d ON d.id = l.domain_id \
         WHERE l.workspace_id = $1 AND l.deleted_at IS NULL \
         AND (d.is_default OR d.workspace_id = $1) ORDER BY l.id"
    ))
    .bind(workspace_id)
    .fetch_all(&s.pool)
    .await?;
    let mut out = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    // Written by hand: `serialize` would only write a header with a first row.
    out.write_record(CSV_COLUMNS).map_err(csv_error)?;
    for r in rows {
        out.serialize(CsvRow {
            code: Some(r.code),
            domain: Some(r.domain),
            target: r.target,
            expiry: r.expires_at,
            tags: r.tags.join(","),
            click_count: r.click_count,
        })
        .map_err(csv_error)?;
    }
    let body = out.into_inner().map_err(csv_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"links.csv\"",
            ),
        ],
        body,
    )
        .into_response())
}

/// `POST /api/workspaces/{slug}/links.csv`: creates a Link from each row of a CSV in
/// the export's format, never touching existing Links. Not atomic: a row that fails
/// (taken Code, unusable Domain, bad cell) is skipped and reported with its `kind`.
pub(crate) async fn import_links(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    body: String,
) -> Result<Json<Vec<crablink_api::ImportedRow>>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let mut reader = csv::Reader::from_reader(body.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    // Where each column but Click Count is in this file, if anywhere.
    let columns: Vec<Option<usize>> = CSV_COLUMNS[..5]
        .iter()
        .map(|c| headers.iter().position(|h| h == *c))
        .collect();
    let mut results = Vec::new();
    for record in reader.records() {
        let line = match &record {
            Ok(r) => r.position(),
            Err(e) => e.position(),
        }
        .map_or(0, csv::Position::line);
        let row = record.and_then(|r| {
            let cell = |c: &Option<usize>| c.and_then(|i| r.get(i)).unwrap_or("");
            csv::StringRecord::from_iter(columns.iter().map(cell)).deserialize::<CsvRow>(None)
        });
        let created = match row {
            Ok(row) => {
                let req = crablink_api::CreateLink {
                    target: row.target,
                    code: row.code,
                    expires_at: row.expiry,
                    domain: row.domain,
                    tags: row
                        .tags
                        .split(',')
                        .filter(|t| !t.trim().is_empty())
                        .map(str::to_owned)
                        .collect(),
                };
                insert_link(&s, workspace_id, req).await
            }
            Err(e) => Err(ApiError::BadRequest(e.to_string())),
        };
        results.push(match created {
            Ok(link) => crablink_api::ImportedRow {
                line,
                short_url: Some(link.short_url),
                skipped: None,
            },
            Err(e) => crablink_api::ImportedRow {
                line,
                short_url: None,
                skipped: Some(e.body()),
            },
        });
    }
    Ok(Json(results))
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
    fetch_link(&s, workspace_id, q.domain.as_deref(), &code)
        .await
        .map(Json)
}

/// The id of the Workspace's live or expired Link with this Code on `domain` (the
/// Default Domain when `None`); 404 if missing or deleted.
pub(crate) async fn link_id(
    s: &AppState,
    workspace_id: i64,
    domain: Option<&str>,
    code: &str,
) -> Result<i64, ApiError> {
    sqlx::query_scalar(
        "SELECT l.id FROM links l JOIN domains d ON d.id = l.domain_id \
         WHERE l.workspace_id = $1 AND d.host = lower($2) AND l.code = $3 \
         AND l.deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(s.domain_or_default(domain))
    .bind(code)
    .fetch_optional(&s.pool)
    .await?
    .ok_or(ApiError::LinkNotFound)
}

/// `link_id`'s Link as its DTO.
async fn fetch_link(
    s: &AppState,
    workspace_id: i64,
    domain: Option<&str>,
    code: &str,
) -> Result<crablink_api::Link, ApiError> {
    let id = link_id(s, workspace_id, domain, code).await?;
    let row = link_by_id(&mut *s.pool.acquire().await?, id).await?;
    Ok(row.into_dto(&s.scheme))
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
    let link = fetch_link(&s, workspace_id, q.domain.as_deref(), &code).await?;
    // Its Short URL no longer redirects, so a QR code of it would only mislead.
    if link.blocked {
        return Err(ApiError::LinkBlocked);
    }
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
    let id = link_id(&s, workspace_id, q.domain.as_deref(), &code).await?;
    if let Some(target) = &req.target {
        // A Blocked Link refuses every change, so say so before asking Safe Browsing.
        if link_by_id(&mut *s.pool.acquire().await?, id).await?.blocked {
            return Err(ApiError::LinkBlocked);
        }
        check_target(&s, target).await?;
    }
    let mut tx = s.pool.begin().await?;
    // Still live and not blocked: it may have changed since `link_id` found it.
    let blocked: Option<bool> = sqlx::query_scalar(
        "UPDATE links SET target = COALESCE($1, target), \
         expires_at = CASE WHEN $2 THEN $3 ELSE expires_at END, updated_at = $4 \
         WHERE id = $5 AND deleted_at IS NULL RETURNING blocked_at IS NOT NULL",
    )
    .bind(&req.target)
    .bind(req.expires_at.is_some())
    .bind(req.expires_at.flatten())
    .bind((s.clock)())
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    match blocked {
        None => return Err(ApiError::LinkNotFound),
        // Dropping `tx` rolls the change back.
        Some(true) => return Err(ApiError::LinkBlocked),
        Some(false) => {}
    }
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
    let id = link_id(&s, workspace_id, q.domain.as_deref(), &code).await?;
    // Still live: a concurrent delete may have got there first.
    let done = sqlx::query("UPDATE links SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL")
        .bind((s.clock)())
        .bind(id)
        .execute(&s.pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::LinkNotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
