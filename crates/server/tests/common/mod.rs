//! Shared test harness: builds the real app through `crablink_server::app`
//! with a fresh Postgres database, and sends requests in process.
//!
//! Use from a test file with `mod common;` then `use common::*;`.
#![allow(dead_code)]

mod pg;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use chrono::{DateTime, TimeDelta, Utc};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

pub const TOKEN: &str = "test-token";
pub const BASE_URL: &str = "https://crab.link";
/// The `Host` and `Origin` a browser on `BASE_URL` sends.
pub const HOST: &str = "crab.link";
pub const ORIGIN: &str = BASE_URL;
/// The password `TestApp::create_user` gives every User.
pub const PASSWORD: &str = "correct horse battery";

/// App with random Generated Codes.
pub async fn app() -> TestApp {
    app_with_codes(&[]).await
}

/// App whose Code generator yields `codes` in order, then random Codes.
pub async fn app_with_codes(codes: &[&str]) -> TestApp {
    let pool = crablink_server::connect(&pg::database_url().await)
        .await
        .unwrap();
    let script = Mutex::new(codes.iter().map(|c| c.to_string()).collect::<VecDeque<_>>());
    let generator: crablink_server::CodeGenerator = Arc::new(move || {
        script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(crablink_server::random_code)
    });
    let now = Arc::new(Mutex::new(Utc::now()));
    let clock = now.clone();
    let clock: crablink_server::Clock = Arc::new(move || *clock.lock().unwrap());
    TestApp {
        router: crablink_server::app(pool.clone(), generator, clock, TOKEN, BASE_URL),
        pool,
        now,
    }
}

pub struct TestApp {
    pub router: Router,
    pub pool: PgPool,
    /// What the app's clock reads; starts at the real time.
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl TestApp {
    pub async fn send(&self, req: Request<Body>) -> Res {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        Res::read(resp).await
    }

    /// Moves the app's clock forward.
    pub fn advance(&self, by: TimeDelta) {
        *self.now.lock().unwrap() += by;
    }

    /// Creates a User, not an Instance Admin, whose password is `PASSWORD`.
    pub async fn create_user(&self, email: &str) {
        crablink_server::create_user(&self.pool, email, PASSWORD, false)
            .await
            .unwrap();
    }

    /// Signs in, asserting `200`; returns the `Cookie` header value to send back.
    pub async fn sign_in(&self, email: &str, password: &str) -> String {
        let res = self.sign_in_res(email, password).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        let set_cookie = res.header("set-cookie");
        set_cookie.split(';').next().unwrap().to_string()
    }

    /// `POST /api/session` with `email` and `password`, from the app's own origin.
    pub async fn sign_in_res(&self, email: &str, password: &str) -> Res {
        let body = serde_json::json!({ "email": email, "password": password });
        self.request("POST", "/api/session", Some(body), &[("origin", ORIGIN)])
            .await
    }

    /// A browser request carrying the session `cookie`, from the app's own origin.
    pub async fn as_user(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        cookie: &str,
    ) -> Res {
        self.request(
            method,
            path,
            body,
            &[("cookie", cookie), ("origin", ORIGIN)],
        )
        .await
    }

    /// A request to `HOST` with `headers` and an optional JSON body.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> Res {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, HOST);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let req = match body {
            Some(json) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json.to_string())),
            None => req.body(Body::empty()),
        };
        self.send(req.unwrap()).await
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
    /// The body as text; lossy, so a binary body (a PNG) doesn't panic.
    pub text: String,
    pub bytes: Vec<u8>,
}

impl Res {
    async fn read(resp: Response) -> Self {
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        Res {
            status,
            headers,
            text: String::from_utf8_lossy(&bytes).into_owned(),
            bytes: bytes.to_vec(),
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
