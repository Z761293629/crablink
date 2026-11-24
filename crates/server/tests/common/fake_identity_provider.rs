//! A fake Identity Provider: a real OpenID Connect issuer on a local port
//! (discovery, JWKS, authorization and token endpoints) that signs ID tokens with
//! a fixed key and the claims each test chooses. The tests drive it in process
//! through `approve`; `examples/e2e.rs` serves it to a browser, whose
//! authorization request gets a form asking for an email.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreResponseType, CoreRsaPrivateSigningKey, CoreSubjectIdentifierType,
};
use openidconnect::{
    Audience, AuthUrl, EmptyAdditionalClaims, EmptyAdditionalProviderMetadata, EndUserEmail,
    IssuerUrl, JsonWebKeyId, JsonWebKeySetUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    PrivateSigningKey, ResponseTypes, StandardClaims, SubjectIdentifier, TokenUrl,
};

/// The client id and secret the instance is set up with at the fake.
pub const CLIENT_ID: &str = "crablink";
pub const CLIENT_SECRET: &str = "fake client secret";

/// Who approves a sign-in at the fake, and what ID token the fake then issues.
#[derive(Clone, Debug)]
pub struct Person {
    pub subject: String,
    pub email: String,
    pub email_verified: Option<bool>,
    pub token: Token,
}

impl Person {
    /// Someone whose `email` the fake has verified, with a subject derived from it.
    pub fn verified(email: &str) -> Person {
        Person {
            subject: format!("subject-{}", email.to_lowercase()),
            email: email.into(),
            email_verified: Some(true),
            token: Token::Valid,
        }
    }
}

/// How the fake signs the ID token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Valid,
    /// Signed with a key the fake's JWKS doesn't hold.
    WrongKey,
    /// Validly signed for another email, then given this person's claims.
    Tampered,
}

/// An authorization the fake granted, waiting for its code to be exchanged.
struct Grant {
    person: Person,
    nonce: String,
    client_id: String,
    pkce_challenge: String,
}

#[derive(Clone)]
pub struct FakeIdentityProvider {
    /// Its issuer URL, `http://127.0.0.1:<port>`.
    pub issuer: String,
    grants: Arc<Mutex<HashMap<String, Grant>>>,
}

impl FakeIdentityProvider {
    /// Starts the fake on a free local port, serving until the process ends.
    pub async fn start() -> FakeIdentityProvider {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fake = FakeIdentityProvider {
            issuer: format!("http://{}", listener.local_addr().unwrap()),
            grants: Arc::default(),
        };
        let router = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .with_state(fake.clone());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        fake
    }

    /// The instance's configuration of the fake as Identity Provider `id`, shown as `name`.
    pub fn identity_provider(&self, id: &str, name: &str) -> crablink_server::IdentityProvider {
        crablink_server::IdentityProvider {
            id: id.into(),
            name: name.into(),
            issuer: self.issuer.clone(),
            client_id: CLIENT_ID.into(),
            client_secret: CLIENT_SECRET.into(),
        }
    }

    /// `person` approves the authorization request at `authorize_url`: returns the
    /// redirect URI the fake sends the browser back to, with a code and the state.
    pub fn approve(&self, authorize_url: &str, person: Person) -> String {
        let url = url::Url::parse(authorize_url).unwrap();
        self.grant(&url.query_pairs().into_owned().collect(), person)
    }

    fn grant(&self, request: &HashMap<String, String>, person: Person) -> String {
        let param = |name: &str| request.get(name).cloned().unwrap_or_default();
        assert_eq!(param("code_challenge_method"), "S256", "{request:?}");
        let code = format!("code-{}", rand::random::<u64>());
        let grant = Grant {
            person,
            nonce: param("nonce"),
            client_id: param("client_id"),
            pkce_challenge: param("code_challenge"),
        };
        self.grants.lock().unwrap().insert(code.clone(), grant);
        let mut back = url::Url::parse(&param("redirect_uri")).unwrap();
        back.query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &param("state"));
        back.into()
    }

    /// An ID token for `grant`, signed as its `Token` says.
    fn id_token(&self, grant: &Grant) -> String {
        let p = &grant.person;
        let sign = |email: &str, key: &str| {
            let now = chrono::Utc::now();
            let standard = StandardClaims::new(SubjectIdentifier::new(p.subject.clone()))
                .set_email(Some(EndUserEmail::new(email.into())))
                .set_email_verified(p.email_verified);
            let claims = CoreIdTokenClaims::new(
                IssuerUrl::new(self.issuer.clone()).unwrap(),
                vec![Audience::new(grant.client_id.clone())],
                now + chrono::TimeDelta::minutes(5),
                now,
                standard,
                EmptyAdditionalClaims {},
            )
            .set_nonce(Some(Nonce::new(grant.nonce.clone())));
            let alg = CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256;
            let token = CoreIdToken::new(claims, &signing_key(key), alg, None, None).unwrap();
            token.to_string()
        };
        match p.token {
            Token::Valid => sign(&p.email, KEY),
            Token::WrongKey => sign(&p.email, WRONG_KEY),
            Token::Tampered => {
                let claimed = sign(&p.email, KEY);
                let signed = sign("someone-else@example.com", KEY);
                let [header, payload, _] = parts(&claimed);
                let [_, _, signature] = parts(&signed);
                format!("{header}.{payload}.{signature}")
            }
        }
    }
}

/// A JWT's header, payload and signature.
fn parts(jwt: &str) -> [&str; 3] {
    let parts: Vec<&str> = jwt.split('.').collect();
    parts.try_into().unwrap()
}

fn signing_key(pem: &str) -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(pem, Some(JsonWebKeyId::new("fake".into()))).unwrap()
}

async fn discovery(State(fake): State<FakeIdentityProvider>) -> Json<CoreProviderMetadata> {
    let url = |path: &str| format!("{}{path}", fake.issuer);
    let metadata = CoreProviderMetadata::new(
        IssuerUrl::new(fake.issuer.clone()).unwrap(),
        AuthUrl::new(url("/authorize")).unwrap(),
        JsonWebKeySetUrl::new(url("/jwks")).unwrap(),
        vec![ResponseTypes::new(vec![CoreResponseType::Code])],
        vec![CoreSubjectIdentifierType::Public],
        vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
        EmptyAdditionalProviderMetadata {},
    )
    .set_token_endpoint(Some(TokenUrl::new(url("/token")).unwrap()));
    Json(metadata)
}

async fn jwks() -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        signing_key(KEY).as_verification_key(),
    ]))
}

/// A browser's authorization request: a form asking for the email to sign in as,
/// then, with `email`, approval as that verified person.
async fn authorize(
    State(fake): State<FakeIdentityProvider>,
    Query(request): Query<HashMap<String, String>>,
) -> Response {
    if let Some(email) = request.get("email") {
        return Redirect::to(&fake.grant(&request, Person::verified(email))).into_response();
    }
    let hidden: String = request
        .iter()
        .map(|(name, value)| {
            format!(
                r#"<input type="hidden" name="{}" value="{}">"#,
                escape(name),
                escape(value)
            )
        })
        .collect();
    Html(format!(
        r#"<!doctype html><title>Fake Identity Provider</title>
<form method="get" action="/authorize">{hidden}
<label>Email <input name="email" type="email" required></label>
<button>Continue</button></form>"#
    ))
    .into_response()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// The token endpoint: exchanges a code for an ID token, checking the PKCE verifier.
async fn token(
    State(fake): State<FakeIdentityProvider>,
    Form(request): Form<HashMap<String, String>>,
) -> Response {
    let code = request.get("code").cloned().unwrap_or_default();
    let verifier = request.get("code_verifier").cloned().unwrap_or_default();
    let Some(grant) = fake.grants.lock().unwrap().remove(&code) else {
        return invalid_grant();
    };
    let challenge = PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(verifier));
    if challenge.as_str() != grant.pkce_challenge {
        return invalid_grant();
    }
    Json(serde_json::json!({
        "access_token": "fake access token",
        "token_type": "bearer",
        "expires_in": 300,
        "id_token": fake.id_token(&grant),
    }))
    .into_response()
}

fn invalid_grant() -> Response {
    let body = serde_json::json!({ "error": "invalid_grant" });
    (StatusCode::BAD_REQUEST, Json(body)).into_response()
}

/// The fixed key the fake signs ID tokens with. A test fixture, used nowhere else.
const KEY: &str = "-----BEGIN RSA PRIVATE KEY-----
MIIEogIBAAKCAQEAvHad3n3KYAtWLk1E835Zaj7pOHNS59uR6Q7i0ajIwGXN416j
oMcz8nzu5ZlGQNcc0jqkveRVfy9AJ15ls7Ix7V0Ohv/hsC8+PsoLmXVaHHt2LfsH
PwU/Rias59Qho18XRv994Yow3MHXr7tw/YiZvu95tr1OBHGCVBkSjAxPchCSpLzd
o0XHR2UV9lrj1T4zIHZvor8PQqAFANtdnSZ9rcMMa35j6y2OAlo5g7BQOewIMCRg
by+2T9MGzm1ntzX5McR8I1bha8g5H2FuA9GCLvD5Z6In8lsfbzZS3R8SxJUq5Q4z
XgfMZXrGxuJl5h9ny3Nsc3szl3TgVJVQALPvzQIDAQABAoIBADelVFap0JPMNR2F
vhqhD1iiC62Zy3xfJfVo6kraaa/5PzHnsyuZNbre7ZDIUWh6u21NWHAjF6Vud6RI
KIJG1pwNsmjk66fgQyI5+PPOJRWs1IeeC5UtgPG/IKVwyp/Yx2A0tt/dtDCyYXNn
RmTtrOul/MuamIsSUtdu8b19MwXr4emFRYlraBpRDOrwr0LLzhtSUyvB2g81CBHe
z/GAPFA3MvRMGW7YZrrCUvv9IkVwpuMfbjWqRY869OsNFGl7r40y8F2j4GfBybZg
N1/+5FL8yewmNBjled5yvKT9293xs+k/k25GFRlRmDEM+MPuwZyO7nbJlihDeyOJ
lQ1YwvcCgYEA5txHtV6StgIK9bSTf8rolMnMOGnrcwZFCaUf4bsrXeprwiRk3DyH
g5/wtQvzGc7VvruANdIKtGEC/pjNTLp4rk3hHwmCRbjr/ckyzw2ILWZDJk2XoJcx
N9OoUC0VzqltFAdkvlxd/5c915dB0oG5r33gbw7y6P7clKqAiJESk5MCgYEA0Pxt
heYHa1eHDPz0++qzlwH0235ExbrPRESkP5/N0KaG1B4Iw9g+qBe+UPDXczoyrhVM
N9Q4QqOTU3COjNjmKI3gNSIVJElHVSpTqs5vbaTbuCrBTlrbwqiEs8UVoYlOO3Wt
XTH1N5wYXGGqikPWxD0VNyTN+PwFN2cn+VvoSx8CgYB3tylUayuHSL3oNnD6pv0/
EvpdEsgvCMRKQR+sTgQqGoMw1PCcqKLZZdpL+FtjObfrxY2x2y8D5P7JifGEZXpp
YSdpkePFRiTSoUBA0NKaQFp4BWqos16phY/Vq9uQJJnU9hSPBpUUEpjWMLDIec8R
qyP0U3sxyeAXE+e9agWJEwKBgE1tiMJ4ND1/40hbnASNs1X6YpbD1IURrmMTr2rn
MSDh4chLDGJLBwPOMDPa12gYotg+icuRc/q0kgvzAs2KOtPBpNTquLyF7cjO8sqw
yzXrQ4CbTp1SKmOXffsAxP/hWDvNYJrLvJeK3Le1c7LgaXyxSN/yYeGitegN7xCD
DbBvAoGAcq7QhwHPaPQszf3BSIA2Rt3DAfiLWVJgkPFZ25hnRkBVbId3yhlJZEuN
AayrHv0CsW7znuI2fICsa7wQ5jQ1/CvqdUonJOZD5BCs8uh70f5KuyBf08l+27vP
pkHNwfWIG1uzVx7celRGNKnhBboZKOrFoER4coWZlK+GzuFPebI=
-----END RSA PRIVATE KEY-----";

/// A key the fake's JWKS doesn't hold.
const WRONG_KEY: &str = "-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEAztISM05OdAM3qL5NltwotZc1RhQlf021xK1sOfyJJtarLblY
EAPbFaReZrON22Cph9c/wWCImLMRQH1tBOrBNIjWAXCHZG3uMciossfuXaMcrSmH
fG39pqkB1zIlIuDJApp8WXv16nZs5RC2XN7IXqskPboL8TTqu/sgqUfyhCN1NE4h
sHZybRSw1bxKZpCcp0u30oJpgT8854QU74SdmG7hBIpEe7MoEnvVrecYwJryUrBs
JB0juAylqoAoYm/T7/+5EkAKDStEU1ny4JgXQaKX7Z0di4rOjErY/2p3meo0VZfA
4nmEw1QHi1trz3VKUv+yGaZng0Q6eTznmyHqywIDAQABAoIBACXNmgwMtSRZbjQG
zNHVXg+LGu67QftZ7e6MMGsM8NjLhjBuX11WBxaWm7az/SrrKTGuM3YaOf2HRZD/
HA05B6WdmVhcBDNkMJSi88rx5mzxIo24OI6S4Dk1TSPTg5wzcCpsqGwGz3VSvibE
fkDNa9x59b7eoc4t+PXxOhAXGYnzSMIOB8W/LIRsWCUA4qFRBNU0d4QAxzOu1fDH
34C0nVaXNOQpB1xPpdj9atsvjEac60AGChFgCmO/dC3M4EJ/nFvxBre9HJQbK48i
CLMhPBbwf6r9t0ShpiA/x0ZWlamc4gnLRBtiJNGwf+6WSFTxZjNSdv/MP5HIL5rX
Kpz8gYECgYEA/Bp6lbO63FT7adDp74PcJsJKc9gBVd/mS6ablFG1h81dLkri9nwe
W2vZ4om32EI70zSc4Ayaxjtj4WeXxO0rkY8OyIs+ke0zN0z6TNjf2Dulqd4Dd3n8
CbOI/2Qx2UkzuKQHfvaJAmC+M4oFhnTw0gYrZdH265r2221Xs7/gIQsCgYEA0gRq
2vT0eDSA3t8v7IXV0SoGqC0EsdPz2HNIc3CQiDUvPlqONE2w638iQ0+6wA1WbTUW
IZI9MXJGwxSeGkFcIilDHgfagOzKmjx4vcp4++/bUni28UPH2ve0Pz43/6edreEF
emP+sfW8wJlG37/n7R5VbzcultrzZGjyyN8O9UECgYEArkLLW3F6B9VCWBmNqv81
jSEd/8de3PJAqShIQJTeazm8YERpcEHJZh495QaXKwhVfB/RFFoCHmZtGyDVWs0q
IZqGeH8vyATzMxWlqrr6t+g7EhkhwlNysW4Fk6u5fV3JP6c6ezuE5zziHJQaT5/D
2xlipCYnjSJ34qNJTTmjfpMCgYA5YlGcIEBIM/hebVdX5j3FGBmXjC1zNXm0U9Oh
D0m+neGZxP6EFhJYVnWTNM7PORW7Yzz30VB9GT9ygEiu3dsBzopzQywV/B1jxVAs
c41ZMXGY+DjsdVu/Xd1XRnXgmfkAkLKBfZ8TGxUG/xqZeoMxjA9GAyrst0i3z+E+
t4wLwQKBgDafxBUUc7qxNzrQGbu2CwMbSCL0Ro1RkcHAq3smdEzBE2DyyPCN832M
+KdwWhO8aPAJx0PKmROzmS4K4J2O5xVo8ix4YE5pA+rZ8LiwYV6vUr4tFW4eOO4o
hvynIRwBnhPchOMpXDnlrdhuxD1BnIKUB9xAiO+XDMSOT4SUj3vr
-----END RSA PRIVATE KEY-----";
