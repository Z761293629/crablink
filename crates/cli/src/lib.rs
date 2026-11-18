//! `crablink`: a thin client for the crablink management API. All validation lives in the server.

use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, TimeDelta, Utc};
use clap::{Args, CommandFactory, Parser, Subcommand};
use crablink_api::{CreateLink, Link, LinkPage, QrFormat, UpdateLink};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ureq::http::{Method, Request, Response};
use ureq::{Body, SendBody};

pub mod qr;
pub mod tui;

#[derive(Parser)]
#[command(
    name = "crablink",
    version,
    about = "Manage Links on a crablink server"
)]
pub struct Cli {
    #[command(flatten)]
    pub creds: Creds,
    /// None: open the TUI.
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Prompt for the token, check it against the server, and save it.
    Login,
    /// Create a Link and print its Short URL.
    Create {
        target: String,
        /// Custom Code (a Generated Code is used otherwise).
        #[arg(long)]
        code: Option<String>,
        /// RFC 3339 timestamp, or a duration from now like 30m, 12h, 7d.
        #[arg(long)]
        expires: Option<String>,
        /// Print the API's JSON for the new Link instead.
        #[arg(long)]
        json: bool,
    },
    /// List all Links, newest first: Code, Click Count, Expiry, Target.
    Ls {
        /// Print one JSON array of every Link instead.
        #[arg(long)]
        json: bool,
    },
    /// Show one Link in detail.
    Get {
        code: String,
        /// Print the API's JSON instead.
        #[arg(long)]
        json: bool,
    },
    /// Change a Link's Target and/or Expiry.
    Edit {
        code: String,
        #[arg(long)]
        target: Option<String>,
        /// RFC 3339 timestamp, or a duration from now like 30m, 12h, 7d.
        #[arg(long, conflicts_with = "no_expires")]
        expires: Option<String>,
        /// Remove the Expiry.
        #[arg(long)]
        no_expires: bool,
        /// Print the API's JSON for the updated Link instead.
        #[arg(long)]
        json: bool,
    },
    /// Delete a Link (its Code is never reused).
    Rm { code: String },
    /// Show a Link's QR code in the terminal, or save it to a file.
    Qr {
        code: String,
        /// Save to this file instead: .svg or .png.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// Server and token. Flags beat env vars (clap), which beat the config file (`or`).
/// Also the shape of `~/.config/crablink/config.toml`.
#[derive(Args, Serialize, Deserialize, Default)]
pub struct Creds {
    /// Server URL, e.g. https://crab.link
    #[arg(long, env = "CRABLINK_URL", global = true)]
    pub server: Option<String>,
    /// API token
    #[arg(long, env = "CRABLINK_TOKEN", global = true, hide_env_values = true)]
    pub token: Option<String>,
}

impl Creds {
    /// Fills each missing field from `fallback`.
    pub fn or(self, fallback: Creds) -> Creds {
        Creds {
            server: self.server.or(fallback.server),
            token: self.token.or(fallback.token),
        }
    }

    /// The client, or an error pointing at `crablink login` if a field is missing.
    pub fn api(self) -> anyhow::Result<Api> {
        let missing = |what| anyhow!("no {what} configured: run `crablink login --server <url>`");
        Ok(Api {
            server: self
                .server
                .ok_or_else(|| missing("server"))?
                .trim_end_matches('/')
                .into(),
            token: self.token.ok_or_else(|| missing("token"))?,
            agent: ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(REQUEST_TIMEOUT))
                .build()
                .into(),
        })
    }

    /// These credentials completed from the config file: the resolution every subcommand uses.
    pub fn resolve(self) -> anyhow::Result<Api> {
        self.or(load_config()?).api()
    }
}

fn config_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config/crablink/config.toml"))
}

fn load_config() -> anyhow::Result<Creds> {
    let path = config_path()?;
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("invalid {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Creds::default()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Writes the config readable only by the owner (`0600` on Unix), even if the file already existed.
fn save_config(creds: &Creds) -> anyhow::Result<PathBuf> {
    use std::io::Write;
    let path = config_path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(&path)?;
    #[cfg(unix)]
    f.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    f.write_all(toml::to_string(creds)?.as_bytes())?;
    Ok(path)
}

/// Longest a single API request may take, so a hung server can't freeze the TUI.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Authenticated HTTP client for `/api`. Every method fails with the server's
/// `{ "error" }` message (plus status) on a non-2xx response. Reuses connections.
pub struct Api {
    server: String,
    token: String,
    agent: ureq::Agent,
}

impl Api {
    /// The server URL, without a trailing slash.
    pub fn server(&self) -> &str {
        &self.server
    }

    /// `POST /api/links`.
    pub fn create(&self, body: &CreateLink) -> anyhow::Result<Link> {
        self.json(Method::POST, "/api/links", SendBody::from_json(body)?)
    }

    /// `GET /api/links`, following `next_cursor`; calls `each_page` with every page, newest first.
    pub fn list(&self, mut each_page: impl FnMut(Vec<Link>)) -> anyhow::Result<()> {
        let mut cursor: Option<String> = None;
        loop {
            let path = match &cursor {
                Some(c) => format!("/api/links?cursor={}", encode(c)),
                None => "/api/links".into(),
            };
            let page: LinkPage = self.json(Method::GET, &path, SendBody::none())?;
            each_page(page.links);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(());
            }
        }
    }

    /// `GET /api/links/{code}`.
    pub fn get(&self, code: &str) -> anyhow::Result<Link> {
        self.json(Method::GET, &link_path(code), SendBody::none())
    }

    /// `PATCH /api/links/{code}`.
    pub fn update(&self, code: &str, body: &UpdateLink) -> anyhow::Result<Link> {
        self.json(Method::PATCH, &link_path(code), SendBody::from_json(body)?)
    }

    /// `DELETE /api/links/{code}`.
    pub fn delete(&self, code: &str) -> anyhow::Result<()> {
        self.send(Method::DELETE, &link_path(code), SendBody::none())
            .map(drop)
    }

    /// `GET /api/links/{code}/qr`: the QR code file's bytes.
    pub fn qr(&self, code: &str, format: QrFormat) -> anyhow::Result<Vec<u8>> {
        let path = format!("{}/qr?format={}", link_path(code), format.as_str());
        let mut res = self.send(Method::GET, &path, SendBody::none())?;
        Ok(res.body_mut().read_to_vec()?)
    }

    fn send(&self, method: Method, path: &str, body: SendBody) -> anyhow::Result<Response<Body>> {
        let req = Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.server))
            .header("Authorization", format!("Bearer {}", self.token))
            // `agent.run` ignores `SendBody::from_json`'s content type; every API body is JSON.
            .header("Content-Type", "application/json")
            .body(body)?;
        let mut res = self
            .agent
            .run(req)
            .with_context(|| format!("cannot reach {}", self.server))?;
        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }
        match res.body_mut().read_json::<crablink_api::ErrorBody>() {
            Ok(e) => bail!("{} ({status})", e.error),
            Err(_) => bail!("server returned {status}"),
        }
    }

    /// `send`, then read the JSON response body.
    fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: SendBody,
    ) -> anyhow::Result<T> {
        Ok(self.send(method, path, body)?.body_mut().read_json()?)
    }
}

/// Runs one subcommand, or the TUI when there is none; the binary prints the error and exits 1.
pub fn run(cli: Cli) -> anyhow::Result<()> {
    let Some(cmd) = cli.cmd else {
        // Piped or redirected: a full-screen app would only garble the output.
        if !std::io::stdout().is_terminal() {
            return Ok(Cli::command().print_help()?);
        }
        return tui::run(cli.creds.resolve()?, tui::Caps::detect());
    };
    if let Cmd::Login = cmd {
        // A broken config must not stop `login` from replacing it.
        return login(cli.creds.or(load_config().unwrap_or_default()).server);
    }
    let api = cli.creds.resolve()?;
    let now = Utc::now();
    match cmd {
        Cmd::Login => unreachable!("handled above"),
        Cmd::Create {
            target,
            code,
            expires,
            json,
        } => {
            let expires_at = expires.map(|e| expires_flag(&e, now)).transpose()?;
            let link = api.create(&CreateLink {
                target,
                code,
                expires_at,
            })?;
            if json {
                print_json(&link)?;
            } else {
                println!("{}", link.short_url);
            }
        }
        Cmd::Ls { json: true } => {
            let mut all = Vec::new();
            api.list(|links| all.extend(links))?;
            print_json(&all)?;
        }
        Cmd::Ls { json: false } => api.list(|links| {
            for l in &links {
                println!(
                    "{:<20} {:>7}  {:<30}  {}",
                    l.code,
                    l.click_count,
                    expiry(l, now),
                    l.target
                );
            }
        })?,
        Cmd::Get { code, json } => show_link(&api.get(&code)?, json, now)?,
        Cmd::Edit {
            code,
            target,
            expires,
            no_expires,
            json,
        } => {
            let body = edit_body(target, expires, no_expires, now)?;
            show_link(&api.update(&code, &body)?, json, now)?;
        }
        Cmd::Rm { code } => {
            api.delete(&code)?;
            eprintln!("Deleted {code}");
        }
        Cmd::Qr {
            code,
            output: Some(path),
        } => {
            let format = qr_format(&path)?;
            std::fs::write(&path, api.qr(&code, format)?)
                .with_context(|| format!("cannot write {}", path.display()))?;
            eprintln!("Saved {}", path.display());
        }
        Cmd::Qr { code, output: None } => {
            if !std::io::stdout().is_terminal() {
                bail!("stdout is not a terminal: pass -o <file.svg|file.png> to save the QR code");
            }
            let link = api.get(&code)?;
            // As the server refuses `-o`: its Short URL no longer redirects.
            if link.is_expired(now) {
                bail!("Link {} has expired: no QR code", link.code);
            }
            for row in qr::half_blocks(&link.short_url) {
                // Forced dark on light, then reset, so it scans in any terminal theme.
                println!("\x1b[38;5;{};48;5;{}m{row}\x1b[0m", qr::DARK, qr::LIGHT);
            }
            println!("{}", link.short_url);
        }
    }
    Ok(())
}

/// The Code as one path segment, so e.g. `rm 'a?x'` cannot hit Link `a`.
/// Whether the Code is valid is the server's call.
fn link_path(code: &str) -> String {
    format!("/api/links/{}", encode(code))
}

/// Percent-encodes every byte except `A-Z a-z 0-9 _ -`.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The QR code format named by `path`'s extension.
fn qr_format(path: &Path) -> anyhow::Result<QrFormat> {
    let ext = path.extension().and_then(|e| e.to_str());
    match ext.map(str::to_ascii_lowercase).as_deref() {
        Some("svg") => Ok(QrFormat::Svg),
        Some("png") => Ok(QrFormat::Png),
        _ => bail!("{} must end in .svg or .png", path.display()),
    }
}

/// `edit` flags to a PATCH body; clap already rejects `--expires` with `--no-expires`.
fn edit_body(
    target: Option<String>,
    expires: Option<String>,
    no_expires: bool,
    now: DateTime<Utc>,
) -> anyhow::Result<UpdateLink> {
    let expires_at = match expires {
        Some(e) => Some(Some(expires_flag(&e, now)?)),
        None if no_expires => Some(None),
        None => None,
    };
    Ok(UpdateLink { target, expires_at })
}

pub(crate) fn timestamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The Expiry as every command shows it: `never`, a timestamp, or a timestamp marked `(expired)`.
pub(crate) fn expiry(l: &Link, now: DateTime<Utc>) -> String {
    match l.expires_at {
        None => "never".into(),
        Some(t) if l.is_expired(now) => format!("{} (expired)", timestamp(t)),
        Some(t) => timestamp(t),
    }
}

/// The Link as the API's JSON with `--json`, else as `print_link` shows it.
fn show_link(l: &Link, json: bool, now: DateTime<Utc>) -> anyhow::Result<()> {
    if json {
        return print_json(l);
    }
    print_link(l, now);
    Ok(())
}

/// Prints an API DTO as the API serializes it (pretty-printed; `jq` reads it either way).
fn print_json(dto: &impl Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(dto)?);
    Ok(())
}

fn print_link(l: &Link, now: DateTime<Utc>) {
    println!("Code:        {}", l.code);
    println!("Short URL:   {}", l.short_url);
    println!("Target:      {}", l.target);
    println!("Expiry:      {}", expiry(l, now));
    println!("Click Count: {}", l.click_count);
    println!("Created:     {}", timestamp(l.created_at));
    println!("Updated:     {}", timestamp(l.updated_at));
}

fn login(server: Option<String>) -> anyhow::Result<()> {
    let server = server.context("pass --server <url>")?;
    let token = read_token()?;
    let api = Creds {
        server: Some(server),
        token: Some(token),
    }
    .api()?;
    api.send(Method::GET, "/api/links?limit=1", SendBody::none())
        .context("login failed, nothing saved")?;
    let path = save_config(&Creds {
        server: Some(api.server),
        token: Some(api.token),
    })?;
    eprintln!("Logged in; saved {}", path.display());
    Ok(())
}

/// Prompts with echo disabled on a terminal; reads one line from stdin otherwise (scripts).
fn read_token() -> anyhow::Result<String> {
    let token = if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Token: ")?
    } else {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        line
    };
    let token = token.trim().to_string();
    if token.is_empty() {
        bail!("empty token");
    }
    Ok(token)
}

/// `--expires`, with an error naming the flag.
fn expires_flag(s: &str, now: DateTime<Utc>) -> anyhow::Result<DateTime<Utc>> {
    parse_expires(s, now).with_context(|| format!("invalid --expires {s:?}"))
}

/// An Expiry as typed: an RFC 3339 timestamp, or a duration from `now` like `30m`, `12h`, `7d`.
/// The error only says what's accepted; callers name what was invalid.
pub(crate) fn parse_expires(s: &str, now: DateTime<Utc>) -> anyhow::Result<DateTime<Utc>> {
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Ok(t.into());
    }
    let bad = || anyhow!("use an RFC 3339 timestamp or e.g. 30m, 12h, 7d");
    let unit = s.chars().last().ok_or_else(bad)?;
    let n: i64 = s[..s.len() - unit.len_utf8()].parse().map_err(|_| bad())?;
    let delta = match unit {
        'm' => TimeDelta::try_minutes(n),
        'h' => TimeDelta::try_hours(n),
        'd' => TimeDelta::try_days(n),
        _ => None,
    };
    delta
        .and_then(|d| now.checked_add_signed(d))
        .ok_or_else(bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn creds(server: Option<&str>, token: Option<&str>) -> Creds {
        Creds {
            server: server.map(Into::into),
            token: token.map(Into::into),
        }
    }

    #[test]
    fn flags_and_env_override_saved_config_per_field() {
        let saved = creds(Some("https://saved"), Some("saved-token"));
        let got = creds(None, Some("flag-token")).or(saved).api().unwrap();
        assert_eq!(got.server, "https://saved");
        assert_eq!(got.token, "flag-token");
    }

    #[test]
    fn missing_credentials_point_at_login() {
        let err = creds(Some("https://x"), None).api().err().unwrap();
        assert!(err.to_string().contains("crablink login"), "{err}");
    }

    #[test]
    fn expires_accepts_relative_duration() {
        let now = at("2026-11-05T10:00:00Z");
        assert_eq!(
            parse_expires("7d", now).unwrap(),
            at("2026-11-12T10:00:00Z")
        );
        assert_eq!(
            parse_expires("12h", now).unwrap(),
            at("2026-11-05T22:00:00Z")
        );
        assert_eq!(
            parse_expires("30m", now).unwrap(),
            at("2026-11-05T10:30:00Z")
        );
    }

    #[test]
    fn expires_accepts_rfc3339_timestamp() {
        let now = at("2026-11-05T10:00:00Z");
        assert_eq!(
            parse_expires("2027-01-01T09:00:00+01:00", now).unwrap(),
            at("2027-01-01T08:00:00Z")
        );
    }

    #[test]
    fn version_flag_reports_workspace_version() {
        let err = Cli::try_parse_from(["crablink", "--version"])
            .err()
            .unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        // The release check pins the workspace version to the tag, so this is the tag's version.
        assert_eq!(
            err.to_string(),
            format!("crablink {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn edit_rejects_expires_together_with_no_expires() {
        let err = Cli::try_parse_from(["crablink", "edit", "c", "--expires", "7d", "--no-expires"])
            .err()
            .unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn edit_maps_expiry_flags_to_three_state_field() {
        let now = at("2026-11-05T10:00:00Z");
        let body = |e: Option<&str>, none| edit_body(None, e.map(Into::into), none, now).unwrap();
        assert_eq!(body(None, false).expires_at, None);
        assert_eq!(body(None, true).expires_at, Some(None));
        assert_eq!(
            body(Some("7d"), false).expires_at,
            Some(Some(at("2026-11-12T10:00:00Z")))
        );
        assert!(edit_body(None, Some("soon".into()), false, now).is_err());
    }

    #[test]
    fn codes_are_sent_as_one_encoded_path_segment() {
        assert_eq!(link_path("Ab_-9"), "/api/links/Ab_-9");
        assert_eq!(link_path("a?x"), "/api/links/a%3Fx");
        assert_eq!(link_path("a/b#c"), "/api/links/a%2Fb%23c");
        assert_eq!(link_path(".."), "/api/links/%2E%2E");
        assert_eq!(link_path("é"), "/api/links/%C3%A9");
    }

    #[test]
    fn expires_rejects_garbage() {
        let now = at("2026-11-05T10:00:00Z");
        for bad in ["", "d", "7", "7y", "x7d", "tomorrow", "7é"] {
            assert!(parse_expires(bad, now).is_err(), "{bad:?} accepted");
        }
    }
}
