//! What to do when a stream fails (ARCHITECTURE.md, "Stream resolution and
//! refresh"): try the next candidate URL; when they are exhausted and the
//! failure suggests a stale URL (403/404/unreachable), re-resolve **once**;
//! only then give up.

use std::collections::VecDeque;

use crate::StreamDescriptor;

/// The caller's next step after a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Play this URL (next candidate).
    Play(String),
    /// Call `SourceRegistry::refresh` and hand the result to
    /// [`Failover::refreshed`].
    Refresh,
    /// Show the error.
    GiveUp,
}

/// Failover state for one station.
#[derive(Debug, Clone)]
pub struct Failover {
    candidates: VecDeque<String>,
    headers: Vec<(String, String)>,
    refreshed: bool,
}

impl Failover {
    /// Start with a resolved descriptor.
    pub fn new(d: &StreamDescriptor) -> Self {
        Self {
            candidates: d.urls.iter().cloned().collect(),
            headers: d.headers.clone(),
            refreshed: false,
        }
    }

    /// The first URL to play (or the next one), if any.
    pub fn next_url(&mut self) -> Option<String> {
        self.candidates.pop_front()
    }

    /// Headers to send with the candidates.
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// The current URL failed; `stale_hint` = the error suggests the URL
    /// itself is stale (HTTP 403/404, unreachable).
    pub fn on_failure(&mut self, stale_hint: bool) -> Action {
        if let Some(url) = self.next_url() {
            Action::Play(url)
        } else if stale_hint && !self.refreshed {
            Action::Refresh
        } else {
            Action::GiveUp
        }
    }

    /// Fresh candidates after [`Action::Refresh`]; returns the URL to play.
    /// No further refresh will be offered.
    pub fn refreshed(&mut self, d: &StreamDescriptor) -> Option<String> {
        self.refreshed = true;
        self.candidates = d.urls.iter().cloned().collect();
        self.headers = d.headers.clone();
        self.next_url()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(urls: &[&str]) -> StreamDescriptor {
        StreamDescriptor {
            urls: urls.iter().map(|u| (*u).to_owned()).collect(),
            ..StreamDescriptor::default()
        }
    }

    #[test]
    fn mirrors_then_one_refresh_then_give_up() {
        let mut f = Failover::new(&desc(&["a", "b"]));
        assert_eq!(f.next_url().as_deref(), Some("a"));
        assert_eq!(f.on_failure(true), Action::Play("b".into()));
        assert_eq!(f.on_failure(true), Action::Refresh);
        assert_eq!(f.refreshed(&desc(&["c"])).as_deref(), Some("c"));
        assert_eq!(f.on_failure(true), Action::GiveUp, "refresh only once");
    }

    #[test]
    fn non_stale_errors_do_not_refresh() {
        let mut f = Failover::new(&desc(&["a"]));
        f.next_url();
        assert_eq!(f.on_failure(false), Action::GiveUp);
    }

    #[test]
    fn empty_refresh_result() {
        let mut f = Failover::new(&desc(&["a"]));
        f.next_url();
        assert_eq!(f.on_failure(true), Action::Refresh);
        assert_eq!(f.refreshed(&desc(&[])), None);
        assert_eq!(f.on_failure(true), Action::GiveUp);
    }
}
