//! The full-screen TUI `crablink` opens with no subcommand. [`App`] is backend-free
//! (key events in, [`App::render`] onto any `Frame`); only [`run`] touches the real terminal.

use chrono::{DateTime, Utc};
use crablink_api::Link;
use ratatui::Frame;
use ratatui::crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, Wrap};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Capability, Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FontSize, StatefulImage};

use crate::Api;

const CRAB_RED: Color = Color::Rgb(0xE0, 0x41, 0x2A);

/// Key hints shown in the sidebar.
const KEYS: &[(&str, &str)] = &[("q", "quit")];

/// Which stacked lockup suits the terminal background (logo/GUIDELINES.md §7).
#[derive(Debug, PartialEq, Eq)]
pub enum Lockup {
    /// Shell Ink wordmark, for light backgrounds.
    Standard,
    /// White wordmark, for dark or unknown backgrounds.
    Reversed,
}

/// Light (perceived luminance above 50%) gets the standard lockup; dark, or no
/// answer to the background query, gets the reversed one.
pub fn lockup(bg: Option<(u8, u8, u8)>) -> Lockup {
    match bg {
        Some((r, g, b)) if 299 * r as u32 + 587 * g as u32 + 114 * b as u32 > 128_000 => {
            Lockup::Standard
        }
        _ => Lockup::Reversed,
    }
}

/// What the terminal can do, detected before the TUI starts. The default is a terminal
/// with no image support, which is what tests use.
#[derive(Default)]
pub struct Caps {
    logo: Option<Logo>,
}

/// The stacked lockup, ready to draw through the terminal's image protocol.
struct Logo {
    img: StatefulProtocol,
    /// Image size in pixels.
    px: (u32, u32),
    /// Cell size in pixels.
    font: FontSize,
}

impl Caps {
    /// Asks the terminal for its image protocol, cell size, and background (OSC 11).
    /// Writes to stdout and reads stdin, so call it only on a terminal, before the
    /// alternate screen.
    pub fn detect() -> Caps {
        let picker = Picker::from_query_stdio_with_options(QueryStdioOptions {
            terminal_background_color_osc: true,
            ..Default::default()
        })
        .unwrap_or_else(|_| Picker::halfblocks());
        Caps::from_picker(&picker)
    }

    /// Halfblocks means no image protocol: no logo, never a character approximation.
    pub fn from_picker(picker: &Picker) -> Caps {
        if picker.protocol_type() == ProtocolType::Halfblocks {
            return Caps::default();
        }
        let bg = picker.capabilities().iter().find_map(|c| match *c {
            Capability::Background(r, g, b) => Some((r, g, b)),
            _ => None,
        });
        let png: &[u8] = match lockup(bg) {
            Lockup::Standard => {
                include_bytes!("../../../logo/export/stacked/crablink-stacked-800.png")
            }
            Lockup::Reversed => {
                include_bytes!("../../../logo/export/stacked/crablink-stacked-reversed-800.png")
            }
        };
        let img = image::load_from_memory(png).expect("bundled PNG decodes");
        Caps {
            logo: Some(Logo {
                px: (img.width(), img.height()),
                font: picker.font_size(),
                img: picker.new_resize_protocol(img),
            }),
        }
    }
}

/// Logo width in columns, centred in the sidebar.
const LOGO_COLS: u16 = 20;

/// Draws the logo at the top of `side` (one row of margin, two of clear space below)
/// and returns the area left under it; all of `side` when there's no logo.
fn sidebar_logo(f: &mut Frame, side: Rect, logo: Option<&mut Logo>) -> Rect {
    let Some(logo) = logo else { return side };
    let cols = LOGO_COLS.min(side.width);
    let (w, h) = logo.px;
    let (fw, fh) = (logo.font.width as u32, logo.font.height as u32);
    let rows = (cols as u32 * fw * h).div_ceil(w * fh) as u16;
    let rows = rows.min(side.height.saturating_sub(1));
    let r = Rect {
        x: side.x + (side.width - cols) / 2,
        y: side.y + side.height.min(1),
        width: cols,
        height: rows,
    };
    f.render_stateful_widget(StatefulImage::default(), r, &mut logo.img);
    let top = (r.bottom() + 2).min(side.bottom());
    Rect {
        y: top,
        height: side.bottom() - top,
        ..side
    }
}

pub struct App {
    api: Api,
    caps: Caps,
    /// Every Link, newest first, or the server's error message.
    links: Result<Vec<Link>, String>,
    quit: bool,
}

impl App {
    /// Builds the app and loads every Link from the server (all pages).
    pub fn new(api: Api, caps: Caps) -> App {
        let mut app = App {
            api,
            caps,
            links: Ok(Vec::new()),
            quit: false,
        };
        app.load();
        app
    }

    fn load(&mut self) {
        let mut links = Vec::new();
        self.links = match self.api.list(|page| links.extend(page)) {
            Ok(()) => Ok(links),
            Err(e) => Err(format!("{e:#}")),
        };
    }

    /// Whether the operator has asked to leave.
    pub fn quit(&self) -> bool {
        self.quit
    }

    pub fn handle_key(&mut self, k: KeyEvent) {
        if let KeyCode::Char('q') | KeyCode::Esc = k.code {
            self.quit = true;
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let host = self.api.server().split("://").last().unwrap_or_default();
        let total = match &self.links {
            Ok(l) if l.len() == 1 => "1 Link".to_string(),
            Ok(l) => format!("{} Links", l.len()),
            Err(_) => "? Links".into(),
        };
        let main = if area.width < 80 {
            let [header, main] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
            f.render_widget(Line::from(format!("{host} · {total}")).dim(), header);
            main
        } else {
            let [side, main] =
                Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(area);
            f.render_widget(Block::new().borders(Borders::RIGHT).dim(), side);
            let mut info = vec![
                Line::from(host).bold(),
                Line::from(total).dim(),
                Line::from(""),
            ];
            info.extend(KEYS.iter().map(|(k, what)| {
                Line::from(vec![
                    Span::from(format!("{k:>2} ")).fg(CRAB_RED).bold(),
                    Span::from(*what).dim(),
                ])
            }));
            // Inside the right border, with one column of padding.
            let side = Rect {
                width: side.width - 1,
                ..side
            };
            let below = sidebar_logo(f, side, self.caps.logo.as_mut());
            f.render_widget(Paragraph::new(info), pad(below, 1));
            main
        };
        let main = pad(main, 1);
        match &self.links {
            Err(e) => f.render_widget(
                Paragraph::new(e.as_str())
                    .fg(CRAB_RED)
                    .wrap(Wrap { trim: false }),
                main,
            ),
            Ok(links) if links.is_empty() => {
                f.render_widget(Line::from("press n to create your first Link").dim(), main)
            }
            Ok(links) => f.render_widget(link_table(links, Utc::now()), main),
        }
    }
}

fn link_table(links: &[Link], now: DateTime<Utc>) -> Table<'_> {
    let rows = links.iter().map(|l| {
        let expiry = match l.expires_at {
            None => Span::from("never").dim(),
            Some(t) if t <= now => Span::from("expired").fg(CRAB_RED),
            Some(t) => Span::from(t.format("%Y-%m-%d %H:%M").to_string()).dim(),
        };
        Row::new(vec![
            Span::from(l.code.as_str()).bold(),
            Span::from(format!("{:>6}", l.click_count)),
            expiry,
            Span::from(l.target.as_str()).dim(),
        ])
    });
    let header = Row::new(["Code", "Clicks", "Expiry", "Target"])
        .style(Style::new().underlined().dim())
        .bottom_margin(1);
    let widths = [
        Constraint::Length(13),
        Constraint::Length(6),
        Constraint::Length(16),
        Constraint::Min(10),
    ];
    Table::new(rows, widths).header(header).column_spacing(2)
}

/// `r` shrunk by `n` columns on each side.
fn pad(r: Rect, n: u16) -> Rect {
    Rect {
        x: r.x + n,
        width: r.width.saturating_sub(n * 2),
        ..r
    }
}

/// Runs the TUI on the real terminal until the operator quits. Raw mode and the
/// alternate screen are restored on exit, error, and (through ratatui's hook) panic.
pub fn run(api: Api, caps: Caps) -> anyhow::Result<()> {
    let mut app = App::new(api, caps);
    let mut term = ratatui::try_init()?;
    let res = (|| -> std::io::Result<()> {
        while !app.quit() {
            term.draw(|f| app.render(f))?;
            if let Some(k) = event::read()?.as_key_press_event() {
                app.handle_key(k);
            }
        }
        Ok(())
    })();
    ratatui::restore();
    Ok(res?)
}
