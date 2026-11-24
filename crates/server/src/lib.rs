//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::net::IpAddr;
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
mod links;
mod members;
mod redirect;
mod users;
mod web;
mod workspaces;

pub use clicks::{ClickRecording, dbip_country_lookup};
pub use db::connect;
pub use error::ApiError;
pub use links::random_code;
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

/// Everything `app` takes besides the database: the instance switches and the
/// injected services. Start from `Settings::new` and override fields with struct
/// update syntax, so a new field only touches the callers that set it.
#[derive(Clone)]
pub struct Settings {
    /// `CRABLINK_BASE_URL`: the scheme of Short URLs and the Default Domain.
    pub base_url: String,
    /// `CRABLINK_TRUST_PROXY`: whether the visitor's IP is the first `X-Forwarded-For` entry.
    pub trust_proxy: bool,
    pub codes: CodeGenerator,
    pub clock: Clock,
    pub country: CountryLookup,
}

impl Settings {
    /// An instance on `base_url` with every switch off, random Generated Codes,
    /// the real clock and no country lookup.
    pub fn new(base_url: impl Into<String>) -> Self {
        Settings {
            base_url: base_url.into(),
            trust_proxy: false,
            codes: Arc::new(random_code),
            clock: Arc::new(Utc::now),
            country: Arc::new(|_| None),
        }
    }
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    country: CountryLookup,
    /// Whether the visitor's IP is the first `X-Forwarded-For` entry rather than the socket's.
    trust_proxy: bool,
    /// The scheme of Short URLs: `CRABLINK_BASE_URL`'s.
    scheme: Arc<str>,
    /// The Default Domain: its host, and port if any.
    default_domain: Arc<str>,
    /// The Default Domain's host alone: a Target there would loop.
    default_host: Arc<str>,
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
/// `settings.base_url` (its host, and port if any) if missing. Serve it with
/// `ConnectInfo<SocketAddr>` so Clicks can find the visitor's country.
pub async fn app(
    pool: PgPool,
    settings: Settings,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let Settings {
        base_url,
        trust_proxy,
        codes,
        clock,
        country,
    } = settings;
    let url = url::Url::parse(&base_url)?;
    let Some(default_host) = url.host_str() else {
        return Err(format!("{base_url} has no host").into());
    };
    let default_domain = match url.port() {
        Some(port) => format!("{default_host}:{port}"),
        None => default_host.to_owned(),
    };
    db::default_domain(&pool, &default_domain).await?;
    std::sync::LazyLock::force(&users::DUMMY);
    let state = AppState {
        pool,
        codes,
        clock,
        country,
        trust_proxy,
        scheme: url.scheme().into(),
        default_domain: default_domain.into(),
        default_host: default_host.into(),
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
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route(
            "/workspaces/{slug}",
            patch(rename_workspace).delete(delete_workspace),
        )
        .route("/session", post(sign_in).delete(sign_out))
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .fallback(api_fallback);
    Ok(Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/internal/domain-allowed", get(domain_allowed))
        .nest_service("/app", get(web::serve))
        .route("/{code}", get(redirect))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        .with_state(state))
}
