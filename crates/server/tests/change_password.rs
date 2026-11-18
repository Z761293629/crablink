mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

const NEW_PASSWORD: &str = "tr0ub4dor & 3";

fn change(current: &str, new: &str) -> Option<Value> {
    Some(json!({ "current_password": current, "new_password": new }))
}

#[tokio::test]
async fn changing_password_needs_the_current_one() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    let body = change("wrong one", NEW_PASSWORD);
    let res = app.as_user("POST", "/api/me/password", body, &cookie).await;

    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(res.json()["kind"], "wrong_password");
    app.sign_in("ada@example.com", PASSWORD).await;
}

#[tokio::test]
async fn a_new_password_replaces_the_old_one_and_ends_other_sessions() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let laptop = app.sign_in("ada@example.com", PASSWORD).await;
    let phone = app.sign_in("ada@example.com", PASSWORD).await;

    let body = change(PASSWORD, NEW_PASSWORD);
    let res = app.as_user("POST", "/api/me/password", body, &laptop).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let me = |cookie| app.as_user("GET", "/api/me", None, cookie);
    assert_eq!(me(&laptop).await.status, StatusCode::OK);
    assert_eq!(me(&phone).await.status, StatusCode::UNAUTHORIZED);
    let res = app.sign_in_res("ada@example.com", PASSWORD).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    app.sign_in("ada@example.com", NEW_PASSWORD).await;
}

#[tokio::test]
async fn a_new_password_must_be_8_characters() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    let body = change(PASSWORD, "short");
    let res = app.as_user("POST", "/api/me/password", body, &cookie).await;

    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert_eq!(res.json()["kind"], "password_too_short");
}

#[tokio::test]
async fn changing_password_from_a_foreign_origin_is_403() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    let headers = [
        ("cookie", cookie.as_str()),
        ("origin", "https://evil.example"),
    ];
    let body = change(PASSWORD, NEW_PASSWORD);
    let res = app
        .request("POST", "/api/me/password", body, &headers)
        .await;

    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    app.sign_in("ada@example.com", PASSWORD).await;
}
