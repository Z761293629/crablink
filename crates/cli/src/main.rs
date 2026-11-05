//! `crablink`: a thin client for the crablink management API. All validation lives in the server.

use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, TimeDelta, Utc};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use ureq::http::Response;
use ureq::{Body, RequestBuilder, typestate::WithBody, typestate::WithoutBody};

#[derive(Parser)]
#[command(version, about = "Manage Links on a crablink server")]
struct Cli {
    #[command(flatten)]
    creds: Creds,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
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
    },
    /// List all Links, newest first: Code, Click Count, Expiry, Target.
    Ls,
    /// Show one Link in detail.
    Get { code: String },
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
    },
    /// Delete a Link (its Code is never reused).
    Rm { code: String },
}

/// Server and token. Flags beat env vars (clap), which beat the config file (`or`).
/// Also the shape of `~/.config/crablink/config.toml`.
#[derive(Args, Serialize, Deserialize, Default)]
struct Creds {
    /// Server URL, e.g. https://crab.link
    #[arg(long, env = "CRABLINK_URL", global = true)]
    server: Option<String>,
    /// API token
    #[arg(long, env = "CRABLINK_TOKEN", global = true, hide_env_values = true)]
    token: Option<String>,
}

impl Creds {
    fn or(self, fallback: Creds) -> Creds {
        Creds {
            server: self.server.or(fallback.server),
            token: self.token.or(fallback.token),
        }
    }

    fn api(self) -> anyhow::Result<Api> {
        let missing = |what| anyhow!("no {what} configured: run `crablink login --server <url>`");
        Ok(Api {
            server: self
                .server
                .ok_or_else(|| missing("server"))?
                .trim_end_matches('/')
                .into(),
            token: self.token.ok_or_else(|| missing("token"))?,
        })
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

/// Writes the config readable only by the owner (`0600`), even if the file already existed.
fn save_config(creds: &Creds) -> anyhow::Result<PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let path = config_path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    f.write_all(toml::to_string(creds)?.as_bytes())?;
    Ok(path)
}

/// Authenticated HTTP client for `/api`.
struct Api {
    server: String,
    token: String,
}

impl Api {
    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent()
    }

    fn auth<B>(&self, req: RequestBuilder<B>) -> RequestBuilder<B> {
        req.header("Authorization", format!("Bearer {}", self.token))
    }

    fn get(&self, path: &str) -> RequestBuilder<WithoutBody> {
        self.auth(Self::agent().get(format!("{}{path}", self.server)))
    }

    fn post(&self, path: &str) -> RequestBuilder<WithBody> {
        self.auth(Self::agent().post(format!("{}{path}", self.server)))
    }

    fn patch(&self, path: &str) -> RequestBuilder<WithBody> {
        self.auth(Self::agent().patch(format!("{}{path}", self.server)))
    }

    fn delete(&self, path: &str) -> RequestBuilder<WithoutBody> {
        self.auth(Self::agent().delete(format!("{}{path}", self.server)))
    }

    /// Fails with the server's `{ "error" }` message on a non-2xx response.
    fn check(&self, res: Result<Response<Body>, ureq::Error>) -> anyhow::Result<Response<Body>> {
        let mut res = res.with_context(|| format!("cannot reach {}", self.server))?;
        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }
        match res.body_mut().read_json::<crablink_api::Error>() {
            Ok(e) => bail!("{} ({status})", e.error),
            Err(_) => bail!("server returned {status}"),
        }
    }
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.cmd {
        // A broken config must not stop `login` from replacing it.
        Cmd::Login => login(cli.creds.or(load_config().unwrap_or_default()).server),
        Cmd::Create {
            target,
            code,
            expires,
        } => {
            let expires_at = expires.map(|e| parse_expires(&e, Utc::now())).transpose()?;
            let api = cli.creds.or(load_config()?).api()?;
            let body = crablink_api::CreateLink {
                target,
                code,
                expires_at,
            };
            let link: crablink_api::Link = api
                .check(api.post("/api/links").send_json(&body))?
                .body_mut()
                .read_json()?;
            println!("{}", link.short_url);
            Ok(())
        }
        Cmd::Ls => {
            let api = cli.creds.or(load_config()?).api()?;
            let now = Utc::now();
            let mut cursor = None;
            loop {
                let mut req = api.get("/api/links");
                if let Some(c) = &cursor {
                    req = req.query("cursor", c);
                }
                let page: crablink_api::LinkPage = api.check(req.call())?.body_mut().read_json()?;
                for l in &page.links {
                    let expiry = match l.expires_at {
                        None => "never".to_string(),
                        Some(t) if t <= now => format!("{} (expired)", t.format("%Y-%m-%d %H:%M")),
                        Some(t) => t.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    println!(
                        "{:<20} {:>7}  {:<26}  {}",
                        l.code, l.click_count, expiry, l.target
                    );
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    return Ok(());
                }
            }
        }
        Cmd::Get { code } => {
            let api = cli.creds.or(load_config()?).api()?;
            let link = api
                .check(api.get(&link_path(&code)?).call())?
                .body_mut()
                .read_json()?;
            print_link(&link);
            Ok(())
        }
        Cmd::Edit {
            code,
            target,
            expires,
            no_expires,
        } => {
            let body = update_link(target, expires, no_expires, Utc::now())?;
            let api = cli.creds.or(load_config()?).api()?;
            let link = api
                .check(api.patch(&link_path(&code)?).send_json(&body))?
                .body_mut()
                .read_json()?;
            print_link(&link);
            Ok(())
        }
        Cmd::Rm { code } => {
            let api = cli.creds.or(load_config()?).api()?;
            api.check(api.delete(&link_path(&code)?).call())?;
            eprintln!("Deleted {code}");
            Ok(())
        }
    }
}

/// Refuses characters no Code can contain, so e.g. `rm 'a?x'` cannot hit Link `a`.
fn link_path(code: &str) -> anyhow::Result<String> {
    let ok = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    if code.is_empty() || !code.bytes().all(ok) {
        bail!("invalid code {code:?}");
    }
    Ok(format!("/api/links/{code}"))
}

/// `edit` flags to a PATCH body; clap already rejects `--expires` with `--no-expires`.
fn update_link(
    target: Option<String>,
    expires: Option<String>,
    no_expires: bool,
    now: DateTime<Utc>,
) -> anyhow::Result<crablink_api::UpdateLink> {
    let expires_at = match expires {
        Some(e) => Some(Some(parse_expires(&e, now)?)),
        None if no_expires => Some(None),
        None => None,
    };
    Ok(crablink_api::UpdateLink { target, expires_at })
}

fn print_link(l: &crablink_api::Link) {
    let ts = |t: DateTime<Utc>| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    println!("Code:      {}", l.code);
    println!("Short URL: {}", l.short_url);
    println!("Target:    {}", l.target);
    println!("Expiry:    {}", l.expires_at.map_or("never".into(), ts));
    println!("Clicks:    {}", l.click_count);
    println!("Created:   {}", ts(l.created_at));
    println!("Updated:   {}", ts(l.updated_at));
}

fn login(server: Option<String>) -> anyhow::Result<()> {
    let server = server.context("pass --server <url>")?;
    let token = read_token()?;
    let api = Creds {
        server: Some(server),
        token: Some(token),
    }
    .api()?;
    api.check(api.get("/api/links?limit=1").call())
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

/// `--expires`: an RFC 3339 timestamp, or a duration from `now` like `30m`, `12h`, `7d`.
fn parse_expires(s: &str, now: DateTime<Utc>) -> anyhow::Result<DateTime<Utc>> {
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Ok(t.into());
    }
    let bad = || anyhow!("invalid --expires {s:?}: use an RFC 3339 timestamp or e.g. 30m, 12h, 7d");
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
    fn edit_rejects_expires_together_with_no_expires() {
        let err = Cli::try_parse_from(["crablink", "edit", "c", "--expires", "7d", "--no-expires"])
            .err()
            .unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn edit_maps_expiry_flags_to_three_state_field() {
        let now = at("2026-11-05T10:00:00Z");
        let body = |e: Option<&str>, none| update_link(None, e.map(Into::into), none, now).unwrap();
        assert_eq!(body(None, false).expires_at, None);
        assert_eq!(body(None, true).expires_at, Some(None));
        assert_eq!(
            body(Some("7d"), false).expires_at,
            Some(Some(at("2026-11-12T10:00:00Z")))
        );
        assert!(update_link(None, Some("soon".into()), false, now).is_err());
    }

    #[test]
    fn expires_rejects_garbage() {
        let now = at("2026-11-05T10:00:00Z");
        for bad in ["", "d", "7", "7y", "x7d", "tomorrow", "7é"] {
            assert!(parse_expires(bad, now).is_err(), "{bad:?} accepted");
        }
    }
}
