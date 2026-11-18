//! `Creds` and the config file that saves them; `crablink login` writes it.

use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};
use clap::Args;
use serde::{Deserialize, Serialize};
use ureq::SendBody;
use ureq::http::Method;

use crate::Api;

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
        Ok(Api::new(
            self.server.ok_or_else(|| missing("server"))?,
            self.token.ok_or_else(|| missing("token"))?,
        ))
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

pub(crate) fn load_config() -> anyhow::Result<Creds> {
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

pub(crate) fn login(server: Option<String>) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
