//! `crablink`: a thin client for the crablink management API. All validation lives in the server.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, TimeDelta, Utc};
use clap::{CommandFactory, Parser, Subcommand};
use crablink_api::{CreateLink, Link, QrFormat, UpdateLink};
use serde::Serialize;

mod api;
mod config;
pub mod qr;
pub mod tui;

pub use api::Api;
pub use config::Creds;

use config::{load_config, login};

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
    /// Prompt for an API Key, learn its Workspace from the server, and save both.
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
        /// The Domain the Link lives on, e.g. go.example.com (the Default Domain otherwise).
        #[arg(long)]
        domain: Option<String>,
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
        /// The Link's Domain, e.g. go.example.com (the Default Domain otherwise).
        #[arg(long)]
        domain: Option<String>,
        /// Print the API's JSON instead.
        #[arg(long)]
        json: bool,
    },
    /// Change a Link's Target and/or Expiry.
    Edit {
        code: String,
        /// The Link's Domain, e.g. go.example.com (the Default Domain otherwise).
        #[arg(long)]
        domain: Option<String>,
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
    Rm {
        code: String,
        /// The Link's Domain, e.g. go.example.com (the Default Domain otherwise).
        #[arg(long)]
        domain: Option<String>,
    },
    /// Show a Link's QR code in the terminal, or save it to a file.
    Qr {
        code: String,
        /// The Link's Domain, e.g. go.example.com (the Default Domain otherwise).
        #[arg(long)]
        domain: Option<String>,
        /// Save to this file instead: .svg or .png.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
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
            domain,
            json,
        } => {
            let expires_at = expires.map(|e| expires_flag(&e, now)).transpose()?;
            let link = api.create(&CreateLink {
                target,
                code,
                expires_at,
                domain,
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
        Cmd::Get { code, domain, json } => {
            show_link(&api.get(&code, domain.as_deref())?, json, now)?
        }
        Cmd::Edit {
            code,
            domain,
            target,
            expires,
            no_expires,
            json,
        } => {
            let body = edit_body(target, expires, no_expires, now)?;
            show_link(&api.update(&code, domain.as_deref(), &body)?, json, now)?;
        }
        Cmd::Rm { code, domain } => {
            api.delete(&code, domain.as_deref())?;
            eprintln!("Deleted {code}");
        }
        Cmd::Qr {
            code,
            domain,
            output: Some(path),
        } => {
            let format = qr_format(&path)?;
            std::fs::write(&path, api.qr(&code, domain.as_deref(), format)?)
                .with_context(|| format!("cannot write {}", path.display()))?;
            eprintln!("Saved {}", path.display());
        }
        Cmd::Qr {
            code,
            domain,
            output: None,
        } => {
            if !std::io::stdout().is_terminal() {
                bail!("stdout is not a terminal: pass -o <file.svg|file.png> to save the QR code");
            }
            let link = api.get(&code, domain.as_deref())?;
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
/// A closed stdout is an `io::Error`, not a panic as with `println!`.
fn print_json(dto: &impl Serialize) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    // Back to the io::Error inside, so `main` can spot a broken pipe.
    serde_json::to_writer_pretty(&mut out, dto).map_err(std::io::Error::from)?;
    writeln!(out)?;
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
    fn expires_rejects_garbage() {
        let now = at("2026-11-05T10:00:00Z");
        for bad in ["", "d", "7", "7y", "x7d", "tomorrow", "7é"] {
            assert!(parse_expires(bad, now).is_err(), "{bad:?} accepted");
        }
    }
}
