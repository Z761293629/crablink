//! Shared CLI test harness: starts a real `crablink-server` (built through
//! `crablink_server::app` with a fresh Postgres database) on `127.0.0.1:0`,
//! served on a background runtime, and signs in with an API Key of a User's
//! personal Workspace. The CLI itself stays synchronous.
//!
//! Use from a test file with `mod common;` then `use common::*;`.
#![allow(dead_code)]

#[path = "../../../server/tests/common/pg.rs"]
mod pg;

use std::sync::{Arc, mpsc};

use crablink_api::{CreateLink, Link};
use crablink_cli::tui::{App, Caps};
use crablink_cli::{Api, Creds};
pub use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Terminal, backend::TestBackend};

const EMAIL: &str = "owner@example.com";
const PASSWORD: &str = "correct horse battery";

/// A running server and the credentials the CLI uses on it; the server lives until
/// the test process exits.
pub struct TestServer {
    pub url: String,
    /// An API Key of `workspace`.
    pub key: String,
    /// The slug of `EMAIL`'s personal Workspace.
    pub workspace: String,
}

/// Starts a server with random Generated Codes and an empty database, signs in as a
/// new User and issues an API Key of their personal Workspace.
pub fn server() -> TestServer {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let pool = crablink_server::connect(&pg::database_url().await)
                .await
                .unwrap();
            crablink_server::create_user(&pool, EMAIL, PASSWORD, false)
                .await
                .unwrap();
            let app = crablink_server::app(
                pool,
                Arc::new(crablink_server::random_code),
                Arc::new(chrono::Utc::now),
                &url,
            )
            .await
            .unwrap();
            tx.send(url).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    let url = rx.recv().expect("test server failed to start");
    let (key, workspace) = issue_api_key(&url);
    TestServer {
        url,
        key,
        workspace,
    }
}

/// Signs in as `EMAIL` like a browser would and issues an API Key of their personal
/// Workspace; returns its secret and the Workspace's slug.
fn issue_api_key(url: &str) -> (String, String) {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(true)
        .build()
        .into();
    let res = agent
        .post(format!("{url}/api/session"))
        .header("Origin", url)
        .send_json(serde_json::json!({ "email": EMAIL, "password": PASSWORD }))
        .unwrap();
    let set_cookie = res.headers()["set-cookie"].to_str().unwrap();
    let cookie = set_cookie.split(';').next().unwrap().to_string();
    let workspaces: serde_json::Value = agent
        .get(format!("{url}/api/workspaces"))
        .header("Cookie", &cookie)
        .call()
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap();
    let slug = workspaces[0]["slug"].as_str().unwrap().to_string();
    let key: serde_json::Value = agent
        .post(format!("{url}/api/workspaces/{slug}/api-keys"))
        .header("Cookie", &cookie)
        .header("Origin", url)
        .send_json(serde_json::json!({ "name": "CLI tests" }))
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap();
    (key["secret"].as_str().unwrap().to_string(), slug)
}

impl TestServer {
    /// The CLI's authenticated API client for this server.
    pub fn api(&self) -> Api {
        Creds {
            server: Some(self.url.clone()),
            api_key: Some(self.key.clone()),
            workspace: Some(self.workspace.clone()),
        }
        .api()
        .unwrap()
    }

    /// Creates a Link with a Custom Code and no Expiry.
    pub fn create(&self, code: &str, target: &str) -> Link {
        self.api()
            .create(&CreateLink {
                target: target.into(),
                code: Some(code.into()),
                expires_at: None,
            })
            .unwrap()
    }

    /// The TUI opened against this server on a 100x30 screen, with no image support.
    pub fn tui(&self) -> Tui {
        Tui::new(self.api())
    }

    /// The global flags that point the `crablink` binary at this server.
    pub fn flags(&self) -> [&str; 6] {
        [
            "--server",
            &self.url,
            "--api-key",
            &self.key,
            "--workspace",
            &self.workspace,
        ]
    }

    /// Runs the `crablink` binary against this server with `args`; never reads a real config file.
    pub fn cli(&self, args: &[&str]) -> CliOutput {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_crablink"))
            .args(self.flags())
            .args(args)
            .env("HOME", std::env::temp_dir().join("crablink-test-no-home"))
            .output()
            .unwrap();
        CliOutput {
            success: out.status.success(),
            stdout: String::from_utf8(out.stdout).unwrap(),
            stderr: String::from_utf8(out.stderr).unwrap(),
        }
    }

    /// The raw JSON body of a `GET` with the API Key on `path` under the Workspace
    /// (e.g. `/links/abc`), bypassing the CLI.
    pub fn raw_get(&self, path: &str) -> serde_json::Value {
        ureq::get(format!(
            "{}/api/workspaces/{}{path}",
            self.url, self.workspace
        ))
        .header("Authorization", format!("Bearer {}", self.key))
        .call()
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap()
    }
}

/// What one `crablink` run printed, and whether it exited 0.
pub struct CliOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    /// Stdout parsed as JSON, after asserting the run succeeded.
    pub fn json(&self) -> serde_json::Value {
        assert!(self.success, "crablink failed: {}", self.stderr);
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", self.stdout))
    }
}

/// Reads a QR code drawn in half blocks (`█ ▀ ▄`, space for two light modules) back
/// into modules and decodes it. Rows may differ in length; anything missing is light.
pub fn decode_half_blocks(rows: &[&str]) -> String {
    const PX: usize = 4; // pixels per module
    const MARGIN: usize = 4; // modules of extra light border, in case `rows` was cropped
    let cols = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let (w, h) = (cols + 2 * MARGIN, rows.len() * 2 + 2 * MARGIN);
    let dark = |x: usize, y: usize| -> bool {
        let (Some(x), Some(y)) = (x.checked_sub(MARGIN), y.checked_sub(MARGIN)) else {
            return false;
        };
        let cell = rows.get(y / 2).and_then(|r| r.chars().nth(x));
        match (cell, y % 2) {
            (Some('█'), _) | (Some('▀'), 0) | (Some('▄'), 1) => true,
            (None | Some(' ' | '▀' | '▄'), _) => false,
            (Some(c), _) => panic!("not a half block: {c:?}"),
        }
    };
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(w * PX, h * PX, |x, y| {
        if dark(x / PX, y / PX) { 0 } else { 255 }
    });
    let grids = img.detect_grids();
    assert_eq!(
        grids.len(),
        1,
        "expected one QR code in:\n{}",
        rows.join("\n")
    );
    grids[0].decode().expect("a decodable QR code").1
}

/// The TUI app fed key events and rendered to ratatui's in-memory `TestBackend`.
pub struct Tui {
    pub app: App,
    term: Terminal<TestBackend>,
}

impl Tui {
    pub fn new(api: Api) -> Tui {
        Tui::with_caps(api, Caps::default())
    }

    pub fn with_caps(api: Api, caps: Caps) -> Tui {
        Tui {
            app: App::new(api, caps),
            term: Terminal::new(TestBackend::new(100, 30)).unwrap(),
        }
    }

    /// Presses each character as one key.
    pub fn keys(&mut self, s: &str) {
        for c in s.chars() {
            self.key(KeyCode::Char(c));
        }
    }

    pub fn key(&mut self, code: KeyCode) {
        self.app.handle_key(KeyEvent::from(code));
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.term.backend_mut().resize(width, height);
    }

    /// Draws the app and returns the screen as text: one line per row, trailing blanks trimmed.
    pub fn screen(&mut self) -> String {
        let buf = self.term.draw(|f| self.app.render(f)).unwrap().buffer;
        buf.content
            .chunks(buf.area.width as usize)
            .map(|row| {
                let line: String = row.iter().map(|c| c.symbol()).collect();
                line.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
