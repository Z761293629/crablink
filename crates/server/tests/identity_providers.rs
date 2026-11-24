//! Signing in through an Identity Provider, against the fake one in the harness.

mod common;
use common::*;

use axum::http::StatusCode;
use serde_json::json;

/// An app with the fake set up as Identity Provider `fake`, shown as "Fake ID".
async fn app_and_fake() -> (TestApp, FakeIdentityProvider) {
    let fake = FakeIdentityProvider::start().await;
    let provider = fake.identity_provider("fake", "Fake ID");
    let app = app_with(|s| crablink_server::Settings {
        identity_providers: vec![provider],
        ..s
    })
    .await;
    (app, fake)
}

/// Asserts the callback signed someone in and sent them to the web UI; returns
/// the session's cookie.
fn signed_in(res: &Res) -> String {
    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.text);
    assert_eq!(res.header("location"), "/app/");
    res.cookie("crablink_session").expect("a session cookie")
}

/// Asserts the callback refused with `kind`, sending the browser to the web UI's
/// sign-in page to say so, and started no Session.
fn refused(res: &Res, kind: &str) {
    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.text);
    assert_eq!(res.header("location"), format!("/app/?error={kind}"));
    assert_eq!(res.cookie("crablink_session"), None);
}

async fn me(app: &TestApp, cookie: &str) -> serde_json::Value {
    let res = app.as_user("GET", "/api/me", None, cookie).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.json()
}

#[tokio::test]
async fn the_sign_in_page_lists_the_identity_providers() {
    let (app, _fake) = app_and_fake().await;
    let res = app.get("/api/identity-providers").await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.json(), json!([{ "id": "fake", "name": "Fake ID" }]));

    let res = common::app().await.get("/api/identity-providers").await;
    assert_eq!(res.json(), json!([]));
}

#[tokio::test]
async fn a_user_signs_in_through_an_identity_provider_that_verified_their_email() {
    let (app, fake) = app_and_fake().await;
    app.create_user("ada@example.com").await;

    let res = app
        .sign_in_through(&fake, "fake", Person::verified("ADA@example.com"))
        .await;

    let cookie = signed_in(&res);
    let me = me(&app, &cookie).await;
    assert_eq!(me["user"]["email"], "ada@example.com");
}

#[tokio::test]
async fn afterwards_the_user_signs_in_either_way() {
    let (app, fake) = app_and_fake().await;
    app.create_user("ada@example.com").await;
    let ada = Person::verified("ada@example.com");
    signed_in(&app.sign_in_through(&fake, "fake", ada.clone()).await);

    // The identity is now Ada's, whatever email the Identity Provider says next.
    let renamed = Person {
        email: "ada@elsewhere.example".into(),
        email_verified: None,
        ..ada
    };
    let cookie = signed_in(&app.sign_in_through(&fake, "fake", renamed).await);
    assert_eq!(me(&app, &cookie).await["user"]["email"], "ada@example.com");
    app.sign_in("ada@example.com", PASSWORD).await;
}

#[tokio::test]
async fn an_email_the_identity_provider_didnt_verify_is_refused() {
    let (app, fake) = app_and_fake().await;
    app.create_user("ada@example.com").await;
    for email_verified in [None, Some(false)] {
        let person = Person {
            email_verified,
            ..Person::verified("ada@example.com")
        };
        let res = app.sign_in_through(&fake, "fake", person).await;
        refused(&res, "identity_provider_email_unverified");
    }
}

#[tokio::test]
async fn an_email_no_user_has_needs_an_invite() {
    let (app, fake) = app_and_fake().await;
    let res = app
        .sign_in_through(&fake, "fake", Person::verified("stranger@example.com"))
        .await;
    refused(&res, "invite_needed");
}

#[tokio::test]
async fn a_disabled_user_is_refused() {
    let (app, fake) = app_and_fake().await;
    app.create_user("ada@example.com").await;
    app.create_user("bob@example.com").await;
    let ada = Person::verified("ada@example.com");
    signed_in(&app.sign_in_through(&fake, "fake", ada.clone()).await);
    for email in ["ada@example.com", "bob@example.com"] {
        crablink_server::set_user_disabled(&app.pool, email, true)
            .await
            .unwrap();
    }

    refused(
        &app.sign_in_through(&fake, "fake", ada).await,
        "user_disabled",
    );
    let bob = Person::verified("bob@example.com");
    refused(
        &app.sign_in_through(&fake, "fake", bob).await,
        "user_disabled",
    );
}

#[tokio::test]
async fn a_tampered_or_wrongly_signed_id_token_is_refused() {
    let (app, fake) = app_and_fake().await;
    app.create_user("ada@example.com").await;
    for token in [Token::Tampered, Token::WrongKey] {
        let person = Person {
            token,
            ..Person::verified("ada@example.com")
        };
        let res = app.sign_in_through(&fake, "fake", person).await;
        refused(&res, "identity_provider_failed");
    }
}

#[tokio::test]
async fn a_sign_in_finishes_only_in_the_browser_that_started_it() {
    let (app, fake) = app_and_fake().await;
    app.create_user("ada@example.com").await;
    let start = app.get("/auth/fake").await;
    let back = fake.approve(
        start.header("location"),
        Person::verified("ada@example.com"),
    );
    let back = url::Url::parse(&back).unwrap();
    let path = format!("{}?{}", back.path(), back.query().unwrap());

    // Another browser, with no sign-in cookie, or another sign-in's.
    refused(&app.get(&path).await, "identity_provider_failed");
    let other = app.get("/auth/fake").await;
    let other_cookie = other.header("set-cookie").split(';').next().unwrap();
    let res = app
        .request("GET", &path, None, &[("cookie", other_cookie)])
        .await;
    refused(&res, "identity_provider_failed");
}

#[tokio::test]
async fn an_unknown_identity_provider_is_not_found() {
    let (app, _fake) = app_and_fake().await;
    assert_eq!(app.get("/auth/nope").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_identity_provider_that_cant_be_discovered_stops_the_app() {
    // Nothing listens on port 9 of this host.
    let provider = crablink_server::IdentityProvider {
        id: "keycloak".into(),
        name: "Keycloak".into(),
        issuer: "http://127.0.0.1:9/realms/crablink".into(),
        client_id: "crablink".into(),
        client_secret: "secret".into(),
    };
    let e = try_app_with(|s| crablink_server::Settings {
        identity_providers: vec![provider],
        ..s
    })
    .await
    .err()
    .expect("the app refuses to start");
    assert!(e.contains("Identity Provider keycloak"), "{e}");
    assert!(e.contains("http://127.0.0.1:9/realms/crablink"), "{e}");
}
