//! `config.toml` (`$XDG_CONFIG_HOME/ttnr/config.toml`, SPEC F8). Missing file
//! = defaults; see `examples/config.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use ttnr_themes::ColorDepth;

use crate::paths;

/// One key or a list of keys.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Keys {
    /// `quit = "q"`.
    One(String),
    /// `volume_up = ["+", "="]`.
    Many(Vec<String>),
}

/// Colour depth setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSetting {
    /// Detect from the environment.
    #[default]
    Auto,
    /// 24-bit.
    Truecolor,
    /// 256 colours.
    #[serde(rename = "256")]
    Ansi256,
    /// 16 colours.
    #[serde(rename = "16")]
    Ansi16,
    /// No colour.
    Mono,
}

impl ColorSetting {
    /// The depth to use (`auto` = detect from the environment).
    pub fn depth(self) -> ColorDepth {
        match self {
            Self::Auto => ColorDepth::detect(|k| std::env::var(k).ok()),
            Self::Truecolor => ColorDepth::TrueColor,
            Self::Ansi256 => ColorDepth::Ansi256,
            Self::Ansi16 => ColorDepth::Ansi16,
            Self::Mono => ColorDepth::Mono,
        }
    }
}

/// Mosaic glyphs: sextants (2×3, Unicode 13) or quadrant blocks (2×2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MosaicSetting {
    /// Quadrants in Terminal.app (its fonts lack sextants), else sextants.
    #[default]
    Auto,
    /// Always sextants.
    Sextant,
    /// Always quadrants.
    Quadrant,
}

impl MosaicSetting {
    /// Whether themes should use their quadrant fallback.
    pub fn use_quadrants(self) -> bool {
        match self {
            Self::Auto => std::env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal"),
            Self::Sextant => false,
            Self::Quadrant => true,
        }
    }
}

/// The whole configuration (grows with later milestones).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Theme name (built-in or from `themes/`); default `plain`.
    pub theme: Option<String>,
    /// Colour depth: `auto` (default), `truecolor`, `256`, `16`, `mono`.
    pub color: ColorSetting,
    /// Mosaic glyphs for retro themes: `auto` (default), `sextant`, `quadrant`.
    pub mosaic: MosaicSetting,
    /// Key bindings: action → key(s); replaces that action's defaults.
    pub keys: BTreeMap<String, Keys>,
    /// Delay (seconds) for external now-playing per source, e.g. `bbc = 25`.
    pub sync_offset: BTreeMap<String, u32>,
}

/// Built-in sync offsets (seconds) per source, overridable in config.
/// None: by the owner's choice track info is shown as soon as it is known
/// (ADR-032); the offset is opt-in.
pub const DEFAULT_SYNC_OFFSETS: &[(&str, u32)] = &[];

impl Config {
    /// Effective sync offsets: built-in defaults, then `[sync_offset]`.
    pub fn sync_offsets(&self) -> BTreeMap<String, std::time::Duration> {
        let secs = |s: u32| std::time::Duration::from_secs(u64::from(s));
        let mut out: BTreeMap<String, std::time::Duration> = DEFAULT_SYNC_OFFSETS
            .iter()
            .map(|(source, s)| ((*source).to_owned(), secs(*s)))
            .collect();
        for (source, s) in &self.sync_offset {
            out.insert(source.clone(), secs(*s));
        }
        out
    }

    /// `[keys]` as action → list of key names.
    pub fn key_overrides(&self) -> BTreeMap<String, Vec<String>> {
        self.keys
            .iter()
            .map(|(action, keys)| {
                let list = match keys {
                    Keys::One(k) => vec![k.clone()],
                    Keys::Many(ks) => ks.clone(),
                };
                (action.clone(), list)
            })
            .collect()
    }
}

/// Load the user's config (defaults if there is none).
pub fn load() -> Result<Config> {
    match paths::config_dir() {
        Some(dir) => load_from(&dir.join("config.toml")),
        None => Ok(Config::default()),
    }
}

/// Load `path`; a missing file is the default config.
pub fn load_from(path: &Path) -> Result<Config> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    toml::from_str(&text).with_context(|| format!("invalid {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_one_or_many() {
        let c: Config =
            toml::from_str("[keys]\nquit = \"x\"\nvolume_up = [\"+\", \"k\"]\n").unwrap();
        let o = c.key_overrides();
        assert_eq!(o["quit"], ["x"]);
        assert_eq!(o["volume_up"], ["+", "k"]);
    }

    #[test]
    fn theme_and_color() {
        let c: Config = toml::from_str("theme = \"amber\"\ncolor = \"256\"\n").unwrap();
        assert_eq!(c.theme.as_deref(), Some("amber"));
        assert_eq!(c.color.depth(), ColorDepth::Ansi256);
        assert!(toml::from_str::<Config>("color = \"8\"\n").is_err());
    }

    #[test]
    fn sync_offsets_from_config() {
        let c: Config = toml::from_str("[sync_offset]\nbbc = 25\n").unwrap();
        assert_eq!(c.sync_offsets()["bbc"], std::time::Duration::from_secs(25));
    }

    #[test]
    fn unknown_fields_are_errors() {
        assert!(toml::from_str::<Config>("colour = \"red\"\n").is_err());
    }

    #[test]
    fn missing_file_is_default() {
        assert_eq!(
            load_from(Path::new("/nonexistent/ttnr/config.toml")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn example_file_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config.toml");
        let c = load_from(&path).unwrap();
        ttnr_tui::Keymap::with_overrides(&c.key_overrides()).unwrap();
    }
}
