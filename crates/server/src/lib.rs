//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::collections::HashSet;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::routing::{delete, get, patch, post};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

mod admin;
mod api_keys;
mod clicks;
mod db;
mod domains;
mod error;
mod identity_providers;
mod links;
mod members;
mod rate_limit;
mod redirect;
mod reports;
mod safe_browsing;
mod users;
mod web;
mod workspaces;

pub use clicks::{ClickRecording, dbip_country_lookup};
pub use db::connect;
pub use error::ApiError;
pub use identity_providers::IdentityProvider;
pub use links::random_code;
pub use rate_limit::{Limit, RateLimits};
pub use safe_browsing::google_safe_browsing;
pub use users::{create_user, set_user_disabled};

use admin::{
    accept_invite, block_link, create_invite, list_blocked_links, list_users, reset_password,
    unblock_link, update_user,
};
use api_keys::{create_api_key, list_api_keys, revoke_api_key};
use domains::{
    add_domain, domain_allowed, list_domains, list_managed_domains, remove_domain,
    update_managed_domain,
};
use links::{
    create_link, delete_link, export_links, get_link, import_links, link_qr, list_links, list_tags,
    update_link,
};
use members::{add_member, list_workspace_users, remove_workspace_user, update_workspace_user};
use redirect::redirect;
use reports::{create_report, dismiss_report, list_reports};
use users::{change_password, me, sign_in, sign_out};
use workspaces::{
    api_fallback, create_workspace, delete_workspace, list_workspaces, rename_workspace,
};

/// Produces a candidate Generated Code. Injected so tests can script collisions.
pub type CodeGenerator = Arc<dyn Fn() -> String + Send + Sync>;

/// The current time. Injected so tests can expire sessions.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// The ISO 3166-1 alpha-2 country of an IP address, if known. Injected so tests
/// can place visitors; production reads the bundled DB-IP Lite database.
pub type CountryLookup = Arc<dyn Fn(IpAddr) -> Option<String> + Send + Sync>;

/// Whether a Target is known to be malicious: `Ok(true)` refuses it, and an `Err`
/// (Safe Browsing down, a timeout) lets it through. Injected so tests can fake it;
/// production asks Google Safe Browsing when `CRABLINK_SAFE_BROWSING_KEY` is set.
pub type TargetCheck =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<bool, String>> + Send>> + Send + Sync>;

/// Everything `app` takes besides the database: the instance switches and the
/// injected services. Start from `Settings::new` and override fields with struct
/// update syntax, so a new field only touches the callers that set it.
#[derive(Clone)]
pub struct Settings {
    /// `CRABLINK_BASE_URL`: the scheme of Short URLs and the Default Domain.
    pub base_url: String,
    /// `CRABLINK_TRUST_PROXY`: whether the visitor's IP is the first `X-Forwarded-For` entry.
    pub trust_proxy: bool,
    /// `CRABLINK_RATE_LIMIT_*`: all off by default.
    pub rate_limits: RateLimits,
    /// `CRABLINK_RESERVED_CODES`: the Reserved Codes, compared case-sensitively.
    pub reserved_codes: HashSet<String>,
    /// The Identity Providers Users can sign in through; discovered when `app` starts.
    pub identity_providers: Vec<IdentityProvider>,
    pub codes: CodeGenerator,
    pub clock: Clock,
    pub country: CountryLookup,
    pub target_check: TargetCheck,
}

impl Settings {
    /// An instance on `base_url` with every switch off, random Generated Codes,
    /// the real clock, no country lookup and no Target check.
    pub fn new(base_url: impl Into<String>) -> Self {
        Settings {
            base_url: base_url.into(),
            trust_proxy: false,
            rate_limits: RateLimits::default(),
            reserved_codes: HashSet::new(),
            identity_providers: Vec::new(),
            codes: Arc::new(random_code),
            clock: Arc::new(Utc::now),
            country: Arc::new(|_| None),
            target_check: Arc::new(|_| Box::pin(async { Ok(false) })),
        }
    }
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    country: CountryLookup,
    target_check: TargetCheck,
    /// Whether the visitor's IP is the first `X-Forwarded-For` entry rather than the socket's.
    trust_proxy: bool,
    limiters: Arc<rate_limit::Limiters>,
    /// Codes nobody can take as a Custom Code on the Default Domain.
    reserved_codes: Arc<HashSet<String>>,
    /// The scheme of Short URLs: `CRABLINK_BASE_URL`'s.
    scheme: Arc<str>,
    /// The Default Domain: its host, and port if any.
    default_domain: Arc<str>,
    /// The Default Domain's host alone: a Target there would loop.
    default_host: Arc<str>,
    identity_providers: Arc<[identity_providers::Discovered]>,
    /// For calls to Identity Providers.
    http: reqwest::Client,
}

impl AppState {
    /// The Domain a request names (e.g. `?domain=`), the Default Domain when it names none.
    fn domain_or_default<'a>(&'a self, requested: Option<&'a str>) -> &'a str {
        requested.unwrap_or(&self.default_domain)
    }
}

/// The host a request was sent to, with its port if any: the `Host` header, or the
/// URI's authority (HTTP/2).
fn request_host<'a>(headers: &'a HeaderMap, uri: &'a Uri) -> Option<&'a str> {
    headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .or_else(|| uri.authority().map(|a| a.as_str()))
}

/// Builds the full HTTP app, first creating the Default Domain from
/// `settings.base_url` (its host, and port if any) if missing, and running
/// discovery for each Identity Provider. The error names what stopped it. Serve
/// it with `ConnectInfo<SocketAddr>` so Clicks can find the visitor's country.
pub async fn app(pool: PgPool, settings: Settings) -> Result<Router, String> {
    let Settings {
        base_url,
        trust_proxy,
        rate_limits,
        reserved_codes,
        identity_providers,
        codes,
        clock,
        country,
        target_check,
    } = settings;
    let domain_problem = |e: &dyn std::fmt::Display| {
        format!("cannot set up the Default Domain from {base_url}: {e}")
    };
    let url = url::Url::parse(&base_url).map_err(|e| domain_problem(&e))?;
    let Some(default_host) = url.host_str() else {
        return Err(domain_problem(&"it has no host"));
    };
    let default_domain = match url.port() {
        Some(port) => format!("{default_host}:{port}"),
        None => default_host.to_owned(),
    };
    db::default_domain(&pool, &default_domain)
        .await
        .map_err(|e| domain_problem(&e))?;
    std::sync::LazyLock::force(&users::DUMMY);
    // reqwest's TLS needs a crypto provider; an error means one is installed already.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("cannot set up an HTTP client: {e}"))?;
    let origin = format!("{}://{default_domain}", url.scheme());
    let mut discovered = Vec::new();
    for provider in identity_providers {
        discovered.push(identity_providers::discover(&http, &origin, provider).await?);
    }
    let state = AppState {
        pool,
        codes,
        clock,
        country,
        target_check,
        trust_proxy,
        limiters: Arc::new(rate_limit::Limiters::new(rate_limits)),
        reserved_codes: Arc::new(reserved_codes),
        scheme: url.scheme().into(),
        default_domain: default_domain.into(),
        default_host: default_host.into(),
        identity_providers: discovered.into(),
        http,
    };
    let workspace = Router::new()
        .route("/links", post(create_link).get(list_links))
        .route("/links.csv", get(export_links).post(import_links))
        .route(
            "/links/{code}",
            get(get_link).patch(update_link).delete(delete_link),
        )
        .route("/links/{code}/qr", get(link_qr))
        .route("/links/{code}/clicks", get(clicks::link_clicks))
        .route("/clicks", get(clicks::workspace_clicks))
        .route("/tags", get(list_tags))
        .route("/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/domains", post(add_domain).get(list_domains))
        .route("/domains/{domain}", delete(remove_domain))
        .route("/members", post(add_member).get(list_workspace_users))
        .route(
            "/members/{email}",
            patch(update_workspace_user).delete(remove_workspace_user),
        );
    let admin = Router::new()
        .route("/invites", post(create_invite))
        .route("/domains", get(list_managed_domains))
        .route("/domains/{domain}", patch(update_managed_domain))
        .route("/users", get(list_users))
        .route("/users/{id}", patch(update_user))
        .route("/users/{id}/password", post(reset_password))
        .route("/reports", get(list_reports))
        .route("/reports/{id}", delete(dismiss_report))
        .route(
            "/blocked-links",
            get(list_blocked_links)
                .post(block_link)
                .delete(unblock_link),
        );
    let api = Router::new()
        .nest("/workspaces/{slug}", workspace)
        .nest("/admin", admin)
        .route("/invites/accept", post(accept_invite))
        .route("/reports", post(create_report))
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route(
            "/workspaces/{slug}",
            patch(rename_workspace).delete(delete_workspace),
        )
        .route("/session", post(sign_in).delete(sign_out))
        .route("/identity-providers", get(identity_providers::list))
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .fallback(api_fallback);
    Ok(Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/internal/domain-allowed", get(domain_allowed))
        .route("/auth/{id}", get(identity_providers::start))
        .route("/auth/{id}/callback", get(identity_providers::callback))
        .nest_service("/app", get(web::serve))
        // The public report form lives in the web UI; keeps `?short_url=`.
        .route(
            "/report",
            get(|uri: Uri| async move {
                let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
                axum::response::Redirect::to(&format!("/app/report{query}"))
            }),
        )
        .route("/{code}", get(redirect))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        .with_state(state))
}
