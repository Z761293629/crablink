mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

async fn patch(app: &TestApp, code: &str, body: Value) -> Res {
    app.api("PATCH", &format!("/api/links/{code}"), Some(body))
        .await
}

#[tokio::test]
async fn changed_target_is_used_by_redirect() {
    let app = app_with_codes(&["Edit001"]).await;
    let before = app.create(json!({ "target": "https://old.example" })).await;

    let res = patch(&app, "Edit001", json!({ "target": "https://new.example/" })).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let link = res.json();
    assert_eq!(link["code"], "Edit001");
    assert_eq!(link["target"], "https://new.example/");
    assert_ne!(link["updated_at"], before["updated_at"]);
    assert_eq!(link["created_at"], before["created_at"]);
    let res = app.get("/Edit001").await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.header("location"), "https://new.example/");
}

#[tokio::test]
async fn invalid_target_is_rejected_and_link_unchanged() {
    let app = app_with_codes(&["Edit002"]).await;
    app.create(json!({ "target": "https://old.example" })).await;

    for bad in ["javascript:alert(1)", "https://crab.link/loop", "not a url"] {
        let res = patch(&app, "Edit002", json!({ "target": bad })).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(res.json()["error"].is_string());
    }
    assert_eq!(
        app.get("/Edit002").await.header("location"),
        "https://old.example"
    );
}

#[tokio::test]
async fn unknown_or_deleted_link_is_not_found() {
    let app = app_with_codes(&["Gone001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;
    app.api("DELETE", "/api/links/Gone001", None).await;

    for code in ["Gone001", "Nope001"] {
        let res = patch(&app, code, json!({ "target": "https://new.example" })).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{code}");
        assert!(res.json()["error"].is_string());
    }
}

#[tokio::test]
async fn patch_needs_token() {
    let app = app_with_codes(&["Auth001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    let body = Some(json!({ "target": "https://new.example" }));
    let res = app
        .api_with_token("PATCH", "/api/links/Auth001", body, None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn omitted_expiry_is_unchanged_and_null_clears_it() {
    let app = app_with_codes(&["Exp0001"]).await;
    app.create(json!({ "target": "https://example.com", "expires_at": "2999-01-01T00:00:00Z" }))
        .await;

    let link = patch(&app, "Exp0001", json!({ "target": "https://new.example/" }))
        .await
        .json();
    assert!(link["expires_at"].is_string(), "{link}");

    let link = patch(&app, "Exp0001", json!({ "expires_at": null }))
        .await
        .json();
    assert!(link["expires_at"].is_null(), "{link}");
    assert_eq!(link["target"], "https://new.example/");
}

#[tokio::test]
async fn moving_expiry_pauses_and_resumes_redirect() {
    let app = app_with_codes(&["Exp0002"]).await;
    app.create(json!({ "target": "https://example.com", "expires_at": "2000-01-01T00:00:00Z" }))
        .await;
    assert_eq!(app.get("/Exp0002").await.status, StatusCode::GONE);

    let res = patch(
        &app,
        "Exp0002",
        json!({ "expires_at": "2999-01-01T00:00:00Z" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(app.get("/Exp0002").await.status, StatusCode::FOUND);

    patch(
        &app,
        "Exp0002",
        json!({ "expires_at": "2000-01-01T00:00:00Z" }),
    )
    .await;
    assert_eq!(app.get("/Exp0002").await.status, StatusCode::GONE);
}

#[tokio::test]
async fn malformed_expiry_is_rejected() {
    let app = app_with_codes(&["Exp0003"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    let res = patch(&app, "Exp0003", json!({ "expires_at": "tomorrow" })).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert!(res.json()["error"].is_string());
}
