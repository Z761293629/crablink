//! Self-service password reset, on an instance that sends email: a User asks for a
//! link by email, and following it sets a new password.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use crablink_api::{ForgotPassword, ResetPassword, User};

use crate::email_tokens::{self, Purpose};
use crate::error::ApiError;
use crate::rate_limit::VisitorIp;
use crate::users::{self, SameOrigin};
use crate::{AppState, email};

/// `POST /api/password-resets`: emails a reset link to the enabled User with this
/// email who has a password, if any, in the language the request asks for. Answers
/// `204` either way, so it reveals nothing about who has a User. Not found on an
/// instance that sends no email.
pub(crate) async fn forgot_password(
    State(s): State<AppState>,
    _: SameOrigin,
    VisitorIp(ip): VisitorIp,
    headers: HeaderMap,
    body: Result<Json<ForgotPassword>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Some(mailer) = &s.mailer else {
        return Err(ApiError::NotFound);
    };
    s.limiters.password_reset.hit(ip, (s.clock)())?;
    let Json(req) = body?;
    let user: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, email FROM users \
         WHERE lower(email) = lower($1) AND NOT disabled AND password_hash IS NOT NULL",
    )
    .bind(req.email.trim())
    .fetch_optional(&s.pool)
    .await?;
    if let Some((id, address)) = user {
        let url = email_tokens::issue(
            &s,
            &mut *s.pool.acquire().await?,
            id,
            Purpose::PasswordReset,
        )
        .await?;
        let chinese = crate::prefers_chinese(&headers);
        mailer(email::password_reset(
            chinese,
            &s.default_domain,
            &address,
            &url,
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/password-resets/use`: gives the link's User the new password, ends
/// all their sessions and signs them in. Uses up every reset link of theirs. Needs
/// this host's `Origin`, like signing in.
pub(crate) async fn reset_password(
    State(s): State<AppState>,
    _: SameOrigin,
    body: Result<Json<ResetPassword>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(req) = body?;
    // Hashed before taking the link's row lock, which it would hold up otherwise.
    let hash = users::hash_password(&req.password).await?;
    let mut tx = s.pool.begin().await?;
    let id = email_tokens::redeem(&mut tx, Purpose::PasswordReset, &req.token, (s.clock)())
        .await?
        .ok_or(ApiError::ResetLinkExpired)?;
    // A disabled User's link is refused; dropping `tx` puts it back.
    let (email, instance_admin): (String, bool) =
        sqlx::query_as("SELECT email, instance_admin FROM users WHERE id = $1 AND NOT disabled")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ApiError::ResetLinkExpired)?;
    email_tokens::redeem_all(&mut tx, id, Purpose::PasswordReset).await?;
    users::set_password(&mut tx, id, hash, None).await?;
    // The link reached them by email, so the email is theirs.
    sqlx::query(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, $2) WHERE id = $1",
    )
    .bind(id)
    .bind((s.clock)())
    .execute(&mut *tx)
    .await?;
    let user = User {
        email,
        instance_admin,
        has_password: true,
        email_verified: true,
    };
    let res = users::start_session(&s, &mut tx, id, user).await?;
    tx.commit().await?;
    Ok(res)
}
