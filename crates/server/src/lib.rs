//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{delete, get, patch, post};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

mod admin;
mod api_keys;
mod db;
mod error;
mod links;
mod redirect;
mod users;
mod workspaces;

pub use db::connect;
pub use error::ApiError;
pub use links::random_code;
pub use users::{create_user, set_user_disabled};

use admin::{accept_invite, create_invite, list_users, reset_password, update_user};
use api_keys::{create_api_key, list_api_keys, revoke_api_key};
use links::{create_link, delete_link, get_link, link_qr, list_links, update_link};
use redirect::redirect;
use users::{change_password, me, sign_in, sign_out};
use workspaces::{api_fallback, list_workspaces};

/// Produces a candidate Generated Code. Injected so tests can script collisions.
pub type CodeGenerator = Arc<dyn Fn() -> String + Send + Sync>;

/// The current time. Injected so tests can expire sessions.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    base_url: Arc<str>,
    /// The scheme of Short URLs: `base_url`'s.
    scheme: Arc<str>,
    /// The Default Domain's id: every Link lives on it for now.
    domain_id: i64,
}

/// Builds the full HTTP app, first creating the Default Domain from `base_url`
/// (its host, and port if any) if missing.
pub async fn app(
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    base_url: &str,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let url = url::Url::parse(base_url)?;
    let host = match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => return Err(format!("{base_url} has no host").into()),
    };
    let domain_id = db::default_domain(&pool, &host).await?;
    let state = AppState {
        pool,
        codes,
        clock,
        base_url: base_url.trim_end_matches('/').into(),
        scheme: url.scheme().into(),
        domain_id,
    };
    let workspace = Router::new()
        .route("/links", post(create_link).get(list_links))
        .route(
            "/links/{code}",
            get(get_link).patch(update_link).delete(delete_link),
        )
        .route("/links/{code}/qr", get(link_qr))
        .route("/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api-keys/{id}", delete(revoke_api_key));
    let admin = Router::new()
        .route("/invites", post(create_invite))
        .route("/users", get(list_users))
        .route("/users/{id}", patch(update_user))
        .route("/users/{id}/password", post(reset_password));
    let api = Router::new()
        .nest("/workspaces/{slug}", workspace)
        .nest("/admin", admin)
        .route("/invites/accept", post(accept_invite))
        .route("/workspaces", get(list_workspaces))
        .route("/session", post(sign_in).delete(sign_out))
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .fallback(api_fallback);
    Ok(Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/{code}", get(redirect))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        .with_state(state))
}
