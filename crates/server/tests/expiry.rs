mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn future_expiry_is_returned_and_link_redirects() {
    let app = app_with_codes(&["Later01"]).await;
    let link = app
        .create(json!({ "target": "https://example.com", "expires_at": "2999-01-01T00:00:00Z" }))
        .await;

    let expires_at: chrono::DateTime<chrono::Utc> =
        link["expires_at"].as_str().unwrap().parse().unwrap();
    assert_eq!(expires_at.to_rfc3339(), "2999-01-01T00:00:00+00:00");
    assert_eq!(app.get("/Later01").await.status, StatusCode::FOUND);
}

#[tokio::test]
async fn expired_link_is_gone_and_not_counted() {
    let app = app_with_codes(&["Past001"]).await;
    app.create(json!({ "target": "https://example.com", "expires_at": "2000-01-01T00:00:00Z" }))
        .await;

    for _ in 0..2 {
        let res = app.get("/Past001").await;
        assert_eq!(res.status, StatusCode::GONE);
        assert!(!res.text.is_empty());
    }

    let link = app.api("GET", "/api/links/Past001", None).await.json();
    assert_eq!(link["click_count"], 0);
}

#[tokio::test]
async fn malformed_expiry_is_rejected() {
    let app = app().await;
    for bad in [json!("tomorrow"), json!("2030-01-01"), json!(12345)] {
        let res = app
            .api(
                "POST",
                "/api/links",
                Some(json!({ "target": "https://example.com", "expires_at": bad })),
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(res.json()["error"].is_string(), "{}", res.text);
    }
}
