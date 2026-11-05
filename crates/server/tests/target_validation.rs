mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

async fn assert_rejected(app: &TestApp, target: &str) {
    let res = app
        .api("POST", "/api/links", Some(json!({ "target": target })))
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{target}");
    assert!(res.json()["error"].is_string(), "{target}: {}", res.text);
}

#[tokio::test]
async fn http_and_https_targets_are_accepted() {
    let app = app().await;
    for target in ["http://example.com/a", "https://example.com/a?b=c#d"] {
        assert_eq!(
            app.create(json!({ "target": target })).await["target"],
            target
        );
    }
}

#[tokio::test]
async fn non_http_schemes_and_relative_urls_are_rejected() {
    let app = app().await;
    for target in [
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "file:///etc/passwd",
        "/relative/path",
        "example.com/no-scheme",
        "",
    ] {
        assert_rejected(&app, target).await;
    }
}

#[tokio::test]
async fn targets_on_the_base_url_host_are_rejected_in_any_casing() {
    let app = app().await;
    for target in [
        "https://crab.link/abc",
        "http://CRAB.LINK/abc",
        "https://Crab.Link:8443/",
        "https://user@crab.link/",
    ] {
        assert_rejected(&app, target).await;
    }
    // A different host that merely contains the base host is fine.
    app.create(json!({ "target": "https://crab.link.example.com/" }))
        .await;
}

#[tokio::test]
async fn targets_longer_than_2048_characters_are_rejected() {
    let app = app().await;
    let prefix = "https://example.com/";
    let at_limit = format!("{prefix}{}", "a".repeat(2048 - prefix.len()));
    app.create(json!({ "target": at_limit })).await;

    assert_rejected(&app, &format!("{at_limit}a")).await;
}

#[tokio::test]
async fn targets_are_not_fetched_so_unresolvable_hosts_are_accepted() {
    let app = app().await;
    app.create(json!({ "target": "https://does-not-exist.invalid/x" }))
        .await;
}

/// The URL parser silently strips these, so they would be stored raw and break the redirect.
const WHITESPACE_OR_CONTROL: &[&str] = &[
    "https://example.com/a\nb",
    "https://example.com/a\tb",
    "https://example.com/a\rb",
    " https://example.com/a",
    "https://example.com/a ",
    "https://example.com/a b",
    "https://example.com/a\u{7f}b",
    "https://example.com/a\u{0}b",
];

#[tokio::test]
async fn targets_with_whitespace_or_control_characters_are_rejected_on_create() {
    let app = app().await;
    for target in WHITESPACE_OR_CONTROL {
        assert_rejected(&app, target).await;
    }
}

#[tokio::test]
async fn targets_with_whitespace_or_control_characters_are_rejected_on_edit() {
    let app = app().await;
    let link = app
        .create(json!({ "target": "https://example.com/ok" }))
        .await;
    let path = format!("/api/links/{}", link["code"].as_str().unwrap());
    for target in WHITESPACE_OR_CONTROL {
        let res = app
            .api("PATCH", &path, Some(json!({ "target": target })))
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{target:?}");
    }
    // The Link still redirects to its original Target.
    let res = app
        .get(&format!("/{}", link["code"].as_str().unwrap()))
        .await;
    assert_eq!(res.header("location"), "https://example.com/ok");
}
