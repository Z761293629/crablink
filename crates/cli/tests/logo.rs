mod common;

use common::*;
use crablink_cli::tui::{Caps, Lockup, lockup};
use ratatui_image::picker::{Picker, ProtocolType};

#[test]
fn a_light_background_gets_the_standard_lockup() {
    assert_eq!(lockup(Some((255, 255, 255))), Lockup::Standard);
    assert_eq!(lockup(Some((0xF6, 0xF1, 0xE7))), Lockup::Standard);
}

#[test]
fn a_dark_background_gets_the_reversed_lockup() {
    assert_eq!(lockup(Some((0, 0, 0))), Lockup::Reversed);
    assert_eq!(lockup(Some((0x28, 0x2C, 0x34))), Lockup::Reversed);
}

#[test]
fn mid_grey_129_is_light_and_128_is_dark() {
    assert_eq!(lockup(Some((129, 129, 129))), Lockup::Standard);
    assert_eq!(lockup(Some((128, 128, 128))), Lockup::Reversed);
}

#[test]
fn an_unknown_background_gets_the_reversed_lockup() {
    assert_eq!(lockup(None), Lockup::Reversed);
}

fn picker(protocol: ProtocolType) -> Picker {
    let mut p = Picker::halfblocks(); // 10x20 px cells
    p.set_protocol_type(protocol);
    p
}

#[test]
fn halfblocks_shows_no_logo() {
    let srv = server();
    let host = srv.url.trim_start_matches("http://");
    let caps = Caps::from_picker(&picker(ProtocolType::Halfblocks));
    let mut tui = Tui::with_caps(srv.api(), caps);

    let screen = tui.screen();

    // Nothing above the server: no logo, and no character approximation of one.
    let first = screen.lines().next().unwrap();
    assert!(first.starts_with(&format!(" {host}")), "{screen}");
    assert!(!screen.contains(['▀', '▄']), "{screen}");
}

#[test]
fn an_image_protocol_puts_the_logo_above_the_server() {
    let srv = server();
    let host = srv.url.trim_start_matches("http://");
    let caps = Caps::from_picker(&picker(ProtocolType::Kitty));
    let mut tui = Tui::with_caps(srv.api(), caps);

    let screen = tui.screen();

    // 800x661 px, 20 cols of 10x20 px cells: 9 rows. 1 margin + 9 logo + 2 clear space.
    let row = screen
        .lines()
        .position(|l| l.contains(host))
        .expect(&screen);
    assert_eq!(row, 12, "{screen}");
    assert!(screen.contains("q quit"), "{screen}");
}

#[test]
fn a_short_terminal_clamps_the_logo() {
    let srv = server();
    let caps = Caps::from_picker(&picker(ProtocolType::Kitty));
    let mut tui = Tui::with_caps(srv.api(), caps);
    for h in [1, 2, 3, 8, 12] {
        tui.resize(100, h);
        tui.screen(); // drawing outside the buffer panics
    }
}
