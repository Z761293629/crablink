//! crablink-server as a library: `app` builds the full HTTP app, and is the
//! seam both `main` and the tests use.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use sqlx::SqlitePool;
use subtle::ConstantTimeEq;

mod db;
mod error;
mod links;
mod redirect;

pub use db::{checkpoint, connect};
pub use links::random_code;

use error::ApiError;
use links::{create_link, delete_link, get_link, link_qr, list_links, update_link};
use redirect::redirect;

/// Produces a candidate Generated Code. Injected so tests can script collisions.
pub type CodeGenerator = Arc<dyn Fn() -> String + Send + Sync>;

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    codes: CodeGenerator,
    token: Arc<str>,
    base_url: Arc<str>,
}

/// Builds the full HTTP app.
pub fn app(pool: SqlitePool, codes: CodeGenerator, token: &str, base_url: &str) -> Router {
    let state = AppState {
        pool,
        codes,
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

/// Every `/api` request needs `Authorization: Bearer <token>`, compared in constant time.
async fn require_token(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path != "/api" && !path.starts_with("/api/") {
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
