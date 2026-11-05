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
