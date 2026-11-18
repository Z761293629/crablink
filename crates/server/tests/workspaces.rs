mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

async fn workspaces(app: &TestApp, cookie: &str) -> Value {
    let res = app.as_user("GET", "/api/workspaces", None, cookie).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

#[tokio::test]
async fn a_user_creates_a_workspace_and_becomes_its_owner() {
    let app = app().await;

    let body = Some(json!({ "name": "Marketing Team" }));
    let res = app
        .as_user("POST", "/api/workspaces", body, &app.owner.cookie)
        .await;

    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let created = res.json();
    assert_eq!(created["name"], "Marketing Team");
    assert_eq!(created["slug"], "marketing-team");
    assert_eq!(created["role"], "owner");
    let listed = workspaces(&app, &app.owner.cookie).await;
    assert!(listed.as_array().unwrap().contains(&created), "{listed}");
}

#[tokio::test]
async fn an_owner_renames_a_workspace_and_its_slug_stays() {
    let app = app().await;
    let path = format!("/api/workspaces/{}", app.owner.slug);

    let body = Some(json!({ "name": "  Growth  " }));
    let res = app.as_user("PATCH", &path, body, &app.owner.cookie).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let renamed = json!({ "slug": app.owner.slug, "name": "Growth" });
    assert_eq!(res.json(), renamed);
    assert_eq!(
        workspaces(&app, &app.owner.cookie).await[0]["name"],
        "Growth"
    );
    for name in ["", " ", &"x".repeat(101)] {
        let body = Some(json!({ "name": name }));
        let res = app.as_user("PATCH", &path, body, &app.owner.cookie).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{name:?}");
        assert_eq!(res.json()["kind"], "bad_request", "{}", res.text);
    }
}

#[tokio::test]
async fn deleting_a_workspace_deletes_its_links_keeps_their_codes_and_stops_its_api_keys() {
    let app = app().await;
    let bob = join(&app, "bob@example.com").await;
    let eve = app.account("eve@example.com").await;
    app.create(json!({ "target": "https://example.com/", "code": "promo" }))
        .await;

    let path = format!("/api/workspaces/{}", app.owner.slug);
    let res = app.as_user("DELETE", &path, None, &app.owner.cookie).await;

    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(app.get("/promo").await.status, StatusCode::GONE);
    let res = app.api("GET", "/links", None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    assert_eq!(res.json()["kind"], "unauthorized", "{}", res.text);
    for cookie in [&app.owner.cookie, &bob] {
        let theirs = workspaces(&app, cookie).await;
        assert!(
            theirs
                .as_array()
                .unwrap()
                .iter()
                .all(|w| w["slug"] != app.owner.slug.as_str()),
            "{theirs}"
        );
    }
    let links = format!("/api/workspaces/{}/links", eve.slug);
    let body = Some(json!({ "target": "https://example.com/", "code": "promo" }));
    let res = app.as_user("POST", &links, body, &eve.cookie).await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.json()["kind"], "code_taken", "{}", res.text);
}

fn members_path(app: &TestApp) -> String {
    format!("/api/workspaces/{}/members", app.owner.slug)
}

/// Signs up `email` and adds them to `OWNER`'s Workspace as a Member; returns their cookie.
async fn join(app: &TestApp, email: &str) -> String {
    let cookie = app.account(email).await.cookie;
    let body = Some(json!({ "email": email }));
    let res = app
        .as_user("POST", &members_path(app), body, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    cookie
}

#[tokio::test]
async fn an_owner_adds_a_user_by_email_who_then_manages_its_links() {
    let app = app().await;
    let bob = app.account("bob@example.com").await;

    let body = Some(json!({ "email": "BOB@example.com" }));
    let res = app
        .as_user("POST", &members_path(&app), body, &app.owner.cookie)
        .await;

    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(
        res.json(),
        json!({ "email": "bob@example.com", "role": "member" })
    );
    let theirs = workspaces(&app, &bob.cookie).await;
    let shared = theirs
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["slug"] == app.owner.slug.as_str())
        .unwrap_or_else(|| panic!("{theirs}"));
    assert_eq!(shared["role"], "member");
    let links = format!("/api/workspaces/{}/links", app.owner.slug);
    let body = Some(json!({ "target": "https://example.com/", "code": "bobs" }));
    let res = app.as_user("POST", &links, body, &bob.cookie).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let res = app
        .as_user("DELETE", &format!("{links}/bobs"), None, &bob.cookie)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let res = app
        .as_user("GET", &members_path(&app), None, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.json(),
        json!([
            { "email": "bob@example.com", "role": "member" },
            { "email": OWNER, "role": "owner" },
        ])
    );
}

#[tokio::test]
async fn adding_an_unknown_user_or_a_member_again_fails() {
    let app = app().await;
    join(&app, "bob@example.com").await;

    for (email, status, kind) in [
        (
            "nobody@example.com",
            StatusCode::NOT_FOUND,
            "user_not_found",
        ),
        ("bob@example.com", StatusCode::CONFLICT, "already_member"),
        (OWNER, StatusCode::CONFLICT, "already_member"),
    ] {
        let body = Some(json!({ "email": email }));
        let res = app
            .as_user("POST", &members_path(&app), body, &app.owner.cookie)
            .await;
        assert_eq!(res.status, status, "{email}: {}", res.text);
        assert_eq!(res.json()["kind"], kind, "{}", res.text);
    }
}

fn member_path(app: &TestApp, email: &str) -> String {
    format!("{}/{email}", members_path(app))
}

#[tokio::test]
async fn an_owner_promotes_and_demotes_members() {
    let app = app().await;
    let bob = join(&app, "bob@example.com").await;

    let promote = Some(json!({ "role": "owner" }));
    let path = member_path(&app, "bob@example.com");
    let res = app
        .as_user("PATCH", &path, promote, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.json(),
        json!({ "email": "bob@example.com", "role": "owner" })
    );
    let res = app.as_user("GET", &members_path(&app), None, &bob).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);

    let demote = Some(json!({ "role": "member" }));
    let res = app
        .as_user("PATCH", &member_path(&app, OWNER), demote, &bob)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let res = app
        .as_user("GET", &members_path(&app), None, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
}

#[tokio::test]
async fn the_last_owner_cannot_leave_or_be_demoted() {
    let app = app().await;
    join(&app, "bob@example.com").await;
    let me = member_path(&app, OWNER);

    for res in [
        app.as_user(
            "PATCH",
            &me,
            Some(json!({ "role": "member" })),
            &app.owner.cookie,
        )
        .await,
        app.as_user("DELETE", &me, None, &app.owner.cookie).await,
    ] {
        assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
        assert_eq!(res.json()["kind"], "last_owner", "{}", res.text);
    }
    assert_eq!(
        workspaces(&app, &app.owner.cookie).await[0]["role"],
        "owner"
    );
}

#[tokio::test]
async fn an_owner_removes_a_member_and_a_member_leaves() {
    let app = app().await;
    let bob = join(&app, "bob@example.com").await;
    let carol = join(&app, "carol@example.com").await;
    let links = format!("/api/workspaces/{}/links", app.owner.slug);

    let res = app
        .as_user(
            "DELETE",
            &member_path(&app, "bob@example.com"),
            None,
            &app.owner.cookie,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = app
        .as_user(
            "DELETE",
            &member_path(&app, "carol@example.com"),
            None,
            &carol,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    for cookie in [&bob, &carol] {
        let theirs = workspaces(&app, cookie).await;
        assert_eq!(theirs.as_array().unwrap().len(), 1, "{theirs}");
        let res = app.as_user("GET", &links, None, cookie).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    }
    let res = app
        .as_user(
            "DELETE",
            &member_path(&app, "bob@example.com"),
            None,
            &app.owner.cookie,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(res.json()["kind"], "user_not_found", "{}", res.text);
}

#[tokio::test]
async fn members_and_api_keys_cannot_manage_the_workspace() {
    let app = app().await;
    let bob = join(&app, "bob@example.com").await;
    join(&app, "carol@example.com").await;
    let workspace = format!("/api/workspaces/{}", app.owner.slug);
    let members = members_path(&app);
    let carol = member_path(&app, "carol@example.com");
    let owner = member_path(&app, OWNER);
    let keys = format!("{workspace}/api-keys");
    let name = || Some(json!({ "name": "Mine now" }));
    let attempts = [
        ("PATCH", workspace.clone(), name()),
        ("DELETE", workspace.clone(), None),
        ("GET", members.clone(), None),
        (
            "POST",
            members.clone(),
            Some(json!({ "email": "eve@example.com" })),
        ),
        (
            "PATCH",
            member_path(&app, "bob@example.com"),
            Some(json!({ "role": "owner" })),
        ),
        ("PATCH", owner.clone(), Some(json!({ "role": "member" }))),
        ("DELETE", carol.clone(), None),
        ("DELETE", owner.clone(), None),
        ("GET", keys.clone(), None),
        ("POST", keys.clone(), name()),
    ];
    app.account("eve@example.com").await;

    for (method, path, body) in attempts {
        for res in [
            app.as_user(method, &path, body.clone(), &bob).await,
            app.bearer(method, &path, body.clone(), Some(&app.owner.key))
                .await,
        ] {
            assert_eq!(
                res.status,
                StatusCode::FORBIDDEN,
                "{method} {path}: {}",
                res.text
            );
            assert_eq!(res.json()["kind"], "forbidden", "{}", res.text);
        }
    }
    let res = app
        .bearer("POST", "/api/workspaces", name(), Some(&app.owner.key))
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    let res = app.as_user("GET", &members, None, &app.owner.cookie).await;
    assert_eq!(res.json().as_array().unwrap().len(), 3, "{}", res.text);
}
