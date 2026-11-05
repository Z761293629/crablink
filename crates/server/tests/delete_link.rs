mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn deleting_a_link_makes_its_short_url_gone() {
    let app = app_with_codes(&["Gone001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    let res = app.api("DELETE", "/api/links/Gone001", None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    assert_eq!(app.get("/Gone001").await.status, StatusCode::GONE);
}

#[tokio::test]
async fn a_deleted_link_is_not_found_by_the_api() {
    let app = app_with_codes(&["Gone001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;
    app.api("DELETE", "/api/links/Gone001", None).await;

    let res = app.api("GET", "/api/links/Gone001", None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
}

#[tokio::test]
async fn deleting_twice_or_deleting_an_unknown_code_is_404() {
    let app = app_with_codes(&["Gone001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;
    app.api("DELETE", "/api/links/Gone001", None).await;

    for path in ["/api/links/Gone001", "/api/links/nope123"] {
        let res = app.api("DELETE", path, None).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{path}: {}", res.text);
    }
}

#[tokio::test]
async fn a_deleted_code_can_never_be_reused() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com", "code": "old-code" }))
        .await;
    app.api("DELETE", "/api/links/old-code", None).await;

    let res = app
        .api(
            "POST",
            "/api/links",
            Some(json!({ "target": "https://example.org", "code": "old-code" })),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(app.get("/old-code").await.status, StatusCode::GONE);
}

#[tokio::test]
async fn deleting_needs_a_token() {
    let app = app_with_codes(&["Keep001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    let res = app
        .api_with_token("DELETE", "/api/links/Keep001", None, None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    assert_eq!(app.get("/Keep001").await.status, StatusCode::FOUND);
}
