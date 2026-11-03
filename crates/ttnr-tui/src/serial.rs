//! Slow serial line for terminal-like themes (VT100): what the terminal
//! shows lags behind what the app drew, and changed cells arrive one by
//! one, row by row, at the line speed – like a real terminal receiving a
//! screen update at 9600 or 300 baud. Whole-screen redraws (first frame,
//! resize, speed switched on) appear at once; only UI changes are slowed.
//! (Cursor-positioning bytes are not counted: a simplification.)

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

/// Shortest interval between frames while characters are arriving.
const MIN_TICK: Duration = Duration::from_millis(20);

/// The terminal's screen as received so far.
#[derive(Debug, Clone, Default)]
pub struct SlowScreen {
    cps: u32,
    shown: Option<Buffer>,
    last: Option<Instant>,
    budget: Duration,
    pending: bool,
}

impl SlowScreen {
    /// Line speed in bits per second (10 bits per character); 0 = instant.
    pub fn set_baud(&mut self, baud: u32) {
        let cps = baud / 10;
        if cps != self.cps {
            self.cps = cps;
            if cps == 0 {
                self.shown = None;
                self.pending = false;
            }
        }
    }

    /// Whether a speed is set.
    pub fn active(&self) -> bool {
        self.cps > 0
    }

    /// When the next characters are due; `None` when idle.
    pub fn tick(&self) -> Option<Duration> {
        (self.pending && self.cps > 0).then(|| (Duration::from_secs(1) / self.cps).max(MIN_TICK))
    }

    /// Show `target` at once (and remember it as received).
    pub fn sync(&mut self, target: &Buffer) {
        self.shown = (self.cps > 0).then(|| target.clone());
        self.pending = false;
        self.last = None;
    }

    /// What the terminal shows at `now`, given the app's latest frame;
    /// `None` = the frame itself (no speed set).
    pub fn present(&mut self, target: &Buffer, now: Instant) -> Option<Buffer> {
        if self.cps == 0 {
            return None;
        }
        let shown = match &mut self.shown {
            Some(b) if b.area == target.area => b,
            _ => {
                // Whole-screen redraws (first frame, resize, speed switched
                // on) appear at once; only later UI changes travel at the
                // line speed (owner: retyping the screen is annoying).
                self.budget = Duration::ZERO;
                self.last = None;
                self.pending = false;
                self.shown = Some(target.clone());
                return self.shown.clone();
            }
        };
        // Time only counts while characters are on their way.
        let elapsed = match (self.pending, self.last) {
            (true, Some(last)) => now.saturating_duration_since(last),
            _ => Duration::ZERO,
        };
        self.last = Some(now);
        let per_char = Duration::from_secs(1) / self.cps;
        self.budget += elapsed;
        let mut next = None;
        for (i, (have, want)) in shown.content.iter_mut().zip(&target.content).enumerate() {
            if have == want {
                continue;
            }
            if self.budget >= per_char {
                self.budget -= per_char;
                *have = want.clone();
            } else {
                next = Some(i);
                break;
            }
        }
        if next.is_none() {
            self.budget = Duration::ZERO;
        }
        self.pending = next.is_some();
        let mut out = shown.clone();
        // While characters are arriving, a block cursor sits where the
        // next one will be written – the screen visibly isn't hung.
        if let Some(cell) = next.and_then(|i| out.content.get_mut(i)) {
            cell.modifier.toggle(Modifier::REVERSED);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(lines: &[&str]) -> Buffer {
        Buffer::with_lines(lines.iter().copied())
    }

    #[test]
    fn changes_arrive_row_by_row_at_line_speed() {
        let t0 = Instant::now();
        let mut s = SlowScreen::default();
        s.set_baud(300); // 30 characters per second
        let first = buffer(&["...", "..."]);
        assert_eq!(
            s.present(&first, t0),
            Some(first.clone()),
            "first frame at once"
        );
        assert_eq!(s.tick(), None);
        let target = buffer(&["abc", "def"]);
        // The change starts arriving: nothing yet, cursor on the first cell.
        let mut shown = s.present(&target, t0).unwrap();
        assert!(shown[(0, 0)].modifier.contains(Modifier::REVERSED));
        shown[(0, 0)].modifier.remove(Modifier::REVERSED);
        assert_eq!(shown, first);
        assert_eq!(s.tick(), Some(Duration::from_secs(1) / 30));
        let step = Duration::from_secs(1) / 30;
        let mut shown = s.present(&target, t0 + step * 4).unwrap();
        // Cursor on the next cell to arrive ("e").
        assert!(shown[(1, 1)].modifier.contains(Modifier::REVERSED));
        shown[(1, 1)].modifier.remove(Modifier::REVERSED);
        assert_eq!(shown, buffer(&["abc", "d.."]));
        let shown = s.present(&target, t0 + step * 6).unwrap();
        assert_eq!(shown, target);
        assert_eq!(s.tick(), None);
        // Only changed cells cost time; idle time is not banked.
        let next = buffer(&["abX", "def"]);
        let later = t0 + Duration::from_secs(60);
        let mut shown = s.present(&next, later).unwrap();
        assert!(shown[(2, 0)].modifier.contains(Modifier::REVERSED));
        shown[(2, 0)].modifier.remove(Modifier::REVERSED);
        assert_eq!(shown, target);
        assert_eq!(
            s.present(&next, later + step),
            Some(next),
            "done: no cursor"
        );
    }

    #[test]
    fn resize_redraws_at_once() {
        let t0 = Instant::now();
        let mut s = SlowScreen::default();
        s.set_baud(110);
        s.present(&buffer(&["ab", "cd"]), t0);
        s.present(&buffer(&["xy", "zw"]), t0); // in flight
        assert!(s.tick().is_some());
        let resized = buffer(&["abc", "def", "ghi"]);
        assert_eq!(s.present(&resized, t0), Some(resized.clone()));
        assert_eq!(s.tick(), None, "nothing pending after a resize");
    }

    #[test]
    fn instant_and_sync() {
        let mut s = SlowScreen::default();
        let target = buffer(&["hi"]);
        assert!(!s.active());
        assert_eq!(s.present(&target, Instant::now()), None);
        // Switched on mid-session: no blank screen, only changes typed.
        s.set_baud(9600);
        assert_eq!(s.present(&target, Instant::now()), Some(target.clone()));
        assert_eq!(s.tick(), None);
        s.set_baud(0);
        s.set_baud(300);
        s.sync(&target);
        assert_eq!(s.present(&target, Instant::now()), Some(target));
    }
}
