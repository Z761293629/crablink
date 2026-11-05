//! The full-screen TUI `crablink` opens with no subcommand. [`App`] is backend-free
//! (key events in, [`App::render`] onto any `Frame`); only [`run`] touches the real terminal.

use chrono::{DateTime, Utc};
use crablink_api::{CreateLink, Link, UpdateLink};
use ratatui::Frame;
use ratatui::crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Row, Table, TableState, Wrap};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Capability, Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FontSize, StatefulImage};

use crate::Api;

const CRAB_RED: Color = Color::Rgb(0xE0, 0x41, 0x2A);

/// Every key binding, shown in the sidebar and the `?` help.
const KEYS: &[(&str, &str)] = &[
    ("jk", "move ↑↓"),
    ("/", "filter"),
    ("n", "new"),
    ("e", "edit"),
    ("r", "refresh"),
    ("d", "delete"),
    ("?", "help"),
    ("q", "quit (Esc)"),
];

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
    /// The selected row of the visible Links; clamped to them by [`App::selected`].
    table: TableState,
    /// The `/` filter; empty shows every Link.
    filter: String,
    mode: Mode,
    quit: bool,
}

/// Where key presses go.
enum Mode {
    /// Moving through the list.
    List,
    /// Typing the `/` filter.
    Filter,
    /// The `?` help over the list; any key closes it.
    Help,
    /// The Link form over the list.
    Form(Form),
    /// Asking to delete the selected Link; only `y` confirms.
    Delete,
}

/// The Link form: Target, Code, Expiry. `n` opens it empty to create a Link; `e`
/// pre-filled to edit the selected one. Errors
/// (the server's, or a bad Expiry) are shown in it and leave the input as typed.
#[derive(Default)]
struct Form {
    /// Target, Code, Expiry, as typed.
    values: [String; 3],
    /// Index into `values` of the field being typed in.
    focus: usize,
    /// Editing an existing Link: the Code is shown, but can't be focused or changed.
    edit: bool,
    error: Option<String>,
}

/// The Code's index in [`Form::values`].
const CODE: usize = 1;

impl Form {
    /// Moves the focus `by` fields forward, wrapping, skipping a read-only Code.
    fn move_focus(&mut self, by: usize) {
        loop {
            self.focus = (self.focus + by) % self.values.len();
            if !(self.edit && self.focus == CODE) {
                return;
            }
        }
    }

    /// The form for editing `l`, its Expiry as an RFC 3339 timestamp.
    fn edit(l: &Link) -> Form {
        Form {
            values: [
                l.target.clone(),
                l.code.clone(),
                l.expires_at.map(crate::timestamp).unwrap_or_default(),
            ],
            edit: true,
            ..Form::default()
        }
    }

    /// Creates or updates the Link as typed; returns its Code. An empty Code means a
    /// generated one; an empty Expiry means none (clearing it when editing).
    fn save(&self, api: &Api) -> anyhow::Result<String> {
        let [target, code, expires] = &self.values;
        let expires_at = (!expires.is_empty())
            .then(|| crate::parse_expires(expires, Utc::now()))
            .transpose()?;
        let link = if self.edit {
            api.update(
                code,
                &UpdateLink {
                    target: Some(target.clone()),
                    expires_at: Some(expires_at),
                },
            )?
        } else {
            api.create(&CreateLink {
                target: target.clone(),
                code: (!code.is_empty()).then(|| code.clone()),
                expires_at,
            })?
        };
        Ok(link.code)
    }
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
        // Before the match, which borrows the mode mutably for the form's arms.
        let has_selection = self.selected().is_some();
        match (&mut self.mode, k.code) {
            (Mode::Help, _) => self.mode = Mode::List,
            (Mode::Form(_), KeyCode::Esc) => self.mode = Mode::List,
            (Mode::Form(_), KeyCode::Enter) => self.submit(),
            (Mode::Form(f), KeyCode::Tab | KeyCode::Down) => f.move_focus(1),
            (Mode::Form(f), KeyCode::BackTab | KeyCode::Up) => f.move_focus(f.values.len() - 1),
            (Mode::Form(f), KeyCode::Backspace) => drop(f.values[f.focus].pop()),
            (Mode::Form(f), KeyCode::Char(c)) => f.values[f.focus].push(c),
            (Mode::Form(_), _) => {}
            (Mode::Delete, KeyCode::Char('y')) => {
                self.mode = Mode::List;
                self.delete_selected();
            }
            (Mode::Delete, _) => self.mode = Mode::List,
            (_, KeyCode::Down) => self.move_selection(1),
            (_, KeyCode::Up) => self.move_selection(-1),
            (Mode::List, KeyCode::Char('q') | KeyCode::Esc) => self.quit = true,
            (Mode::List, KeyCode::Char('j')) => self.move_selection(1),
            (Mode::List, KeyCode::Char('k')) => self.move_selection(-1),
            (Mode::List, KeyCode::Char('/')) => self.mode = Mode::Filter,
            (Mode::List, KeyCode::Char('n')) => self.mode = Mode::Form(Form::default()),
            (Mode::List, KeyCode::Char('r')) => self.load(),
            (Mode::List, KeyCode::Char('?')) => self.mode = Mode::Help,
            (Mode::List, KeyCode::Char('e')) => {
                if let Some(form) = self.selected().map(Form::edit) {
                    self.mode = Mode::Form(form);
                }
            }
            (Mode::List, KeyCode::Char('d')) if has_selection => self.mode = Mode::Delete,
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

    /// Saves the Link in the form and selects it, or shows the error in the form.
    fn submit(&mut self) {
        let Mode::Form(form) = &mut self.mode else {
            return;
        };
        match form.save(&self.api) {
            Ok(code) => {
                self.mode = Mode::List;
                // The saved Link may not match the filter, and only a visible Link can be selected.
                self.filter.clear();
                self.load();
                self.select_code(&code);
            }
            Err(e) => form.error = Some(format!("{e:#}")),
        }
    }

    /// Deletes the selected Link and reloads; the selection keeps its row, so it lands
    /// on a neighbour.
    fn delete_selected(&mut self) {
        let Some(code) = self.selected().map(|l| l.code.clone()) else {
            return;
        };
        match self.api.delete(&code) {
            Ok(()) => self.load(),
            Err(e) => self.links = Err(format!("{e:#}")),
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
            let below = sidebar_logo(f, side, self.caps.logo.as_mut());
            f.render_widget(Paragraph::new(info), pad(below, 1));
            main
        };
        let [list, detail] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(8)]).areas(main);
        let mut list = pad(list, 1);
        let filtering = matches!(self.mode, Mode::Filter);
        if filtering || !self.filter.is_empty() {
            let [line, rest] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(list);
            f.render_widget(Line::from(format!("/{}", self.filter)), line);
            if filtering {
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
        match &self.mode {
            Mode::Help => render_help(f, main),
            Mode::Form(form) => render_form(f, main, form),
            Mode::Delete => {
                if let Some(l) = self.selected() {
                    render_delete(f, main, &l.code);
                }
            }
            Mode::List | Mode::Filter => {}
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

/// The Link form, centred over `area`, with the cursor in the focused field.
fn render_form(f: &mut Frame, area: Rect, form: &Form) {
    const LABEL: u16 = 13;
    let code = if form.edit { "Code" } else { "Custom Code" };
    let fields = [
        ("Target", ""),
        (code, "generated"),
        ("Expiry", "never, or 30m, 12h, 7d, RFC 3339"),
    ];
    let area = area.centered(Constraint::Length(64), Constraint::Length(8));
    let block = Block::bordered()
        .title(if form.edit {
            " edit Link "
        } else {
            " new Link "
        })
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    // Columns left for a value after its label, keeping one for the cursor.
    let room = inner.width.saturating_sub(LABEL + 1) as usize;
    let mut text: Vec<Line> = fields
        .iter()
        .zip(&form.values)
        .enumerate()
        .map(|(i, ((label, placeholder), v))| {
            let value = match v.as_str() {
                "" => Span::from(*placeholder).dim(),
                v if form.edit && i == CODE => Span::from(v).dim(),
                v => Span::from(tail(v, room)),
            };
            Line::from(vec![Span::from(format!("{label:<13}")).dim(), value])
        })
        .collect();
    text.push(Line::from(""));
    text.push(Line::from(form.error.as_deref().unwrap_or_default()).fg(CRAB_RED));
    text.push(Line::from("Tab next field · Enter save · Esc cancel").dim());
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(block), area);
    let typed = tail(&form.values[form.focus], room).chars().count() as u16;
    f.set_cursor_position((inner.x + LABEL + typed, inner.y + form.focus as u16));
}

/// The last `n` characters of `s`, so the end of a long value stays in view while typing.
fn tail(s: &str, n: usize) -> &str {
    let skip = s.chars().count().saturating_sub(n);
    s.char_indices().nth(skip).map_or("", |(i, _)| &s[i..])
}

/// The delete confirmation, centred over `area`.
fn render_delete(f: &mut Frame, area: Rect, code: &str) {
    let text = vec![
        Line::from(vec![
            Span::from("Delete "),
            Span::from(code).bold(),
            Span::from("?"),
        ]),
        Line::from("Its Code will never be reused."),
        Line::from(""),
        Line::from(vec![
            Span::from("y").fg(CRAB_RED).bold(),
            Span::from(" deletes, any other key cancels").dim(),
        ]),
    ];
    // Wide enough for a 64-character Custom Code: "Delete " + Code + "?" + border and padding.
    let width = (code.chars().count() as u16 + 12).max(40);
    let area = area.centered(
        Constraint::Length(width),
        Constraint::Length(text.len() as u16 + 2),
    );
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(text).block(
            Block::bordered()
                .title(" delete ")
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
