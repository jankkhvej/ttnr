//! `config.toml` (`$XDG_CONFIG_HOME/ttnr/config.toml`, SPEC F8). Missing file
//! = defaults; see `examples/config.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

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

/// The whole configuration (grows with later milestones).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Key bindings: action → key(s); replaces that action's defaults.
    pub keys: BTreeMap<String, Keys>,
}

impl Config {
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
