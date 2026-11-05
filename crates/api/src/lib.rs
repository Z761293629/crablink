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
    pub error: String,
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
}

fn  fmt_violation( ) {}
