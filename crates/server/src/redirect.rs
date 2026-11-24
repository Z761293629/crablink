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
    let live: Option<(i64, String, bool)> = sqlx::query_as(
        "SELECT id, target, blocked_at IS NOT NULL FROM links \
         WHERE domain_id = $1 AND code = $2 AND deleted_at IS NULL \
         AND (expires_at IS NULL OR expires_at > $3 OR blocked_at IS NOT NULL)",
    )
    .bind(domain_id)
    .bind(code)
    .bind((s.clock)())
    .fetch_optional(&s.pool)
    .await?;
    if let Some((_, _, true)) = live {
        return Ok(blocked_page(headers));
    }
    if let Some((id, target, _)) = live {
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

/// What a visitor of a Blocked Link sees instead of its Target, which it never names:
/// Simplified Chinese when the browser's first language is Chinese, like the web UI.
fn blocked_page(headers: &HeaderMap) -> Response {
    let (lang, title, text) = if crate::prefers_chinese(headers) {
        (
            "zh-CN",
            "链接已被封禁",
            "此短链接因滥用已被本站管理员封禁，不会再跳转到原来的地址。",
        )
    } else {
        (
            "en",
            "Link blocked",
            "This Short URL was blocked for abuse by the administrators of this service, so it no longer redirects.",
        )
    };
    let html = format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title></head><body style=\"font-family: system-ui, sans-serif; \
         max-width: 36rem; margin: 15vh auto; padding: 0 1rem\">\
         <h1>{title}</h1><p>{text}</p></body></html>"
    );
    let headers = [
        (header::CONTENT_TYPE, "text/html; charset=utf-8"),
        // Unblocking must take effect at once.
        (header::CACHE_CONTROL, "no-store"),
    ];
    (StatusCode::FORBIDDEN, headers, html).into_response()
}
