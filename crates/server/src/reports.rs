//! Reports: any visitor reports a Short URL with a reason, and Instance Admins work
//! through the open ones. Blocking the Link (`admin::block_link`) or dismissing the
//! report closes it.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use crablink_api::{CreateReport, Report, ReportedUser};

use crate::AppState;
use crate::admin::InstanceAdmin;
use crate::error::ApiError;
use crate::links::{parse_short_url, short_url};
use crate::rate_limit::VisitorIp;
use crate::workspaces::role;

/// The longest reason a report takes, in characters.
const MAX_REASON: usize = 1000;

/// `POST /api/reports`: needs no credential; the Short URL must be a live Link of
/// this instance. Limited per visitor IP.
pub(crate) async fn create_report(
    State(s): State<AppState>,
    VisitorIp(ip): VisitorIp,
    body: Result<Json<CreateReport>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    s.limiters.reports.hit(ip, (s.clock)())?;
    let Json(req) = body?;
    let (host, code) = parse_short_url(&req.short_url)?;
    let reason = req.reason.trim();
    if reason.is_empty() || reason.chars().count() > MAX_REASON {
        return Err(ApiError::BadRequest(format!(
            "reason must be 1 to {MAX_REASON} characters"
        )));
    }
    let done = sqlx::query(
        "INSERT INTO reports (link_id, reason, reported_at) \
         SELECT l.id, $3, $4 FROM links l JOIN domains d ON d.id = l.domain_id \
         WHERE d.host = lower($1) AND l.code = $2 AND l.deleted_at IS NULL",
    )
    .bind(host)
    .bind(code)
    .bind(reason)
    .bind((s.clock)())
    .execute(&s.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::LinkNotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct ReportRow {
    id: i64,
    host: String,
    code: String,
    target: String,
    workspace_id: i64,
    workspace_slug: String,
    reason: String,
    reported_at: DateTime<Utc>,
}

/// `GET /api/admin/reports`: the open reports of live Links, oldest first, each with
/// its Workspace's Owners and Members.
pub(crate) async fn list_reports(
    State(s): State<AppState>,
    _: InstanceAdmin,
) -> Result<Json<Vec<Report>>, ApiError> {
    let rows: Vec<ReportRow> = sqlx::query_as(
        "SELECT r.id, d.host, l.code, l.target, w.id AS workspace_id, w.slug AS workspace_slug, r.reason, \
         r.reported_at \
         FROM reports r JOIN links l ON l.id = r.link_id \
         JOIN domains d ON d.id = l.domain_id JOIN workspaces w ON w.id = l.workspace_id \
         WHERE r.closed_at IS NULL AND l.deleted_at IS NULL \
         ORDER BY r.reported_at, r.id",
    )
    .fetch_all(&s.pool)
    .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.workspace_id).collect();
    let users: Vec<(i64, i64, String, String, bool)> = sqlx::query_as(
        "SELECT m.workspace_id, u.id, u.email, m.role, u.disabled \
         FROM memberships m JOIN users u ON u.id = m.user_id \
         WHERE m.workspace_id = ANY($1) ORDER BY lower(u.email)",
    )
    .bind(&ids)
    .fetch_all(&s.pool)
    .await?;
    let reports = rows.into_iter().map(|r| {
        let workspace_users = users
            .iter()
            .filter(|u| u.0 == r.workspace_id)
            .map(|(_, id, email, stored, disabled)| {
                Ok(ReportedUser {
                    id: *id,
                    email: email.clone(),
                    role: role(stored)?,
                    disabled: *disabled,
                })
            })
            .collect::<Result<_, ApiError>>()?;
        Ok(Report {
            id: r.id,
            short_url: short_url(&s.scheme, &r.host, &r.code),
            target: r.target,
            workspace_slug: r.workspace_slug,
            reason: r.reason,
            reported_at: r.reported_at,
            workspace_users,
        })
    });
    Ok(Json(reports.collect::<Result<_, ApiError>>()?))
}

/// `DELETE /api/admin/reports/{id}`: dismisses an open report, leaving its Link as
/// it is; 404 if no open report has that id.
pub(crate) async fn dismiss_report(
    State(s): State<AppState>,
    _: InstanceAdmin,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let done = sqlx::query("UPDATE reports SET closed_at = $2 WHERE id = $1 AND closed_at IS NULL")
        .bind(id)
        .bind((s.clock)())
        .execute(&s.pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
