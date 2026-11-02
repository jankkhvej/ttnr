//! [`Station`]: something the user can tune to.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A station as listed by a source or in `stations.toml`. Stream URLs are
/// *not* stored here (except a user override): sources resolve them at
/// play time because they change (ADR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    /// Identifier unique within its source, e.g. `bbc_6music`.
    pub id: String,
    /// Display name, e.g. `BBC Radio 6 Music`.
    pub name: String,
    /// Id of the source that resolves this station, e.g. `bbc`, `url`.
    pub source: String,
    /// User-pinned stream URL. For the `url` source it *is* the station;
    /// for other sources it is the last resort of the resolver chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Source-specific settings (e.g. a preferred region or bitrate).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

/// Id of the source that plays plain URLs (`ttnr-sources::url_source`).
pub const URL_SOURCE: &str = "url";

impl Station {
    /// An ad-hoc station for a URL or file typed by the user.
    pub fn from_url(url: &str) -> Self {
        Self {
            id: url.to_owned(),
            name: url.to_owned(),
            source: URL_SOURCE.to_owned(),
            url: Some(url.to_owned()),
            params: BTreeMap::new(),
        }
    }

    /// Key identifying this station across sources (`source:id`).
    pub fn key(&self) -> String {
        format!("{}:{}", self.source, self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_combines_source_and_id() {
        let st = Station {
            id: "bbc_6music".into(),
            name: "BBC Radio 6 Music".into(),
            source: "bbc".into(),
            url: None,
            params: BTreeMap::new(),
        };
        assert_eq!(st.key(), "bbc:bbc_6music");
    }
}
