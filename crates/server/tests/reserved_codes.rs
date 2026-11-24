mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;

#[tokio::test]
async fn a_reserved_code_is_refused_as_a_custom_code_on_the_default_domain() {
    let app = app_with_reserved_codes(&[], &["google", "paypal"]).await;

    let res = app
        .api(
            "POST",
            "/links",
            Some(json!({ "target": "https://example.com/", "code": "paypal" })),
        )
        .await;

    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert_eq!(res.json()["kind"], "code_reserved", "{}", res.text);
    assert_eq!(app.get("/paypal").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn csv_import_skips_a_row_asking_for_a_reserved_code() {
    let app = app_with_reserved_codes(&[], &["paypal"]).await;
    let req = Request::post(format!("/api/workspaces/{}/links.csv", app.owner.slug))
        .header(header::HOST, HOST)
        .header(header::AUTHORIZATION, format!("Bearer {}", app.owner.key))
        .header(header::CONTENT_TYPE, "text/csv")
        .body(Body::from(
            "Code,Domain,Target\npaypal,,https://example.com/a\nfine,,https://example.com/b\n",
        ))
        .unwrap();

    let rows = app.send(req).await.json();

    assert_eq!(rows[0]["skipped"]["kind"], "code_reserved", "{rows}");
    assert_eq!(rows[1]["short_url"], format!("{BASE_URL}/fine"), "{rows}");
}

#[tokio::test]
async fn reserved_codes_compare_case_sensitively() {
    let app = app_with_reserved_codes(&[], &["paypal"]).await;

    let link = app
        .create(json!({ "target": "https://example.com/", "code": "PayPal" }))
        .await;

    assert_eq!(link["short_url"], format!("{BASE_URL}/PayPal"));
}

#[tokio::test]
async fn a_generated_code_is_never_a_reserved_code() {
    let app = app_with_reserved_codes(&["Google1", "Fresh01"], &["Google1"]).await;

    let link = app
        .create(json!({ "target": "https://example.com/" }))
        .await;

    assert_eq!(link["code"], "Fresh01");
    assert_eq!(app.get("/Google1").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_custom_domain_may_use_a_reserved_code() {
    let app = app_with_reserved_codes(&[], &["paypal"]).await;
    app.approved_domain("go.example.com").await;

    let link = app
        .create(json!({ "target": "https://example.com/", "code": "paypal", "domain": "go.example.com" }))
        .await;

    assert_eq!(link["short_url"], "https://go.example.com/paypal");
    let res = app.visit_on("go.example.com", "/paypal", &[]).await;
    assert_eq!(res.header("location"), "https://example.com/");
}
