//! Users and their sessions: creating Users (and so the first Instance Admin),
//! signing in (creating a personal Workspace the first time) and out, `/api/me`
//! and changing password.

use std::sync::LazyLock;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::TimeDelta;
use crablink_api::{ChangePassword, Me, SignIn, User, Workspace};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};

use crate::AppState;
use crate::error::ApiError;
use crate::rate_limit::VisitorIp;
use crate::workspaces::{self, Caller};

const COOKIE: &str = "crablink_session";
const SESSION_TTL: TimeDelta = TimeDelta::days(30);
const MIN_PASSWORD_LEN: usize = 8;

/// Creates a User who signs in with `email` and `password`, as an Instance Admin
/// when `instance_admin`. This is what `crablink-server admin create` runs.
pub async fn create_user(
    pool: &PgPool,
    email: &str,
    password: &str,
    instance_admin: bool,
) -> Result<(), ApiError> {
    valid_email(email)?;
    let hash = hash_password(password).await?;
    insert_user(
        &mut *pool.acquire().await?,
        email,
        Some(&hash),
        instance_admin,
    )
    .await
    .map(drop)
}

/// Inserts a User whose password hashes to `password_hash` (from `hash_password`,
/// done beforehand so no lock is held meanwhile), or with no password, who signs
/// in only through Identity Providers; returns their id.
pub(crate) async fn insert_user(
    conn: &mut PgConnection,
    email: &str,
    password_hash: Option<&str>,
    instance_admin: bool,
) -> Result<i64, ApiError> {
    let email = valid_email(email)?;
    let res = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, instance_admin) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(email)
    .bind(password_hash)
    .bind(instance_admin)
    .fetch_one(conn)
    .await;
    match res {
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(ApiError::EmailTaken),
        res => res.map_err(Into::into),
    }
}

/// `email` trimmed, if it looks like `name@host`.
pub(crate) fn valid_email(email: &str) -> Result<&str, ApiError> {
    let email = email.trim();
    let valid = email
        .split_once('@')
        .is_some_and(|(name, host)| !name.is_empty() && !host.is_empty() && !host.contains('@'));
    if !valid || email.len() > 254 || email.chars().any(char::is_whitespace) {
        return Err(ApiError::InvalidEmail);
    }
    Ok(email)
}

/// Disables or re-enables the User with `email` (ignoring case), as `set_disabled`.
pub async fn set_user_disabled(pool: &PgPool, email: &str, disabled: bool) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    let id = sqlx::query_scalar(
        "UPDATE users SET disabled = $2 WHERE lower(email) = lower($1) RETURNING id",
    )
    .bind(email.trim())
    .bind(disabled)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::UserNotFound)?;
    if disabled {
        end_sessions(&mut tx, id, None).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Disables or re-enables User `id`. A disabled User can't sign in, and their
/// sessions end, so re-enabling them doesn't bring those back.
pub(crate) async fn set_disabled(
    conn: &mut PgConnection,
    id: i64,
    disabled: bool,
) -> Result<(), ApiError> {
    let res = sqlx::query("UPDATE users SET disabled = $2 WHERE id = $1")
        .bind(id)
        .bind(disabled)
        .execute(&mut *conn)
        .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::UserNotFound);
    }
    if disabled {
        end_sessions(conn, id, None).await?;
    }
    Ok(())
}

/// Gives User `id` the password hashed as `hash`, ending all their sessions but
/// `keep` (a session's token hash).
pub(crate) async fn set_password(
    conn: &mut PgConnection,
    id: i64,
    hash: String,
    keep: Option<&[u8]>,
) -> Result<(), ApiError> {
    let res = sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(hash)
        .execute(&mut *conn)
        .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::UserNotFound);
    }
    end_sessions(conn, id, keep).await
}

/// Ends every session of User `id` but `keep` (a session's token hash).
async fn end_sessions(
    conn: &mut PgConnection,
    id: i64,
    keep: Option<&[u8]>,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND token_hash IS DISTINCT FROM $2")
        .bind(id)
        .bind(keep)
        .execute(conn)
        .await?;
    Ok(())
}

/// argon2id with a random salt, off the async threads: it is slow on purpose.
pub(crate) async fn hash_password(password: &str) -> Result<String, ApiError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiError::PasswordTooShort);
    }
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
    })
    .await
    .map_err(internal)?
    .map_err(internal)
}

/// What `verify_password` checks when there is no User, so a wrong email takes as
/// long as a wrong password. `app` forces it, so no sign-in pays for hashing it.
pub(crate) static DUMMY: LazyLock<String> = LazyLock::new(|| {
    Argon2::default()
        .hash_password(b"not anyone's password")
        .expect("hashing a constant password with default argon2 parameters")
        .to_string()
});

/// Whether `password` matches `hash`, or `DUMMY` with no `hash` (no such User).
async fn verify_password(password: String, hash: Option<String>) -> Result<bool, ApiError> {
    tokio::task::spawn_blocking(move || {
        let hash = hash.as_deref().unwrap_or(&DUMMY);
        PasswordHash::new(hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .map_err(internal)
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    eprintln!("password hashing error: {e}");
    ApiError::Internal
}

/// SHA-256 of a session token or API Key secret: both are random enough that a
/// slow password hash would buy nothing.
pub(crate) fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// 32 random bytes as 64 hex characters.
pub(crate) fn random_token() -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for b in rand::random::<[u8; 32]>() {
        write!(hex, "{b:02x}").expect("writing to a String never fails");
    }
    hex
}

fn session_cookie(token: &str, max_age: i64) -> String {
    format!("{COOKIE}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}")
}

#[derive(sqlx::FromRow)]
struct SignInRow {
    id: i64,
    email: String,
    instance_admin: bool,
    password_hash: Option<String>,
}

/// `POST /api/session`: signs in, setting the session cookie; returns the User.
/// The first sign-in also creates the User's personal Workspace. Needs this host's
/// `Origin`, so another site can't sign a browser in as someone else.
pub(crate) async fn sign_in(
    State(s): State<AppState>,
    _: SameOrigin,
    VisitorIp(ip): VisitorIp,
    body: Result<Json<SignIn>, JsonRejection>,
) -> Result<Response, ApiError> {
    s.limiters.sign_in.hit(ip, (s.clock)())?;
    let Json(req) = body?;
    let row: Option<SignInRow> = sqlx::query_as(
        "SELECT id, email, instance_admin, password_hash FROM users \
         WHERE lower(email) = lower($1) AND NOT disabled",
    )
    .bind(req.email.trim())
    .fetch_optional(&s.pool)
    .await?;
    // A User without a password fails here exactly like a wrong password.
    let hash = row.as_ref().and_then(|r| r.password_hash.clone());
    let (Some(row), true) = (row, verify_password(req.password, hash).await?) else {
        return Err(ApiError::InvalidCredentials);
    };
    let mut tx = s.pool.begin().await?;
    let user = User {
        email: row.email,
        instance_admin: row.instance_admin,
        has_password: true,
    };
    let res = start_session(&s, &mut tx, row.id, user).await?;
    tx.commit().await?;
    Ok(res)
}

/// Signs in User `id` on `conn`: creates their personal Workspace the first time,
/// and a session. The response sets its cookie and returns `user`, once `conn`'s
/// transaction commits.
pub(crate) async fn start_session(
    s: &AppState,
    conn: &mut PgConnection,
    id: i64,
    user: User,
) -> Result<Response, ApiError> {
    let cookie = new_session(s, conn, id, &user.email).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(user)).into_response())
}

/// `start_session` for User `id` with `email`, returning only the `Set-Cookie`
/// value of the new session.
pub(crate) async fn new_session(
    s: &AppState,
    conn: &mut PgConnection,
    id: i64,
    email: &str,
) -> Result<String, ApiError> {
    let first = sqlx::query("UPDATE users SET signed_in = TRUE WHERE id = $1 AND NOT signed_in")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if first.rows_affected() == 1 {
        workspaces::create_personal(conn, id, email).await?;
    }
    let token = random_token();
    // ponytail: expired sessions are never swept; add a cleanup if the table grows.
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(token_hash(&token))
        .bind(id)
        .bind((s.clock)() + SESSION_TTL)
        .execute(conn)
        .await?;
    Ok(session_cookie(&token, SESSION_TTL.num_seconds()))
}

/// `DELETE /api/session`: signs out, ending the session and clearing the cookie.
pub(crate) async fn sign_out(
    State(s): State<AppState>,
    session: Session,
) -> Result<Response, ApiError> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(&session.token_hash)
        .execute(&s.pool)
        .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, session_cookie("", 0))],
    )
        .into_response())
}

/// `GET /api/me`: the signed-in User, or the API Key's Workspace.
pub(crate) async fn me(State(s): State<AppState>, caller: Caller) -> Result<Json<Me>, ApiError> {
    Ok(Json(match caller {
        Caller::User(session) => Me::User(session.user),
        Caller::ApiKey { workspace_id } => {
            let (slug, name) = sqlx::query_as("SELECT slug, name FROM workspaces WHERE id = $1")
                .bind(workspace_id)
                .fetch_one(&s.pool)
                .await?;
            Me::Workspace(Workspace { slug, name })
        }
    }))
}

/// `POST /api/me/password`: needs the current password, so a User without one
/// can't; ends the User's other sessions.
pub(crate) async fn change_password(
    State(s): State<AppState>,
    session: Session,
    body: Result<Json<ChangePassword>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(req) = body?;
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(session.user_id)
        .fetch_one(&s.pool)
        .await?;
    if !verify_password(req.current_password, hash).await? {
        return Err(ApiError::WrongPassword);
    }
    let hash = hash_password(&req.new_password).await?;
    let mut tx = s.pool.begin().await?;
    set_password(&mut tx, session.user_id, hash, Some(&session.token_hash)).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A live session of an enabled User, from the session cookie. A state-changing
/// request must also come from this host's `Origin`, which is what stops CSRF.
pub(crate) struct Session {
    pub(crate) user_id: i64,
    token_hash: Vec<u8>,
    pub(crate) user: User,
}

impl FromRequestParts<AppState> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, s: &AppState) -> Result<Self, ApiError> {
        let token = cookie(&parts.headers, COOKIE).ok_or(ApiError::NotSignedIn)?;
        let safe = matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS);
        if !safe && !same_origin(parts) {
            return Err(ApiError::OriginMismatch);
        }
        let token_hash = token_hash(token);
        let (user_id, email, instance_admin, has_password) = sqlx::query_as(
            "SELECT u.id, u.email, u.instance_admin, u.password_hash IS NOT NULL FROM sessions s \
             JOIN users u ON u.id = s.user_id \
             WHERE s.token_hash = $1 AND s.expires_at > $2 AND NOT u.disabled",
        )
        .bind(&token_hash)
        .bind((s.clock)())
        .fetch_optional(&s.pool)
        .await?
        .ok_or(ApiError::NotSignedIn)?;
        Ok(Session {
            user_id,
            token_hash,
            user: User {
                email,
                instance_admin,
                has_password,
            },
        })
    }
}

/// The non-empty value of the cookie named `name` that the request sends.
pub(crate) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| c.trim().strip_prefix(name)?.strip_prefix('='))
        .filter(|v| !v.is_empty())
}

/// A request whose `Origin` header names the host it was sent to: what a request
/// that signs someone in, or changes state with a session, needs against CSRF.
pub(crate) struct SameOrigin;

impl<S: Sync> FromRequestParts<S> for SameOrigin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        same_origin(parts)
            .then_some(SameOrigin)
            .ok_or(ApiError::OriginMismatch)
    }
}

/// Whether the `Origin` header names the host the request was sent to.
fn same_origin(parts: &Parts) -> bool {
    let host = crate::request_host(&parts.headers, &parts.uri);
    let origin = parts
        .headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .and_then(|o| url::Url::parse(o).ok());
    let (Some(host), Some(origin)) = (host, origin) else {
        return false;
    };
    let Some(origin_host) = origin.host_str() else {
        return false;
    };
    match origin.port() {
        Some(port) => host.eq_ignore_ascii_case(&format!("{origin_host}:{port}")),
        None => host.eq_ignore_ascii_case(origin_host),
    }
}
