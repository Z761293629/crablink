mod common;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use common::*;

/// A fresh, empty HOME in the test scratch directory.
fn home(name: &str) -> PathBuf {
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    home
}

/// `crablink <args>` with `home` as HOME, no credentials in the environment and
/// `stdin` piped in.
fn crablink(home: &PathBuf, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_crablink"))
        .args(args)
        .env("HOME", home)
        .env_remove("CRABLINK_URL")
        .env_remove("CRABLINK_API_KEY")
        .env_remove("CRABLINK_WORKSPACE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn login_saves_the_api_key_and_its_workspace_for_every_later_command() {
    let srv = server();
    srv.create("abc", "https://example.com/a");
    let home = home("login-ok");

    let out = crablink(&home, &["login", "--server", &srv.url], &srv.key);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains(&srv.workspace), "{stderr}");
    let config = home.join(".config/crablink/config.toml");
    let saved = std::fs::read_to_string(&config).unwrap();
    assert!(saved.contains(&srv.key), "{saved}");
    assert!(
        saved.contains(&format!("workspace = \"{}\"", srv.workspace)),
        "{saved}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&config).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    let out = crablink(&home, &["ls"], "");
    assert!(out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("abc"));
}

#[test]
fn login_with_a_0_2_token_says_to_use_an_api_key_and_saves_nothing() {
    let srv = server();
    let home = home("login-old-token");

    let out = crablink(&home, &["login", "--server", &srv.url], "test-token\n");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("login failed, nothing saved"), "{stderr}");
    assert!(stderr.contains("sign in again with an API Key"), "{stderr}");
    assert!(!home.join(".config/crablink/config.toml").exists());
}

#[test]
fn an_api_key_alone_finds_its_workspace() {
    let srv = server();
    srv.create("abc", "https://example.com/a");
    let home = home("api-key-alone");

    let args = ["--server", &srv.url, "--api-key", &srv.key, "get", "abc"];
    let out = crablink(&home, &args, "");

    assert!(out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("https://example.com/a"));
}
