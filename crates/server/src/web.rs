//! `/app`: the web UI (ADR-0008), the SPA built into `web/dist`.
//!
//! A release build embeds `web/dist`; a debug build reads it from disk on each
//! request. Without it the server still builds, and `/app` says so.

use axum::http::{Uri, header};
use axum::response::{Html, IntoResponse, Response};

#[derive(rust_embed::Embed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Dist;

const NOT_BUILT: &str = "<!doctype html><title>crablink</title>\
<p>The crablink web UI wasn't built: run <code>pnpm install && pnpm build</code> in <code>web/</code>, then rebuild the server.</p>";

/// `GET /app/{path}` (nested, so `uri` is `/{path}`): the file at `path`, or
/// else the SPA's index page, which routes `path` itself.
pub(crate) async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = Dist::get(path) {
        let mime = file.metadata.mimetype().to_owned();
        return ([(header::CONTENT_TYPE, mime)], file.data).into_response();
    }
    match Dist::get("index.html") {
        Some(index) => Html(index.data).into_response(),
        None => Html(NOT_BUILT).into_response(),
    }
}
