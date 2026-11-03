//! Rendering [`AppState`] through a [`Theme`]: the main view (SPEC F5.1)
//! and the overlays (stations, open URL, help, devices). Adapts to any size
//! from 32×16 up: lines are dropped by priority, text is cut with the
//! theme's ellipsis. No colours or glyphs are hard-coded here.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ttnr_core::NowPlaying;
use ttnr_core::app::{AppState, Status, View};
use ttnr_themes::{CursorShape, StatusKind, Theme, ThemeColor, TitleStyle};

use crate::fx::{Fx, FxFrame};
use crate::keys::Keymap;

/// Draw the whole UI for `state` with `theme`; help texts follow `keymap`.
pub fn draw(frame: &mut Frame, state: &AppState, theme: &Theme, keymap: &Keymap) {
    draw_with_printout(frame, state, theme, keymap, &[]);
}

/// [`draw`] for themes in printout mode (Teletype): the main area shows
/// `printout` (lines printed so far, oldest first) instead of the main view.
pub fn draw_with_printout(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    keymap: &Keymap,
    printout: &[String],
) {
    draw_with(frame, state, theme, keymap, &Extras { printout, fx: None });
}

/// Optional extras for [`draw_with`].
#[derive(Default)]
pub struct Extras<'a> {
    /// Printed lines for printout themes (oldest first).
    pub printout: &'a [String],
    /// An animation frame painted over the UI.
    pub fx: Option<(&'a Fx, FxFrame)>,
}

/// [`draw`] with a printout and/or an animation frame.
pub fn draw_with(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    keymap: &Keymap,
    extras: &Extras<'_>,
) {
    let printout = extras.printout;
    let full = frame.area();
    let area = logical_screen(full, theme);
    if area != full {
        // The machine's border around its fixed-size screen.
        frame.render_widget(
            Block::default().style(Style::default().bg(color(theme.screen.border))),
            full,
        );
    }
    let base = Style::default()
        .fg(color(theme.palette.fg))
        .bg(color(theme.palette.bg));
    frame.render_widget(Block::default().style(base), area);
    let inner = framed(frame, area, theme, &theme.title);
    // One column of margin when there is room for it.
    let padded = if inner.width >= 40 {
        Rect {
            x: inner.x + 1,
            width: inner.width - 2,
            ..inner
        }
    } else {
        inner
    };
    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(padded);
    if theme.chrome.printout {
        printout_view(frame, body, printout, theme);
    } else {
        main_view(frame, body, state, theme);
    }
    frame.render_widget(
        Paragraph::new(footer_text(theme, keymap, usize::from(footer.width))).style(dim(theme)),
        footer,
    );
    match &state.view {
        View::Main => {}
        View::Stations { selected } => stations(frame, inner, state, *selected, theme),
        View::OpenUrl { input } => open_url(frame, inner, input, theme),
        View::Help => help_overlay(frame, inner, theme, keymap),
        View::Devices { selected } => devices(frame, inner, state, *selected, theme),
    }
    if let Some((fx, fx_frame)) = extras.fx {
        fx.paint(fx_frame, frame.buffer_mut(), full, area, theme);
    }
}

/// The theme's logical screen (e.g. 32×24) centred in `full`; the whole
/// area when the theme has no fixed size or the terminal is smaller.
pub fn logical_screen(full: Rect, theme: &Theme) -> Rect {
    let w = match theme.screen.width {
        0 => full.width,
        w => w.min(full.width),
    };
    let h = match theme.screen.height {
        0 => full.height,
        h => h.min(full.height),
    };
    Rect {
        x: full.x + (full.width - w) / 2,
        y: full.y + (full.height - h) / 2,
        width: w,
        height: h,
    }
}

/// Printed lines, newest at the bottom (paper scrolls up).
fn printout_view(frame: &mut Frame, area: Rect, printout: &[String], theme: &Theme) {
    let h = usize::from(area.height);
    let w = usize::from(area.width);
    let start = printout.len().saturating_sub(h);
    let mut lines: Vec<Line> = printout[start..]
        .iter()
        .map(|l| Line::raw(theme.fit(l, w)))
        .collect();
    // Bottom-align: the newest line sits just above the key help.
    let pad = h.saturating_sub(lines.len());
    let mut text = vec![Line::raw(""); pad];
    text.append(&mut lines);
    frame.render_widget(Paragraph::new(text), area);
}

/// The key help line for `width`: leading bindings that fit, then always
/// help and quit.
fn footer_text(theme: &Theme, keymap: &Keymap, width: usize) -> String {
    let sep = &theme.glyphs.separator;
    let (entries, tail) = keymap.footer();
    let tail = tail.join(sep);
    let mut parts: Vec<String> = Vec::new();
    for entry in entries {
        let mut candidate = parts.clone();
        candidate.push(entry.clone());
        let len = candidate.join(sep).chars().count() + sep.chars().count() + tail.chars().count();
        if len > width {
            break;
        }
        parts = candidate;
    }
    parts.push(tail);
    theme.fit(&parts.join(sep), width)
}

/// Foreground style for a palette role (`Faint` = default colour + SGR 2).
pub fn fg(c: ThemeColor) -> Style {
    match c {
        ThemeColor::Faint => Style::default()
            .fg(Color::Reset)
            .add_modifier(Modifier::DIM),
        c => Style::default().fg(color(c)),
    }
}

/// ThemeColor → ratatui colour (already downgraded by `Theme::for_depth`).
pub fn color(c: ThemeColor) -> Color {
    const ANSI: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    match c {
        ThemeColor::Reset | ThemeColor::Faint => Color::Reset,
        ThemeColor::Ansi(n) => ANSI[usize::from(n & 15)],
        ThemeColor::Indexed(n) => Color::Indexed(n),
        ThemeColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn dim(theme: &Theme) -> Style {
    fg(theme.palette.dim)
}

fn accent(theme: &Theme) -> Style {
    fg(theme.palette.accent)
}

/// Draw the theme's frame (if it has borders) with `title`; returns the
/// inner area.
fn framed(frame: &mut Frame, area: Rect, theme: &Theme, title: &str) -> Rect {
    let Some(b) = &theme.glyphs.borders else {
        return area;
    };
    let set = border::Set {
        top_left: &b.top_left,
        top_right: &b.top_right,
        bottom_left: &b.bottom_left,
        bottom_right: &b.bottom_right,
        vertical_left: &b.vertical,
        vertical_right: &b.vertical,
        horizontal_top: &b.horizontal,
        horizontal_bottom: &b.horizontal,
    };
    let title_width = usize::from(area.width.saturating_sub(4));
    let block = Block::bordered()
        .border_set(set)
        .border_style(dim(theme))
        .title(Span::styled(
            format!(" {} ", theme.fit(title, title_width.saturating_sub(2))),
            accent(theme),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

/// "Artist – Title" (or just the title).
fn track(np: &NowPlaying, theme: &Theme) -> Option<String> {
    match (&np.artist, &np.title) {
        (Some(a), Some(t)) => Some(format!("{a}{}{t}", theme.glyphs.dash)),
        (None, Some(t)) => Some(t.clone()),
        (Some(a), None) => Some(a.clone()),
        (None, None) => None,
    }
}

/// The status line (from the theme's template) and its style.
pub fn status_parts(status: &Status, theme: &Theme) -> (String, Style) {
    let g = &theme.glyphs;
    let p = &theme.palette;
    let (kind, glyph, message, percent, c) = match status {
        Status::Idle => (StatusKind::Idle, &g.stopped, "", None, p.dim),
        Status::Resolving => (StatusKind::Resolving, &g.busy, "", None, p.warn),
        Status::Loading => (StatusKind::Loading, &g.busy, "", None, p.warn),
        Status::Buffering(n) => (StatusKind::Buffering, &g.busy, "", *n, p.warn),
        Status::Playing => (StatusKind::Playing, &g.playing, "", None, p.accent),
        Status::Paused => (StatusKind::Paused, &g.paused, "", None, p.fg),
        Status::Reconnecting(why) => (
            StatusKind::Reconnecting,
            &g.busy,
            why.as_str(),
            None,
            p.warn,
        ),
        Status::Ended => (StatusKind::Ended, &g.stopped, "", None, p.dim),
        Status::Error(e) => (StatusKind::Error, &g.error, e.as_str(), None, p.error),
    };
    (theme.status_line(kind, glyph, message, percent), fg(c))
}

/// The output device label: pinned device's description or "system default".
fn device_label(state: &AppState) -> String {
    match &state.device {
        None => "system default".into(),
        Some(name) => state
            .devices
            .iter()
            .find(|d| &d.name == name)
            .map_or_else(|| name.clone(), |d| d.description.clone()),
    }
}

fn main_view(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let w = usize::from(area.width);
    let h = usize::from(area.height);
    // (priority, line): lower priority number = kept longer.
    let mut lines: Vec<(u8, Line)> = Vec::new();
    for b in &theme.chrome.banner {
        lines.push((8, Line::styled(theme.fit(b, w), dim(theme))));
    }
    let station = state
        .station
        .as_ref()
        .map_or("no station – press s or o", |s| s.name.as_str());
    match theme.chrome.title {
        TitleStyle::Plain => lines.push((
            0,
            Line::styled(
                theme.fit(station, w),
                accent(theme).add_modifier(Modifier::BOLD),
            ),
        )),
        TitleStyle::Banner => {
            // Emulated double height: a two-row bar in the accent colour;
            // inverse video when there is no colour (mono fallback).
            let bar = if theme.palette.accent == ThemeColor::Reset {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default()
                    .bg(color(theme.palette.accent))
                    .fg(color(theme.palette.bg))
                    .add_modifier(Modifier::BOLD)
            };
            let text = theme.fit(&format!(" {station}"), w);
            let pad = w.saturating_sub(text.chars().count());
            lines.push((0, Line::styled(format!("{text}{}", " ".repeat(pad)), bar)));
            lines.push((5, Line::styled(" ".repeat(w), bar)));
        }
    }
    if let Some(unit) = theme.chrome.rule.as_deref().filter(|u| !u.is_empty()) {
        let n = w / unit.chars().count().max(1);
        lines.push((7, Line::styled(unit.repeat(n), accent(theme))));
    }
    let now = state.now.as_ref();
    if let Some(p) = now.and_then(|n| n.programme.as_deref()) {
        lines.push((5, Line::styled(theme.fit(p, w), dim(theme))));
    }
    lines.push((9, Line::raw("")));
    if let Some(t) = now.and_then(|n| track(n, theme)) {
        lines.push((
            1,
            Line::styled(
                theme.fit(&t, w),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ));
    }
    if let Some(album) = now.and_then(|n| n.album.as_deref()) {
        lines.push((6, Line::styled(theme.fit(album, w), dim(theme))));
    }
    lines.push((9, Line::raw("")));
    if let Some(info) = &state.stream {
        let summary = info.summary().replace(" · ", &theme.glyphs.separator);
        if !summary.is_empty() {
            lines.push((4, Line::styled(theme.fit(&summary, w), dim(theme))));
        }
    }
    let (status, style) = status_parts(&state.status, theme);
    lines.push((2, Line::styled(theme.fit(&status, w), style)));
    let sync = if state.sync_offset.is_zero() {
        String::new()
    } else {
        format!(
            "{}sync {}s",
            theme.glyphs.separator,
            theme.number(state.sync_offset.as_secs())
        )
    };
    let vol = format!(
        "vol {}{}{}{}{sync}",
        theme.percent(u64::from(state.volume)),
        if state.muted { " muted" } else { "" },
        theme.glyphs.separator,
        device_label(state)
    );
    lines.push((3, Line::styled(theme.fit(&vol, w), Style::default())));
    if !state.history.is_empty() {
        lines.push((9, Line::raw("")));
        lines.push((7, Line::styled(theme.fit("recent", w), dim(theme))));
        for (i, np) in state.history.iter().enumerate() {
            if let Some(t) = track(np, theme) {
                let prio = 8u8.saturating_add(u8::try_from(i).unwrap_or(u8::MAX));
                lines.push((
                    prio,
                    Line::styled(theme.fit(&format!("  {t}"), w), dim(theme)),
                ));
            }
        }
    }
    // Drop the least important lines until everything fits.
    while lines.len() > h {
        let Some((idx, _)) = lines.iter().enumerate().max_by_key(|(i, (p, _))| (*p, *i)) else {
            break;
        };
        lines.remove(idx);
    }
    // No blank line twice in a row, none at the start or end.
    let mut text: Vec<Line> = Vec::new();
    for (_, line) in lines {
        let blank = line.width() == 0;
        if blank && text.last().is_none_or(|l: &Line| l.width() == 0) {
            continue;
        }
        text.push(line);
    }
    while text.last().is_some_and(|l| l.width() == 0) {
        text.pop();
    }
    frame.render_widget(Paragraph::new(text), area);
}

/// A centred popup of at most `w`×`h` inside `area`, cleared and framed.
fn popup(frame: &mut Frame, area: Rect, w: u16, h: u16, title: &str, theme: &Theme) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, rect);
    let base = Style::default()
        .fg(color(theme.palette.fg))
        .bg(color(theme.palette.bg));
    frame.render_widget(Block::default().style(base), rect);
    framed(frame, rect, theme, title)
}

/// A selectable list: highlighted row reversed, scrolled to stay visible.
fn list(frame: &mut Frame, area: Rect, rows: &[(bool, String)], selected: usize, theme: &Theme) {
    let h = usize::from(area.height).max(1);
    let w = usize::from(area.width);
    let first = selected.saturating_sub(h - 1);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(first)
        .take(h)
        .map(|(i, (current, text))| {
            let cursor = if i == selected {
                &theme.glyphs.cursor
            } else {
                " "
            };
            let mark = if *current { &theme.glyphs.current } else { " " };
            let line = theme.fit(&format!("{cursor}{mark} {text}"), w);
            if i == selected {
                Line::styled(line, accent(theme).add_modifier(Modifier::REVERSED))
            } else {
                Line::raw(line)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn stations(frame: &mut Frame, area: Rect, state: &AppState, selected: usize, theme: &Theme) {
    let current = state.station.as_ref().map(ttnr_core::Station::key);
    let rows: Vec<(bool, String)> = state
        .stations
        .iter()
        .map(|s| (Some(s.key()) == current, s.name.clone()))
        .collect();
    let h = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .max(3);
    let inner = popup(frame, area, 48, h, "stations", theme);
    if rows.is_empty() {
        let msg = "no stations – see examples/stations.toml";
        frame.render_widget(
            Paragraph::new(theme.fit(msg, usize::from(inner.width))).style(dim(theme)),
            inner,
        );
    } else {
        list(frame, inner, &rows, selected, theme);
    }
}

fn devices(frame: &mut Frame, area: Rect, state: &AppState, selected: usize, theme: &Theme) {
    let mut rows = vec![(state.device.is_none(), "system default".to_owned())];
    rows.extend(state.devices.iter().map(|d| {
        (
            state.device.as_deref() == Some(d.name.as_str()),
            d.description.clone(),
        )
    }));
    let h = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let inner = popup(frame, area, 48, h, "output device", theme);
    list(frame, inner, &rows, selected, theme);
}

fn open_url(frame: &mut Frame, area: Rect, input: &str, theme: &Theme) {
    let inner = popup(frame, area, 64, 3, "open URL", theme);
    let w = usize::from(inner.width);
    let prompt = theme.chrome.prompt.as_deref().unwrap_or("> ");
    // The input is shown verbatim, not through the theme's text rules: an
    // uppercased or transliterated URL would misrepresent what is typed.
    let input = input.to_owned();
    // Show the end of a long URL (that is where the user types).
    let room = w.saturating_sub(prompt.chars().count() + 1);
    let shown: String = {
        let chars: Vec<char> = input.chars().collect();
        if chars.len() <= room {
            input.clone()
        } else {
            // Mark the cut-off start with the theme's ellipsis.
            let mark = &theme.glyphs.ellipsis;
            let keep = room.saturating_sub(mark.chars().count());
            let tail: String = chars[chars.len() - keep..].iter().collect();
            format!("{mark}{tail}")
        }
    };
    let mut cursor = match theme.chrome.cursor {
        CursorShape::Block => Style::default().add_modifier(Modifier::REVERSED),
        CursorShape::Underline => Style::default().add_modifier(Modifier::UNDERLINED),
    };
    if theme.chrome.cursor_blink {
        cursor = cursor.add_modifier(Modifier::SLOW_BLINK);
    }
    let line = Line::from(vec![
        Span::styled(theme.text(prompt), accent(theme)),
        Span::raw(shown),
        Span::styled(" ", cursor),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

fn help_overlay(frame: &mut Frame, area: Rect, theme: &Theme, keymap: &Keymap) {
    let rows = keymap.help();
    let h = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let inner = popup(frame, area, 40, h, "keys", theme);
    let w = usize::from(inner.width);
    let key_w = rows
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let lines: Vec<Line> = rows
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(format!("{k:key_w$}  "), accent(theme)),
                Span::raw(theme.fit(d, w.saturating_sub(key_w + 2))),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
