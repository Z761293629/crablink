//! Signing in through an Identity Provider: OpenID Connect's authorization code
//! flow with PKCE. `/auth/{id}` sends the browser to the Identity Provider, which
//! sends it back to `/auth/{id}/callback`; that verifies the ID token, matches it
//! to a User and starts their Session.

use std::error::Error;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{AppendHeaders, IntoResponse, Redirect, Response};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreJsonWebKeySet, CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;

use crate::AppState;
use crate::error::ApiError;
use crate::users;

/// An Identity Provider the instance trusts, as its configuration sets it up.
#[derive(Clone)]
pub struct IdentityProvider {
    /// Names it in its sign-in URLs, `/auth/{id}`.
    pub id: String,
    /// What the sign-in page shows: "Sign in with {name}".
    pub name: String,
    /// Its OpenID Connect issuer URL, where discovery starts.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
}

/// An Identity Provider once discovery has found its endpoints.
pub(crate) struct Discovered {
    id: String,
    name: String,
    metadata: CoreProviderMetadata,
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect: RedirectUrl,
}

type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// Binds a sign-in to the browser that started it: the Identity Provider's id, the
/// state, the nonce and the PKCE verifier.
const COOKIE: &str = "crablink_sign_in";
/// How long a browser has to come back from the Identity Provider.
const SIGN_IN_SECONDS: i64 = 600;

/// Runs discovery for `provider`, whose redirect URI is under `origin`
/// (`{scheme}://{Default Domain}`); the error names the problem.
pub(crate) async fn discover(
    http: &reqwest::Client,
    origin: &str,
    provider: IdentityProvider,
) -> Result<Discovered, String> {
    let IdentityProvider {
        id,
        name,
        issuer,
        client_id,
        client_secret,
    } = provider;
    let problem = |e: String| format!("cannot discover Identity Provider {id} at {issuer}: {e}");
    let issuer_url = IssuerUrl::new(issuer.clone()).map_err(|e| problem(chain(&e)))?;
    let metadata = CoreProviderMetadata::discover_async(issuer_url, &|r| send(http.clone(), r))
        .await
        .map_err(|e| problem(chain(&e)))?;
    if metadata.token_endpoint().is_none() {
        return Err(problem("it has no token endpoint".into()));
    }
    let redirect =
        RedirectUrl::new(format!("{origin}/auth/{id}/callback")).map_err(|e| problem(chain(&e)))?;
    Ok(Discovered {
        id,
        name,
        metadata,
        client_id: ClientId::new(client_id),
        client_secret: ClientSecret::new(client_secret),
        redirect,
    })
}

/// `e` and its sources, for a log or an error message.
fn chain(e: &dyn Error) -> String {
    let mut message = e.to_string();
    let mut source = e.source();
    while let Some(e) = source {
        message += &format!(": {e}");
        source = e.source();
    }
    message
}

/// Sends an OpenID Connect request with the app's HTTP client.
/// Owns its client, so its future borrows nothing: axum handlers must be `Send` for
/// any lifetime.
async fn send(http: reqwest::Client, request: HttpRequest) -> Result<HttpResponse, reqwest::Error> {
    let response = http.execute(request.try_into()?).await?;
    let (status, headers) = (response.status(), response.headers().clone());
    let mut out = HttpResponse::new(response.bytes().await?.to_vec());
    *out.status_mut() = status;
    *out.headers_mut() = headers;
    Ok(out)
}

/// `GET /api/identity-providers`: the Identity Providers the sign-in page offers.
pub(crate) async fn list(State(s): State<AppState>) -> Json<Vec<crablink_api::IdentityProvider>> {
    Json(
        s.identity_providers
            .iter()
            .map(|p| crablink_api::IdentityProvider {
                id: p.id.clone(),
                name: p.name.clone(),
            })
            .collect(),
    )
}

/// `GET /auth/{id}`: sends the browser to Identity Provider `id` to sign in,
/// binding the sign-in to it with a short-lived cookie.
pub(crate) async fn start(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(provider) = s.identity_providers.iter().find(|p| p.id == id) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, state, nonce) = provider
        .client(provider.metadata.clone())
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".into()))
        .set_pkce_challenge(challenge)
        .url();
    // All four are base64url or `[a-z0-9-]`, so never contain a `.`.
    let pending = [&id, state.secret(), nonce.secret(), verifier.secret()].map(String::as_str);
    let cookie = sign_in_cookie(&pending.join("."), SIGN_IN_SECONDS);
    (
        AppendHeaders([(header::SET_COOKIE, cookie)]),
        Redirect::to(url.as_str()),
    )
        .into_response()
}

fn sign_in_cookie(value: &str, max_age: i64) -> String {
    format!("{COOKIE}={value}; HttpOnly; Secure; SameSite=Lax; Path=/auth/; Max-Age={max_age}")
}

/// The query the Identity Provider sends the browser back with.
#[derive(Deserialize)]
pub(crate) struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /auth/{id}/callback`: signs in the User the Identity Provider vouches for
/// and sends the browser into the web UI, or back to its sign-in page with
/// `?error={kind}`.
pub(crate) async fn callback(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(callback): Query<Callback>,
    headers: HeaderMap,
) -> Response {
    let mut cookies = vec![sign_in_cookie("", 0)];
    let to = match sign_in(&s, &id, callback, &headers).await {
        Ok(session) => {
            cookies.push(session);
            "/app/".to_owned()
        }
        Err(e) => {
            let kind = serde_json::to_value(e.kind()).unwrap_or_default();
            format!("/app/?error={}", kind.as_str().unwrap_or_default())
        }
    };
    let cookies = cookies.into_iter().map(|c| (header::SET_COOKIE, c));
    (AppendHeaders(cookies), Redirect::to(&to)).into_response()
}

/// Verifies the callback, then signs in the User it names: the `Set-Cookie` of
/// their new Session.
async fn sign_in(
    s: &AppState,
    id: &str,
    callback: Callback,
    headers: &HeaderMap,
) -> Result<String, ApiError> {
    let failed = |why: &str| {
        eprintln!("signing in through Identity Provider {id} failed: {why}");
        ApiError::IdentityProviderFailed
    };
    let provider = s
        .identity_providers
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| failed("no such Identity Provider"))?;
    if let Some(error) = callback.error {
        return Err(failed(&format!("the Identity Provider answered {error}")));
    }
    let pending = users::cookie(headers, COOKIE)
        .ok_or_else(|| failed("no sign-in cookie: it expired, or another browser started"))?;
    let [started_at, state, nonce, verifier] = pending
        .split('.')
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| failed("malformed sign-in cookie"))?;
    if started_at != id || callback.state.as_deref() != Some(state) {
        return Err(failed("the state isn't the one this browser started with"));
    }
    let code = callback.code.ok_or_else(|| failed("no code"))?;
    let identity = provider
        .verify(s, code, verifier, nonce)
        .await
        .map_err(|e| failed(&e))?;
    start_session(s, provider, identity).await
}

/// Who a verified ID token names.
struct Identity {
    subject: String,
    email: Option<String>,
    email_verified: bool,
}

impl Discovered {
    fn client(&self, metadata: CoreProviderMetadata) -> Client {
        CoreClient::from_provider_metadata(
            metadata,
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect.clone())
    }

    /// Exchanges `code` for an ID token and verifies its signature, issuer,
    /// audience, expiry and nonce.
    async fn verify(
        &self,
        s: &AppState,
        code: String,
        pkce_verifier: &str,
        nonce: &str,
    ) -> Result<Identity, String> {
        let http = |r| send(s.http.clone(), r);
        // ponytail: fetches the JWKS on every sign-in, so rotated keys just work;
        // cache it if sign-ins ever get frequent.
        let jwks = CoreJsonWebKeySet::fetch_async(self.metadata.jwks_uri(), &http)
            .await
            .map_err(|e| chain(&e))?;
        let client = self.client(self.metadata.clone().set_jwks(jwks));
        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|e| chain(&e))?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.into()))
            .request_async(&http)
            .await
            .map_err(|e| chain(&e))?;
        let id_token = token
            .id_token()
            .ok_or("the token response has no ID token")?;
        let verifier = client.id_token_verifier().set_time_fn(|| (s.clock)());
        let claims = id_token
            .claims(&verifier, &Nonce::new(nonce.into()))
            .map_err(|e| chain(&e))?;
        Ok(Identity {
            subject: claims.subject().to_string(),
            email: claims.email().map(|e| e.to_string()),
            email_verified: claims.email_verified() == Some(true),
        })
    }
}

/// Starts a Session for the User `identity` at `provider` belongs to. An identity
/// no User has yet is added to the User with its email, ignoring case, if the
/// Identity Provider verified that email.
async fn start_session(
    s: &AppState,
    provider: &Discovered,
    identity: Identity,
) -> Result<String, ApiError> {
    let issuer = provider.metadata.issuer().to_string();
    let mut tx = s.pool.begin().await?;
    let known: Option<(i64, String, bool)> = sqlx::query_as(
        "SELECT u.id, u.email, u.disabled FROM identities i JOIN users u ON u.id = i.user_id \
         WHERE i.issuer = $1 AND i.subject = $2",
    )
    .bind(&issuer)
    .bind(&identity.subject)
    .fetch_optional(&mut *tx)
    .await?;
    let (user_id, email, disabled) = match known {
        Some(user) => user,
        None => {
            let email = identity
                .email
                .filter(|_| identity.email_verified)
                .ok_or(ApiError::IdentityProviderEmailUnverified)?;
            let user: (i64, String, bool) = sqlx::query_as(
                "SELECT id, email, disabled FROM users WHERE lower(email) = lower($1)",
            )
            .bind(email.trim())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ApiError::InviteNeeded)?;
            if !user.2 {
                sqlx::query(
                    "INSERT INTO identities (issuer, subject, user_id) VALUES ($1, $2, $3) \
                     ON CONFLICT DO NOTHING",
                )
                .bind(&issuer)
                .bind(&identity.subject)
                .bind(user.0)
                .execute(&mut *tx)
                .await?;
            }
            user
        }
    };
    if disabled {
        return Err(ApiError::UserDisabled);
    }
    let cookie = users::new_session(s, &mut tx, user_id, &email).await?;
    tx.commit().await?;
    Ok(cookie)
}
