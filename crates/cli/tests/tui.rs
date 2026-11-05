mod common;

use common::*;
use crablink_api::CreateLink;

#[test]
fn lists_every_link_newest_first_across_pages() {
    let srv = server();
    for i in 0..51 {
        srv.create(&format!("link{i:03}"), &format!("https://example.com/{i}"));
    }

    let screen = srv.tui().screen();

    for col in ["Code", "Clicks", "Expiry", "Target"] {
        assert!(screen.contains(col), "missing {col}:\n{screen}");
    }
    assert!(screen.contains("https://example.com/50"), "{screen}");
    assert!(screen.contains("never"), "{screen}");
    let newest = screen.find("link050").expect(&screen);
    let older = screen.find("link049").expect(&screen);
    assert!(newest < older, "not newest first:\n{screen}");
    // link000 is on the second page: the total proves every page was loaded.
    assert!(screen.contains("51 Links"), "{screen}");
}

#[test]
fn expired_links_are_marked_and_deleted_links_are_absent() {
    let srv = server();
    let api = srv.api();
    api.create(&CreateLink {
        target: "https://example.com/old".into(),
        code: Some("old".into()),
        expires_at: Some("2020-01-01T00:00:00Z".parse().unwrap()),
    })
    .unwrap();
    srv.create("gone", "https://example.com/gone");
    api.delete("gone").unwrap();
    srv.create("live", "https://example.com/live");

    let screen = srv.tui().screen();

    let old = screen.lines().find(|l| l.contains("old")).expect(&screen);
    assert!(old.contains("expired"), "{old}");
    let live = screen.lines().find(|l| l.contains("live ")).expect(&screen);
    assert!(!live.contains("expired"), "{live}");
    assert!(!screen.contains("gone"), "{screen}");
    assert!(screen.contains("2 Links"), "{screen}");
}

#[test]
fn no_arguments_without_a_terminal_prints_help() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_crablink"))
        .env_remove("CRABLINK_URL")
        .env_remove("CRABLINK_TOKEN")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Usage: crablink"), "{stdout}");
    assert!(stdout.contains("login"), "{stdout}");
}

#[test]
fn an_empty_instance_says_how_to_create_the_first_link() {
    let screen = server().tui().screen();
    assert!(
        screen.contains("press n to create your first Link"),
        "{screen}"
    );
    assert!(screen.contains("0 Links"), "{screen}");
}

#[test]
fn a_wrong_token_shows_the_servers_error() {
    let srv = server();
    let wrong = TestServer {
        url: srv.url.clone(),
        token: "wrong".into(),
    };
    let screen = wrong.tui().screen();
    assert!(screen.contains("missing or invalid token"), "{screen}");
}

#[test]
fn an_unreachable_server_shows_an_error() {
    let wrong = TestServer {
        url: "http://127.0.0.1:1".into(),
        token: TOKEN.into(),
    };
    let screen = wrong.tui().screen();
    assert!(
        screen.contains("cannot reach http://127.0.0.1:1"),
        "{screen}"
    );
}

#[test]
fn the_sidebar_shows_server_total_and_key_hints() {
    let srv = server();
    srv.create("abc", "https://example.com/");
    let host = srv.url.trim_start_matches("http://");

    let screen = srv.tui().screen();

    let first = screen.lines().next().unwrap();
    assert!(first.starts_with(&format!(" {host}")), "{screen}");
    assert!(screen.contains("1 Link"), "{screen}");
    assert!(screen.contains("q quit"), "{screen}");
}

#[test]
fn below_80_columns_a_one_line_header_replaces_the_sidebar() {
    let srv = server();
    srv.create("abc", "https://example.com/");
    let host = srv.url.trim_start_matches("http://");
    let mut tui = srv.tui();

    tui.resize(79, 20);
    let screen = tui.screen();

    assert_eq!(screen.lines().next().unwrap(), format!("{host} · 1 Link"));
    assert!(!screen.contains("q quit"), "{screen}");
    assert!(screen.contains("abc"), "{screen}");

    tui.resize(80, 20);
    assert!(tui.screen().contains("q quit"));
}

#[test]
fn q_and_esc_quit() {
    let srv = server();
    let mut tui = srv.tui();
    tui.keys("jx");
    assert!(!tui.app.quit());
    tui.keys("q");
    assert!(tui.app.quit());

    let mut tui = srv.tui();
    tui.key(KeyCode::Esc);
    assert!(tui.app.quit());
}
