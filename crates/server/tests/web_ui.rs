//! The web UI at `/app`. These pass with or without a web build (`web/dist`):
//! either way every SPA route gets an HTML page, the built index or a note
//! that the UI wasn't built.

mod common;

use axum::http::StatusCode;
use common::*;

#[tokio::test]
async fn every_spa_route_under_app_gets_an_html_page() {
    let app = app().await;

    for path in ["/app", "/app/", "/app/links/some-route"] {
        let res = app.get(path).await;

        assert_eq!(res.status, StatusCode::OK, "{path}: {}", res.text);
        assert!(
            res.header("content-type").starts_with("text/html"),
            "{path}: {}",
            res.header("content-type")
        );
        assert!(
            res.text.contains(r#"<div id="root">"#) || res.text.contains("wasn't built"),
            "{path}: {}",
            res.text
        );
    }
}
