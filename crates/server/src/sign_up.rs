//! Open sign-up, on an instance that turns it on: anyone signs up with an email and
//! a password, and verifies the email through an emailed link before creating Links
//! and API Keys.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use crablink_api::{SignUp, User, VerifyEmail};

use crate::email_tokens::{self, Purpose};
use crate::error::ApiError;
use crate::rate_limit::VisitorIp;
use crate::users::{self, SameOrigin, Session};
use crate::{AppState, email};

/// `POST /api/sign-ups`: creates a User with an unverified email, emails them a
/// verification link in the language the request asks for, and signs them in. Not
/// found unless sign-up is open. Needs this host's `Origin`, like signing in.
pub(crate) async fn sign_up(
    State(s): State<AppState>,
    _: SameOrigin,
    VisitorIp(ip): VisitorIp,
    headers: HeaderMap,
    body: Result<Json<SignUp>, JsonRejection>,
) -> Result<Response, ApiError> {
    // `app` refuses open sign-up without a mailer.
    let Some(mailer) = s.mailer.as_ref().filter(|_| s.instance.sign_up) else {
        return Err(ApiError::NotFound);
    };
    s.limiters.sign_up.hit(ip, (s.clock)())?;
    let Json(req) = body?;
    let email = users::valid_email(&req.email)?.to_owned();
    let hash = users::hash_password(&req.password).await?;
    let mut tx = s.pool.begin().await?;
    let id = users::insert_user(&mut tx, &email, Some(&hash), false, false).await?;
    let url = email_tokens::issue(&s, &mut tx, id, Purpose::EmailVerification).await?;
    let user = User {
        email: email.clone(),
        instance_admin: false,
        has_password: true,
        email_verified: false,
    };
    let res = users::start_session(&s, &mut tx, id, user).await?;
    tx.commit().await?;
    let chinese = crate::prefers_chinese(&headers);
    mailer(email::verification(
        chinese,
        &s.default_domain,
        &email,
        &url,
    ));
    Ok(res)
}

/// `POST /api/email-verifications`: emails the signed-in User a new verification
/// link, unless their email is verified already. Not found on an instance that
/// sends no email.
pub(crate) async fn resend(
    State(s): State<AppState>,
    session: Session,
    VisitorIp(ip): VisitorIp,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let Some(mailer) = &s.mailer else {
        return Err(ApiError::NotFound);
    };
    if session.user.email_verified {
        return Ok(StatusCode::NO_CONTENT);
    }
    // Counted like a sign-up, so nobody floods someone else's inbox with it.
    s.limiters.sign_up.hit(ip, (s.clock)())?;
    let url = email_tokens::issue(
        &s,
        &mut *s.pool.acquire().await?,
        session.user_id,
        Purpose::EmailVerification,
    )
    .await?;
    let chinese = crate::prefers_chinese(&headers);
    mailer(email::verification(
        chinese,
        &s.default_domain,
        &session.user.email,
        &url,
    ));
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/email-verifications/use`: verifies the email of the link's User.
pub(crate) async fn verify(
    State(s): State<AppState>,
    _: SameOrigin,
    body: Result<Json<VerifyEmail>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(req) = body?;
    let mut tx = s.pool.begin().await?;
    let id = email_tokens::redeem(&mut tx, Purpose::EmailVerification, &req.token, (s.clock)())
        .await?
        .ok_or(ApiError::VerificationLinkExpired)?;
    sqlx::query(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, $2) WHERE id = $1",
    )
    .bind(id)
    .bind((s.clock)())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
