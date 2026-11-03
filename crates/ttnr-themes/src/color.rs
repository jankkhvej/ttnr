//! Colours: theme syntax, terminal colour depth, and downgrading
//! (truecolor → 256 → 16 → mono; SPEC F6.4).

use serde::Deserialize;

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
    /// The terminal's default colour drawn faint (SGR 2): secondary text
    /// that suits any terminal colour scheme.
    Faint,
}

const NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

/// xterm's default RGB values for the 16 ANSI colours.
const ANSI_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

impl ThemeColor {
    /// Parse the theme syntax: `default`/`reset`, an ANSI name, `#rrggbb`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim().to_ascii_lowercase();
        if t == "default" || t == "reset" {
            return Ok(Self::Reset);
        }
        if t == "faint" {
            return Ok(Self::Faint);
        }
        if let Some(i) = NAMES.iter().position(|n| *n == t) {
            return Ok(Self::Ansi(u8::try_from(i).unwrap_or(0)));
        }
        if let Some(hex) = t.strip_prefix('#')
            && hex.len() == 6
            && let Ok(v) = u32::from_str_radix(hex, 16)
        {
            let [_, r, g, b] = v.to_be_bytes();
            return Ok(Self::Rgb(r, g, b));
        }
        Err(format!(
            "unknown colour `{s}` (use default, faint, an ANSI name, #rrggbb or 0–255)"
        ))
    }

    /// This colour as the terminal at `depth` can show it.
    pub fn downgrade(self, depth: ColorDepth) -> Self {
        match (self, depth) {
            (Self::Faint, _) => Self::Faint,
            (_, ColorDepth::Mono) | (Self::Reset, _) => Self::Reset,
            (c, ColorDepth::TrueColor) => c,
            (Self::Rgb(r, g, b), ColorDepth::Ansi256) => Self::Indexed(rgb_to_256(r, g, b)),
            (c, ColorDepth::Ansi256) => c,
            (Self::Ansi(n), ColorDepth::Ansi16) => Self::Ansi(n),
            (Self::Indexed(n), ColorDepth::Ansi16) => {
                let (r, g, b) = index_to_rgb(n);
                Self::Ansi(rgb_to_16(r, g, b))
            }
            (Self::Rgb(r, g, b), ColorDepth::Ansi16) => Self::Ansi(rgb_to_16(r, g, b)),
        }
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Index(u8),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Index(i) if i < 16 => Ok(Self::Ansi(i)),
            Raw::Index(i) => Ok(Self::Indexed(i)),
            Raw::Text(s) => Self::parse(&s).map_err(serde::de::Error::custom),
        }
    }
}

fn dist((r1, g1, b1): (u8, u8, u8), (r2, g2, b2): (u8, u8, u8)) -> u32 {
    let d = |a: u8, b: u8| u32::from(a.abs_diff(b)).pow(2);
    d(r1, r2) + d(g1, g2) + d(b1, b2)
}

/// RGB of a 256-colour index (xterm palette).
fn index_to_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..=15 => ANSI_RGB[usize::from(n)],
        16..=231 => {
            let i = n - 16;
            (
                CUBE[usize::from(i / 36)],
                CUBE[usize::from(i / 6 % 6)],
                CUBE[usize::from(i % 6)],
            )
        }
        _ => {
            let v = 8 + (n - 232) * 10;
            (v, v, v)
        }
    }
}

/// Nearest xterm 256-colour index (cube or grey ramp; never 0–15, whose
/// RGB values vary between terminals).
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    let nearest = |v: u8| (0..6).min_by_key(|&i| CUBE[i].abs_diff(v)).unwrap_or(0);
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let cube = u8::try_from(cube).unwrap_or(16);
    let avg = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let grey_step = (avg.saturating_sub(3) / 10).min(23);
    let grey = 232 + u8::try_from(grey_step).unwrap_or(0);
    if dist((r, g, b), index_to_rgb(grey)) < dist((r, g, b), index_to_rgb(cube)) {
        grey
    } else {
        cube
    }
}

/// Nearest of the 16 ANSI colours.
fn rgb_to_16(r: u8, g: u8, b: u8) -> u8 {
    (0u8..16)
        .min_by_key(|&i| dist((r, g, b), ANSI_RGB[usize::from(i)]))
        .unwrap_or(7)
}

/// How many colours the terminal shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorDepth {
    /// No colour (`NO_COLOR`, dumb terminals).
    Mono,
    /// 16 ANSI colours.
    #[serde(rename = "16")]
    Ansi16,
    /// 256-colour palette.
    #[serde(rename = "256")]
    Ansi256,
    /// 24-bit colour.
    TrueColor,
}

impl ColorDepth {
    /// Detect from the environment (looked up with `var`): `NO_COLOR` →
    /// mono; `COLORTERM=truecolor|24bit` → truecolor; `TERM=dumb` → mono;
    /// `TERM` with `256color`, `kitty`, or Terminal.app → 256; else 16.
    /// (docs/RESEARCH.md; Terminal.app's truecolor support since macOS 26
    /// is not detectable reliably – use the `color` setting.)
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Self {
        if var("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Self::Mono;
        }
        if var("COLORTERM")
            .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "truecolor" | "24bit"))
        {
            return Self::TrueColor;
        }
        let term = var("TERM").unwrap_or_default().to_ascii_lowercase();
        if term.is_empty() || term == "dumb" {
            return Self::Mono;
        }
        if term.contains("256color")
            || term.contains("kitty")
            || var("TERM_PROGRAM").as_deref() == Some("Apple_Terminal")
        {
            return Self::Ansi256;
        }
        Self::Ansi16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_syntax() {
        assert_eq!(ThemeColor::parse("Default"), Ok(ThemeColor::Reset));
        assert_eq!(ThemeColor::parse("faint"), Ok(ThemeColor::Faint));
        assert_eq!(
            ThemeColor::Faint.downgrade(ColorDepth::Mono),
            ThemeColor::Faint
        );
        assert_eq!(ThemeColor::parse("cyan"), Ok(ThemeColor::Ansi(6)));
        assert_eq!(ThemeColor::parse("bright_black"), Ok(ThemeColor::Ansi(8)));
        assert_eq!(
            ThemeColor::parse("#33FF00"),
            Ok(ThemeColor::Rgb(0x33, 0xFF, 0))
        );
        assert!(ThemeColor::parse("#12345").is_err());
        assert!(ThemeColor::parse("teal").is_err());
    }

    #[test]
    fn downgrade_chain() {
        let amber = ThemeColor::Rgb(255, 176, 0);
        assert_eq!(amber.downgrade(ColorDepth::TrueColor), amber);
        assert_eq!(
            amber.downgrade(ColorDepth::Ansi256),
            ThemeColor::Indexed(214)
        );
        assert_eq!(amber.downgrade(ColorDepth::Ansi16), ThemeColor::Ansi(3));
        assert_eq!(amber.downgrade(ColorDepth::Mono), ThemeColor::Reset);
        assert_eq!(
            ThemeColor::Rgb(128, 128, 128).downgrade(ColorDepth::Ansi256),
            ThemeColor::Indexed(244)
        );
        assert_eq!(
            ThemeColor::Indexed(196).downgrade(ColorDepth::Ansi16),
            ThemeColor::Ansi(9)
        );
        assert_eq!(
            ThemeColor::Ansi(6).downgrade(ColorDepth::Ansi16),
            ThemeColor::Ansi(6)
        );
        assert_eq!(
            ThemeColor::Reset.downgrade(ColorDepth::Ansi16),
            ThemeColor::Reset
        );
    }

    #[test]
    fn detection() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(
            ColorDepth::detect(env(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")])),
            ColorDepth::Mono
        );
        assert_eq!(
            ColorDepth::detect(env(&[
                ("NO_COLOR", ""),
                ("COLORTERM", "truecolor"),
                ("TERM", "xterm-kitty")
            ])),
            ColorDepth::TrueColor
        );
        assert_eq!(
            ColorDepth::detect(env(&[("TERM", "xterm-256color")])),
            ColorDepth::Ansi256
        );
        assert_eq!(
            ColorDepth::detect(env(&[
                ("TERM", "xterm-256color"),
                ("TERM_PROGRAM", "Apple_Terminal")
            ])),
            ColorDepth::Ansi256
        );
        assert_eq!(
            ColorDepth::detect(env(&[("TERM", "xterm")])),
            ColorDepth::Ansi16
        );
        assert_eq!(
            ColorDepth::detect(env(&[("TERM", "dumb")])),
            ColorDepth::Mono
        );
        assert_eq!(ColorDepth::detect(env(&[])), ColorDepth::Mono);
    }
}
