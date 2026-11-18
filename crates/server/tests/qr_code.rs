mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn a_links_qr_code_is_svg_by_default() {
    let app = app_with_codes(&["Qr00001"]).await;
    app.create(json!({ "target": "https://example.com/qr" }))
        .await;

    let res = app.api("GET", "/links/Qr00001/qr", None).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.header("content-type"), "image/svg+xml");
    assert!(res.text.contains("<svg"), "{}", res.text);
}

#[tokio::test]
async fn a_png_qr_code_decodes_to_exactly_the_short_url() {
    let app = app_with_codes(&["Qr00002"]).await;
    app.create(json!({ "target": "https://example.com/qr" }))
        .await;

    let res = app.api("GET", "/links/Qr00002/qr?format=png", None).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.header("content-type"), "image/png");
    assert_eq!(decode_png(&res.bytes), "https://crab.link/Qr00002");
}

#[tokio::test]
async fn an_unknown_or_deleted_code_has_no_qr_code() {
    let app = app_with_codes(&["Gone001"]).await;
    app.create(json!({ "target": "https://example.com" })).await;
    app.api("DELETE", "/links/Gone001", None).await;

    for code in ["nope123", "Gone001"] {
        let res = app.api("GET", &format!("/links/{code}/qr"), None).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{code}: {}", res.text);
        assert_eq!(res.json()["kind"], "link_not_found", "{}", res.text);
        assert_eq!(res.json()["error"], "link not found");
    }
}

#[tokio::test]
async fn an_expired_link_has_no_qr_code() {
    let app = app_with_codes(&["Old0001"]).await;
    app.create(json!({ "target": "https://example.com", "expires_at": "2020-01-01T00:00:00Z" }))
        .await;

    let res = app.api("GET", "/links/Old0001/qr", None).await;

    assert_eq!(res.status, StatusCode::GONE, "{}", res.text);
    assert_eq!(res.json()["kind"], "link_expired", "{}", res.text);
    assert_eq!(res.json()["error"], "link has expired");
}

#[tokio::test]
async fn an_unknown_format_is_a_bad_request() {
    let app = app_with_codes(&["Qr00003"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    let res = app.api("GET", "/links/Qr00003/qr?format=gif", None).await;

    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert_eq!(res.json()["kind"], "bad_request", "{}", res.text);
    assert!(res.json()["error"].is_string());
}

#[tokio::test]
async fn a_qr_code_needs_a_credential() {
    let app = app_with_codes(&["Qr00004"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    for key in [None, Some("wrong")] {
        let res = app
            .api_with_key("GET", "/links/Qr00004/qr", None, key)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "key {key:?}");
        assert_eq!(res.json()["kind"], "unauthorized", "{}", res.text);
    }
}

/// The text of the one QR code in a PNG.
fn decode_png(png: &[u8]) -> String {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("a valid PNG")
        .into_luma8();
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
        img.width() as usize,
        img.height() as usize,
        |x, y| img.get_pixel(x as u32, y as u32).0[0],
    );
    let grids = prepared.detect_grids();
    assert_eq!(grids.len(), 1, "expected one QR code");
    grids[0].decode().expect("a decodable QR code").1
}
