//! `GET /{code}`: the public redirect, on the Domain the `Host` header names.

use std::net::{IpAddr, SocketAddr};

use axum::Extension;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::clicks;
use crate::error::ApiError;

/// `GET /{code}`: 302 to the Target. Never 301/308, which browsers cache forever.
/// Only when no live Link matches does a lookup tell a Link that never existed (404)
/// from a dead one (410). A host that isn't a served Domain is 404.
pub(crate) async fn redirect(
    State(s): State<AppState>,
    Path(code): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    socket: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> Response {
    let socket = socket.map(|Extension(ConnectInfo(addr))| addr.ip());
    let domain = crate::request_host(&headers, &uri).unwrap_or_default();
    // Unlike the management API, redirect error bodies are plain text.
    follow(&s, domain, &code, &headers, socket)
        .await
        .unwrap_or_else(|e| (e.status(), e.to_string()).into_response())
}

/// Redirects to the Target of a live (not deleted, not expired) Link, recording a Click.
async fn follow(
    s: &AppState,
    domain: &str,
    code: &str,
    headers: &HeaderMap,
    socket: Option<IpAddr>,
) -> Result<Response, ApiError> {
    let Some(domain_id) = crate::domains::served(&s.pool, domain).await? else {
        return Ok((StatusCode::NOT_FOUND, "not found").into_response());
    };
    let live: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, target FROM links \
         WHERE domain_id = $1 AND code = $2 AND deleted_at IS NULL \
         AND (expires_at IS NULL OR expires_at > $3)",
    )
    .bind(domain_id)
    .bind(code)
    .bind((s.clock)())
    .fetch_optional(&s.pool)
    .await?;
    if let Some((id, target)) = live {
        let click = clicks::record(s, id, headers, socket);
        let headers = [
            (header::LOCATION, target),
            (header::CACHE_CONTROL, "private, max-age=0".into()),
        ];
        let mut res = (StatusCode::FOUND, headers).into_response();
        if let Some(click) = click {
            res.extensions_mut().insert(click);
        }
        return Ok(res);
    }
    let exists = sqlx::query("SELECT 1 FROM links WHERE domain_id = $1 AND code = $2")
        .bind(domain_id)
        .bind(code)
        .fetch_optional(&s.pool)
        .await?
        .is_some();
    Ok(if exists {
        (StatusCode::GONE, "gone").into_response()
    } else {
        (StatusCode::NOT_FOUND, "not found").into_response()
    })
}
