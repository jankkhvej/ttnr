//! The `bbc` source: BBC national, nations and local radio (M4, ADR-029).
//!
//! Streams are discovered exactly like BBC's own web player: an anonymous
//! token from `rms.api.bbc.co.uk/v2/sign/token/<service>`, then media
//! selector v3 with that token. We play what BBC serves for the listener's
//! location (`nonuk` streams abroad); every refusal is final – no guessed
//! URL patterns, no region spoofing. Facts and sources: docs/RESEARCH.md.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use ttnr_core::{
    ListQuery, ResolveCtx, Source, SourceCaps, SourceError, Station, StreamDescriptor,
};

/// Source id.
pub const ID: &str = "bbc";

const RMS: &str = "https://rms.api.bbc.co.uk/v2";
const MEDIASELECTOR: &str = "https://open.live.bbc.co.uk/mediaselector/6/select/version/3.0/mediaset/pc/cvid/urn:bbc:pips:pid:";

/// How long resolved stream URLs are reused (the master playlists are
/// stable; the token is only needed to find them).
const STREAM_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Deserialize)]
struct Networks {
    data: Vec<Network>,
}

#[derive(Debug, Deserialize)]
struct Network {
    id: String,
    #[serde(default)]
    coverage: String,
    long_title: Option<String>,
    short_title: Option<String>,
    #[serde(default)]
    services: Vec<Service>,
}

#[derive(Debug, Deserialize)]
struct Service {
    id: String,
    short_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Token {
    token: String,
}

#[derive(Debug, Deserialize)]
struct Selection {
    #[serde(default)]
    media: Vec<Media>,
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Media {
    #[serde(default, deserialize_with = "number_or_string")]
    bitrate: u32,
    #[serde(default)]
    connection: Vec<Connection>,
}

#[derive(Debug, Deserialize)]
struct Connection {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    supplier: String,
    #[serde(rename = "transferFormat", default)]
    transfer_format: String,
    href: String,
}

/// Media selector sends bitrates as numbers or strings depending on version.
fn number_or_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        N(u32),
        S(String),
    }
    Ok(match Raw::deserialize(d)? {
        Raw::N(n) => n,
        Raw::S(s) => s.parse().unwrap_or(0),
    })
}

fn parse_err(what: &str, e: impl std::fmt::Display) -> SourceError {
    SourceError::Parse {
        what: what.to_owned(),
        detail: e.to_string(),
    }
}

/// Stations from the `/v2/networks` answer: one per service. Single-service
/// networks use the network's long title ("BBC Radio 6 Music"); others
/// "BBC <service>" ("BBC Radio 4 FM").
pub fn stations_from_networks(json: &[u8]) -> Result<Vec<Station>, SourceError> {
    let networks: Networks =
        serde_json::from_slice(json).map_err(|e| parse_err("BBC networks", e))?;
    let mut out = Vec::new();
    for n in networks.data {
        let network_title = n
            .long_title
            .or(n.short_title)
            .unwrap_or_else(|| n.id.clone());
        let single = n.services.len() == 1;
        for s in &n.services {
            let name = match (&s.short_title, single) {
                (_, true) | (None, _) => network_title.clone(),
                (Some(t), false) => format!("BBC {t}"),
            };
            let mut st = Station {
                id: s.id.clone(),
                name,
                source: ID.to_owned(),
                url: None,
                params: Default::default(),
            };
            st.params.insert("coverage".into(), n.coverage.clone());
            st.params.insert("network".into(), n.id.clone());
            out.push(st);
        }
    }
    Ok(out)
}

/// Pick stream URLs from a media selector answer: highest bitrate first,
/// HLS only (mpv cannot play DASH with Homebrew's FFmpeg, ADR-016), https
/// preferred, one URL per CDN supplier (later ones are failover candidates).
pub fn streams_from_selection(json: &[u8], service: &str) -> Result<Vec<String>, SourceError> {
    let sel: Selection =
        serde_json::from_slice(json).map_err(|e| parse_err("BBC media selection", e))?;
    if let Some(result) = sel.result.filter(|_| sel.media.is_empty()) {
        return Err(refusal(service, &result));
    }
    let mut media = sel.media;
    media.sort_by_key(|m| std::cmp::Reverse(m.bitrate));
    let mut urls: Vec<String> = Vec::new();
    let mut suppliers: Vec<String> = Vec::new();
    for m in &media {
        let mut conns: Vec<&Connection> = m
            .connection
            .iter()
            .filter(|c| c.transfer_format == "hls")
            .collect();
        conns.sort_by_key(|c| c.protocol != "https");
        for c in conns {
            if !suppliers.contains(&c.supplier) && !urls.contains(&c.href) {
                suppliers.push(c.supplier.clone());
                urls.push(c.href.clone());
            }
        }
    }
    if urls.is_empty() {
        return Err(SourceError::Other(format!(
            "BBC offers no HLS stream for {service}"
        )));
    }
    Ok(urls)
}

/// A clear, final message for BBC's refusals.
fn refusal(service: &str, result: &str) -> SourceError {
    let why = match result {
        "geolocation" => "BBC does not offer it in your location (rights restriction)".to_owned(),
        "selectionunavailable" => "BBC does not offer it here right now".to_owned(),
        other => format!("BBC refused it ({other})"),
    };
    SourceError::Other(format!("{service}: {why}"))
}

/// The BBC source.
#[derive(Debug, Default, Clone, Copy)]
pub struct BbcSource;

#[async_trait]
impl Source for BbcSource {
    fn id(&self) -> &'static str {
        ID
    }

    fn capabilities(&self) -> SourceCaps {
        SourceCaps {
            browse: true,
            search: true,
            metadata: true,
        }
    }

    async fn list(&self, q: &ListQuery, ctx: &ResolveCtx) -> Result<Vec<Station>, SourceError> {
        let url = format!("{RMS}/networks?limit=100");
        let resp = ctx.http.get(&url, &[]).await?;
        if !resp.is_success() {
            return Err(SourceError::Status {
                status: resp.status,
                url,
            });
        }
        let needle = q.search.as_deref().map(str::to_lowercase);
        Ok(stations_from_networks(&resp.body)?
            .into_iter()
            .filter(|s| {
                needle
                    .as_deref()
                    .is_none_or(|n| s.name.to_lowercase().contains(n) || s.id.contains(n))
            })
            .take(q.limit.unwrap_or(usize::MAX))
            .collect())
    }

    async fn discover(
        &self,
        st: &Station,
        ctx: &ResolveCtx,
    ) -> Result<Option<StreamDescriptor>, SourceError> {
        let service = st.id.as_str();
        let token_url = format!("{RMS}/sign/token/{service}");
        let resp = ctx.http.get(&token_url, &[]).await?;
        if !resp.is_success() {
            return Err(SourceError::Status {
                status: resp.status,
                url: token_url,
            });
        }
        let token: Token =
            serde_json::from_slice(&resp.body).map_err(|e| parse_err("BBC token", e))?;
        let url = format!("{MEDIASELECTOR}{service}/format/json");
        let auth = [(
            "Authorization".to_owned(),
            format!("Bearer {}", token.token),
        )];
        let resp = ctx.http.get(&url, &auth).await?;
        // Refusals come as 403/404 with a JSON `result`; parse them too.
        if !resp.is_success() && !resp.body.starts_with(b"{") {
            return Err(SourceError::Status {
                status: resp.status,
                url,
            });
        }
        let urls = streams_from_selection(&resp.body, service)?;
        Ok(Some(StreamDescriptor {
            urls,
            ttl: Some(STREAM_TTL),
            ..StreamDescriptor::default()
        }))
    }
}
