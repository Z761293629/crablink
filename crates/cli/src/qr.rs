//! QR codes drawn in the terminal, shared by `crablink qr` and the TUI.

/// Dark modules: index 16 of the 256-colour cube, true black, which terminal themes
/// leave alone (unlike the 16 named colours).
pub const DARK: u8 = 16;
/// Light modules: index 231, true white.
pub const LIGHT: u8 = 231;

/// Light modules around the code, as the QR spec asks.
const QUIET_ZONE: usize = 4;

/// The QR code of `short_url` in Unicode half blocks, one string per row: two modules
/// per cell (top and bottom), every module drawn, with the quiet zone. Meant to be
/// drawn in [`DARK`] on [`LIGHT`], whatever the terminal's own colours.
pub fn half_blocks(short_url: &str) -> Vec<String> {
    let qr = qrcode::QrCode::new(short_url).expect("a Short URL fits in a QR code");
    let width = qr.width();
    let colors = qr.to_colors();
    let size = width + 2 * QUIET_ZONE;
    let dark = |x: usize, y: usize| {
        let (Some(x), Some(y)) = (x.checked_sub(QUIET_ZONE), y.checked_sub(QUIET_ZONE)) else {
            return false;
        };
        x < width && y < width && colors[y * width + x] == qrcode::Color::Dark
    };
    (0..size)
        .step_by(2)
        .map(|y| {
            (0..size)
                .map(|x| match (dark(x, y), dark(x, y + 1)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}
