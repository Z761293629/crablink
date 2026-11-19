mod common;

use common::*;

/// The Codes of the Short URLs `crablink ls` prints, first column, newest first.
fn ls_codes(s: &TestServer, args: &[&str]) -> Vec<String> {
    let out = s.cli(&[&["ls"], args].concat());
    assert!(out.success, "{}", out.stderr);
    out.stdout
        .lines()
        .map(|l| {
            let short_url = l.split_whitespace().next().unwrap();
            short_url.rsplit('/').next().unwrap().to_string()
        })
        .collect()
}

#[test]
fn create_takes_repeated_tags_and_ls_filters_by_tag_and_search() {
    let s = server();
    let out = s.cli(&[
        "create",
        "https://example.com/spring-sale",
        "--code",
        "sale",
        "--tag",
        "spring",
        "--tag",
        "promo",
    ]);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        s.raw_get("/links/sale")["tags"],
        serde_json::json!(["promo", "spring"])
    );
    let out = s.cli(&[
        "create",
        "https://example.com/blog",
        "--code",
        "blog",
        "--tag",
        "spring",
    ]);
    assert!(out.success, "{}", out.stderr);
    s.create("docs", "https://docs.example.com/SALE-terms");

    assert_eq!(ls_codes(&s, &["--tag", "spring"]), ["blog", "sale"]);
    assert_eq!(ls_codes(&s, &["--tag", "promo"]), ["sale"]);
    assert_eq!(ls_codes(&s, &["--search", "sale"]), ["docs", "sale"]);
    assert_eq!(
        ls_codes(&s, &["--tag", "spring", "--search", "blog"]),
        ["blog"]
    );

    let json = s.cli(&["ls", "--tag", "promo", "--json"]).json();
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["tags"], serde_json::json!(["promo", "spring"]));
}
