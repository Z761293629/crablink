//! One-time links the instance emails to a User (`email_tokens`): stored hashed
//! like Session tokens, expiring, and used up by their first use.

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgConnection;

use crate::AppState;
use crate::error::ApiError;
use crate::users;

#[derive(Clone, Copy)]
pub(crate) enum Purpose {
    PasswordReset,
    EmailVerification,
}

impl Purpose {
    /// `email_tokens.purpose`.
    fn stored(self) -> &'static str {
        match self {
            Purpose::PasswordReset => "password_reset",
            Purpose::EmailVerification => "email_verification",
        }
    }

    fn ttl(self) -> TimeDelta {
        match self {
            Purpose::PasswordReset => TimeDelta::hours(1),
            Purpose::EmailVerification => TimeDelta::hours(24),
        }
    }

    /// The web UI page that uses the link.
    fn page(self) -> &'static str {
        match self {
            Purpose::PasswordReset => "reset-password",
            Purpose::EmailVerification => "verify-email",
        }
    }
}

/// The web UI's link to `page` carrying `token`.
pub(crate) fn link(s: &AppState, page: &str, token: &str) -> String {
    // The token rides in the fragment, which browsers never send to a server.
    format!("{}://{}/app/{page}#{token}", s.scheme, s.default_domain)
}

/// A new link of `purpose` for User `user_id`, to email them.
pub(crate) async fn issue(
    s: &AppState,
    conn: &mut PgConnection,
    user_id: i64,
    purpose: Purpose,
) -> Result<String, ApiError> {
    let token = users::random_token();
    sqlx::query(
        "INSERT INTO email_tokens (token_hash, user_id, purpose, expires_at) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(users::token_hash(&token))
    .bind(user_id)
    .bind(purpose.stored())
    .bind((s.clock)() + purpose.ttl())
    .execute(conn)
    .await?;
    Ok(link(s, purpose.page(), &token))
}

/// Uses up a live link of `purpose`: its User's id, or `None` if `token` is unknown,
/// used or expired.
pub(crate) async fn redeem(
    conn: &mut PgConnection,
    purpose: Purpose,
    token: &str,
    now: DateTime<Utc>,
) -> Result<Option<i64>, ApiError> {
    // Deleting it locks the row: a second use of the same link waits, then finds nothing.
    Ok(sqlx::query_scalar(
        "DELETE FROM email_tokens WHERE token_hash = $1 AND purpose = $2 AND expires_at > $3 \
         RETURNING user_id",
    )
    .bind(users::token_hash(token))
    .bind(purpose.stored())
    .bind(now)
    .fetch_optional(conn)
    .await?)
}

/// Uses up every link of `purpose` User `user_id` has.
pub(crate) async fn redeem_all(
    conn: &mut PgConnection,
    user_id: i64,
    purpose: Purpose,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM email_tokens WHERE user_id = $1 AND purpose = $2")
        .bind(user_id)
        .bind(purpose.stored())
        .execute(conn)
        .await?;
    Ok(())
}
