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
            })
            .unwrap();
        assert_eq!(link.code, format!("link{i:03}"));
    }

    let mut pages = Vec::new();
    api.list(|page| pages.push(page)).unwrap();

    assert!(pages.len() > 1, "expected several pages, got {}", pages.len());
    let codes: Vec<String> = pages.concat().into_iter().map(|l| l.code).collect();
    let expected: Vec<String> = (0..51).rev().map(|i| format!("link{i:03}")).collect();
    assert_eq!(codes, expected);
}
