//! Text-only theme engine for ttnr: theme model, built-in themes,
//! glyph/case transforms (TOML loader and colour-depth fallback arrive in
//! M3 task 4; see `docs/THEMES.md`).
//!
//! Views never hard-code colours or glyphs: they ask the [`Theme`].

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A colour as a theme specifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeColor {
    /// The terminal's own default.
    Reset,
    /// One of the 16 ANSI colours (0–15).
    Ansi(u8),
    /// 256-colour palette index.
    Indexed(u8),
    /// 24-bit colour.
    Rgb(u8, u8, u8),
}

/// Colour roles used by the views.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Border characters (corners, lines).
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// A complete theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Theme id, e.g. `plain`.
    pub name: String,
    /// Colours.
    pub palette: Palette,
    /// Characters.
    pub glyphs: Glyphs,
    /// Map all text to upper case (ZX80, Teletype).
    pub uppercase: bool,
    /// Banner/title shown in the frame.
    pub title: String,
}

impl Theme {
    /// The default theme: terminal colours, box-drawing borders.
    pub fn plain() -> Self {
        Self {
            name: "plain".into(),
            palette: Palette {
                fg: ThemeColor::Reset,
                bg: ThemeColor::Reset,
                accent: ThemeColor::Ansi(6),
                dim: ThemeColor::Ansi(8),
                warn: ThemeColor::Ansi(3),
                error: ThemeColor::Ansi(1),
            },
            glyphs: Glyphs {
                borders: Some(Borders {
                    top_left: "┌".into(),
                    top_right: "┐".into(),
                    bottom_left: "└".into(),
                    bottom_right: "┘".into(),
                    horizontal: "─".into(),
                    vertical: "│".into(),
                }),
                ellipsis: "…".into(),
                dash: " – ".into(),
                separator: " · ".into(),
                playing: "▶".into(),
                paused: "‖".into(),
                busy: "…".into(),
                stopped: "■".into(),
                error: "✕".into(),
                current: "•".into(),
                cursor: "›".into(),
            },
            uppercase: false,
            title: "ttnr".into(),
        }
    }

    /// Apply the theme's case rule to `text`.
    pub fn text(&self, text: &str) -> String {
        if self.uppercase {
            text.to_uppercase()
        } else {
            text.to_owned()
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_truncates_with_ellipsis() {
        let t = Theme::plain();
        assert_eq!(t.fit("Groove Salad", 20), "Groove Salad");
        assert_eq!(t.fit("Groove Salad", 8), "Groove …");
        assert_eq!(t.fit("Groove Salad", 1), "…");
        assert_eq!(t.fit("Groove Salad", 0), "");
        // Wide characters take two columns.
        assert_eq!(t.fit("华语金曲500首", 7), "华语金…");
        assert_eq!(t.fit("华语", 4), "华语");
    }

    #[test]
    fn uppercase_rule() {
        let t = Theme {
            uppercase: true,
            ..Theme::plain()
        };
        assert_eq!(t.text("Żółw – song"), "ŻÓŁW – SONG");
        assert_eq!(t.fit("abcdef", 4), "ABC…");
    }
}
