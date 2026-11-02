//! Engine events: what the player reports to the rest of ttnr, and the
//! pure translation from raw mpv events to [`EngineEvent`]s.

use libmpv2::mpv_error;

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
    Log { prefix: String, text: String },
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
            RawEvent::Shutdown => vec![EngineEvent::Shutdown],
        }
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
