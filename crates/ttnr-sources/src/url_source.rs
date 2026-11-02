//! The `url` source: any URL the user enters (SPEC F1.1) – direct streams,
//! HLS, and playlist files (`m3u`, `pls`, `xspf`, `asx`) that point to
//! streams.

use std::time::Duration;

use async_trait::async_trait;
use ttnr_core::{
    MetadataProvider, ResolveCtx, Source, SourceCaps, SourceError, Station, StreamDescriptor,
};
use url::Url;

use crate::hls_id3::HlsId3Provider;
use crate::playlist::{self, Kind};

/// Source id.
pub const ID: &str = ttnr_core::station::URL_SOURCE;

/// How long an expanded playlist may be reused from the cache.
const PLAYLIST_TTL: Duration = Duration::from_secs(60 * 60);

/// Resolves a station whose `url` is the thing to play.
#[derive(Debug, Default, Clone, Copy)]
pub struct UrlSource;

impl UrlSource {
    /// An ad-hoc station for a URL typed by the user (`ttnr play <url>`).
    pub fn station(url: &str) -> Station {
        Station::from_url(url)
    }
}

#[async_trait]
impl Source for UrlSource {
    fn id(&self) -> &'static str {
        ID
    }

    fn capabilities(&self) -> SourceCaps {
        SourceCaps::default()
    }

    /// HLS URLs get the segment-ID3 poller (it stops by itself if the
    /// stream carries no ID3 now-playing frames).
    fn metadata(&self, st: &Station, ctx: &ResolveCtx) -> Option<Box<dyn MetadataProvider>> {
        let url = st.url.as_deref()?;
        let path = url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .to_ascii_lowercase();
        path.ends_with(".m3u8").then(|| {
            Box::new(HlsId3Provider::new(url, ctx.http.clone())) as Box<dyn MetadataProvider>
        })
    }

    /// Fetch the URL's head: media stream or HLS → play the URL as is;
    /// playlist → its entries (mirrors become fallback candidates). Errors
    /// end in the chain's override, i.e. the URL itself, so mpv still gets
    /// to try (and report a classified error).
    async fn discover(
        &self,
        st: &Station,
        ctx: &ResolveCtx,
    ) -> Result<Option<StreamDescriptor>, SourceError> {
        let Some(url) = st.url.as_deref() else {
            return Err(SourceError::Other(format!(
                "station {} has no URL",
                st.key()
            )));
        };
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            // Local files, mms://, rtsp://…: nothing to expand.
            return Ok(Some(StreamDescriptor::direct(url)));
        }
        let resp = match ctx.http.get(url, &[]).await {
            Ok(resp) => resp,
            // E.g. Shoutcast v1 answers "ICY 200 OK", which HTTP clients
            // reject but mpv plays fine.
            Err(e) => {
                tracing::debug!(url, error = %e, "cannot inspect URL; passing it to the player");
                return Ok(Some(StreamDescriptor::direct(url)));
            }
        };
        if !resp.is_success() {
            return Err(SourceError::Status {
                status: resp.status,
                url: url.to_owned(),
            });
        }
        if resp.streaming {
            return Ok(Some(StreamDescriptor::direct(url)));
        }
        let body = playlist::decode(&resp.body);
        match playlist::detect(&resp.final_url, resp.content_type.as_deref(), &body) {
            None | Some(Kind::Hls) => Ok(Some(StreamDescriptor::direct(url))),
            Some(kind) => {
                let base = Url::parse(&resp.final_url).map_err(|e| SourceError::Parse {
                    what: "playlist URL".into(),
                    detail: e.to_string(),
                })?;
                let entries = playlist::parse(kind, &body, &base);
                if entries.is_empty() {
                    return Err(SourceError::Parse {
                        what: format!("{kind:?} playlist"),
                        detail: "no stream URLs".into(),
                    });
                }
                Ok(Some(StreamDescriptor {
                    urls: entries.into_iter().map(|e| e.url).collect(),
                    ttl: Some(PLAYLIST_TTL),
                    ..StreamDescriptor::default()
                }))
            }
        }
    }
}
