//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use subtle::ConstantTimeEq;

mod db;
mod error;
mod links;
mod redirect;
mod users;

pub use db::connect;
pub use error::ApiError;
pub use links::random_code;
pub use users::{create_user, set_user_disabled};

use links::{create_link, delete_link, get_link, link_qr, list_links, update_link};
use redirect::redirect;
use users::{change_password, me, sign_in, sign_out};

/// Produces a candidate Generated Code. Injected so tests can script collisions.
pub type CodeGenerator = Arc<dyn Fn() -> String + Send + Sync>;

/// The current time. Injected so tests can expire sessions.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    token: Arc<str>,
    base_url: Arc<str>,
}

/// Builds the full HTTP app.
pub fn app(
    pool: PgPool,
    codes: CodeGenerator,
    clock: Clock,
    token: &str,
    base_url: &str,
) -> Router {
    let state = AppState {
        pool,
        codes,
        clock,
        token: token.into(),
        base_url: base_url.trim_end_matches('/').into(),
    };
    let api = Router::new()
        .route("/links", post(create_link).get(list_links))
        .route(
            "/links/{code}",
            get(get_link).patch(update_link).delete(delete_link),
        )
        .route("/links/{code}/qr", get(link_qr))
        .route("/session", post(sign_in).delete(sign_out))
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .fallback(|| async { ApiError::NotFound });
    Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/{code}", get(redirect))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        // On the outer router, so unknown `/api` paths and methods need the token too.
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

/// Paths under `/api` that a session cookie guards instead of the token.
const SESSION_PATHS: [&str; 3] = ["/api/session", "/api/me", "/api/me/password"];

/// Every other `/api` request needs `Authorization: Bearer <token>`, compared in
/// constant time.
async fn require_token(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path != "/api" && !path.starts_with("/api/") || SESSION_PATHS.contains(&path) {
        return next.run(req).await;
    }
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !s.token.is_empty() && bool::from(given.as_bytes().ct_eq(s.token.as_bytes())) {
        next.run(req).await
    } else {
        ApiError::Unauthorized.into_response()
    }
}
