//! [`NowPlaying`]: the unified "what is on" model (SPEC F4.1).

use std::time::SystemTime;

/// Where a piece of now-playing information came from. Order = priority
/// when merging (ARCHITECTURE.md, "Metadata pipeline"): external > tags > ICY.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InfoSource {
    /// Icecast/Shoutcast in-stream `StreamTitle`.
    Icy,
    /// In-stream tags: HLS timed ID3, or a file's own tags.
    Tags,
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

/// Raw in-stream metadata as reported by the player.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamTags {
    /// ICY `StreamTitle` (usually `Artist - Title`).
    pub icy_title: Option<String>,
    /// ICY station name (`icy-name` header).
    pub icy_name: Option<String>,
    /// `title` tag (ID3 TIT2, Vorbis comment, …).
    pub title: Option<String>,
    /// `artist` tag.
    pub artist: Option<String>,
    /// `album` tag.
    pub album: Option<String>,
}

impl NowPlaying {
    /// Now-playing info from in-stream tags, if they say anything about the
    /// track: tags (timed ID3 / file tags) win over ICY.
    pub fn from_tags(tags: &StreamTags, at: SystemTime) -> Option<Self> {
        let station = tags.icy_name.clone();
        if tags.title.is_some() || tags.artist.is_some() {
            return Some(Self {
                station,
                artist: tags.artist.clone(),
                title: tags.title.clone(),
                album: tags.album.clone(),
                ..Self::empty(InfoSource::Tags, at)
            });
        }
        let (artist, title) = split_icy_title(tags.icy_title.as_deref()?)?;
        Some(Self {
            station,
            artist,
            title: Some(title),
            ..Self::empty(InfoSource::Icy, at)
        })
    }
}

/// Split an ICY `StreamTitle` into artist and title at the first ` - `
/// (the de-facto convention); without it the whole text is the title.
/// `None` for blank titles (some stations send `StreamTitle='';`).
pub fn split_icy_title(text: &str) -> Option<(Option<String>, String)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match text.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            Some((Some(artist.trim().to_owned()), title.trim().to_owned()))
        }
        _ => Some((None, text.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icy_title_split() {
        assert_eq!(
            split_icy_title(" Boards of Canada - Roygbiv "),
            Some((Some("Boards of Canada".into()), "Roygbiv".into()))
        );
        assert_eq!(
            split_icy_title("A - B - C"),
            Some((Some("A".into()), "B - C".into()))
        );
        assert_eq!(
            split_icy_title("Station jingle"),
            Some((None, "Station jingle".into()))
        );
        assert_eq!(split_icy_title("- Title"), Some((None, "- Title".into())));
        assert_eq!(split_icy_title("  "), None);
    }

    #[test]
    fn tags_win_over_icy() {
        let at = SystemTime::UNIX_EPOCH;
        let mut tags = StreamTags {
            icy_title: Some("X - Y".into()),
            icy_name: Some("Test FM".into()),
            ..StreamTags::default()
        };
        let np = NowPlaying::from_tags(&tags, at).unwrap();
        assert_eq!(
            (np.source, np.artist.as_deref(), np.title.as_deref()),
            (InfoSource::Icy, Some("X"), Some("Y"))
        );
        assert_eq!(np.station.as_deref(), Some("Test FM"));
        tags.title = Some("Real Title".into());
        tags.artist = Some("Real Artist".into());
        let np = NowPlaying::from_tags(&tags, at).unwrap();
        assert_eq!(np.source, InfoSource::Tags);
        assert_eq!(np.title.as_deref(), Some("Real Title"));
    }

    #[test]
    fn nothing_to_say() {
        let tags = StreamTags {
            icy_name: Some("Test FM".into()),
            icy_title: Some(String::new()),
            ..StreamTags::default()
        };
        assert_eq!(NowPlaying::from_tags(&tags, SystemTime::UNIX_EPOCH), None);
    }
}
