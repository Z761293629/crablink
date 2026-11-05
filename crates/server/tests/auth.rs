mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn api_rejects_missing_or_wrong_token() {
    let app = app().await;
    let body = || Some(json!({ "target": "https://example.com" }));

    for token in [None, Some("wrong"), Some("")] {
        let res = app
            .api_with_token("POST", "/api/links", body(), token)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "token {token:?}");
        assert!(res.json()["error"].is_string());
    }
}

#[tokio::test]
async fn redirects_need_no_token() {
    let app = app().await;
    let link = app.create(json!({ "target": "https://example.com" })).await;

    let res = app
        .get(&format!("/{}", link["code"].as_str().unwrap()))
        .await;

    assert_eq!(res.status, StatusCode::FOUND);
}
