mod common;

use axum::http::StatusCode;
use chrono::TimeDelta;
use common::*;
use crablink_server::ApiError;
use serde_json::json;

#[tokio::test]
async fn a_signed_in_user_sees_themselves_at_me() {
    let app = app().await;
    app.create_user("Ada@Example.com").await;

    let cookie = app.sign_in("ada@example.com", PASSWORD).await;
    let res = app.as_user("GET", "/api/me", None, &cookie).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["user"]["email"], "Ada@Example.com");
    assert_eq!(res.json()["user"]["instance_admin"], false);
}

#[tokio::test]
async fn the_session_cookie_is_http_only_secure_and_lax() {
    let app = app().await;
    app.create_user("ada@example.com").await;

    let res = app.sign_in_res("ada@example.com", PASSWORD).await;

    let set_cookie = res.header("set-cookie");
    assert!(set_cookie.starts_with("crablink_session="), "{set_cookie}");
    for attr in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/", "Max-Age="] {
        assert!(set_cookie.contains(attr), "{attr} missing: {set_cookie}");
    }
}

#[tokio::test]
async fn a_wrong_email_or_password_gets_401_without_saying_which() {
    let app = app().await;
    app.create_user("ada@example.com").await;

    let wrong_password = app.sign_in_res("ada@example.com", "not the password").await;
    let wrong_email = app.sign_in_res("bob@example.com", PASSWORD).await;

    for res in [&wrong_password, &wrong_email] {
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
        assert_eq!(res.json()["kind"], "invalid_credentials");
        assert_eq!(res.header("set-cookie"), "");
    }
    assert_eq!(wrong_password.text, wrong_email.text);
}

#[tokio::test]
async fn a_disabled_user_cant_sign_in() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    crablink_server::set_user_disabled(&app.pool, "ada@example.com", true)
        .await
        .unwrap();

    let res = app.sign_in_res("ada@example.com", PASSWORD).await;

    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    assert_eq!(res.json()["kind"], "invalid_credentials");
}

#[tokio::test]
async fn disabling_an_unknown_email_is_user_not_found() {
    let app = app().await;

    let err = crablink_server::set_user_disabled(&app.pool, "nobody@example.com", true)
        .await
        .unwrap_err();

    assert!(matches!(err, ApiError::UserNotFound), "{err}");
}

#[tokio::test]
async fn a_disabled_users_sessions_stop_working() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    crablink_server::set_user_disabled(&app.pool, "ada@example.com", true)
        .await
        .unwrap();
    let res = app.as_user("GET", "/api/me", None, &cookie).await;

    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
}

#[tokio::test]
async fn me_without_a_session_is_401() {
    let app = app().await;

    for cookie in ["", "crablink_session=", "crablink_session=forged"] {
        let res = app.as_user("GET", "/api/me", None, cookie).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{cookie:?}");
        assert_eq!(res.json()["kind"], "unauthorized", "{}", res.text);
    }
}

#[tokio::test]
async fn a_signed_out_session_is_rejected() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    let res = app.as_user("DELETE", "/api/session", None, &cookie).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert!(res.header("set-cookie").contains("Max-Age=0"));

    let res = app.as_user("GET", "/api/me", None, &cookie).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
}

#[tokio::test]
async fn a_session_expires_after_30_days() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    app.advance(TimeDelta::days(30) - TimeDelta::seconds(1));
    let res = app.as_user("GET", "/api/me", None, &cookie).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);

    app.advance(TimeDelta::seconds(1));
    let res = app.as_user("GET", "/api/me", None, &cookie).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
}

#[tokio::test]
async fn a_cookie_request_that_changes_state_needs_this_hosts_origin() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    for origin in [
        Some("https://evil.example"),
        Some("https://crabl.ink.evil.example"),
        Some("http://crabl.ink:8080"),
        Some("null"),
        None,
    ] {
        let mut headers = vec![("cookie", cookie.as_str())];
        headers.extend(origin.map(|o| ("origin", o)));
        let res = app.request("DELETE", "/api/session", None, &headers).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{origin:?}");
        assert_eq!(res.json()["kind"], "origin_mismatch", "{}", res.text);
    }

    // None of those signed Ada out, and a read needs no Origin.
    let res = app
        .request("GET", "/api/me", None, &[("cookie", cookie.as_str())])
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn signing_in_needs_this_hosts_origin() {
    let app = app().await;
    app.create_user("ada@example.com").await;
    let body = json!({ "email": "ada@example.com", "password": PASSWORD });

    for origin in [Some("https://evil.example"), None] {
        let headers: Vec<_> = origin.map(|o| ("origin", o)).into_iter().collect();
        let res = app
            .request("POST", "/api/session", Some(body.clone()), &headers)
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{origin:?}");
        assert_eq!(res.json()["kind"], "origin_mismatch", "{}", res.text);
        assert_eq!(res.header("set-cookie"), "", "{origin:?}");
    }
}

#[tokio::test]
async fn emails_are_unique_ignoring_case() {
    let app = app().await;
    app.create_user("ada@example.com").await;

    let err = crablink_server::create_user(&app.pool, "ADA@example.com", PASSWORD, false)
        .await
        .unwrap_err();

    assert!(matches!(err, ApiError::EmailTaken), "{err}");
}

#[tokio::test]
async fn admin_create_makes_an_instance_admin_who_can_sign_in() {
    let app = app().await;

    crablink_server::create_user(&app.pool, " root@example.com ", PASSWORD, true)
        .await
        .unwrap();
    let cookie = app.sign_in("root@example.com", PASSWORD).await;
    let res = app.as_user("GET", "/api/me", None, &cookie).await;

    assert_eq!(res.json()["user"]["email"], "root@example.com");
    assert_eq!(res.json()["user"]["instance_admin"], true);
}

#[tokio::test]
async fn users_need_a_plausible_email_and_an_8_character_password() {
    let app = app().await;

    for email in ["", "ada", "@example.com", "ada@", "a da@example.com"] {
        let err = crablink_server::create_user(&app.pool, email, PASSWORD, true)
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::InvalidEmail), "{email:?}: {err}");
    }
    let err = crablink_server::create_user(&app.pool, "ada@example.com", "7 chars", true)
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::PasswordTooShort), "{err}");
}
