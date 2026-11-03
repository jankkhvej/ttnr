//! Keys → [`Command`]s: SPEC F5.3 defaults, remappable via `[keys]` in
//! `config.toml` (ADR-024). Navigation keys (arrows, Enter, Esc, j/k) and
//! Ctrl-C (always quits) are fixed.

use std::collections::{BTreeMap, HashMap};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ttnr_core::app::{Command, View};

/// A remappable action: config name, command, help text.
struct Action {
    name: &'static str,
    command: Command,
    help: &'static str,
    defaults: &'static [&'static str],
}

const ACTIONS: &[Action] = &[
    Action {
        name: "toggle_pause",
        command: Command::TogglePause,
        help: "play / pause",
        defaults: &["space"],
    },
    Action {
        name: "volume_up",
        command: Command::VolumeUp,
        help: "volume up",
        defaults: &["+", "="],
    },
    Action {
        name: "volume_down",
        command: Command::VolumeDown,
        help: "volume down",
        defaults: &["-"],
    },
    Action {
        name: "mute",
        command: Command::ToggleMute,
        help: "mute",
        defaults: &["m"],
    },
    Action {
        name: "next_station",
        command: Command::NextStation,
        help: "next station",
        defaults: &["n"],
    },
    Action {
        name: "prev_station",
        command: Command::PrevStation,
        help: "previous station",
        defaults: &["p"],
    },
    Action {
        name: "stations",
        command: Command::OpenStations,
        help: "station list",
        defaults: &["s"],
    },
    Action {
        name: "open_url",
        command: Command::OpenUrl,
        help: "open URL",
        defaults: &["o"],
    },
    Action {
        name: "devices",
        command: Command::OpenDevices,
        help: "output device",
        defaults: &["d"],
    },
    Action {
        name: "cycle_theme",
        command: Command::CycleTheme,
        help: "next theme",
        defaults: &["t"],
    },
    Action {
        name: "help",
        command: Command::ToggleHelp,
        help: "help",
        defaults: &["?"],
    },
    Action {
        name: "quit",
        command: Command::Quit,
        help: "quit",
        defaults: &["q"],
    },
];

/// Footer entries: (actions, short label), in display order; help and quit
/// always close the line.
const FOOTER: &[(&[&str], &str)] = &[
    (&["toggle_pause"], "play"),
    (&["volume_up", "volume_down"], "vol"),
    (&["mute"], "mute"),
    (&["next_station", "prev_station"], "station"),
    (&["stations"], "list"),
    (&["open_url"], "url"),
    (&["devices"], "device"),
    (&["cycle_theme"], "theme"),
];

/// A key with modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
}

impl Key {
    fn from_event(e: &KeyEvent) -> Self {
        // Shift is already folded into the character (`?`, `+`, `A`).
        Self {
            code: e.code,
            ctrl: e.modifiers.contains(KeyModifiers::CONTROL),
            alt: e.modifiers.contains(KeyModifiers::ALT),
        }
    }

    /// Parse `space`, `enter`, `tab`, `backspace`, `esc`, `up`, `f5`,
    /// `ctrl-x`, `alt-x`, or a single character.
    pub fn parse(s: &str) -> Result<Self, KeymapError> {
        let bad = || KeymapError::BadKey(s.to_owned());
        let mut rest = s.trim();
        let (mut ctrl, mut alt) = (false, false);
        loop {
            let lower = rest.to_ascii_lowercase();
            if let Some(r) = lower.strip_prefix("ctrl-").filter(|r| !r.is_empty()) {
                ctrl = true;
                rest = &rest[rest.len() - r.len()..];
            } else if let Some(r) = lower.strip_prefix("alt-").filter(|r| !r.is_empty()) {
                alt = true;
                rest = &rest[rest.len() - r.len()..];
            } else {
                break;
            }
        }
        let code = match rest.to_ascii_lowercase().as_str() {
            "space" => KeyCode::Char(' '),
            "enter" | "return" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "backspace" => KeyCode::Backspace,
            "esc" | "escape" => KeyCode::Esc,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            f if f.len() > 1 && f.starts_with('f') => KeyCode::F(
                f[1..]
                    .parse()
                    .ok()
                    .filter(|n| (1..=24).contains(n))
                    .ok_or_else(bad)?,
            ),
            _ => {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    // Terminals report ctrl/alt + letter in lower case.
                    (Some(c), None) if ctrl || alt => KeyCode::Char(c.to_ascii_lowercase()),
                    (Some(c), None) => KeyCode::Char(c),
                    _ => return Err(bad()),
                }
            }
        };
        Ok(Self { code, ctrl, alt })
    }

    /// Short label for help texts (`spc`, `ctrl-x`, `q`).
    pub fn label(&self) -> String {
        let base = match self.code {
            KeyCode::Char(' ') => "spc".to_owned(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "enter".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::Backspace => "bksp".into(),
            KeyCode::Esc => "esc".into(),
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::Home => "home".into(),
            KeyCode::End => "end".into(),
            KeyCode::PageUp => "pgup".into(),
            KeyCode::PageDown => "pgdn".into(),
            KeyCode::F(n) => format!("f{n}"),
            other => format!("{other:?}").to_lowercase(),
        };
        match (self.ctrl, self.alt) {
            (true, _) => format!("ctrl-{base}"),
            (_, true) => format!("alt-{base}"),
            _ => base,
        }
    }
}

/// Invalid `[keys]` configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeymapError {
    /// Unknown action name.
    #[error("unknown action `{0}` in [keys] (known: {1})")]
    UnknownAction(String, String),
    /// Unparsable key.
    #[error("cannot understand key `{0}` in [keys]")]
    BadKey(String),
    /// A key bound twice.
    #[error("key `{key}` is bound to both `{first}` and `{second}` in [keys]")]
    Conflict {
        /// The key.
        key: String,
        /// One action.
        first: String,
        /// The other action.
        second: String,
    },
    /// A key reserved for navigation or quitting.
    #[error("key `{0}` is reserved (navigation / ctrl-c) and cannot be remapped")]
    Reserved(String),
}

/// The active key bindings.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: HashMap<Key, usize>,
    keys: Vec<Vec<Key>>, // per ACTIONS index, in config order
}

impl Default for Keymap {
    fn default() -> Self {
        Self::with_overrides(&BTreeMap::new())
            .unwrap_or_else(|e| unreachable!("defaults are valid: {e}"))
    }
}

impl Keymap {
    /// Defaults with `overrides` (action → keys) replacing an action's keys.
    pub fn with_overrides(overrides: &BTreeMap<String, Vec<String>>) -> Result<Self, KeymapError> {
        for name in overrides.keys() {
            if !ACTIONS.iter().any(|a| a.name == name) {
                let known: Vec<_> = ACTIONS.iter().map(|a| a.name).collect();
                return Err(KeymapError::UnknownAction(name.clone(), known.join(", ")));
            }
        }
        let mut bindings: HashMap<Key, usize> = HashMap::new();
        let mut keys = Vec::with_capacity(ACTIONS.len());
        for (i, action) in ACTIONS.iter().enumerate() {
            let specs: Vec<String> = match overrides.get(action.name) {
                Some(list) => list.clone(),
                None => action.defaults.iter().map(|s| (*s).to_owned()).collect(),
            };
            let mut list = Vec::new();
            for spec in &specs {
                let key = Key::parse(spec)?;
                if is_reserved(&key) {
                    return Err(KeymapError::Reserved(spec.clone()));
                }
                if let Some(&other) = bindings.get(&key)
                    && other != i
                {
                    return Err(KeymapError::Conflict {
                        key: spec.clone(),
                        first: ACTIONS[other].name.to_owned(),
                        second: action.name.to_owned(),
                    });
                }
                bindings.insert(key, i);
                list.push(key);
            }
            keys.push(list);
        }
        Ok(Self { bindings, keys })
    }

    /// The command for `key` in `view`, if any. In the open-URL prompt
    /// printable keys are text.
    pub fn command_for(&self, key: KeyEvent, view: &View) -> Option<Command> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        let k = Key::from_event(&key);
        if k.ctrl && k.code == KeyCode::Char('c') {
            return Some(Command::Quit); // raw mode: Ctrl-C is a key
        }
        let list = matches!(view, View::Stations { .. } | View::Devices { .. });
        let prompt = matches!(view, View::OpenUrl { .. });
        match key.code {
            KeyCode::Char(c) if prompt && !k.ctrl && !k.alt => return Some(Command::Input(c)),
            KeyCode::Backspace if prompt => return Some(Command::Backspace),
            KeyCode::Enter => return Some(Command::Select),
            KeyCode::Esc => return Some(Command::Back),
            KeyCode::Up => return Some(Command::Up),
            KeyCode::Down => return Some(Command::Down),
            KeyCode::Char('k') if list && !k.ctrl && !k.alt => return Some(Command::Up),
            KeyCode::Char('j') if list && !k.ctrl && !k.alt => return Some(Command::Down),
            _ if prompt => return None,
            _ => {}
        }
        self.bindings.get(&k).map(|&i| ACTIONS[i].command.clone())
    }

    fn keys_of(&self, name: &str) -> &[Key] {
        ACTIONS
            .iter()
            .position(|a| a.name == name)
            .map_or(&[], |i| self.keys[i].as_slice())
    }

    fn label_of(&self, names: &[&str]) -> Option<String> {
        let labels: Vec<String> = names
            .iter()
            .filter_map(|n| self.keys_of(n).first().map(Key::label))
            .collect();
        (!labels.is_empty()).then(|| labels.join(if names.len() > 1 { "/" } else { "" }))
    }

    /// Help overlay rows: (keys, description).
    pub fn help(&self) -> Vec<(String, &'static str)> {
        ACTIONS
            .iter()
            .zip(&self.keys)
            .filter(|(_, keys)| !keys.is_empty())
            .map(|(a, keys)| {
                let labels: Vec<String> = keys.iter().map(Key::label).collect();
                (labels.join(" "), a.help)
            })
            .collect()
    }

    /// Footer entries (`spc play`, `+/- vol`, …) and the closing tail
    /// (`? help`, `q quit`).
    pub fn footer(&self) -> (Vec<String>, Vec<String>) {
        let entries = FOOTER
            .iter()
            .filter_map(|(names, label)| self.label_of(names).map(|k| format!("{k} {label}")))
            .collect();
        let tail = [("help", "help"), ("quit", "quit")]
            .iter()
            .filter_map(|(n, label)| self.label_of(&[n]).map(|k| format!("{k} {label}")))
            .collect();
        (entries, tail)
    }
}

/// Keys that keep their fixed meaning.
fn is_reserved(k: &Key) -> bool {
    (!k.ctrl
        && !k.alt
        && matches!(
            k.code,
            KeyCode::Enter | KeyCode::Esc | KeyCode::Up | KeyCode::Down
        ))
        || (k.ctrl && k.code == KeyCode::Char('c'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn overrides(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(a, ks)| {
                (
                    (*a).to_owned(),
                    ks.iter().map(|k| (*k).to_owned()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn defaults() {
        let km = Keymap::default();
        let main = View::Main;
        assert_eq!(
            km.command_for(key(KeyCode::Char(' ')), &main),
            Some(Command::TogglePause)
        );
        assert_eq!(
            km.command_for(key(KeyCode::Char('=')), &main),
            Some(Command::VolumeUp)
        );
        assert_eq!(
            km.command_for(key(KeyCode::Char('q')), &main),
            Some(Command::Quit)
        );
        assert_eq!(km.command_for(key(KeyCode::Char('x')), &main), None);
        assert_eq!(
            km.command_for(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &main
            ),
            Some(Command::Quit)
        );
        // Shift is folded into the character.
        assert_eq!(
            km.command_for(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
                &main
            ),
            Some(Command::ToggleHelp)
        );
    }

    #[test]
    fn prompt_takes_text() {
        let km = Keymap::default();
        let prompt = View::OpenUrl {
            input: String::new(),
        };
        assert_eq!(
            km.command_for(key(KeyCode::Char('q')), &prompt),
            Some(Command::Input('q'))
        );
        assert_eq!(
            km.command_for(key(KeyCode::Backspace), &prompt),
            Some(Command::Backspace)
        );
        assert_eq!(
            km.command_for(key(KeyCode::Enter), &prompt),
            Some(Command::Select)
        );
        assert_eq!(
            km.command_for(key(KeyCode::Esc), &prompt),
            Some(Command::Back)
        );
    }

    #[test]
    fn list_navigation() {
        let km = Keymap::default();
        assert_eq!(
            km.command_for(key(KeyCode::Char('j')), &View::Stations { selected: 0 }),
            Some(Command::Down)
        );
        assert_eq!(km.command_for(key(KeyCode::Char('k')), &View::Main), None);
    }

    #[test]
    fn overrides_replace_defaults() {
        let km = Keymap::with_overrides(&overrides(&[
            ("quit", &["x", "ctrl-q"]),
            ("toggle_pause", &["p"]),
            ("prev_station", &["b"]),
        ]))
        .unwrap();
        let main = View::Main;
        assert_eq!(
            km.command_for(key(KeyCode::Char('q')), &main),
            None,
            "old key unbound"
        );
        assert_eq!(
            km.command_for(key(KeyCode::Char('x')), &main),
            Some(Command::Quit)
        );
        assert_eq!(
            km.command_for(
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
                &main
            ),
            Some(Command::Quit)
        );
        assert_eq!(
            km.command_for(key(KeyCode::Char('p')), &main),
            Some(Command::TogglePause)
        );
        assert_eq!(km.command_for(key(KeyCode::Char(' ')), &main), None);
        let (_, tail) = km.footer();
        assert_eq!(tail, ["? help", "x quit"]);
    }

    #[test]
    fn invalid_overrides() {
        let err = |o| Keymap::with_overrides(&o).unwrap_err().to_string();
        assert!(err(overrides(&[("jump", &["j"])])).contains("unknown action `jump`"));
        assert!(err(overrides(&[("quit", &["ctrl-"])])).contains("cannot understand key"));
        assert!(err(overrides(&[("quit", &["f99"])])).contains("cannot understand key"));
        assert!(err(overrides(&[("quit", &["m"])])).contains("bound to both `mute` and `quit`"));
        assert!(err(overrides(&[("quit", &["esc"])])).contains("reserved"));
        assert!(err(overrides(&[("quit", &["ctrl-c"])])).contains("reserved"));
    }

    #[test]
    fn parse_and_label() {
        for (spec, label) in [
            ("space", "spc"),
            ("Ctrl-X", "ctrl-x"),
            ("alt-1", "alt-1"),
            ("F5", "f5"),
            ("pageup", "pgup"),
            ("+", "+"),
        ] {
            assert_eq!(Key::parse(spec).unwrap().label(), label, "{spec}");
        }
        assert!(Key::parse("ab").is_err());
    }

    #[test]
    fn help_and_footer_follow_bindings() {
        let km = Keymap::default();
        let help = km.help();
        assert_eq!(help[0], ("spc".into(), "play / pause"));
        assert_eq!(help[1], ("+ =".into(), "volume up"));
        let (entries, tail) = km.footer();
        assert_eq!(entries[0], "spc play");
        assert_eq!(entries[1], "+/- vol");
        assert_eq!(tail, ["? help", "q quit"]);
    }
}
