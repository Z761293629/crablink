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

    let trust_proxy = match std::env::var("CRABLINK_TRUST_PROXY").as_deref() {
        Err(_) | Ok("false") => false,
        Ok("true") => true,
        Ok(_) => {
            eprintln!("crablink-server: CRABLINK_TRUST_PROXY must be true or false");
            exit(2);
        }
    };
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

    // `vars()` would panic on any variable that isn't Unicode, even one not ours.
    let vars = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)));
    let identity_providers = identity_providers(vars).unwrap_or_else(|e| {
        eprintln!("crablink-server: {e}");
        exit(2);
    });

    let pool = open_database(&db_url).await;
    let settings = crablink_server::Settings {
        trust_proxy,
        identity_providers,
        country,
        ..crablink_server::Settings::new(base_url)
    };
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
/// What each Identity Provider needs, as `CRABLINK_IDENTITY_PROVIDER_<ID>_<FIELD>`.
const IDENTITY_PROVIDER_FIELDS: [&str; 4] = ["NAME", "ISSUER", "CLIENT_ID", "CLIENT_SECRET"];

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
                "{var} isn't {IDENTITY_PROVIDER}<ID>_NAME, _ISSUER, _CLIENT_ID or _CLIENT_SECRET \
                 with an <ID> of A-Z, 0-9 and _"
            ));
        };
        found.entry(id.into()).or_default().insert(field, value);
    }
    found
        .into_iter()
        .map(|(id, mut fields)| {
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
