mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

const ROOT: &str = "root@example.com";

/// Creates and signs in an Instance Admin; returns their cookie.
async fn instance_admin(app: &TestApp) -> String {
    crablink_server::create_user(&app.pool, ROOT, PASSWORD, true)
        .await
        .unwrap();
    app.sign_in(ROOT, PASSWORD).await
}

/// The listed User with `email`, as the Instance Admin sees them.
async fn listed(app: &TestApp, admin: &str, email: &str) -> Value {
    let res = app.as_user("GET", "/api/admin/users", None, admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["email"] == email)
        .unwrap_or_else(|| panic!("{email} not listed: {}", res.text))
        .clone()
}

async fn patch_user(app: &TestApp, admin: &str, email: &str, body: Value) -> Res {
    let id = listed(app, admin, email).await["id"].clone();
    let path = format!("/api/admin/users/{id}");
    app.as_user("PATCH", &path, Some(body), admin).await
}

async fn signed_in(app: &TestApp, cookie: &str) -> bool {
    let res = app.as_user("GET", "/api/me", None, cookie).await;
    res.status == StatusCode::OK
}

#[tokio::test]
async fn an_instance_admin_lists_every_user() {
    let app = app().await;
    let admin = instance_admin(&app).await;

    let user = listed(&app, &admin, OWNER).await;

    assert_eq!(user["instance_admin"], false);
    assert_eq!(user["disabled"], false);
    assert_eq!(listed(&app, &admin, ROOT).await["instance_admin"], true);
}

#[tokio::test]
async fn a_disabled_user_cant_sign_in_and_their_sessions_end_for_good() {
    let app = app().await;
    let admin = instance_admin(&app).await;

    let res = patch_user(&app, &admin, OWNER, json!({ "disabled": true })).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["disabled"], true);
    assert!(!signed_in(&app, &app.owner.cookie).await);
    let res = app.sign_in_res(OWNER, PASSWORD).await;
    assert_eq!(res.json()["kind"], "invalid_credentials", "{}", res.text);

    patch_user(&app, &admin, OWNER, json!({ "disabled": false })).await;

    assert!(
        !signed_in(&app, &app.owner.cookie).await,
        "re-enabling doesn't revive old sessions"
    );
    app.sign_in(OWNER, PASSWORD).await;
}

#[tokio::test]
async fn resetting_a_password_returns_a_new_one_and_ends_sessions() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let id = listed(&app, &admin, OWNER).await["id"].clone();

    let path = format!("/api/admin/users/{id}/password");
    let res = app.as_user("POST", &path, None, &admin).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let password = res.json()["password"].as_str().unwrap().to_string();
    assert!(!signed_in(&app, &app.owner.cookie).await);
    let res = app.sign_in_res(OWNER, PASSWORD).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "old password");
    app.sign_in(OWNER, &password).await;
}

#[tokio::test]
async fn an_instance_admin_makes_another_user_an_instance_admin() {
    let app = app().await;
    let admin = instance_admin(&app).await;

    let res = patch_user(&app, &admin, OWNER, json!({ "instance_admin": true })).await;

    assert_eq!(res.json()["instance_admin"], true, "{}", res.text);
    let res = app
        .as_user("GET", "/api/admin/users", None, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn the_last_instance_admin_cant_be_disabled_or_demoted() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    // A disabled Instance Admin can't run the instance either.
    patch_user(&app, &admin, OWNER, json!({ "instance_admin": true })).await;
    patch_user(&app, &admin, OWNER, json!({ "disabled": true })).await;

    for body in [
        json!({ "disabled": true }),
        json!({ "instance_admin": false }),
    ] {
        let res = patch_user(&app, &admin, ROOT, body).await;
        assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
        assert_eq!(res.json()["kind"], "last_instance_admin", "{}", res.text);
    }
    assert!(signed_in(&app, &admin).await);
    assert_eq!(listed(&app, &admin, ROOT).await["instance_admin"], true);
}

#[tokio::test]
async fn an_instance_admin_steps_down_once_another_runs_the_instance() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    patch_user(&app, &admin, OWNER, json!({ "instance_admin": true })).await;

    let res = patch_user(&app, &admin, ROOT, json!({ "instance_admin": false })).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["instance_admin"], false);
    let res = app
        .as_user("GET", "/api/admin/users", None, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn an_unknown_user_is_not_found() {
    let app = app().await;
    let admin = instance_admin(&app).await;

    let patch = app
        .as_user(
            "PATCH",
            "/api/admin/users/0",
            Some(json!({ "disabled": true })),
            &admin,
        )
        .await;
    let reset = app
        .as_user("POST", "/api/admin/users/0/password", None, &admin)
        .await;

    for res in [patch, reset] {
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
        assert_eq!(res.json()["kind"], "user_not_found");
    }
}

#[tokio::test]
async fn a_non_instance_admin_gets_403_on_every_admin_endpoint() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let id = listed(&app, &admin, ROOT).await["id"].clone();

    for (method, path, body) in [
        ("GET", "/api/admin/users".to_string(), None),
        (
            "PATCH",
            format!("/api/admin/users/{id}"),
            Some(json!({ "disabled": true })),
        ),
        ("POST", format!("/api/admin/users/{id}/password"), None),
        (
            "POST",
            "/api/admin/invites".to_string(),
            Some(json!({ "email": "eve@example.com" })),
        ),
    ] {
        let by_session = app
            .as_user(method, &path, body.clone(), &app.owner.cookie)
            .await;
        let by_key = app.bearer(method, &path, body, Some(&app.owner.key)).await;
        for res in [by_session, by_key] {
            assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {path}");
            assert_eq!(res.json()["kind"], "forbidden", "{}", res.text);
        }
    }
    assert!(signed_in(&app, &admin).await);
}
