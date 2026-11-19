//! Domains: the custom Domains Owners add under `/api/workspaces/{slug}/domains` and
//! Instance Admins approve under `/api/admin/domains` (ADR-0005), which Domains are
//! served, and `/internal/domain-allowed`, which the reverse proxy asks before
//! issuing a certificate (ADR-0009).

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use crablink_api::{AddDomain, Domain, DomainStatus, ManagedDomain, UpdateDomain};
use sqlx::PgPool;

use crate::AppState;
use crate::admin::InstanceAdmin;
use crate::error::ApiError;
use crate::workspaces::Caller;

/// The id of the Domain `host` if it is served: the Default Domain, or a custom
/// Domain that a Workspace holds and an Instance Admin approved.
pub(crate) async fn served(pool: &PgPool, host: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM domains WHERE host = lower($1) \
         AND (is_default OR (status = 'approved' AND workspace_id IS NOT NULL))",
    )
    .bind(host)
    .fetch_optional(pool)
    .await
}

/// The id of the Domain `host` if Workspace `workspace_id` may put Links on it: the
/// Default Domain, or one of its own approved Domains.
pub(crate) async fn usable(
    pool: &PgPool,
    workspace_id: i64,
    host: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM domains WHERE host = lower($1) \
         AND (is_default OR (status = 'approved' AND workspace_id = $2))",
    )
    .bind(host)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
}

#[derive(serde::Deserialize)]
pub(crate) struct AllowedQuery {
    domain: String,
}

/// `GET /internal/domain-allowed?domain=`: 200 if the Domain is served, 404
/// otherwise. Needs no credential and says nothing beyond yes or no.
pub(crate) async fn domain_allowed(
    State(s): State<AppState>,
    query: Result<Query<AllowedQuery>, QueryRejection>,
) -> Result<StatusCode, ApiError> {
    let Query(q) = query?;
    Ok(match served(&s.pool, &q.domain).await? {
        Some(_) => StatusCode::OK,
        None => StatusCode::NOT_FOUND,
    })
}

/// A custom Domain's host, lowercased: a host name, not an IP address, with no port.
fn valid_host(host: &str) -> Result<String, ApiError> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    match url::Host::parse(&host) {
        Ok(url::Host::Domain(d)) if d == host && host.contains('.') => Ok(host),
        _ => Err(ApiError::InvalidDomain),
    }
}

/// The `DomainStatus` a `domains.status` value stands for; any other value is a bug.
fn status(stored: &str) -> Result<DomainStatus, ApiError> {
    match stored {
        "pending" => Ok(DomainStatus::Pending),
        "approved" => Ok(DomainStatus::Approved),
        "rejected" => Ok(DomainStatus::Rejected),
        other => {
            eprintln!("unknown status in domains.status: {other:?}");
            Err(ApiError::Internal)
        }
    }
}

/// How `status` is stored in `domains.status`.
fn stored_status(status: DomainStatus) -> &'static str {
    match status {
        DomainStatus::Pending => "pending",
        DomainStatus::Approved => "approved",
        DomainStatus::Rejected => "rejected",
    }
}

/// `POST /api/workspaces/{slug}/domains`, Owners only: adds a pending custom Domain.
/// A host the Default Domain or another Workspace has is `domain_taken`; one no
/// Workspace holds any more is taken over, its Deleted Links keeping their Codes.
pub(crate) async fn add_domain(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    body: Result<Json<AddDomain>, JsonRejection>,
) -> Result<(StatusCode, Json<Domain>), ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let Json(req) = body?;
    let host = valid_host(&req.host)?;
    let added = sqlx::query(
        "INSERT INTO domains (host, workspace_id, status) VALUES ($1, $2, 'pending') \
         ON CONFLICT (host) DO UPDATE SET workspace_id = $2, status = 'pending' \
         WHERE domains.workspace_id IS NULL AND NOT domains.is_default",
    )
    .bind(&host)
    .bind(workspace_id)
    .execute(&s.pool)
    .await?;
    if added.rows_affected() == 0 {
        return Err(ApiError::DomainTaken);
    }
    let status = DomainStatus::Pending;
    let domain = Domain {
        host,
        status,
        default: false,
    };
    Ok((StatusCode::CREATED, Json(domain)))
}

/// `GET /api/workspaces/{slug}/domains`: the Default Domain, then the Workspace's
/// custom Domains by host.
pub(crate) async fn list_domains(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
) -> Result<Json<Vec<Domain>>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let rows: Vec<(String, String, bool)> = sqlx::query_as(
        "SELECT host, status, is_default FROM domains \
         WHERE is_default OR workspace_id = $1 ORDER BY is_default DESC, host",
    )
    .bind(workspace_id)
    .fetch_all(&s.pool)
    .await?;
    let domains = rows.into_iter().map(|(host, stored, default)| {
        Ok(Domain {
            host,
            status: status(&stored)?,
            default,
        })
    });
    Ok(Json(domains.collect::<Result<_, ApiError>>()?))
}

/// `DELETE /api/workspaces/{slug}/domains/{host}`, Owners only: its Links become
/// Deleted Links, and the Domain no Workspace's, so their Codes stay taken (ADR-0002).
pub(crate) async fn remove_domain(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, host)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let mut tx = s.pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "UPDATE domains SET workspace_id = NULL \
         WHERE host = lower($1) AND workspace_id = $2 RETURNING id",
    )
    .bind(&host)
    .bind(workspace_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::DomainNotFound)?;
    sqlx::query("UPDATE links SET deleted_at = $2 WHERE domain_id = $1 AND deleted_at IS NULL")
        .bind(id)
        .bind((s.clock)())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/admin/domains`: every custom Domain a Workspace holds, by host.
pub(crate) async fn list_managed_domains(
    State(s): State<AppState>,
    _: InstanceAdmin,
) -> Result<Json<Vec<ManagedDomain>>, ApiError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT d.host, d.status, w.slug FROM domains d \
         JOIN workspaces w ON w.id = d.workspace_id ORDER BY d.host",
    )
    .fetch_all(&s.pool)
    .await?;
    let domains = rows.into_iter().map(managed_domain);
    Ok(Json(domains.collect::<Result<_, ApiError>>()?))
}

/// `PATCH /api/admin/domains/{host}`: approves or rejects a custom Domain a
/// Workspace holds.
pub(crate) async fn update_managed_domain(
    State(s): State<AppState>,
    _: InstanceAdmin,
    Path(host): Path<String>,
    body: Result<Json<UpdateDomain>, JsonRejection>,
) -> Result<Json<ManagedDomain>, ApiError> {
    let Json(req) = body?;
    let row = sqlx::query_as(
        "UPDATE domains d SET status = $2 FROM workspaces w \
         WHERE w.id = d.workspace_id AND d.host = lower($1) \
         RETURNING d.host, d.status, w.slug",
    )
    .bind(&host)
    .bind(stored_status(req.status))
    .fetch_optional(&s.pool)
    .await?
    .ok_or(ApiError::DomainNotFound)?;
    managed_domain(row).map(Json)
}

fn managed_domain(
    (host, stored, workspace): (String, String, String),
) -> Result<ManagedDomain, ApiError> {
    Ok(ManagedDomain {
        host,
        status: status(&stored)?,
        workspace,
    })
}
