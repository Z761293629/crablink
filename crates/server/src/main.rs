use std::collections::{BTreeMap, HashMap};
use std::io::IsTerminal;
use std::process::exit;
use std::sync::Arc;

use crablink_server::IdentityProvider;

fn required(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("crablink-server: {name} must be set and non-empty");
            exit(2);
        }
    }
}

/// A rate limit from env var `name` such as `10/1m`; `None` when unset.
fn rate_limit(name: &str) -> Option<crablink_server::Limit> {
    let value = std::env::var(name).ok()?;
    Some(value.parse().unwrap_or_else(|e| {
        eprintln!("crablink-server: {name}: {e}");
        exit(2);
    }))
}

/// Env var `name` as `true` or `false`; `false` when unset.
fn flag(name: &str) -> bool {
    match std::env::var(name).as_deref() {
        Err(_) | Ok("false") => false,
        Ok("true") => true,
        Ok(_) => {
            eprintln!("crablink-server: {name} must be true or false");
            exit(2);
        }
    }
}

/// An `http(s)` URL from env var `name`; `None` when unset or empty.
fn page_url(name: &str) -> Option<String> {
    let value = std::env::var(name).ok().filter(|v| !v.trim().is_empty())?;
    match url::Url::parse(value.trim()) {
        Ok(url) if ["http", "https"].contains(&url.scheme()) => Some(url.into()),
        _ => {
            eprintln!("crablink-server: {name} must be an http(s) URL");
            exit(2);
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => serve().await,
        ["admin", "create", "--email", email] => admin_create(email).await,
        _ => {
            eprintln!("usage: crablink-server [admin create --email <email>]");
            exit(2);
        }
    }
}

/// `admin create`: creates an Instance Admin, reading the password from a prompt,
/// or from stdin's first line when it isn't a terminal.
async fn admin_create(email: &str) {
    let db_url = required("CRABLINK_DATABASE_URL");
    let password = if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Password: ")
    } else {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).map(|_| line)
    };
    let password = password.unwrap_or_else(|e| {
        eprintln!("crablink-server: cannot read the password: {e}");
        exit(1);
    });
    let pool = open_database(&db_url).await;
    let password = password.trim_end_matches(['\r', '\n']);
    if let Err(e) = crablink_server::create_user(&pool, email, password, true).await {
        eprintln!("crablink-server: {e}");
        exit(1);
    }
    eprintln!("crablink-server: created Instance Admin {email}");
}

/// The SMTP mailer from `CRABLINK_SMTP_URL` and `CRABLINK_SMTP_FROM`, which go
/// together; `None` when neither is set.
fn mailer() -> Option<crablink_server::Mailer> {
    let set = |name| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    let (url, from) = match (set("CRABLINK_SMTP_URL"), set("CRABLINK_SMTP_FROM")) {
        (None, None) => return None,
        (Some(url), Some(from)) => (url, from),
        _ => {
            eprintln!("crablink-server: CRABLINK_SMTP_URL and CRABLINK_SMTP_FROM go together");
            exit(2);
        }
    };
    Some(
        crablink_server::smtp_mailer(&url, &from).unwrap_or_else(|e| {
            eprintln!("crablink-server: CRABLINK_SMTP_*: {e}");
            exit(2);
        }),
    )
}

async fn open_database(db_url: &str) -> sqlx::PgPool {
    crablink_server::connect(db_url).await.unwrap_or_else(|e| {
        // Not the URL itself: it usually carries the database password.
        eprintln!("crablink-server: cannot open the database: {e}");
        exit(1);
    })
}

async fn serve() {
    let base_url = required("CRABLINK_BASE_URL");
    let db_url = required("CRABLINK_DATABASE_URL");
    let listen = std::env::var("CRABLINK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());

    if std::env::var_os("CRABLINK_TOKEN").is_some() {
        eprintln!(
            "crablink-server: CRABLINK_TOKEN is no longer used: programs sign in with an API Key"
        );
    }

    let trust_proxy = flag("CRABLINK_TRUST_PROXY");
    let country: crablink_server::CountryLookup = match std::env::var("CRABLINK_COUNTRY_DB") {
        Ok(path) => crablink_server::dbip_country_lookup(&path).unwrap_or_else(|e| {
            eprintln!("crablink-server: cannot read the country database {path}: {e}");
            exit(1);
        }),
        Err(_) => {
            eprintln!("crablink-server: CRABLINK_COUNTRY_DB is not set, so Clicks get no country");
            Arc::new(|_| None)
        }
    };

    let rate_limits = crablink_server::RateLimits {
        sign_in: rate_limit("CRABLINK_RATE_LIMIT_SIGN_IN"),
        sign_up: rate_limit("CRABLINK_RATE_LIMIT_SIGN_UP"),
        password_reset: rate_limit("CRABLINK_RATE_LIMIT_PASSWORD_RESET"),
        links: rate_limit("CRABLINK_RATE_LIMIT_LINKS"),
        reports: rate_limit("CRABLINK_RATE_LIMIT_REPORTS"),
    };

    let reserved_codes = std::env::var("CRABLINK_RESERVED_CODES")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(String::from)
        .collect();

    // `vars()` would panic on any variable that isn't Unicode, even one not ours.
    let vars = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)));
    let identity_providers = identity_providers(vars).unwrap_or_else(|e| {
        eprintln!("crablink-server: {e}");
        exit(2);
    });

    let mut settings = crablink_server::Settings {
        trust_proxy,
        rate_limits,
        reserved_codes,
        identity_providers,
        country,
        open_sign_up: flag("CRABLINK_OPEN_SIGN_UP"),
        terms_url: page_url("CRABLINK_TERMS_URL"),
        privacy_url: page_url("CRABLINK_PRIVACY_URL"),
        ..crablink_server::Settings::new(base_url)
    };
    match std::env::var("CRABLINK_SAFE_BROWSING_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            settings.target_check =
                crablink_server::google_safe_browsing(key).unwrap_or_else(|e| {
                    eprintln!("crablink-server: cannot set up Safe Browsing checks: {e}");
                    exit(1);
                });
        }
        _ => {}
    }
    settings.mailer = mailer();

    let pool = open_database(&db_url).await;
    let app = crablink_server::app(pool.clone(), settings)
        .await
        .unwrap_or_else(|e| {
            eprintln!("crablink-server: {e}");
            exit(1);
        });
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|e| {
            eprintln!("crablink-server: cannot listen on {listen}: {e}");
            exit(1);
        });
    eprintln!("crablink-server listening on {listen}");
    let app = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("crablink-server: server error: {e}");
        exit(1);
    }
    pool.close().await;
    eprintln!("crablink-server stopped");
}

const IDENTITY_PROVIDER: &str = "CRABLINK_IDENTITY_PROVIDER_";
/// What each Identity Provider is set up with, as
/// `CRABLINK_IDENTITY_PROVIDER_<ID>_<FIELD>`; all but the last are needed.
const IDENTITY_PROVIDER_FIELDS: [&str; 5] = [
    "NAME",
    "ISSUER",
    "CLIENT_ID",
    "CLIENT_SECRET",
    "ALLOWED_EMAIL_DOMAINS",
];

/// The Identity Providers that `CRABLINK_IDENTITY_PROVIDER_<ID>_<FIELD>` variables
/// among `vars` set up, each with id `<ID>` lowercased, `_` as `-`. The error names
/// an incomplete one, or a variable that isn't one of those.
fn identity_providers(
    vars: impl IntoIterator<Item = (String, String)>,
) -> Result<Vec<IdentityProvider>, String> {
    let mut found: BTreeMap<String, HashMap<&str, String>> = BTreeMap::new();
    for (var, value) in vars {
        let Some(rest) = var.strip_prefix(IDENTITY_PROVIDER) else {
            continue;
        };
        let id_and_field = IDENTITY_PROVIDER_FIELDS
            .iter()
            .find_map(|f| Some((rest.strip_suffix(f)?.strip_suffix('_')?, *f)))
            .filter(|(id, _)| {
                !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        let Some((id, field)) = id_and_field else {
            return Err(format!(
                "{var} isn't {IDENTITY_PROVIDER}<ID>_NAME, _ISSUER, _CLIENT_ID, _CLIENT_SECRET \
                 or _ALLOWED_EMAIL_DOMAINS with an <ID> of A-Z, 0-9 and _"
            ));
        };
        found.entry(id.into()).or_default().insert(field, value);
    }
    found
        .into_iter()
        .map(|(id, mut fields)| {
            let allowed_email_domains = fields
                .remove("ALLOWED_EMAIL_DOMAINS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(String::from)
                .collect();
            let mut take = |field| {
                fields
                    .remove(field)
                    .filter(|v| !v.trim().is_empty())
                    .ok_or_else(|| {
                        format!(
                            "Identity Provider {id} is incomplete: \
                             {IDENTITY_PROVIDER}{id}_{field} must be set and non-empty"
                        )
                    })
            };
            Ok(IdentityProvider {
                allowed_email_domains,
                name: take("NAME")?,
                issuer: take("ISSUER")?,
                client_id: take("CLIENT_ID")?,
                client_secret: take("CLIENT_SECRET")?,
                id: id.to_ascii_lowercase().replace('_', "-"),
            })
        })
        .collect()
}

/// Resolves on Ctrl-C, or on SIGTERM (what Docker and systemd send) on Unix.
/// In-flight requests then finish before `serve` returns.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("installing a SIGTERM handler");
        tokio::select! {
            // An `Err` here would otherwise read as "Ctrl-C pressed" and stop the server at once.
            r = tokio::signal::ctrl_c() => r.expect("installing a Ctrl-C handler"),
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("installing a Ctrl-C handler");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(vars: &[(&str, &str)]) -> Vec<(String, String)> {
        vars.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn identity_providers_come_from_their_variables() {
        let set_up = identity_providers(vars(&[
            ("CRABLINK_BASE_URL", "https://crabl.ink"),
            ("CRABLINK_IDENTITY_PROVIDER_MY_KEYCLOAK_NAME", "Keycloak"),
            (
                "CRABLINK_IDENTITY_PROVIDER_MY_KEYCLOAK_ISSUER",
                "https://id.example.com",
            ),
            (
                "CRABLINK_IDENTITY_PROVIDER_MY_KEYCLOAK_CLIENT_ID",
                "crablink",
            ),
            (
                "CRABLINK_IDENTITY_PROVIDER_MY_KEYCLOAK_CLIENT_SECRET",
                "s3cret",
            ),
            (
                "CRABLINK_IDENTITY_PROVIDER_MY_KEYCLOAK_ALLOWED_EMAIL_DOMAINS",
                " example.com, example.org ,",
            ),
        ]))
        .unwrap();
        let [p] = &set_up[..] else {
            panic!("expected one Identity Provider, got {}", set_up.len());
        };
        assert_eq!(
            [&p.id, &p.name, &p.issuer, &p.client_id, &p.client_secret],
            [
                "my-keycloak",
                "Keycloak",
                "https://id.example.com",
                "crablink",
                "s3cret"
            ]
        );
        assert_eq!(p.allowed_email_domains, ["example.com", "example.org"]);
        assert!(identity_providers(vars(&[])).unwrap().is_empty());
    }

    #[test]
    fn an_incomplete_identity_provider_is_named() {
        let e = identity_providers(vars(&[
            ("CRABLINK_IDENTITY_PROVIDER_GOOGLE_NAME", "Google"),
            (
                "CRABLINK_IDENTITY_PROVIDER_GOOGLE_ISSUER",
                "https://accounts.google.com",
            ),
            ("CRABLINK_IDENTITY_PROVIDER_GOOGLE_CLIENT_ID", "crablink"),
        ]))
        .err()
        .unwrap();
        assert!(
            e.contains("CRABLINK_IDENTITY_PROVIDER_GOOGLE_CLIENT_SECRET"),
            "{e}"
        );

        let misspelt = vars(&[("CRABLINK_IDENTITY_PROVIDER_GOOGLE_SECRET", "s3cret")]);
        let e = identity_providers(misspelt).err().unwrap();
        assert!(
            e.contains("CRABLINK_IDENTITY_PROVIDER_GOOGLE_SECRET"),
            "{e}"
        );
    }
}
