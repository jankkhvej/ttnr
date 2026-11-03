//! Theme animations (ADR-034): a power-on sequence when a theme is switched
//! to, the ZX Spectrum tape-loading border while a station loads, and the
//! ZX80's blank picture while it is busy. Text and colours only. Time is
//! passed in, so the model is testable; the UI ticks only while something
//! is animating.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ttnr_core::app::{AppState, Status};
use ttnr_themes::{BootEffect, ColorDepth, LoadingEffect, Theme, ThemeColor};

use crate::view::color;

/// Frame interval while animating (25 frames per second).
pub const FRAME: Duration = Duration::from_millis(40);

// ZX Spectrum 48K timing in T-states (World of Spectrum FAQ, RESEARCH.md).
const T_PER_SECOND: f64 = 3_500_000.0;
const T_PER_LINE: f64 = 224.0;
const PILOT_PULSE: f64 = 2168.0;
const BIT0_PULSE: f64 = 855.0;
const BIT1_PULSE: f64 = 1710.0;
/// Scan lines per half character row (8 lines per row; `▀` halves a cell).
const LINES_PER_HALF_ROW: f64 = 4.0;

/// Tape: pilot tone first, then data; the stripes go on a little after
/// the audio starts, as if the program were still loading.
const PILOT_TIME: Duration = Duration::from_millis(1600);
const MIN_DATA_TIME: Duration = Duration::from_millis(1200);
const TAIL_TIME: Duration = Duration::from_millis(2000);
const MAX_LOAD_TIME: Duration = Duration::from_secs(20);
/// ZX80 blank: at least this long, so it is seen even on a fast start.
const MIN_BLANK_TIME: Duration = Duration::from_millis(400);

/// An RGB colour.
type Rgb = (u8, u8, u8);

/// ZX Spectrum colours, non-bright level 0xD7 (a common emulator palette;
/// the real level is analogue – design choice, RESEARCH.md).
mod zx {
    pub const BLACK: (u8, u8, u8) = (0, 0, 0);
    pub const BLUE: (u8, u8, u8) = (0, 0, 0xd7);
    pub const RED: (u8, u8, u8) = (0xd7, 0, 0);
    pub const MAGENTA: (u8, u8, u8) = (0xd7, 0, 0xd7);
    pub const GREEN: (u8, u8, u8) = (0, 0xd7, 0);
    pub const CYAN: (u8, u8, u8) = (0, 0xd7, 0xd7);
    pub const YELLOW: (u8, u8, u8) = (0xd7, 0xd7, 0);
    pub const WHITE: (u8, u8, u8) = (0xd7, 0xd7, 0xd7);
    pub const ALL: [(u8, u8, u8); 8] = [BLACK, BLUE, RED, MAGENTA, GREEN, CYAN, YELLOW, WHITE];
}

#[derive(Debug, Clone, Copy)]
struct Load {
    kind: LoadingEffect,
    start: Instant,
    /// Still resolving/opening (not yet playing).
    busy: bool,
    playing_since: Option<Instant>,
}

/// What to paint over this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FxFrame {
    /// Tape stripes in the border; `tape` = position on the tape in
    /// T-states, `seed` varies the data bits per frame.
    Tape {
        /// Pilot tone (red/cyan) or data (blue/yellow).
        pilot: bool,
        /// Tape position (T-states) at the top of the picture.
        tape: f64,
        /// Seed for the data bits.
        seed: u64,
    },
    /// The whole picture blank (ZX80 busy).
    Blank,
    /// A power-on sequence at `t` after it started.
    Boot {
        /// Which machine.
        effect: BootEffect,
        /// Time since the start.
        t: Duration,
        /// Seed for random content (garbage, test data).
        seed: u64,
    },
}

/// Animation state; feed it every state the UI draws.
#[derive(Debug, Clone)]
pub struct Fx {
    enabled: bool,
    depth: ColorDepth,
    theme: Option<String>,
    /// Running boot sequence: effect, start, length.
    boot: Option<(BootEffect, Instant, Duration)>,
    load: Option<Load>,
}

impl Fx {
    /// `enabled = false` (config `effects = false`) never animates.
    pub fn new(enabled: bool, depth: ColorDepth) -> Self {
        Self {
            enabled,
            depth,
            theme: None,
            boot: None,
            load: None,
        }
    }

    /// Track theme switches (→ boot sequence) and loading (→ tape/blank).
    pub fn observe(&mut self, s: &AppState, theme: &Theme, now: Instant) {
        if self.theme.as_deref() != Some(theme.name.as_str()) {
            self.theme = Some(theme.name.clone());
            self.load = None;
            let effect = theme.effects.boot;
            self.boot = (self.enabled && effect != BootEffect::None).then(|| {
                (
                    effect,
                    now,
                    boot_length(effect, theme.effects.boot_text.len()),
                )
            });
        }
        let kind = theme.effects.loading;
        if !self.enabled || kind == LoadingEffect::None {
            self.load = None;
            return;
        }
        let busy = matches!(
            s.status,
            Status::Resolving | Status::Loading | Status::Reconnecting(_)
        );
        match (&mut self.load, &s.status) {
            (None, _) if busy => {
                self.load = Some(Load {
                    kind,
                    start: now,
                    busy,
                    playing_since: None,
                });
            }
            (Some(load), Status::Playing | Status::Buffering(_)) => {
                load.busy = false;
                if matches!(s.status, Status::Playing) && load.playing_since.is_none() {
                    load.playing_since = Some(now);
                }
            }
            (Some(load), _) if busy => load.busy = true,
            (Some(_), _) => self.load = None,
            (None, _) => {}
        }
    }

    /// Skip the boot sequence (any key).
    pub fn skip_boot(&mut self) {
        self.boot = None;
    }

    /// Whether a boot sequence is showing.
    pub fn booting(&self, now: Instant) -> bool {
        self.boot
            .is_some_and(|(_, start, length)| now.saturating_duration_since(start) < length)
    }

    /// When to draw the next frame; `None` when nothing animates.
    pub fn tick(&mut self, now: Instant) -> Option<Duration> {
        self.expire(now);
        (self.boot.is_some() || self.load.is_some()).then_some(FRAME)
    }

    /// What to paint now, if anything.
    pub fn frame(&mut self, now: Instant) -> Option<FxFrame> {
        self.expire(now);
        let seed = frame_seed(now, self.boot.map(|b| b.1).or(self.load.map(|l| l.start)));
        if let Some((effect, start, _)) = self.boot {
            return Some(FxFrame::Boot {
                effect,
                t: now.saturating_duration_since(start),
                seed,
            });
        }
        let load = self.load?;
        let t = now.saturating_duration_since(load.start);
        match load.kind {
            LoadingEffect::Tape if self.depth != ColorDepth::Mono => Some(FxFrame::Tape {
                pilot: t < PILOT_TIME,
                tape: t.as_secs_f64() * T_PER_SECOND,
                seed,
            }),
            LoadingEffect::Blank => Some(FxFrame::Blank),
            _ => None,
        }
    }

    fn expire(&mut self, now: Instant) {
        if !self.booting(now) {
            self.boot = None;
        }
        if let Some(load) = self.load {
            let t = now.saturating_duration_since(load.start);
            let done = match load.kind {
                LoadingEffect::Tape => {
                    t >= PILOT_TIME + MIN_DATA_TIME
                        && load
                            .playing_since
                            .is_some_and(|p| now.saturating_duration_since(p) >= TAIL_TIME)
                }
                LoadingEffect::Blank => !load.busy && t >= MIN_BLANK_TIME,
                LoadingEffect::None => true,
            };
            if done || t >= MAX_LOAD_TIME {
                self.load = None;
            }
        }
    }

    /// Paint `frame` into `buf`: `full` is the terminal, `screen` the
    /// theme's logical screen.
    pub fn paint(&self, frame: FxFrame, buf: &mut Buffer, full: Rect, screen: Rect, theme: &Theme) {
        match frame {
            FxFrame::Tape { pilot, tape, seed } => {
                let rows = tape_rows(pilot, tape, seed, full.height);
                for (dy, (top, bottom)) in rows.into_iter().enumerate() {
                    let y = full.y + u16::try_from(dy).unwrap_or(u16::MAX);
                    let style = Style::default().fg(self.rgb(top)).bg(self.rgb(bottom));
                    for x in full.left()..full.right() {
                        let inside = x >= screen.left()
                            && x < screen.right()
                            && y >= screen.top()
                            && y < screen.bottom();
                        if !inside {
                            put(buf, x, y, "▀", style);
                        }
                    }
                }
            }
            FxFrame::Blank => fill(buf, full, " ", Style::default().bg(color(theme.palette.bg))),
            FxFrame::Boot { effect, t, seed } => {
                self.paint_boot(effect, t, seed, buf, full, screen, theme)
            }
        }
    }

    fn rgb(&self, c: Rgb) -> ratatui::style::Color {
        color(ThemeColor::Rgb(c.0, c.1, c.2).downgrade(self.depth))
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_boot(
        &self,
        effect: BootEffect,
        t: Duration,
        seed: u64,
        buf: &mut Buffer,
        full: Rect,
        screen: Rect,
        theme: &Theme,
    ) {
        let ms = t.as_millis();
        let base = Style::default()
            .fg(color(theme.palette.fg))
            .bg(color(theme.palette.bg));
        match effect {
            BootEffect::None => {}
            BootEffect::ZxSpectrum => {
                // START_NEW sets the border white, fills RAM with 2s, reads
                // it back (2 → 1 → 0), then CLS and the copyright line.
                // Screen bytes of 2 = one thin line per cell, attributes of
                // 2 = red ink on black (inferred from the ROM, RESEARCH.md).
                fill(buf, full, " ", Style::default().bg(self.rgb(zx::WHITE)));
                if ms < 250 {
                    // Static: power-on RAM content does not move.
                    let mut rng = Rng(0x5eed_2b1d);
                    const GARBAGE: [&str; 12] =
                        ["▘", "▝", "▖", "▗", "▚", "▞", "▀", "▄", "▌", "▐", "█", " "];
                    for y in screen.top()..screen.bottom() {
                        for x in screen.left()..screen.right() {
                            let g = GARBAGE[rng.below(GARBAGE.len())];
                            let fg = zx::ALL[rng.below(8)];
                            let bg = zx::ALL[rng.below(8)];
                            put(
                                buf,
                                x,
                                y,
                                g,
                                Style::default().fg(self.rgb(fg)).bg(self.rgb(bg)),
                            );
                        }
                    }
                } else if ms < 1000 {
                    // Broken bar: the thin per-cell lines, as remembered
                    // ("lines / exclamation marks").
                    let glyph = "¦";
                    let style = Style::default()
                        .fg(self.rgb(zx::RED))
                        .bg(self.rgb(zx::BLACK));
                    fill(buf, screen, glyph, style);
                } else {
                    let paper = Style::default()
                        .fg(self.rgb(zx::BLACK))
                        .bg(self.rgb(zx::WHITE));
                    fill(buf, screen, " ", paper);
                    let content = crate::view::content_area(screen, theme);
                    let y = content.bottom().saturating_sub(1);
                    text(
                        buf,
                        content.left(),
                        y,
                        content.width,
                        "© 1982 Sinclair Research Ltd",
                        paper,
                    );
                }
            }
            BootEffect::Zx80 => {
                // Blank picture, inverse K cursor bottom left (manual).
                fill(buf, full, " ", Style::default().bg(color(theme.palette.bg)));
                if ms >= 300 {
                    let y = screen.bottom().saturating_sub(1);
                    put(
                        buf,
                        screen.left(),
                        y,
                        "K",
                        base.add_modifier(Modifier::REVERSED),
                    );
                }
            }
            BootEffect::BbcMicro => {
                // MODE 7: the banner, then BASIC's ">" with a flashing cursor.
                fill(
                    buf,
                    full,
                    " ",
                    Style::default().bg(color(theme.screen.border)),
                );
                fill(buf, screen, " ", base);
                if ms >= 300 {
                    let mut y = screen.top();
                    for line in &theme.chrome.banner {
                        text(buf, screen.left(), y, screen.width, line, base);
                        y += 1;
                    }
                    let prompt = theme.chrome.prompt.as_deref().unwrap_or(">");
                    text(buf, screen.left(), y, screen.width, prompt, base);
                    let cx = screen.left() + u16::try_from(prompt.chars().count()).unwrap_or(0);
                    if (ms / 320).is_multiple_of(2) && cx < screen.right() {
                        put(buf, cx, y, "_", base);
                    }
                }
            }
            BootEffect::Vt100 => {
                // Self-test: test data, erase, bell + cursor top left
                // (VT100/VT102 user guides); then the RSX-11M console boot.
                fill(
                    buf,
                    full,
                    " ",
                    Style::default().bg(color(theme.screen.border)),
                );
                fill(buf, screen, " ", base);
                if ms < 500 {
                    let mut rng = Rng(seed | 1);
                    for y in screen.top()..screen.bottom() {
                        for x in screen.left()..screen.right() {
                            let c = char::from(b'!' + u8::try_from(rng.below(94)).unwrap_or(0));
                            put(buf, x, y, &c.to_string(), base);
                        }
                    }
                } else if ms < 800 {
                    put(
                        buf,
                        screen.left(),
                        screen.top(),
                        " ",
                        base.add_modifier(Modifier::REVERSED),
                    );
                } else {
                    let lines = &theme.effects.boot_text;
                    let per_line = u128::from(line_delay(lines.len()));
                    let shown = usize::try_from((ms - 800) / per_line + 1)
                        .unwrap_or(usize::MAX)
                        .min(lines.len());
                    // The console scrolls when the lines exceed the screen.
                    let rows = usize::from(screen.height);
                    let first = shown.saturating_sub(rows);
                    let mut y = screen.top();
                    for line in &lines[first..shown] {
                        text(buf, screen.left(), y, screen.width, line, base);
                        y += 1;
                    }
                    if let Some(last) = lines[first..shown].last()
                        && shown == lines.len()
                    {
                        // Cursor after the final prompt.
                        let len = u16::try_from(last.chars().count()).unwrap_or(0);
                        let x = (screen.left() + len).min(screen.right().saturating_sub(1));
                        put(buf, x, y - 1, " ", base.add_modifier(Modifier::REVERSED));
                    }
                }
            }
        }
    }
}

/// Time between console lines of a `vt100` boot: 350 ms, faster for long
/// transcripts so the whole boot stays under ~3 s of text.
fn line_delay(lines: usize) -> u64 {
    let n = u64::try_from(lines.max(1)).unwrap_or(1);
    (2400 / n).clamp(80, 350)
}

/// How long each boot sequence lasts.
/// `boot_lines` = number of `boot_text` lines (for the `vt100` boot).
pub fn boot_length(effect: BootEffect, boot_lines: usize) -> Duration {
    Duration::from_millis(match effect {
        BootEffect::None => 0,
        BootEffect::ZxSpectrum => 1400,
        BootEffect::Zx80 => 1200,
        BootEffect::BbcMicro => 1900,
        BootEffect::Vt100 => {
            let n = u64::try_from(boot_lines).unwrap_or(0);
            800 + line_delay(boot_lines) * n + 600
        }
    })
}

/// Border colours (upper, lower half) for each terminal row: the colour
/// flips on every tape pulse edge, sampled every 4 scan lines.
fn tape_rows(pilot: bool, tape: f64, seed: u64, rows: u16) -> Vec<(Rgb, Rgb)> {
    let (on, off) = if pilot {
        (zx::CYAN, zx::RED)
    } else {
        (zx::YELLOW, zx::BLUE)
    };
    // Pulse edges, in scan lines from the top of the picture.
    let total_lines = f64::from(rows) * 2.0 * LINES_PER_HALF_ROW;
    let mut edges = Vec::new();
    if pilot {
        // Constant pulses; the pattern rolls as the tape position moves.
        let phase = (tape % PILOT_PULSE) / T_PER_LINE;
        let step = PILOT_PULSE / T_PER_LINE;
        let mut line = -phase;
        while line < total_lines {
            line += step;
            edges.push(line);
        }
    } else {
        // Random bits, each two equal pulses (855 or 1710 T-states).
        let mut rng = Rng(seed | 1);
        let mut line = -(rng.below(8) as f64);
        while line < total_lines {
            let pulse = if rng.below(2) == 0 {
                BIT0_PULSE
            } else {
                BIT1_PULSE
            } / T_PER_LINE;
            line += pulse;
            edges.push(line);
            line += pulse;
            edges.push(line);
        }
    }
    let colour_at = |line: f64| {
        let flips = edges.iter().take_while(|e| **e <= line).count();
        if flips % 2 == 0 { off } else { on }
    };
    (0..rows)
        .map(|r| {
            let top = (f64::from(r) * 2.0 + 0.5) * LINES_PER_HALF_ROW;
            let bottom = (f64::from(r) * 2.0 + 1.5) * LINES_PER_HALF_ROW;
            (colour_at(top), colour_at(bottom))
        })
        .collect()
}

fn frame_seed(now: Instant, start: Option<Instant>) -> u64 {
    let since = start.map_or(0, |s| now.saturating_duration_since(s).as_millis());
    u64::try_from(since / FRAME.as_millis())
        .unwrap_or(0)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// xorshift64 – tiny deterministic noise for garbage and data bits.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % n.max(1) as u64).unwrap_or(0)
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        // Reset first: `set_style` patches, so modifiers of the UI below
        // (the list highlight's reverse video) would show through.
        cell.reset();
        cell.set_symbol(symbol).set_style(style);
    }
}

fn fill(buf: &mut Buffer, area: Rect, symbol: &str, style: Style) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            put(buf, x, y, symbol, style);
        }
    }
}

fn text(buf: &mut Buffer, x: u16, y: u16, width: u16, s: &str, style: Style) {
    for (i, c) in s.chars().take(usize::from(width)).enumerate() {
        put(
            buf,
            x + u16::try_from(i).unwrap_or(0),
            y,
            &c.to_string(),
            style,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttnr_themes::ThemeSet;

    fn theme(name: &str) -> Theme {
        let set = ThemeSet::builtin();
        set.get(set.position(name).unwrap())
            .for_depth(ColorDepth::TrueColor)
    }

    fn state(status: Status) -> AppState {
        AppState {
            status,
            ..AppState::default()
        }
    }

    #[test]
    fn boot_runs_once_per_switch_and_can_be_skipped() {
        let t0 = Instant::now();
        let mut fx = Fx::new(true, ColorDepth::TrueColor);
        let zx = theme("zx-spectrum");
        fx.observe(&state(Status::Idle), &zx, t0);
        assert!(matches!(
            fx.frame(t0),
            Some(FxFrame::Boot {
                effect: BootEffect::ZxSpectrum,
                ..
            })
        ));
        assert_eq!(fx.tick(t0), Some(FRAME));
        let after = t0 + boot_length(BootEffect::ZxSpectrum, 0);
        fx.observe(&state(Status::Idle), &zx, after);
        assert_eq!(fx.frame(after), None, "same theme: no new boot");
        assert_eq!(fx.tick(after), None, "idle: no ticks");
        let plain = theme("plain");
        fx.observe(&state(Status::Idle), &plain, after);
        assert_eq!(fx.frame(after), None, "plain has no boot");
        fx.observe(&state(Status::Idle), &zx, after);
        assert!(fx.booting(after));
        fx.skip_boot();
        assert_eq!(fx.frame(after), None);
    }

    #[test]
    fn disabled_never_animates() {
        let t0 = Instant::now();
        let mut fx = Fx::new(false, ColorDepth::TrueColor);
        let zx = theme("zx-spectrum");
        fx.observe(&state(Status::Resolving), &zx, t0);
        assert_eq!(fx.frame(t0), None);
        assert_eq!(fx.tick(t0), None);
    }

    #[test]
    fn tape_pilot_then_data_then_stops_after_tail() {
        let t0 = Instant::now();
        let mut fx = Fx::new(true, ColorDepth::TrueColor);
        let zx = theme("zx-spectrum");
        fx.observe(&state(Status::Idle), &zx, t0);
        fx.skip_boot();
        let ms = |n| t0 + Duration::from_millis(n);
        fx.observe(&state(Status::Resolving), &zx, ms(0));
        assert!(matches!(
            fx.frame(ms(0)),
            Some(FxFrame::Tape { pilot: true, .. })
        ));
        fx.observe(&state(Status::Playing), &zx, ms(500));
        assert!(matches!(
            fx.frame(ms(1700)),
            Some(FxFrame::Tape { pilot: false, .. })
        ));
        // Playing since 0.5 s: tail until 2.5 s, minimum pilot+data 2.8 s.
        assert!(fx.frame(ms(2700)).is_some());
        assert_eq!(fx.frame(ms(2800)), None);
        assert_eq!(fx.tick(ms(2800)), None);
    }

    #[test]
    fn tape_stops_on_error_and_skips_mono() {
        let t0 = Instant::now();
        let zx = theme("zx-spectrum");
        let mut fx = Fx::new(true, ColorDepth::TrueColor);
        fx.observe(&state(Status::Idle), &zx, t0);
        fx.skip_boot();
        fx.observe(&state(Status::Resolving), &zx, t0);
        fx.observe(&state(Status::Error("x".into())), &zx, t0);
        assert_eq!(fx.frame(t0), None);
        let mut mono = Fx::new(true, ColorDepth::Mono);
        mono.observe(&state(Status::Idle), &zx, t0);
        mono.skip_boot();
        mono.observe(&state(Status::Resolving), &zx, t0);
        assert_eq!(mono.frame(t0), None, "no stripes without colour");
    }

    #[test]
    fn zx80_blanks_while_busy_at_least_briefly() {
        let t0 = Instant::now();
        let zx80 = theme("zx80");
        let mut fx = Fx::new(true, ColorDepth::TrueColor);
        fx.observe(&state(Status::Idle), &zx80, t0);
        fx.skip_boot();
        let ms = |n| t0 + Duration::from_millis(n);
        fx.observe(&state(Status::Loading), &zx80, ms(0));
        fx.observe(&state(Status::Playing), &zx80, ms(100));
        assert_eq!(fx.frame(ms(300)), Some(FxFrame::Blank));
        assert_eq!(fx.frame(ms(400)), None);
    }

    #[test]
    fn pilot_stripes_are_about_ten_lines() {
        // 2168 T-states / 224 per line ≈ 9.7 lines ≈ 2.4 half rows per
        // colour: over 24 rows (48 half rows) about 20 colour changes.
        let rows = tape_rows(true, 0.0, 1, 24);
        let halves: Vec<_> = rows.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let changes = halves.windows(2).filter(|w| w[0] != w[1]).count();
        assert!((17..=22).contains(&changes), "{changes}");
        assert!(halves.iter().all(|c| *c == zx::RED || *c == zx::CYAN));
        let data = tape_rows(false, 0.0, 7, 24);
        assert!(
            data.iter()
                .all(|(a, b)| [*a, *b].iter().all(|c| *c == zx::BLUE || *c == zx::YELLOW))
        );
    }
}
