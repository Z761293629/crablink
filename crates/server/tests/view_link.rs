mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn a_new_link_can_be_viewed_with_zero_clicks() {
    let app = app_with_codes(&["View001"]).await;
    let created = app
        .create(json!({ "target": "https://example.com/v" }))
        .await;

    let res = app.api("GET", "/api/links/View001", None).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let link = res.json();
    assert_eq!(link["code"], "View001");
    assert_eq!(link["short_url"], "https://crab.link/View001");
    assert_eq!(link["target"], "https://example.com/v");
    assert_eq!(link["click_count"], 0);
    assert!(link["expires_at"].is_null());
    assert_eq!(link["created_at"], created["created_at"]);
    assert_eq!(link["updated_at"], created["updated_at"]);
}

#[tokio::test]
async fn viewing_an_unknown_code_is_not_found() {
    let app = app().await;

    let res = app.api("GET", "/api/links/nope123", None).await;

    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert!(res.json()["error"].is_string());
}

#[tokio::test]
async fn viewing_a_link_needs_a_token() {
    let app = app_with_codes(&["View001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    for token in [None, Some("wrong")] {
        let res = app
            .api_with_token("GET", "/api/links/View001", None, token)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "token {token:?}");
    }
}
