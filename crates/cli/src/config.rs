//! `Creds` and the config file that saves them; `crablink login` writes it.

use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::Api;

/// Server, API Key and Workspace. Flags beat env vars (clap), which beat the config
/// file (`or`). Also the shape of `~/.config/crablink/config.toml`; a 0.2 file's
/// `token` is ignored, so its user is sent to `crablink login`.
#[derive(Args, Serialize, Deserialize, Default)]
pub struct Creds {
    /// Server URL, e.g. https://crabl.ink
    #[arg(long, env = "CRABLINK_URL", global = true)]
    pub server: Option<String>,
    /// API Key of the Workspace to manage
    #[arg(long, env = "CRABLINK_API_KEY", global = true, hide_env_values = true)]
    pub api_key: Option<String>,
    /// Slug of the API Key's Workspace (asked from the server when missing)
    #[arg(long, env = "CRABLINK_WORKSPACE", global = true)]
    pub workspace: Option<String>,
}

impl Creds {
    /// Fills each missing field from `fallback`.
    pub fn or(self, fallback: Creds) -> Creds {
        Creds {
            server: self.server.or(fallback.server),
            api_key: self.api_key.or(fallback.api_key),
            workspace: self.workspace.or(fallback.workspace),
        }
    }

    /// The client, or an error pointing at `crablink login` if the server or API Key
    /// is missing. A missing Workspace is the API Key's own, asked from `/api/me`.
    pub fn api(self) -> anyhow::Result<Api> {
        let missing = |what| anyhow!("no {what} configured: run `crablink login --server <url>`");
        let server = self.server.ok_or_else(|| missing("server"))?;
        let api_key = self.api_key.ok_or_else(|| missing("API Key"))?;
        let workspace = match self.workspace {
            Some(workspace) => workspace,
            None => {
                Api::new(server.clone(), api_key.clone(), String::new())
                    .me()?
                    .slug
            }
        };
        Ok(Api::new(server, api_key, workspace))
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

/// Reads an API Key, learns its Workspace from the server and saves all three.
pub(crate) fn login(server: Option<String>) -> anyhow::Result<()> {
    let server = server.context("pass --server <url>")?;
    let api = Api::new(server, read_api_key()?, String::new());
    let workspace = api.me().context("login failed, nothing saved")?;
    let path = save_config(&Creds {
        server: Some(api.server),
        api_key: Some(api.api_key),
        workspace: Some(workspace.slug.clone()),
    })?;
    eprintln!(
        "Logged in to Workspace {} ({}); saved {}",
        workspace.name,
        workspace.slug,
        path.display()
    );
    Ok(())
}

/// Prompts with echo disabled on a terminal; reads one line from stdin otherwise (scripts).
fn read_api_key() -> anyhow::Result<String> {
    let key = if std::io::stdin().is_terminal() {
        rpassword::prompt_password("API Key: ")?
    } else {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        line
    };
    let key = key.trim().to_string();
    if key.is_empty() {
        bail!("empty API Key");
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(server: Option<&str>, api_key: Option<&str>) -> Creds {
        Creds {
            server: server.map(Into::into),
            api_key: api_key.map(Into::into),
            workspace: Some("ws".into()),
        }
    }

    #[test]
    fn flags_and_env_override_saved_config_per_field() {
        let saved = creds(Some("https://saved"), Some("saved-key"));
        let got = creds(None, Some("flag-key")).or(saved).api().unwrap();
        assert_eq!(got.server, "https://saved");
        assert_eq!(got.api_key, "flag-key");
        assert_eq!(got.workspace, "ws");
    }

    #[test]
    fn missing_credentials_point_at_login() {
        let err = creds(Some("https://x"), None).api().err().unwrap();
        assert!(err.to_string().contains("crablink login"), "{err}");
    }

    #[test]
    fn a_0_2_config_with_a_token_has_no_api_key() {
        let old: Creds = toml::from_str("server = \"https://x\"\ntoken = \"t\"\n").unwrap();
        let err = old.api().err().unwrap();
        assert!(err.to_string().contains("no API Key"), "{err}");
    }
}
