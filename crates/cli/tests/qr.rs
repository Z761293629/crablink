mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

use common::*;

/// Runs `crablink qr <args>` against `srv` with stdout piped, never reading the
/// developer's own config.
fn crablink_qr(srv: &TestServer, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crablink"))
        .env("HOME", env!("CARGO_TARGET_TMPDIR"))
        .args(srv.flags())
        .arg("qr")
        .args(args)
        .output()
        .unwrap()
}

/// Nothing listens on port 9: reaching for the server would fail with `cannot reach`.
fn nowhere() -> TestServer {
    TestServer {
        url: "http://127.0.0.1:9".into(),
        key: "crab_unused".into(),
        workspace: "nobody".into(),
    }
}

/// A fresh path in the test scratch directory.
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn qr_with_a_png_file_writes_a_png_of_the_short_url() {
    let srv = server();
    let link = srv.create("poster", "https://example.com/");
    let path = scratch("qr-poster.png");

    let out = crablink_qr(&srv, &["poster", "-o", path.to_str().unwrap()]);

    assert!(out.status.success(), "{out:?}");
    let img = image::open(&path).expect("a valid PNG").into_luma8();
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
        img.width() as usize,
        img.height() as usize,
        |x, y| img.get_pixel(x as u32, y as u32).0[0],
    );
    let grids = prepared.detect_grids();
    assert_eq!(grids[0].decode().unwrap().1, link.short_url);
}

#[test]
fn qr_with_an_svg_file_writes_svg() {
    let srv = server();
    srv.create("poster", "https://example.com/");
    let path = scratch("qr-poster.svg");

    let out = crablink_qr(&srv, &["poster", "-o", path.to_str().unwrap()]);

    assert!(out.status.success(), "{out:?}");
    let svg = std::fs::read_to_string(&path).unwrap();
    assert!(svg.contains("<svg"), "{svg}");
}

#[test]
fn qr_rejects_other_extensions_before_calling_the_server() {
    let path = scratch("qr-poster.gif");

    let out = crablink_qr(&nowhere(), &["poster", "-o", path.to_str().unwrap()]);

    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains(".svg") && err.contains(".png"), "{err}");
    assert!(!err.contains("cannot reach"), "{err}");
    assert!(!path.exists());
}

#[test]
fn qr_without_a_file_asks_for_one_when_stdout_is_not_a_terminal() {
    let out = crablink_qr(&nowhere(), &["poster"]);

    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("-o"), "{err}");
    assert!(!err.contains("cannot reach"), "{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn half_blocks_draw_every_module_inside_a_four_module_quiet_zone() {
    let url = "https://crab.link/aZ3k9";

    let rows = crablink_cli::qr::half_blocks(url);

    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    assert_eq!(decode_half_blocks(&rows), url);
    // Version 2 (25 modules) plus 4 light modules each side, two modules per row.
    let size: usize = 25 + 2 * 4;
    assert_eq!(rows.len(), size.div_ceil(2));
    assert!(rows.iter().all(|r| r.chars().count() == size));
    // The quiet zone: the first two rows (4 modules) and the outer 4 columns are light.
    assert!(rows[..2].iter().all(|r| r.trim().is_empty()));
    assert!(
        rows.iter()
            .all(|r| r.starts_with("    ") && r.ends_with("    "))
    );
}
