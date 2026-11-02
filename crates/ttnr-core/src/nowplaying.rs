//! [`NowPlaying`]: the unified "what is on" model (SPEC F4.1).

use std::time::SystemTime;

/// Where a piece of now-playing information came from. Order = priority
/// when merging (ARCHITECTURE.md, "Metadata pipeline"): external > ID3 > ICY.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InfoSource {
    /// Icecast/Shoutcast in-stream `StreamTitle`.
    Icy,
    /// HLS timed ID3 tags.
    TimedId3,
    /// An external provider (e.g. BBC's now-playing service).
    External,
}

/// What is playing, as far as anyone knows. Every field is optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    /// Station display name.
    pub station: Option<String>,
    /// Programme / show.
    pub programme: Option<String>,
    /// Track artist.
    pub artist: Option<String>,
    /// Track title.
    pub title: Option<String>,
    /// Album, when known.
    pub album: Option<String>,
    /// Record label, when known.
    pub label: Option<String>,
    /// Artwork URL, when known.
    pub artwork_url: Option<String>,
    /// Who supplied this information.
    pub source: InfoSource,
    /// When the information became valid (before sync offset).
    pub at: SystemTime,
}

impl NowPlaying {
    /// An empty record from `source`, valid at `at`.
    pub fn empty(source: InfoSource, at: SystemTime) -> Self {
        Self {
            station: None,
            programme: None,
            artist: None,
            title: None,
            album: None,
            label: None,
            artwork_url: None,
            source,
            at,
        }
    }
}
