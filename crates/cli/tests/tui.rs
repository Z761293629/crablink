mod common;

use common::*;
use crablink_api::{CreateLink, UpdateLink};

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

/// The screen line containing `label`, e.g. the detail pane's "Short URL" row.
fn line_with<'a>(screen: &'a str, label: &str) -> &'a str {
    screen
        .lines()
        .find(|l| l.contains(label))
        .unwrap_or_else(|| panic!("no {label:?} line:\n{screen}"))
}

#[test]
fn arrows_and_jk_move_the_selection_and_the_detail_pane_follows() {
    let srv = server();
    srv.create("aaa", "https://example.com/a");
    srv.create("bbb", "https://example.com/b");
    let mut tui = srv.tui();
    let shows = |tui: &mut Tui, code: &str| {
        let screen = tui.screen();
        let short = line_with(&screen, "Short URL");
        assert!(short.contains(&format!("{}/{code}", srv.url)), "{screen}");
        // The selected row is highlighted with a bar in front of it.
        assert!(
            line_with(&screen, &format!("{code} ")).contains('▌'),
            "{screen}"
        );
    };

    shows(&mut tui, "bbb");
    tui.keys("j");
    shows(&mut tui, "aaa");
    tui.keys("j"); // already at the bottom
    shows(&mut tui, "aaa");
    tui.keys("k");
    shows(&mut tui, "bbb");
    tui.key(KeyCode::Down);
    shows(&mut tui, "aaa");
    tui.key(KeyCode::Up);
    shows(&mut tui, "bbb");
}

#[test]
fn the_detail_pane_shows_the_whole_link_and_the_list_truncates_long_targets() {
    let srv = server();
    let target = "https://example.com/a/rather/long/path/that/never/fits/in/a/row";
    let link = srv
        .api()
        .create(&CreateLink {
            target: target.into(),
            code: Some("long".into()),
            expires_at: Some("2099-01-02T03:04:05Z".parse().unwrap()),
        })
        .unwrap();

    let screen = srv.tui().screen();

    let row = line_with(&screen, "long ");
    assert!(row.contains("https://example.com/a/"), "{screen}");
    assert!(
        !row.contains(target),
        "not truncated in the list:\n{screen}"
    );
    let short = format!("{}/long", srv.url);
    assert!(line_with(&screen, "Short URL").contains(&short), "{screen}");
    line_with(&screen, "Target");
    // Only the detail pane can hold the whole Target (it wraps there if needed).
    assert!(screen.contains(target), "{screen}");
    assert!(line_with(&screen, "Click Count").contains('0'), "{screen}");
    let expiry = line_with(&screen, "2099-01-02T03:04:05Z");
    assert!(expiry.contains("Expiry"), "{screen}");
    let created = link.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    assert!(line_with(&screen, "Created").contains(&created), "{screen}");
}

#[test]
fn slash_filters_by_code_or_target_case_insensitively() {
    let srv = server();
    srv.create("alpha", "https://one.example/");
    srv.create("beta", "https://TWO.example/");
    srv.create("gamma", "https://three.example/AlphaBet");
    let mut tui = srv.tui();
    // The Codes in the list, top to bottom.
    let codes = |tui: &mut Tui| {
        let screen = tui.screen();
        screen
            .lines()
            .filter_map(|l| {
                ["alpha ", "beta ", "gamma "]
                    .into_iter()
                    .find(|c| l.contains(c))
            })
            .map(str::trim)
            .collect::<Vec<_>>()
    };

    tui.keys("/ALPH");
    assert_eq!(codes(&mut tui), ["gamma", "alpha"]);
    assert!(tui.screen().contains("/ALPH"));
    tui.key(KeyCode::Backspace);
    tui.keys("H");
    assert_eq!(codes(&mut tui), ["gamma", "alpha"]);

    // While filtering, letters are typed into the filter, not taken as keys.
    for _ in 0..4 {
        tui.key(KeyCode::Backspace);
    }
    tui.keys("two");
    assert_eq!(codes(&mut tui), ["beta"]);
    assert!(!tui.app.quit());

    // Enter keeps the filter and returns the keys to the list.
    tui.key(KeyCode::Enter);
    tui.keys("k");
    assert_eq!(codes(&mut tui), ["beta"]);

    // Clearing the filter brings every Link back.
    tui.keys("/");
    for _ in 0..3 {
        tui.key(KeyCode::Backspace);
    }
    assert_eq!(codes(&mut tui), ["gamma", "beta", "alpha"]);

    // Esc while filtering clears the filter instead of quitting.
    tui.keys("zzz");
    assert!(tui.screen().contains("no Links match"), "{}", tui.screen());
    tui.key(KeyCode::Esc);
    assert!(!tui.app.quit());
    assert_eq!(codes(&mut tui), ["gamma", "beta", "alpha"]);
}

#[test]
fn r_picks_up_links_created_and_changed_on_the_server() {
    let srv = server();
    srv.create("aaa", "https://example.com/before");
    let mut tui = srv.tui();
    assert!(tui.screen().contains("1 Link"));

    srv.create("bbb", "https://example.com/new");
    let patch = UpdateLink {
        target: Some("https://example.com/after".into()),
        ..Default::default()
    };
    srv.api().update("aaa", &patch).unwrap();
    let screen = tui.screen();
    assert!(!screen.contains("bbb"), "{screen}");

    tui.keys("r");
    let screen = tui.screen();
    assert!(screen.contains("2 Links"), "{screen}");
    assert!(
        line_with(&screen, "bbb ").contains("example.com/new"),
        "{screen}"
    );
    assert!(
        line_with(&screen, "aaa ").contains("example.com/after"),
        "{screen}"
    );
    // The selection stays on the same Link, though a newer one now sits above it.
    let short = format!("{}/aaa", srv.url);
    assert!(line_with(&screen, "Short URL").contains(&short), "{screen}");
}

#[test]
fn question_mark_shows_every_key_and_the_version() {
    let srv = server();
    srv.create("abc", "https://example.com/");
    let mut tui = srv.tui();
    tui.resize(79, 30); // no sidebar, so the hints on screen come from the help panel
    assert!(!tui.screen().contains("refresh"));

    tui.keys("?");
    let screen = tui.screen();
    let version = format!("crablink {}", env!("CARGO_PKG_VERSION"));
    assert!(screen.contains(&version), "{screen}");
    for key in [
        "↑↓",
        "jk move",
        "/ filter",
        "r refresh",
        "? help",
        "q quit",
        "Esc",
    ] {
        assert!(screen.contains(key), "missing {key:?}:\n{screen}");
    }

    // Any key closes the help, without acting on the list.
    tui.keys("q");
    assert!(!tui.app.quit());
    assert!(!tui.screen().contains("refresh"), "{}", tui.screen());
}
