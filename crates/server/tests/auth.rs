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
async fn unknown_api_paths_and_methods_also_need_a_token() {
    let app = app().await;

    for (method, path) in [
        ("GET", "/api"),
        ("GET", "/api/"),
        ("GET", "/api/nope"),
        ("GET", "/api/links/abc/extra"),
        ("PUT", "/api/links"),
        ("POST", "/api/links/abc"),
    ] {
        let res = app.api_with_token(method, path, None, None).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {path}");
        assert!(res.json()["error"].is_string(), "{method} {path}");
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
