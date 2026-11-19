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
