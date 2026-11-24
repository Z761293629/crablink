mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

/// `POST /api/admin/blocked-links` blocking `short_url`, as the Instance Admin.
async fn block(app: &TestApp, short_url: &str) -> Res {
    let admin = app.instance_admin().await;
    let body = json!({ "short_url": short_url });
    app.as_user("POST", "/api/admin/blocked-links", Some(body), &admin)
        .await
}

/// `DELETE /api/admin/blocked-links?short_url=…`, as the Instance Admin.
async fn unblock(app: &TestApp, short_url: &str) -> Res {
    let admin = app.instance_admin().await;
    let path = format!("/api/admin/blocked-links?short_url={short_url}");
    app.as_user("DELETE", &path, None, &admin).await
}

async fn link(app: &TestApp, code: &str) -> Value {
    let res = app.api("GET", &format!("/links/{code}"), None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

#[tokio::test]
async fn a_blocked_link_tells_visitors_it_was_blocked_and_makes_no_click() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;

    let res = block(&app, "https://crabl.ink/phish").await;

    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let res = app.visit("/phish", &[("user-agent", BROWSER_UA)]).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert!(res.headers.get("location").is_none());
    assert!(res.header("content-type").starts_with("text/html"));
    assert!(res.text.contains("blocked for abuse"), "{}", res.text);
    assert!(!res.text.contains("evil.example"), "{}", res.text);
    assert_eq!(link(&app, "phish").await["click_count"], 0);
}

#[tokio::test]
async fn the_blocked_page_speaks_the_visitors_language() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    block(&app, "crabl.ink/phish").await;

    let res = app
        .visit("/phish", &[("accept-language", "zh-CN,zh;q=0.9,en;q=0.8")])
        .await;

    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert!(res.text.contains("lang=\"zh-CN\""), "{}", res.text);
    assert!(res.text.contains("封禁"), "{}", res.text);
}

#[tokio::test]
async fn an_unblocked_link_redirects_again() {
    let app = app().await;
    app.create(json!({ "code": "oops", "target": "https://fine.example" }))
        .await;
    block(&app, "https://crabl.ink/oops").await;

    let res = unblock(&app, "https://crabl.ink/oops").await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(app.get("/oops").await.status, StatusCode::FOUND);
    assert_eq!(link(&app, "oops").await["blocked"], false);
    let res = unblock(&app, "https://crabl.ink/oops").await;
    assert_eq!(res.json()["kind"], "link_not_found", "{}", res.text);
}

#[tokio::test]
async fn an_instance_admin_lists_blocked_links_with_their_workspace() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    app.create(json!({ "code": "fine", "target": "https://fine.example" }))
        .await;
    block(&app, "https://crabl.ink/phish").await;

    let admin = app.instance_admin().await;
    let res = app
        .as_user("GET", "/api/admin/blocked-links", None, &admin)
        .await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let list = res.json();
    assert_eq!(list.as_array().unwrap().len(), 1, "{}", res.text);
    assert_eq!(list[0]["short_url"], "https://crabl.ink/phish");
    assert_eq!(list[0]["target"], "https://evil.example");
    assert_eq!(list[0]["workspace_slug"], app.owner.slug.as_str());
}

#[tokio::test]
async fn only_an_instance_admin_blocks_or_unblocks() {
    let app = app().await;
    app.create(json!({ "code": "mine", "target": "https://fine.example" }))
        .await;
    let body = json!({ "short_url": "https://crabl.ink/mine" });
    let cookie = &app.owner.cookie;

    let res = app
        .as_user("POST", "/api/admin/blocked-links", Some(body), cookie)
        .await;

    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    let path = "/api/admin/blocked-links?short_url=https://crabl.ink/mine";
    let res = app.as_user("DELETE", path, None, cookie).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
}

#[tokio::test]
async fn blocking_a_short_url_without_a_live_link_is_not_found() {
    let app = app().await;
    app.create(json!({ "code": "gone", "target": "https://fine.example" }))
        .await;
    app.api("DELETE", "/links/gone", None).await;

    for short_url in ["https://crabl.ink/gone", "https://crabl.ink/never"] {
        let res = block(&app, short_url).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{short_url}");
        assert_eq!(res.json()["kind"], "link_not_found", "{}", res.text);
    }
    let res = block(&app, "not a short url").await;
    assert_eq!(res.json()["kind"], "bad_request", "{}", res.text);
}

#[tokio::test]
async fn its_workspace_sees_a_blocked_link_as_blocked() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    block(&app, "https://crabl.ink/phish").await;

    assert_eq!(link(&app, "phish").await["blocked"], true);
    let res = app.api("GET", "/links", None).await;
    assert_eq!(res.json()["links"][0]["blocked"], true, "{}", res.text);
}

#[tokio::test]
async fn changing_a_blocked_links_target_tags_or_expiry_is_refused() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    block(&app, "https://crabl.ink/phish").await;

    for body in [
        json!({ "target": "https://other.example" }),
        json!({ "tags": ["x"] }),
        json!({ "expires_at": null }),
    ] {
        let res = app.api("PATCH", "/links/phish", Some(body.clone())).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(res.json()["kind"], "link_blocked", "{}", res.text);
    }
    let link = link(&app, "phish").await;
    assert_eq!(link["target"], "https://evil.example");
    assert_eq!(link["tags"], json!([]));
}

#[tokio::test]
async fn a_blocked_link_has_no_qr_code() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    block(&app, "https://crabl.ink/phish").await;

    let res = app.api("GET", "/links/phish/qr", None).await;

    assert_eq!(res.json()["kind"], "link_blocked", "{}", res.text);
}

#[tokio::test]
async fn deleting_a_blocked_link_makes_it_a_deleted_link() {
    let app = app().await;
    app.create(json!({ "code": "phish", "target": "https://evil.example" }))
        .await;
    block(&app, "https://crabl.ink/phish").await;

    let res = app.api("DELETE", "/links/phish", None).await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(app.get("/phish").await.status, StatusCode::GONE);
    let res = app.api("GET", "/links/phish", None).await;
    assert_eq!(res.json()["kind"], "link_not_found", "{}", res.text);
    let res = unblock(&app, "https://crabl.ink/phish").await;
    assert_eq!(res.json()["kind"], "link_not_found", "{}", res.text);
}
