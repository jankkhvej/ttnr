//! Printout mode (Teletype themes): instead of redrawing in place, every
//! change is *printed* as a new line, like a terminal on paper.

use ttnr_core::app::AppState;
use ttnr_themes::Theme;

use crate::view::status_parts;

/// Most lines kept (older ones scroll off the paper).
const MAX_LINES: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Seen {
    station: Option<String>,
    status: String,
    programme: Option<String>,
    track: Option<String>,
    format: String,
    volume: (u8, bool),
}

/// The printed lines so far.
#[derive(Debug, Clone, Default)]
pub struct Printout {
    lines: Vec<String>,
    seen: Option<Seen>,
}

impl Printout {
    /// Start a printout with the theme's banner.
    pub fn new(theme: &Theme) -> Self {
        Self {
            lines: theme.chrome.banner.clone(),
            seen: None,
        }
    }

    /// Lines printed so far, oldest first.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Print what changed since the last observed state.
    pub fn observe(&mut self, s: &AppState, theme: &Theme) {
        let now = s.now.as_ref();
        let track = now.and_then(|n| match (&n.artist, &n.title) {
            (Some(a), Some(t)) => Some(format!("{a}{}{t}", theme.glyphs.dash)),
            (None, Some(t)) => Some(t.clone()),
            _ => None,
        });
        let next = Seen {
            station: s.station.as_ref().map(|st| st.name.clone()),
            status: status_parts(&s.status, theme).0,
            programme: now.and_then(|n| n.programme.clone()),
            track,
            format: s
                .stream
                .as_ref()
                .map(ttnr_core::StreamInfo::summary)
                .unwrap_or_default(),
            volume: (s.volume, s.muted),
        };
        let prev = self.seen.take().unwrap_or_default();
        if next.station != prev.station
            && let Some(name) = &next.station
        {
            self.print(format!("play {name}"));
        }
        if next.status != prev.status {
            self.print(next.status.clone());
        }
        if next.format != prev.format && !next.format.is_empty() {
            self.print(format!("format: {}", next.format));
        }
        if next.programme != prev.programme
            && let Some(p) = &next.programme
        {
            self.print(format!("on air: {p}"));
        }
        if next.track != prev.track
            && let Some(t) = &next.track
        {
            self.print(format!("now: {t}"));
        }
        if next.volume != prev.volume && prev.station.is_some() {
            let (v, muted) = next.volume;
            self.print(if muted {
                "muted".to_owned()
            } else {
                format!("vol {}%", theme.number(u64::from(v)))
            });
        }
        self.seen = Some(next);
    }

    fn print(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > MAX_LINES {
            self.lines.drain(..self.lines.len() - MAX_LINES);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use ttnr_core::app::Status;
    use ttnr_core::{InfoSource, NowPlaying, Station};

    #[test]
    fn prints_changes_only() {
        let theme = Theme::plain();
        let mut p = Printout::new(&theme);
        let mut s = AppState {
            volume: 80,
            ..AppState::default()
        };
        p.observe(&s, &theme);
        assert_eq!(p.lines(), ["■ idle"]);
        s.station = Some(Station::from_url("http://x"));
        s.status = Status::Playing;
        p.observe(&s, &theme);
        p.observe(&s, &theme);
        s.now = Some(NowPlaying {
            artist: Some("A".into()),
            title: Some("B".into()),
            ..NowPlaying::empty(InfoSource::Icy, SystemTime::UNIX_EPOCH)
        });
        s.volume = 85;
        p.observe(&s, &theme);
        assert_eq!(
            p.lines(),
            [
                "■ idle",
                "play http://x",
                "▶ playing",
                "now: A – B",
                "vol 85%"
            ]
        );
    }
}
