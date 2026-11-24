mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use crablink_server::TargetCheck;
use serde_json::json;

const MALICIOUS: &str = "https://malware.example/payload";

/// A fake Safe Browsing check that calls the Targets in `malicious` unsafe and the rest safe.
fn calling_unsafe(malicious: &'static [&'static str]) -> TargetCheck {
    Arc::new(move |target| Box::pin(async move { Ok(malicious.contains(&target.as_str())) }))
}

/// App whose Target check calls `MALICIOUS` unsafe.
async fn app_checking() -> TestApp {
    app_with(|s| crablink_server::Settings {
        target_check: calling_unsafe(&[MALICIOUS]),
        ..s
    })
    .await
}

fn assert_unsafe(res: &Res) {
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert_eq!(res.json()["kind"], "target_unsafe", "{}", res.text);
}

#[tokio::test]
async fn a_link_to_a_malicious_target_is_refused() {
    let app = app_checking().await;
    let res = app
        .api(
            "POST",
            "/links",
            Some(json!({ "target": MALICIOUS, "code": "bad" })),
        )
        .await;
    assert_unsafe(&res);
    assert_eq!(app.get("/bad").await.status, StatusCode::NOT_FOUND);
    app.create(json!({ "target": "https://example.com/fine" }))
        .await;
}

#[tokio::test]
async fn changing_a_target_to_a_malicious_one_is_refused() {
    let app = app_checking().await;
    app.create(json!({ "target": "https://example.com/fine", "code": "fine" }))
        .await;
    let res = app
        .api("PATCH", "/links/fine", Some(json!({ "target": MALICIOUS })))
        .await;
    assert_unsafe(&res);
    let res = app.get("/fine").await;
    assert_eq!(res.header("location"), "https://example.com/fine");
}

#[tokio::test]
async fn a_csv_import_skips_rows_with_malicious_targets_and_imports_the_rest() {
    let app = app_checking().await;
    let csv = format!("Code,Target\nbad,{MALICIOUS}\ngood,https://example.com/good\n");
    let req = Request::post(format!("/api/workspaces/{}/links.csv", app.owner.slug))
        .header(header::HOST, HOST)
        .header(header::AUTHORIZATION, format!("Bearer {}", app.owner.key))
        .header(header::CONTENT_TYPE, "text/csv")
        .body(Body::from(csv))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let rows = res.json();
    assert_eq!(rows[0]["line"], 2);
    assert_eq!(rows[0]["skipped"]["kind"], "target_unsafe", "{rows}");
    assert_eq!(rows[1]["short_url"], "https://crabl.ink/good", "{rows}");
    assert_eq!(app.get("/bad").await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get("/good").await.status, StatusCode::FOUND);
}

#[tokio::test]
async fn a_link_goes_ahead_when_the_check_fails() {
    let app = app_with(|s| crablink_server::Settings {
        target_check: Arc::new(|_| Box::pin(async { Err("Safe Browsing timed out".into()) })),
        ..s
    })
    .await;
    app.create(json!({ "target": MALICIOUS, "code": "through" }))
        .await;
    let res = app
        .api(
            "PATCH",
            "/links/through",
            Some(json!({ "target": "https://example.com/b" })),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn without_a_check_no_target_is_refused() {
    let app = app().await;
    app.create(json!({ "target": MALICIOUS })).await;
}
