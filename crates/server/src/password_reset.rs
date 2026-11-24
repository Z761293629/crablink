//! Self-service password reset, on an instance that sends email: a User asks for a
//! link by email, and following it sets a new password.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use chrono::TimeDelta;
use crablink_api::{ForgotPassword, ResetPassword, User};

use crate::error::ApiError;
use crate::rate_limit::VisitorIp;
use crate::users::{self, SameOrigin};
use crate::{AppState, email};

const RESET_TTL: TimeDelta = TimeDelta::hours(1);
/// `email_tokens.purpose` of a password reset link.
const PURPOSE: &str = "password_reset";

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
        let token = users::random_token();
        sqlx::query(
            "INSERT INTO email_tokens (token_hash, user_id, purpose, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(users::token_hash(&token))
        .bind(id)
        .bind(PURPOSE)
        .bind((s.clock)() + RESET_TTL)
        .execute(&s.pool)
        .await?;
        // The token rides in the fragment, which browsers never send to a server.
        let url = format!(
            "{}://{}/app/reset-password#{token}",
            s.scheme, s.default_domain
        );
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

#[derive(sqlx::FromRow)]
struct ResetRow {
    id: i64,
    email: String,
    instance_admin: bool,
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
    // Deleting it locks the row: a second use of the same link waits, then finds nothing.
    let row: Option<ResetRow> = sqlx::query_as(
        "DELETE FROM email_tokens t USING users u \
         WHERE t.token_hash = $1 AND t.purpose = $2 AND t.expires_at > $3 \
           AND u.id = t.user_id AND NOT u.disabled \
         RETURNING u.id, u.email, u.instance_admin",
    )
    .bind(users::token_hash(&req.token))
    .bind(PURPOSE)
    .bind((s.clock)())
    .fetch_optional(&mut *tx)
    .await?;
    let ResetRow {
        id,
        email,
        instance_admin,
    } = row.ok_or(ApiError::ResetLinkExpired)?;
    sqlx::query("DELETE FROM email_tokens WHERE user_id = $1 AND purpose = $2")
        .bind(id)
        .bind(PURPOSE)
        .execute(&mut *tx)
        .await?;
    users::set_password(&mut tx, id, hash, None).await?;
    let user = User {
        email,
        instance_admin,
        has_password: true,
    };
    let res = users::start_session(&s, &mut tx, id, user).await?;
    tx.commit().await?;
    Ok(res)
}
