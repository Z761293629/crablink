mod common;

use common::*;
use crablink_api::CreateLink;

#[test]
fn api_client_creates_links_and_lists_them_across_pages() {
    let api = server().api();
    for i in 0..51 {
        let link = api
            .create(&CreateLink {
                target: format!("https://example.com/{i}"),
                code: Some(format!("link{i:03}")),
                expires_at: None,
                domain: None,
                tags: Vec::new(),
            })
            .unwrap();
        assert_eq!(link.code, format!("link{i:03}"));
    }

    let mut pages = Vec::new();
    api.list(None, None, |page| pages.push(page)).unwrap();

    assert!(
        pages.len() > 1,
        "expected several pages, got {}",
        pages.len()
    );
    let codes: Vec<String> = pages.concat().into_iter().map(|l| l.code).collect();
    let expected: Vec<String> = (0..51).rev().map(|i| format!("link{i:03}")).collect();
    assert_eq!(codes, expected);
}

#[test]
fn over_the_link_limit_the_api_client_says_how_long_to_wait() {
    let api = server_with(|s| s.rate_limits.links = Some("1/1h".parse().unwrap())).api();
    let link = |target: &str| CreateLink {
        target: target.into(),
        code: None,
        expires_at: None,
        domain: None,
        tags: Vec::new(),
    };
    api.create(&link("https://example.com/1")).unwrap();

    let err = api.create(&link("https://example.com/2")).unwrap_err();
    let err = err.to_string();
    // 3600 seconds, less however long the first request took.
    assert!(
        err.starts_with("too many requests: try again in 3")
            && err.ends_with(" seconds (429 Too Many Requests)"),
        "{err}"
    );
}
