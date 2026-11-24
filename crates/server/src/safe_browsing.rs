//! The production Target check: the Google Safe Browsing Lookup API v4, which is for
//! non-commercial use only (a paid service would move to the Web Risk API).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::TargetCheck;

const LOOKUP_URL: &str = "https://safebrowsing.googleapis.com/v4/threatMatches:find";

/// How long a check may take before the Target is let through.
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(serde::Deserialize)]
struct Lookup {
    /// Absent when the Target is on no threat list.
    #[serde(default)]
    matches: Vec<serde_json::Value>,
}

/// A Target check asking Google Safe Browsing with API key `key`: a Target on any of
/// its threat lists is malicious. Errors and timeouts come back as `Err`.
pub fn google_safe_browsing(key: String) -> Result<TargetCheck, reqwest::Error> {
    // Already installed (by an earlier call) is fine.
    drop(rustls::crypto::ring::default_provider().install_default());
    let client = reqwest::Client::builder().timeout(TIMEOUT).build()?;
    Ok(Arc::new(move |target| {
        let body = json!({
            "client": { "clientId": "crablink", "clientVersion": env!("CARGO_PKG_VERSION") },
            "threatInfo": {
                "threatTypes": [
                    "MALWARE",
                    "SOCIAL_ENGINEERING",
                    "UNWANTED_SOFTWARE",
                    "POTENTIALLY_HARMFUL_APPLICATION",
                ],
                "platformTypes": ["ANY_PLATFORM"],
                "threatEntryTypes": ["URL"],
                "threatEntries": [{ "url": target }],
            },
        });
        // A header rather than `?key=`, so the key never shows up in an error's URL.
        let req = client
            .post(LOOKUP_URL)
            .header("x-goog-api-key", &key)
            .json(&body);
        Box::pin(async move {
            let lookup: Lookup = req
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?;
            Ok(!lookup.matches.is_empty())
        })
    }))
}
