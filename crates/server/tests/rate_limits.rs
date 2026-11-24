mod common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use chrono::TimeDelta;
use common::*;
use serde_json::json;

const WRONG: &str = "wrong password";

/// `n` per `window`, e.g. `limit("2/1m")`.
fn limit(spec: &str) -> Option<crablink_server::Limit> {
    Some(spec.parse().unwrap())
}

/// A sign-in to `OWNER` with `password` from socket `SOCKET_IP`, sending `headers`.
async fn sign_in_from(app: &TestApp, password: &str, headers: &[(&str, &str)]) -> Res {
    let body = json!({ "email": OWNER, "password": password });
    let mut req = Request::post("/api/session")
        .header(header::HOST, HOST)
        .header("origin", ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let mut req = req.body(Body::from(body.to_string())).unwrap();
    let socket = SocketAddr::new(SOCKET_IP.parse().unwrap(), 50000);
    req.extensions_mut().insert(ConnectInfo(socket));
    app.send(req).await
}

/// Asserts `res` is a 429 telling the caller to wait `seconds`.
fn assert_rate_limited(res: &Res, seconds: &str) {
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS, "{}", res.text);
    assert_eq!(res.json()["kind"], "rate_limited");
    assert_eq!(res.header("retry-after"), seconds);
}

#[tokio::test]
async fn sign_in_is_limited_per_visitor_ip_until_the_window_passes() {
    let app = app_with(|s| {
        s.trust_proxy = true;
        s.rate_limits.sign_in = limit("2/1m");
    })
    .await;
    let from = |ip| [("x-forwarded-for", ip)];
    for _ in 0..2 {
        let res = sign_in_from(&app, WRONG, &from(NZ_IP)).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    }

    app.advance(TimeDelta::seconds(15));
    let res = sign_in_from(&app, WRONG, &from(NZ_IP)).await;
    assert_rate_limited(&res, "45");
    // Even the right password waits.
    assert_rate_limited(&sign_in_from(&app, PASSWORD, &from(NZ_IP)).await, "45");
    let other_visitor = sign_in_from(&app, WRONG, &from("203.0.113.9")).await;
    assert_eq!(other_visitor.status, StatusCode::UNAUTHORIZED);

    app.advance(TimeDelta::seconds(45));
    let res = sign_in_from(&app, WRONG, &from(NZ_IP)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
}

#[tokio::test]
async fn without_a_trusted_proxy_a_forged_x_forwarded_for_doesnt_dodge_the_limit() {
    let app = app_with(|s| s.rate_limits.sign_in = limit("1/1h")).await;
    let res = sign_in_from(&app, WRONG, &[("x-forwarded-for", NZ_IP)]).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);

    let res = sign_in_from(&app, WRONG, &[("x-forwarded-for", "203.0.113.9")]).await;
    assert_rate_limited(&res, "3600");
}

#[tokio::test]
async fn link_creation_is_limited_per_workspace_until_the_window_passes() {
    let app = app_with(|s| s.rate_limits.links = limit("2/1h")).await;
    app.create(json!({ "target": "https://example.com/1" }))
        .await;
    app.advance(TimeDelta::minutes(30));
    app.create(json!({ "target": "https://example.com/2" }))
        .await;

    let res = app
        .api(
            "POST",
            "/links",
            Some(json!({ "target": "https://example.com/3" })),
        )
        .await;
    assert_rate_limited(&res, "1800");
    // From the web UI, as a User of the same Workspace, too.
    let path = format!("/api/workspaces/{}/links", app.owner.slug);
    let body = json!({ "target": "https://example.com/3" });
    let res = app
        .as_user("POST", &path, Some(body), &app.owner.cookie)
        .await;
    assert_rate_limited(&res, "1800");
    // Another Workspace has its own count.
    let other = app.account("other@example.com").await;
    let path = format!("/api/workspaces/{}/links", other.slug);
    let body = json!({ "target": "https://example.com/other" });
    let res = app
        .bearer("POST", &path, Some(body), Some(&other.key))
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);

    app.advance(TimeDelta::minutes(30));
    app.create(json!({ "target": "https://example.com/3" }))
        .await;
}

#[tokio::test]
async fn a_csv_import_counts_each_row_against_the_link_limit() {
    let app = app_with(|s| s.rate_limits.links = limit("2/1h")).await;
    let csv = "Code,Target\none,https://example.com/1\ntwo,https://example.com/2\nthree,https://example.com/3\n";
    let req = Request::post(format!("/api/workspaces/{}/links.csv", app.owner.slug))
        .header(header::HOST, HOST)
        .header(header::AUTHORIZATION, format!("Bearer {}", app.owner.key))
        .header(header::CONTENT_TYPE, "text/csv")
        .body(Body::from(csv))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);

    let rows = res.json();
    assert_eq!(rows[0]["short_url"], "https://crabl.ink/one");
    assert_eq!(rows[1]["short_url"], "https://crabl.ink/two");
    assert_eq!(rows[2]["skipped"]["kind"], "rate_limited", "{rows}");
    let res = app
        .api(
            "POST",
            "/links",
            Some(json!({ "target": "https://example.com/4" })),
        )
        .await;
    assert_rate_limited(&res, "3600");
}

#[tokio::test]
async fn redirects_are_never_limited() {
    let app = app_with(|s| {
        s.rate_limits.sign_in = limit("1/1h");
        s.rate_limits.links = limit("1/1h");
    })
    .await;
    app.create(json!({ "target": "https://example.com/viral", "code": "viral" }))
        .await;
    for _ in 0..20 {
        let res = app.visit("/viral", &[("user-agent", BROWSER_UA)]).await;
        assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);
    }
}

#[tokio::test]
async fn with_no_limit_set_nothing_is_limited() {
    let app = app().await;
    for i in 0..30 {
        let res = sign_in_from(&app, WRONG, &[]).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
        app.create(json!({ "target": format!("https://example.com/{i}") }))
            .await;
    }
}

#[test]
fn a_limit_reads_as_count_per_window() {
    let limit = |s: &str| s.parse::<crablink_server::Limit>();
    let ten_a_minute = limit("10/1m").unwrap();
    assert_eq!(
        (ten_a_minute.count, ten_a_minute.window),
        (10, TimeDelta::minutes(1))
    );
    assert_eq!(limit("1000/1d").unwrap().window, TimeDelta::days(1));
    assert_eq!(limit("5/30s").unwrap().window, TimeDelta::seconds(30));
    assert_eq!(limit("5/2h").unwrap().window, TimeDelta::hours(2));
    for bad in [
        "", "10", "10/", "10/m", "10/1", "10/1w", "0/1m", "10/0s", "-1/1m", "x/1m",
    ] {
        assert!(limit(bad).is_err(), "{bad:?} parsed");
    }
}
