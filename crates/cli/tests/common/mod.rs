//! Shared CLI test harness: starts a real `crablink-server` (built through
//! `crablink_server::app` with a fresh in-memory database) on `127.0.0.1:0`,
//! served on a background runtime. The CLI itself stays synchronous.
//!
//! Use from a test file with `mod common;` then `use common::*;`.
#![allow(dead_code)]

use std::sync::{Arc, mpsc};

use crablink_api::{CreateLink, Link};
use crablink_cli::tui::{App, Caps};
use crablink_cli::{Api, Creds};
pub use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Terminal, backend::TestBackend};

pub const TOKEN: &str = "test-token";

/// A running server; it lives until the test process exits.
pub struct TestServer {
    pub url: String,
    pub token: String,
}

/// Starts a server with random Generated Codes and an empty database.
pub fn server() -> TestServer {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let pool = crablink_server::connect("sqlite::memory:").await.unwrap();
            let app =
                crablink_server::app(pool, Arc::new(crablink_server::random_code), TOKEN, &url);
            tx.send(url).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    TestServer {
        url: rx.recv().expect("test server failed to start"),
        token: TOKEN.into(),
    }
}

impl TestServer {
    /// The CLI's authenticated API client for this server.
    pub fn api(&self) -> Api {
        Creds {
            server: Some(self.url.clone()),
            token: Some(self.token.clone()),
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

    /// Runs the `crablink` binary against this server with `args`; never reads a real config file.
    pub fn cli(&self, args: &[&str]) -> CliOutput {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_crablink"))
            .args(["--server", &self.url, "--token", &self.token])
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

    /// The raw JSON body of an authenticated `GET` on this server, bypassing the CLI.
    pub fn raw_get(&self, path: &str) -> serde_json::Value {
        ureq::get(format!("{}{path}", self.url))
            .header("Authorization", format!("Bearer {}", self.token))
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
