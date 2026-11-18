//! `Api`: the authenticated HTTP client for the management API.

use std::time::Duration;

use anyhow::{Context, bail};
use crablink_api::{CreateLink, Link, LinkPage, QrFormat, UpdateLink};
use serde::de::DeserializeOwned;
use ureq::http::{Method, Request, Response};
use ureq::{Body, SendBody};

/// Longest a single API request may take, so a hung server can't freeze the TUI.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Authenticated HTTP client for `/api`. Every method fails with the server's
/// `{ "error" }` message (plus status) on a non-2xx response. Reuses connections.
pub struct Api {
    pub(crate) server: String,
    pub(crate) token: String,
    agent: ureq::Agent,
}

impl Api {
    /// A client for `server` (any trailing slash dropped) that authenticates with `token`.
    pub(crate) fn new(server: String, token: String) -> Api {
        Api {
            server: server.trim_end_matches('/').into(),
            token,
            agent: ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(REQUEST_TIMEOUT))
                .build()
                .into(),
        }
    }

    /// The server URL, without a trailing slash.
    pub fn server(&self) -> &str {
        &self.server
    }

    /// `POST /api/links`.
    pub fn create(&self, body: &CreateLink) -> anyhow::Result<Link> {
        self.json(Method::POST, "/api/links", SendBody::from_json(body)?)
    }

    /// `GET /api/links`, following `next_cursor`; calls `each_page` with every page, newest first.
    pub fn list(&self, mut each_page: impl FnMut(Vec<Link>)) -> anyhow::Result<()> {
        let mut cursor: Option<String> = None;
        loop {
            let path = match &cursor {
                Some(c) => format!("/api/links?cursor={}", encode(c)),
                None => "/api/links".into(),
            };
            let page: LinkPage = self.json(Method::GET, &path, SendBody::none())?;
            each_page(page.links);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(());
            }
        }
    }

    /// `GET /api/links/{code}`.
    pub fn get(&self, code: &str) -> anyhow::Result<Link> {
        self.json(Method::GET, &link_path(code), SendBody::none())
    }

    /// `PATCH /api/links/{code}`.
    pub fn update(&self, code: &str, body: &UpdateLink) -> anyhow::Result<Link> {
        self.json(Method::PATCH, &link_path(code), SendBody::from_json(body)?)
    }

    /// `DELETE /api/links/{code}`.
    pub fn delete(&self, code: &str) -> anyhow::Result<()> {
        self.send(Method::DELETE, &link_path(code), SendBody::none())
            .map(drop)
    }

    /// `GET /api/links/{code}/qr`: the QR code file's bytes.
    pub fn qr(&self, code: &str, format: QrFormat) -> anyhow::Result<Vec<u8>> {
        let path = format!("{}/qr?format={}", link_path(code), format.as_str());
        let mut res = self.send(Method::GET, &path, SendBody::none())?;
        Ok(res.body_mut().read_to_vec()?)
    }

    pub(crate) fn send(
        &self,
        method: Method,
        path: &str,
        body: SendBody,
    ) -> anyhow::Result<Response<Body>> {
        let req = Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.server))
            .header("Authorization", format!("Bearer {}", self.token))
            // `agent.run` ignores `SendBody::from_json`'s content type; every API body is JSON.
            .header("Content-Type", "application/json")
            .body(body)?;
        let mut res = self
            .agent
            .run(req)
            .with_context(|| format!("cannot reach {}", self.server))?;
        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }
        match res.body_mut().read_json::<crablink_api::ErrorBody>() {
            Ok(e) => bail!("{} ({status})", e.error),
            Err(_) => bail!("server returned {status}"),
        }
    }

    /// `send`, then read the JSON response body.
    fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: SendBody,
    ) -> anyhow::Result<T> {
        Ok(self.send(method, path, body)?.body_mut().read_json()?)
    }
}

/// The Code as one path segment, so e.g. `rm 'a?x'` cannot hit Link `a`.
/// Whether the Code is valid is the server's call.
fn link_path(code: &str) -> String {
    format!("/api/links/{}", encode(code))
}

/// Percent-encodes every byte except `A-Z a-z 0-9 _ -`.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_sent_as_one_encoded_path_segment() {
        assert_eq!(link_path("Ab_-9"), "/api/links/Ab_-9");
        assert_eq!(link_path("a?x"), "/api/links/a%3Fx");
        assert_eq!(link_path("a/b#c"), "/api/links/a%2Fb%23c");
        assert_eq!(link_path(".."), "/api/links/%2E%2E");
        assert_eq!(link_path("é"), "/api/links/%C3%A9");
    }
}
