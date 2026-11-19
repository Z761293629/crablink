mod common;

use axum::http::StatusCode;
use chrono::TimeDelta;
use common::*;
use serde_json::{Value, json};

async fn click_count(app: &TestApp, code: &str) -> serde_json::Value {
    let res = app.api("GET", &format!("/links/{code}"), None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()["click_count"].clone()
}

/// `GET …/links/{code}/clicks` with `query`, asserting `200`.
async fn clicks(app: &TestApp, code: &str, query: &str) -> Value {
    let res = app
        .api("GET", &format!("/links/{code}/clicks?{query}"), None)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

/// The countries of Link `code`'s Clicks in the day around the app's clock.
async fn countries(app: &TestApp, code: &str) -> Value {
    let day = TimeDelta::days(1);
    let (from, to) = (app.now() - day, app.now() + day);
    let query = format!(
        "from={}&to={}&interval=day",
        from.format("%Y-%m-%dT%H:%M:%SZ"),
        to.format("%Y-%m-%dT%H:%M:%SZ")
    );
    clicks(app, code, &query).await["countries"].clone()
}

#[tokio::test]
async fn a_browser_makes_a_click_but_bots_and_link_previews_do_not() {
    let app = app_with_codes(&["Click01"]).await;
    app.create(json!({ "target": "https://example.com" })).await;

    for ua in [
        BROWSER_UA,
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "Slackbot-LinkExpanding 1.0 (+https://api.slack.com/robots)",
    ] {
        let res = app.visit("/Click01", &[("user-agent", ua)]).await;
        assert_eq!(res.status, StatusCode::FOUND, "{ua}");
        assert_eq!(res.header("location"), "https://example.com", "{ua}");
    }

    assert_eq!(click_count(&app, "Click01").await, 1);
}

#[tokio::test]
async fn behind_a_trusted_proxy_the_country_comes_from_the_first_forwarded_address() {
    let app = app_behind_proxy().await;
    app.create(json!({ "code": "geo", "target": "https://example.com" }))
        .await;
    let forwarded = format!("{NZ_IP}, 10.0.0.1");

    let headers = [
        ("user-agent", BROWSER_UA),
        ("x-forwarded-for", forwarded.as_str()),
    ];
    app.visit("/geo", &headers).await;

    assert_eq!(
        countries(&app, "geo").await,
        json!([{ "value": "NZ", "count": 1 }])
    );
}

/// Recording a Click never holds up the redirect, even when the database is stuck.
#[tokio::test]
async fn the_redirect_does_not_wait_for_its_click_to_be_recorded() {
    use tower::ServiceExt;
    let app = app().await;
    app.create(json!({ "code": "fast", "target": "https://example.com" }))
        .await;
    let mut lock = app.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE clicks IN EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();

    let req = visitor_request(HOST, "/fast", &[("user-agent", BROWSER_UA)]);
    let redirect = app.router.clone().oneshot(req);
    let res = tokio::time::timeout(std::time::Duration::from_secs(10), redirect)
        .await
        .expect("the redirect waited for the Click")
        .unwrap();

    assert_eq!(res.status(), StatusCode::FOUND);
    lock.commit().await.unwrap();
    let click = res.extensions().get::<crablink_server::ClickRecording>();
    click.expect("a Click is being recorded").done().await;
    assert_eq!(click_count(&app, "fast").await, 1);
}

#[tokio::test]
async fn behind_a_trusted_proxy_with_no_forwarded_address_the_country_comes_from_the_socket() {
    let app = app_behind_proxy().await;
    app.create(json!({ "code": "geo", "target": "https://example.com" }))
        .await;

    app.visit("/geo", &[("user-agent", BROWSER_UA)]).await;

    assert_eq!(
        countries(&app, "geo").await,
        json!([{ "value": "FR", "count": 1 }])
    );
}

#[tokio::test]
async fn without_proxy_trust_the_country_comes_from_the_socket() {
    let app = app().await;
    app.create(json!({ "code": "geo", "target": "https://example.com" }))
        .await;

    let headers = [("user-agent", BROWSER_UA), ("x-forwarded-for", NZ_IP)];
    app.visit("/geo", &headers).await;

    assert_eq!(
        countries(&app, "geo").await,
        json!([{ "value": "FR", "count": 1 }])
    );
}

const IPHONE_SAFARI: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const IPAD_FIREFOX: &str = "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/130.0 Mobile/15E148 Safari/605.1.15";
const WINDOWS_EDGE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
const ANDROID_CHROME: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
const HN: &str = "https://news.ycombinator.com/item?id=1";

#[tokio::test]
async fn clicks_are_counted_per_hour_or_day_and_broken_down() {
    let app = app_behind_proxy().await;
    app.create(json!({ "code": "stats", "target": "https://example.com" }))
        .await;
    let visits = [
        ("2026-03-01T10:15:00Z", BROWSER_UA, Some(HN), NZ_IP),
        ("2026-03-01T10:45:00Z", IPHONE_SAFARI, None, NZ_IP),
        (
            "2026-03-01T12:05:00Z",
            IPAD_FIREFOX,
            Some(HN),
            "203.0.113.9",
        ),
        ("2026-03-01T12:59:59Z", WINDOWS_EDGE, Some(HN), NZ_IP),
        // Outside the hourly range below.
        (
            "2026-03-02T09:00:00Z",
            ANDROID_CHROME,
            Some("https://t.co/x"),
            NZ_IP,
        ),
    ];
    for (at, ua, referrer, ip) in visits {
        app.set_now(at);
        let mut headers = vec![("user-agent", ua), ("x-forwarded-for", ip)];
        headers.extend(referrer.map(|r| ("referer", r)));
        assert_eq!(
            app.visit("/stats", &headers).await.status,
            StatusCode::FOUND
        );
    }

    let hourly = clicks(
        &app,
        "stats",
        "from=2026-03-01T10:00:00Z&to=2026-03-01T13:00:00Z&interval=hour",
    )
    .await;
    assert_eq!(
        hourly,
        json!({
            "series": [
                { "time": "2026-03-01T10:00:00Z", "count": 2 },
                { "time": "2026-03-01T11:00:00Z", "count": 0 },
                { "time": "2026-03-01T12:00:00Z", "count": 2 },
            ],
            "referrers": [
                { "value": "news.ycombinator.com", "count": 3 },
                { "value": null, "count": 1 },
            ],
            "countries": [{ "value": "NZ", "count": 3 }, { "value": null, "count": 1 }],
            "devices": [
                { "value": "desktop", "count": 2 },
                { "value": "mobile", "count": 1 },
                { "value": "tablet", "count": 1 },
            ],
            "browsers": [
                { "value": "Chrome", "count": 1 },
                { "value": "Edge", "count": 1 },
                { "value": "Firefox", "count": 1 },
                { "value": "Safari", "count": 1 },
            ],
        })
    );

    // A range that starts mid-day still buckets by whole UTC days.
    let daily = clicks(
        &app,
        "stats",
        "from=2026-03-01T10:30:00Z&to=2026-03-03T00:00:00Z",
    )
    .await;
    assert_eq!(
        daily["series"],
        json!([
            { "time": "2026-03-01T00:00:00Z", "count": 3 },
            { "time": "2026-03-02T00:00:00Z", "count": 1 },
        ])
    );
    assert_eq!(click_count(&app, "stats").await, 5);
}

#[tokio::test]
async fn a_range_that_ends_before_it_starts_is_a_bad_request() {
    let app = app().await;
    app.create(json!({ "code": "stats", "target": "https://example.com" }))
        .await;

    let path = "/links/stats/clicks?from=2026-03-02T00:00:00Z&to=2026-03-01T00:00:00Z";
    let res = app.api("GET", path, None).await;

    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert_eq!(res.json()["kind"], "bad_request");
}

#[tokio::test]
async fn clicks_of_an_unknown_link_are_not_found() {
    let app = app().await;

    let path = "/links/nope/clicks?from=2026-03-01T00:00:00Z&to=2026-03-02T00:00:00Z";
    let res = app.api("GET", path, None).await;

    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(res.json()["kind"], "link_not_found");
}

/// ADR-0006: nothing in the database holds the visitor's IP or full User-Agent.
#[tokio::test]
async fn a_click_stores_neither_the_ip_nor_the_user_agent() {
    let app = app_behind_proxy().await;
    app.create(json!({ "code": "private", "target": "https://example.com" }))
        .await;
    let forwarded = format!("{NZ_IP}, {SOCKET_IP}");
    let headers = [
        ("user-agent", BROWSER_UA),
        ("x-forwarded-for", forwarded.as_str()),
    ];
    app.visit("/private", &headers).await;
    assert_eq!(click_count(&app, "private").await, 1);

    let tables: Vec<String> =
        sqlx::query_scalar("SELECT tablename::text FROM pg_tables WHERE schemaname = 'public'")
            .fetch_all(&app.pool)
            .await
            .unwrap();
    for table in tables {
        let sql = format!("SELECT t::text FROM \"{table}\" t");
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_all(&app.pool)
            .await
            .unwrap();
        for row in rows {
            for secret in [NZ_IP, SOCKET_IP, BROWSER_UA] {
                assert!(!row.contains(secret), "{table} holds {secret}: {row}");
            }
        }
    }
}

#[tokio::test]
async fn workspace_clicks_total_its_links_and_rank_the_top_links() {
    let app = app().await;
    for code in ["one", "two", "three"] {
        app.create(json!({ "code": code, "target": "https://example.com" }))
            .await;
    }
    let other = app.account("other@example.com").await;
    let path = format!("/api/workspaces/{}/links", other.slug);
    let body = json!({ "code": "theirs", "target": "https://example.com" });
    let res = app
        .bearer("POST", &path, Some(body), Some(&other.key))
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let visits = [
        ("2026-03-01T10:00:00Z", "/one"),
        ("2026-03-01T11:00:00Z", "/two"),
        ("2026-03-01T12:00:00Z", "/two"),
        ("2026-03-02T09:00:00Z", "/two"),
        ("2026-03-02T09:30:00Z", "/three"),
        ("2026-03-02T10:00:00Z", "/three"),
        ("2026-03-02T10:00:00Z", "/theirs"),
        // After the range below.
        ("2026-03-03T00:00:00Z", "/one"),
        ("2026-03-03T00:00:00Z", "/one"),
    ];
    for (at, path) in visits {
        app.set_now(at);
        let res = app.visit(path, &[("user-agent", BROWSER_UA)]).await;
        assert_eq!(res.status, StatusCode::FOUND, "{path}");
    }

    let res = app
        .api(
            "GET",
            "/clicks?from=2026-03-01T00:00:00Z&to=2026-03-03T00:00:00Z",
            None,
        )
        .await;

    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let link = |code: &str, count: i64| {
        json!({
            "code": code,
            "domain": HOST,
            "short_url": format!("{BASE_URL}/{code}"),
            "count": count,
        })
    };
    assert_eq!(
        res.json(),
        json!({
            "series": [
                { "time": "2026-03-01T00:00:00Z", "count": 3 },
                { "time": "2026-03-02T00:00:00Z", "count": 3 },
            ],
            "top_links": [link("two", 3), link("three", 2), link("one", 1)],
        })
    );
}

/// Clicks are kept forever: a Workspace's past totals never shrink when a Link is
/// deleted or its Domain removed.
#[tokio::test]
async fn workspace_clicks_still_count_deleted_links_and_removed_domains() {
    let app = app().await;
    app.approved_domain("go.example.com").await;
    app.create(json!({ "code": "gone", "target": "https://example.com" }))
        .await;
    app.create(
        json!({ "code": "away", "domain": "go.example.com", "target": "https://example.com" }),
    )
    .await;
    app.set_now("2026-03-01T10:00:00Z");
    app.visit("/gone", &[("user-agent", BROWSER_UA)]).await;
    let res = app
        .visit_on("go.example.com", "/away", &[("user-agent", BROWSER_UA)])
        .await;
    assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);

    let res = app.api("DELETE", "/links/gone", None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let path = format!("/api/workspaces/{}/domains/go.example.com", app.owner.slug);
    let res = app.as_user("DELETE", &path, None, &app.owner.cookie).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let res = app
        .api(
            "GET",
            "/clicks?from=2026-03-01T00:00:00Z&to=2026-03-02T00:00:00Z",
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.json()["series"],
        json!([{ "time": "2026-03-01T00:00:00Z", "count": 2 }])
    );
}
