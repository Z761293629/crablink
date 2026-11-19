mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;

/// `GET links.csv` of Workspace `slug` with API Key `key`, asserting `200`; returns the CSV.
async fn export(app: &TestApp, slug: &str, key: &str) -> String {
    let path = format!("/api/workspaces/{slug}/links.csv");
    let res = app.bearer("GET", &path, None, Some(key)).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.header("content-type"), "text/csv; charset=utf-8");
    res.text
}

/// `POST links.csv` of Workspace `slug` with API Key `key`, asserting `200`; returns
/// the per-row results.
async fn import(app: &TestApp, slug: &str, key: &str, csv: &str) -> serde_json::Value {
    let req = Request::post(format!("/api/workspaces/{slug}/links.csv"))
        .header(header::HOST, HOST)
        .header(header::AUTHORIZATION, format!("Bearer {key}"))
        .header(header::CONTENT_TYPE, "text/csv")
        .body(Body::from(csv.to_owned()))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

/// A human visitor following `/{code}` on `domain`.
async fn click(app: &TestApp, domain: &str, code: &str) {
    let ua = [("user-agent", BROWSER_UA)];
    let res = app.visit_on(domain, &format!("/{code}"), &ua).await;
    assert_eq!(res.status, StatusCode::FOUND);
}

#[tokio::test]
async fn export_then_import_into_another_workspace_on_another_domain_reproduces_the_links() {
    let app = app().await;
    app.approved_domain("go.a.example").await;
    app.create(json!({
        "target": "https://example.com/sale",
        "code": "sale",
        "domain": "go.a.example",
        "expires_at": "2030-01-01T00:00:00Z",
        "tags": ["spring", "launch"],
    }))
    .await;
    app.create(json!({ "target": "https://example.com/plain?a=1,b=2", "code": "plain", "domain": "go.a.example" }))
        .await;
    click(&app, "go.a.example", "sale").await;

    let exported = export(&app, &app.owner.slug, &app.owner.key).await;
    assert_eq!(
        exported,
        "Code,Domain,Target,Expiry,Tags,Click Count\n\
         sale,go.a.example,https://example.com/sale,2030-01-01T00:00:00Z,\"launch,spring\",1\n\
         plain,go.a.example,\"https://example.com/plain?a=1,b=2\",,,0\n"
    );

    let other = app.account("other@example.com").await;
    let res = app
        .add_domain(&other.cookie, &other.slug, "go.b.example")
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    app.set_domain_status("go.b.example", "approved").await;
    let moved = exported.replace("go.a.example", "go.b.example");
    let rows = import(&app, &other.slug, &other.key, &moved).await;
    assert_eq!(
        rows,
        json!([
            { "line": 2, "short_url": "https://go.b.example/sale" },
            { "line": 3, "short_url": "https://go.b.example/plain" },
        ])
    );

    assert_eq!(
        export(&app, &other.slug, &other.key).await,
        "Code,Domain,Target,Expiry,Tags,Click Count\n\
         sale,go.b.example,https://example.com/sale,2030-01-01T00:00:00Z,\"launch,spring\",0\n\
         plain,go.b.example,\"https://example.com/plain?a=1,b=2\",,,0\n"
    );
}

#[tokio::test]
async fn import_creates_the_valid_rows_and_reports_each_skipped_one_with_its_kind() {
    let app = app_with_codes(&["gen1234"]).await;
    app.create(json!({ "target": "https://example.com", "code": "taken" }))
        .await;
    app.create(json!({ "target": "https://example.com", "code": "gone" }))
        .await;
    app.api("DELETE", "/links/gone", None).await;

    let csv = "Code,Domain,Target,Expiry,Tags,Click Count\n\
               fresh,,https://example.com/fresh,,,7\n\
               taken,crabl.ink,https://example.com/x,,,\n\
               gone,,https://example.com/y,,,\n\
               ,,https://example.com/generated,,\"docs, guides\",\n\
               elsewhere,nope.example,https://example.com/z,,,\n\
               late,,https://example.com/late,tomorrow,,\n";
    let rows = import(&app, &app.owner.slug, &app.owner.key, csv).await;
    let skipped =
        |line, kind, error| json!({ "line": line, "skipped": { "kind": kind, "error": error } });
    assert_eq!(
        rows[0],
        json!({ "line": 2, "short_url": "https://crabl.ink/fresh" })
    );
    assert_eq!(rows[1], skipped(3, "code_taken", "code already taken"));
    assert_eq!(rows[2], skipped(4, "code_taken", "code already taken"));
    assert_eq!(
        rows[3],
        json!({ "line": 5, "short_url": "https://crabl.ink/gen1234" })
    );
    assert_eq!(
        rows[4],
        skipped(
            6,
            "domain_not_approved",
            "domain is neither the Default Domain nor an approved Domain of this Workspace"
        )
    );
    assert_eq!(rows[5]["skipped"]["kind"], "bad_request");
    assert_eq!(rows.as_array().unwrap().len(), 6);

    let fresh = app.api("GET", "/links/fresh", None).await.json();
    assert_eq!(fresh["click_count"], 0);
    let generated = app.api("GET", "/links/gen1234", None).await.json();
    assert_eq!(generated["tags"], json!(["docs", "guides"]));
    assert_eq!(generated["target"], "https://example.com/generated");
    let taken = app.api("GET", "/links/taken", None).await.json();
    assert_eq!(taken["target"], "https://example.com");
}
