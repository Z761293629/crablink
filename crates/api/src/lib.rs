//! Request and response DTOs shared by `crablink-server` and the `crablink` CLI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

/// Body of `POST /api/workspaces/{slug}/links`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLink {
    pub target: String,
    /// Custom Code; a Generated Code is used when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Expiry; a past value means "expired now".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

/// Body of `PATCH /api/workspaces/{slug}/links/{code}`. An absent field is left unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `None`: leave the Expiry unchanged (field absent). `Some(None)`: clear it
    /// (`null`). `Some(Some(t))`: set it.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

/// Wraps any present value (including `null`) in `Some`; `default` covers absence.
fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// A Link as returned by the management API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub code: String,
    /// The Domain the Link lives on, e.g. `crab.link`.
    pub domain: String,
    /// The Domain joined with the Code.
    pub short_url: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub click_count: i64,
}

impl Link {
    /// Whether the Link's Expiry has passed at `now`: its Short URL no longer redirects.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|t| t <= now)
    }
}

/// Body of `POST /api/session`: signing in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignIn {
    /// Matched ignoring case.
    pub email: String,
    pub password: String,
}

/// Body of `POST /api/me/password`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePassword {
    pub current_password: String,
    /// At least 8 characters.
    pub new_password: String,
}

/// The signed-in User, as returned by sign-in and `GET /api/me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// As the User was created, case included.
    pub email: String,
    pub instance_admin: bool,
}

/// A Workspace: the owner of Links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique; names the Workspace in API paths: `/api/workspaces/{slug}/…`.
    pub slug: String,
    pub name: String,
}

/// A User's role in a Workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Manages the Workspace's members, API Keys and settings, as well as its Links.
    Owner,
    /// Manages the Workspace's Links only.
    Member,
}

/// A Workspace the signed-in User belongs to, as listed by `GET /api/workspaces`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    #[serde(flatten)]
    pub workspace: Workspace,
    pub role: Role,
}

/// Body of `POST /api/workspaces`: its creator becomes its Owner, and its slug
/// comes from the name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspace {
    /// 1-100 characters.
    pub name: String,
}

/// Body of `PATCH /api/workspaces/{slug}`: renames it; the slug stays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkspace {
    /// 1-100 characters.
    pub name: String,
}

/// A member of a Workspace, as listed by `GET /api/workspaces/{slug}/members`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Addresses the member in `/api/workspaces/{slug}/members/{email}`.
    pub email: String,
    pub role: Role,
}

/// Body of `POST /api/workspaces/{slug}/members`: adds an existing User as a Member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMember {
    pub email: String,
}

/// Body of `PATCH /api/workspaces/{slug}/members/{email}`: promotes or demotes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMember {
    pub role: Role,
}

/// Response of `GET /api/me`: who the credential acts as.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Me {
    /// A session cookie: `{ "user": { … } }`.
    User(User),
    /// An API Key: `{ "workspace": { … } }`, the Workspace it belongs to.
    Workspace(Workspace),
}

/// Body of `POST /api/workspaces/{slug}/api-keys`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKey {
    /// What the key is for, e.g. `CI`; 1-100 characters.
    pub name: String,
}

/// An API Key as listed; never its secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Addresses the key in `DELETE /api/workspaces/{slug}/api-keys/{id}`.
    pub id: i64,
    pub name: String,
    /// The secret's first characters, to tell keys apart.
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Response of `POST /api/workspaces/{slug}/api-keys`: the only time the secret is shown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewApiKey {
    #[serde(flatten)]
    pub key: ApiKey,
    /// What a program sends as `Authorization: Bearer <secret>`.
    pub secret: String,
}

/// Body of every management API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable and machine-readable: branch on this, not on `error`. `Unknown` when
    /// absent, as from a 0.2 server.
    #[serde(default)]
    pub kind: ErrorKind,
    /// English message for people.
    pub error: String,
}

/// What went wrong, as `ErrorBody::kind`. Named `kind`, never `code`, so it
/// can't be read as a Link's Code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// No credential, an unknown or revoked API Key (such as a 0.2 token), or no live
    /// session: never signed in, signed out or expired.
    Unauthorized,
    /// A valid credential that may not do this: another Workspace's, an API Key where
    /// only a User will do, or a Member where only an Owner will do.
    Forbidden,
    /// Sign-in with an unknown email, a wrong password or a disabled User; `error`
    /// never says which.
    InvalidCredentials,
    /// A state-changing request with a session cookie whose `Origin` is missing or
    /// isn't this host.
    OriginMismatch,
    /// Changing password with a wrong current password.
    WrongPassword,
    /// An email that doesn't look like `name@host`.
    InvalidEmail,
    /// A new password shorter than 8 characters.
    PasswordTooShort,
    /// Another User already has this email, ignoring case.
    EmailTaken,
    /// No such API route, or no such API Key.
    NotFound,
    /// No live Link has this Code.
    LinkNotFound,
    /// The Link's Expiry has passed, and only a live Link will do (e.g. its QR code).
    LinkExpired,
    /// Malformed body or query.
    BadRequest,
    /// A Target that isn't an absolute `http(s)` URL of at most 2048 characters, or
    /// that points at the service's own host; `error` says which.
    InvalidTarget,
    /// A Custom Code that isn't `[A-Za-z0-9_-]{3,64}`.
    InvalidCode,
    /// A Custom Code that would shadow one of the service's own routes.
    CodeReserved,
    /// The Code is taken on its Domain, possibly by a Deleted Link.
    CodeTaken,
    /// No User has this email; on a member route, none in this Workspace.
    UserNotFound,
    /// Adding a User who is already in the Workspace.
    AlreadyMember,
    /// Removing or demoting the Workspace's last Owner, themself included: it would
    /// leave no one to manage it.
    LastOwner,
    /// The server failed; details are in its log, never in `error`. Includes running out
    /// of free Generated Codes, where retrying may help.
    Internal,
    /// A kind this build doesn't know, sent by a newer server, or none at all, from a
    /// 0.2 server. Never sent.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Query of `GET /api/workspaces/{slug}/links`. `limit` defaults to 50 and must be 1-200.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Opaque: pass back the previous page's `next_cursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Response of `GET /api/workspaces/{slug}/links`: newest first; `next_cursor` is `None` on the last page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPage {
    pub links: Vec<Link>,
    pub next_cursor: Option<String>,
}

/// Query of `GET /api/workspaces/{slug}/links/{code}/qr`: SVG when `format` is absent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QrQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<QrFormat>,
}

/// The file format of a Link's QR code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QrFormat {
    Svg,
    Png,
}

impl QrFormat {
    /// The value of `format` in the query string.
    pub fn as_str(self) -> &'static str {
        match self {
            QrFormat::Svg => "svg",
            QrFormat::Png => "png",
        }
    }

    /// The `Content-Type` the server sends a QR code of this format with.
    pub fn content_type(self) -> &'static str {
        match self {
            QrFormat::Svg => "image/svg+xml",
            QrFormat::Png => "image/png",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn update_link_expiry_is_three_state() {
        let t: DateTime<Utc> = "2030-01-01T00:00:00Z".parse().unwrap();
        for (dto, wire) in [
            (None, json!({})),
            (Some(None), json!({ "expires_at": null })),
            (
                Some(Some(t)),
                json!({ "expires_at": "2030-01-01T00:00:00Z" }),
            ),
        ] {
            let req = UpdateLink {
                target: None,
                expires_at: dto,
            };
            assert_eq!(serde_json::to_value(&req).unwrap(), wire);
            let back: UpdateLink = serde_json::from_value(wire).unwrap();
            assert_eq!(back.expires_at, dto);
        }
    }

    #[test]
    fn a_link_is_expired_from_its_expiry_on() {
        let t: DateTime<Utc> = "2030-01-01T00:00:00Z".parse().unwrap();
        let link = |expires_at| Link {
            code: "abc".into(),
            domain: "crab.link".into(),
            short_url: "https://crab.link/abc".into(),
            target: "https://example.com".into(),
            created_at: t,
            updated_at: t,
            expires_at,
            click_count: 0,
        };
        let second = chrono::TimeDelta::seconds(1);
        assert!(!link(None).is_expired(t));
        assert!(!link(Some(t)).is_expired(t - second));
        assert!(link(Some(t)).is_expired(t));
        assert!(link(Some(t)).is_expired(t + second));
    }

    #[test]
    fn error_body_carries_a_snake_case_kind() {
        let body = ErrorBody {
            kind: ErrorKind::LinkNotFound,
            error: "link not found".into(),
        };
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            json!({ "kind": "link_not_found", "error": "link not found" })
        );
    }

    #[test]
    fn a_kind_from_a_newer_server_still_yields_the_message() {
        let body: ErrorBody =
            serde_json::from_value(json!({ "kind": "domain_pending", "error": "pending" }))
                .unwrap();
        assert_eq!(body.kind, ErrorKind::Unknown);
        assert_eq!(body.error, "pending");
    }

    #[test]
    fn a_body_from_a_0_2_server_without_a_kind_still_yields_the_message() {
        let body: ErrorBody = serde_json::from_value(json!({ "error": "link not found" })).unwrap();
        assert_eq!(body.kind, ErrorKind::Unknown);
        assert_eq!(body.error, "link not found");
    }
}
