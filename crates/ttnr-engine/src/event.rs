//! Engine events: what the player reports to the rest of ttnr, and the
//! pure translation from raw mpv events to [`EngineEvent`]s.

use libmpv2::mpv_error;
use ttnr_core::StreamInfo;

/// Receiving end of the engine's event channel.
pub type EngineEvents = tokio::sync::mpsc::UnboundedReceiver<EngineEvent>;

/// Something the playback engine wants the app to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// A new stream started loading (after [`crate::Player::play`]).
    Loading,
    /// Audio is playing (first start, after buffering, or after a seek).
    Playing,
    /// Playback is stalled waiting for network data.
    Buffering {
        /// Cache fill towards resuming, if mpv reports it.
        percent: Option<u8>,
    },
    /// The pause state changed.
    Paused(bool),
    /// The stream format became known or changed (SPEC F2.4).
    StreamInfo(StreamInfo),
    /// The stream ended normally (finite file or a live stream that closed).
    Eof,
    /// Playback was stopped by a command (stop, or replaced by a new play).
    Stopped,
    /// The stream failed.
    Error {
        /// Classified cause; drives retry / re-resolve decisions in core.
        kind: ErrorKind,
        /// The most relevant mpv/FFmpeg log line, for display and logs.
        detail: Option<String>,
    },
    /// The mpv core shut down; no further events follow.
    Shutdown,
}

/// Why a stream failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// HTTP 403 Forbidden (often a stale or geo-blocked stream URL).
    Http403,
    /// HTTP 404 Not Found (often a moved stream URL).
    Http404,
    /// Any other HTTP error status.
    Http(u16),
    /// Connection refused, timed out, DNS or TLS failure.
    Unreachable,
    /// Opened, but no playable audio (unknown format, no audio track).
    Unsupported,
    /// The audio output could not be initialised.
    AudioOutput,
    /// Loading failed without a more specific cause.
    LoadFailed,
    /// Any other mpv error code.
    Other(i32),
}

/// Why mpv ended the current file (subset of `mpv_end_file_reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndReason {
    Eof,
    Stop,
    /// Playlist redirect: mpv continues with the target, not an end.
    Redirect,
    /// Failed with this mpv error code.
    Error(i32),
}

/// A property value from mpv; `None` = property unavailable.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PropValue {
    Str(String),
    Int(i64),
    Double(f64),
    None,
}

impl PropValue {
    fn string(self) -> Option<String> {
        match self {
            Self::Str(s) if !s.is_empty() => Some(s),
            _ => None,
        }
    }

    fn uint(self) -> Option<u32> {
        match self {
            Self::Int(i) => u32::try_from(i).ok().filter(|v| *v > 0),
            Self::Double(d) if d >= 1.0 && d < f64::from(u32::MAX) => Some(d.round() as u32),
            _ => None,
        }
    }
}

/// Raw mpv events, decoupled from libmpv2's borrowed event type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RawEvent {
    StartFile,
    FileLoaded,
    PlaybackRestart,
    EndFile(EndReason),
    Pause(bool),
    PausedForCache(bool),
    CacheBufferingState(i64),
    /// A stream-format property (see `STREAM_INFO_PROPS` in `mpv.rs`).
    Prop {
        name: String,
        value: PropValue,
    },
    Log {
        prefix: String,
        text: String,
    },
    Shutdown,
}

/// A network failure seen in the log while loading the current file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NetFailure {
    Http(u16),
    Unreachable,
}

/// Turns [`RawEvent`]s into [`EngineEvent`]s, tracking per-file state.
#[derive(Debug, Default)]
pub(crate) struct Translator {
    loaded: bool,
    buffering: bool,
    paused: bool,
    buffer_percent: Option<u8>,
    net_failure: Option<(NetFailure, String)>,
    info: StreamInfo,
    /// Last `StreamInfo` sent; see [`Translator::flush`].
    sent_info: StreamInfo,
    /// Nominal bitrates (container, HLS variant) and the measured one.
    demux_bitrate: Option<u32>,
    hls_bitrate: Option<u32>,
    measured_bitrate: Option<u32>,
}

impl Translator {
    /// Translate one raw event; may yield zero or more engine events.
    pub(crate) fn translate(&mut self, raw: RawEvent) -> Vec<EngineEvent> {
        match raw {
            RawEvent::StartFile => {
                self.loaded = false;
                self.buffering = false;
                self.buffer_percent = None;
                self.net_failure = None;
                self.info = StreamInfo::default();
                self.sent_info = StreamInfo::default();
                self.demux_bitrate = None;
                self.hls_bitrate = None;
                self.measured_bitrate = None;
                vec![EngineEvent::Loading]
            }
            RawEvent::FileLoaded => {
                self.loaded = true;
                vec![]
            }
            RawEvent::PlaybackRestart => {
                self.loaded = true;
                if self.buffering {
                    vec![]
                } else {
                    vec![EngineEvent::Playing]
                }
            }
            RawEvent::PausedForCache(true) => {
                if self.buffering {
                    return vec![];
                }
                self.buffering = true;
                vec![EngineEvent::Buffering {
                    percent: self.buffer_percent,
                }]
            }
            RawEvent::PausedForCache(false) => {
                if !self.buffering {
                    return vec![];
                }
                self.buffering = false;
                if self.loaded {
                    vec![EngineEvent::Playing]
                } else {
                    vec![]
                }
            }
            RawEvent::CacheBufferingState(p) => {
                let p = u8::try_from(p.clamp(0, 100)).unwrap_or(0);
                if self.buffer_percent == Some(p) {
                    return vec![];
                }
                self.buffer_percent = Some(p);
                if self.buffering {
                    vec![EngineEvent::Buffering { percent: Some(p) }]
                } else {
                    vec![]
                }
            }
            RawEvent::Pause(on) => {
                if self.paused == on {
                    return vec![];
                }
                self.paused = on;
                vec![EngineEvent::Paused(on)]
            }
            RawEvent::EndFile(reason) => {
                self.loaded = false;
                self.buffering = false;
                match reason {
                    EndReason::Eof => vec![EngineEvent::Eof],
                    EndReason::Stop => vec![EngineEvent::Stopped],
                    EndReason::Redirect => vec![],
                    EndReason::Error(code) => {
                        let (kind, detail) = self.classify(code);
                        vec![EngineEvent::Error { kind, detail }]
                    }
                }
            }
            RawEvent::Log { prefix, text } => {
                if let Some(f) = parse_net_failure(&prefix, &text) {
                    self.net_failure = Some((f, text));
                }
                vec![]
            }
            RawEvent::Prop { name, value } => {
                self.apply_prop(&name, value);
                vec![]
            }
            RawEvent::Shutdown => vec![EngineEvent::Shutdown],
        }
    }

    /// Emit the accumulated [`StreamInfo`] if it changed since last sent.
    /// Called when mpv's event queue is drained, so a burst of property
    /// changes yields one event.
    pub(crate) fn flush(&mut self) -> Option<EngineEvent> {
        if self.info == self.sent_info {
            return None;
        }
        self.sent_info = self.info.clone();
        Some(EngineEvent::StreamInfo(self.info.clone()))
    }

    /// Update [`StreamInfo`] from one mpv property. Facts are sticky within
    /// a file: unavailable/empty values (e.g. while the decoder is torn
    /// down at end of file) never clear what is known; `StartFile` resets.
    fn apply_prop(&mut self, name: &str, value: PropValue) {
        fn keep<T>(slot: &mut Option<T>, new: Option<T>) {
            if new.is_some() {
                *slot = new;
            }
        }
        let info = &mut self.info;
        match name {
            "current-tracks/audio/codec" => keep(&mut info.codec, value.string()),
            "current-tracks/audio/codec-desc" => keep(&mut info.codec_desc, value.string()),
            "current-tracks/audio/codec-profile" => keep(&mut info.codec_profile, value.string()),
            "file-format" => keep(&mut info.container, value.string()),
            "audio-params/samplerate" => keep(&mut info.sample_rate, value.uint()),
            "audio-params/format" => {
                keep(&mut info.sample_format, value.string());
                info.bit_depth = info
                    .sample_format
                    .as_deref()
                    .and_then(StreamInfo::bit_depth_of);
            }
            "audio-params/hr-channels" => keep(&mut info.channels, value.string()),
            "audio-params/channel-count" => keep(&mut info.channel_count, value.uint()),
            "audio-out-params/samplerate" => keep(&mut info.output_sample_rate, value.uint()),
            "audio-out-params/hr-channels" => keep(&mut info.output_channels, value.string()),
            "current-tracks/audio/demux-bitrate" => keep(&mut self.demux_bitrate, value.uint()),
            "current-tracks/audio/hls-bitrate" => keep(&mut self.hls_bitrate, value.uint()),
            // Measured bitrate jitters; keep kbps resolution to avoid churn.
            "audio-bitrate" => keep(
                &mut self.measured_bitrate,
                value.uint().map(|b| (b + 500) / 1000 * 1000),
            ),
            _ => {}
        }
        self.info.bitrate = self
            .demux_bitrate
            .or(self.hls_bitrate)
            .or(self.measured_bitrate);
    }

    /// Classify a failed file from the mpv error code and the last network
    /// failure logged for it.
    fn classify(&mut self, code: i32) -> (ErrorKind, Option<String>) {
        if let Some((failure, line)) = self.net_failure.take() {
            let kind = match failure {
                NetFailure::Http(403) => ErrorKind::Http403,
                NetFailure::Http(404) => ErrorKind::Http404,
                NetFailure::Http(status) => ErrorKind::Http(status),
                NetFailure::Unreachable => ErrorKind::Unreachable,
            };
            return (kind, Some(line));
        }
        let kind = match code {
            c if c == mpv_error::AoInitFailed => ErrorKind::AudioOutput,
            c if c == mpv_error::UnknownFormat
                || c == mpv_error::NothingToPlay
                || c == mpv_error::Unsupported =>
            {
                ErrorKind::Unsupported
            }
            c if c == mpv_error::LoadingFailed => ErrorKind::LoadFailed,
            c => ErrorKind::Other(c),
        };
        (kind, None)
    }
}

/// Recognise FFmpeg network errors in mpv log lines. Formats observed with
/// mpv 0.41.0 / FFmpeg 9.0.2 (docs/RESEARCH.md):
/// `[ffmpeg] http: HTTP error 403 Forbidden`,
/// `[ffmpeg] tcp: Connection to tcp://host:port failed: …`,
/// `[ffmpeg] tcp: Failed to resolve hostname …`.
fn parse_net_failure(prefix: &str, text: &str) -> Option<NetFailure> {
    if prefix != "ffmpeg" {
        return None;
    }
    if let Some(rest) = text.strip_prefix("http: HTTP error ") {
        let status = rest.split_whitespace().next()?.parse().ok()?;
        return Some(NetFailure::Http(status));
    }
    if text.starts_with("tcp: ") || text.starts_with("tls: ") {
        return Some(NetFailure::Unreachable);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use EngineEvent as E;

    fn log(text: &str) -> RawEvent {
        RawEvent::Log {
            prefix: "ffmpeg".into(),
            text: text.into(),
        }
    }

    fn run(events: impl IntoIterator<Item = RawEvent>) -> Vec<EngineEvent> {
        let mut t = Translator::default();
        events.into_iter().flat_map(|e| t.translate(e)).collect()
    }

    #[test]
    fn normal_play_to_eof() {
        let out = run([
            RawEvent::StartFile,
            RawEvent::FileLoaded,
            RawEvent::PlaybackRestart,
            RawEvent::EndFile(EndReason::Eof),
        ]);
        assert_eq!(out, [E::Loading, E::Playing, E::Eof]);
    }

    #[test]
    fn buffering_reports_progress_then_resumes() {
        let out = run([
            RawEvent::StartFile,
            RawEvent::PlaybackRestart,
            RawEvent::CacheBufferingState(100),
            RawEvent::PausedForCache(true),
            RawEvent::CacheBufferingState(30),
            RawEvent::CacheBufferingState(30),
            RawEvent::CacheBufferingState(80),
            RawEvent::PausedForCache(false),
        ]);
        assert_eq!(
            out,
            [
                E::Loading,
                E::Playing,
                E::Buffering { percent: Some(100) },
                E::Buffering { percent: Some(30) },
                E::Buffering { percent: Some(80) },
                E::Playing,
            ]
        );
    }

    #[test]
    fn no_playing_while_buffering() {
        let out = run([
            RawEvent::StartFile,
            RawEvent::PausedForCache(true),
            RawEvent::PlaybackRestart,
        ]);
        assert_eq!(out, [E::Loading, E::Buffering { percent: None }]);
    }

    #[test]
    fn pause_changes_only() {
        let out = run([
            RawEvent::Pause(false),
            RawEvent::Pause(true),
            RawEvent::Pause(true),
            RawEvent::Pause(false),
        ]);
        assert_eq!(out, [E::Paused(true), E::Paused(false)]);
    }

    #[test]
    fn stop_and_redirect() {
        let out = run([
            RawEvent::EndFile(EndReason::Redirect),
            RawEvent::EndFile(EndReason::Stop),
        ]);
        assert_eq!(out, [E::Stopped]);
    }

    #[test]
    fn http_403_and_404_from_log() {
        for (line, kind) in [
            ("http: HTTP error 403 Forbidden", ErrorKind::Http403),
            ("http: HTTP error 404 Not Found", ErrorKind::Http404),
            (
                "http: HTTP error 500 Internal Server Error",
                ErrorKind::Http(500),
            ),
        ] {
            let out = run([
                RawEvent::StartFile,
                log(line),
                RawEvent::EndFile(EndReason::Error(mpv_error::LoadingFailed)),
            ]);
            assert_eq!(
                out[1],
                E::Error {
                    kind,
                    detail: Some(line.into())
                }
            );
        }
    }

    #[test]
    fn unreachable_from_log() {
        for line in [
            "tcp: Connection to tcp://127.0.0.1:1 failed: Connection refused",
            "tcp: Failed to resolve hostname no-such-host.invalid: nodename nor servname provided, or not known",
        ] {
            let out = run([
                log(line),
                RawEvent::EndFile(EndReason::Error(mpv_error::LoadingFailed)),
            ]);
            assert!(matches!(
                out[0],
                E::Error {
                    kind: ErrorKind::Unreachable,
                    ..
                }
            ));
        }
    }

    #[test]
    fn net_failure_is_forgotten_on_next_file() {
        let out = run([
            log("http: HTTP error 403 Forbidden"),
            RawEvent::StartFile,
            RawEvent::EndFile(EndReason::Error(mpv_error::UnknownFormat)),
        ]);
        assert_eq!(
            out[1],
            E::Error {
                kind: ErrorKind::Unsupported,
                detail: None
            }
        );
    }

    #[test]
    fn error_codes_without_log() {
        for (code, kind) in [
            (mpv_error::AoInitFailed, ErrorKind::AudioOutput),
            (mpv_error::NothingToPlay, ErrorKind::Unsupported),
            (mpv_error::LoadingFailed, ErrorKind::LoadFailed),
            (mpv_error::Generic, ErrorKind::Other(mpv_error::Generic)),
        ] {
            let out = run([RawEvent::EndFile(EndReason::Error(code))]);
            assert_eq!(out, [E::Error { kind, detail: None }]);
        }
    }

    fn prop(name: &str, value: PropValue) -> RawEvent {
        RawEvent::Prop {
            name: name.into(),
            value,
        }
    }

    #[test]
    fn stream_info_is_coalesced_until_flush() {
        let mut t = Translator::default();
        for raw in [
            RawEvent::StartFile,
            prop("current-tracks/audio/codec", PropValue::Str("aac".into())),
            prop("audio-params/samplerate", PropValue::Int(48_000)),
            prop("audio-params/format", PropValue::Str("s16".into())),
        ] {
            let out = t.translate(raw);
            assert!(!out.iter().any(|e| matches!(e, E::StreamInfo(_))));
        }
        let Some(E::StreamInfo(info)) = t.flush() else {
            panic!("expected one StreamInfo")
        };
        assert_eq!(info.codec.as_deref(), Some("aac"));
        assert_eq!(info.sample_rate, Some(48_000));
        assert_eq!(info.bit_depth, Some(16));
        assert_eq!(t.flush(), None, "unchanged info is not re-sent");
        t.translate(prop("audio-params/samplerate", PropValue::Int(48_000)));
        assert_eq!(t.flush(), None, "same value is not a change");
    }

    #[test]
    fn nominal_bitrate_wins_over_measured() {
        let mut t = Translator::default();
        t.translate(prop("audio-bitrate", PropValue::Double(127_400.0)));
        assert_eq!(t.info.bitrate, Some(127_000));
        t.translate(prop("audio-bitrate", PropValue::Double(127_600.0)));
        assert_eq!(t.info.bitrate, Some(128_000));
        t.translate(prop(
            "current-tracks/audio/hls-bitrate",
            PropValue::Int(96_000),
        ));
        assert_eq!(t.info.bitrate, Some(96_000));
        t.translate(prop(
            "current-tracks/audio/demux-bitrate",
            PropValue::Int(320_000),
        ));
        assert_eq!(t.info.bitrate, Some(320_000));
    }

    #[test]
    fn stream_info_resets_on_new_file() {
        let mut t = Translator::default();
        t.translate(prop("audio-params/samplerate", PropValue::Int(44_100)));
        assert!(t.flush().is_some());
        t.translate(RawEvent::StartFile);
        assert_eq!(t.info, StreamInfo::default());
        t.translate(prop("audio-params/samplerate", PropValue::Int(44_100)));
        assert!(
            t.flush().is_some(),
            "same value in a new file is sent again"
        );
    }

    #[test]
    fn facts_are_sticky_within_a_file() {
        let mut t = Translator::default();
        t.translate(prop(
            "audio-params/hr-channels",
            PropValue::Str("stereo".into()),
        ));
        t.translate(prop(
            "audio-params/hr-channels",
            PropValue::Str(String::new()),
        ));
        t.translate(prop("audio-params/hr-channels", PropValue::None));
        assert_eq!(t.info.channels.as_deref(), Some("stereo"));
        t.translate(prop("audio-params/samplerate", PropValue::Int(44_100)));
        t.translate(prop("audio-params/samplerate", PropValue::Int(0)));
        assert_eq!(t.info.sample_rate, Some(44_100));
        t.translate(prop(
            "audio-params/hr-channels",
            PropValue::Str("5.1".into()),
        ));
        assert_eq!(
            t.info.channels.as_deref(),
            Some("5.1"),
            "real changes apply"
        );
    }

    #[test]
    fn other_log_lines_are_ignored() {
        assert_eq!(
            parse_net_failure("ffmpeg/demuxer", "http: HTTP error 403 x"),
            None
        );
        assert_eq!(
            parse_net_failure("ffmpeg", "hls: Failed to open segment 1"),
            None
        );
        assert_eq!(parse_net_failure("ffmpeg", "http: HTTP error abc"), None);
    }
}
