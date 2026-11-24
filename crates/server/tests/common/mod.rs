//! Shared test harness: builds the real app through `crablink_server::app`
//! with a fresh Postgres database, and sends requests in process.
//!
//! Use from a test file with `mod common;` then `use common::*;`.
#![allow(dead_code)]

mod pg;

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
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
pub const INSTANCE_ADMIN: &str = "admin@example.com";
/// The socket address `TestApp::visit` comes from; the fake country lookup puts it in France.
pub const SOCKET_IP: &str = "192.0.2.1";
/// An address the fake country lookup puts in New Zealand.
pub const NZ_IP: &str = "198.51.100.7";
pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// The fake country lookup: `SOCKET_IP` is `FR`, `NZ_IP` is `NZ`, anything else unknown.
fn fake_country(ip: IpAddr) -> Option<String> {
    let country = match ip.to_string().as_str() {
        SOCKET_IP => "FR",
        NZ_IP => "NZ",
        _ => return None,
    };
    Some(country.into())
}

/// The test instance's settings on `base_url`: random Generated Codes, the fake
/// country lookup, and `clock`.
fn settings(
    base_url: &str,
    clock: crablink_server::Clock,
    trust_proxy: bool,
) -> crablink_server::Settings {
    crablink_server::Settings {
        trust_proxy,
        clock,
        country: Arc::new(fake_country),
        ..crablink_server::Settings::new(base_url)
    }
}

/// App with random Generated Codes.
pub async fn app() -> TestApp {
    app_with_codes(&[]).await
}

/// App with random Generated Codes that trusts `X-Forwarded-For` from its proxy.
pub async fn app_behind_proxy() -> TestApp {
    build(&[], true).await
}

/// App whose Code generator yields `codes` in order, then random Codes.
pub async fn app_with_codes(codes: &[&str]) -> TestApp {
    build(codes, false).await
}

async fn build(codes: &[&str], trust_proxy: bool) -> TestApp {
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
    let settings = crablink_server::Settings {
        codes: generator,
        ..settings(BASE_URL, clock, trust_proxy)
    };
    let router = crablink_server::app(pool.clone(), settings).await.unwrap();
    let mut app = TestApp {
        router,
        pool,
        now,
        trust_proxy,
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
    trust_proxy: bool,
    /// `OWNER`'s Account: what `api` and `create` act as.
    pub owner: Account,
}

/// A visitor's request for `path` on Domain `host` from `SOCKET_IP`, sending `headers`.
pub fn visitor_request(host: &str, path: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut req = Request::get(path).header(header::HOST, host);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let mut req = req.body(Body::empty()).unwrap();
    let socket = SocketAddr::new(SOCKET_IP.parse().unwrap(), 50000);
    req.extensions_mut().insert(ConnectInfo(socket));
    req
}

impl TestApp {
    /// Sends `req`; after a redirect, also waits for its Click to be recorded, so
    /// tests see it at once.
    pub async fn send(&self, req: Request<Body>) -> Res {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        if let Some(click) = resp.extensions().get::<crablink_server::ClickRecording>() {
            click.done().await;
        }
        Res::read(resp).await
    }

    /// The app restarted with `CRABLINK_BASE_URL` set to `base_url`, on the same
    /// database and clock.
    pub async fn restarted_on(&self, base_url: &str) -> TestApp {
        let now = self.now.clone();
        let clock: crablink_server::Clock = Arc::new(move || *now.lock().unwrap());
        let settings = settings(base_url, clock, self.trust_proxy);
        let router = crablink_server::app(self.pool.clone(), settings)
            .await
            .unwrap();
        TestApp {
            router,
            pool: self.pool.clone(),
            now: self.now.clone(),
            trust_proxy: self.trust_proxy,
            owner: self.owner.clone(),
        }
    }

    /// What the app's clock reads.
    pub fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }

    /// Sets the app's clock, e.g. to place Clicks at chosen times.
    pub fn set_now(&self, to: &str) {
        *self.now.lock().unwrap() = to.parse().unwrap();
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

    /// Signs in as the Instance Admin `INSTANCE_ADMIN`, creating them if missing; returns the cookie.
    pub async fn instance_admin(&self) -> String {
        // Already created by an earlier call is fine.
        let _ = crablink_server::create_user(&self.pool, INSTANCE_ADMIN, PASSWORD, true).await;
        self.sign_in(INSTANCE_ADMIN, PASSWORD).await
    }

    /// `POST /api/workspaces/{slug}/domains` adding `domain`, as the session `cookie`.
    pub async fn add_domain(&self, cookie: &str, slug: &str, domain: &str) -> Res {
        let path = format!("/api/workspaces/{slug}/domains");
        let body = serde_json::json!({ "domain": domain });
        self.as_user("POST", &path, Some(body), cookie).await
    }

    /// Sets `domain`'s status (`approved`, `rejected`, …) as the Instance Admin,
    /// asserting `200`.
    pub async fn set_domain_status(&self, domain: &str, status: &str) {
        let admin = self.instance_admin().await;
        let path = format!("/api/admin/domains/{domain}");
        let body = serde_json::json!({ "status": status });
        let res = self.as_user("PATCH", &path, Some(body), &admin).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    }

    /// Adds `domain` to `OWNER`'s Workspace and approves it.
    pub async fn approved_domain(&self, domain: &str) {
        let (cookie, slug) = (&self.owner.cookie, &self.owner.slug);
        let res = self.add_domain(cookie, slug, domain).await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        self.set_domain_status(domain, "approved").await;
    }

    /// A visitor following a Short URL from `SOCKET_IP`, sending `headers`.
    pub async fn visit(&self, path: &str, headers: &[(&str, &str)]) -> Res {
        self.visit_on(HOST, path, headers).await
    }

    /// `visit` on Domain `host`.
    pub async fn visit_on(&self, host: &str, path: &str, headers: &[(&str, &str)]) -> Res {
        self.send(visitor_request(host, path, headers)).await
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
