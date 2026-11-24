//! Rate limits: at most `count` requests per `window` for each key (a visitor IP,
//! a Workspace), counted in fixed windows in process memory. Never written to the
//! database, so no IP is stored (ADR-0006).

use std::collections::HashMap;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, header};
use chrono::{DateTime, TimeDelta, Utc};

use crate::AppState;
use crate::error::ApiError;

/// At most `count` per `window`. Parsed from `<count>/<window>`, the window a number
/// with `s`, `m`, `h` or `d`, e.g. `10/1m` or `1000/1d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub count: u32,
    pub window: TimeDelta,
}

impl FromStr for Limit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("{s:?} is not <count>/<window> such as 10/1m");
        let (count, window) = s.trim().split_once('/').ok_or_else(bad)?;
        let count: u32 = count.parse().map_err(|_| bad())?;
        let unit = window.len().checked_sub(1).ok_or_else(bad)?;
        let (n, unit) = window.split_at(unit);
        let n: i64 = n.parse().map_err(|_| bad())?;
        let window = match unit {
            "s" => TimeDelta::try_seconds(n),
            "m" => TimeDelta::try_minutes(n),
            "h" => TimeDelta::try_hours(n),
            "d" => TimeDelta::try_days(n),
            _ => None,
        }
        .filter(|w| *w > TimeDelta::zero())
        .ok_or_else(bad)?;
        if count == 0 {
            return Err(bad());
        }
        Ok(Limit { count, window })
    }
}

/// The instance's rate limits; `None` (the default) limits nothing.
#[derive(Debug, Clone, Default)]
pub struct RateLimits {
    /// `CRABLINK_RATE_LIMIT_SIGN_IN`: sign-in attempts per visitor IP.
    pub sign_in: Option<Limit>,
    /// `CRABLINK_RATE_LIMIT_PASSWORD_RESET`: password reset requests per visitor IP.
    pub password_reset: Option<Limit>,
    /// `CRABLINK_RATE_LIMIT_LINKS`: Links created per Workspace, by any means.
    pub links: Option<Limit>,
}

/// One limiter per `RateLimits` field. A new limit adds a field to both, set from
/// its env var in `main`, and calls `hit` in its handler (with `VisitorIp` for one
/// per visitor IP).
pub(crate) struct Limiters {
    /// Requests with no known IP (served without `ConnectInfo`) share one count.
    pub sign_in: Limiter<Option<IpAddr>>,
    pub password_reset: Limiter<Option<IpAddr>>,
    pub links: Limiter<i64>,
}

impl Limiters {
    pub fn new(limits: RateLimits) -> Self {
        Limiters {
            sign_in: Limiter::new(limits.sign_in),
            password_reset: Limiter::new(limits.password_reset),
            links: Limiter::new(limits.links),
        }
    }
}

/// Counts requests per key in fixed windows that start at a key's first request.
// ponytail: counters live in this process, so a second server instance would allow
// twice the limit; that needs a shared store (Redis or Postgres), an ADR then.
pub(crate) struct Limiter<K> {
    limit: Option<Limit>,
    /// Each key's window start and requests counted in it.
    windows: Mutex<HashMap<K, (DateTime<Utc>, u32)>>,
}

/// Above this many keys, a request first forgets the windows that have passed.
const PRUNE_ABOVE: usize = 10_000;

impl<K: Hash + Eq> Limiter<K> {
    pub fn new(limit: Option<Limit>) -> Self {
        Limiter {
            limit,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Counts one request for `key` at `now`, or refuses it with how long to wait.
    pub fn hit(&self, key: K, now: DateTime<Utc>) -> Result<(), ApiError> {
        let Some(Limit { count, window }) = self.limit else {
            return Ok(());
        };
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        // ponytail: an O(keys) sweep while more than PRUNE_ABOVE keys are live; a
        // timer-driven sweep if that ever shows up in a profile.
        if windows.len() > PRUNE_ABOVE {
            windows.retain(|_, (start, _)| now < *start + window);
        }
        let (start, n) = windows.entry(key).or_insert((now, 0));
        if now >= *start + window {
            (*start, *n) = (now, 0);
        }
        if *n >= count {
            let wait = *start + window - now;
            // Round up, so a retry after `Retry-After` seconds is never too early.
            let seconds = (wait.num_milliseconds() + 999) / 1000;
            return Err(ApiError::RateLimited(seconds as u64));
        }
        *n += 1;
        Ok(())
    }
}

/// The visitor's IP: the first `X-Forwarded-For` entry when the instance trusts its
/// proxy and the proxy added one, else the socket's; `None` when served without
/// `ConnectInfo` (only tests are).
pub(crate) fn visitor_ip(
    s: &AppState,
    headers: &HeaderMap,
    socket: Option<IpAddr>,
) -> Option<IpAddr> {
    let forwarded = || {
        headers
            .get(header::HeaderName::from_static("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|first| first.trim().parse().ok())
    };
    s.trust_proxy.then(forwarded).flatten().or(socket)
}

/// The visitor's IP as an extractor; see `visitor_ip`.
pub(crate) struct VisitorIp(pub Option<IpAddr>);

impl FromRequestParts<AppState> for VisitorIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, s: &AppState) -> Result<Self, Self::Rejection> {
        let socket = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip());
        Ok(VisitorIp(visitor_ip(s, &parts.headers, socket)))
    }
}
