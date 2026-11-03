//! Merging now-playing information from several providers by priority
//! (ARCHITECTURE.md, "Metadata pipeline"): external > tags > ICY.

use std::collections::BTreeMap;

use crate::{InfoSource, NowPlaying};

/// Keeps the latest [`NowPlaying`] per source and answers which one to show.
#[derive(Debug, Default)]
pub struct Merger {
    latest: BTreeMap<InfoSource, NowPlaying>,
    shown: Option<NowPlaying>,
}

impl Merger {
    /// Record `np`; returns the record to show if it changed.
    pub fn update(&mut self, np: NowPlaying) -> Option<&NowPlaying> {
        self.latest.insert(np.source, np);
        let best = self.latest.values().next_back().cloned();
        if same_track(best.as_ref(), self.shown.as_ref()) {
            return None;
        }
        self.shown = best;
        self.shown.as_ref()
    }

    /// Forget what `sources` said (e.g. the stream cleared its title);
    /// returns `Some(now shown)` if that changes what is shown.
    pub fn forget(&mut self, sources: &[InfoSource]) -> Option<Option<&NowPlaying>> {
        let before = self.latest.len();
        self.latest.retain(|s, _| !sources.contains(s));
        if self.latest.len() == before {
            return None;
        }
        let best = self.latest.values().next_back().cloned();
        if same_track(best.as_ref(), self.shown.as_ref()) {
            return None;
        }
        self.shown = best;
        Some(self.shown.as_ref())
    }

    /// Forget everything (new station or stream).
    pub fn clear(&mut self) {
        self.latest.clear();
        self.shown = None;
    }
}

/// Equal ignoring the timestamp (a repeated announcement is not a change).
fn same_track(a: Option<&NowPlaying>, b: Option<&NowPlaying>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            NowPlaying {
                at: b.at,
                ..a.clone()
            } == *b
        }
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    #[test]
    fn forgetting_a_cleared_source_falls_back() {
        let mut m = Merger::default();
        m.update(np(InfoSource::Icy, "Song", 1));
        assert_eq!(m.forget(&[InfoSource::Icy, InfoSource::Tags]), Some(None));
        assert_eq!(m.forget(&[InfoSource::Icy]), None, "nothing left to forget");
        m.update(np(InfoSource::External, "Listing", 2));
        m.update(np(InfoSource::Icy, "Song", 3));
        // External outranks ICY: clearing ICY changes nothing shown.
        assert_eq!(m.forget(&[InfoSource::Icy]), None);
    }

    fn np(source: InfoSource, title: &str, secs: u64) -> NowPlaying {
        NowPlaying {
            title: Some(title.into()),
            ..NowPlaying::empty(source, SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
        }
    }

    #[test]
    fn higher_priority_wins_and_repeats_are_quiet() {
        let mut m = Merger::default();
        assert_eq!(
            m.update(np(InfoSource::Icy, "A", 1))
                .unwrap()
                .title
                .as_deref(),
            Some("A")
        );
        assert!(
            m.update(np(InfoSource::Icy, "A", 2)).is_none(),
            "same track again"
        );
        assert_eq!(
            m.update(np(InfoSource::Tags, "B", 3))
                .unwrap()
                .title
                .as_deref(),
            Some("B")
        );
        assert!(
            m.update(np(InfoSource::Icy, "C", 4)).is_none(),
            "ICY is outranked by tags"
        );
        assert_eq!(
            m.update(np(InfoSource::External, "D", 5)).unwrap().source,
            InfoSource::External
        );
        m.clear();
        assert_eq!(
            m.update(np(InfoSource::Icy, "E", 6))
                .unwrap()
                .title
                .as_deref(),
            Some("E")
        );
    }
}
