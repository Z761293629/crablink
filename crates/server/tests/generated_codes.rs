mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn same_target_twice_gives_two_different_codes() {
    let app = app().await;
    let body = json!({ "target": "https://example.com" });

    let a = app.create(body.clone()).await;
    let b = app.create(body).await;

    assert_ne!(a["code"], b["code"]);
}

#[tokio::test]
async fn a_colliding_generated_code_is_retried() {
    let app = app_with_codes(&["Taken01", "Taken01", "Fresh01"]).await;
    app.create(json!({ "target": "https://example.com/1" }))
        .await;

    let link = app
        .create(json!({ "target": "https://example.com/2" }))
        .await;

    assert_eq!(link["code"], "Fresh01");
    assert_eq!(
        app.get("/Fresh01").await.header("location"),
        "https://example.com/2"
    );
    assert_eq!(
        app.get("/Taken01").await.header("location"),
        "https://example.com/1"
    );
}

#[tokio::test]
async fn the_fifth_attempt_can_still_succeed() {
    // First create takes Taken01; the second collides four times, then gets Fresh01.
    let app = app_with_codes(&[
        "Taken01", "Taken01", "Taken01", "Taken01", "Taken01", "Fresh01",
    ])
    .await;
    app.create(json!({ "target": "https://example.com/1" }))
        .await;

    let link = app
        .create(json!({ "target": "https://example.com/2" }))
        .await;

    assert_eq!(link["code"], "Fresh01");
}

#[tokio::test]
async fn gives_up_after_five_colliding_codes() {
    let app = app_with_codes(&["Taken01"; 6]).await;
    app.create(json!({ "target": "https://example.com/1" }))
        .await;

    let res = app
        .api(
            "POST",
            "/api/links",
            Some(json!({ "target": "https://example.com/2" })),
        )
        .await;

    assert_eq!(res.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(res.json()["error"].is_string());
}
