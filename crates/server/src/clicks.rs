//! Clicks (ADR-0006): one recorded per human redirect, and a Link's Clicks over a
//! time range with breakdowns.

use std::net::IpAddr;
use std::sync::{Arc, LazyLock};

use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, header};
use chrono::{DateTime, Utc};
use crablink_api::{
    ClickBreakdown, ClickBucket, ClickStats, ClicksQuery, Interval, TopLink, WorkspaceClicks,
    WorkspaceClicksQuery,
};

use crate::error::ApiError;
use crate::workspaces::Caller;
use crate::{AppState, CountryLookup};

/// The maintained bot and link-preview fetcher pattern list.
static BOTS: LazyLock<isbot::Bots> = LazyLock::new(isbot::Bots::default);

/// Browser names by a User-Agent token that marks them, checked in order: Edge,
/// Opera and others also claim `Chrome/` and `Safari/`.
const BROWSERS: &[(&str, &str)] = &[
    ("Edg", "Edge"),
    ("OPR/", "Opera"),
    ("SamsungBrowser/", "Samsung Internet"),
    ("MicroMessenger/", "WeChat"),
    ("Firefox/", "Firefox"),
    ("FxiOS/", "Firefox"),
    ("CriOS/", "Chrome"),
    ("Chrome/", "Chrome"),
    ("Safari/", "Safari"),
];

/// The country lookup over the DB-IP Lite country database (an `.mmdb` file) at `path`.
pub fn dbip_country_lookup(path: &str) -> Result<CountryLookup, maxminddb::MaxMindDbError> {
    let db = maxminddb::Reader::open_readfile(path)?;
    Ok(Arc::new(move |ip| {
        let iso_code: &str = db
            .lookup(ip)
            .ok()?
            .decode_path(&maxminddb::path!["country", "iso_code"])
            .ok()??;
        Some(iso_code.to_owned())
    }))
}

/// Records a Click on Link `link_id` unless the visitor is a bot or link-preview
/// fetcher. `socket` is the peer address. Never fails: an error is logged and the
/// redirect goes ahead.
pub(crate) async fn record(
    s: &AppState,
    link_id: i64,
    headers: &HeaderMap,
    socket: Option<IpAddr>,
) {
    let text = |name| headers.get(name).and_then(|v| v.to_str().ok());
    let ua = text(header::USER_AGENT).unwrap_or("");
    if BOTS.is_bot(ua) {
        return;
    }
    let forwarded = || {
        text(header::HeaderName::from_static("x-forwarded-for"))
            .and_then(|v| v.split(',').next())
            .and_then(|first| first.trim().parse().ok())
    };
    // A request that reached us without the proxy adding the header came from the socket.
    let ip = s.trust_proxy.then(forwarded).flatten().or(socket);
    // The IP goes no further than this lookup.
    let country = ip.and_then(|ip| (s.country)(ip));
    let referrer_host = text(header::REFERER)
        .and_then(|r| url::Url::parse(r).ok())
        .and_then(|u| u.host_str().map(str::to_owned));
    let browser = BROWSERS
        .iter()
        .find(|(token, _)| ua.contains(token))
        .map(|(_, name)| *name);
    let done = sqlx::query(
        "WITH c AS (INSERT INTO clicks (link_id, at, referrer_host, country, device, browser) \
         VALUES ($1, $2, $3, $4, $5, $6)) \
         UPDATE links SET click_count = click_count + 1 WHERE id = $1",
    )
    .bind(link_id)
    .bind((s.clock)())
    .bind(referrer_host)
    .bind(country)
    .bind(device(ua))
    .bind(browser)
    .execute(&s.pool)
    .await;
    if let Err(e) = done {
        eprintln!("cannot record a Click: {e}");
    }
}

/// `tablet`, `mobile` or `desktop`. An iPad asking for the desktop site, as Safari
/// does by default, counts as a desktop.
fn device(ua: &str) -> &'static str {
    if ua.contains("iPad")
        || ua.contains("Tablet")
        || (ua.contains("Android") && !ua.contains("Mobi"))
    {
        "tablet"
    } else if ua.contains("Mobi") || ua.contains("iPhone") {
        "mobile"
    } else {
        "desktop"
    }
}

/// The most buckets one request may ask for: about six weeks by hour.
const MAX_BUCKETS: i64 = 1000;

/// The `date_trunc` unit of buckets over `[from, to)`, refusing a range that ends
/// before it starts or holds too many buckets.
fn bucket_unit(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    interval: Option<Interval>,
) -> Result<&'static str, ApiError> {
    let (unit, secs) = match interval.unwrap_or(Interval::Day) {
        Interval::Hour => ("hour", 3600),
        Interval::Day => ("day", 86400),
    };
    if from >= to {
        return Err(ApiError::BadRequest("from must be before to".into()));
    }
    if (to - from).num_seconds() / secs >= MAX_BUCKETS {
        return Err(ApiError::BadRequest(format!(
            "at most {MAX_BUCKETS} buckets: shorten the range or use a longer interval"
        )));
    }
    Ok(unit)
}

/// Clicks per `$1` bucket in `[$2, $3)`, empty buckets included, of the Clicks whose
/// `link_id` matches `$filter` (which may use `$4`).
macro_rules! series_sql {
    ($filter:literal) => {
        concat!(
            "WITH counts AS (SELECT date_trunc($1, at, 'UTC') AS t, count(*) AS n FROM clicks \
             WHERE link_id ",
            $filter,
            " AND at >= $2 AND at < $3 GROUP BY 1) \
             SELECT g.t, COALESCE(counts.n, 0) FROM generate_series(\
             date_trunc($1, $2, 'UTC'), $3 - interval '1 microsecond', ('1 ' || $1)::interval) g(t) \
             LEFT JOIN counts USING (t) ORDER BY g.t"
        )
    };
}

/// Runs a `series_sql!` with `$4` bound to `id`.
async fn series(
    s: &AppState,
    sql: &'static str,
    unit: &str,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    id: i64,
) -> Result<Vec<ClickBucket>, ApiError> {
    let rows: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(sql)
        .bind(unit)
        .bind(from)
        .bind(to)
        .bind(id)
        .fetch_all(&s.pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(time, count)| ClickBucket { time, count })
        .collect())
}

/// A breakdown of one Link's Clicks in `[$2, $3)` by column `$col`, most first.
macro_rules! breakdown_sql {
    ($col:literal) => {
        concat!(
            "SELECT ",
            $col,
            ", count(*) FROM clicks WHERE link_id = $1 AND at >= $2 AND at < $3 \
             GROUP BY 1 ORDER BY 2 DESC, 1"
        )
    };
}

/// `GET /api/workspaces/{slug}/links/{code}/clicks?domain=`: the Link's Clicks over a time
/// range, per hour or day (UTC), and by Referrer, country, device and browser.
pub(crate) async fn link_clicks(
    State(s): State<AppState>,
    caller: Caller,
    Path((slug, code)): Path<(String, String)>,
    query: Result<Query<ClicksQuery>, QueryRejection>,
) -> Result<Json<ClickStats>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Query(q) = query?;
    let unit = bucket_unit(q.from, q.to, q.interval)?;
    let link_id: i64 = sqlx::query_scalar(
        "SELECT l.id FROM links l JOIN domains d ON d.id = l.domain_id \
         WHERE l.workspace_id = $1 AND d.host = lower($2) AND l.code = $3 \
         AND l.deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(q.domain.as_deref().unwrap_or(&s.default_domain))
    .bind(&code)
    .fetch_optional(&s.pool)
    .await?
    .ok_or(ApiError::LinkNotFound)?;
    let by = async |sql: &'static str| -> Result<Vec<ClickBreakdown>, ApiError> {
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(sql)
            .bind(link_id)
            .bind(q.from)
            .bind(q.to)
            .fetch_all(&s.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(value, count)| ClickBreakdown { value, count })
            .collect())
    };
    Ok(Json(ClickStats {
        series: series(&s, series_sql!("= $4"), unit, (q.from, q.to), link_id).await?,
        referrers: by(breakdown_sql!("referrer_host")).await?,
        countries: by(breakdown_sql!("country")).await?,
        devices: by(breakdown_sql!("device")).await?,
        browsers: by(breakdown_sql!("browser")).await?,
    }))
}

/// How many Links `workspace_clicks` ranks.
const TOP_LINKS: i64 = 10;

/// `GET /api/workspaces/{slug}/clicks`: the Clicks of all the Workspace's Links over a
/// time range, per hour or day (UTC), Deleted Links included so past totals never
/// shrink, and the live Links with the most of them.
pub(crate) async fn workspace_clicks(
    State(s): State<AppState>,
    caller: Caller,
    Path(slug): Path<String>,
    query: Result<Query<WorkspaceClicksQuery>, QueryRejection>,
) -> Result<Json<WorkspaceClicks>, ApiError> {
    let workspace_id = caller.workspace(&s, &slug).await?.workspace_id;
    let Query(q) = query?;
    let unit = bucket_unit(q.from, q.to, q.interval)?;
    // A removed Domain's Links keep their Workspace; a deleted Workspace's are no one's.
    let sql = series_sql!("IN (SELECT id FROM links WHERE workspace_id = $4)");
    let series = series(&s, sql, unit, (q.from, q.to), workspace_id).await?;
    let top: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT l.code, d.host, count(*) FROM clicks c \
         JOIN links l ON l.id = c.link_id JOIN domains d ON d.id = l.domain_id \
         WHERE l.workspace_id = $1 AND l.deleted_at IS NULL AND c.at >= $2 AND c.at < $3 \
         GROUP BY l.id, l.code, d.host ORDER BY 3 DESC, 2, 1 LIMIT $4",
    )
    .bind(workspace_id)
    .bind(q.from)
    .bind(q.to)
    .bind(TOP_LINKS)
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(WorkspaceClicks {
        series,
        top_links: top
            .into_iter()
            .map(|(code, domain, count)| TopLink {
                short_url: format!("{}://{domain}/{code}", s.scheme),
                code,
                domain,
                count,
            })
            .collect(),
    }))
}
