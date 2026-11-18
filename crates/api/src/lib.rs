//! Request and response DTOs shared by `crablink-server` and the `crablink` CLI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

/// Body of `POST /api/links`.
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

/// Body of `PATCH /api/links/{code}`. An absent field is left unchanged.
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
    pub short_url: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub click_count: i64,
}

/// Body of every management API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable and machine-readable: branch on this, not on `error`.
    pub kind: ErrorKind,
    /// English message for people.
    pub error: String,
}

/// What went wrong, as `ErrorBody::kind`. Named `kind`, never `code`, so it
/// can't be read as a Link's Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Missing or wrong token.
    Unauthorized,
    /// No such API route.
    NotFound,
    /// No live Link has this Code.
    LinkNotFound,
    /// Malformed body or query.
    BadRequest,
    InvalidTarget,
    /// A Custom Code that isn't `[A-Za-z0-9_-]{3,64}`.
    InvalidCode,
    /// A Custom Code that would shadow one of the service's own routes.
    CodeReserved,
    /// The Code is taken on its Domain, possibly by a Deleted Link.
    CodeTaken,
    Internal,
    /// A kind this build doesn't know, sent by a newer server. Never sent.
    #[serde(other)]
    Unknown,
}

/// Query of `GET /api/links`. `limit` defaults to 50 and must be 1-200.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Opaque: pass back the previous page's `next_cursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Response of `GET /api/links`: newest first; `next_cursor` is `None` on the last page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPage {
    pub links: Vec<Link>,
    pub next_cursor: Option<String>,
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
}
