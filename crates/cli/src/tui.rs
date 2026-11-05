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

use crate::Api;

const CRAB_RED: Color = Color::Rgb(0xE0, 0x41, 0x2A);

/// Key hints shown in the sidebar.
const KEYS: &[(&str, &str)] = &[("q", "quit")];

/// What the terminal can do, detected before the TUI starts. The default is a terminal
/// with no image support, which is what tests use.
#[derive(Default)]
pub struct Caps {}

pub struct App {
    api: Api,
    #[allow(dead_code)] // read by the sidebar logo (#29)
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
            f.render_widget(Paragraph::new(info), pad(side, 1));
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
