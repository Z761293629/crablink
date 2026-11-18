mod common;

use axum::http::StatusCode;
use chrono::TimeDelta;
use common::*;
use serde_json::{Value, json};

fn keys_path(app: &TestApp) -> String {
    format!("/api/workspaces/{}/api-keys", app.owner.slug)
}

async fn list_keys(app: &TestApp) -> Value {
    let res = app
        .as_user("GET", &keys_path(app), None, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

#[tokio::test]
async fn an_owner_sees_the_secret_once_and_then_only_its_prefix() {
    let app = app().await;

    let body = Some(json!({ "name": "CI" }));
    let res = app
        .as_user("POST", &keys_path(&app), body, &app.owner.cookie)
        .await;

    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let created = res.json();
    let secret = created["secret"].as_str().unwrap();
    let prefix = created["prefix"].as_str().unwrap();
    assert!(secret.len() >= 32, "{secret}");
    assert!(secret.starts_with(prefix) && prefix.len() < secret.len());
    assert_eq!(created["name"], "CI");
    assert!(created["created_at"].is_string());
    assert!(created["last_used_at"].is_null());

    let keys = list_keys(&app).await;
    // The harness's own key comes first, then this one.
    let listed = &keys[1];
    assert_eq!(listed["id"], created["id"]);
    assert_eq!(listed["name"], "CI");
    assert_eq!(listed["prefix"], prefix);
    assert!(listed.get("secret").is_none(), "{listed}");
    assert!(!keys.to_string().contains(secret));
}

#[tokio::test]
async fn using_an_api_key_records_when_it_was_last_used() {
    let app = app().await;
    assert!(list_keys(&app).await[0]["last_used_at"].is_null());

    app.advance(TimeDelta::hours(1));
    app.api("GET", "/links", None).await;

    let used = list_keys(&app).await[0]["last_used_at"].clone();
    let created = list_keys(&app).await[0]["created_at"].clone();
    assert!(used.is_string(), "{used}");
    assert_ne!(used, created);
}

#[tokio::test]
async fn a_revoked_api_key_gets_401_at_once() {
    let app = app().await;
    let id = list_keys(&app).await[0]["id"].clone();

    let path = format!("{}/{id}", keys_path(&app));
    let res = app.as_user("DELETE", &path, None, &app.owner.cookie).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let res = app.api("GET", "/links", None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    assert_eq!(res.json()["kind"], "unauthorized");
    assert_eq!(list_keys(&app).await, json!([]));
}

#[tokio::test]
async fn revoking_an_unknown_or_another_workspaces_key_is_404() {
    let app = app().await;
    let eve = app.account("eve@example.com").await;
    let eves_keys = format!("/api/workspaces/{}/api-keys", eve.slug);
    let res = app.as_user("GET", &eves_keys, None, &eve.cookie).await;
    let eves_id = res.json()[0]["id"].clone();

    for id in [eves_id, json!(999_999)] {
        let path = format!("{}/{id}", keys_path(&app));
        let res = app.as_user("DELETE", &path, None, &app.owner.cookie).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{id}: {}", res.text);
    }
    let res = app.bearer("GET", "/api/me", None, Some(&eve.key)).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn an_api_key_keeps_working_after_its_creator_is_gone() {
    let app = app().await;

    crablink_server::set_user_disabled(&app.pool, OWNER, true)
        .await
        .unwrap();
    let res = app.api("GET", "/links", None).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn api_keys_are_managed_by_owners_not_by_api_keys_or_other_workspaces() {
    let app = app().await;
    let eve = app.account("eve@example.com").await;
    let path = keys_path(&app);
    let body = || Some(json!({ "name": "sneaky" }));

    for res in [
        app.bearer("GET", &path, None, Some(&app.owner.key)).await,
        app.bearer("POST", &path, body(), Some(&app.owner.key))
            .await,
        app.as_user("GET", &path, None, &eve.cookie).await,
        app.as_user("POST", &path, body(), &eve.cookie).await,
    ] {
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
        assert_eq!(res.json()["kind"], "forbidden", "{}", res.text);
    }
}

#[tokio::test]
async fn an_api_key_needs_a_name() {
    let app = app().await;

    for name in ["", "   ", &"x".repeat(101)] {
        let body = Some(json!({ "name": name }));
        let res = app
            .as_user("POST", &keys_path(&app), body, &app.owner.cookie)
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{name:?}");
        assert_eq!(res.json()["kind"], "bad_request", "{}", res.text);
    }
}
