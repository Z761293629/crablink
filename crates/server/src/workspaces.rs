//! Workspaces and who may act on them: `Caller`, the one extractor for both
//! credentials, and the personal Workspace each User gets on first sign-in.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use crablink_api::{CreateWorkspace, Membership, Role, UpdateWorkspace, Workspace};
use sqlx::PgConnection;

use crate::AppState;
use crate::error::ApiError;
use crate::users::Session;

/// Who is acting: a signed-in User, from the session cookie, or a Workspace, from
/// `Authorization: Bearer <API Key>`. A Bearer value wins over a cookie, and one
/// that is no live API Key is a 401 telling a 0.2 CLI user to sign in again.
pub(crate) enum Caller {
    User(Session),
    ApiKey { workspace_id: i64 },
}

impl FromRequestParts<AppState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, s: &AppState) -> Result<Self, ApiError> {
        let bearer = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        match bearer {
            Some(secret) => crate::api_keys::workspace_of(s, secret.trim())
                .await?
                .map(|workspace_id| Caller::ApiKey { workspace_id })
                .ok_or(ApiError::InvalidApiKey),
            None => Session::from_request_parts(parts, s)
                .await
                .map(Caller::User),
        }
    }
}

/// A Workspace a `Caller` may act on.
#[derive(sqlx::FromRow)]
pub(crate) struct Access {
    pub(crate) workspace_id: i64,
    /// Whether the caller is the session of one of its Owners.
    pub(crate) owner: bool,
}

/// A Workspace acted on by the session of one of its Owners.
pub(crate) struct OwnerAccess {
    pub(crate) workspace_id: i64,
    pub(crate) user_id: i64,
}

impl Caller {
    /// Workspace `slug`, if this is a member's session or one of its API Keys. Any
    /// other Workspace, existing or not, is 403.
    pub(crate) async fn workspace(&self, s: &AppState, slug: &str) -> Result<Access, ApiError> {
        let query = match self {
            Caller::User(session) => sqlx::query_as(
                "SELECT w.id AS workspace_id, m.role = 'owner' AS owner FROM workspaces w \
                 JOIN memberships m ON m.workspace_id = w.id \
                 WHERE w.slug = $1 AND m.user_id = $2",
            )
            .bind(slug)
            .bind(session.user_id),
            Caller::ApiKey { workspace_id } => sqlx::query_as(
                "SELECT id AS workspace_id, FALSE AS owner FROM workspaces \
                     WHERE slug = $1 AND id = $2",
            )
            .bind(slug)
            .bind(workspace_id),
        };
        query
            .fetch_optional(&s.pool)
            .await?
            .ok_or(ApiError::Forbidden)
    }

    /// Workspace `slug`, if this is the session of one of its Owners; 403 otherwise,
    /// API Keys included.
    pub(crate) async fn owner_of(&self, s: &AppState, slug: &str) -> Result<OwnerAccess, ApiError> {
        match (self, self.workspace(s, slug).await?) {
            (
                Caller::User(session),
                Access {
                    workspace_id,
                    owner: true,
                },
            ) => Ok(OwnerAccess {
                workspace_id,
                user_id: session.user_id,
            }),
            _ => Err(ApiError::Forbidden),
        }
    }
}

const MAX_NAME_LEN: usize = 100;

/// The role stored in `memberships.role`.
pub(crate) fn role(stored: &str) -> Role {
    match stored {
        "owner" => Role::Owner,
        _ => Role::Member,
    }
}

/// A Workspace name, trimmed: 1-100 characters.
fn valid_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(ApiError::BadRequest(format!(
            "name must be 1-{MAX_NAME_LEN} characters"
        )));
    }
    Ok(name)
}

/// `GET /api/workspaces`: the signed-in User's Workspaces, by name, with their role.
pub(crate) async fn list_workspaces(
    State(s): State<AppState>,
    caller: Caller,
) -> Result<Json<Vec<Membership>>, ApiError> {
    let Caller::User(session) = caller else {
        return Err(ApiError::Forbidden);
    };
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT w.slug, w.name, m.role FROM workspaces w \
         JOIN memberships m ON m.workspace_id = w.id \
         WHERE m.user_id = $1 ORDER BY w.name, w.slug",
    )
    .bind(session.user_id)
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(slug, name, r)| Membership {
                workspace: Workspace { slug, name },
                role: role(&r),
            })
            .collect(),
    ))
}

/// `POST /api/workspaces`: the signed-in User becomes the new Workspace's Owner.
pub(crate) async fn create_workspace(
    State(s): State<AppState>,
    caller: Caller,
    body: Result<Json<CreateWorkspace>, JsonRejection>,
) -> Result<(StatusCode, Json<Membership>), ApiError> {
    let Caller::User(session) = caller else {
        return Err(ApiError::Forbidden);
    };
    let Json(req) = body?;
    let name = valid_name(&req.name)?;
    let mut tx = s.pool.begin().await?;
    let workspace = create(&mut tx, session.user_id, name, name).await?;
    tx.commit().await?;
    let role = Role::Owner;
    Ok((StatusCode::CREATED, Json(Membership { workspace, role })))
}

/// `PATCH /api/workspaces/{slug}`: renames it; the slug stays.
pub(crate) async fn rename_workspace(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    body: Result<Json<UpdateWorkspace>, JsonRejection>,
) -> Result<Json<Workspace>, ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let Json(req) = body?;
    let name = valid_name(&req.name)?.to_owned();
    sqlx::query("UPDATE workspaces SET name = $2 WHERE id = $1")
        .bind(workspace_id)
        .bind(&name)
        .execute(&s.pool)
        .await?;
    Ok(Json(Workspace { slug, name }))
}

/// `DELETE /api/workspaces/{slug}`: its Links become Deleted Links, their Codes
/// staying taken (ADR-0002), and its API Keys stop working.
pub(crate) async fn delete_workspace(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let mut tx = s.pool.begin().await?;
    sqlx::query("UPDATE links SET deleted_at = $2 WHERE workspace_id = $1 AND deleted_at IS NULL")
        .bind(workspace_id)
        .bind((s.clock)())
        .execute(&mut *tx)
        .await?;
    // Takes its memberships and API Keys with it; its Links stay, with no Workspace.
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Any other `/api` path: 404, except that an unknown Bearer value gets its 401, so a
/// 0.2 CLI calling the old `/api/links` learns to sign in again with an API Key.
pub(crate) async fn api_fallback(caller: Result<Caller, ApiError>) -> ApiError {
    match caller {
        Err(e @ ApiError::InvalidApiKey) => e,
        _ => ApiError::NotFound,
    }
}

/// Creates the personal Workspace of User `user_id`, named after their email.
pub(crate) async fn create_personal(
    conn: &mut PgConnection,
    user_id: i64,
    email: &str,
) -> Result<(), ApiError> {
    let local = email.split('@').next().unwrap_or_default();
    create(conn, user_id, email, local).await.map(drop)
}

/// Creates Workspace `name` with User `user_id` as its Owner, on `conn`: run it in a
/// transaction, or a failure can leave a Workspace with no Owner. Its slug comes from
/// `slug_from`, lowercased, numbered if taken: `ada`, `ada-2`, …
async fn create(
    conn: &mut PgConnection,
    user_id: i64,
    name: &str,
    slug_from: &str,
) -> Result<Workspace, ApiError> {
    let base: String = slug_from
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = match base.get(..32) {
        _ if base.is_empty() => "workspace".to_owned(),
        Some(cut) => cut.trim_end_matches('-').to_owned(),
        None => base,
    };
    // ponytail: one query per taken slug; random suffixes if same-named Workspaces pile up.
    let mut n = 1;
    loop {
        let slug = match n {
            1 => base.clone(),
            n => format!("{base}-{n}"),
        };
        n += 1;
        let id: Option<i64> = sqlx::query_scalar(
            "INSERT INTO workspaces (name, slug) VALUES ($1, $2) \
             ON CONFLICT (slug) DO NOTHING RETURNING id",
        )
        .bind(name)
        .bind(&slug)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(id) = id {
            sqlx::query(
                "INSERT INTO memberships (user_id, workspace_id, role) VALUES ($1, $2, 'owner')",
            )
            .bind(user_id)
            .bind(id)
            .execute(&mut *conn)
            .await?;
            let name = name.to_owned();
            return Ok(Workspace { slug, name });
        }
    }
}
