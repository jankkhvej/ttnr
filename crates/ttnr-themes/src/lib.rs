//! Text-only theme engine for ttnr: theme model, TOML loader, built-in
//! themes (embedded), user themes (`~/.config/ttnr/themes/*.toml`), case
//! and width-aware truncation, colour-depth fallback (SPEC F6; THEMES.md).
//!
//! Views never hard-code colours or glyphs: they ask the [`Theme`].

mod color;
mod text;

use std::path::Path;

use serde::Deserialize;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use color::{ColorDepth, ThemeColor};
pub use text::{Numbers, TextRules, transliterate};

/// Built-in theme sources, embedded at compile time.
const BUILTIN: &[&str] = &[
    include_str!("../themes/plain.toml"),
    include_str!("../themes/pdp11-vt100.toml"),
    include_str!("../themes/pdp11-vt100-amber.toml"),
    include_str!("../themes/pdp11-vt100-green.toml"),
    include_str!("../themes/pdp11-teletype.toml"),
    include_str!("../themes/zx80.toml"),
    include_str!("../themes/zx-spectrum.toml"),
    include_str!("../themes/bbc-mode7.toml"),
];

/// Colour roles used by the views.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    /// Normal text.
    pub fg: ThemeColor,
    /// Background.
    pub bg: ThemeColor,
    /// Station name, highlights, titles.
    pub accent: ThemeColor,
    /// Secondary text (format line, history, help).
    pub dim: ThemeColor,
    /// Warnings (reconnecting, buffering).
    pub warn: ThemeColor,
    /// Errors.
    pub error: ThemeColor,
}

/// Per-depth overrides of single palette roles.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteOverride {
    fg: Option<ThemeColor>,
    bg: Option<ThemeColor>,
    accent: Option<ThemeColor>,
    dim: Option<ThemeColor>,
    warn: Option<ThemeColor>,
    error: Option<ThemeColor>,
}

impl Palette {
    fn apply(&mut self, o: &PaletteOverride) {
        let set = |slot: &mut ThemeColor, v: Option<ThemeColor>| {
            if let Some(v) = v {
                *slot = v;
            }
        };
        set(&mut self.fg, o.fg);
        set(&mut self.bg, o.bg);
        set(&mut self.accent, o.accent);
        set(&mut self.dim, o.dim);
        set(&mut self.warn, o.warn);
        set(&mut self.error, o.error);
    }

    fn downgrade(&self, depth: ColorDepth) -> Self {
        Self {
            fg: self.fg.downgrade(depth),
            bg: self.bg.downgrade(depth),
            accent: self.accent.downgrade(depth),
            dim: self.dim.downgrade(depth),
            warn: self.warn.downgrade(depth),
            error: self.error.downgrade(depth),
        }
    }
}

/// Border characters (corners, lines).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Borders {
    /// Top-left corner.
    pub top_left: String,
    /// Top-right corner.
    pub top_right: String,
    /// Bottom-left corner.
    pub bottom_left: String,
    /// Bottom-right corner.
    pub bottom_right: String,
    /// Horizontal line.
    pub horizontal: String,
    /// Vertical line.
    pub vertical: String,
}

impl Borders {
    fn of(corners: [&str; 4], horizontal: &str, vertical: &str) -> Self {
        let [top_left, top_right, bottom_left, bottom_right] = corners.map(str::to_owned);
        Self {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            horizontal: horizontal.to_owned(),
            vertical: vertical.to_owned(),
        }
    }
}

/// `borders` in a theme file: a preset name or explicit glyphs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum BordersSpec {
    Preset(String),
    Custom(Borders),
}

impl BordersSpec {
    fn resolve(self) -> Result<Option<Borders>, String> {
        match self {
            Self::Custom(b) => Ok(Some(b)),
            Self::Preset(p) => match p.as_str() {
                "box" => Ok(Some(Borders::of(["┌", "┐", "└", "┘"], "─", "│"))),
                "double" => Ok(Some(Borders::of(["╔", "╗", "╚", "╝"], "═", "║"))),
                "ascii" => Ok(Some(Borders::of(["+", "+", "+", "+"], "-", "|"))),
                "none" => Ok(None),
                other => Err(format!(
                    "unknown borders preset `{other}` (box, double, ascii, none)"
                )),
            },
        }
    }
}

/// Glyphs the views use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyphs {
    /// Border set; `None` = no borders.
    pub borders: Option<Borders>,
    /// Truncation mark.
    pub ellipsis: String,
    /// Between artist and title.
    pub dash: String,
    /// Between fields of a line (e.g. format line).
    pub separator: String,
    /// Status: playing.
    pub playing: String,
    /// Status: paused.
    pub paused: String,
    /// Status: loading/buffering/reconnecting.
    pub busy: String,
    /// Status: stopped/idle/ended.
    pub stopped: String,
    /// Status: error.
    pub error: String,
    /// Marks the current item in lists.
    pub current: String,
    /// Marks the highlighted row in lists.
    pub cursor: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlyphsFile {
    borders: BordersSpec,
    ellipsis: String,
    dash: String,
    separator: String,
    playing: String,
    paused: String,
    busy: String,
    stopped: String,
    error: String,
    current: String,
    cursor: String,
}

/// The logical screen: a fixed-size area centred in the terminal, the rest
/// filled with the border colour (ZX80: 32×24 on white).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Screen {
    /// Columns; 0 = the whole terminal width.
    pub width: u16,
    /// Rows; 0 = the whole terminal height.
    pub height: u16,
    /// Colour outside the logical screen.
    pub border: ThemeColor,
    /// Paper-coloured columns on each side of the content (keeps border
    /// effects away from the text); dropped first when space is short.
    pub margin_x: u16,
    /// Paper-coloured rows above and below the content.
    pub margin_y: u16,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            border: ThemeColor::Reset,
            margin_x: 0,
            margin_y: 0,
        }
    }
}

/// Cursor shape in text prompts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShape {
    /// Reverse-video block.
    #[default]
    Block,
    /// Underline.
    Underline,
}

/// How the station name is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleStyle {
    /// A bold line.
    #[default]
    Plain,
    /// A two-row coloured bar: emulated double height (teletext).
    Banner,
}

/// Animation while a station loads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadingEffect {
    /// No animation.
    #[default]
    None,
    /// ZX Spectrum tape loading: red/cyan then blue/yellow border stripes.
    Tape,
    /// ZX80: the picture blanks while the machine is busy.
    Blank,
}

/// Power-on sequence shown when the theme is switched to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootEffect {
    /// No boot sequence.
    #[default]
    None,
    /// RAM test stripes, then the copyright line.
    ZxSpectrum,
    /// Empty screen with the inverse K cursor.
    Zx80,
    /// "BBC Computer 32K", "BASIC", ">".
    BbcMicro,
    /// VT100 self-test, then RSX-11M boot lines.
    Vt100,
}

/// Theme animations (off globally with config `effects = false`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Effects {
    /// While loading a station.
    pub loading: LoadingEffect,
    /// When the theme is switched to (and at start).
    pub boot: BootEffect,
}

/// Machine "chrome": banner, prompt, cursor, title style, decoration,
/// printout mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chrome {
    /// Lines shown at the top of the main view (boot banner).
    pub banner: Vec<String>,
    /// Prompt of the open-URL input.
    pub prompt: Option<String>,
    /// Cursor shape.
    pub cursor: CursorShape,
    /// Blink the cursor (terminal attribute; no timers).
    pub cursor_blink: bool,
    /// Station name style.
    pub title: TitleStyle,
    /// One repeating decoration unit for a rule under the title (e.g.
    /// teletext sextants); none if unset.
    pub rule: Option<String>,
    /// The same rule for fonts without the rule's glyphs (e.g. quadrants).
    pub rule_fallback: Option<String>,
    /// Teletype mode: new information is printed (appended) rather than
    /// redrawn in place.
    pub printout: bool,
    /// A terminal on a serial line: the speed key (config `baud`) slows
    /// its output down.
    pub serial: bool,
}

/// Status line templates; `{glyph}`, `{message}`, `{percent}` (" 40%" or
/// "") are replaced.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(missing_docs)]
pub struct StatusTemplates {
    pub idle: String,
    pub resolving: String,
    pub loading: String,
    pub buffering: String,
    pub playing: String,
    pub paused: String,
    pub reconnecting: String,
    pub ended: String,
    pub error: String,
}

impl Default for StatusTemplates {
    fn default() -> Self {
        let t = |s: &str| s.to_owned();
        Self {
            idle: t("{glyph} idle"),
            resolving: t("{glyph} resolving"),
            loading: t("{glyph} loading"),
            buffering: t("{glyph} buffering{percent}"),
            playing: t("{glyph} playing"),
            paused: t("{glyph} paused"),
            reconnecting: t("{glyph} reconnecting: {message}"),
            ended: t("{glyph} stream ended"),
            error: t("{glyph} error: {message}"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: String,
    #[serde(default)]
    title: Option<String>,
    /// Kept for compatibility; same as `[text] uppercase`.
    #[serde(default)]
    uppercase: bool,
    palette: Palette,
    #[serde(default)]
    palette_256: PaletteOverride,
    #[serde(default)]
    palette_16: PaletteOverride,
    glyphs: GlyphsFile,
    #[serde(default)]
    screen: Screen,
    #[serde(default)]
    text: TextRules,
    #[serde(default)]
    chrome: Chrome,
    #[serde(default)]
    status: StatusTemplates,
    #[serde(default)]
    effects: Effects,
}

/// A complete theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Theme id, e.g. `plain`.
    pub name: String,
    /// Colours (as specified, or resolved by [`Theme::for_depth`]).
    pub palette: Palette,
    palette_256: PaletteOverride,
    palette_16: PaletteOverride,
    /// Characters.
    pub glyphs: Glyphs,
    /// Text transformation rules.
    pub text_rules: TextRules,
    /// Banner/title shown in the frame.
    pub title: String,
    /// Logical screen.
    pub screen: Screen,
    /// Banner, prompt, cursor, title style, decoration, printout.
    pub chrome: Chrome,
    /// Animations.
    pub effects: Effects,
    /// Status line templates.
    pub status: StatusTemplates,
}

/// Status kinds for [`Theme::status_line`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum StatusKind {
    Idle,
    Resolving,
    Loading,
    Buffering,
    Playing,
    Paused,
    Reconnecting,
    Ended,
    Error,
}

/// A theme file could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("theme {origin}: {detail}")]
pub struct ThemeError {
    /// File name or `built-in`.
    pub origin: String,
    /// What is wrong.
    pub detail: String,
}

impl Theme {
    /// Parse a theme file; `origin` names it in errors.
    pub fn from_toml(text: &str, origin: &str) -> Result<Self, ThemeError> {
        let err = |detail: String| ThemeError {
            origin: origin.to_owned(),
            detail,
        };
        let f: ThemeFile = toml::from_str(text).map_err(|e| err(e.message().to_owned()))?;
        if f.name.trim().is_empty() {
            return Err(err("name must not be empty".into()));
        }
        let g = f.glyphs;
        Ok(Self {
            title: f.title.unwrap_or_else(|| "ttnr".into()),
            name: f.name,
            palette: f.palette,
            palette_256: f.palette_256,
            palette_16: f.palette_16,
            glyphs: Glyphs {
                borders: g.borders.resolve().map_err(err)?,
                ellipsis: g.ellipsis,
                dash: g.dash,
                separator: g.separator,
                playing: g.playing,
                paused: g.paused,
                busy: g.busy,
                stopped: g.stopped,
                error: g.error,
                current: g.current,
                cursor: g.cursor,
            },
            text_rules: TextRules {
                uppercase: f.uppercase || f.text.uppercase,
                ..f.text
            },
            screen: f.screen,
            chrome: f.chrome,
            effects: f.effects,
            status: f.status,
        })
    }

    /// The default theme: terminal colours, box-drawing borders.
    pub fn plain() -> Self {
        Self::from_toml(BUILTIN[0], "built-in plain")
            .unwrap_or_else(|e| unreachable!("embedded theme is valid: {e}"))
    }

    /// This theme with its palette resolved for a terminal at `depth`
    /// (per-depth overrides first, then automatic downgrading).
    pub fn for_depth(&self, depth: ColorDepth) -> Self {
        let mut palette = self.palette.clone();
        if depth <= ColorDepth::Ansi256 {
            palette.apply(&self.palette_256);
        }
        if depth <= ColorDepth::Ansi16 {
            palette.apply(&self.palette_16);
        }
        Self {
            palette: palette.downgrade(depth),
            screen: Screen {
                border: self.screen.border.downgrade(depth),
                ..self.screen.clone()
            },
            ..self.clone()
        }
    }

    /// Apply the theme's text rules (case, ASCII-only, character map).
    pub fn text(&self, text: &str) -> String {
        self.text_rules.apply(text)
    }

    /// `n` in the theme's number base.
    pub fn number(&self, n: u64) -> String {
        self.text_rules.number(n)
    }

    /// A percentage per the theme's rules (`80%`, octal `120`).
    pub fn percent(&self, n: u64) -> String {
        self.text_rules.percent(n)
    }

    /// The status line for `kind`, from the theme's template.
    pub fn status_line(
        &self,
        kind: StatusKind,
        glyph: &str,
        message: &str,
        percent: Option<u8>,
    ) -> String {
        let s = &self.status;
        let template = match kind {
            StatusKind::Idle => &s.idle,
            StatusKind::Resolving => &s.resolving,
            StatusKind::Loading => &s.loading,
            StatusKind::Buffering => &s.buffering,
            StatusKind::Playing => &s.playing,
            StatusKind::Paused => &s.paused,
            StatusKind::Reconnecting => &s.reconnecting,
            StatusKind::Ended => &s.ended,
            StatusKind::Error => &s.error,
        };
        let percent = percent
            .map(|p| format!(" {}", self.text_rules.percent(u64::from(p))))
            .unwrap_or_default();
        template
            .replace("{glyph}", glyph)
            .replace("{message}", message)
            .replace("{percent}", &percent)
    }

    /// This theme for terminals whose font lacks the rule's glyphs:
    /// `rule` becomes `rule_fallback` (e.g. sextants → quadrants).
    pub fn with_mosaic_fallback(&self) -> Self {
        let mut t = self.clone();
        if t.chrome.rule_fallback.is_some() {
            t.chrome.rule = t.chrome.rule_fallback.clone();
        }
        t
    }

    /// `text` cut to `width` terminal columns (wide CJK characters count
    /// two), ending in the ellipsis glyph when cut – nothing is clipped
    /// silently (THEMES.md).
    pub fn fit(&self, text: &str, width: usize) -> String {
        let text = self.text(text);
        if text.width() <= width {
            return text;
        }
        let mark = self.glyphs.ellipsis.width();
        if width < mark {
            return String::new();
        }
        let mut out = String::new();
        let mut used = 0;
        for c in text.chars() {
            let w = c.width().unwrap_or(0);
            if used + w > width - mark {
                break;
            }
            out.push(c);
            used += w;
        }
        out.push_str(&self.glyphs.ellipsis);
        out
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::plain()
    }
}

/// The available themes: built-ins, then user themes (a user theme with a
/// built-in's name replaces it).
#[derive(Debug, Clone)]
pub struct ThemeSet {
    themes: Vec<Theme>,
}

impl ThemeSet {
    /// Built-in themes only.
    pub fn builtin() -> Self {
        let themes = BUILTIN
            .iter()
            .filter_map(|src| Theme::from_toml(src, "built-in").ok())
            .collect();
        Self { themes }
    }

    /// Built-ins plus every `*.toml` in `dir` (missing dir = none). Broken
    /// files are skipped and reported, never fatal.
    pub fn load(dir: &Path) -> (Self, Vec<ThemeError>) {
        let mut set = Self::builtin();
        let mut errors = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                    .collect()
            })
            .unwrap_or_default();
        paths.sort();
        for path in paths {
            let origin = path.display().to_string();
            let theme = std::fs::read_to_string(&path)
                .map_err(|e| ThemeError {
                    origin: origin.clone(),
                    detail: e.to_string(),
                })
                .and_then(|text| Theme::from_toml(&text, &origin));
            match theme {
                Ok(t) => match set.themes.iter_mut().find(|x| x.name == t.name) {
                    Some(slot) => *slot = t,
                    None => set.themes.push(t),
                },
                Err(e) => errors.push(e),
            }
        }
        (set, errors)
    }

    /// Theme names in cycling order.
    pub fn names(&self) -> Vec<&str> {
        self.themes.iter().map(|t| t.name.as_str()).collect()
    }

    /// Index of the theme called `name`.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.themes.iter().position(|t| t.name == name)
    }

    /// The theme at `index`, wrapping around (for cycling).
    pub fn get(&self, index: usize) -> &Theme {
        &self.themes[index % self.themes.len()]
    }

    /// Number of themes (at least 1).
    pub fn len(&self) -> usize {
        self.themes.len()
    }

    /// Never empty (the built-ins are always there).
    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtins_parse() {
        for src in BUILTIN {
            Theme::from_toml(src, "built-in").unwrap();
        }
        let set = ThemeSet::builtin();
        assert_eq!(
            set.names(),
            [
                "plain",
                "pdp11-vt100",
                "pdp11-vt100-amber",
                "pdp11-vt100-green",
                "pdp11-teletype",
                "zx80",
                "zx-spectrum",
                "bbc-mode7"
            ]
        );
    }

    #[test]
    fn builtins_fall_back_to_16_colours_and_mono() {
        let set = ThemeSet::builtin();
        for i in 0..set.len() {
            let t16 = set.get(i).for_depth(ColorDepth::Ansi16);
            let mono = set.get(i).for_depth(ColorDepth::Mono);
            let p = &t16.palette;
            for c in [
                p.fg,
                p.bg,
                p.accent,
                p.dim,
                p.warn,
                p.error,
                t16.screen.border,
            ] {
                assert!(
                    matches!(
                        c,
                        ThemeColor::Reset | ThemeColor::Ansi(_) | ThemeColor::Faint
                    ),
                    "{}: {c:?} at 16 colours",
                    t16.name
                );
            }
            let p = &mono.palette;
            for c in [
                p.fg,
                p.bg,
                p.accent,
                p.dim,
                p.warn,
                p.error,
                mono.screen.border,
            ] {
                assert!(
                    matches!(c, ThemeColor::Reset | ThemeColor::Faint),
                    "{}: {c:?} in mono",
                    mono.name
                );
            }
        }
    }

    #[test]
    fn retro_rules_status_and_mosaic_fallback() {
        let set = ThemeSet::builtin();
        let get = |n: &str| set.get(set.position(n).unwrap());
        let vt = get("pdp11-vt100");
        assert_eq!(vt.number(80), "120");
        assert_eq!(vt.percent(100), "144");
        assert_eq!(vt.text("Lauren Laverne"), "LAUREN LAVERNE");
        assert_eq!(
            vt.status_line(StatusKind::Error, "?", "HTTP 404", None),
            "?TTNR-F-HTTP 404"
        );
        assert_eq!(
            vt.status_line(StatusKind::Buffering, "▒", "", Some(64)),
            "▒ BUFFERING 100",
            "percent in octal"
        );
        let zx = get("zx80");
        assert_eq!(zx.text("Björk"), "BJORK");
        assert_eq!(zx.status_line(StatusKind::Paused, "", "", None), "9/0");
        let bbc = get("bbc-mode7");
        assert_eq!(bbc.text("#1 [new]"), "£1 ←new→");
        assert_eq!(bbc.chrome.rule.as_deref(), Some("🬭"));
        assert_eq!(bbc.with_mosaic_fallback().chrome.rule.as_deref(), Some("▄"));
        let plain = get("plain");
        assert_eq!(plain.with_mosaic_fallback(), plain.clone());
        assert_eq!(
            plain.status_line(StatusKind::Playing, "▶", "", None),
            "▶ playing"
        );
    }

    #[test]
    fn plain_is_embedded_and_valid() {
        let t = Theme::plain();
        assert_eq!(t.name, "plain");
        assert_eq!(t.palette.accent, ThemeColor::Ansi(6));
        assert_eq!(t.glyphs.borders.as_ref().unwrap().top_left, "┌");
    }

    #[test]
    fn fit_truncates_with_ellipsis() {
        let t = Theme::plain();
        assert_eq!(t.fit("Groove Salad", 20), "Groove Salad");
        assert_eq!(t.fit("Groove Salad", 8), "Groove …");
        assert_eq!(t.fit("Groove Salad", 1), "…");
        assert_eq!(t.fit("Groove Salad", 0), "");
        assert_eq!(t.fit("华语金曲500首", 7), "华语金…");
        assert_eq!(t.fit("华语", 4), "华语");
    }

    #[test]
    fn uppercase_rule() {
        let mut t = Theme::plain();
        t.text_rules.uppercase = true;
        assert_eq!(t.text("Żółw – song"), "ŻÓŁW – SONG");
        assert_eq!(t.fit("abcdef", 4), "ABC…");
    }

    const AMBER: &str = r##"
name = "amber"
[palette]
fg = "#ffb000"
bg = "black"
accent = "#ffd000"
dim = 136
warn = "yellow"
error = "red"
[palette_16]
fg = "yellow"
[glyphs]
borders = "ascii"
ellipsis = "..."
dash = " - "
separator = " | "
playing = ">"
paused = "="
busy = "~"
stopped = "."
error = "!"
current = "*"
cursor = ">"
"##;

    #[test]
    fn depth_resolution_with_overrides() {
        let t = Theme::from_toml(AMBER, "test").unwrap();
        assert_eq!(t.title, "ttnr");
        assert_eq!(t.glyphs.borders.as_ref().unwrap().horizontal, "-");
        assert_eq!(
            t.for_depth(ColorDepth::TrueColor).palette.fg,
            ThemeColor::Rgb(0xff, 0xb0, 0)
        );
        assert_eq!(
            t.for_depth(ColorDepth::Ansi256).palette.fg,
            ThemeColor::Indexed(214)
        );
        assert_eq!(
            t.for_depth(ColorDepth::Ansi256).palette.dim,
            ThemeColor::Indexed(136)
        );
        let p16 = t.for_depth(ColorDepth::Ansi16).palette;
        assert_eq!(p16.fg, ThemeColor::Ansi(3), "override wins at 16 colours");
        assert!(matches!(p16.accent, ThemeColor::Ansi(_)));
        assert_eq!(t.for_depth(ColorDepth::Mono).palette.fg, ThemeColor::Reset);
    }

    #[test]
    fn bad_theme_files() {
        for (text, needle) in [
            ("name = 'x'\n", "missing field `palette`"),
            (
                &AMBER.replace("\"ascii\"", "\"wavy\""),
                "unknown borders preset `wavy`",
            ),
            (
                &AMBER.replace("\"#ffb000\"", "\"teal\""),
                "unknown colour `teal`",
            ),
            (
                &AMBER.replace("name = \"amber\"", "name = \"amber\"\nfont = \"x\""),
                "unknown field `font`",
            ),
        ] {
            let e = Theme::from_toml(text, "my.toml").unwrap_err().to_string();
            assert!(
                e.starts_with("theme my.toml: ") && e.contains(needle),
                "{e}"
            );
        }
    }

    #[test]
    fn set_loads_user_themes() {
        let dir = std::env::temp_dir().join(format!("ttnr-themes-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("amber.toml"), AMBER).unwrap();
        std::fs::write(
            dir.join("plain.toml"),
            AMBER.replace("name = \"amber\"", "name = \"plain\""),
        )
        .unwrap();
        std::fs::write(dir.join("broken.toml"), "name = 1").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let (set, errors) = ThemeSet::load(&dir);
        let names = set.names();
        assert_eq!(names.first().copied(), Some("plain"));
        assert_eq!(names.last().copied(), Some("amber"));
        assert_eq!(names.len(), ThemeSet::builtin().len() + 1);
        assert_eq!(
            set.get(0).glyphs.ellipsis,
            "...",
            "user plain replaced the built-in"
        );
        assert_eq!(set.get(names.len()).name, "plain", "index wraps");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].origin.ends_with("broken.toml"));
        std::fs::remove_dir_all(&dir).unwrap();
        let (set, errors) = ThemeSet::load(Path::new("/nonexistent"));
        assert_eq!((set.len(), errors.len()), (ThemeSet::builtin().len(), 0));
    }
}
