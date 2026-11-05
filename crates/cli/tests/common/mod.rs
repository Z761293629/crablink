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
