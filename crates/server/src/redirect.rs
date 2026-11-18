//! `GET /{code}`: the public redirect.

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use sqlx::PgPool;

use crate::AppState;
use crate::error::ApiError;

/// `GET /{code}`: 302 to the Target. Never 301/308, which browsers cache forever.
/// Checks liveness and counts the click in one statement; only when that misses
/// does a lookup tell a Link that never existed (404) from a dead one (410).
pub(crate) async fn redirect(State(s): State<AppState>, Path(code): Path<String>) -> Response {
    // Unlike the management API, redirect error bodies are plain text.
    follow(&s.pool, &code)
        .await
        .unwrap_or_else(|e| (e.status(), e.to_string()).into_response())
}

/// Redirects to the Target of a live (not deleted, not expired) Link, counting the click.
async fn follow(pool: &PgPool, code: &str) -> Result<Response, ApiError> {
    let target: Option<String> = sqlx::query_scalar(
        "UPDATE links SET click_count = click_count + 1 \
         WHERE code = $1 AND deleted_at IS NULL AND (expires_at IS NULL OR expires_at > $2) \
         RETURNING target",
    )
    .bind(code)
    .bind(Utc::now())
    .fetch_optional(pool)
    .await?;
    if let Some(target) = target {
        let headers = [
            (header::LOCATION, target),
            (header::CACHE_CONTROL, "private, max-age=0".into()),
        ];
        return Ok((StatusCode::FOUND, headers).into_response());
    }
    let exists = sqlx::query("SELECT 1 FROM links WHERE code = $1")
        .bind(code)
        .fetch_optional(pool)
        .await?
        .is_some();
    Ok(if exists {
        (StatusCode::GONE, "gone").into_response()
    } else {
        (StatusCode::NOT_FOUND, "not found").into_response()
    })
}
