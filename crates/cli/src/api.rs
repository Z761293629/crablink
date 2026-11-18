//! `Api`: the HTTP client for one Workspace's Links, authenticated with its API Key.

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use crablink_api::{CreateLink, Link, LinkPage, Me, QrFormat, UpdateLink, Workspace};
use serde::de::DeserializeOwned;
use ureq::http::{Method, Request, Response};
use ureq::{Body, SendBody};

/// Longest a single API request may take, so a hung server can't freeze the TUI.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP client for `/api`, acting on Workspace `workspace` with `api_key`. Every
/// method fails with the server's `{ "error" }` message (plus status) on a non-2xx
/// response. Reuses connections.
pub struct Api {
    pub(crate) server: String,
    pub(crate) api_key: String,
    /// The Workspace's slug.
    pub(crate) workspace: String,
    agent: ureq::Agent,
}

impl Api {
    /// A client for `server` (any trailing slash dropped) that authenticates with
    /// `api_key` and acts on Workspace `workspace`.
    pub(crate) fn new(server: String, api_key: String, workspace: String) -> Api {
        Api {
            server: server.trim_end_matches('/').into(),
            api_key,
            workspace,
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

    /// The Workspace's slug.
    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    /// `GET /api/me`: the API Key's Workspace.
    pub fn me(&self) -> anyhow::Result<Workspace> {
        match self.json(Method::GET, "/api/me", SendBody::none())? {
            Me::Workspace(w) => Ok(w),
            Me::User(_) => Err(anyhow!("the server took the API Key for a User")),
        }
    }

    /// `POST /api/workspaces/{slug}/links`.
    pub fn create(&self, body: &CreateLink) -> anyhow::Result<Link> {
        let path = self.links_path();
        self.json(Method::POST, &path, SendBody::from_json(body)?)
    }

    /// `GET /api/workspaces/{slug}/links`, following `next_cursor`; calls `each_page`
    /// with every page, newest first.
    pub fn list(&self, mut each_page: impl FnMut(Vec<Link>)) -> anyhow::Result<()> {
        let mut cursor: Option<String> = None;
        loop {
            let path = match &cursor {
                Some(c) => format!("{}?cursor={}", self.links_path(), encode(c)),
                None => self.links_path(),
            };
            let page: LinkPage = self.json(Method::GET, &path, SendBody::none())?;
            each_page(page.links);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(());
            }
        }
    }

    /// `GET /api/workspaces/{slug}/links/{code}`.
    pub fn get(&self, code: &str) -> anyhow::Result<Link> {
        self.json(Method::GET, &self.link_path(code), SendBody::none())
    }

    /// `PATCH /api/workspaces/{slug}/links/{code}`.
    pub fn update(&self, code: &str, body: &UpdateLink) -> anyhow::Result<Link> {
        let path = self.link_path(code);
        self.json(Method::PATCH, &path, SendBody::from_json(body)?)
    }

    /// `DELETE /api/workspaces/{slug}/links/{code}`.
    pub fn delete(&self, code: &str) -> anyhow::Result<()> {
        self.send(Method::DELETE, &self.link_path(code), SendBody::none())
            .map(drop)
    }

    /// `GET /api/workspaces/{slug}/links/{code}/qr`: the QR code file's bytes.
    pub fn qr(&self, code: &str, format: QrFormat) -> anyhow::Result<Vec<u8>> {
        let path = format!("{}/qr?format={}", self.link_path(code), format.as_str());
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
            .header("Authorization", format!("Bearer {}", self.api_key))
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

    /// The Workspace's Links. The slug is encoded as one path segment, like a Code.
    fn links_path(&self) -> String {
        format!("/api/workspaces/{}/links", encode(&self.workspace))
    }

    /// The Code as one path segment, so e.g. `rm 'a?x'` cannot hit Link `a`.
    /// Whether the Code is valid is the server's call.
    fn link_path(&self, code: &str) -> String {
        format!("{}/{}", self.links_path(), encode(code))
    }
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
        let api = Api::new("https://x".into(), "key".into(), "ada".into());
        let link_path = |code| api.link_path(code);
        assert_eq!(link_path("Ab_-9"), "/api/workspaces/ada/links/Ab_-9");
        assert_eq!(link_path("a?x"), "/api/workspaces/ada/links/a%3Fx");
        assert_eq!(link_path("a/b#c"), "/api/workspaces/ada/links/a%2Fb%23c");
        assert_eq!(link_path(".."), "/api/workspaces/ada/links/%2E%2E");
        assert_eq!(link_path("é"), "/api/workspaces/ada/links/%C3%A9");
    }
}
