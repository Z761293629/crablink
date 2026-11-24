mod common;

use axum::http::StatusCode;
use chrono::TimeDelta;
use common::*;
use serde_json::json;

const ADA: &str = "ada@example.com";
const NEW_PASSWORD: &str = "a brand new password";

/// `POST /api/password-resets` asking for a reset link for `email`, from this host.
async fn forgot(app: &TestApp, email: &str) -> Res {
    let body = json!({ "email": email });
    app.request(
        "POST",
        "/api/password-resets",
        Some(body),
        &[("origin", ORIGIN)],
    )
    .await
}

/// Follows reset link `link`, choosing `password`.
async fn reset(app: &TestApp, link: &str, password: &str) -> Res {
    let (base, token) = link.split_once('#').unwrap();
    assert_eq!(base, format!("{BASE_URL}/app/reset-password"));
    let body = json!({ "token": token, "password": password });
    app.request(
        "POST",
        "/api/password-resets/use",
        Some(body),
        &[("origin", ORIGIN)],
    )
    .await
}

#[tokio::test]
async fn a_forgotten_password_is_reset_through_the_emailed_link() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    let elsewhere = app.sign_in(ADA, PASSWORD).await;

    let res = forgot(&app, ADA).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = reset(&app, &mailbox.link_to(ADA), NEW_PASSWORD).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["email"], ADA);
    assert_eq!(res.json()["has_password"], true);

    // Signed in by the reset, and signed out everywhere else.
    let cookie = res.header("set-cookie").split(';').next().unwrap();
    let me = app.as_user("GET", "/api/me", None, cookie).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.text);
    let me = app.as_user("GET", "/api/me", None, &elsewhere).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.text);
    assert_eq!(
        app.sign_in_res(ADA, PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );
    app.sign_in(ADA, NEW_PASSWORD).await;
}

#[tokio::test]
async fn asking_for_a_reset_answers_the_same_whether_or_not_a_user_has_the_email() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    let known = forgot(&app, "ADA@example.com").await;
    let unknown = forgot(&app, "nobody@example.com").await;

    assert_eq!(known.status, StatusCode::NO_CONTENT, "{}", known.text);
    assert_eq!((unknown.status, &unknown.text), (known.status, &known.text));
    assert_eq!(mailbox.to(ADA).len(), 1);
    assert!(mailbox.to("nobody@example.com").is_empty());
}

#[tokio::test]
async fn a_disabled_user_gets_no_reset_link() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    crablink_server::set_user_disabled(&app.pool, ADA, true)
        .await
        .unwrap();

    let res = forgot(&app, ADA).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert!(mailbox.to(ADA).is_empty());
}

#[tokio::test]
async fn a_user_without_a_password_gets_no_reset_link() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    // As if an Identity Provider had created Ada.
    sqlx::query("UPDATE users SET password_hash = NULL WHERE email = $1")
        .bind(ADA)
        .execute(&app.pool)
        .await
        .unwrap();

    let res = forgot(&app, ADA).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert!(mailbox.to(ADA).is_empty());
}

fn assert_expired(res: &Res) {
    assert_eq!(res.status, StatusCode::GONE, "{}", res.text);
    assert_eq!(res.json()["kind"], "reset_link_expired", "{}", res.text);
}

#[tokio::test]
async fn a_reset_link_works_once() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    forgot(&app, ADA).await;
    let link = mailbox.link_to(ADA);
    forgot(&app, ADA).await;
    let newer = mailbox.link_to(ADA);

    let res = reset(&app, &link, NEW_PASSWORD).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_expired(&reset(&app, &link, "yet another password").await);
    // Using one link uses them all.
    assert_expired(&reset(&app, &newer, "yet another password").await);
    app.sign_in(ADA, NEW_PASSWORD).await;
}

#[tokio::test]
async fn a_reset_link_expires_after_an_hour() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    forgot(&app, ADA).await;
    let link = mailbox.link_to(ADA);

    app.advance(TimeDelta::hours(1));
    assert_expired(&reset(&app, &link, NEW_PASSWORD).await);
    app.sign_in(ADA, PASSWORD).await;
}

#[tokio::test]
async fn an_unknown_reset_link_is_refused() {
    let (app, _) = app_with_email().await;
    let link = format!("{BASE_URL}/app/reset-password#{}", "0".repeat(64));
    assert_expired(&reset(&app, &link, NEW_PASSWORD).await);
}

#[tokio::test]
async fn reset_requests_are_limited_per_visitor_ip() {
    let mailbox = Mailbox::default();
    let mailer = mailbox.mailer();
    let app = app_with(|s| crablink_server::Settings {
        mailer: Some(mailer),
        trust_proxy: true,
        rate_limits: crablink_server::RateLimits {
            password_reset: Some("2/1h".parse().unwrap()),
            ..Default::default()
        },
        ..s
    })
    .await;
    for email in [ADA, "nobody@example.com"] {
        let res = forgot_from(&app, NZ_IP, email).await;
        assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    }

    let res = forgot_from(&app, NZ_IP, ADA).await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS, "{}", res.text);
    assert_eq!(res.json()["kind"], "rate_limited");
    assert_eq!(res.header("retry-after"), "3600");
    let res = forgot_from(&app, SOCKET_IP, ADA).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
}

/// `forgot` from visitor IP `ip`, as a trusted proxy says.
async fn forgot_from(app: &TestApp, ip: &str, email: &str) -> Res {
    let body = json!({ "email": email });
    let headers = [("origin", ORIGIN), ("x-forwarded-for", ip)];
    app.request("POST", "/api/password-resets", Some(body), &headers)
        .await
}

#[tokio::test]
async fn emails_follow_the_language_of_the_web_ui_that_asked() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    forgot(&app, ADA).await;
    let body = json!({ "email": ADA });
    let headers = [("origin", ORIGIN), ("accept-language", "zh-CN")];
    let res = app
        .request("POST", "/api/password-resets", Some(body), &headers)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let [english, chinese] = &mailbox.to(ADA)[..] else {
        panic!("not two emails");
    };
    assert_eq!(english.subject, "Reset your password on crabl.ink");
    assert_eq!(chinese.subject, "重置你在 crabl.ink 的密码");
    assert!(chinese.body.contains("/app/reset-password#"), "{chinese:?}");
}

#[tokio::test]
async fn without_email_users_cant_reset_a_password_themselves() {
    let app = app().await;
    let res = app.get("/api/instance").await;
    assert_eq!(res.json()["password_reset"], false, "{}", res.text);
    let res = forgot(&app, OWNER).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);

    let (app, _) = app_with_email().await;
    let res = app.get("/api/instance").await;
    assert_eq!(res.json()["password_reset"], true, "{}", res.text);
}

#[tokio::test]
async fn resetting_needs_this_hosts_origin_and_a_long_enough_password() {
    let (app, mailbox) = app_with_email().await;
    app.create_user(ADA).await;
    let res = app
        .request(
            "POST",
            "/api/password-resets",
            Some(json!({ "email": ADA })),
            &[("origin", "https://evil.example")],
        )
        .await;
    assert_eq!(res.json()["kind"], "origin_mismatch", "{}", res.text);
    assert!(mailbox.to(ADA).is_empty());

    forgot(&app, ADA).await;
    let link = mailbox.link_to(ADA);
    let res = reset(&app, &link, "short").await;
    assert_eq!(res.json()["kind"], "password_too_short", "{}", res.text);
    // The link still works.
    let res = reset(&app, &link, NEW_PASSWORD).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}
