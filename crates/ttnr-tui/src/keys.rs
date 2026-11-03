//! Keys → [`Command`]s (SPEC F5.3 defaults).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ttnr_core::app::{Command, View};

/// Default bindings for the help overlay: (keys, description).
pub const HELP: &[(&str, &str)] = &[
    ("space", "play / pause"),
    ("+ -", "volume up / down"),
    ("m", "mute"),
    ("n p", "next / previous station"),
    ("s", "station list"),
    ("o", "open URL"),
    ("d", "output device"),
    ("t", "next theme"),
    ("?", "help"),
    ("q", "quit"),
];

/// Key help for the main view's bottom line: as many of these as fit,
/// then always [`FOOTER_TAIL`].
pub const FOOTER: &[&str] = &[
    "spc play",
    "+- vol",
    "m mute",
    "n/p station",
    "s list",
    "o url",
    "d device",
    "t theme",
];

/// Always shown at the end of the key help line.
pub const FOOTER_TAIL: &str = "? help · q quit";

/// The command for `key` in `view`, if any. In the open-URL prompt
/// printable keys are text.
pub fn command_for(key: KeyEvent, view: &View) -> Option<Command> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            // Raw mode: Ctrl-C arrives as a key.
            KeyCode::Char('c') => Some(Command::Quit),
            _ => None,
        };
    }
    let list = matches!(view, View::Stations { .. } | View::Devices { .. });
    match (view, key.code) {
        (View::OpenUrl { .. }, KeyCode::Char(c)) => Some(Command::Input(c)),
        (View::OpenUrl { .. }, KeyCode::Backspace) => Some(Command::Backspace),
        (_, KeyCode::Enter) => Some(Command::Select),
        (_, KeyCode::Esc) => Some(Command::Back),
        (_, KeyCode::Up) => Some(Command::Up),
        (_, KeyCode::Down) => Some(Command::Down),
        (_, KeyCode::Char('k')) if list => Some(Command::Up),
        (_, KeyCode::Char('j')) if list => Some(Command::Down),
        (View::OpenUrl { .. }, _) => None,
        (_, KeyCode::Char(c)) => match c {
            ' ' => Some(Command::TogglePause),
            '+' | '=' => Some(Command::VolumeUp),
            '-' => Some(Command::VolumeDown),
            'm' => Some(Command::ToggleMute),
            'n' => Some(Command::NextStation),
            'p' => Some(Command::PrevStation),
            's' => Some(Command::OpenStations),
            'o' => Some(Command::OpenUrl),
            'd' => Some(Command::OpenDevices),
            't' => Some(Command::CycleTheme),
            '?' => Some(Command::ToggleHelp),
            'q' => Some(Command::Quit),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn defaults() {
        let main = View::Main;
        assert_eq!(
            command_for(key(KeyCode::Char(' ')), &main),
            Some(Command::TogglePause)
        );
        assert_eq!(
            command_for(key(KeyCode::Char('q')), &main),
            Some(Command::Quit)
        );
        assert_eq!(command_for(key(KeyCode::Char('x')), &main), None);
        assert_eq!(
            command_for(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &main
            ),
            Some(Command::Quit)
        );
    }

    #[test]
    fn prompt_takes_text() {
        let prompt = View::OpenUrl {
            input: String::new(),
        };
        assert_eq!(
            command_for(key(KeyCode::Char('q')), &prompt),
            Some(Command::Input('q'))
        );
        assert_eq!(
            command_for(key(KeyCode::Backspace), &prompt),
            Some(Command::Backspace)
        );
        assert_eq!(
            command_for(key(KeyCode::Enter), &prompt),
            Some(Command::Select)
        );
        assert_eq!(command_for(key(KeyCode::Esc), &prompt), Some(Command::Back));
    }

    #[test]
    fn list_navigation() {
        let list = View::Stations { selected: 0 };
        assert_eq!(
            command_for(key(KeyCode::Char('j')), &list),
            Some(Command::Down)
        );
        assert_eq!(command_for(key(KeyCode::Char('k')), &View::Main), None);
    }
}
