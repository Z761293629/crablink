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

/// The id of `domain` if it is served: the Default Domain, or a custom
/// Domain that a Workspace holds and an Instance Admin approved.
pub(crate) async fn served(pool: &PgPool, domain: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM domains WHERE host = lower($1) \
         AND (is_default OR (status = 'approved' AND workspace_id IS NOT NULL))",
    )
    .bind(domain)
    .fetch_optional(pool)
    .await
}

/// The id of `domain` if Workspace `workspace_id` may put Links on it: the
/// Default Domain, or one of its own approved Domains.
pub(crate) async fn usable(
    pool: &PgPool,
    workspace_id: i64,
    domain: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM domains WHERE host = lower($1) \
         AND (is_default OR (status = 'approved' AND workspace_id = $2))",
    )
    .bind(domain)
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

/// A custom Domain, lowercased: a host name, not an IP address, with no port.
fn valid_domain(domain: &str) -> Result<String, ApiError> {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    match url::Host::parse(&domain) {
        Ok(url::Host::Domain(d)) if d == domain && domain.contains('.') => Ok(domain),
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
/// A Domain that is the Default Domain or another Workspace's is `domain_taken`; one no
/// Workspace holds any more is taken over, its Deleted Links keeping their Codes.
pub(crate) async fn add_domain(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    body: Result<Json<AddDomain>, JsonRejection>,
) -> Result<(StatusCode, Json<Domain>), ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let Json(req) = body?;
    let domain = valid_domain(&req.domain)?;
    let added = sqlx::query(
        "INSERT INTO domains (host, workspace_id, status) VALUES ($1, $2, 'pending') \
         ON CONFLICT (host) DO UPDATE SET workspace_id = $2, status = 'pending' \
         WHERE domains.workspace_id IS NULL AND NOT domains.is_default",
    )
    .bind(&domain)
    .bind(workspace_id)
    .execute(&s.pool)
    .await?;
    if added.rows_affected() == 0 {
        return Err(ApiError::DomainTaken);
    }
    let status = DomainStatus::Pending;
    let domain = Domain {
        domain,
        status,
        default: false,
    };
    Ok((StatusCode::CREATED, Json(domain)))
}

/// `GET /api/workspaces/{slug}/domains`: the Default Domain, then the Workspace's
/// custom Domains by name.
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
    let domains = rows.into_iter().map(|(domain, stored, default)| {
        Ok(Domain {
            domain,
            status: status(&stored)?,
            default,
        })
    });
    Ok(Json(domains.collect::<Result<_, ApiError>>()?))
}

/// `DELETE /api/workspaces/{slug}/domains/{domain}`, Owners only: its Links become
/// Deleted Links, and the Domain no Workspace's, so their Codes stay taken (ADR-0002).
pub(crate) async fn remove_domain(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, domain)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let mut tx = s.pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "UPDATE domains SET workspace_id = NULL \
         WHERE host = lower($1) AND workspace_id = $2 RETURNING id",
    )
    .bind(&domain)
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

/// `GET /api/admin/domains`: every custom Domain a Workspace holds, by name.
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

/// `PATCH /api/admin/domains/{domain}`: approves or rejects a custom Domain a
/// Workspace holds.
pub(crate) async fn update_managed_domain(
    State(s): State<AppState>,
    _: InstanceAdmin,
    Path(domain): Path<String>,
    body: Result<Json<UpdateDomain>, JsonRejection>,
) -> Result<Json<ManagedDomain>, ApiError> {
    let Json(req) = body?;
    let row = sqlx::query_as(
        "UPDATE domains d SET status = $2 FROM workspaces w \
         WHERE w.id = d.workspace_id AND d.host = lower($1) \
         RETURNING d.host, d.status, w.slug",
    )
    .bind(&domain)
    .bind(stored_status(req.status))
    .fetch_optional(&s.pool)
    .await?
    .ok_or(ApiError::DomainNotFound)?;
    managed_domain(row).map(Json)
}

fn managed_domain(
    (domain, stored, workspace_slug): (String, String, String),
) -> Result<ManagedDomain, ApiError> {
    Ok(ManagedDomain {
        domain,
        status: status(&stored)?,
        workspace_slug,
    })
}
