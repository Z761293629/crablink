mod common;

use common::*;

#[test]
fn get_json_prints_the_link_as_the_api_returns_it() {
    let s = server();
    s.create("abc", "https://example.com/a");

    let out = s.cli(&["get", "abc", "--json"]);

    assert_eq!(out.json(), s.raw_get("/api/links/abc"));
}

#[test]
fn json_errors_go_to_stderr_with_a_failing_exit_and_nothing_on_stdout() {
    let s = server();

    let out = s.cli(&["get", "missing", "--json"]);

    assert!(!out.success);
    assert_eq!(out.stdout, "");
    assert!(out.stderr.contains("404"), "{}", out.stderr);
}

#[test]
fn ls_json_prints_one_array_of_every_link_across_pages_newest_first() {
    let s = server();
    // One more than the API's default page size, so `ls` has to follow the cursor.
    for i in 0..51 {
        s.create(&format!("link{i:03}"), &format!("https://example.com/{i}"));
    }

    let links = s.cli(&["ls", "--json"]).json();

    let codes: Vec<&str> = links
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["code"].as_str().unwrap())
        .collect();
    let expected: Vec<String> = (0..51).rev().map(|i| format!("link{i:03}")).collect();
    assert_eq!(codes, expected);
    assert_eq!(links, s.raw_get("/api/links?limit=200")["links"]);
}

#[test]
fn create_json_prints_the_created_link_as_the_api_returns_it() {
    let s = server();

    let out = s.cli(&[
        "create",
        "https://example.com/new",
        "--code",
        "fresh",
        "--expires",
        "2030-01-01T00:00:00Z",
        "--json",
    ]);

    let link = out.json();
    assert_eq!(link["code"], "fresh");
    assert_eq!(link["expires_at"], "2030-01-01T00:00:00Z");
    assert_eq!(link, s.raw_get("/api/links/fresh"));
}

#[test]
fn edit_json_prints_the_updated_link_as_the_api_returns_it() {
    let s = server();
    s.create("abc", "https://example.com/old");

    let out = s.cli(&[
        "edit",
        "abc",
        "--target",
        "https://example.com/new",
        "--json",
    ]);

    let link = out.json();
    assert_eq!(link["target"], "https://example.com/new");
    assert_eq!(link, s.raw_get("/api/links/abc"));
}

#[test]
fn json_into_a_closed_pipe_exits_quietly_like_any_unix_tool() {
    let s = server();
    s.create("abc", "https://example.com/a");
    // `crablink ls --json | head -0`: the reader is gone before the first write.
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_crablink"))
        .args(["--server", &s.url, "--token", &s.token, "ls", "--json"])
        .env("HOME", std::env::temp_dir().join("crablink-test-no-home"))
        .stdout(writer)
        .output()
        .unwrap();

    assert!(out.status.success(), "{:?}", out.status);
    assert_eq!(String::from_utf8_lossy(&out.stderr), "");
}
