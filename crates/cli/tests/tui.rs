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
        "d delete",
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

/// Every Link on the server, newest first.
fn server_links(srv: &TestServer) -> Vec<crablink_api::Link> {
    let mut links = Vec::new();
    srv.api().list(|page| links.extend(page)).unwrap();
    links
}

#[test]
fn n_creates_a_link_with_a_generated_code_and_selects_it() {
    let srv = server();
    srv.create("older", "https://example.com/older");
    let mut tui = srv.tui();
    tui.keys("j"); // move off the top, so selecting the new Link is the form's doing

    tui.keys("n");
    assert!(tui.screen().contains("Custom Code"), "{}", tui.screen());
    tui.keys("https://example.com/new");
    tui.key(KeyCode::Enter);

    let links = server_links(&srv);
    assert_eq!(links.len(), 2);
    let new = &links[0];
    assert_eq!(new.target, "https://example.com/new");
    assert_eq!(new.expires_at, None);
    let screen = tui.screen();
    assert!(
        !screen.contains("Custom Code"),
        "form still open:\n{screen}"
    );
    assert!(screen.contains("2 Links"), "{screen}");
    assert!(
        line_with(&screen, "Short URL").contains(&new.short_url),
        "{screen}"
    );
}

#[test]
fn the_form_takes_a_custom_code_and_either_expiry_format() {
    let srv = server();
    let mut tui = srv.tui();
    // Fill the form: Target, then Tab to Custom Code, Tab to Expiry.
    let create = |tui: &mut Tui, target: &str, code: &str, expires: &str| {
        tui.keys("n");
        tui.keys(target);
        tui.key(KeyCode::Tab);
        tui.keys(code);
        tui.key(KeyCode::Tab);
        tui.keys(expires);
        tui.key(KeyCode::Enter);
    };

    let before = chrono::Utc::now();
    create(&mut tui, "https://example.com/rel", "rel", "7d");
    create(
        &mut tui,
        "https://example.com/abs",
        "abs",
        "2099-01-02T03:04:05Z",
    );

    let rel = srv.api().get("rel").unwrap();
    assert_eq!(rel.target, "https://example.com/rel");
    let in_7d = rel.expires_at.unwrap() - before;
    let week = chrono::TimeDelta::days(7);
    assert!(
        in_7d >= week && in_7d < week + chrono::TimeDelta::minutes(1),
        "{in_7d}"
    );
    let abs = srv.api().get("abs").unwrap();
    assert_eq!(
        abs.expires_at,
        Some("2099-01-02T03:04:05Z".parse().unwrap())
    );
    let screen = tui.screen();
    assert!(
        line_with(&screen, "Short URL").contains(&abs.short_url),
        "{screen}"
    );
}

#[test]
fn a_taken_code_shows_the_servers_error_and_keeps_the_input() {
    let srv = server();
    srv.create("taken", "https://example.com/first");
    let mut tui = srv.tui();

    tui.keys("n");
    tui.keys("https://example.com/second");
    tui.key(KeyCode::Tab);
    tui.keys("taken");
    tui.key(KeyCode::Tab);
    tui.keys("12h");
    tui.key(KeyCode::Enter);

    let screen = tui.screen();
    assert!(screen.contains("code already taken (409"), "{screen}");
    // The form's rows: from its title down.
    let form = &screen[screen.find("new Link").expect(&screen)..];
    let field = |label| line_with(form, label);
    assert!(field("Target").contains("https://example.com/second"));
    assert!(field("Custom Code").contains("taken"));
    assert!(field("Expiry").contains("12h"));
    assert_eq!(server_links(&srv).len(), 1);

    // Still open and editable: back to the Code, fix it, and save.
    tui.key(KeyCode::BackTab);
    tui.keys("2");
    tui.key(KeyCode::Enter);
    let link = srv.api().get("taken2").unwrap();
    assert_eq!(link.target, "https://example.com/second");
    assert!(!tui.screen().contains("Custom Code"), "{}", tui.screen());
}

#[test]
fn esc_cancels_the_form_without_creating_anything() {
    let srv = server();
    let mut tui = srv.tui();

    tui.keys("n");
    tui.keys("https://example.com/q"); // `q` is typed, not taken as quit
    tui.key(KeyCode::Esc);

    assert!(!tui.app.quit());
    let screen = tui.screen();
    assert!(!screen.contains("Custom Code"), "{screen}");
    assert!(
        screen.contains("press n to create your first Link"),
        "{screen}"
    );
    assert!(server_links(&srv).is_empty());
    // A new form starts empty.
    tui.keys("n");
    assert!(!tui.screen().contains("example.com/q"), "{}", tui.screen());
}

#[test]
fn d_then_y_deletes_the_selected_link_after_naming_its_code() {
    let srv = server();
    srv.create("keep", "https://example.com/keep");
    srv.create("doomed", "https://example.com/doomed");
    let mut tui = srv.tui();

    tui.keys("d");
    let screen = tui.screen();
    assert!(line_with(&screen, "Delete").contains("doomed"), "{screen}");
    assert!(screen.contains("never be reused"), "{screen}");
    assert!(srv.api().get("doomed").is_ok());

    tui.keys("y");
    let screen = tui.screen();
    assert!(srv.api().get("doomed").is_err());
    assert!(!screen.contains("doomed"), "{screen}");
    assert!(!screen.contains("never be reused"), "{screen}");
    assert!(screen.contains("1 Link"), "{screen}");
}

#[test]
fn any_key_but_y_cancels_the_delete() {
    let srv = server();
    srv.create("safe", "https://example.com/safe");
    let mut tui = srv.tui();

    for key in [
        KeyCode::Char('n'),
        KeyCode::Char('q'),
        KeyCode::Esc,
        KeyCode::Enter,
        KeyCode::Down,
    ] {
        tui.keys("d");
        assert!(tui.screen().contains("never be reused"));
        tui.key(key);
        let screen = tui.screen();
        assert!(!screen.contains("never be reused"), "{key:?}:\n{screen}");
        assert!(!tui.app.quit(), "{key:?}");
        assert!(srv.api().get("safe").is_ok(), "{key:?}");
        assert!(screen.contains("safe "), "{key:?}:\n{screen}");
    }
}

#[test]
fn after_deleting_the_selection_lands_on_a_neighbour() {
    let srv = server();
    for code in ["ccc", "bbb", "aaa"] {
        srv.create(code, &format!("https://example.com/{code}"));
    }
    let mut tui = srv.tui();
    let selected = |tui: &mut Tui| {
        let screen = tui.screen();
        let short = line_with(&screen, "Short URL").to_owned();
        let row = line_with(&screen, "▌").to_owned();
        (short, row)
    };

    // List is aaa, bbb, ccc. Deleting the middle row selects the one below it.
    tui.keys("jdy");
    let (short, row) = selected(&mut tui);
    assert!(short.contains(&format!("{}/ccc", srv.url)), "{short}");
    assert!(row.contains("ccc"), "{row}");

    // Deleting the last row selects the one above it.
    tui.keys("dy");
    let (short, row) = selected(&mut tui);
    assert!(short.contains(&format!("{}/aaa", srv.url)), "{short}");
    assert!(row.contains("aaa"), "{row}");

    // Deleting the only Link leaves an empty list, and d then does nothing.
    tui.keys("dy");
    assert!(tui.screen().contains("0 Links"));
    tui.keys("d");
    assert!(!tui.screen().contains("never be reused"));
}
