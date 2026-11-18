mod common;

use axum::http::StatusCode;
use chrono::TimeDelta;
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

/// Creates an invite as the Instance Admin, asserting `201`; returns its token.
async fn invite(app: &TestApp, admin: &str, body: Value) -> String {
    let res = app
        .as_user("POST", "/api/admin/invites", Some(body), admin)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let url = res.json()["url"].as_str().unwrap().to_string();
    let (base, token) = url.split_once('#').unwrap();
    assert_eq!(base, format!("{BASE_URL}/app/invite"));
    token.to_string()
}

async fn accept(app: &TestApp, token: &str, password: &str) -> Res {
    let body = json!({ "token": token, "password": password });
    app.request("POST", "/api/invites/accept", Some(body), &[])
        .await
}

#[tokio::test]
async fn accepting_an_invite_sets_the_password_and_signs_in() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let token = invite(&app, &admin, json!({ "email": "ada@example.com" })).await;

    let res = accept(&app, &token, "ada's password").await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["email"], "ada@example.com");
    assert_eq!(res.json()["instance_admin"], false);
    let cookie = res.header("set-cookie").split(';').next().unwrap();
    let res = app.as_user("GET", "/api/me", None, cookie).await;
    assert_eq!(
        res.json()["user"]["email"],
        "ada@example.com",
        "{}",
        res.text
    );
    app.sign_in("ada@example.com", "ada's password").await;
}

#[tokio::test]
async fn an_invite_naming_a_workspace_makes_the_new_user_a_member_of_it() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let body = json!({ "email": "ada@example.com", "workspace": app.owner.slug });
    let token = invite(&app, &admin, body).await;

    let res = accept(&app, &token, "ada's password").await;
    let cookie = res.header("set-cookie").split(';').next().unwrap();

    let res = app.as_user("GET", "/api/workspaces", None, cookie).await;
    let slugs: Vec<Value> = res
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["slug"].clone())
        .collect();
    assert!(slugs.contains(&json!(app.owner.slug)), "{}", res.text);
    assert!(
        slugs.contains(&json!("ada")),
        "personal Workspace too: {}",
        res.text
    );
    let path = format!("/api/workspaces/{}/links", app.owner.slug);
    let res = app.as_user("GET", &path, None, cookie).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let path = format!("/api/workspaces/{}/api-keys", app.owner.slug);
    let res = app.as_user("GET", &path, None, cookie).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "a Member, not an Owner");
}

#[tokio::test]
async fn an_invite_works_only_once() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let token = invite(&app, &admin, json!({ "email": "ada@example.com" })).await;
    accept(&app, &token, "ada's password").await;

    let res = accept(&app, &token, "eve's password").await;

    assert_eq!(res.status, StatusCode::GONE, "{}", res.text);
    assert_eq!(res.json()["kind"], "invite_used");
    app.sign_in("ada@example.com", "ada's password").await;
}

#[tokio::test]
async fn an_expired_invite_is_rejected() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let token = invite(&app, &admin, json!({ "email": "ada@example.com" })).await;
    app.advance(TimeDelta::days(8));

    let res = accept(&app, &token, "ada's password").await;

    assert_eq!(res.status, StatusCode::GONE, "{}", res.text);
    assert_eq!(res.json()["kind"], "invite_expired");
    let res = app.sign_in_res("ada@example.com", "ada's password").await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unknown_invite_is_not_found() {
    let app = app().await;

    let res = accept(&app, "forged", "ada's password").await;

    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(res.json()["kind"], "invite_not_found");
}

#[tokio::test]
async fn a_rejected_acceptance_leaves_the_invite_usable() {
    let app = app().await;
    let admin = instance_admin(&app).await;
    let token = invite(&app, &admin, json!({ "email": "ada@example.com" })).await;

    let res = accept(&app, &token, "short").await;
    assert_eq!(res.json()["kind"], "password_too_short", "{}", res.text);

    let res = accept(&app, &token, "ada's password").await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn an_invite_needs_a_new_email_and_an_existing_workspace() {
    let app = app().await;
    let admin = instance_admin(&app).await;

    for (body, kind) in [
        (json!({ "email": "OWNER@example.com" }), "email_taken"),
        (json!({ "email": "not an email" }), "invalid_email"),
        (
            json!({ "email": "ada@example.com", "workspace": "nope" }),
            "not_found",
        ),
    ] {
        let res = app
            .as_user("POST", "/api/admin/invites", Some(body), &admin)
            .await;
        assert_eq!(res.json()["kind"], kind, "{}", res.text);
    }
}
