mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn creating_with_a_custom_code_uses_it_and_redirects() {
    let app = app().await;

    let link = app
        .create(json!({ "target": "https://example.com/meetup", "code": "rust-meetup" }))
        .await;

    assert_eq!(link["code"], "rust-meetup");
    assert_eq!(link["short_url"], format!("{BASE_URL}/rust-meetup"));
    let res = app.get("/rust-meetup").await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.header("location"), "https://example.com/meetup");
}

#[tokio::test]
async fn malformed_custom_codes_are_rejected() {
    let app = app().await;
    let too_long = "a".repeat(65);
    for code in [
        "ab",
        too_long.as_str(),
        "has space",
        "dot.ted",
        "slash/y",
        "ümlaut",
        "",
    ] {
        let res = app
            .api(
                "POST",
                "/api/links",
                Some(json!({ "target": "https://example.com/", "code": code })),
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "code {code:?}");
        assert!(res.json()["error"].is_string(), "{}", res.text);
    }
    // The boundaries are accepted.
    app.create(json!({ "target": "https://example.com/", "code": "a_-" }))
        .await;
    app.create(json!({ "target": "https://example.com/", "code": "Z".repeat(64) }))
        .await;
}

#[tokio::test]
async fn reserved_words_are_rejected_in_any_casing() {
    let app = app().await;
    for code in ["api", "API", "Admin", "HEALTH", "health"] {
        let res = app
            .api(
                "POST",
                "/api/links",
                Some(json!({ "target": "https://example.com/", "code": code })),
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "code {code:?}");
        assert!(res.json()["error"].is_string(), "{}", res.text);
    }
    assert_eq!(app.get("/health").await.status, StatusCode::OK);
}

#[tokio::test]
async fn codes_differing_only_in_case_are_distinct_links() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com/upper", "code": "Meetup" }))
        .await;
    app.create(json!({ "target": "https://example.com/lower", "code": "meetup" }))
        .await;

    assert_eq!(
        app.get("/Meetup").await.header("location"),
        "https://example.com/upper"
    );
    assert_eq!(
        app.get("/meetup").await.header("location"),
        "https://example.com/lower"
    );
    assert_eq!(app.get("/MEETUP").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reusing_a_taken_custom_code_conflicts_and_keeps_the_original() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com/first", "code": "taken" }))
        .await;

    let res = app
        .api(
            "POST",
            "/api/links",
            Some(json!({ "target": "https://example.com/second", "code": "taken" })),
        )
        .await;

    assert_eq!(res.status, StatusCode::CONFLICT);
    assert!(res.json()["error"].is_string(), "{}", res.text);
    assert_eq!(
        app.get("/taken").await.header("location"),
        "https://example.com/first"
    );
}
