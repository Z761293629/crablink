//! `/api/workspaces/{slug}/members`: Owners add Users by email, promote, demote and
//! remove them; any member can leave. A Workspace always keeps an Owner.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use crablink_api::{AddMember, Member, Role, UpdateMember};

use crate::AppState;
use crate::error::ApiError;
use crate::workspaces::{Caller, role};

/// `GET /api/workspaces/{slug}/members`: by email.
pub(crate) async fn list_members(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
) -> Result<Json<Vec<Member>>, ApiError> {
    let (workspace_id, _) = caller.owner_of(&s, &slug).await?;
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.email, m.role FROM memberships m JOIN users u ON u.id = m.user_id \
         WHERE m.workspace_id = $1 ORDER BY lower(u.email)",
    )
    .bind(workspace_id)
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(email, r)| Member {
                email,
                role: role(&r),
            })
            .collect(),
    ))
}

/// `POST /api/workspaces/{slug}/members`: adds an existing User, ignoring the email's
/// case, as a Member.
pub(crate) async fn add_member(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    body: Result<Json<AddMember>, JsonRejection>,
) -> Result<(StatusCode, Json<Member>), ApiError> {
    let (workspace_id, _) = caller.owner_of(&s, &slug).await?;
    let Json(req) = body?;
    let (user_id, email): (i64, String) =
        sqlx::query_as("SELECT id, email FROM users WHERE lower(email) = lower($1)")
            .bind(req.email.trim())
            .fetch_optional(&s.pool)
            .await?
            .ok_or(ApiError::UserNotFound)?;
    let added = sqlx::query(
        "INSERT INTO memberships (user_id, workspace_id, role) VALUES ($1, $2, 'member') \
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(workspace_id)
    .execute(&s.pool)
    .await?;
    if added.rows_affected() == 0 {
        return Err(ApiError::AlreadyMember);
    }
    let role = Role::Member;
    Ok((StatusCode::CREATED, Json(Member { email, role })))
}

/// `PATCH /api/workspaces/{slug}/members/{email}`: promotes or demotes, Owners themselves
/// included.
pub(crate) async fn update_member(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, email)): Path<(String, String)>,
    body: Result<Json<UpdateMember>, JsonRejection>,
) -> Result<Json<Member>, ApiError> {
    let (workspace_id, _) = caller.owner_of(&s, &slug).await?;
    let Json(UpdateMember { role }) = body?;
    let email = change(&s, workspace_id, &email, None, Some(role)).await?;
    Ok(Json(Member { email, role }))
}

/// `DELETE /api/workspaces/{slug}/members/{email}`: an Owner removes anyone; a
/// Member only themself, i.e. leaves.
pub(crate) async fn remove_member(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, email)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let (workspace_id, owner) = caller.workspace(&s, &slug).await?;
    let only = match (owner, &caller) {
        (true, _) => None,
        (false, Caller::User(session)) => Some(session.user_id),
        (false, Caller::ApiKey { .. }) => return Err(ApiError::Forbidden),
    };
    change(&s, workspace_id, &email, only, None).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Gives member `email` (ignoring case) of the Workspace `role`, or removes them
/// when `None`, unless that would leave it without an Owner. With `only`, they
/// must be that User, or it is 403. Returns their email as stored.
async fn change(
    s: &AppState,
    workspace_id: i64,
    email: &str,
    only: Option<i64>,
    role: Option<Role>,
) -> Result<String, ApiError> {
    let mut tx = s.pool.begin().await?;
    // One change per Workspace at a time, so two Owners demoting each other can't
    // both see the other still an Owner.
    sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR UPDATE")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
    let (user_id, email): (i64, String) = sqlx::query_as(
        "SELECT u.id, u.email FROM memberships m JOIN users u ON u.id = m.user_id \
         WHERE m.workspace_id = $1 AND lower(u.email) = lower($2)",
    )
    .bind(workspace_id)
    .bind(email)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::UserNotFound)?;
    if only.is_some_and(|only| only != user_id) {
        return Err(ApiError::Forbidden);
    }
    let query = match role {
        Some(role) => {
            sqlx::query("UPDATE memberships SET role = $3 WHERE workspace_id = $1 AND user_id = $2")
                .bind(workspace_id)
                .bind(user_id)
                .bind(match role {
                    Role::Owner => "owner",
                    Role::Member => "member",
                })
        }
        None => sqlx::query("DELETE FROM memberships WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace_id)
            .bind(user_id),
    };
    query.execute(&mut *tx).await?;
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM memberships WHERE workspace_id = $1 AND role = 'owner'",
    )
    .bind(workspace_id)
    .fetch_one(&mut *tx)
    .await?;
    if owners == 0 {
        return Err(ApiError::LastOwner);
    }
    tx.commit().await?;
    Ok(email)
}
