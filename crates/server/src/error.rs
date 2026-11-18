//! `ApiError`: every management API error and its JSON response.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use crablink_api::ErrorKind;

/// A management API error: JSON `{ "kind": "...", "error": "..." }`, where
/// `error` is the `Display` message.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("missing or invalid token")]
    Unauthorized,
    #[error("not signed in, or the session has expired")]
    NotSignedIn,
    #[error("wrong email or password")]
    InvalidCredentials,
    #[error("Origin header must match the host")]
    OriginMismatch,
    #[error("current password is wrong")]
    WrongPassword,
    #[error("email must look like name@host")]
    InvalidEmail,
    #[error("password must be at least 8 characters")]
    PasswordTooShort,
    #[error("email already taken")]
    EmailTaken,
    #[error("not found")]
    NotFound,
    /// A Code with no live Link (missing or deleted).
    #[error("link not found")]
    LinkNotFound,
    /// A Link whose Expiry has passed, where only a live one will do.
    #[error("link has expired")]
    LinkExpired,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    InvalidTarget(&'static str),
    #[error("code must be 3-64 characters of A-Z, a-z, 0-9, _ or -")]
    InvalidCode,
    #[error("code {0:?} is reserved")]
    CodeReserved(String),
    #[error("code already taken")]
    CodeTaken,
    #[error("could not generate a free code, try again")]
    CodesExhausted,
    /// Details are logged, never sent.
    #[error("internal error")]
    Internal,
}

impl ApiError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Unauthorized | Self::NotSignedIn => ErrorKind::Unauthorized,
            Self::InvalidCredentials => ErrorKind::InvalidCredentials,
            Self::OriginMismatch => ErrorKind::OriginMismatch,
            Self::WrongPassword => ErrorKind::WrongPassword,
            Self::InvalidEmail => ErrorKind::InvalidEmail,
            Self::PasswordTooShort => ErrorKind::PasswordTooShort,
            Self::EmailTaken => ErrorKind::EmailTaken,
            Self::NotFound => ErrorKind::NotFound,
            Self::LinkNotFound => ErrorKind::LinkNotFound,
            Self::LinkExpired => ErrorKind::LinkExpired,
            Self::BadRequest(_) => ErrorKind::BadRequest,
            Self::InvalidTarget(_) => ErrorKind::InvalidTarget,
            Self::InvalidCode => ErrorKind::InvalidCode,
            Self::CodeReserved(_) => ErrorKind::CodeReserved,
            Self::CodeTaken => ErrorKind::CodeTaken,
            Self::CodesExhausted | Self::Internal => ErrorKind::Internal,
        }
    }

    pub(crate) fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized | Self::NotSignedIn | Self::InvalidCredentials => {
                StatusCode::UNAUTHORIZED
            }
            Self::OriginMismatch | Self::WrongPassword => StatusCode::FORBIDDEN,
            Self::NotFound | Self::LinkNotFound => StatusCode::NOT_FOUND,
            Self::LinkExpired => StatusCode::GONE,
            Self::BadRequest(_)
            | Self::InvalidTarget(_)
            | Self::InvalidCode
            | Self::CodeReserved(_)
            | Self::InvalidEmail
            | Self::PasswordTooShort => StatusCode::BAD_REQUEST,
            Self::CodeTaken | Self::EmailTaken => StatusCode::CONFLICT,
            Self::CodesExhausted | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = crablink_api::ErrorBody {
            kind: self.kind(),
            error: self.to_string(),
        };
        (self.status(), Json(body)).into_response()
    }
}

/// A body that doesn't match the DTO (e.g. a malformed `expires_at`) is a 400, not axum's 422.
impl From<JsonRejection> for ApiError {
    fn from(e: JsonRejection) -> Self {
        ApiError::BadRequest(e.body_text())
    }
}

/// A malformed query string (e.g. `limit=x`) is a JSON 400 like any other API error.
impl From<QueryRejection> for ApiError {
    fn from(e: QueryRejection) -> Self {
        ApiError::BadRequest(e.body_text())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        eprintln!("database error: {e}");
        ApiError::Internal
    }
}
