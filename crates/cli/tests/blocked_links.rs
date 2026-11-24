mod common;

use common::*;

#[test]
fn ls_and_get_show_a_blocked_link_as_blocked() {
    let s = server();
    s.create("phish", "https://evil.example");
    s.create("fine", "https://fine.example");
    s.block("phish");

    let ls = s.cli(&["ls"]);
    let get = s.cli(&["get", "phish"]);

    assert!(ls.success, "{}", ls.stderr);
    let phish = ls.stdout.lines().find(|l| l.contains("phish")).unwrap();
    assert!(phish.contains("blocked"), "{phish}");
    let fine = ls.stdout.lines().find(|l| l.contains("fine")).unwrap();
    assert!(!fine.contains("blocked"), "{fine}");
    assert!(get.stdout.contains("Blocked:"), "{}", get.stdout);
    let get_fine = s.cli(&["get", "fine"]);
    assert!(!get_fine.stdout.contains("Blocked"), "{}", get_fine.stdout);
}

#[test]
fn editing_a_blocked_link_fails_saying_it_was_blocked() {
    let s = server();
    s.create("phish", "https://evil.example");
    s.block("phish");

    let out = s.cli(&["edit", "phish", "--target", "https://other.example"]);

    assert!(!out.success);
    assert!(out.stderr.contains("blocked"), "{}", out.stderr);
}

#[test]
fn the_tui_marks_a_blocked_link_and_refuses_to_edit_it() {
    let s = server();
    s.create("phish", "https://evil.example");
    s.block("phish");
    let mut tui = s.tui();

    let screen = tui.screen();
    let row = screen.lines().find(|l| l.contains("phish")).expect(&screen);
    assert!(row.contains("blocked"), "{row}");

    tui.keys("e");
    let screen = tui.screen();
    assert!(
        !screen.contains(" edit Link "),
        "opened the form:\n{screen}"
    );
    assert!(screen.contains("phish was blocked"), "{screen}");
}
