//! Printout mode (Teletype themes): instead of redrawing in place, every
//! change is *printed* as a new line, like a terminal on paper.

use std::collections::VecDeque;
use std::time::Duration;

use ttnr_core::app::AppState;
use ttnr_themes::Theme;

use crate::view::status_parts;

/// Most lines kept (older ones scroll off the paper).
const MAX_LINES: usize = 1000;
/// With a line speed set, at most this much printing may be queued; older
/// queued lines are printed at once so the paper never lags behind the
/// state by more than this.
const MAX_BACKLOG: Duration = Duration::from_secs(4);
/// Characters per line end (CR + LF).
const EOL_CHARS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Seen {
    station: Option<String>,
    status: String,
    programme: Option<String>,
    track: Option<String>,
    format: String,
    volume: (u8, bool),
}

/// The printed lines so far, optionally printed at a simulated line speed.
#[derive(Debug, Clone, Default)]
pub struct Printout {
    /// Printed lines; while a line is being printed it is the last one.
    lines: Vec<String>,
    seen: Option<Seen>,
    /// Characters per second; 0 = print instantly.
    cps: u32,
    /// Lines waiting to be printed (the first may be partly printed).
    queue: VecDeque<String>,
    /// Characters of `queue[0]` already printed (incl. its line end).
    done: usize,
    /// Printing time not yet spent on a whole character.
    budget: Duration,
}

impl Printout {
    /// Start a printout with the theme's banner, printed instantly.
    pub fn new(theme: &Theme) -> Self {
        Self::with_baud(theme, 0)
    }

    /// Start a printout simulating a serial line of `baud` bits per second
    /// (10 bits per character: start, 7 data, parity, stop – 110 baud is
    /// the Teletype Model 33's 10 characters per second); 0 = instant.
    pub fn with_baud(theme: &Theme, baud: u32) -> Self {
        let mut p = Self {
            cps: baud / 10,
            ..Self::default()
        };
        for line in &theme.chrome.banner {
            p.print(line.clone());
        }
        p
    }

    /// Lines printed so far, oldest first (the last may be partial).
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Whether characters are waiting to be printed.
    pub fn busy(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Time per character while busy (the UI's tick), `None` when idle.
    pub fn tick(&self) -> Option<Duration> {
        (self.busy() && self.cps > 0).then(|| Duration::from_secs(1) / self.cps)
    }

    /// Print what `elapsed` time at the line speed allows; true if the
    /// paper changed.
    pub fn advance(&mut self, elapsed: Duration) -> bool {
        if self.queue.is_empty() || self.cps == 0 {
            self.budget = Duration::ZERO;
            return false;
        }
        let per_char = Duration::from_secs(1) / self.cps;
        self.budget += elapsed;
        let mut changed = false;
        while self.budget >= per_char {
            let Some(line) = self.queue.front() else {
                self.budget = Duration::ZERO;
                break;
            };
            self.budget -= per_char;
            let len = line.chars().count();
            if self.done == 0 {
                self.lines.push(String::new());
            }
            self.done += 1;
            if self.done <= len {
                let shown: String = line.chars().take(self.done).collect();
                if let Some(last) = self.lines.last_mut() {
                    *last = shown;
                }
                changed = true;
            }
            if self.done >= len + EOL_CHARS {
                self.queue.pop_front();
                self.done = 0;
            }
        }
        changed
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
                format!("vol {}", theme.percent(u64::from(v)))
            });
        }
        self.seen = Some(next);
    }

    fn print(&mut self, line: String) {
        if self.cps == 0 {
            self.lines.push(line);
        } else {
            self.queue.push_back(line);
            self.limit_backlog();
        }
        if self.lines.len() > MAX_LINES {
            self.lines.drain(..self.lines.len() - MAX_LINES);
        }
    }
}

impl Printout {
    /// Print queued lines at once (oldest first) until the rest fits in
    /// `MAX_BACKLOG` at the line speed; the newest line is always typed.
    fn limit_backlog(&mut self) {
        let max = u128::from(self.cps) * MAX_BACKLOG.as_millis() / 1000;
        let chars = |q: &VecDeque<String>, done: usize| -> u128 {
            let total: usize = q.iter().map(|l| l.chars().count() + EOL_CHARS).sum();
            (total - done.min(total)) as u128
        };
        while self.queue.len() > 1 && chars(&self.queue, self.done) > max {
            if let Some(line) = self.queue.pop_front() {
                if self.done > 0 {
                    self.lines.pop();
                }
                self.lines.push(line);
                self.done = 0;
            }
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

    #[test]
    fn line_speed_types_characters() {
        let theme = Theme::plain();
        // 300 baud = 30 characters per second.
        let mut p = Printout::with_baud(&theme, 300);
        assert!(!p.busy());
        let s = AppState::default();
        p.observe(&s, &theme); // "■ idle" (6 chars + CR LF)
        assert!(p.busy());
        assert_eq!(p.tick(), Some(Duration::from_secs(1) / 30));
        let step = Duration::from_secs(1) / 30;
        assert!(p.advance(step * 3));
        assert_eq!(p.lines(), ["■ i"]);
        assert!(p.advance(step * 3));
        assert_eq!(p.lines(), ["■ idle"]);
        assert!(p.busy(), "line end still printing");
        assert!(!p.advance(step * 2));
        assert!(!p.busy());
        assert_eq!(p.tick(), None);
    }

    #[test]
    fn backlog_is_bounded() {
        let theme = Theme::plain();
        // 110 baud = 11 cps → at most ~44 characters queued.
        let mut p = Printout::with_baud(&theme, 110);
        for i in 0..10 {
            p.print(format!("line number {i} with some text"));
        }
        assert!(
            p.lines().len() >= 8,
            "old lines printed at once: {:?}",
            p.lines()
        );
        assert_eq!(p.lines()[0], "line number 0 with some text");
        p.advance(Duration::from_secs(10));
        assert!(!p.busy());
        assert_eq!(p.lines().len(), 10);
        assert_eq!(p.lines()[9], "line number 9 with some text");
    }
}
