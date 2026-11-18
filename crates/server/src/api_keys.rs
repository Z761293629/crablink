//! `/api/workspaces/{slug}/api-keys`: Owners create, list and revoke a Workspace's
//! API Keys. A key belongs to its Workspace, so it outlives the User who made it.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use crablink_api::{ApiKey, CreateApiKey, NewApiKey};

use crate::AppState;
use crate::error::ApiError;
use crate::users::{random_token, token_hash};
use crate::workspaces::Caller;

/// Every secret starts with this, so a leaked one is recognisable.
const SECRET_PREFIX: &str = "crab_";
/// Characters of the secret kept visible: `crab_` and 7 more.
const VISIBLE: usize = 12;
const MAX_NAME_LEN: usize = 100;

/// The Workspace of the live API Key `secret`, recording that it was used now.
pub(crate) async fn workspace_of(s: &AppState, secret: &str) -> Result<Option<i64>, ApiError> {
    // ponytail: one write per request; batch `last_used_at` if it ever shows up in profiles.
    Ok(sqlx::query_scalar(
        "UPDATE api_keys SET last_used_at = $2 \
         WHERE secret_hash = $1 AND revoked_at IS NULL RETURNING workspace_id",
    )
    .bind(token_hash(secret))
    .bind((s.clock)())
    .fetch_optional(&s.pool)
    .await?)
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: i64,
    name: String,
    prefix: String,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<KeyRow> for ApiKey {
    fn from(r: KeyRow) -> ApiKey {
        ApiKey {
            id: r.id,
            name: r.name,
            prefix: r.prefix,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        }
    }
}

/// `POST /api/workspaces/{slug}/api-keys`: the response is the only time the secret is shown.
pub(crate) async fn create_api_key(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    body: Result<Json<CreateApiKey>, JsonRejection>,
) -> Result<(StatusCode, Json<NewApiKey>), ApiError> {
    let owner = caller.owner_of(&s, &slug).await?;
    let Json(req) = body?;
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(ApiError::BadRequest(format!(
            "name must be 1-{MAX_NAME_LEN} characters"
        )));
    }
    let secret = format!("{SECRET_PREFIX}{}", random_token());
    let row: KeyRow = sqlx::query_as(
        "INSERT INTO api_keys (workspace_id, name, prefix, secret_hash, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, name, prefix, created_at, last_used_at",
    )
    .bind(owner.workspace_id)
    .bind(name)
    .bind(&secret[..VISIBLE])
    .bind(token_hash(&secret))
    .bind(owner.user_id)
    .bind((s.clock)())
    .fetch_one(&s.pool)
    .await?;
    let key = row.into();
    Ok((StatusCode::CREATED, Json(NewApiKey { key, secret })))
}

/// `GET /api/workspaces/{slug}/api-keys`: the live keys, oldest first.
pub(crate) async fn list_api_keys(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
) -> Result<Json<Vec<ApiKey>>, ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let rows: Vec<KeyRow> = sqlx::query_as(
        "SELECT id, name, prefix, created_at, last_used_at FROM api_keys \
         WHERE workspace_id = $1 AND revoked_at IS NULL ORDER BY id",
    )
    .bind(workspace_id)
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// `DELETE /api/workspaces/{slug}/api-keys/{id}`: the key stops working at once.
/// 404 if it isn't a live key of this Workspace.
pub(crate) async fn revoke_api_key(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, id)): Path<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = caller.owner_of(&s, &slug).await?.workspace_id;
    let done = sqlx::query(
        "UPDATE api_keys SET revoked_at = $3 \
         WHERE id = $1 AND workspace_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(workspace_id)
    .bind((s.clock)())
    .execute(&s.pool)
    .await?;
    match done.rows_affected() {
        0 => Err(ApiError::NotFound),
        _ => Ok(StatusCode::NO_CONTENT),
    }
}
