//! The `stations` source: the user's own list in `stations.toml`
//! (`$XDG_CONFIG_HOME/ttnr/stations.toml`, see `examples/stations.toml`).
//!
//! It is a catalogue: each station names the source that resolves it
//! (`url`, `bbc`, …), so this source only lists and looks up.

use std::collections::HashSet;
use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;
use ttnr_core::{ListQuery, ResolveCtx, Source, SourceCaps, SourceError, Station};

/// Source id.
pub const ID: &str = "stations";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    station: Vec<Station>,
}

/// Stations loaded from `stations.toml`.
#[derive(Debug, Default, Clone)]
pub struct StationsSource {
    stations: Vec<Station>,
}

impl StationsSource {
    /// Load `path`; a missing file is an empty list (the app runs without
    /// any config). Syntax or validation errors name the file.
    pub fn load(path: &Path) -> Result<Self, SourceError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, &path.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(SourceError::Other(format!("{}: {e}", path.display()))),
        }
    }

    /// Parse and validate `stations.toml` content; `origin` is used in
    /// error messages.
    pub fn parse(text: &str, origin: &str) -> Result<Self, SourceError> {
        let file: File = toml::from_str(text).map_err(|e| SourceError::Parse {
            what: origin.to_owned(),
            detail: e.to_string(),
        })?;
        let invalid = |detail: String| SourceError::Parse {
            what: origin.to_owned(),
            detail,
        };
        let mut seen = HashSet::new();
        for st in &file.station {
            if st.id.trim().is_empty() || st.name.trim().is_empty() || st.source.trim().is_empty() {
                return Err(invalid(format!(
                    "station {:?}: id, name and source must not be empty",
                    st.id
                )));
            }
            if st.source == ID {
                return Err(invalid(format!(
                    "station {:?}: source must be the resolving source (e.g. \"url\"), not \"{ID}\"",
                    st.id
                )));
            }
            if st.source == crate::url_source::ID && st.url.is_none() {
                return Err(invalid(format!(
                    "station {:?}: source \"url\" needs a url",
                    st.id
                )));
            }
            if !seen.insert(st.id.as_str()) {
                return Err(invalid(format!("duplicate station id {:?}", st.id)));
            }
        }
        Ok(Self {
            stations: file.station,
        })
    }

    /// All stations, in file order.
    pub fn stations(&self) -> &[Station] {
        &self.stations
    }

    /// The station with `id`.
    pub fn find(&self, id: &str) -> Option<&Station> {
        self.stations.iter().find(|s| s.id == id)
    }

    fn matching(&self, q: &ListQuery) -> Vec<Station> {
        let needle = q.search.as_deref().map(str::to_lowercase);
        self.stations
            .iter()
            .filter(|s| {
                needle.as_deref().is_none_or(|n| {
                    s.name.to_lowercase().contains(n) || s.id.to_lowercase().contains(n)
                })
            })
            .take(q.limit.unwrap_or(usize::MAX))
            .cloned()
            .collect()
    }
}

#[async_trait]
impl Source for StationsSource {
    fn id(&self) -> &'static str {
        ID
    }

    fn capabilities(&self) -> SourceCaps {
        SourceCaps {
            browse: true,
            search: true,
            metadata: false,
        }
    }

    async fn list(&self, q: &ListQuery, _ctx: &ResolveCtx) -> Result<Vec<Station>, SourceError> {
        Ok(self.matching(q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[station]]
id = "6music"
name = "BBC Radio 6 Music"
source = "bbc"

[[station]]
id = "groovesalad"
name = "SomaFM Groove Salad"
source = "url"
url = "https://somafm.com/groovesalad.pls"
params = { note = "ambient" }
"#;

    #[test]
    fn parses_and_finds() {
        let s = StationsSource::parse(SAMPLE, "test").unwrap();
        assert_eq!(s.stations().len(), 2);
        let gs = s.find("groovesalad").unwrap();
        assert_eq!(
            gs.url.as_deref(),
            Some("https://somafm.com/groovesalad.pls")
        );
        assert_eq!(gs.params["note"], "ambient");
        assert!(s.find("nope").is_none());
    }

    #[test]
    fn search_and_limit() {
        let s = StationsSource::parse(SAMPLE, "test").unwrap();
        let q = |search: Option<&str>, limit| ListQuery {
            search: search.map(Into::into),
            limit,
        };
        assert_eq!(s.matching(&q(Some("SOMA"), None)).len(), 1);
        assert_eq!(s.matching(&q(Some("6music"), None))[0].source, "bbc");
        assert_eq!(s.matching(&q(None, Some(1))).len(), 1);
    }

    #[test]
    fn validation_errors_name_the_problem() {
        for (text, needle) in [
            (
                "[[station]]\nid='a'\nname='A'\nsource='url'\n",
                "needs a url",
            ),
            (
                "[[station]]\nid='a'\nname='A'\nsource='bbc'\n[[station]]\nid='a'\nname='B'\nsource='bbc'\n",
                "duplicate station id",
            ),
            (
                "[[station]]\nid=''\nname='A'\nsource='bbc'\n",
                "must not be empty",
            ),
            (
                "[[station]]\nid='a'\nname='A'\nsource='stations'\n",
                "resolving source",
            ),
            ("[[station]]\nid='a'\nname='A'\n", "missing field `source`"),
            ("[[stations]]\nid='a'\n", "unknown field `stations`"),
        ] {
            let err = StationsSource::parse(text, "my.toml")
                .unwrap_err()
                .to_string();
            assert!(err.contains("my.toml") && err.contains(needle), "{err}");
        }
    }

    #[test]
    fn missing_file_is_empty() {
        let s = StationsSource::load(Path::new("/nonexistent/ttnr/stations.toml")).unwrap();
        assert!(s.stations().is_empty());
    }

    #[test]
    fn example_file_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/stations.toml");
        let s = StationsSource::load(&path).unwrap();
        assert!(!s.stations().is_empty());
    }
}
