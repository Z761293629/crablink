mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

/// A session cookie or an API Key.
#[derive(Clone, Copy)]
enum Cred<'a> {
    Cookie(&'a str),
    Key(&'a str),
}

/// A request to `path` under `OWNER`'s Workspace with `cred`.
async fn send(app: &TestApp, cred: Cred<'_>, method: &str, path: &str, body: Option<Value>) -> Res {
    let path = format!("/api/workspaces/{}{path}", app.owner.slug);
    match cred {
        Cred::Cookie(cookie) => app.as_user(method, &path, body, cookie).await,
        Cred::Key(key) => app.bearer(method, &path, body, Some(key)).await,
    }
}

/// Creates, lists, views, edits and deletes a Link in `OWNER`'s Workspace with
/// `cred`; asserts every step succeeds.
async fn manage_a_link(app: &TestApp, cred: Cred<'_>) {
    let body = json!({ "target": "https://example.com", "code": "mine" });
    let res = send(app, cred, "POST", "/links", Some(body)).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let res = send(app, cred, "GET", "/links", None).await;
    assert_eq!(res.json()["links"][0]["code"], "mine", "{}", res.text);
    let res = send(app, cred, "GET", "/links/mine", None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let edit = json!({ "target": "https://example.com/new" });
    let res = send(app, cred, "PATCH", "/links/mine", Some(edit)).await;
    assert_eq!(
        res.json()["target"],
        "https://example.com/new",
        "{}",
        res.text
    );
    let res = send(app, cred, "DELETE", "/links/mine", None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
}

#[tokio::test]
async fn a_members_session_manages_the_workspaces_links() {
    let app = app().await;

    manage_a_link(&app, Cred::Cookie(&app.owner.cookie)).await;
}

#[tokio::test]
async fn the_workspaces_api_key_manages_its_links() {
    let app = app().await;

    manage_a_link(&app, Cred::Key(&app.owner.key)).await;
}

#[tokio::test]
async fn another_workspaces_session_or_api_key_gets_403() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com", "code": "mine" }))
        .await;
    let eve = app.account("eve@example.com").await;
    let ws = format!("/api/workspaces/{}", app.owner.slug);

    for (method, path) in [
        ("GET", "/links"),
        ("POST", "/links"),
        ("GET", "/links/mine"),
        ("PATCH", "/links/mine"),
        ("DELETE", "/links/mine"),
        ("GET", "/links/mine/qr"),
    ] {
        let path = format!("{ws}{path}");
        let body = Some(json!({ "target": "https://evil.example" }));
        let by_key = app
            .bearer(method, &path, body.clone(), Some(&eve.key))
            .await;
        let by_session = app.as_user(method, &path, body, &eve.cookie).await;
        for res in [by_key, by_session] {
            assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {path}");
            assert_eq!(res.json()["kind"], "forbidden", "{}", res.text);
        }
    }
    let res = app.api("GET", "/links/mine", None).await;
    assert_eq!(res.json()["target"], "https://example.com");
}

#[tokio::test]
async fn an_unknown_workspace_gets_403_like_someone_elses() {
    let app = app().await;

    let res = app
        .bearer(
            "GET",
            "/api/workspaces/nobody/links",
            None,
            Some(&app.owner.key),
        )
        .await;

    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(res.json()["kind"], "forbidden", "{}", res.text);
}

#[tokio::test]
async fn links_need_a_credential() {
    let app = app().await;

    for cookie in ["", "crablink_session=forged"] {
        let path = format!("/api/workspaces/{}/links", app.owner.slug);
        let res = app.as_user("GET", &path, None, cookie).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{cookie:?}");
        assert_eq!(res.json()["kind"], "unauthorized", "{}", res.text);
    }
}

#[tokio::test]
async fn an_unknown_bearer_value_is_told_to_sign_in_again_with_an_api_key() {
    let app = app().await;

    // What a 0.2 CLI sends: its old token, to the old path.
    for path in ["/api/links", "/api/me", "/api/workspaces/owner/links"] {
        let res = app.bearer("GET", path, None, Some("test-token")).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(res.json()["kind"], "unauthorized", "{}", res.text);
        let msg = res.json()["error"].as_str().unwrap().to_string();
        assert!(msg.contains("API Key"), "{msg}");
        assert!(msg.contains("crablink login"), "{msg}");
    }
}

#[tokio::test]
async fn the_0_2_paths_are_gone() {
    let app = app().await;

    for path in ["/api/links", "/api/links/abc"] {
        let res = app.bearer("GET", path, None, Some(&app.owner.key)).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(res.json()["kind"], "not_found", "{}", res.text);
    }
}

#[tokio::test]
async fn me_with_an_api_key_is_its_workspace() {
    let app = app().await;

    let res = app
        .bearer("GET", "/api/me", None, Some(&app.owner.key))
        .await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["workspace"]["slug"], app.owner.slug);
    assert!(res.json()["workspace"]["name"].is_string(), "{}", res.text);
}

#[tokio::test]
async fn a_user_gets_one_personal_workspace_on_first_sign_in() {
    let app = app().await;
    app.create_user("ada@example.com").await;

    app.sign_in("ada@example.com", PASSWORD).await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;
    let res = app.as_user("GET", "/api/workspaces", None, &cookie).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let workspaces = res.json();
    assert_eq!(workspaces.as_array().unwrap().len(), 1, "{workspaces}");
    assert_eq!(workspaces[0]["slug"], "ada");
}

#[tokio::test]
async fn personal_workspace_slugs_never_collide() {
    let app = app().await;
    app.create_user("ada@example.org").await;

    let cookie = app.sign_in("ada@example.org", PASSWORD).await;
    let other = app.workspace_of(&cookie).await;
    app.create_user("ada@example.com").await;
    let cookie = app.sign_in("ada@example.com", PASSWORD).await;

    assert_ne!(app.workspace_of(&cookie).await, other);
}

#[tokio::test]
async fn a_code_is_taken_on_its_domain_across_workspaces() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com", "code": "mine" }))
        .await;
    app.api("DELETE", "/links/mine", None).await;
    app.create(json!({ "target": "https://example.com", "code": "live" }))
        .await;
    let eve = app.account("eve@example.com").await;
    let eves = format!("/api/workspaces/{}/links", eve.slug);

    for code in ["mine", "live"] {
        let body = json!({ "target": "https://evil.example", "code": code });
        let res = app.bearer("POST", &eves, Some(body), Some(&eve.key)).await;
        assert_eq!(res.status, StatusCode::CONFLICT, "{code}: {}", res.text);
        assert_eq!(res.json()["kind"], "code_taken", "{}", res.text);
    }
    // And Eve can't see the owner's Link through her own Workspace.
    let res = app
        .bearer("GET", &format!("{eves}/live"), None, Some(&eve.key))
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(app.get("/live").await.status, StatusCode::FOUND);
    assert_eq!(app.get("/mine").await.status, StatusCode::GONE);
}

#[tokio::test]
async fn unknown_api_paths_are_not_found() {
    let app = app().await;

    let res = app.api("GET", "/nope", None).await;

    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.json()["kind"], "not_found", "{}", res.text);
    assert_eq!(res.json()["error"], "not found");
}

#[tokio::test]
async fn redirects_need_no_credential() {
    let app = app().await;
    let link = app.create(json!({ "target": "https://example.com" })).await;

    let res = app
        .get(&format!("/{}", link["code"].as_str().unwrap()))
        .await;

    assert_eq!(res.status, StatusCode::FOUND);
}
