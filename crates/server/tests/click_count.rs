mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

async fn clicks(app: &TestApp, code: &str) -> serde_json::Value {
    let res = app.api("GET", &format!("/api/links/{code}"), None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()["click_count"].clone()
}

#[tokio::test]
async fn each_redirect_counts_one_click() {
    let app = app_with_codes(&["Click01", "Other01"]).await;
    app.create(json!({ "target": "https://example.com/1" }))
        .await;
    app.create(json!({ "target": "https://example.com/2" }))
        .await;

    for _ in 0..3 {
        assert_eq!(app.get("/Click01").await.status, StatusCode::FOUND);
    }

    assert_eq!(clicks(&app, "Click01").await, 3);
    assert_eq!(clicks(&app, "Other01").await, 0);
}

#[tokio::test]
async fn unknown_codes_do_not_count_clicks() {
    let app = app_with_codes(&["Click01"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    // Codes are case-sensitive, so `click01` is unknown.
    for path in ["/nope123", "/click01"] {
        assert_eq!(app.get(path).await.status, StatusCode::NOT_FOUND);
    }

    assert_eq!(clicks(&app, "Click01").await, 0);
}
