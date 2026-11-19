mod common;

use common::*;

#[test]
fn create_with_domain_puts_the_link_on_that_domain() {
    let s = server();
    s.approved_domain("go.example.com");

    let out = s.cli(&[
        "create",
        "https://example.com/sale",
        "--code",
        "sale",
        "--domain",
        "go.example.com",
    ]);

    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "http://go.example.com/sale\n");
    let link = s.raw_get("/links/sale?domain=go.example.com");
    assert_eq!(link["target"], "https://example.com/sale");
}

#[test]
fn create_on_a_domain_the_workspace_cannot_use_fails_with_the_servers_message() {
    let s = server();

    let out = s.cli(&[
        "create",
        "https://example.com/a",
        "--domain",
        "go.example.com",
    ]);

    assert!(!out.success);
    assert!(out.stderr.contains("approved Domain"), "{}", out.stderr);
}

/// Creates Link `sale` on the Default Domain, then `sale` on approved Domain
/// `go.example.com`, so the custom-Domain one is newest and listed first.
fn same_code_on_two_domains() -> TestServer {
    let s = server();
    s.approved_domain("go.example.com");
    s.create("sale", "https://example.com/default");
    s.api()
        .create(&crablink_api::CreateLink {
            target: "https://example.com/custom".into(),
            code: Some("sale".into()),
            expires_at: None,
            domain: Some("go.example.com".into()),
        })
        .unwrap();
    s
}

/// Every Link on the server as `Short URL -> Target`, newest first.
fn links(s: &TestServer) -> Vec<String> {
    let mut links = Vec::new();
    s.api().list(|page| links.extend(page)).unwrap();
    links
        .into_iter()
        .map(|l| format!("{} -> {}", l.short_url, l.target))
        .collect()
}

#[test]
fn tui_deletes_the_selected_custom_domain_link_not_the_default_domains() {
    let s = same_code_on_two_domains();
    let mut tui = s.tui();

    tui.keys("dy");

    assert_eq!(
        links(&s),
        [format!("{}/sale -> https://example.com/default", s.url)]
    );
}

#[test]
fn tui_edits_the_selected_custom_domain_link_not_the_default_domains() {
    let s = same_code_on_two_domains();
    let mut tui = s.tui();

    tui.keys("e");
    for _ in 0.."custom".len() {
        tui.key(KeyCode::Backspace);
    }
    tui.keys("edited");
    tui.key(KeyCode::Enter);

    assert_eq!(
        links(&s),
        [
            "http://go.example.com/sale -> https://example.com/edited".to_string(),
            format!("{}/sale -> https://example.com/default", s.url),
        ]
    );
}

#[test]
fn tui_reload_keeps_the_default_domain_link_selected_not_its_code_twin() {
    let s = same_code_on_two_domains();
    let mut tui = s.tui();

    tui.keys("jr");

    let screen = tui.screen();
    let detail = screen.lines().find(|l| l.contains("Short URL")).unwrap();
    assert!(detail.contains(&format!("{}/sale", s.url)), "{screen}");
}

#[test]
fn get_with_domain_shows_the_link_on_that_domain() {
    let s = same_code_on_two_domains();

    let on_custom = s.cli(&["get", "sale", "--domain", "go.example.com", "--json"]);
    let on_default = s.cli(&["get", "sale", "--json"]);

    assert_eq!(on_custom.json()["target"], "https://example.com/custom");
    assert_eq!(on_default.json()["target"], "https://example.com/default");
}

#[test]
fn edit_with_domain_changes_only_the_link_on_that_domain() {
    let s = same_code_on_two_domains();

    let out = s.cli(&[
        "edit",
        "sale",
        "--domain",
        "go.example.com",
        "--target",
        "https://example.com/edited",
    ]);

    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        links(&s),
        [
            "http://go.example.com/sale -> https://example.com/edited".to_string(),
            format!("{}/sale -> https://example.com/default", s.url),
        ]
    );
}

#[test]
fn rm_with_domain_deletes_only_the_link_on_that_domain() {
    let s = same_code_on_two_domains();

    let out = s.cli(&["rm", "sale", "--domain", "go.example.com"]);

    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        links(&s),
        [format!("{}/sale -> https://example.com/default", s.url)]
    );
}

#[test]
fn qr_with_domain_saves_the_qr_code_of_the_link_on_that_domain() {
    let s = same_code_on_two_domains();
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("qr-domain.svg");

    let out = s.cli(&[
        "qr",
        "sale",
        "--domain",
        "go.example.com",
        "-o",
        path.to_str().unwrap(),
    ]);

    assert!(out.success, "{}", out.stderr);
    let svg = crablink_api::QrFormat::Svg;
    let custom = s.api().qr("sale", Some("go.example.com"), svg).unwrap();
    assert_ne!(custom, s.api().qr("sale", None, svg).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), custom);
}
