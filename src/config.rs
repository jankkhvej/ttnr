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

/// One external-command source.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecConfig {
    /// Program and arguments; `~/` and paths relative to the config
    /// directory are expanded for the program.
    pub command: Vec<String>,
}

/// Source ids built into ttnr (an exec source may not reuse them).
pub const BUILTIN_SOURCES: &[&str] = &["url", "stations", "bbc", "radio-browser"];

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
    /// Theme animations (boot, tape loading, screen blanking); default on.
    pub effects: Option<bool>,
    /// Simulated line speed for printout themes (bits/s, e.g. 110, 300,
    /// 9600); 0 or unset = print instantly.
    pub baud: Option<u32>,
    /// Pinned output device at start (its name in the device picker, or
    /// mpv's device name); unset = follow the system default.
    pub output_device: Option<String>,
    /// Exclusive (hog) output mode – other apps move to another output
    /// while ttnr plays (default off).
    pub exclusive: Option<bool>,
    /// Switch the device's sample rate to the stream's (default off).
    pub match_sample_rate: Option<bool>,
    /// External-command sources (`[sources.<id>] command = [...]`, the
    /// `exec` protocol in docs/SOURCES.md).
    pub sources: BTreeMap<String, ExecConfig>,
    /// macOS Now Playing / Control Center and media keys; default on.
    pub media_controls: Option<bool>,
    /// Key bindings: action → key(s); replaces that action's defaults.
    pub keys: BTreeMap<String, Keys>,
    /// Delay (seconds) for external now-playing per source, e.g. `bbc = 25`.
    pub sync_offset: BTreeMap<String, u32>,
}

/// `~/x` → `$HOME/x`; `./x` or `x/y` → relative to the config directory;
/// a bare name (`python3`) is left for `PATH`.
fn expand_program(program: &str, config_dir: Option<&std::path::Path>) -> String {
    if let Some(rest) = program.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return std::path::Path::new(&home)
            .join(rest)
            .to_string_lossy()
            .into_owned();
    }
    let relative = !program.starts_with('/') && program.contains('/');
    match (relative, config_dir) {
        (true, Some(dir)) => dir.join(program).to_string_lossy().into_owned(),
        _ => program.to_owned(),
    }
}

/// Built-in sync offsets (seconds) per source, overridable in config.
/// None: by the owner's choice track info is shown as soon as it is known
/// (ADR-032); the offset is opt-in.
pub const DEFAULT_SYNC_OFFSETS: &[(&str, u32)] = &[];

impl Config {
    /// Apply the audio options (`exclusive`, `match_sample_rate`) to `cfg`.
    pub fn apply_audio(&self, cfg: &mut ttnr_engine::EngineConfig) {
        cfg.exclusive = self.exclusive.unwrap_or(false);
        cfg.match_sample_rate = self.match_sample_rate.unwrap_or(false);
    }

    /// The configured external-command sources, validated, with the
    /// program path expanded (`~/…`, relative to `config_dir`).
    pub fn exec_sources(
        &self,
        config_dir: Option<&std::path::Path>,
    ) -> anyhow::Result<Vec<ttnr_sources::ExecSource>> {
        let mut out = Vec::new();
        for (id, c) in &self.sources {
            if BUILTIN_SOURCES.contains(&id.as_str()) {
                anyhow::bail!("[sources.{id}]: `{id}` is a built-in source id");
            }
            let mut command = c.command.clone();
            if let Some(program) = command.first_mut() {
                *program = expand_program(program, config_dir);
            }
            out.push(
                ttnr_sources::ExecSource::new(id, command)
                    .map_err(|e| anyhow::anyhow!("[sources.{id}]: {e}"))?,
            );
        }
        Ok(out)
    }

    /// Whether to integrate with Now Playing and media keys (default true).
    pub fn media_controls_enabled(&self) -> bool {
        self.media_controls.unwrap_or(true)
    }

    /// Whether theme animations run (`effects`, default true).
    pub fn effects_enabled(&self) -> bool {
        self.effects.unwrap_or(true)
    }

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
    fn exec_sources_are_validated_and_paths_expanded() {
        let c: Config = toml::from_str(
            "[sources.mine]\ncommand = [\"scripts/mine.py\", \"--fast\"]\n[sources.sys]\ncommand = [\"python3\", \"x.py\"]\n",
        )
        .unwrap();
        let dir = std::path::Path::new("/cfg/ttnr");
        let list = c.exec_sources(Some(dir)).unwrap();
        let ids: Vec<_> = list.iter().map(|s| ttnr_core::Source::id(s)).collect();
        assert_eq!(ids, ["mine", "sys"]);
        assert_eq!(
            expand_program("scripts/mine.py", Some(dir)),
            "/cfg/ttnr/scripts/mine.py"
        );
        assert_eq!(
            expand_program("python3", Some(dir)),
            "python3",
            "PATH lookup"
        );
        assert_eq!(expand_program("/abs/x", Some(dir)), "/abs/x");
        let bad: Config = toml::from_str("[sources.bbc]\ncommand = [\"x\"]\n").unwrap();
        assert!(
            bad.exec_sources(None)
                .unwrap_err()
                .to_string()
                .contains("built-in")
        );
        let empty: Config = toml::from_str("[sources.e]\ncommand = []\n").unwrap();
        assert!(empty.exec_sources(None).is_err());
        assert!(
            toml::from_str::<Config>("[sources.x]\ncmd = [\"a\"]\n").is_err(),
            "unknown key"
        );
    }

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
