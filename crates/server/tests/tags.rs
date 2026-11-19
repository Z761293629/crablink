mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

/// `GET` a list page, asserting `200`; returns `(codes, next_cursor)`.
async fn page(app: &TestApp, query: &str) -> (Vec<String>, Value) {
    let res = app.api("GET", &format!("/links{query}"), None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let body = res.json();
    let codes = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["code"].as_str().unwrap().to_string())
        .collect();
    (codes, body["next_cursor"].clone())
}

/// Every Code `query` matches, following `next_cursor` one Link per page.
async fn all_pages(app: &TestApp, query: &str) -> Vec<String> {
    let (mut seen, mut cursor) = page(app, &format!("?limit=1&{query}")).await;
    while let Some(c) = cursor.as_str() {
        let (codes, next) = page(app, &format!("?limit=1&cursor={c}&{query}")).await;
        seen.extend(codes);
        cursor = next;
    }
    seen
}

#[tokio::test]
async fn a_link_is_created_with_its_tags_sorted_and_deduplicated() {
    let app = app().await;
    let link = app
        .create(json!({
            "target": "https://example.com",
            "code": "tagged",
            "tags": ["spring", "launch", "spring"],
        }))
        .await;
    assert_eq!(link["tags"], json!(["launch", "spring"]));

    let untagged = app
        .create(json!({ "target": "https://example.com", "code": "plain" }))
        .await;
    assert_eq!(untagged["tags"], json!([]));

    let got = app.api("GET", "/links/tagged", None).await.json();
    assert_eq!(got["tags"], json!(["launch", "spring"]));
}

#[tokio::test]
async fn a_tag_is_unique_per_workspace_and_listed_once() {
    let app = app().await;
    for code in ["one", "two"] {
        let body = json!({ "target": "https://example.com", "code": code, "tags": ["launch"] });
        app.create(body).await;
    }
    let body = json!({ "target": "https://example.com", "code": "gone", "tags": ["old"] });
    app.create(body).await;
    app.api("DELETE", "/links/gone", None).await;

    let other = app.account("other@example.com").await;
    let path = format!("/api/workspaces/{}/links", other.slug);
    let body = json!({ "target": "https://example.com", "tags": ["launch", "secret"] });
    let res = app
        .bearer("POST", &path, Some(body), Some(&other.key))
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);

    // Only Tags on live Links, each once, and never another Workspace's.
    let res = app.api("GET", "/tags", None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json(), json!(["launch"]));
    let path = format!("/api/workspaces/{}/tags", other.slug);
    let res = app.bearer("GET", &path, None, Some(&other.key)).await;
    assert_eq!(res.json(), json!(["launch", "secret"]));
    // Filtering by the other Workspace's Tag finds none of its Links.
    assert_eq!(page(&app, "?tag=secret").await.0, Vec::<String>::new());
}

#[tokio::test]
async fn filtering_by_tag_pages_through_only_its_links() {
    let app = app().await;
    for (code, tags) in [
        ("aaa1", json!(["spring"])),
        ("bbb1", json!(["autumn"])),
        ("aaa2", json!(["spring", "autumn"])),
        ("ccc1", json!([])),
        ("aaa3", json!(["spring"])),
    ] {
        let body = json!({ "target": "https://example.com", "code": code, "tags": tags });
        app.create(body).await;
    }

    assert_eq!(
        all_pages(&app, "tag=spring").await,
        ["aaa3", "aaa2", "aaa1"]
    );
    assert_eq!(all_pages(&app, "tag=autumn").await, ["aaa2", "bbb1"]);
    assert_eq!(all_pages(&app, "tag=Spring").await, Vec::<String>::new());
}

#[tokio::test]
async fn searching_matches_part_of_a_code_or_target_ignoring_case_across_pages() {
    let app = app().await;
    for (code, target) in [
        ("promo1", "https://example.com/a"),
        ("other", "https://shop.example.com/PROMO/b"),
        ("misc", "https://example.org/c"),
        ("Promo2", "https://example.net/d"),
        ("pct", "https://example.net/x_50%25"),
        ("like", "https://example.net/a150b"),
    ] {
        app.create(json!({ "target": target, "code": code })).await;
    }

    assert_eq!(
        all_pages(&app, "q=promo").await,
        ["Promo2", "other", "promo1"]
    );
    assert_eq!(all_pages(&app, "q=example.org").await, ["misc"]);
    // `%` and `_` are literal characters, not wildcards.
    assert_eq!(all_pages(&app, "q=_50%25").await, ["pct"]);
    assert_eq!(all_pages(&app, "q=nothing").await, Vec::<String>::new());
}

#[tokio::test]
async fn tag_and_search_combine() {
    let app = app().await;
    for (code, tags) in [
        ("promo1", json!(["spring"])),
        ("promo2", json!([])),
        ("misc", json!(["spring"])),
    ] {
        let body = json!({ "target": "https://example.com", "code": code, "tags": tags });
        app.create(body).await;
    }
    assert_eq!(all_pages(&app, "tag=spring&q=promo").await, ["promo1"]);
}

#[tokio::test]
async fn patching_tags_replaces_them_and_leaves_the_rest_alone() {
    let app = app().await;
    app.create(json!({ "target": "https://example.com", "code": "abc", "tags": ["a", "b"] }))
        .await;

    let res = app
        .api("PATCH", "/links/abc", Some(json!({ "tags": ["b", "c"] })))
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["tags"], json!(["b", "c"]));
    assert_eq!(res.json()["target"], "https://example.com");

    // Leaving `tags` out keeps them; `[]` removes them all.
    let patch = json!({ "target": "https://example.org" });
    let res = app.api("PATCH", "/links/abc", Some(patch)).await;
    assert_eq!(res.json()["tags"], json!(["b", "c"]));
    let res = app
        .api("PATCH", "/links/abc", Some(json!({ "tags": [] })))
        .await;
    assert_eq!(res.json()["tags"], json!([]));
}

#[tokio::test]
async fn tag_names_are_trimmed_and_bad_ones_rejected() {
    let app = app().await;
    let link = app
        .create(json!({ "target": "https://example.com", "tags": ["  spring  "] }))
        .await;
    assert_eq!(link["tags"], json!(["spring"]));
    let code = link["code"].as_str().unwrap();

    let too_long = "x".repeat(65);
    for bad in ["", "   ", "a,b", "tab\there", too_long.as_str()] {
        let body = json!({ "target": "https://example.com", "tags": [bad] });
        let res = app.api("POST", "/links", Some(body)).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{bad:?}: {}", res.text);
        assert_eq!(res.json()["kind"], "invalid_tag", "{}", res.text);

        let body = json!({ "tags": [bad] });
        let res = app
            .api("PATCH", &format!("/links/{code}"), Some(body))
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{bad:?}: {}", res.text);
        assert_eq!(res.json()["kind"], "invalid_tag", "{}", res.text);
    }
}
