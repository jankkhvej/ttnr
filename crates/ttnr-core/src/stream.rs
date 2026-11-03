//! Stream-level domain types: what to play ([`StreamDescriptor`]) and
//! what is actually playing ([`StreamInfo`]).

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Something playable *now*, produced by a source at play time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamDescriptor {
    /// Ordered candidate URLs (quality / region fallbacks), best first.
    pub urls: Vec<String>,
    /// Extra HTTP request headers as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    /// When this was resolved (`None` for ad-hoc descriptors).
    pub resolved_at: Option<SystemTime>,
    /// How long the URLs may be reused from the cache (`None` = default).
    pub ttl: Option<Duration>,
    /// Which resolution steps were tried and how they ended (`--debug`).
    pub trace: ResolverTrace,
}

impl StreamDescriptor {
    /// A descriptor for a single URL with no extra headers.
    pub fn direct(url: impl Into<String>) -> Self {
        Self {
            urls: vec![url.into()],
            ..Self::default()
        }
    }
}

/// A resolution strategy of the resolver chain (ARCHITECTURE.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    /// Last good URL from the on-disk cache.
    Cache,
    /// The source's authoritative lookup.
    Discovery,
    /// Known URL templates, probed in order.
    Patterns,
    /// A URL pinned by the user in `stations.toml`.
    Override,
}

/// One step of a resolution attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    /// The strategy tried.
    pub strategy: Strategy,
    /// What happened, human-readable (e.g. `hit`, `HTTP 404 for …`).
    pub outcome: String,
}

/// Ordered record of a resolution attempt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolverTrace(pub Vec<TraceStep>);

impl ResolverTrace {
    /// Append a step.
    pub fn push(&mut self, strategy: Strategy, outcome: impl Into<String>) {
        self.0.push(TraceStep {
            strategy,
            outcome: outcome.into(),
        });
    }
}

impl std::fmt::Display for ResolverTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, step) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" → ")?;
            }
            write!(f, "{:?}: {}", step.strategy, step.outcome)?;
        }
        Ok(())
    }
}

/// The actual format of the stream being played (SPEC F2.4), as reported
/// by the decoder. Every field is optional: not all streams expose all
/// facts, and they arrive one by one while a stream opens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamInfo {
    /// Short codec name, e.g. `aac`, `mp3`, `flac`, `opus`.
    pub codec: Option<String>,
    /// Descriptive codec name, e.g. `AAC (Advanced Audio Coding)`.
    pub codec_desc: Option<String>,
    /// Codec profile when known, e.g. `LC`, `HE-AAC`, `HE-AACv2`.
    pub codec_profile: Option<String>,
    /// Container / demuxer format, e.g. `hls`, `flac`, `mov,mp4,m4a,…`.
    pub container: Option<String>,
    /// Decoded sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Decoder sample format (`s16`, `s32`, `floatp`, …).
    pub sample_format: Option<String>,
    /// Bit depth, only when unambiguous (integer `u8`/`s16` formats); 24-bit
    /// sources decode to `s32`, so `s32` yields `None`.
    pub bit_depth: Option<u8>,
    /// Human-readable channel layout, e.g. `mono`, `stereo`, `5.1(side)`.
    pub channels: Option<String>,
    /// Number of decoded channels.
    pub channel_count: Option<u32>,
    /// Bitrate in bits/s: nominal (container or HLS variant) if known,
    /// else measured.
    pub bitrate: Option<u32>,
    /// Sample rate sent to the audio device (differs if resampled).
    pub output_sample_rate: Option<u32>,
    /// Channel layout sent to the audio device (differs if downmixed).
    pub output_channels: Option<String>,
}

impl StreamInfo {
    /// Bit depth implied by an mpv sample format name, if unambiguous.
    pub fn bit_depth_of(sample_format: &str) -> Option<u8> {
        match sample_format.trim_end_matches('p') {
            "u8" => Some(8),
            "s16" => Some(16),
            _ => None,
        }
    }

    /// One-line summary for the UI and logs, e.g.
    /// `AAC LC · 48 kHz · 16-bit · stereo · 128 kbps`. Empty if nothing
    /// is known yet.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(codec) = &self.codec {
            let codec = codec.to_uppercase();
            // "HE-AAC" already says AAC; "LC" does not.
            let c = match &self.codec_profile {
                Some(p) if p.to_uppercase().contains(&codec) => p.clone(),
                Some(p) => format!("{codec} {p}"),
                None => codec,
            };
            parts.push(c);
        }
        if let Some(rate) = self.sample_rate {
            parts.push(format_khz(rate));
        }
        if let Some(bits) = self.bit_depth {
            parts.push(format!("{bits}-bit"));
        }
        if let Some(ch) = &self.channels {
            parts.push(ch.clone());
        }
        if let Some(br) = self.bitrate {
            parts.push(format!("{} kbps", (br + 500) / 1000));
        }
        let mut line = parts.join(" · ");
        let resampled = self
            .output_sample_rate
            .is_some_and(|out| Some(out) != self.sample_rate);
        let remixed = self
            .output_channels
            .as_ref()
            .is_some_and(|out| Some(out) != self.channels.as_ref());
        if resampled || remixed {
            let mut out = Vec::new();
            if let Some(rate) = self.output_sample_rate {
                out.push(format_khz(rate));
            }
            if let Some(ch) = &self.output_channels {
                out.push(ch.clone());
            }
            line.push_str(&format!(" → {}", out.join(" · ")));
        }
        line
    }
}

/// `44100` → `44.1 kHz`, `48000` → `48 kHz`.
fn format_khz(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{:.1} kHz", f64::from(hz) / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_depth_only_when_unambiguous() {
        assert_eq!(StreamInfo::bit_depth_of("s16"), Some(16));
        assert_eq!(StreamInfo::bit_depth_of("s16p"), Some(16));
        assert_eq!(StreamInfo::bit_depth_of("u8"), Some(8));
        assert_eq!(StreamInfo::bit_depth_of("s32"), None);
        assert_eq!(StreamInfo::bit_depth_of("floatp"), None);
    }

    #[test]
    fn summary_full_and_empty() {
        assert_eq!(StreamInfo::default().summary(), "");
        let info = StreamInfo {
            codec: Some("aac".into()),
            codec_profile: Some("HE-AACv2".into()),
            sample_rate: Some(44_100),
            channels: Some("stereo".into()),
            bitrate: Some(95_800),
            output_sample_rate: Some(44_100),
            output_channels: Some("stereo".into()),
            ..StreamInfo::default()
        };
        assert_eq!(info.summary(), "HE-AACv2 · 44.1 kHz · stereo · 96 kbps");
        let lc = StreamInfo {
            codec: Some("aac".into()),
            codec_profile: Some("LC".into()),
            ..StreamInfo::default()
        };
        assert_eq!(lc.summary(), "AAC LC");
    }

    #[test]
    fn summary_shows_resampling_and_downmix() {
        let info = StreamInfo {
            codec: Some("ac3".into()),
            sample_rate: Some(48_000),
            channels: Some("5.1(side)".into()),
            output_sample_rate: Some(44_100),
            output_channels: Some("stereo".into()),
            ..StreamInfo::default()
        };
        assert_eq!(
            info.summary(),
            "AC3 · 48 kHz · 5.1(side) → 44.1 kHz · stereo"
        );
    }

    #[test]
    fn direct_has_one_url_and_no_headers() {
        let d = StreamDescriptor::direct("https://example.org/a.m3u8");
        assert_eq!(d.urls, ["https://example.org/a.m3u8"]);
        assert!(d.headers.is_empty());
    }
}
