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

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http403 => f.write_str("HTTP 403 Forbidden"),
            Self::Http404 => f.write_str("HTTP 404 Not Found"),
            Self::Http(status) => write!(f, "HTTP {status}"),
            Self::Unreachable => f.write_str("server unreachable"),
            Self::Unsupported => f.write_str("no playable audio"),
            Self::AudioOutput => f.write_str("audio output failed"),
            Self::LoadFailed => f.write_str("loading failed"),
            Self::Other(code) => write!(f, "mpv error {code}"),
        }
    }
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
    /// mpv reconfigured the audio output (no engine event of its own).
    AudioReconfig,
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
    /// Mirror of mpv's current stream properties (unavailable = `None`).
    /// Never reset on `StartFile`: mpv only notifies *changes*, so a value
    /// equal to the previous file's is not re-sent.
    info: StreamInfo,
    /// Last `StreamInfo` sent in this file; see [`Translator::flush`].
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
                self.sent_info = StreamInfo::default();
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
            RawEvent::AudioReconfig => vec![],
            RawEvent::Shutdown => vec![EngineEvent::Shutdown],
        }
    }

    /// Emit the current [`StreamInfo`] if it changed since last sent.
    /// Called when mpv's event queue is drained, so a burst of property
    /// changes yields one event. Only while a file is loaded (not during
    /// teardown); within a file, facts are sticky: a field that becomes
    /// unavailable keeps its last sent value. `StartFile` forgets what was
    /// sent, so the next file is reported from the mirror afresh.
    pub(crate) fn flush(&mut self) -> Option<EngineEvent> {
        if !self.loaded {
            return None;
        }
        let next = sticky(&self.info, &self.sent_info);
        if next == self.sent_info {
            return None;
        }
        self.sent_info = next.clone();
        Some(EngineEvent::StreamInfo(next))
    }

    /// Mirror one mpv property into [`Self::info`].
    fn apply_prop(&mut self, name: &str, value: PropValue) {
        let info = &mut self.info;
        match name {
            "current-tracks/audio/codec" => info.codec = value.string(),
            "current-tracks/audio/codec-desc" => info.codec_desc = value.string(),
            "current-tracks/audio/codec-profile" => info.codec_profile = value.string(),
            "file-format" => info.container = value.string(),
            "audio-params/samplerate" => info.sample_rate = value.uint(),
            "audio-params/format" => {
                info.sample_format = value.string();
                info.bit_depth = info
                    .sample_format
                    .as_deref()
                    .and_then(StreamInfo::bit_depth_of);
            }
            "audio-params/hr-channels" => info.channels = value.string(),
            "audio-params/channel-count" => info.channel_count = value.uint(),
            "audio-out-params/samplerate" => info.output_sample_rate = value.uint(),
            "audio-out-params/hr-channels" => info.output_channels = value.string(),
            "current-tracks/audio/demux-bitrate" => self.demux_bitrate = value.uint(),
            "current-tracks/audio/hls-bitrate" => self.hls_bitrate = value.uint(),
            "audio-bitrate" => self.measured_bitrate = smooth(self.measured_bitrate, value.uint()),
            _ => {}
        }
        // The measured (packet-level) rate of a lossless stream says
        // nothing about its format and swings with the content.
        let measured = self
            .measured_bitrate
            .filter(|_| !self.info.codec.as_deref().is_some_and(is_lossless));
        self.info.bitrate = self.demux_bitrate.or(self.hls_bitrate).or(measured);
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

/// Codecs whose bitrate varies with content by nature (FFmpeg names).
fn is_lossless(codec: &str) -> bool {
    codec.starts_with("pcm_")
        || codec.starts_with("dsd_")
        || matches!(
            codec,
            "flac" | "alac" | "wavpack" | "tta" | "ape" | "truehd" | "mlp" | "shorten"
        )
}

/// Measured bitrate with hysteresis: rounded to kbps and only updated when
/// it moves by 10 % or more, so VBR jitter does not flood the UI.
fn smooth(old: Option<u32>, new: Option<u32>) -> Option<u32> {
    let new = new.map(|b| (b + 500) / 1000 * 1000)?;
    match old {
        Some(old) if new.abs_diff(old) * 10 < old => Some(old),
        _ => Some(new),
    }
}

/// Field-wise `current`, falling back to `sent` where `current` is `None`.
fn sticky(current: &StreamInfo, sent: &StreamInfo) -> StreamInfo {
    fn or<T: Clone>(a: &Option<T>, b: &Option<T>) -> Option<T> {
        a.clone().or_else(|| b.clone())
    }
    StreamInfo {
        codec: or(&current.codec, &sent.codec),
        codec_desc: or(&current.codec_desc, &sent.codec_desc),
        codec_profile: or(&current.codec_profile, &sent.codec_profile),
        container: or(&current.container, &sent.container),
        sample_rate: or(&current.sample_rate, &sent.sample_rate),
        sample_format: or(&current.sample_format, &sent.sample_format),
        bit_depth: or(&current.bit_depth, &sent.bit_depth),
        channels: or(&current.channels, &sent.channels),
        channel_count: or(&current.channel_count, &sent.channel_count),
        bitrate: or(&current.bitrate, &sent.bitrate),
        output_sample_rate: or(&current.output_sample_rate, &sent.output_sample_rate),
        output_channels: or(&current.output_channels, &sent.output_channels),
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

    fn info_of(event: Option<EngineEvent>) -> StreamInfo {
        match event {
            Some(E::StreamInfo(info)) => info,
            other => panic!("expected StreamInfo, got {other:?}"),
        }
    }

    #[test]
    fn stream_info_is_coalesced_until_flush() {
        let mut t = Translator::default();
        for raw in [
            RawEvent::StartFile,
            RawEvent::FileLoaded,
            prop("current-tracks/audio/codec", PropValue::Str("aac".into())),
            prop("audio-params/samplerate", PropValue::Int(48_000)),
            prop("audio-params/format", PropValue::Str("s16".into())),
        ] {
            let out = t.translate(raw);
            assert!(!out.iter().any(|e| matches!(e, E::StreamInfo(_))));
        }
        let info = info_of(t.flush());
        assert_eq!(info.codec.as_deref(), Some("aac"));
        assert_eq!(info.sample_rate, Some(48_000));
        assert_eq!(info.bit_depth, Some(16));
        assert_eq!(t.flush(), None, "unchanged info is not re-sent");
        t.translate(prop("audio-params/samplerate", PropValue::Int(48_000)));
        assert_eq!(t.flush(), None, "same value is not a change");
    }

    #[test]
    fn nothing_is_sent_until_the_file_is_loaded() {
        let mut t = Translator::default();
        t.translate(RawEvent::StartFile);
        t.translate(prop("audio-params/samplerate", PropValue::Int(44_100)));
        assert_eq!(t.flush(), None);
        t.translate(RawEvent::FileLoaded);
        assert_eq!(info_of(t.flush()).sample_rate, Some(44_100));
    }

    #[test]
    fn nominal_bitrate_wins_over_measured() {
        let mut t = Translator::default();
        t.translate(prop("audio-bitrate", PropValue::Double(127_400.0)));
        assert_eq!(t.info.bitrate, Some(127_000));
        t.translate(prop("audio-bitrate", PropValue::Double(131_600.0)));
        assert_eq!(t.info.bitrate, Some(127_000), "jitter under 10 % ignored");
        t.translate(prop("audio-bitrate", PropValue::Double(150_000.0)));
        assert_eq!(t.info.bitrate, Some(150_000));
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
        t.translate(prop("current-tracks/audio/demux-bitrate", PropValue::None));
        assert_eq!(t.info.bitrate, Some(96_000), "mirror follows mpv");
    }

    #[test]
    fn lossless_codecs_get_no_measured_bitrate() {
        let mut t = Translator::default();
        t.translate(prop("audio-bitrate", PropValue::Double(900_000.0)));
        assert_eq!(t.info.bitrate, Some(900_000));
        t.translate(prop(
            "current-tracks/audio/codec",
            PropValue::Str("flac".into()),
        ));
        t.translate(prop("audio-bitrate", PropValue::Double(500_000.0)));
        assert_eq!(t.info.bitrate, None);
        t.translate(prop(
            "current-tracks/audio/demux-bitrate",
            PropValue::Int(1_411_200),
        ));
        assert_eq!(t.info.bitrate, Some(1_411_200), "nominal still shown");
        assert!(is_lossless("pcm_s24le") && is_lossless("alac") && !is_lossless("aac"));
    }

    #[test]
    fn unchanged_values_carry_over_to_the_next_file() {
        // mpv does not re-notify a value equal to the previous file's.
        let mut t = Translator::default();
        t.translate(RawEvent::StartFile);
        t.translate(RawEvent::FileLoaded);
        t.translate(prop("audio-params/samplerate", PropValue::Int(48_000)));
        t.translate(prop(
            "current-tracks/audio/codec-profile",
            PropValue::Str("HE-AAC".into()),
        ));
        info_of(t.flush());
        t.translate(RawEvent::EndFile(EndReason::Stop));
        t.translate(RawEvent::StartFile);
        t.translate(RawEvent::FileLoaded);
        t.translate(prop(
            "current-tracks/audio/codec-profile",
            PropValue::Str("HE-AACv2".into()),
        ));
        let info = info_of(t.flush());
        assert_eq!(info.sample_rate, Some(48_000));
        assert_eq!(info.codec_profile.as_deref(), Some("HE-AACv2"));
    }

    #[test]
    fn teardown_is_not_reported_and_absent_fields_clear_in_next_file() {
        let mut t = Translator::default();
        t.translate(RawEvent::StartFile);
        t.translate(RawEvent::FileLoaded);
        t.translate(prop(
            "current-tracks/audio/codec",
            PropValue::Str("aac".into()),
        ));
        t.translate(prop(
            "current-tracks/audio/codec-profile",
            PropValue::Str("LC".into()),
        ));
        info_of(t.flush());
        // Teardown: properties vanish before end-file is delivered.
        t.translate(prop("current-tracks/audio/codec-profile", PropValue::None));
        t.translate(prop("current-tracks/audio/codec", PropValue::None));
        assert_eq!(t.flush(), None, "sticky within the file");
        t.translate(RawEvent::EndFile(EndReason::Eof));
        // Next file (mp3: no profile).
        t.translate(RawEvent::StartFile);
        t.translate(RawEvent::FileLoaded);
        t.translate(prop(
            "current-tracks/audio/codec",
            PropValue::Str("mp3".into()),
        ));
        let info = info_of(t.flush());
        assert_eq!(info.codec.as_deref(), Some("mp3"));
        assert_eq!(info.codec_profile, None, "stale profile not carried over");
    }

    #[test]
    fn facts_are_sticky_within_a_file() {
        let mut t = Translator::default();
        t.translate(RawEvent::FileLoaded);
        t.translate(prop(
            "audio-params/hr-channels",
            PropValue::Str("stereo".into()),
        ));
        info_of(t.flush());
        t.translate(prop(
            "audio-params/hr-channels",
            PropValue::Str(String::new()),
        ));
        assert_eq!(t.flush(), None);
        t.translate(prop(
            "audio-params/hr-channels",
            PropValue::Str("5.1".into()),
        ));
        assert_eq!(info_of(t.flush()).channels.as_deref(), Some("5.1"));
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
