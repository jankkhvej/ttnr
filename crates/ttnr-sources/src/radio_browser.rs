//! The `radio-browser` source: search the open station directory
//! radio-browser.info (ADR-040). Requests go to `all.api.radio-browser.info`
//! (DNS round-robin over the API servers; its certificate covers
//! `*.api.radio-browser.info`) with ttnr's descriptive User-Agent, as the API
//! docs ask. Starting a station calls `/json/url/<uuid>`, which the docs ask
//! clients to call on every play (popularity counting).

use async_trait::async_trait;
use serde::Deserialize;
use ttnr_core::{
    ListQuery, MetadataProvider, ResolveCtx, Source, SourceCaps, SourceError, Station,
    StreamDescriptor,
};

use crate::url_source::UrlSource;

/// Source id.
pub const ID: &str = "radio-browser";

/// API base URL.
pub const API: &str = "https://all.api.radio-browser.info";

/// Results per search unless the query says otherwise.
const DEFAULT_LIMIT: usize = 50;

/// One station as the API returns it (fields we use; others ignored).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct ApiStation {
    stationuuid: String,
    name: String,
    url: String,
    url_resolved: String,
    codec: String,
    bitrate: u64,
    countrycode: String,
    hls: u8,
}

/// The answer of `/json/url/<uuid>`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ClickAnswer {
    ok: bool,
    url: String,
}

/// Parse a `/json/stations/search` answer into stations.
pub fn stations_from_search(json: &[u8]) -> Result<Vec<Station>, SourceError> {
    let list: Vec<ApiStation> = serde_json::from_slice(json).map_err(|e| SourceError::Parse {
        what: "radio-browser search".into(),
        detail: e.to_string(),
    })?;
    Ok(list
        .into_iter()
        .filter(|s| !s.stationuuid.is_empty())
        .map(station)
        .collect())
}

fn station(s: ApiStation) -> Station {
    let mut params = std::collections::BTreeMap::new();
    params.insert("uuid".to_owned(), s.stationuuid.clone());
    let url = if s.url_resolved.is_empty() {
        s.url
    } else {
        s.url_resolved
    };
    if !url.is_empty() {
        params.insert("url_resolved".to_owned(), url);
    }
    let detail = detail(&s.codec, s.bitrate, &s.countrycode, s.hls == 1);
    if !detail.is_empty() {
        params.insert("detail".to_owned(), detail);
    }
    let name = s.name.trim();
    Station {
        id: s.stationuuid,
        name: if name.is_empty() {
            "(unnamed)".to_owned()
        } else {
            name.to_owned()
        },
        source: ID.to_owned(),
        url: None,
        params,
    }
}

/// "MP3 128k GB" from the directory's fields. Bitrates are entered by
/// users; values over 5000 are taken as bit/s, not kbit/s.
fn detail(codec: &str, bitrate: u64, country: &str, hls: bool) -> String {
    let kbps = if bitrate > 5000 {
        bitrate / 1000
    } else {
        bitrate
    };
    let mut parts = Vec::new();
    if !codec.is_empty() && codec != "UNKNOWN" {
        parts.push(codec.to_owned());
    }
    if kbps > 0 {
        parts.push(format!("{kbps}k"));
    }
    if hls {
        parts.push("HLS".to_owned());
    }
    if !country.is_empty() {
        parts.push(country.to_owned());
    }
    parts.join(" ")
}

/// The search URL for `q`.
pub fn search_url(q: &ListQuery) -> String {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT);
    let name = q.search.as_deref().unwrap_or_default();
    let mut url = url::Url::parse(&format!("{API}/json/stations/search"))
        .unwrap_or_else(|_| unreachable!("constant URL"));
    url.query_pairs_mut()
        .append_pair("name", name)
        .append_pair("hidebroken", "true")
        .append_pair("order", "clickcount")
        .append_pair("reverse", "true")
        .append_pair("limit", &limit.to_string());
    url.into()
}

/// Searches radio-browser.info and resolves its stations.
#[derive(Debug, Default, Clone, Copy)]
pub struct RadioBrowserSource;

impl RadioBrowserSource {
    /// A station known only by its directory id (e.g. from `stations.toml`
    /// or `ttnr play radio-browser:<uuid>`).
    pub fn station(uuid: &str, name: Option<&str>) -> Station {
        let mut params = std::collections::BTreeMap::new();
        params.insert("uuid".to_owned(), uuid.to_owned());
        Station {
            id: uuid.to_owned(),
            name: name.unwrap_or(uuid).to_owned(),
            source: ID.to_owned(),
            url: None,
            params,
        }
    }

    fn uuid(st: &Station) -> &str {
        st.params.get("uuid").map_or(st.id.as_str(), String::as_str)
    }
}

#[async_trait]
impl Source for RadioBrowserSource {
    fn id(&self) -> &'static str {
        ID
    }

    fn capabilities(&self) -> SourceCaps {
        SourceCaps {
            search: true,
            ..SourceCaps::default()
        }
    }

    async fn list(&self, q: &ListQuery, ctx: &ResolveCtx) -> Result<Vec<Station>, SourceError> {
        let url = search_url(q);
        let resp = ctx.http.get(&url, &[]).await?;
        if !resp.is_success() {
            return Err(SourceError::Status {
                status: resp.status,
                url,
            });
        }
        stations_from_search(&resp.body)
    }

    /// Count the click (`/json/url/<uuid>`, which also returns the stream
    /// URL), then play the directory's resolved URL – expanded like any URL
    /// (playlists, HLS).
    async fn discover(
        &self,
        st: &Station,
        ctx: &ResolveCtx,
    ) -> Result<Option<StreamDescriptor>, SourceError> {
        let uuid = Self::uuid(st);
        let click_url = format!("{API}/json/url/{uuid}");
        let clicked = match ctx.http.get(&click_url, &[]).await {
            Ok(resp) if resp.is_success() => serde_json::from_slice::<ClickAnswer>(&resp.body)
                .ok()
                .filter(|a| a.ok && !a.url.is_empty())
                .map(|a| a.url),
            Ok(resp) => {
                tracing::debug!(status = resp.status, uuid, "radio-browser click refused");
                None
            }
            Err(e) => {
                tracing::debug!(error = %e, uuid, "radio-browser click failed");
                None
            }
        };
        let Some(url) = st.params.get("url_resolved").cloned().or(clicked) else {
            return Err(SourceError::Other(format!(
                "radio-browser has no stream URL for station {uuid}"
            )));
        };
        UrlSource.discover(&Station::from_url(&url), ctx).await
    }

    fn metadata(&self, st: &Station, ctx: &ResolveCtx) -> Option<Box<dyn MetadataProvider>> {
        let url = st.params.get("url_resolved")?;
        UrlSource.metadata(&Station::from_url(url), ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_normalises_bitrates() {
        assert_eq!(detail("MP3", 128, "US", false), "MP3 128k US");
        assert_eq!(detail("MP4", 320_000, "", true), "MP4 320k HLS");
        assert_eq!(detail("UNKNOWN", 0, "", false), "");
    }

    #[test]
    fn search_url_encodes_the_query() {
        let url = search_url(&ListQuery {
            search: Some("groove salad & co".into()),
            limit: Some(5),
        });
        assert_eq!(
            url,
            "https://all.api.radio-browser.info/json/stations/search?name=groove+salad+%26+co&hidebroken=true&order=clickcount&reverse=true&limit=5"
        );
    }
}
