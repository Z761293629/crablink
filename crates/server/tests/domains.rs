mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

const CUSTOM: &str = "go.example.com";

#[tokio::test]
async fn the_same_code_on_two_domains_reaches_two_targets() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;

    let custom = app
        .create(json!({ "target": "https://example.com/custom", "code": "sale", "domain": CUSTOM }))
        .await;
    app.create(json!({ "target": "https://example.com/default", "code": "sale" }))
        .await;

    assert_eq!(custom["domain"], CUSTOM);
    assert_eq!(custom["short_url"], "https://go.example.com/sale");
    let res = app.get_on(CUSTOM, "/sale").await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.header("location"), "https://example.com/custom");
    let res = app.get_on(HOST, "/sale").await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.header("location"), "https://example.com/default");
}

#[tokio::test]
async fn the_link_list_shows_links_on_every_domain_of_the_workspace() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    app.create(json!({ "target": "https://example.com/custom", "code": "sale", "domain": CUSTOM }))
        .await;
    app.create(json!({ "target": "https://example.com/default", "code": "sale" }))
        .await;

    let links = app.api("GET", "/links", None).await.json();

    let short_urls: Vec<_> = links["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["short_url"].clone())
        .collect();
    assert_eq!(
        short_urls,
        ["https://crabl.ink/sale", "https://go.example.com/sale"]
    );
}

/// `GET /internal/domain-allowed?domain=…`, with no credential: its status.
async fn allowed(app: &TestApp, domain: &str) -> StatusCode {
    let path = format!("/internal/domain-allowed?domain={domain}");
    app.bearer("GET", &path, None, None).await.status
}

#[tokio::test]
async fn domain_allowed_says_yes_to_the_default_domain_and_approved_domains_only() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;

    assert_eq!(allowed(&app, HOST).await, StatusCode::OK);
    assert_eq!(allowed(&app, CUSTOM).await, StatusCode::OK);
    assert_eq!(allowed(&app, "GO.Example.com").await, StatusCode::OK);
    assert_eq!(
        allowed(&app, "evil.example.com").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_pending_domain_neither_redirects_nor_is_allowed_nor_takes_links() {
    let app = app().await;
    let res = app
        .add_domain(&app.owner.cookie, &app.owner.slug, CUSTOM)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(
        res.json(),
        json!({ "domain": CUSTOM, "status": "pending", "default": false })
    );
    app.create(json!({ "target": "https://example.com/a", "code": "sale" }))
        .await;

    assert_eq!(allowed(&app, CUSTOM).await, StatusCode::NOT_FOUND);
    assert_eq!(
        app.get_on(CUSTOM, "/sale").await.status,
        StatusCode::NOT_FOUND
    );
    let body = json!({ "target": "https://example.com/b", "domain": CUSTOM });
    let res = app.api("POST", "/links", Some(body)).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.json()["kind"], "domain_not_approved");
}

#[tokio::test]
async fn a_rejected_domain_stops_redirecting_and_is_no_longer_allowed() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    app.create(json!({ "target": "https://example.com/a", "code": "sale", "domain": CUSTOM }))
        .await;

    app.set_domain_status(CUSTOM, "rejected").await;

    assert_eq!(allowed(&app, CUSTOM).await, StatusCode::NOT_FOUND);
    assert_eq!(
        app.get_on(CUSTOM, "/sale").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_unknown_host_is_not_found() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com/a", "code": "sale" }))
        .await;

    let res = app.get_on("evil.example.com", "/sale").await;

    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn adding_a_domain_another_workspace_holds_fails_with_domain_taken() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    let other = app.account("other@example.com").await;

    for domain in [CUSTOM, "GO.example.com", HOST] {
        let res = app.add_domain(&other.cookie, &other.slug, domain).await;
        assert_eq!(res.status, StatusCode::CONFLICT, "{domain}");
        assert_eq!(res.json()["kind"], "domain_taken", "{domain}");
    }
}

#[tokio::test]
async fn a_domain_must_be_a_host_name() {
    let app = app().await;

    for domain in [
        "",
        "go.example.com:8080",
        "https://go.example.com",
        "1.2.3.4",
        "go example.com",
    ] {
        let res = app
            .add_domain(&app.owner.cookie, &app.owner.slug, domain)
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{domain:?}");
        assert_eq!(res.json()["kind"], "invalid_domain", "{domain:?}");
    }
}

#[tokio::test]
async fn removing_a_domain_deletes_its_links_and_re_adding_it_keeps_their_codes_taken() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    app.create(json!({ "target": "https://example.com/a", "code": "sale", "domain": CUSTOM }))
        .await;

    let path = format!("/api/workspaces/{}/domains/{CUSTOM}", app.owner.slug);
    let res = app.as_user("DELETE", &path, None, &app.owner.cookie).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let links = app.api("GET", "/links", None).await.json();
    assert_eq!(links["links"], json!([]));
    assert_eq!(allowed(&app, CUSTOM).await, StatusCode::NOT_FOUND);

    let other = app.account("other@example.com").await;
    let res = app.add_domain(&other.cookie, &other.slug, CUSTOM).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    app.set_domain_status(CUSTOM, "approved").await;
    let body = json!({ "target": "https://example.com/b", "code": "sale", "domain": CUSTOM });
    let path = format!("/api/workspaces/{}/links", other.slug);
    let res = app
        .bearer("POST", &path, Some(body), Some(&other.key))
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.json()["kind"], "code_taken");
    assert_eq!(app.get_on(CUSTOM, "/sale").await.status, StatusCode::GONE);
}

#[tokio::test]
async fn deleting_a_workspace_frees_its_domains() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    let path = format!("/api/workspaces/{}", app.owner.slug);
    let res = app.as_user("DELETE", &path, None, &app.owner.cookie).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let other = app.account("other@example.com").await;
    let res = app.add_domain(&other.cookie, &other.slug, CUSTOM).await;

    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.json()["status"], "pending");
}

#[tokio::test]
async fn a_workspace_lists_the_default_domain_then_its_own() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    let other = app.account("other@example.com").await;
    app.add_domain(&other.cookie, &other.slug, "other.example.com")
        .await;

    let res = app.api("GET", "/domains", None).await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.json(),
        json!([
            { "domain": HOST, "status": "approved", "default": true },
            { "domain": CUSTOM, "status": "approved", "default": false },
        ])
    );
}

#[tokio::test]
async fn only_owners_add_and_remove_domains() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    let member = app.account("member@example.com").await;
    let path = format!("/api/workspaces/{}/members", app.owner.slug);
    let body = json!({ "email": "member@example.com" });
    let res = app
        .as_user("POST", &path, Some(body), &app.owner.cookie)
        .await;
    assert!(res.status.is_success(), "{}", res.text);

    let res = app
        .add_domain(&member.cookie, &app.owner.slug, "new.example.com")
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = app
        .api(
            "POST",
            "/domains",
            Some(json!({ "domain": "new.example.com" })),
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let path = format!("/api/workspaces/{}/domains/{CUSTOM}", app.owner.slug);
    let res = app.as_user("DELETE", &path, None, &member.cookie).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = app.api("DELETE", &format!("/domains/{CUSTOM}"), None).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn instance_admins_list_custom_domains_and_only_they_approve() {
    let app = app().await;
    app.add_domain(&app.owner.cookie, &app.owner.slug, CUSTOM)
        .await;
    let admin = app.instance_admin().await;

    let res = app.as_user("GET", "/api/admin/domains", None, &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.json(),
        json!([{ "domain": CUSTOM, "status": "pending", "workspace_slug": app.owner.slug }])
    );

    let path = format!("/api/admin/domains/{CUSTOM}");
    let body = json!({ "status": "approved" });
    let res = app
        .as_user("PATCH", &path, Some(body.clone()), &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = app
        .as_user("GET", "/api/admin/domains", None, &app.owner.cookie)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = app
        .as_user(
            "PATCH",
            "/api/admin/domains/nope.example.com",
            Some(body),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.json()["kind"], "domain_not_found");
}

#[tokio::test]
async fn a_link_on_a_custom_domain_is_addressed_by_its_domain() {
    let app = app().await;
    app.approved_domain(CUSTOM).await;
    app.create(json!({ "target": "https://example.com/custom", "code": "sale", "domain": CUSTOM }))
        .await;
    app.create(json!({ "target": "https://example.com/default", "code": "sale" }))
        .await;
    let on_custom = format!("/links/sale?domain={CUSTOM}");

    let res = app.api("GET", &on_custom, None).await;
    assert_eq!(res.json()["target"], "https://example.com/custom");
    let res = app.api("GET", "/links/sale", None).await;
    assert_eq!(res.json()["target"], "https://example.com/default");

    let body = json!({ "target": "https://example.com/edited" });
    let res = app.api("PATCH", &on_custom, Some(body)).await;
    assert_eq!(res.json()["domain"], CUSTOM);
    assert_eq!(res.json()["target"], "https://example.com/edited");

    let res = app
        .api("GET", &format!("/links/sale/qr?domain={CUSTOM}"), None)
        .await;
    assert_eq!(res.status, StatusCode::OK);

    let res = app.api("DELETE", &on_custom, None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get_on(CUSTOM, "/sale").await.status, StatusCode::GONE);
    assert_eq!(app.get_on(HOST, "/sale").await.status, StatusCode::FOUND);
}
