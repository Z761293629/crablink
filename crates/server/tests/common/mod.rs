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

pub const BASE_URL: &str = "https://crabl.ink";
/// The `Host` and `Origin` a browser on `BASE_URL` sends.
pub const HOST: &str = "crabl.ink";
pub const ORIGIN: &str = BASE_URL;
/// The password `TestApp::create_user` gives every User.
pub const PASSWORD: &str = "correct horse battery";
/// The User whose personal Workspace and API Key `TestApp::api` uses.
pub const OWNER: &str = "owner@example.com";
/// The Instance Admin `TestApp::instance_admin` signs in as.
pub const ADMIN: &str = "admin@example.com";

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
    let router = crablink_server::app(pool.clone(), generator, clock, BASE_URL)
        .await
        .unwrap();
    let mut app = TestApp {
        router,
        pool,
        now,
        owner: Account::default(),
    };
    app.owner = app.account(OWNER).await;
    app
}

/// A signed-in User with their personal Workspace and an API Key of it.
#[derive(Default, Clone)]
pub struct Account {
    pub cookie: String,
    pub slug: String,
    pub key: String,
}

pub struct TestApp {
    pub router: Router,
    pub pool: PgPool,
    /// What the app's clock reads; starts at the real time.
    now: Arc<Mutex<DateTime<Utc>>>,
    /// `OWNER`'s Account: what `api` and `create` act as.
    pub owner: Account,
}

impl TestApp {
    pub async fn send(&self, req: Request<Body>) -> Res {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        Res::read(resp).await
    }

    /// The app restarted with `CRABLINK_BASE_URL` set to `base_url`, on the same
    /// database and clock.
    pub async fn restarted_on(&self, base_url: &str) -> TestApp {
        let codes: crablink_server::CodeGenerator = Arc::new(crablink_server::random_code);
        let now = self.now.clone();
        let clock: crablink_server::Clock = Arc::new(move || *now.lock().unwrap());
        let router = crablink_server::app(self.pool.clone(), codes, clock, base_url)
            .await
            .unwrap();
        TestApp {
            router,
            pool: self.pool.clone(),
            now: self.now.clone(),
            owner: self.owner.clone(),
        }
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

    /// Creates a User, signs them in and issues an API Key of their personal Workspace.
    pub async fn account(&self, email: &str) -> Account {
        self.create_user(email).await;
        let cookie = self.sign_in(email, PASSWORD).await;
        let slug = self.workspace_of(&cookie).await;
        let key = self.api_key(&cookie, &slug).await;
        Account { cookie, slug, key }
    }

    /// The slug of the signed-in User's first Workspace.
    pub async fn workspace_of(&self, cookie: &str) -> String {
        let res = self.as_user("GET", "/api/workspaces", None, cookie).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        res.json()[0]["slug"].as_str().unwrap().to_string()
    }

    /// Issues an API Key of Workspace `slug` as the signed-in Owner; returns its secret.
    pub async fn api_key(&self, cookie: &str, slug: &str) -> String {
        let path = format!("/api/workspaces/{slug}/api-keys");
        let body = serde_json::json!({ "name": "test" });
        let res = self.as_user("POST", &path, Some(body), cookie).await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        res.json()["secret"].as_str().unwrap().to_string()
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

    /// Unauthenticated GET to `HOST`, e.g. a visitor following a Short URL.
    pub async fn get(&self, path: &str) -> Res {
        self.get_on(HOST, path).await
    }

    /// Unauthenticated GET with `Host: host`, e.g. a visitor on a custom Domain.
    pub async fn get_on(&self, host: &str, path: &str) -> Res {
        let req = Request::get(path).header(header::HOST, host);
        self.send(req.body(Body::empty()).unwrap()).await
    }

    /// Signs in as the Instance Admin `ADMIN`, creating them if missing; returns the cookie.
    pub async fn instance_admin(&self) -> String {
        // Already created by an earlier call is fine.
        let _ = crablink_server::create_user(&self.pool, ADMIN, PASSWORD, true).await;
        self.sign_in(ADMIN, PASSWORD).await
    }

    /// `POST /api/workspaces/{slug}/domains` adding `host`, as the session `cookie`.
    pub async fn add_domain(&self, cookie: &str, slug: &str, host: &str) -> Res {
        let path = format!("/api/workspaces/{slug}/domains");
        let body = serde_json::json!({ "host": host });
        self.as_user("POST", &path, Some(body), cookie).await
    }

    /// Sets Domain `host`'s status (`approved`, `rejected`, …) as the Instance Admin,
    /// asserting `200`.
    pub async fn set_domain_status(&self, host: &str, status: &str) {
        let admin = self.instance_admin().await;
        let path = format!("/api/admin/domains/{host}");
        let body = serde_json::json!({ "status": status });
        let res = self.as_user("PATCH", &path, Some(body), &admin).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    }

    /// Adds `host` to `OWNER`'s Workspace and approves it.
    pub async fn approved_domain(&self, host: &str) {
        let (cookie, slug) = (&self.owner.cookie, &self.owner.slug);
        let res = self.add_domain(cookie, slug, host).await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        self.set_domain_status(host, "approved").await;
    }

    /// A request with `OWNER`'s API Key to `path` under `OWNER`'s Workspace, e.g.
    /// `/links/abc` for `/api/workspaces/owner/links/abc`.
    pub async fn api(&self, method: &str, path: &str, body: Option<Value>) -> Res {
        self.api_with_key(method, path, body, Some(&self.owner.key))
            .await
    }

    /// `api` with `Authorization: Bearer <key>`, or no credential at all.
    pub async fn api_with_key(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> Res {
        let path = format!("/api/workspaces/{}{path}", self.owner.slug);
        self.bearer(method, &path, body, key).await
    }

    /// A request to the absolute `path` with `Authorization: Bearer <key>`, or no credential.
    pub async fn bearer(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> Res {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, HOST);
        if let Some(key) = key {
            req = req.header(header::AUTHORIZATION, format!("Bearer {key}"));
        }
        let req = match body {
            Some(json) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json.to_string())),
            None => req.body(Body::empty()),
        };
        self.send(req.unwrap()).await
    }

    /// `POST /links` in `OWNER`'s Workspace, asserting `201`; returns the Link JSON.
    pub async fn create(&self, body: Value) -> Value {
        let res = self.api("POST", "/links", Some(body)).await;
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
