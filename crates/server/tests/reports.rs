mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

/// `POST /api/reports` by a visitor who isn't signed in.
async fn report(app: &TestApp, short_url: &str, reason: &str) -> Res {
    let body = json!({ "short_url": short_url, "reason": reason });
    app.request("POST", "/api/reports", Some(body), &[]).await
}

/// The open reports, as the Instance Admin sees them.
async fn open_reports(app: &TestApp) -> Value {
    let admin = app.instance_admin().await;
    let res = app.as_user("GET", "/api/admin/reports", None, &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

#[tokio::test]
async fn a_visitor_reports_a_short_url_and_an_instance_admin_sees_it() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example/login" }))
        .await;

    let res = report(&app, "https://crabl.ink/phish", "Steals bank logins").await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let list = open_reports(&app).await;
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");
    let r = &list[0];
    assert_eq!(r["short_url"], "https://crabl.ink/phish");
    assert_eq!(r["target"], "https://evil.example/login");
    assert_eq!(r["workspace_slug"], app.owner.slug.as_str());
    assert_eq!(r["reason"], "Steals bank logins");
    assert_eq!(r["workspace_users"][0]["email"], OWNER);
    assert_eq!(r["workspace_users"][0]["role"], "owner");
    assert_eq!(r["workspace_users"][0]["disabled"], false);
}

#[tokio::test]
async fn only_a_live_link_of_the_instance_can_be_reported() {
    let app = app().await;
    app.create(json!({ "code": "gone", "target": "https://fine.example" }))
        .await;
    app.api("DELETE", "/links/gone", None).await;

    for short_url in [
        "https://crabl.ink/gone",
        "https://crabl.ink/never",
        "https://elsewhere.example/gone",
    ] {
        let res = report(&app, short_url, "spam").await;
        assert_eq!(
            res.status,
            StatusCode::NOT_FOUND,
            "{short_url}: {}",
            res.text
        );
        assert_eq!(res.json()["kind"], "link_not_found");
    }
    assert_eq!(open_reports(&app).await, json!([]));
}

#[tokio::test]
async fn a_report_needs_a_reason() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;

    for reason in ["  ", &"x".repeat(1001)] {
        let res = report(&app, "crabl.ink/phish", reason).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
        assert_eq!(res.json()["kind"], "bad_request");
    }
}

#[tokio::test]
async fn reports_are_limited_per_visitor_ip() {
    let app = app_with(|s| crablink_server::Settings {
        trust_proxy: true,
        rate_limits: crablink_server::RateLimits {
            reports: Some("2/1h".parse().unwrap()),
            ..Default::default()
        },
        ..s
    })
    .await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    async fn from(app: &TestApp, ip: &str) -> Res {
        let body = json!({ "short_url": "crabl.ink/phish", "reason": "phishing" });
        let headers = [("x-forwarded-for", ip)];
        app.request("POST", "/api/reports", Some(body), &headers)
            .await
    }
    for _ in 0..2 {
        assert_eq!(from(&app, NZ_IP).await.status, StatusCode::NO_CONTENT);
    }

    let res = from(&app, NZ_IP).await;

    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS, "{}", res.text);
    assert_eq!(res.json()["kind"], "rate_limited");
    assert_eq!(res.header("retry-after"), "3600");
    assert_eq!(
        from(&app, "203.0.113.9").await.status,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn the_report_page_is_the_web_uis_report_form() {
    let app = app().await;

    let res = app.get("/report?short_url=crabl.ink/phish").await;

    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.text);
    assert_eq!(
        res.header("location"),
        "/app/report?short_url=crabl.ink/phish"
    );
    assert_eq!(app.get("/report").await.header("location"), "/app/report");
}

#[tokio::test]
async fn a_link_with_code_report_on_a_custom_domain_keeps_redirecting() {
    let app = app().await;
    app.approved_domain("go.example.com").await;

    let link = app
        .create(json!({ "target": "https://example.com/", "code": "report", "domain": "go.example.com" }))
        .await;

    assert_eq!(link["short_url"], "https://go.example.com/report");
    let res = app.visit_on("go.example.com", "/report", &[]).await;
    assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);
    assert_eq!(res.header("location"), "https://example.com/");
}

/// Reports `short_url` and returns the open report's id.
async fn reported(app: &TestApp, short_url: &str) -> i64 {
    let res = report(app, short_url, "phishing").await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let list = open_reports(app).await;
    list.as_array().unwrap().last().unwrap()["id"]
        .as_i64()
        .unwrap()
}

#[tokio::test]
async fn a_dismissed_report_leaves_the_open_list() {
    let app = app().await;
    app.create(json!({ "code": "fine", "target": "https://fine.example" }))
        .await;
    let id = reported(&app, "https://crabl.ink/fine").await;
    let admin = app.instance_admin().await;

    let path = format!("/api/admin/reports/{id}");
    let res = app.as_user("DELETE", &path, None, &admin).await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(open_reports(&app).await, json!([]));
    assert_eq!(app.get("/fine").await.status, StatusCode::FOUND);
    let res = app.as_user("DELETE", &path, None, &admin).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(res.json()["kind"], "not_found");
}

#[tokio::test]
async fn blocking_a_reported_link_closes_its_reports_for_good() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    reported(&app, "https://crabl.ink/phish").await;
    reported(&app, "crabl.ink/phish").await;
    let admin = app.instance_admin().await;
    let body = json!({ "short_url": "https://crabl.ink/phish" });

    let res = app
        .as_user("POST", "/api/admin/blocked-links", Some(body), &admin)
        .await;

    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(open_reports(&app).await, json!([]));
    let path = "/api/admin/blocked-links?short_url=https://crabl.ink/phish";
    app.as_user("DELETE", path, None, &admin).await;
    assert_eq!(open_reports(&app).await, json!([]));
}

#[tokio::test]
async fn an_instance_admin_disables_a_member_of_a_reported_links_workspace() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    reported(&app, "https://crabl.ink/phish").await;
    let user = open_reports(&app).await[0]["workspace_users"][0].clone();
    let admin = app.instance_admin().await;

    let path = format!("/api/admin/users/{}", user["id"]);
    let body = json!({ "disabled": true });
    let res = app.as_user("PATCH", &path, Some(body), &admin).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        app.sign_in_res(OWNER, PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );
    let list = open_reports(&app).await;
    assert_eq!(list[0]["workspace_users"][0]["disabled"], true, "{list}");
}

#[tokio::test]
async fn only_an_instance_admin_sees_or_dismisses_reports() {
    let app = app().await;
    app.create(json!({ "code": "fine", "target": "https://fine.example" }))
        .await;
    let id = reported(&app, "https://crabl.ink/fine").await;
    let cookie = &app.owner.cookie;

    let res = app.as_user("GET", "/api/admin/reports", None, cookie).await;

    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    let path = format!("/api/admin/reports/{id}");
    let res = app.as_user("DELETE", &path, None, cookie).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(open_reports(&app).await.as_array().unwrap().len(), 1);
}
