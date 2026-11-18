mod common;

use axum::http::StatusCode;
use chrono::{DateTime, TimeDelta, Utc};
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

    let link = app.api("GET", "/links/Past001", None).await.json();
    assert_eq!(link["click_count"], 0);
}

#[tokio::test]
async fn malformed_expiry_is_rejected() {
    let app = app().await;
    for bad in [json!("tomorrow"), json!("2030-01-01"), json!(12345)] {
        let res = app
            .api(
                "POST",
                "/links",
                Some(json!({ "target": "https://example.com", "expires_at": bad })),
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(res.json()["error"].is_string(), "{}", res.text);
        assert_eq!(res.json()["kind"], "bad_request", "{}", res.text);
    }
}

#[tokio::test]
async fn a_link_expires_by_the_apps_clock() {
    let app = app_with_codes(&["Soon001"]).await;
    let expires_at = Utc::now() + TimeDelta::hours(1);
    app.create(json!({ "target": "https://example.com", "expires_at": expires_at }))
        .await;
    assert_eq!(app.get("/Soon001").await.status, StatusCode::FOUND);

    app.advance(TimeDelta::hours(2));

    assert_eq!(app.get("/Soon001").await.status, StatusCode::GONE);
    let res = app.api("GET", "/links/Soon001/qr", None).await;
    assert_eq!(res.json()["kind"], "link_expired", "{}", res.text);
}

#[tokio::test]
async fn link_timestamps_come_from_the_apps_clock() {
    let app = app_with_codes(&["Clock01"]).await;
    // The clock started at the real time, a moment before this.
    let later = Utc::now() + TimeDelta::days(364);
    app.advance(TimeDelta::days(365));

    let link = app.create(json!({ "target": "https://example.com" })).await;
    app.advance(TimeDelta::days(1));
    let res = app
        .api(
            "PATCH",
            "/links/Clock01",
            Some(json!({ "target": "https://example.org" })),
        )
        .await;

    let at =
        |field: &str| -> DateTime<Utc> { res.json()[field].as_str().unwrap().parse().unwrap() };
    let created: DateTime<Utc> = link["created_at"].as_str().unwrap().parse().unwrap();
    assert!(created >= later, "{link}");
    assert_eq!(at("created_at"), created);
    assert!(
        at("updated_at") >= later + TimeDelta::days(1),
        "{}",
        res.text
    );
}
