//! The full-screen TUI `crablink` opens with no subcommand. [`App`] is backend-free
//! (key events in, [`App::render`] onto any `Frame`); only [`run`] touches the real terminal.

use std::time::Duration;

use anyhow::Context;
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
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
    lockup: Lockup,
    /// Image size in pixels.
    px: (u32, u32),
    /// Cell size in pixels.
    font: FontSize,
    /// Kept to re-encode the image when the background changes.
    picker: Picker,
    /// The terminal answered the background query at startup, so it's worth asking
    /// again while the TUI runs.
    follow: bool,
}

impl Logo {
    fn new(picker: &Picker, lockup: Lockup, follow: bool) -> Logo {
        let png: &[u8] = match lockup {
            Lockup::Standard => {
                include_bytes!("../../../logo/export/stacked/crablink-stacked-800.png")
            }
            Lockup::Reversed => {
                include_bytes!("../../../logo/export/stacked/crablink-stacked-reversed-800.png")
            }
        };
        let img = image::load_from_memory(png).expect("bundled PNG decodes");
        Logo {
            px: (img.width(), img.height()),
            font: picker.font_size(),
            img: picker.new_resize_protocol(img),
            lockup,
            picker: picker.clone(),
            follow,
        }
    }
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
        // ponytail: no re-query inside tmux, it needs passthrough-wrapped escapes.
        let follow = bg.is_some() && !picker.tmux_detected();
        Caps {
            logo: Some(Logo::new(picker, lockup(bg), follow)),
        }
    }

    /// The lockup the sidebar shows, `None` when there's no logo.
    pub fn lockup(&self) -> Option<Lockup> {
        self.logo.as_ref().map(|l| l.lockup)
    }

    /// Switches the lockup when the terminal background has changed sides.
    pub fn set_background(&mut self, bg: Option<(u8, u8, u8)>) {
        if let Some(logo) = &mut self.logo
            && logo.lockup != lockup(bg)
        {
            *logo = Logo::new(&logo.picker, lockup(bg), logo.follow);
        }
    }
}

/// Reads a reply to `ESC ] 11 ; ? ESC \ ESC [ 5 n` from `bytes`: the background, if the
/// terminal answered OSC 11, once the status report (`ESC [ 0 n`) that ends the reply
/// arrives. `None` if `bytes` runs out first. Stops right after the status report.
fn background_reply(bytes: impl IntoIterator<Item = u8>) -> Option<Option<(u8, u8, u8)>> {
    use ratatui_image::picker::cap_parser::{Parser, Response};
    let mut parser = Parser::new();
    let mut bg = None;
    for b in bytes {
        for r in parser.push(b as char) {
            match r {
                Response::Background(r, g, b) => bg = Some((r, g, b)),
                Response::Status => return Some(bg),
                _ => {}
            }
        }
    }
    None
}

/// Asks the terminal for its background again, mid-TUI. `None` when it didn't answer
/// within the timeout.
#[cfg(unix)]
fn query_background() -> Option<Option<(u8, u8, u8)>> {
    use rustix::event::{PollFd, PollFlags, poll};
    use std::io::Write;
    use std::time::Instant;

    let mut out = std::io::stdout();
    out.write_all(b"\x1b]11;?\x1b\\\x1b[5n").ok()?;
    out.flush().ok()?;
    let deadline = Instant::now() + Duration::from_secs(1);
    let stdin = rustix::stdio::stdin();
    // One byte at a time, unbuffered, so nothing after the reply is taken from crossterm.
    // ponytail: a key pressed in the ~1 ms the reply takes is swallowed.
    background_reply(std::iter::from_fn(|| {
        let left = deadline.checked_duration_since(Instant::now())?;
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        if poll(&mut fds, left.as_millis() as i32).ok()? == 0 {
            return None;
        }
        let mut b = [0u8];
        (rustix::io::read(stdin, &mut b).ok()? == 1).then_some(b[0])
    }))
}

#[cfg(not(unix))]
fn query_background() -> Option<Option<(u8, u8, u8)>> {
    None
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
    /// The last failed action's error, shown under the list until the next key.
    error: Option<String>,
    /// `r` was pressed: [`run`] asks the terminal for its background again.
    redetect: bool,
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
    target: String,
    code: String,
    expires: String,
    /// The Expiry as pre-filled for editing. Saved unchanged, it isn't sent, so the
    /// server keeps its exact (possibly sub-second) value.
    prefilled_expires: String,
    /// The field being typed in.
    focus: Field,
    /// Editing an existing Link: the Code is shown, but can't be focused or changed.
    editing: bool,
    error: Option<String>,
}

/// The form's fields, top to bottom.
#[derive(Default, Clone, Copy, PartialEq)]
enum Field {
    #[default]
    Target,
    Code,
    Expiry,
}

impl Form {
    /// Moves the focus one field forward (or back), wrapping, skipping a read-only Code.
    fn move_focus(&mut self, forward: bool) {
        use Field::*;
        loop {
            self.focus = match (self.focus, forward) {
                (Target, true) | (Expiry, false) => Code,
                (Code, true) | (Target, false) => Expiry,
                (Expiry, true) | (Code, false) => Target,
            };
            if !(self.editing && self.focus == Code) {
                return;
            }
        }
    }

    /// The focused field's value.
    fn value_mut(&mut self) -> &mut String {
        match self.focus {
            Field::Target => &mut self.target,
            Field::Code => &mut self.code,
            Field::Expiry => &mut self.expires,
        }
    }

    /// The form for editing `l`, its Expiry as an RFC 3339 timestamp.
    fn edit(l: &Link) -> Form {
        let expires = l.expires_at.map(crate::timestamp).unwrap_or_default();
        Form {
            target: l.target.clone(),
            code: l.code.clone(),
            prefilled_expires: expires.clone(),
            expires,
            editing: true,
            ..Form::default()
        }
    }

    /// Creates or updates the Link as typed; returns its Code. An empty Code means a
    /// generated one; an empty Expiry means none (clearing it when editing).
    fn save(&self, api: &Api) -> anyhow::Result<String> {
        let expires_at = (!self.expires.is_empty())
            .then(|| {
                crate::parse_expires(&self.expires, Utc::now())
                    .with_context(|| format!("invalid Expiry {:?}", self.expires))
            })
            .transpose()?;
        let link = if self.editing {
            api.update(
                &self.code,
                &UpdateLink {
                    target: Some(self.target.clone()),
                    expires_at: (self.expires != self.prefilled_expires).then_some(expires_at),
                },
            )?
        } else {
            api.create(&CreateLink {
                target: self.target.clone(),
                code: (!self.code.is_empty()).then(|| self.code.clone()),
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
            error: None,
            redetect: false,
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
        self.error = None;
        match (&mut self.mode, k.code) {
            (Mode::Help, _) => self.mode = Mode::List,
            (Mode::Form(_), KeyCode::Esc) => self.mode = Mode::List,
            (Mode::Form(_), KeyCode::Enter) => self.submit(),
            (Mode::Form(f), KeyCode::Tab | KeyCode::Down) => f.move_focus(true),
            (Mode::Form(f), KeyCode::BackTab | KeyCode::Up) => f.move_focus(false),
            (Mode::Form(f), KeyCode::Backspace) => drop(f.value_mut().pop()),
            (Mode::Form(f), KeyCode::Char(c)) => f.value_mut().push(c),
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
            (Mode::List, KeyCode::Char('r')) => {
                self.load();
                self.redetect = true;
            }
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
            Err(e) => self.error = Some(format!("{e:#}")),
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
        if let Some(e) = &self.error {
            let [rest, line] =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(list);
            f.render_widget(Line::from(e.as_str()).fg(CRAB_RED), line);
            list = rest;
        }
        let now = Utc::now();
        let visible = self.visible();
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
            Ok(_) if visible.is_empty() => {
                f.render_widget(Line::from("no Links match").dim(), list)
            }
            Ok(_) => {
                let table = link_table(&visible, now, list.width);
                f.render_stateful_widget(table, list, &mut self.table);
            }
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
    let area = area.centered(Constraint::Length(64), Constraint::Length(8));
    let block = Block::bordered()
        .title(if form.editing {
            " edit Link "
        } else {
            " new Link "
        })
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    // Columns left for a value after its label, keeping one for the cursor.
    let room = inner.width.saturating_sub(LABEL as u16 + 1) as usize;
    let row = |label: &str, placeholder: &'static str, v: &str, read_only: bool| {
        let value = match v {
            "" => Span::from(placeholder).dim(),
            v if read_only => Span::from(v.to_owned()).dim(),
            v => Span::from(tail(v, room).to_owned()),
        };
        Line::from(vec![labelled(label), value])
    };
    let mut text = vec![
        row("Target", "", &form.target, false),
        if form.editing {
            row("Code", "", &form.code, true)
        } else {
            row("Custom Code", "generated", &form.code, false)
        },
        row(
            "Expiry",
            "never, or 30m, 12h, 7d, RFC 3339",
            &form.expires,
            false,
        ),
    ];
    text.push(Line::from(""));
    text.push(Line::from(form.error.as_deref().unwrap_or_default()).fg(CRAB_RED));
    text.push(Line::from("Tab next field · Enter save · Esc cancel").dim());
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(block), area);
    let focused = match form.focus {
        Field::Target => &form.target,
        Field::Code => &form.code,
        Field::Expiry => &form.expires,
    };
    let typed = tail(focused, room).chars().count() as u16;
    f.set_cursor_position((inner.x + LABEL as u16 + typed, inner.y + form.focus as u16));
}

/// Width of the labels in the form and the detail pane.
const LABEL: usize = 13;

/// `label` padded to [`LABEL`] columns, dimmed.
fn labelled<'a>(label: &str) -> Span<'a> {
    Span::from(format!("{label:<LABEL$}")).dim()
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
    let field = |k: &str, v: String| Line::from(vec![labelled(k), Span::from(v)]);
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

/// The list, `width` columns wide. Code and Expiry are as wide as their longest visible
/// value (the Code bounded so the Target keeps 10 columns); the Target gets the rest and
/// is what gets truncated.
fn link_table(links: &[&Link], now: DateTime<Utc>, width: u16) -> Table<'static> {
    const HEADER: [&str; 4] = ["Code", "Click Count", "Expiry", "Target"];
    let expiries: Vec<String> = links.iter().map(|l| crate::expiry(l, now)).collect();
    let widest = |header: &str, values: Vec<&str>| {
        values
            .into_iter()
            .chain([header])
            .map(|v| v.chars().count())
            .max()
            .unwrap_or(0) as u16
    };
    let clicks = HEADER[1].len() as u16;
    let expiry = widest(HEADER[2], expiries.iter().map(String::as_str).collect());
    // Highlight symbol, three column gaps, and the Target's minimum.
    let room = width.saturating_sub(1 + 3 * 2 + clicks + expiry + 10);
    let code = widest(HEADER[0], links.iter().map(|l| l.code.as_str()).collect()).min(room);
    let rows = links.iter().zip(expiries).map(|(l, e)| {
        Row::new(vec![
            Line::from(l.code.clone()).bold(),
            Line::from(l.click_count.to_string()).right_aligned(),
            if crate::expired(l, now) {
                Line::from(e).fg(CRAB_RED)
            } else {
                Line::from(e).dim()
            },
            Line::from(l.target.clone()).dim(),
        ])
    });
    let header = Row::new(HEADER)
        .style(Style::new().underlined().dim())
        .bottom_margin(1);
    let widths = [
        Constraint::Length(code),
        Constraint::Length(clicks),
        Constraint::Length(expiry),
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
            // Idle for a second, or `r`: follow a terminal theme change. Only between
            // event reads, so crossterm isn't reading stdin at the same time.
            let idle = !event::poll(Duration::from_secs(1))?;
            if !idle && let Some(k) = event::read()?.as_key_press_event() {
                app.handle_key(k);
            }
            let asked = std::mem::take(&mut app.redetect);
            if (idle || asked) && app.caps.logo.as_ref().is_some_and(|l| l.follow) {
                match query_background() {
                    Some(bg) => app.caps.set_background(bg),
                    // No answer in time: a late one would reach crossterm as keys, so stop asking.
                    None => app.caps.logo.iter_mut().for_each(|l| l.follow = false),
                }
            }
        }
        Ok(())
    })();
    ratatui::restore();
    Ok(res?)
}

#[cfg(test)]
mod tests {
    use super::background_reply;

    #[test]
    fn reads_the_background_up_to_the_status_report() {
        let mut bytes = b"\x1b]11;rgb:ffff/f1f1/e7e7\x1b\\\x1b[0nj".iter().copied();
        assert_eq!(background_reply(&mut bytes), Some(Some((255, 241, 231))));
        // Stops at the status report: a key typed after it is left for crossterm.
        assert_eq!(bytes.next(), Some(b'j'));
    }

    #[test]
    fn a_bel_terminated_answer_counts_too() {
        let bytes = b"\x1b]11;rgb:0000/0000/0000\x07\x1b[0n".iter().copied();
        assert_eq!(background_reply(bytes), Some(Some((0, 0, 0))));
    }

    #[test]
    fn no_background_answer_before_the_status_report_is_unknown() {
        assert_eq!(background_reply(b"\x1b[0n".iter().copied()), Some(None));
    }

    #[test]
    fn no_status_report_means_no_reply() {
        let bytes = b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\".iter().copied();
        assert_eq!(background_reply(bytes), None);
    }
}
