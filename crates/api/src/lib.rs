//! Request and response DTOs shared by `crablink-server` and the `crablink` CLI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
pub struct Error {
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
