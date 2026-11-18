//! Workspaces and who may act on them: `Caller`, the one extractor for both
//! credentials, and the personal Workspace each User gets on first sign-in.

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::header;
use axum::http::request::Parts;
use crablink_api::Workspace;
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

impl Caller {
    /// The id of Workspace `slug`, if this is a member's session or one of its API
    /// Keys; with the session, whether the User is its Owner. Any other Workspace,
    /// existing or not, is 403.
    pub(crate) async fn workspace(
        &self,
        s: &AppState,
        slug: &str,
    ) -> Result<(i64, bool), ApiError> {
        let query = match self {
            Caller::User(session) => sqlx::query_as(
                "SELECT w.id, m.role = 'owner' FROM workspaces w \
                 JOIN memberships m ON m.workspace_id = w.id \
                 WHERE w.slug = $1 AND m.user_id = $2",
            )
            .bind(slug)
            .bind(session.user_id),
            Caller::ApiKey { workspace_id } => {
                sqlx::query_as("SELECT id, FALSE FROM workspaces WHERE slug = $1 AND id = $2")
                    .bind(slug)
                    .bind(workspace_id)
            }
        };
        query
            .fetch_optional(&s.pool)
            .await?
            .ok_or(ApiError::Forbidden)
    }

    /// `(workspace id, User id)` if this is the session of an Owner of Workspace
    /// `slug`; 403 otherwise, API Keys included.
    pub(crate) async fn owner_of(&self, s: &AppState, slug: &str) -> Result<(i64, i64), ApiError> {
        match (self, self.workspace(s, slug).await?) {
            (Caller::User(session), (id, true)) => Ok((id, session.user_id)),
            _ => Err(ApiError::Forbidden),
        }
    }
}

/// `GET /api/workspaces`: the signed-in User's Workspaces, by name.
pub(crate) async fn list_workspaces(
    State(s): State<AppState>,
    caller: Caller,
) -> Result<Json<Vec<Workspace>>, ApiError> {
    let Caller::User(session) = caller else {
        return Err(ApiError::Forbidden);
    };
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT w.slug, w.name FROM workspaces w \
         JOIN memberships m ON m.workspace_id = w.id \
         WHERE m.user_id = $1 ORDER BY w.name, w.slug",
    )
    .bind(session.user_id)
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(slug, name)| Workspace { slug, name })
            .collect(),
    ))
}

/// Any other `/api` path: 404, except that an unknown Bearer value gets its 401, so a
/// 0.2 CLI calling the old `/api/links` learns to sign in again with an API Key.
pub(crate) async fn api_fallback(caller: Result<Caller, ApiError>) -> ApiError {
    match caller {
        Err(e @ ApiError::InvalidApiKey) => e,
        _ => ApiError::NotFound,
    }
}

/// Creates the personal Workspace of User `user_id`, with them as its Owner. Its
/// slug comes from the email's local part, numbered if taken: `ada`, `ada-2`, …
pub(crate) async fn create_personal(
    conn: &mut PgConnection,
    user_id: i64,
    email: &str,
) -> Result<(), ApiError> {
    let local = email.split('@').next().unwrap_or_default().to_lowercase();
    let base: String = local
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = match base.get(..32) {
        _ if base.is_empty() => "workspace".to_owned(),
        Some(cut) => cut.trim_end_matches('-').to_owned(),
        None => base,
    };
    // ponytail: one query per taken slug; random suffixes if same-named Users pile up.
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
        .bind(email)
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
            return Ok(());
        }
    }
}
