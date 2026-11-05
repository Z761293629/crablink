//! PROTOTYPE — throwaway, do not merge to main.
//!
//! Question: with the logo pinned on the left, what should the `crablink` TUI layout be?
//! Three layout variants on fake in-memory data, switched with ← / → (or start with `-- B`).
//!
//!   cargo run -p crablink-cli --example tui_prototype [-- A|B|C]
//!
//! Run it in Ghostty / iTerm2 / kitty / WezTerm (image protocol). Light/dark lockup follows the terminal background.
//! Terminals without an image protocol show no logo at all (decision Q3″ b).

use std::io;

use image::DynamicImage;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Capability, Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;

const CRAB_RED: Color = Color::Rgb(0xE0, 0x41, 0x2A);
const SERVER: &str = "https://crab.link";

struct Link {
    code: &'static str,
    clicks: u64,
    expiry: &'static str,
    target: &'static str,
}

#[rustfmt::skip]
const LINKS: &[Link] = &[
    Link { code: "rust-meetup", clicks: 1283, expiry: "in 6 days", target: "https://rustconf.com/2027/meetups/shanghai" },
    Link { code: "aZ3k9Qx", clicks: 42, expiry: "never", target: "https://example.com/a/very/long/path/that/keeps/going?utm_source=poster" },
    Link { code: "hiring", clicks: 377, expiry: "never", target: "https://jobs.example.com/engineering/backend-rust" },
    Link { code: "Tq8vLm2", clicks: 0, expiry: "expired 2d ago", target: "https://docs.google.com/forms/d/e/1FAIpQLSf-example/viewform" },
    Link { code: "slides", clicks: 96, expiry: "in 3 hours", target: "https://talks.example.com/2026/crablink-intro.pdf" },
    Link { code: "q4-report", clicks: 12, expiry: "never", target: "https://drive.example.com/file/d/1x2y3z/view" },
    Link { code: "m9PzR4w", clicks: 5, expiry: "in 30 days", target: "https://github.com/Z761293629/crablink/issues/1" },
    Link { code: "wechat", clicks: 2048, expiry: "never", target: "https://mp.weixin.qq.com/s/AbCdEfGhIjKlMnOpQrSt" },
    Link { code: "xK2nB7c", clicks: 1, expiry: "never", target: "https://example.org/" },
    Link { code: "docs", clicks: 731, expiry: "never", target: "https://crablink.example.com/docs/getting-started" },
    Link { code: "launch", clicks: 15, expiry: "in 12 days", target: "https://www.producthunt.com/posts/crablink" },
    Link { code: "Ry5sW0e", clicks: 88, expiry: "never", target: "https://news.ycombinator.com/item?id=41234567" },
];

const VARIANTS: &[(&str, &str)] = &[
    ("A", "竖版侧栏 + 列表/详情上下分"),
    ("B", "图形窄栏 + 列表|详情左右分"),
    ("C", "侧栏即详情（logo 下方显示选中 Link）"),
];

struct App {
    variant: usize,
    table: TableState,
    picker: Picker,
    stacked: Option<StatefulProtocol>,
    symbol: Option<StatefulProtocol>,
    stacked_px: (u32, u32),
    note: String,
    bg: &'static str,
}

fn main() -> io::Result<()> {
    // Query protocol + font size before entering the alternate screen.
    let picker = Picker::from_query_stdio_with_options(QueryStdioOptions {
        terminal_background_color_osc: true,
        ..Default::default()
    })
    .unwrap_or_else(|_| Picker::halfblocks());
    // Q3b: pick the lockup by terminal background luminance (OSC 11); unknown → dark (reversed).
    let light = picker.capabilities().iter().any(|c| match *c {
        Capability::Background(r, g, b) => 299 * r as u32 + 587 * g as u32 + 114 * b as u32 > 128_000,
        _ => false,
    });
    let stacked_png: &[u8] = if light {
        include_bytes!("stacked.png")
    } else {
        include_bytes!("stacked-reversed.png")
    };
    let decode = |b: &[u8]| image::load_from_memory(b).expect("bundled png");
    let stacked_img: DynamicImage = decode(stacked_png);
    let stacked_px = (stacked_img.width(), stacked_img.height());
    // Q3″ b: no image protocol → no logo, never a halfblock approximation.
    let has_images = picker.protocol_type() != ProtocolType::Halfblocks;
    let start = std::env::args()
        .nth(1)
        .and_then(|a| VARIANTS.iter().position(|(k, _)| k.eq_ignore_ascii_case(&a)))
        .unwrap_or(0);
    let mut app = App {
        variant: start,
        table: TableState::default().with_selected(Some(0)),
        stacked: has_images.then(|| picker.new_resize_protocol(stacked_img)),
        symbol: has_images.then(|| picker.new_resize_protocol(decode(include_bytes!("symbol.png")))),
        picker,
        stacked_px,
        note: String::new(),
        bg: if light { "浅色背景 → stacked" } else { "深色/未知背景 → stacked-reversed" },
    };
    let terminal = ratatui::init();
    let res = run(terminal, &mut app);
    ratatui::restore();
    res
}

fn run(mut terminal: DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        app.note.clear();
        let n = VARIANTS.len();
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Right => app.variant = (app.variant + 1) % n,
            KeyCode::Left => app.variant = (app.variant + n - 1) % n,
            KeyCode::Down | KeyCode::Char('j') => app.table.select_next(),
            KeyCode::Up | KeyCode::Char('k') => app.table.select_previous(),
            KeyCode::Char(c @ ('n' | 'e' | 'd' | 'r' | '/' | '?')) => {
                app.note = format!("原型：`{c}` 未实现，只看布局");
            }
            _ => {}
        }
    }
}

fn draw(f: &mut Frame, app: &mut App) {
    let [main, bar] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(f.area());
    if main.width < 80 {
        narrow(f, main, app);
    } else {
        match app.variant {
            0 => variant_a(f, main, app),
            1 => variant_b(f, main, app),
            _ => variant_c(f, main, app),
        }
    }
    switcher(f, bar, app);
}

// ---------- Variant A: stacked sidebar (logo + server/count/keys) | list over detail ----------
fn variant_a(f: &mut Frame, area: Rect, app: &mut App) {
    let [side, right] = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(area);
    let below = stacked_logo(f, side, 20, app);
    let info = vec![
        Line::from(SERVER.trim_start_matches("https://")).bold(),
        Line::from(format!("{} Links", LINKS.len())).dim(),
        Line::from(""),
        key_line("n", "新建"),
        key_line("e", "编辑"),
        key_line("d", "删除"),
        key_line("/", "过滤"),
        key_line("r", "刷新"),
        key_line("?", "帮助"),
        key_line("q", "退出"),
    ];
    f.render_widget(Paragraph::new(info), pad(below, 2));
    let [list, detail_area] = Layout::vertical([Constraint::Min(0), Constraint::Length(8)]).areas(right);
    link_table(f, list, app, true);
    detail(f, detail_area, app, Borders::TOP | Borders::LEFT);
}

// ---------- Variant B: symbol-only rail | list beside detail, title line on top ----------
fn variant_b(f: &mut Frame, area: Rect, app: &mut App) {
    let [rail, right] = Layout::horizontal([Constraint::Length(12), Constraint::Min(0)]).areas(area);
    if let Some(sym) = app.symbol.as_mut() {
        let fs = app.picker.font_size();
        let cols = 8u16;
        let rows = (cols as u32 * fs.width as u32).div_ceil(fs.height as u32) as u16;
        let r = Rect { x: rail.x + 2, y: rail.y + 1, width: cols, height: rows.min(rail.height) };
        f.render_stateful_widget(StatefulImage::default(), r, sym);
    }
    let [title, body, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(right);
    f.render_widget(
        Line::from(vec![
            Span::from(SERVER.trim_start_matches("https://")).bold(),
            Span::from(format!(" · {} Links", LINKS.len())).dim(),
        ]),
        title,
    );
    let [list, detail_area] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(body);
    link_table(f, list, app, false);
    detail(f, detail_area, app, Borders::LEFT);
    f.render_widget(keys_inline(&app.note), keys);
}

// ---------- Variant C: sidebar = logo + inspector of the selected Link | full-height list ----------
fn variant_c(f: &mut Frame, area: Rect, app: &mut App) {
    let [side, right] = Layout::horizontal([Constraint::Length(36), Constraint::Min(0)]).areas(area);
    let below = stacked_logo(f, side, 22, app);
    detail(f, pad(below, 1), app, Borders::TOP);
    let [title, list, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(right);
    f.render_widget(
        Line::from(format!(" {} · {} Links", SERVER.trim_start_matches("https://"), LINKS.len())).dim(),
        title,
    );
    link_table(f, list, app, false);
    f.render_widget(keys_inline(&app.note), keys);
}

// ---------- < 80 columns: no sidebar at all (Q10) ----------
fn narrow(f: &mut Frame, area: Rect, app: &mut App) {
    let [title, list, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(area);
    f.render_widget(
        Line::from(format!("{} · {} Links  (窄终端：侧栏隐藏)", SERVER, LINKS.len())).dim(),
        title,
    );
    link_table(f, list, app, false);
    f.render_widget(keys_inline(&app.note), keys);
}

/// Draws the stacked lockup centred in `side`, `cols` wide, with one row of top margin.
/// Returns the area left below it. No image protocol → nothing drawn, all of `side` returned.
fn stacked_logo(f: &mut Frame, side: Rect, cols: u16, app: &mut App) -> Rect {
    f.render_widget(Block::new().borders(Borders::RIGHT).dim(), side);
    let side = Rect { width: side.width - 1, ..side };
    let Some(img) = app.stacked.as_mut() else { return pad(side, 0) };
    let fs = app.picker.font_size();
    let (w, h) = app.stacked_px;
    let rows = ((cols as u32 * fs.width as u32 * h).div_ceil(w * fs.height as u32) as u16)
        .min(side.height.saturating_sub(1));
    let r = Rect { x: side.x + (side.width - cols) / 2, y: side.y + 1, width: cols, height: rows };
    f.render_stateful_widget(StatefulImage::default(), r, img);
    let top = r.y + rows + 2; // ~2X safe space below the lockup
    Rect { y: top, height: side.height.saturating_sub(top - side.y), ..side }
}

fn link_table(f: &mut Frame, area: Rect, app: &mut App, with_header_gap: bool) {
    let rows = LINKS.iter().map(|l| {
        let exp = if l.expiry.starts_with("expired") {
            Span::from(l.expiry).fg(CRAB_RED)
        } else {
            Span::from(l.expiry).dim()
        };
        Row::new(vec![
            Span::from(l.code).bold(),
            Span::from(format!("{:>6}", l.clicks)),
            exp,
            Span::from(l.target).dim(),
        ])
    });
    let header = Row::new(["Code", "Clicks", "Expiry", "Target"])
        .style(Style::new().add_modifier(Modifier::UNDERLINED).dim())
        .bottom_margin(with_header_gap as u16);
    let t = Table::new(
        rows,
        [Constraint::Length(13), Constraint::Length(6), Constraint::Length(15), Constraint::Min(10)],
    )
    .header(header)
    .column_spacing(2)
    .row_highlight_style(Style::new().reversed())
    .highlight_symbol("▌");
    f.render_stateful_widget(t, pad(area, 1), &mut app.table);
}

fn detail(f: &mut Frame, area: Rect, app: &App, borders: Borders) {
    let l = &LINKS[app.table.selected().unwrap_or(0).min(LINKS.len() - 1)];
    let field = |k: &str, v: String| Line::from(vec![Span::from(format!("{k:<12}")).dim(), Span::from(v)]);
    let text = vec![
        field("Short URL", format!("{SERVER}/{}", l.code)).bold(),
        field("Target", l.target.into()),
        field("Click Count", l.clicks.to_string()),
        field("Expiry", l.expiry.into()),
        field("Created", "2026-10-28 14:02".into()),
    ];
    let block = Block::new().borders(borders).border_style(Style::new().dim());
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: false }), pad(inner, 1));
}

fn key_line(k: &'static str, what: &'static str) -> Line<'static> {
    Line::from(vec![Span::from(format!("{k:>2} ")).fg(CRAB_RED).bold(), Span::from(what).dim()])
}

fn keys_inline(note: &str) -> Line<'static> {
    if !note.is_empty() {
        return Line::from(format!(" {note}")).italic();
    }
    let mut spans = vec![Span::from(" ")];
    for (k, w) in [("n", "新建"), ("e", "编辑"), ("d", "删除"), ("/", "过滤"), ("r", "刷新"), ("?", "帮助"), ("q", "退出")] {
        spans.push(Span::from(k).fg(CRAB_RED).bold());
        spans.push(Span::from(format!(" {w}   ")).dim());
    }
    Line::from(spans)
}

/// PROTOTYPE switcher bar — not part of the design being evaluated.
fn switcher(f: &mut Frame, area: Rect, app: &App) {
    let (k, name) = VARIANTS[app.variant];
    let fs = app.picker.font_size();
    let proto = match app.picker.protocol_type() {
        ProtocolType::Halfblocks => "无图片协议 → 不显示 logo".to_string(),
        p => format!("{p:?} · font {}×{}px · {}", fs.width, fs.height, app.bg),
    };
    let note = if app.variant != 0 || app.note.is_empty() { String::new() } else { format!("   {}", app.note) };
    let text = format!(
        " PROTOTYPE  ◀  {k}  {name}  ▶     {proto} · {}×{}{note} ",
        f.area().width,
        f.area().height
    );
    f.render_widget(
        Paragraph::new(text).style(Style::new().bg(Color::Rgb(0xFF, 0xD8, 0x4D)).fg(Color::Black)),
        area,
    );
}

fn pad(r: Rect, n: u16) -> Rect {
    Rect {
        x: r.x + n,
        width: r.width.saturating_sub(n * 2),
        ..r
    }
}
