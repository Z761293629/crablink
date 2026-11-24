//! Open sign-up: anyone signs up with an email and a password, verifies the email
//! through an emailed link, and only then creates Links and API Keys.

mod common;
use common::*;

use axum::http::StatusCode;
use chrono::TimeDelta;
use serde_json::json;

const ADA: &str = "ada@example.com";

/// An app with open sign-up on, sending email to the returned fake mailer.
async fn open_app() -> (TestApp, Mailbox) {
    open_app_with(|s| s).await
}

async fn open_app_with(
    change: impl FnOnce(crablink_server::Settings) -> crablink_server::Settings,
) -> (TestApp, Mailbox) {
    let mailbox = Mailbox::default();
    let mailer = mailbox.mailer();
    let app = app_with(|s| {
        change(crablink_server::Settings {
            open_sign_up: true,
            mailer: Some(mailer),
            ..s
        })
    })
    .await;
    (app, mailbox)
}

/// `POST /api/sign-ups` with `email` and `PASSWORD`, from this host.
async fn sign_up(app: &TestApp, email: &str) -> Res {
    let body = json!({ "email": email, "password": PASSWORD });
    app.request("POST", "/api/sign-ups", Some(body), &[("origin", ORIGIN)])
        .await
}

/// Signs `email` up, asserting `200`; returns the new session's cookie.
async fn signed_up(app: &TestApp, email: &str) -> String {
    let res = sign_up(app, email).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.cookie("crablink_session").expect("a session cookie")
}

/// Follows verification link `link`.
async fn verify(app: &TestApp, link: &str) -> Res {
    let (base, token) = link.split_once('#').unwrap();
    assert_eq!(base, format!("{BASE_URL}/app/verify-email"));
    let body = json!({ "token": token });
    app.request(
        "POST",
        "/api/email-verifications/use",
        Some(body),
        &[("origin", ORIGIN)],
    )
    .await
}

async fn me(app: &TestApp, cookie: &str) -> serde_json::Value {
    let res = app.as_user("GET", "/api/me", None, cookie).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()["user"].clone()
}

/// `POST /api/workspaces/{slug}/links` as the session `cookie`.
async fn create_link(app: &TestApp, cookie: &str, slug: &str) -> Res {
    let path = format!("/api/workspaces/{slug}/links");
    let body = json!({ "target": "https://example.com/" });
    app.as_user("POST", &path, Some(body), cookie).await
}

fn refused_unverified(res: &Res) {
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(res.json()["kind"], "email_not_verified", "{}", res.text);
}

#[tokio::test]
async fn open_sign_up_needs_email() {
    let err = try_app_with(|s| crablink_server::Settings {
        open_sign_up: true,
        ..s
    })
    .await
    .err()
    .expect("refused to start");
    assert!(err.contains("CRABLINK_OPEN_SIGN_UP"), "{err}");
    assert!(err.contains("CRABLINK_SMTP_URL"), "{err}");
}

#[tokio::test]
async fn without_open_sign_up_nobody_signs_up() {
    let (app, _mailbox) = app_with_email().await;
    let res = sign_up(&app, ADA).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(
        app.sign_in_res(ADA, PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );

    let instance = app.get("/api/instance").await.json();
    assert_eq!(instance["sign_up"], false);
}

#[tokio::test]
async fn a_user_signs_up_verifies_their_email_then_creates_links_and_api_keys() {
    let (app, mailbox) = open_app().await;
    let res = sign_up(&app, ADA).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["email"], ADA);
    assert_eq!(res.json()["email_verified"], false);
    let cookie = res.cookie("crablink_session").expect("a session cookie");

    // Signed in, with a Personal Workspace, but nothing to create yet.
    assert_eq!(me(&app, &cookie).await["email_verified"], false);
    let slug = app.workspace_of(&cookie).await;
    refused_unverified(&create_link(&app, &cookie, &slug).await);
    let path = format!("/api/workspaces/{slug}/api-keys");
    refused_unverified(
        &app.as_user("POST", &path, Some(json!({ "name": "CI" })), &cookie)
            .await,
    );
    let csv = "code,domain,target\n,,https://example.com/\n";
    let path = format!("/api/workspaces/{slug}/links.csv");
    let req = axum::http::Request::post(&path)
        .header("host", HOST)
        .header("origin", ORIGIN)
        .header("cookie", &cookie)
        .body(axum::body::Body::from(csv))
        .unwrap();
    refused_unverified(&app.send(req).await);

    // Signing in again works too.
    app.sign_in(ADA, PASSWORD).await;

    let res = verify(&app, &mailbox.link_to(ADA)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(me(&app, &cookie).await["email_verified"], true);
    let res = create_link(&app, &cookie, &slug).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    app.api_key(&cookie, &slug).await;
}

#[tokio::test]
async fn the_verification_email_is_in_the_language_of_the_ui() {
    let (app, mailbox) = open_app().await;
    let body = json!({ "email": ADA, "password": PASSWORD });
    let res = app
        .request(
            "POST",
            "/api/sign-ups",
            Some(body),
            &[("origin", ORIGIN), ("accept-language", "zh-CN")],
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let email = mailbox.to(ADA).pop().unwrap();
    assert!(email.subject.contains("验证"), "{email:?}");
}

#[tokio::test]
async fn a_verification_link_works_once_and_expires() {
    let (app, mailbox) = open_app().await;
    signed_up(&app, ADA).await;
    let link = mailbox.link_to(ADA);
    assert_eq!(verify(&app, &link).await.status, StatusCode::NO_CONTENT);
    let again = verify(&app, &link).await;
    assert_eq!(again.status, StatusCode::GONE, "{}", again.text);
    assert_eq!(again.json()["kind"], "verification_link_expired");

    signed_up(&app, "bob@example.com").await;
    let link = mailbox.link_to("bob@example.com");
    app.advance(TimeDelta::hours(24) + TimeDelta::seconds(1));
    let res = verify(&app, &link).await;
    assert_eq!(res.status, StatusCode::GONE, "{}", res.text);
    assert_eq!(res.json()["kind"], "verification_link_expired");
}

#[tokio::test]
async fn an_unverified_user_asks_for_the_verification_email_again() {
    let (app, mailbox) = open_app().await;
    let cookie = signed_up(&app, ADA).await;
    let res = app
        .as_user("POST", "/api/email-verifications", None, &cookie)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(mailbox.to(ADA).len(), 2);

    let res = verify(&app, &mailbox.link_to(ADA)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    // Verified, so nothing more is sent.
    let res = app
        .as_user("POST", "/api/email-verifications", None, &cookie)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(mailbox.to(ADA).len(), 2);
}

#[tokio::test]
async fn an_email_signs_up_once() {
    let (app, _mailbox) = open_app().await;
    app.create_user(ADA).await;
    let res = sign_up(&app, "ADA@example.com").await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.json()["kind"], "email_taken");
}

#[tokio::test]
async fn sign_up_and_asking_for_the_email_again_are_rate_limited_per_visitor_ip() {
    let (app, _mailbox) = open_app_with(|s| crablink_server::Settings {
        rate_limits: crablink_server::RateLimits {
            sign_up: Some("2/1h".parse().unwrap()),
            ..Default::default()
        },
        ..s
    })
    .await;
    let cookie = signed_up(&app, ADA).await;
    let res = app
        .as_user("POST", "/api/email-verifications", None, &cookie)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = sign_up(&app, "bob@example.com").await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS, "{}", res.text);
    assert_eq!(res.json()["kind"], "rate_limited");
    assert!(!res.header("retry-after").is_empty());
}

#[tokio::test]
async fn users_created_any_other_way_count_as_verified() {
    let (app, _mailbox) = open_app().await;
    // By an Instance Admin (`crablink-server admin create`), as `TestApp` creates them.
    assert_eq!(me(&app, &app.owner.cookie).await["email_verified"], true);

    // By Invite.
    let admin = app.instance_admin().await;
    let body = json!({ "email": ADA });
    let res = app
        .as_user("POST", "/api/admin/invites", Some(body), &admin)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let token = res.json()["url"]
        .as_str()
        .unwrap()
        .split_once('#')
        .unwrap()
        .1
        .to_owned();
    let body = json!({ "token": token, "password": PASSWORD });
    let res = app
        .request(
            "POST",
            "/api/invites/accept",
            Some(body),
            &[("origin", ORIGIN)],
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["email_verified"], true);
}

#[tokio::test]
async fn resetting_the_password_through_the_emailed_link_verifies_the_email() {
    let (app, mailbox) = open_app().await;
    signed_up(&app, ADA).await;
    let body = json!({ "email": ADA });
    let res = app
        .request(
            "POST",
            "/api/password-resets",
            Some(body),
            &[("origin", ORIGIN)],
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let token = mailbox.link_to(ADA).split_once('#').unwrap().1.to_owned();
    let body = json!({ "token": token, "password": "a brand new password" });
    let res = app
        .request(
            "POST",
            "/api/password-resets/use",
            Some(body),
            &[("origin", ORIGIN)],
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json()["email_verified"], true);
}

#[tokio::test]
async fn the_sign_up_page_links_the_terms_and_privacy_policy_when_set() {
    let (app, _mailbox) = open_app_with(|s| crablink_server::Settings {
        terms_url: Some("https://crabl.ink/terms".into()),
        privacy_url: Some("https://crabl.ink/privacy".into()),
        ..s
    })
    .await;
    let instance = app.get("/api/instance").await.json();
    assert_eq!(instance["sign_up"], true);
    assert_eq!(instance["terms_url"], "https://crabl.ink/terms");
    assert_eq!(instance["privacy_url"], "https://crabl.ink/privacy");

    let (app, _mailbox) = open_app().await;
    let instance = app.get("/api/instance").await.json();
    assert_eq!(instance["terms_url"], serde_json::Value::Null);
    assert_eq!(instance["privacy_url"], serde_json::Value::Null);
}

/// An app with open sign-up and the fake set up as Identity Provider `fake`,
/// allowing no email domain.
async fn open_app_and_fake() -> (TestApp, Mailbox, FakeIdentityProvider) {
    let fake = FakeIdentityProvider::start().await;
    let provider = fake.identity_provider("fake", "Fake ID");
    let (app, mailbox) = open_app_with(|s| crablink_server::Settings {
        identity_providers: vec![provider],
        ..s
    })
    .await;
    (app, mailbox, fake)
}

#[tokio::test]
async fn with_open_sign_up_any_identity_provider_creates_verified_users() {
    let (app, _mailbox, fake) = open_app_and_fake().await;
    let res = app
        .sign_in_through(&fake, "fake", Person::verified("eve@example.org"))
        .await;
    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.text);
    assert_eq!(res.header("location"), "/app/");
    let cookie = res.cookie("crablink_session").expect("a session cookie");
    let user = me(&app, &cookie).await;
    assert_eq!(user["email"], "eve@example.org");
    assert_eq!(user["email_verified"], true);
    let slug = app.workspace_of(&cookie).await;
    let res = create_link(&app, &cookie, &slug).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
}

#[tokio::test]
async fn an_identity_provider_vouching_for_a_signed_up_email_verifies_it_and_drops_the_password() {
    // Whoever signed up with the email may not own it: the password they chose
    // must not keep working once the email's owner signs in.
    let (app, _mailbox, fake) = open_app_and_fake().await;
    let squatter = signed_up(&app, ADA).await;
    let res = app
        .sign_in_through(&fake, "fake", Person::verified(ADA))
        .await;
    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.text);
    let cookie = res.cookie("crablink_session").expect("a session cookie");

    let user = me(&app, &cookie).await;
    assert_eq!(user["email_verified"], true);
    assert_eq!(user["has_password"], false);
    let res = app.as_user("GET", "/api/me", None, &squatter).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    assert_eq!(
        app.sign_in_res(ADA, PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );
}
