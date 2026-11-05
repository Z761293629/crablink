//! Shared test harness: builds the real app through `crablink_server::app`
//! with a fresh in-memory database, and sends requests in process.
//!
//! Use from a test file with `mod common;` then `use common::*;`.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub const TOKEN: &str = "test-token";
pub const BASE_URL: &str = "https://crab.link";

/// App with random Generated Codes.
pub async fn app() -> TestApp {
    app_with_codes(&[]).await
}

/// App whose Code generator yields `codes` in order, then random Codes.
pub async fn app_with_codes(codes: &[&str]) -> TestApp {
    let pool = crablink_server::connect("sqlite::memory:").await.unwrap();
    let script = Mutex::new(codes.iter().map(|c| c.to_string()).collect::<VecDeque<_>>());
    let generator: crablink_server::CodeGenerator = Arc::new(move || {
        script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(crablink_server::random_code)
    });
    TestApp(crablink_server::app(pool, generator, TOKEN, BASE_URL))
}

pub struct TestApp(pub Router);

impl TestApp {
    pub async fn send(&self, req: Request<Body>) -> Res {
        let resp = self.0.clone().oneshot(req).await.unwrap();
        Res::read(resp).await
    }

    /// Unauthenticated GET, e.g. a visitor following a Short URL.
    pub async fn get(&self, path: &str) -> Res {
        self.send(Request::get(path).body(Body::empty()).unwrap())
            .await
    }

    /// Request carrying the valid token, with an optional JSON body.
    pub async fn api(&self, method: &str, path: &str, body: Option<Value>) -> Res {
        self.api_with_token(method, path, body, Some(TOKEN)).await
    }

    pub async fn api_with_token(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        token: Option<&str>,
    ) -> Res {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let req = match body {
            Some(json) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json.to_string())),
            None => req.body(Body::empty()),
        };
        self.send(req.unwrap()).await
    }

    /// `POST /api/links`, asserting `201`; returns the Link JSON.
    pub async fn create(&self, body: Value) -> Value {
        let res = self.api("POST", "/api/links", Some(body)).await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        res.json()
    }
}

pub struct Res {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub text: String,
}

impl Res {
    async fn read(resp: Response) -> Self {
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        Res {
            status,
            headers,
            text: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|e| panic!("{e}: {}", self.text))
    }

    pub fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }
}
