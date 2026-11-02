//! [`HlsId3Provider`]: now-playing from ID3 tags at the start of HLS
//! packed-audio segments. mpv 0.41 / FFmpeg 9.0.2 do not surface those
//! (FFmpeg reports "Changing ID3 metadata in HLS audio elementary stream"
//! as a missing feature; docs/RESEARCH.md), so we read the newest segment's
//! first 8 KiB ourselves, like web players do.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use ttnr_core::{Http, InfoSource, MetadataProvider, NowPlaying, SourceError};
use url::Url;

use crate::id3::{self, Id3Info};

/// Bytes fetched from the start of a segment (all ID3 tags we saw fit in
/// ~1 KiB; mpv probes 8 KiB too).
const HEAD_BYTES: u64 = 8 * 1024;
/// Give up after this many segments without any ID3 info.
const MAX_SEGMENTS_WITHOUT_ID3: u32 = 3;
const MIN_POLL: Duration = Duration::from_secs(2);
const MAX_POLL: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Polls an HLS stream's newest segment for ID3 now-playing frames.
pub struct HlsId3Provider {
    url: String,
    http: Arc<dyn Http>,
}

impl HlsId3Provider {
    /// Provider for the HLS playlist at `url` (master or media).
    pub fn new(url: impl Into<String>, http: Arc<dyn Http>) -> Self {
        Self {
            url: url.into(),
            http,
        }
    }

    async fn fetch_text(&self, url: &str) -> Result<(String, Url), SourceError> {
        let resp = self.http.get(url, &[]).await?;
        if !resp.is_success() {
            return Err(SourceError::Status {
                status: resp.status,
                url: url.to_owned(),
            });
        }
        let base = Url::parse(&resp.final_url).map_err(|e| SourceError::Parse {
            what: "HLS URL".into(),
            detail: e.to_string(),
        })?;
        Ok((crate::playlist::decode(&resp.body), base))
    }

    /// The media playlist URL: `self.url` itself, or the first variant of a
    /// master playlist. `None` if the URL is not HLS at all.
    async fn media_playlist(&self) -> Result<Option<String>, SourceError> {
        let (text, base) = self.fetch_text(&self.url).await?;
        if !text.trim_start().starts_with("#EXTM3U") {
            return Ok(None);
        }
        if !text.contains("#EXT-X-STREAM-INF") {
            return Ok(Some(base.to_string()));
        }
        let variant = uris(&text).next().and_then(|u| base.join(u).ok());
        Ok(variant.map(|u| u.to_string()))
    }

    /// Newest segment URL and the target duration of a media playlist.
    async fn newest_segment(&self, media: &str) -> Result<(Option<String>, Duration), SourceError> {
        let (text, base) = self.fetch_text(media).await?;
        let target = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("#EXT-X-TARGETDURATION:"))
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map_or(Duration::from_secs(10), Duration::from_secs);
        let segment = uris(&text)
            .last()
            .and_then(|u| base.join(u).ok())
            .map(|u| u.to_string());
        Ok((segment, target))
    }

    async fn segment_id3(&self, segment: &str) -> Result<Id3Info, SourceError> {
        let range = [("Range".to_owned(), format!("bytes=0-{}", HEAD_BYTES - 1))];
        let resp = self.http.get(segment, &range).await?;
        if !resp.is_success() {
            return Err(SourceError::Status {
                status: resp.status,
                url: segment.to_owned(),
            });
        }
        Ok(id3::read_tags(&resp.body))
    }
}

/// URI lines of an M3U8 (non-empty, not comments).
fn uris(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn now_playing(info: Id3Info) -> NowPlaying {
    NowPlaying {
        station: info.station,
        artist: info.artist,
        title: info.title,
        album: info.album,
        artwork_url: info.artwork_url,
        ..NowPlaying::empty(InfoSource::Tags, SystemTime::now())
    }
}

#[async_trait]
impl MetadataProvider for HlsId3Provider {
    async fn run(self: Box<Self>, tx: Sender<NowPlaying>) -> Result<(), SourceError> {
        let mut backoff = Duration::from_secs(5);
        let mut media = None;
        let mut last_segment = None;
        let mut last_sent: Option<Id3Info> = None;
        let mut without_id3 = 0;
        loop {
            let step = async {
                if media.is_none() {
                    match self.media_playlist().await? {
                        Some(m) => media = Some(m),
                        None => return Ok::<_, SourceError>(None),
                    }
                }
                let (segment, target) = self
                    .newest_segment(media.as_deref().unwrap_or_default())
                    .await?;
                if segment.is_some() && segment != last_segment {
                    last_segment.clone_from(&segment);
                    let info = self
                        .segment_id3(segment.as_deref().unwrap_or_default())
                        .await?;
                    if info.title.is_none() && info.artist.is_none() {
                        without_id3 += 1;
                    } else {
                        without_id3 = 0;
                        if last_sent.as_ref() != Some(&info) {
                            last_sent = Some(info.clone());
                            if tx.send(now_playing(info)).await.is_err() {
                                return Ok(None);
                            }
                        }
                    }
                }
                Ok(Some(target.clamp(MIN_POLL, MAX_POLL)))
            };
            match step.await {
                Ok(None) => return Ok(()),
                Ok(Some(_)) if without_id3 >= MAX_SEGMENTS_WITHOUT_ID3 => {
                    tracing::debug!(url = %self.url, "no ID3 now-playing in segments; stop polling");
                    return Ok(());
                }
                Ok(Some(wait)) => {
                    backoff = Duration::from_secs(5);
                    tokio::time::sleep(wait).await;
                }
                Err(e) => {
                    tracing::debug!(url = %self.url, error = %e, ?backoff, "HLS ID3 poll failed");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
            if tx.is_closed() {
                return Ok(());
            }
        }
    }
}
