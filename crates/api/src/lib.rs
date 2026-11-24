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
    /// The Domain the Link lives on: the Default Domain when absent, else an approved
    /// Domain of the Workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Tag names; each is created in the Workspace if new.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Query of `GET`, `PATCH` and `DELETE /api/workspaces/{slug}/links/{code}`: the
/// Domain of the Link, the Default Domain when absent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkDomain {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// Body of `POST /api/workspaces/{slug}/domains`: a custom Domain for the Workspace,
/// pending until an Instance Admin approves it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddDomain {
    /// A host name such as `go.example.com`, without a port.
    pub domain: String,
}

/// Whether a custom Domain may be used: only an approved one redirects, takes Links
/// and gets a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainStatus {
    Pending,
    Approved,
    Rejected,
}

/// A Domain a Workspace can see, as listed by `GET /api/workspaces/{slug}/domains`:
/// the Default Domain, then its own custom Domains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    /// A host name, with a port for a Default Domain that has one. Addresses the
    /// Domain in `/api/workspaces/{slug}/domains/{domain}`.
    pub domain: String,
    /// Always `approved` for the Default Domain.
    pub status: DomainStatus,
    /// Whether this is the instance's Default Domain, shared by every Workspace.
    pub default: bool,
}

/// A custom Domain as an Instance Admin sees it, from `GET /api/admin/domains`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedDomain {
    /// Addresses the Domain in `/api/admin/domains/{domain}`.
    pub domain: String,
    pub status: DomainStatus,
    /// The Workspace slug of the Workspace that added it.
    pub workspace_slug: String,
}

/// Body of `PATCH /api/admin/domains/{domain}`: approves or rejects it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDomain {
    pub status: DomainStatus,
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
    /// Replaces all the Link's Tags; `[]` removes them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Wraps any present value (including `null`) in `Some`; `default` covers absence.
fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// A Link as returned by the management API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub code: String,
    /// The Domain the Link lives on, e.g. `crabl.ink`.
    pub domain: String,
    /// The Domain joined with the Code.
    pub short_url: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub click_count: i64,
    /// Its Tags' names, sorted.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether it is a Blocked Link: an Instance Admin stopped it for abuse, so it no
    /// longer redirects and can't be changed, only deleted. `false` from older servers.
    #[serde(default)]
    pub blocked: bool,
}

impl Link {
    /// Whether the Link's Expiry has passed at `now`: its Short URL no longer redirects.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|t| t <= now)
    }
}

/// Body of `POST /api/session`: signing in.
#[derive(Clone, Serialize, Deserialize)]
pub struct SignIn {
    /// Matched ignoring case.
    pub email: String,
    pub password: String,
}

/// Body of `POST /api/me/password`.
#[derive(Clone, Serialize, Deserialize)]
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
    /// Manages who is in the Workspace, its API Keys and settings, as well as its Links.
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

/// A User of a Workspace with their role there, Owner or Member, as listed by
/// `GET /api/workspaces/{slug}/members`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceUser {
    /// Addresses the User in `/api/workspaces/{slug}/members/{email}`.
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
pub struct UpdateWorkspaceUser {
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
#[derive(Clone, Serialize, Deserialize)]
pub struct NewApiKey {
    #[serde(flatten)]
    pub key: ApiKey,
    /// What a program sends as `Authorization: Bearer <secret>`.
    pub secret: String,
}

/// Body of `POST /api/admin/invites`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvite {
    /// The new User's email; no User may have it yet.
    pub email: String,
    /// The slug of a Workspace the new User joins as a Member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// Response of `POST /api/admin/invites`: the only time the invite link is shown.
#[derive(Clone, Serialize, Deserialize)]
pub struct NewInvite {
    /// What the Instance Admin sends the new User; its fragment is the token.
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

/// Body of `POST /api/invites/accept`; the response is the new, signed-in `User`.
#[derive(Clone, Serialize, Deserialize)]
pub struct AcceptInvite {
    /// The invite link's fragment.
    pub token: String,
    /// At least 8 characters.
    pub password: String,
}

/// Body of `POST /api/password-resets`: emails a password reset link to the User
/// with this email, if any. The answer is the same either way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgotPassword {
    pub email: String,
}

/// Body of `POST /api/password-resets/use`; the response is the signed-in `User`.
#[derive(Clone, Serialize, Deserialize)]
pub struct ResetPassword {
    /// The reset link's fragment.
    pub token: String,
    /// The new password, at least 8 characters.
    pub password: String,
}

/// What the instance offers, from `GET /api/instance`, for pages shown before sign-in.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Instance {
    /// Whether Users can reset a forgotten password by email (the instance sends email).
    #[serde(default)]
    pub password_reset: bool,
}

/// A User as an Instance Admin sees them, from `GET /api/admin/users`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedUser {
    /// Addresses the User in `/api/admin/users/{id}`.
    pub id: i64,
    pub email: String,
    pub instance_admin: bool,
    /// A disabled User can't sign in and has no sessions.
    pub disabled: bool,
}

/// Body of `PATCH /api/admin/users/{id}`. An absent field is left unchanged; an
/// Instance Admin can't disable or demote themselves.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateUser {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_admin: Option<bool>,
}

/// Response of `POST /api/admin/users/{id}/password`: the only time the new
/// password is shown.
#[derive(Clone, Serialize, Deserialize)]
pub struct NewPassword {
    pub password: String,
}

/// Body of `POST /api/admin/blocked-links`, and query of `DELETE` on it: the Link to
/// block or unblock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockLink {
    /// e.g. `https://crabl.ink/aZ3k9` or `crabl.ink/aZ3k9`.
    pub short_url: String,
}

/// A Blocked Link as an Instance Admin sees it, from `GET /api/admin/blocked-links`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedLink {
    pub short_url: String,
    pub target: String,
    /// The Workspace slug of its Workspace.
    pub workspace_slug: String,
    pub blocked_at: DateTime<Utc>,
}

/// What `Debug` shows instead of a password, token or secret, so none reaches a log.
const REDACTED: &str = "<redacted>";

impl std::fmt::Debug for SignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignIn")
            .field("email", &self.email)
            .field("password", &REDACTED)
            .finish()
    }
}

impl std::fmt::Debug for ChangePassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangePassword")
            .field("current_password", &REDACTED)
            .field("new_password", &REDACTED)
            .finish()
    }
}

impl std::fmt::Debug for NewApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewApiKey")
            .field("key", &self.key)
            .field("secret", &REDACTED)
            .finish()
    }
}

impl std::fmt::Debug for AcceptInvite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceptInvite")
            .field("token", &REDACTED)
            .field("password", &REDACTED)
            .finish()
    }
}

impl std::fmt::Debug for ResetPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResetPassword")
            .field("token", &REDACTED)
            .field("password", &REDACTED)
            .finish()
    }
}

impl std::fmt::Debug for NewInvite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let page = self
            .url
            .split_once('#')
            .map_or(&*self.url, |(page, _)| page);
        f.debug_struct("NewInvite")
            .field("url", &format_args!("{page}#{REDACTED}"))
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl std::fmt::Debug for NewPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewPassword")
            .field("password", &REDACTED)
            .finish()
    }
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
    /// No such API route or API Key, or no Workspace with the slug an invite names.
    NotFound,
    /// No invite has this token.
    InviteNotFound,
    /// The invite's expiry has passed.
    InviteExpired,
    /// The invite has already been accepted.
    InviteUsed,
    /// A password reset link that is unknown, expired, or used (using one of a User's
    /// reset links uses them all): ask for a new link.
    ResetLinkExpired,
    /// No live Link has this Code.
    LinkNotFound,
    /// The Link's Expiry has passed, and only a live Link will do (e.g. its QR code).
    LinkExpired,
    /// The Link is a Blocked Link: its Target, Tags and Expiry can't change, and it has
    /// no QR code. Only deleting it is allowed.
    LinkBlocked,
    /// Malformed body or query.
    BadRequest,
    /// A Target that isn't an absolute `http(s)` URL of at most 2048 characters, or
    /// that points at the service's own host; `error` says which.
    InvalidTarget,
    /// A Target that Google Safe Browsing knows to be malicious (phishing, malware,
    /// unwanted software), on an instance that checks Targets.
    TargetUnsafe,
    /// A Custom Code that isn't `[A-Za-z0-9_-]{3,64}`.
    InvalidCode,
    /// A Custom Code that would shadow one of the service's own routes, or a
    /// Reserved Code asked for on the Default Domain.
    CodeReserved,
    /// The Code is taken on its Domain, possibly by a Deleted Link.
    CodeTaken,
    /// A Tag name that is empty, longer than 64 characters, or has a comma or a
    /// control character.
    InvalidTag,
    /// No User has this email or id; on a `/members` route, none in this Workspace.
    UserNotFound,
    /// Adding a User who is already in the Workspace.
    AlreadyMember,
    /// Removing or demoting the Workspace's last Owner, themself included: it would
    /// leave no one to manage it.
    LastOwner,
    /// Disabling or demoting the instance's last enabled Instance Admin, themself
    /// included: it would leave no one to run it.
    LastInstanceAdmin,
    /// A Domain host that isn't a host name such as `go.example.com`.
    InvalidDomain,
    /// The Domain is the Default Domain or another Workspace's.
    DomainTaken,
    /// The Workspace has no custom Domain with this host; for an Instance Admin, no
    /// Workspace has.
    DomainNotFound,
    /// A Link on a Domain that is neither the Default Domain nor an approved Domain of
    /// the Workspace.
    DomainNotApproved,
    /// Over one of the instance's rate limits (sign-in attempts per visitor IP, Links
    /// created per Workspace, …): status 429, and the `Retry-After` header says how
    /// many seconds to wait.
    RateLimited,
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
    /// Only Links with this Tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Only Links whose Code or Target contains this, ignoring case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
}

/// Response of `GET /api/workspaces/{slug}/links`: newest first; `next_cursor` is `None` on the last page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPage {
    pub links: Vec<Link>,
    pub next_cursor: Option<String>,
}

/// One row of the response of `POST /api/workspaces/{slug}/links.csv`: the created
/// Link's Short URL, or why the row was skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedRow {
    /// The row's line in the CSV file, the header being line 1.
    pub line: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<ErrorBody>,
}

/// Query of `GET /api/workspaces/{slug}/links/{code}/qr`: SVG when `format` is absent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QrQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<QrFormat>,
    /// The Domain of the Link, the Default Domain when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
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

/// Query of `GET /api/workspaces/{slug}/links/{code}/clicks`: Clicks at or after
/// `from` and before `to`, counted per `interval` (a day when absent, in UTC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClicksQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<Interval>,
    /// The Domain of the Link, the Default Domain when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// The width of a time bucket of Clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    Hour,
    Day,
}

/// Response of `GET /api/workspaces/{slug}/links/{code}/clicks`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickStats {
    /// Every bucket from the one holding `from` up to `to`, empty ones included.
    pub series: Vec<ClickBucket>,
    /// By Referrer host; `None` for Clicks with no Referrer.
    pub referrers: Vec<ClickBreakdown>,
    /// By ISO 3166-1 alpha-2 country code; `None` where it is unknown.
    pub countries: Vec<ClickBreakdown>,
    /// By `desktop`, `mobile` or `tablet`.
    pub devices: Vec<ClickBreakdown>,
    /// By browser name, e.g. `Chrome`; `None` where it is unknown.
    pub browsers: Vec<ClickBreakdown>,
}

/// The Clicks in the bucket starting at `time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClickBucket {
    pub time: DateTime<Utc>,
    pub count: i64,
}

/// The Clicks sharing one `value`; breakdowns list the most Clicks first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClickBreakdown {
    pub value: Option<String>,
    pub count: i64,
}

/// Query of `GET /api/workspaces/{slug}/clicks`: Clicks at or after `from` and
/// before `to`, counted per `interval` (a day when absent, in UTC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceClicksQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<Interval>,
}

/// Response of `GET /api/workspaces/{slug}/clicks`: the Clicks of all the Workspace's
/// Links, Deleted Links included, since Clicks are kept forever.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceClicks {
    /// Every bucket from the one holding `from` up to `to`, empty ones included.
    pub series: Vec<ClickBucket>,
    /// The live Links with the most Clicks in the range, most first; at most 10.
    pub top_links: Vec<TopLink>,
}

/// A Link and its Clicks in the range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopLink {
    pub code: String,
    pub domain: String,
    pub short_url: String,
    pub count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debug_output_never_shows_a_password_token_or_secret() {
        let s = || "hunter2-s3cret".to_owned();
        let key = ApiKey {
            id: 1,
            name: "CI".into(),
            prefix: "crab_1234567".into(),
            created_at: "2030-01-01T00:00:00Z".parse().unwrap(),
            last_used_at: None,
        };
        for debug in [
            format!(
                "{:?}",
                SignIn {
                    email: "ada@example.com".into(),
                    password: s()
                }
            ),
            format!(
                "{:?}",
                ChangePassword {
                    current_password: s(),
                    new_password: s()
                }
            ),
            format!(
                "{:?}",
                AcceptInvite {
                    token: s(),
                    password: s()
                }
            ),
            format!("{:?}", NewApiKey { key, secret: s() }),
            format!("{:?}", NewPassword { password: s() }),
            format!(
                "{:?}",
                NewInvite {
                    url: format!("https://crabl.ink/app/invite#{}", s()),
                    expires_at: DateTime::UNIX_EPOCH
                }
            ),
        ] {
            assert!(!debug.contains("s3cret"), "{debug}");
            assert!(debug.contains("<redacted>"), "{debug}");
        }
    }

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
                tags: None,
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
            domain: "crabl.ink".into(),
            short_url: "https://crabl.ink/abc".into(),
            target: "https://example.com".into(),
            created_at: t,
            updated_at: t,
            expires_at,
            click_count: 0,
            tags: Vec::new(),
            blocked: false,
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
