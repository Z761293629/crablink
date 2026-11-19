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
mod members;
mod redirect;
mod users;
mod web;
mod workspaces;

pub use db::connect;
pub use error::ApiError;
pub use links::random_code;
pub use users::{create_user, set_user_disabled};

use admin::{accept_invite, create_invite, list_users, reset_password, update_user};
use api_keys::{create_api_key, list_api_keys, revoke_api_key};
use links::{create_link, delete_link, get_link, link_qr, list_links, update_link};
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

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    /// The scheme of Short URLs: `CRABLINK_BASE_URL`'s.
    scheme: Arc<str>,
    /// The Default Domain: its host, and port if any.
    default_domain: Arc<str>,
    /// The Default Domain's host alone: a Target there would loop.
    default_host: Arc<str>,
    /// The Default Domain's id: every Link lives on it for now.
    domain_id: i64,
}

/// Builds the full HTTP app, first creating the Default Domain from `base_url`
/// (`CRABLINK_BASE_URL`: its host, and port if any) if missing.
pub async fn app(
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    base_url: &str,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let url = url::Url::parse(base_url)?;
    let Some(default_host) = url.host_str() else {
        return Err(format!("{base_url} has no host").into());
    };
    let default_domain = match url.port() {
        Some(port) => format!("{default_host}:{port}"),
        None => default_host.to_owned(),
    };
    let domain_id = db::default_domain(&pool, &default_domain).await?;
    std::sync::LazyLock::force(&users::DUMMY);
    let state = AppState {
        pool,
        codes,
        clock,
        scheme: url.scheme().into(),
        default_domain: default_domain.into(),
        default_host: default_host.into(),
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
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/members", post(add_member).get(list_workspace_users))
        .route(
            "/members/{email}",
            patch(update_workspace_user).delete(remove_workspace_user),
        );
    let admin = Router::new()
        .route("/invites", post(create_invite))
        .route("/users", get(list_users))
        .route("/users/{id}", patch(update_user))
        .route("/users/{id}/password", post(reset_password));
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
        .nest_service("/app", get(web::serve))
        .route("/{code}", get(redirect))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        .with_state(state))
}
