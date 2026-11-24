//! What Instance Admins do under `/api/admin`: invite Users and manage them. Also
//! accepting an invite, which needs no credential.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::Response;
use chrono::{DateTime, TimeDelta, Utc};
use crablink_api::{
    AcceptInvite, BlockLink, BlockedLink, CreateInvite, ManagedUser, NewInvite, NewPassword, Role,
    UpdateUser, User,
};

use crate::AppState;
use crate::error::ApiError;
use crate::users::{self, SameOrigin};
use crate::workspaces::{Caller, stored_role};

const INVITE_TTL: TimeDelta = TimeDelta::days(7);

/// The session of an Instance Admin. Any other credential is 403.
pub(crate) struct InstanceAdmin;

impl FromRequestParts<AppState> for InstanceAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, s: &AppState) -> Result<Self, ApiError> {
        match Caller::from_request_parts(parts, s).await? {
            Caller::User(session) if session.user.instance_admin => Ok(InstanceAdmin),
            _ => Err(ApiError::Forbidden),
        }
    }
}

/// `POST /api/admin/invites`: an invite link for a new User, optionally into a
/// Workspace, that works once within `INVITE_TTL`.
pub(crate) async fn create_invite(
    State(s): State<AppState>,
    _: InstanceAdmin,
    body: Result<Json<CreateInvite>, JsonRejection>,
) -> Result<(StatusCode, Json<NewInvite>), ApiError> {
    let Json(req) = body?;
    let email = users::valid_email(&req.email)?;
    let taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE lower(email) = lower($1))")
            .bind(email)
            .fetch_one(&s.pool)
            .await?;
    if taken {
        return Err(ApiError::EmailTaken);
    }
    let workspace_id: Option<i64> = match &req.workspace {
        Some(slug) => Some(
            sqlx::query_scalar("SELECT id FROM workspaces WHERE slug = $1")
                .bind(slug)
                .fetch_optional(&s.pool)
                .await?
                .ok_or(ApiError::NotFound)?,
        ),
        None => None,
    };
    let token = users::random_token();
    let expires_at = (s.clock)() + INVITE_TTL;
    sqlx::query(
        "INSERT INTO invites (token_hash, email, workspace_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(users::token_hash(&token))
    .bind(email)
    .bind(workspace_id)
    .bind(expires_at)
    .execute(&s.pool)
    .await?;
    // The token rides in the fragment, which browsers never send to a server.
    let url = format!("{}://{}/app/invite#{token}", s.scheme, s.default_domain);
    Ok((StatusCode::CREATED, Json(NewInvite { url, expires_at })))
}

#[derive(sqlx::FromRow)]
struct InviteRow {
    email: String,
    workspace_id: Option<i64>,
    used: bool,
    expired: bool,
}

/// `POST /api/invites/accept`: creates the invited User with the chosen password,
/// adds them to the invite's Workspace as a Member, and signs them in. Needs this
/// host's `Origin`, like signing in.
pub(crate) async fn accept_invite(
    State(s): State<AppState>,
    _: SameOrigin,
    body: Result<Json<AcceptInvite>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(req) = body?;
    // Hashed before taking the invite's row lock, which it would hold up otherwise.
    let hash = users::hash_password(&req.password).await?;
    let mut tx = s.pool.begin().await?;
    let now = (s.clock)();
    // FOR UPDATE: a second acceptance of the same invite waits, then sees it used.
    let invite: Option<InviteRow> = sqlx::query_as(
        "SELECT email, workspace_id, used_at IS NOT NULL AS used, expires_at <= $2 AS expired \
         FROM invites WHERE token_hash = $1 FOR UPDATE",
    )
    .bind(users::token_hash(&req.token))
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    let InviteRow {
        email,
        workspace_id,
        used,
        expired,
    } = invite.ok_or(ApiError::InviteNotFound)?;
    if used {
        return Err(ApiError::InviteUsed);
    }
    if expired {
        return Err(ApiError::InviteExpired);
    }
    let id = users::insert_user(&mut tx, &email, &hash, false).await?;
    if let Some(workspace_id) = workspace_id {
        sqlx::query("INSERT INTO memberships (user_id, workspace_id, role) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(workspace_id)
            .bind(stored_role(Role::Member))
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE invites SET used_at = $2 WHERE token_hash = $1")
        .bind(users::token_hash(&req.token))
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let user = User {
        email,
        instance_admin: false,
    };
    let res = users::start_session(&s, &mut tx, id, user).await?;
    tx.commit().await?;
    Ok(res)
}

/// `GET /api/admin/users`: every User, by email.
pub(crate) async fn list_users(
    State(s): State<AppState>,
    _: InstanceAdmin,
) -> Result<Json<Vec<ManagedUser>>, ApiError> {
    // ponytail: no pagination; add a cursor like Links' if instances grow past a few hundred Users.
    let rows: Vec<(i64, String, bool, bool)> = sqlx::query_as(
        "SELECT id, email, instance_admin, disabled FROM users ORDER BY lower(email)",
    )
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(rows.into_iter().map(managed_user).collect()))
}

fn managed_user((id, email, instance_admin, disabled): (i64, String, bool, bool)) -> ManagedUser {
    ManagedUser {
        id,
        email,
        instance_admin,
        disabled,
    }
}

/// `PATCH /api/admin/users/{id}`: disables or re-enables a User, or makes them an
/// Instance Admin or not, oneself included, as long as an enabled Instance Admin
/// remains to run the instance.
pub(crate) async fn update_user(
    State(s): State<AppState>,
    _: InstanceAdmin,
    Path(id): Path<i64>,
    body: Result<Json<UpdateUser>, JsonRejection>,
) -> Result<Json<ManagedUser>, ApiError> {
    let Json(req) = body?;
    let mut tx = s.pool.begin().await?;
    // Locks every enabled Instance Admin, so two of them demoting each other at once
    // can't both succeed.
    sqlx::query("SELECT id FROM users WHERE instance_admin AND NOT disabled FOR UPDATE")
        .execute(&mut *tx)
        .await?;
    if let Some(disabled) = req.disabled {
        users::set_disabled(&mut tx, id, disabled).await?;
    }
    if let Some(instance_admin) = req.instance_admin {
        sqlx::query("UPDATE users SET instance_admin = $2 WHERE id = $1")
            .bind(id)
            .bind(instance_admin)
            .execute(&mut *tx)
            .await?;
    }
    let row = sqlx::query_as("SELECT id, email, instance_admin, disabled FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::UserNotFound)?;
    let admins_left: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE instance_admin AND NOT disabled)",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !admins_left {
        // Dropping `tx` rolls the change back.
        return Err(ApiError::LastInstanceAdmin);
    }
    tx.commit().await?;
    Ok(Json(managed_user(row)))
}

/// `POST /api/admin/users/{id}/password`: gives a User a new random password,
/// returned only here, and ends their sessions.
pub(crate) async fn reset_password(
    State(s): State<AppState>,
    _: InstanceAdmin,
    Path(id): Path<i64>,
) -> Result<Json<NewPassword>, ApiError> {
    // 80 random bits: short enough to read out, too long to guess.
    let password = users::random_token()[..20].to_owned();
    let hash = users::hash_password(&password).await?;
    let mut tx = s.pool.begin().await?;
    users::set_password(&mut tx, id, hash, None).await?;
    tx.commit().await?;
    Ok(Json(NewPassword { password }))
}

/// The Domain host and Code of a Short URL such as `https://crabl.ink/aZ3k9` or
/// `crabl.ink/aZ3k9`.
pub(crate) fn parse_short_url(short_url: &str) -> Result<(&str, &str), ApiError> {
    let s = short_url.trim();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    match s.split_once('/') {
        Some((host, code))
            if !host.is_empty() && !code.is_empty() && !code.contains(['/', '?', '#']) =>
        {
            Ok((host, code))
        }
        _ => Err(ApiError::BadRequest(
            "short_url must look like https://crabl.ink/aZ3k9".into(),
        )),
    }
}

#[derive(sqlx::FromRow)]
struct BlockedRow {
    host: String,
    code: String,
    target: String,
    slug: String,
    blocked_at: DateTime<Utc>,
}

impl BlockedRow {
    fn into_dto(self, scheme: &str) -> BlockedLink {
        BlockedLink {
            short_url: format!("{scheme}://{}/{}", self.host, self.code),
            target: self.target,
            workspace_slug: self.slug,
            blocked_at: self.blocked_at,
        }
    }
}

/// `GET /api/admin/blocked-links`: every Blocked Link that isn't a Deleted Link,
/// most recently blocked first.
pub(crate) async fn list_blocked_links(
    State(s): State<AppState>,
    _: InstanceAdmin,
) -> Result<Json<Vec<BlockedLink>>, ApiError> {
    let rows: Vec<BlockedRow> = sqlx::query_as(
        "SELECT d.host, l.code, l.target, w.slug, l.blocked_at \
         FROM links l JOIN domains d ON d.id = l.domain_id \
         JOIN workspaces w ON w.id = l.workspace_id \
         WHERE l.blocked_at IS NOT NULL AND l.deleted_at IS NULL \
         ORDER BY l.blocked_at DESC, l.id DESC",
    )
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(
        rows.into_iter().map(|r| r.into_dto(&s.scheme)).collect(),
    ))
}

/// `POST /api/admin/blocked-links`: blocks the live Link of a Short URL at once.
/// Blocking a Blocked Link keeps when it was first blocked. Closes the Link's open
/// reports, which stay closed if it is unblocked.
pub(crate) async fn block_link(
    State(s): State<AppState>,
    _: InstanceAdmin,
    body: Result<Json<BlockLink>, JsonRejection>,
) -> Result<(StatusCode, Json<BlockedLink>), ApiError> {
    let Json(req) = body?;
    let (host, code) = parse_short_url(&req.short_url)?;
    let row: BlockedRow = sqlx::query_as(
        "WITH blocked AS ( \
           UPDATE links l SET blocked_at = COALESCE(l.blocked_at, $3) \
           FROM domains d, workspaces w \
           WHERE d.id = l.domain_id AND w.id = l.workspace_id \
           AND d.host = lower($1) AND l.code = $2 AND l.deleted_at IS NULL \
           RETURNING l.id, d.host, l.code, l.target, w.slug, l.blocked_at), \
         closed AS ( \
           UPDATE reports SET closed_at = $3 \
           WHERE link_id IN (SELECT id FROM blocked) AND closed_at IS NULL) \
         SELECT host, code, target, slug, blocked_at FROM blocked",
    )
    .bind(host)
    .bind(code)
    .bind((s.clock)())
    .fetch_optional(&s.pool)
    .await?
    .ok_or(ApiError::LinkNotFound)?;
    Ok((StatusCode::CREATED, Json(row.into_dto(&s.scheme))))
}

/// `DELETE /api/admin/blocked-links?short_url=`: unblocks a Blocked Link, which
/// redirects again at once; 404 if the Short URL has no Blocked Link.
pub(crate) async fn unblock_link(
    State(s): State<AppState>,
    _: InstanceAdmin,
    query: Result<Query<BlockLink>, QueryRejection>,
) -> Result<StatusCode, ApiError> {
    let Query(req) = query?;
    let (host, code) = parse_short_url(&req.short_url)?;
    let done = sqlx::query(
        "UPDATE links l SET blocked_at = NULL FROM domains d \
         WHERE d.id = l.domain_id AND d.host = lower($1) AND l.code = $2 \
         AND l.deleted_at IS NULL AND l.blocked_at IS NOT NULL",
    )
    .bind(host)
    .bind(code)
    .execute(&s.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::LinkNotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
