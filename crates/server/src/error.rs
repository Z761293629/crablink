//! `ApiError`: every management API error and its JSON response.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use crablink_api::ErrorKind;

/// A management API error: JSON `{ "kind": "...", "error": "..." }`, where
/// `error` is the `Display` message.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// A Bearer value that is no live API Key, such as a 0.2 token.
    #[error(
        "unknown or revoked API Key (0.2 tokens no longer work): sign in again with an API Key, e.g. `crablink login --server <url>`"
    )]
    InvalidApiKey,
    #[error("not signed in, or the session has expired")]
    NotSignedIn,
    #[error("wrong email or password")]
    InvalidCredentials,
    /// Another Workspace's credential, an API Key where only a User will do, or a
    /// Member where only an Owner will do.
    #[error("not allowed for this credential")]
    Forbidden,
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
    #[error("no such invite")]
    InviteNotFound,
    #[error("invite has expired")]
    InviteExpired,
    #[error("invite has already been used")]
    InviteUsed,
    #[error("password reset link has expired or was used: ask for a new one")]
    ResetLinkExpired,
    /// A Code with no live Link (missing or deleted).
    #[error("link not found")]
    LinkNotFound,
    /// A Link whose Expiry has passed, where only a live one will do.
    #[error("link has expired")]
    LinkExpired,
    #[error("link was blocked for abuse by an Instance Admin: it can only be deleted")]
    LinkBlocked,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    InvalidTarget(&'static str),
    #[error("target is known to be malicious (Google Safe Browsing)")]
    TargetUnsafe,
    #[error("code must be 3-64 characters of A-Z, a-z, 0-9, _ or -")]
    InvalidCode,
    #[error("code {0:?} is reserved")]
    CodeReserved(String),
    #[error("code already taken")]
    CodeTaken,
    #[error("tag must be 1-64 characters with no comma or control character")]
    InvalidTag,
    #[error("no such User")]
    UserNotFound,
    #[error("already in this Workspace")]
    AlreadyMember,
    #[error("a Workspace needs an Owner: make someone else Owner first")]
    LastOwner,
    #[error("the instance needs an Instance Admin: make someone else Instance Admin first")]
    LastInstanceAdmin,
    #[error("domain must be a host name such as go.example.com, without a port")]
    InvalidDomain,
    #[error("domain is the Default Domain or another Workspace's")]
    DomainTaken,
    #[error("domain not found")]
    DomainNotFound,
    #[error("domain is neither the Default Domain nor an approved Domain of this Workspace")]
    DomainNotApproved,
    /// Over a rate limit; retry after this many seconds.
    #[error("too many requests: try again in {0} seconds")]
    RateLimited(u64),
    /// Details are logged, never sent.
    #[error("signing in through the Identity Provider failed")]
    IdentityProviderFailed,
    #[error("the Identity Provider hasn't verified this email")]
    IdentityProviderEmailUnverified,
    #[error("no User has this email: ask an Instance Admin for an Invite")]
    InviteNeeded,
    #[error("this User is disabled")]
    UserDisabled,
    #[error("could not generate a free code, try again")]
    CodesExhausted,
    /// Details are logged, never sent.
    #[error("internal error")]
    Internal,
}

impl ApiError {
    pub(crate) fn kind(&self) -> ErrorKind {
        match self {
            Self::InvalidApiKey | Self::NotSignedIn => ErrorKind::Unauthorized,
            Self::Forbidden => ErrorKind::Forbidden,
            Self::InvalidCredentials => ErrorKind::InvalidCredentials,
            Self::OriginMismatch => ErrorKind::OriginMismatch,
            Self::WrongPassword => ErrorKind::WrongPassword,
            Self::InvalidEmail => ErrorKind::InvalidEmail,
            Self::PasswordTooShort => ErrorKind::PasswordTooShort,
            Self::EmailTaken => ErrorKind::EmailTaken,
            Self::NotFound => ErrorKind::NotFound,
            Self::InviteNotFound => ErrorKind::InviteNotFound,
            Self::InviteExpired => ErrorKind::InviteExpired,
            Self::InviteUsed => ErrorKind::InviteUsed,
            Self::ResetLinkExpired => ErrorKind::ResetLinkExpired,
            Self::LinkNotFound => ErrorKind::LinkNotFound,
            Self::LinkExpired => ErrorKind::LinkExpired,
            Self::LinkBlocked => ErrorKind::LinkBlocked,
            Self::BadRequest(_) => ErrorKind::BadRequest,
            Self::InvalidTarget(_) => ErrorKind::InvalidTarget,
            Self::TargetUnsafe => ErrorKind::TargetUnsafe,
            Self::InvalidCode => ErrorKind::InvalidCode,
            Self::CodeReserved(_) => ErrorKind::CodeReserved,
            Self::CodeTaken => ErrorKind::CodeTaken,
            Self::InvalidTag => ErrorKind::InvalidTag,
            Self::UserNotFound => ErrorKind::UserNotFound,
            Self::AlreadyMember => ErrorKind::AlreadyMember,
            Self::LastOwner => ErrorKind::LastOwner,
            Self::LastInstanceAdmin => ErrorKind::LastInstanceAdmin,
            Self::InvalidDomain => ErrorKind::InvalidDomain,
            Self::DomainTaken => ErrorKind::DomainTaken,
            Self::DomainNotFound => ErrorKind::DomainNotFound,
            Self::DomainNotApproved => ErrorKind::DomainNotApproved,
            Self::RateLimited(_) => ErrorKind::RateLimited,
            Self::IdentityProviderFailed => ErrorKind::IdentityProviderFailed,
            Self::IdentityProviderEmailUnverified => ErrorKind::IdentityProviderEmailUnverified,
            Self::InviteNeeded => ErrorKind::InviteNeeded,
            Self::UserDisabled => ErrorKind::UserDisabled,
            Self::CodesExhausted | Self::Internal => ErrorKind::Internal,
        }
    }

    pub(crate) fn status(&self) -> StatusCode {
        match self {
            Self::InvalidApiKey
            | Self::NotSignedIn
            | Self::InvalidCredentials
            | Self::IdentityProviderFailed => StatusCode::UNAUTHORIZED,
            Self::Forbidden
            | Self::OriginMismatch
            | Self::WrongPassword
            | Self::LinkBlocked
            | Self::IdentityProviderEmailUnverified
            | Self::InviteNeeded
            | Self::UserDisabled => StatusCode::FORBIDDEN,
            Self::NotFound
            | Self::LinkNotFound
            | Self::InviteNotFound
            | Self::UserNotFound
            | Self::DomainNotFound => StatusCode::NOT_FOUND,
            Self::LinkExpired | Self::InviteExpired | Self::InviteUsed | Self::ResetLinkExpired => {
                StatusCode::GONE
            }
            Self::BadRequest(_)
            | Self::InvalidTarget(_)
            | Self::TargetUnsafe
            | Self::InvalidCode
            | Self::CodeReserved(_)
            | Self::InvalidTag
            | Self::InvalidEmail
            | Self::PasswordTooShort
            | Self::InvalidDomain
            | Self::DomainNotApproved => StatusCode::BAD_REQUEST,
            Self::CodeTaken
            | Self::EmailTaken
            | Self::AlreadyMember
            | Self::LastOwner
            | Self::LastInstanceAdmin
            | Self::DomainTaken => StatusCode::CONFLICT,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::CodesExhausted | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl ApiError {
    /// The JSON body: `kind` and the English message.
    pub(crate) fn body(&self) -> crablink_api::ErrorBody {
        crablink_api::ErrorBody {
            kind: self.kind(),
            error: self.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut res = (self.status(), Json(self.body())).into_response();
        if let Self::RateLimited(seconds) = self {
            res.headers_mut()
                .insert(header::RETRY_AFTER, seconds.into());
        }
        res
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
