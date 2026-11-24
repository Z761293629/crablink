//! Users deleting themselves: `DELETE /api/me`.

mod common;
use common::*;

use axum::http::StatusCode;
use chrono::TimeDelta;
use serde_json::{Value, json};

fn password(password: &str) -> Option<Value> {
    Some(json!({ "password": password }))
}

#[tokio::test]
async fn a_user_deletes_themself_with_their_password() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com/", "code": "mine" }))
        .await;
    let cookie = &app.owner.cookie;

    let res = app
        .as_user("DELETE", "/api/me", password(PASSWORD), cookie)
        .await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert!(
        res.header("set-cookie").starts_with("crablink_session=;"),
        "the session cookie is cleared"
    );
    let me = app.as_user("GET", "/api/me", None, cookie).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.text);
    let res = app.sign_in_res(OWNER, PASSWORD).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    // The Personal Workspace went with it: its Link is a Deleted Link, its API Key dead.
    assert_eq!(app.get("/mine").await.status, StatusCode::GONE);
    let res = app.api("GET", "/links", None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
}

#[tokio::test]
async fn deleting_oneself_needs_the_right_password() {
    let app = app().await;

    let res = app
        .as_user(
            "DELETE",
            "/api/me",
            password("wrong one"),
            &app.owner.cookie,
        )
        .await;

    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(res.json()["kind"], "wrong_password");
    app.sign_in(OWNER, PASSWORD).await;
}

fn members_path(app: &TestApp) -> String {
    format!("/api/workspaces/{}/members", app.owner.slug)
}

/// Adds `email`, who must have a User, to `OWNER`'s Workspace as `role`.
async fn join(app: &TestApp, email: &str, role: &str) {
    let body = Some(json!({ "email": email }));
    let res = app
        .as_user("POST", &members_path(app), body, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let path = format!("{}/{email}", members_path(app));
    let body = Some(json!({ "role": role }));
    let res = app.as_user("PATCH", &path, body, &app.owner.cookie).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[tokio::test]
async fn a_member_who_deletes_themself_just_leaves_other_workspaces() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com/", "code": "shared" }))
        .await;
    let bob = app.account("bob@example.com").await;
    join(&app, "bob@example.com", "member").await;

    let res = app
        .as_user("DELETE", "/api/me", password(PASSWORD), &bob.cookie)
        .await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(app.get("/shared").await.status, StatusCode::FOUND);
    let res = app
        .as_user("GET", &members_path(&app), None, &app.owner.cookie)
        .await;
    assert_eq!(res.json(), json!([{ "email": OWNER, "role": "owner" }]));
}

#[tokio::test]
async fn an_owner_beside_another_owner_just_leaves() {
    let app = app().await;
    app.create_user("bob@example.com").await;
    join(&app, "bob@example.com", "owner").await;
    app.create(json!({ "target": "https://example.com/", "code": "shared" }))
        .await;

    let res = app
        .as_user("DELETE", "/api/me", password(PASSWORD), &app.owner.cookie)
        .await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(app.get("/shared").await.status, StatusCode::FOUND);
}

#[tokio::test]
async fn the_only_owner_of_a_workspace_with_other_members_cant_delete_themself() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com/", "code": "shared" }))
        .await;
    app.create_user("bob@example.com").await;
    join(&app, "bob@example.com", "member").await;

    let res = app
        .as_user("DELETE", "/api/me", password(PASSWORD), &app.owner.cookie)
        .await;

    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.json()["kind"], "last_owner");
    assert_eq!(app.get("/shared").await.status, StatusCode::FOUND);
    app.sign_in(OWNER, PASSWORD).await;
}

#[tokio::test]
async fn the_last_instance_admin_cant_delete_themself() {
    let app = app().await;
    let admin = app.instance_admin().await;

    let res = app
        .as_user("DELETE", "/api/me", password(PASSWORD), &admin)
        .await;

    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.json()["kind"], "last_instance_admin");

    crablink_server::create_user(&app.pool, "second@example.com", PASSWORD, true)
        .await
        .unwrap();
    let res = app
        .as_user("DELETE", "/api/me", password(PASSWORD), &admin)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
}

#[tokio::test]
async fn a_user_without_a_password_deletes_themself_right_after_signing_in_through_an_identity_provider()
 {
    let fake = FakeIdentityProvider::start().await;
    let provider = crablink_server::IdentityProvider {
        allowed_email_domains: vec!["example.com".into()],
        ..fake.identity_provider("fake", "Fake ID")
    };
    let app = app_with(|s| crablink_server::Settings {
        identity_providers: vec![provider],
        ..s
    })
    .await;
    let ada = Person::verified("ada@example.com");
    let signed_in = |res: Res| res.cookie("crablink_session").expect("a session cookie");
    // Signs in 6 minutes ago: the fake's ID tokens expire too soon to wait instead.
    app.advance(TimeDelta::minutes(-6));
    let cookie = signed_in(app.sign_in_through(&fake, "fake", ada.clone()).await);
    app.advance(TimeDelta::minutes(6));

    let res = app
        .as_user("DELETE", "/api/me", Some(json!({})), &cookie)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(res.json()["kind"], "fresh_sign_in_needed");

    let cookie = signed_in(app.sign_in_through(&fake, "fake", ada).await);
    let res = app
        .as_user("DELETE", "/api/me", Some(json!({})), &cookie)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let me = app.as_user("GET", "/api/me", None, &cookie).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.text);
}
