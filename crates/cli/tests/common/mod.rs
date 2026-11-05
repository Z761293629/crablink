//! Shared CLI test harness: starts a real `crablink-server` (built through
//! `crablink_server::app` with a fresh in-memory database) on `127.0.0.1:0`,
//! served on a background runtime. The CLI itself stays synchronous.
//!
//! Use from a test file with `mod common;` then `use common::*;`.
#![allow(dead_code)]

use std::sync::{Arc, mpsc};

use crablink_cli::{Api, Creds};

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
}
