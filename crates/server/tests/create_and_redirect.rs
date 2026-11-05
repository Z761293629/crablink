mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn creating_a_link_returns_a_generated_code_and_short_url() {
    let app = app().await;

    let link = app
        .create(json!({ "target": "https://example.com/a" }))
        .await;

    let code = link["code"].as_str().unwrap();
    assert_eq!(code.len(), 7);
    assert!(code.chars().all(|c| c.is_ascii_alphanumeric()), "{code}");
    assert_eq!(link["short_url"], format!("{BASE_URL}/{code}"));
    assert_eq!(link["target"], "https://example.com/a");
    assert_eq!(link["click_count"], 0);
    assert!(link["expires_at"].is_null());
    assert!(link["created_at"].is_string());
    assert!(link["updated_at"].is_string());
}

#[tokio::test]
async fn opening_a_short_url_redirects_to_its_target_without_caching() {
    let app = app().await;
    let link = app
        .create(json!({ "target": "https://example.com/a?b=c" }))
        .await;

    let res = app
        .get(&format!("/{}", link["code"].as_str().unwrap()))
        .await;

    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.header("location"), "https://example.com/a?b=c");
    assert_eq!(res.header("cache-control"), "private, max-age=0");
}

#[tokio::test]
async fn unknown_code_and_root_are_not_found() {
    let app = app().await;

    let res = app.get("/nope123").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert!(!res.text.is_empty());

    assert_eq!(app.get("/").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn health_is_ok_without_auth() {
    assert_eq!(app().await.get("/health").await.status, StatusCode::OK);
}
