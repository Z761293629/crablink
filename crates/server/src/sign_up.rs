//! Open sign-up, on an instance that turns it on: anyone signs up with an email and
//! a password, and verifies the email through an emailed link before creating Links
//! and API Keys.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use chrono::TimeDelta;
use crablink_api::{SignUp, User, VerifyEmail};
use sqlx::PgConnection;

use crate::error::ApiError;
use crate::rate_limit::VisitorIp;
use crate::users::{self, SameOrigin, Session};
use crate::{AppState, email};

const VERIFICATION_TTL: TimeDelta = TimeDelta::hours(24);
/// `email_tokens.purpose` of an email verification link.
const PURPOSE: &str = "email_verification";

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
    let url = verification_link(&s, &mut tx, id).await?;
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
    let url = verification_link(&s, &mut *s.pool.acquire().await?, session.user_id).await?;
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
    // Deleting it locks the row: a second use of the same link waits, then finds nothing.
    let verified: Option<i64> = sqlx::query_scalar(
        "WITH used AS (DELETE FROM email_tokens \
           WHERE token_hash = $1 AND purpose = $2 AND expires_at > $3 RETURNING user_id) \
         UPDATE users u SET email_verified_at = COALESCE(u.email_verified_at, $3) \
         FROM used WHERE u.id = used.user_id RETURNING u.id",
    )
    .bind(users::token_hash(&req.token))
    .bind(PURPOSE)
    .bind((s.clock)())
    .fetch_optional(&s.pool)
    .await?;
    verified.ok_or(ApiError::VerificationLinkExpired)?;
    Ok(StatusCode::NO_CONTENT)
}

/// A new verification link for User `id`.
async fn verification_link(
    s: &AppState,
    conn: &mut PgConnection,
    id: i64,
) -> Result<String, ApiError> {
    let token = users::random_token();
    sqlx::query(
        "INSERT INTO email_tokens (token_hash, user_id, purpose, expires_at) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(users::token_hash(&token))
    .bind(id)
    .bind(PURPOSE)
    .bind((s.clock)() + VERIFICATION_TTL)
    .execute(conn)
    .await?;
    // The token rides in the fragment, which browsers never send to a server.
    Ok(format!(
        "{}://{}/app/verify-email#{token}",
        s.scheme, s.default_domain
    ))
}
