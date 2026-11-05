//! The full-screen TUI `crablink` opens with no subcommand. [`App`] is backend-free
//! (key events in, [`App::render`] onto any `Frame`); only [`run`] touches the real terminal.

use chrono::{DateTime, Utc};
use crablink_api::Link;
use ratatui::Frame;
use ratatui::crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Row, Table, TableState, Wrap};

use crate::Api;

const CRAB_RED: Color = Color::Rgb(0xE0, 0x41, 0x2A);

/// Every key binding, shown in the sidebar and the `?` help.
const KEYS: &[(&str, &str)] = &[
    ("jk", "move ↑↓"),
    ("/", "filter"),
    ("r", "refresh"),
    ("?", "help"),
    ("q", "quit (Esc)"),
];

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
    /// The selected row of the visible Links; clamped to them by [`App::selected`].
    table: TableState,
    /// The `/` filter; empty shows every Link.
    filter: String,
    mode: Mode,
    quit: bool,
}

/// Where key presses go.
#[derive(PartialEq)]
enum Mode {
    /// Moving through the list.
    List,
    /// Typing the `/` filter.
    Filter,
    /// The `?` help over the list; any key closes it.
    Help,
}

impl App {
    /// Builds the app and loads every Link from the server (all pages).
    pub fn new(api: Api, caps: Caps) -> App {
        let mut app = App {
            api,
            caps,
            links: Ok(Vec::new()),
            table: TableState::default().with_selected(0),
            filter: String::new(),
            mode: Mode::List,
            quit: false,
        };
        app.load();
        app
    }

    /// (Re)loads every Link from the server, keeping the selected Link selected.
    fn load(&mut self) {
        let selected = self.selected().map(|l| l.code.clone());
        let mut links = Vec::new();
        self.links = match self.api.list(|page| links.extend(page)) {
            Ok(()) => Ok(links),
            Err(e) => Err(format!("{e:#}")),
        };
        if let Some(code) = selected {
            self.select_code(&code);
        }
    }

    /// Selects the visible Link with this Code; leaves the selection alone if it's not visible.
    fn select_code(&mut self, code: &str) {
        if let Some(i) = self.visible().iter().position(|l| l.code == code) {
            self.table.select(Some(i));
        }
    }

    /// Whether the operator has asked to leave.
    pub fn quit(&self) -> bool {
        self.quit
    }

    /// The Links the list shows: every Link matching the filter, newest first.
    fn visible(&self) -> Vec<&Link> {
        matching(self.links.as_deref().unwrap_or_default(), &self.filter)
    }

    /// The selected Link, if the list has any.
    fn selected(&self) -> Option<&Link> {
        let links = self.visible();
        let i = self.table.selected()?.min(links.len().checked_sub(1)?);
        links.get(i).copied()
    }

    /// Moves the selection by `by` rows, clamped to the list.
    fn move_selection(&mut self, by: isize) {
        let last = self.visible().len().saturating_sub(1);
        let i = self.table.selected().unwrap_or(0).min(last);
        self.table
            .select(Some(i.saturating_add_signed(by).min(last)));
    }

    pub fn handle_key(&mut self, k: KeyEvent) {
        match (&self.mode, k.code) {
            (Mode::Help, _) => self.mode = Mode::List,
            (_, KeyCode::Down) => self.move_selection(1),
            (_, KeyCode::Up) => self.move_selection(-1),
            (Mode::List, KeyCode::Char('q') | KeyCode::Esc) => self.quit = true,
            (Mode::List, KeyCode::Char('j')) => self.move_selection(1),
            (Mode::List, KeyCode::Char('k')) => self.move_selection(-1),
            (Mode::List, KeyCode::Char('/')) => self.mode = Mode::Filter,
            (Mode::List, KeyCode::Char('r')) => self.load(),
            (Mode::List, KeyCode::Char('?')) => self.mode = Mode::Help,
            (Mode::Filter, KeyCode::Enter) => self.mode = Mode::List,
            (Mode::Filter, KeyCode::Esc) => {
                self.mode = Mode::List;
                self.set_filter(String::new());
            }
            (Mode::Filter, KeyCode::Backspace) => {
                let mut f = self.filter.clone();
                f.pop();
                self.set_filter(f);
            }
            (Mode::Filter, KeyCode::Char(c)) => self.set_filter(format!("{}{c}", self.filter)),
            _ => {}
        }
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.table.select(Some(0));
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
            info.extend(key_hints());
            // Inside the right border, with one column of padding.
            let side = Rect {
                width: side.width - 1,
                ..side
            };
            f.render_widget(Paragraph::new(info), pad(side, 1));
            main
        };
        let [list, detail] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(8)]).areas(main);
        let mut list = pad(list, 1);
        if self.mode == Mode::Filter || !self.filter.is_empty() {
            let [line, rest] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(list);
            f.render_widget(Line::from(format!("/{}", self.filter)), line);
            if self.mode == Mode::Filter {
                f.set_cursor_position((line.x + 1 + self.filter.chars().count() as u16, line.y));
            }
            list = rest;
        }
        let now = Utc::now();
        match &self.links {
            Err(e) => f.render_widget(
                Paragraph::new(e.as_str())
                    .fg(CRAB_RED)
                    .wrap(Wrap { trim: false }),
                list,
            ),
            Ok(links) if links.is_empty() => {
                f.render_widget(Line::from("press n to create your first Link").dim(), list)
            }
            Ok(links) => match matching(links, &self.filter) {
                m if m.is_empty() => f.render_widget(Line::from("no Links match").dim(), list),
                m => f.render_stateful_widget(link_table(&m, now), list, &mut self.table),
            },
        }
        if let Some(l) = self.selected() {
            render_detail(f, detail, l, now);
        }
        if self.mode == Mode::Help {
            render_help(f, main);
        }
    }
}

fn key_hints<'a>() -> impl Iterator<Item = Line<'a>> {
    KEYS.iter().map(|(k, what)| {
        Line::from(vec![
            Span::from(format!("{k:>2} ")).fg(CRAB_RED).bold(),
            Span::from(*what).dim(),
        ])
    })
}

/// The `?` panel, centred over `area`.
fn render_help(f: &mut Frame, area: Rect) {
    let mut text = vec![
        Line::from(concat!("crablink ", env!("CARGO_PKG_VERSION"))).bold(),
        Line::from(""),
    ];
    text.extend(key_hints());
    text.extend([
        Line::from(""),
        Line::from("filtering: Enter keeps, Esc clears").dim(),
    ]);
    let area = area.centered(
        Constraint::Length(40),
        Constraint::Length(text.len() as u16 + 2),
    );
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(text).block(
            Block::bordered()
                .title(" help ")
                .dim()
                .padding(Padding::horizontal(1)),
        ),
        area,
    );
}

/// The selected Link in full, under the list.
fn render_detail(f: &mut Frame, area: Rect, l: &Link, now: DateTime<Utc>) {
    let field =
        |k: &str, v: String| Line::from(vec![Span::from(format!("{k:<12}")).dim(), Span::from(v)]);
    let text = vec![
        field("Short URL", l.short_url.clone()).bold(),
        field("Target", l.target.clone()),
        field("Click Count", l.click_count.to_string()),
        field("Expiry", crate::expiry(l, now)),
        field("Created", crate::timestamp(l.created_at)),
    ];
    let block = Block::new().borders(Borders::TOP).dim();
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }),
        pad(inner, 1),
    );
}

/// The Links whose Code or Target contains `filter`, ignoring case.
fn matching<'a>(links: &'a [Link], filter: &str) -> Vec<&'a Link> {
    let filter = filter.to_lowercase();
    links
        .iter()
        .filter(|l| {
            l.code.to_lowercase().contains(&filter) || l.target.to_lowercase().contains(&filter)
        })
        .collect()
}

fn link_table<'a>(links: &[&'a Link], now: DateTime<Utc>) -> Table<'a> {
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
    Table::new(rows, widths)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(Style::new().reversed())
        .highlight_symbol("▌")
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
