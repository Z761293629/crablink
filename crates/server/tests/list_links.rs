mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

/// `GET` a list page, asserting `200`; returns `(codes, next_cursor)`.
async fn page(app: &TestApp, query: &str) -> (Vec<String>, Value) {
    let res = app.api("GET", &format!("/api/links{query}"), None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let body = res.json();
    let codes = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["code"].as_str().unwrap().to_string())
        .collect();
    (codes, body["next_cursor"].clone())
}

async fn create_codes(app: &TestApp, n: usize) -> Vec<String> {
    let mut codes = Vec::new();
    for i in 0..n {
        let code = format!("link{i:03}");
        app.create(json!({ "target": "https://example.com", "code": code }))
            .await;
        codes.push(code);
    }
    codes
}

#[tokio::test]
async fn links_come_back_newest_first_as_full_links() {
    let app = app().await;
    create_codes(&app, 3).await;

    let res = app.api("GET", "/api/links", None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let body = res.json();
    assert_eq!(body["next_cursor"], Value::Null);
    let first = &body["links"][0];
    assert_eq!(first["code"], "link002");
    assert_eq!(first["short_url"], "https://crab.link/link002");
    assert_eq!(first["click_count"], 0);

    let (codes, _) = page(&app, "").await;
    assert_eq!(codes, ["link002", "link001", "link000"]);
}

#[tokio::test]
async fn following_next_cursor_visits_every_link_exactly_once() {
    let app = app().await;
    let mut expected = create_codes(&app, 7).await;
    expected.reverse();

    let mut seen = Vec::new();
    let (codes, mut cursor) = page(&app, "?limit=3").await;
    seen.extend(codes);
    let mut pages = 1;
    while let Some(c) = cursor.as_str() {
        let (codes, next) = page(&app, &format!("?limit=3&cursor={c}")).await;
        assert!(codes.len() <= 3);
        seen.extend(codes);
        cursor = next;
        pages += 1;
    }

    assert_eq!(seen, expected);
    assert_eq!(pages, 3);
}

#[tokio::test]
async fn an_exactly_full_last_page_still_ends_with_null_cursor() {
    let app = app().await;
    create_codes(&app, 4).await;

    let (codes, cursor) = page(&app, "?limit=2").await;
    assert_eq!(codes, ["link003", "link002"]);
    let (codes, cursor) = page(
        &app,
        &format!("?limit=2&cursor={}", cursor.as_str().unwrap()),
    )
    .await;
    assert_eq!(codes, ["link001", "link000"]);
    assert_eq!(cursor, Value::Null);
}

#[tokio::test]
async fn default_limit_is_50() {
    let app = app().await;
    create_codes(&app, 51).await;

    let (codes, cursor) = page(&app, "").await;
    assert_eq!(codes.len(), 50);
    let (codes, cursor) = page(&app, &format!("?cursor={}", cursor.as_str().unwrap())).await;
    assert_eq!(codes, ["link000"]);
    assert_eq!(cursor, Value::Null);
}

#[tokio::test]
async fn limit_outside_1_to_200_or_a_bad_cursor_is_rejected() {
    let app = app().await;
    create_codes(&app, 1).await;

    for query in [
        "?limit=201",
        "?limit=0",
        "?limit=-1",
        "?limit=x",
        "?cursor=nonsense",
    ] {
        let res = app.api("GET", &format!("/api/links{query}"), None).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{query}: {}", res.text);
        assert!(res.json()["error"].is_string(), "{query}");
    }
    let (codes, _) = page(&app, "?limit=200").await;
    assert_eq!(codes, ["link000"]);
}

#[tokio::test]
async fn deleted_links_never_appear_but_expired_ones_do() {
    let app = app().await;
    create_codes(&app, 4).await;
    app.create(json!({
        "target": "https://example.com",
        "code": "expired",
        "expires_at": "2000-01-01T00:00:00Z",
    }))
    .await;
    for code in ["link001", "link003"] {
        app.api("DELETE", &format!("/api/links/{code}"), None).await;
    }

    let mut seen = Vec::new();
    let (codes, mut cursor) = page(&app, "?limit=1").await;
    seen.extend(codes);
    while let Some(c) = cursor.as_str() {
        let (codes, next) = page(&app, &format!("?limit=1&cursor={c}")).await;
        seen.extend(codes);
        cursor = next;
    }
    assert_eq!(seen, ["expired", "link002", "link000"]);
}

#[tokio::test]
async fn listing_needs_a_token() {
    let app = app().await;
    let res = app.api_with_token("GET", "/api/links", None, None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
}
