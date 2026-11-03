//! [`MpvPlayer`]: the libmpv implementation of [`Player`].

use std::sync::Arc;
use std::thread::JoinHandle;

use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv, mpv_end_file_reason};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use ttnr_core::StreamDescriptor;

use crate::event::{EndReason, EngineEvent, EngineEvents, PropValue, RawEvent, Translator};
use crate::{EngineConfig, EngineError, Player};

/// Options set before `mpv_initialize`, making the core audio-only and
/// isolated from the user's mpv setup. Names and values verified against
/// the mpv 0.41.0 manual (docs/RESEARCH.md).
const AUDIO_ONLY_OPTIONS: &[(&str, &str)] = &[
    ("vid", "no"),
    ("audio-display", "no"),
    ("force-window", "no"),
    ("config", "no"),
    ("load-scripts", "no"),
    ("terminal", "no"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("input-media-keys", "no"),
    ("idle", "yes"),
    ("audio-client-name", "ttnr"),
];

/// Switches for mpv's built-in Lua/JS scripts (none is useful headless;
/// `auto_profiles` even warns on every file). A libmpv built without Lua
/// (e.g. `scripts/build-libmpv.zsh`) has no such options – then there is
/// nothing to switch off, so "option not found" is fine.
const SCRIPT_OPTIONS: &[(&str, &str)] = &[
    ("ytdl", "no"),
    ("osc", "no"),
    ("load-auto-profiles", "no"),
    ("load-commands", "no"),
    ("load-console", "no"),
    ("load-context-menu", "no"),
    ("load-positioning", "no"),
    ("load-select", "no"),
    ("load-stats-overlay", "no"),
];

/// Properties observed for events: (name, format, reply id).
const OBSERVED: &[(&str, Format, u64)] = &[
    ("pause", Format::Flag, 1),
    ("paused-for-cache", Format::Flag, 2),
    ("cache-buffering-state", Format::Int64, 3),
    // Only a change signal: mpv notifies `metadata` (not its by-key
    // sub-properties) on ICY/tag updates; read as a string because libmpv2
    // cannot decode `Format::Node` events (ADR-020).
    ("metadata", Format::String, 4),
    // A node too; as a string mpv gives JSON. Notified on hotplug.
    ("audio-device-list", Format::String, 5),
];

/// Tag keys read on every `metadata` change and at file load.
const TAG_PROPS: &[&str] = &[
    "metadata/by-key/icy-title",
    "metadata/by-key/icy-name",
    "metadata/by-key/title",
    "metadata/by-key/artist",
    "metadata/by-key/album",
];

/// Properties feeding [`ttnr_core::StreamInfo`] (mpv 0.41.0 manual,
/// docs/RESEARCH.md). Observed as scalars: libmpv2 cannot decode
/// `Format::Node` property events.
const STREAM_INFO_PROPS: &[(&str, Format)] = &[
    ("current-tracks/audio/codec", Format::String),
    ("current-tracks/audio/codec-desc", Format::String),
    ("current-tracks/audio/codec-profile", Format::String),
    ("current-tracks/audio/demux-bitrate", Format::Int64),
    ("current-tracks/audio/hls-bitrate", Format::Int64),
    ("file-format", Format::String),
    ("audio-params/format", Format::String),
    ("audio-params/samplerate", Format::Int64),
    ("audio-params/hr-channels", Format::String),
    ("audio-params/channel-count", Format::Int64),
    ("audio-out-params/samplerate", Format::Int64),
    ("audio-out-params/hr-channels", Format::String),
    ("audio-bitrate", Format::Double),
    // Live streams are not seekable (ICY and live HLS); files are.
    ("seekable", Format::Flag),
    // Known size (Content-Length): files, also unseekable HTTP ones;
    // unavailable for Icecast streams.
    ("file-size", Format::Int64),
];

/// Reply id for all [`STREAM_INFO_PROPS`] (events carry the name).
const STREAM_INFO_ID: u64 = 100;

/// Version string of the linked libmpv, e.g. `"mpv 0.41.0"`.
///
/// Creates and drops a throw-away mpv core (no file loaded, no audio
/// output opened).
pub fn libmpv_version() -> Result<String, EngineError> {
    let mpv = Mpv::new()?;
    Ok(mpv.get_property::<String>("mpv-version")?)
}

/// libmpv-backed [`Player`], audio only.
///
/// A background thread blocks in `mpv_wait_event` (no polling) and turns
/// mpv events into [`EngineEvent`]s; take the receiver with
/// [`Player::take_events`]. Dropping the player shuts the core down.
///
/// Use **one `MpvPlayer` per process**: mpv routes FFmpeg's (global) log
/// callback only to the first mpv core created (`common/av_log.c`), and
/// HTTP/network error classification relies on those log lines.
pub struct MpvPlayer {
    mpv: Arc<Mpv>,
    events: Option<EngineEvents>,
    event_thread: Option<JoinHandle<()>>,
}

impl MpvPlayer {
    /// Create an mpv core configured by `cfg` and start its event thread.
    pub fn new(cfg: &EngineConfig) -> Result<Self, EngineError> {
        // `with_initializer` only passes libmpv2 errors through, so remember
        // which option failed for the error message.
        let mut failed = None;
        let init_result = Mpv::with_initializer(|init| {
            let mut set = |name: &'static str, value: &str| {
                init.set_option(name, value)
                    .inspect_err(|_| failed = Some(name))
            };
            for (name, value) in AUDIO_ONLY_OPTIONS {
                set(name, value)?;
            }
            set("ao", cfg.audio_output.mpv_name())?;
            set("audio-channels", &cfg.audio_channels)?;
            set("volume", &clamp_volume(cfg.volume).to_string())?;
            set("audio-exclusive", if cfg.exclusive { "yes" } else { "no" })?;
            set(
                "coreaudio-change-physical-format",
                if cfg.match_sample_rate { "yes" } else { "no" },
            )?;
            for (name, value) in SCRIPT_OPTIONS {
                match init.set_option(name, *value) {
                    Err(libmpv2::Error::Raw(libmpv2::mpv_error::OptionNotFound)) => {}
                    other => other.inspect_err(|_| failed = Some(name))?,
                }
            }
            Ok(())
        });
        let mpv = init_result.map_err(|source| match failed {
            Some(name) => EngineError::Option { name, source },
            None => EngineError::Mpv(source),
        })?;
        request_log_messages(&mpv, c"warn")?;
        for (name, format, id) in OBSERVED {
            mpv.observe_property(name, *format, *id)?;
        }
        for (name, format) in STREAM_INFO_PROPS {
            mpv.observe_property(name, *format, STREAM_INFO_ID)?;
        }

        let mpv = Arc::new(mpv);
        let (tx, rx) = unbounded_channel();
        let thread_mpv = Arc::clone(&mpv);
        let event_thread = std::thread::Builder::new()
            .name("ttnr-mpv-events".into())
            .spawn(move || event_loop(&thread_mpv, &tx))
            .map_err(EngineError::Thread)?;

        Ok(Self {
            mpv,
            events: Some(rx),
            event_thread: Some(event_thread),
        })
    }

    /// Read an mpv property as a string (`None` if unavailable right now).
    pub fn property_string(&self, name: &str) -> Option<String> {
        self.mpv.get_property::<String>(name).ok()
    }

    /// Read an mpv flag property (`None` if unavailable right now).
    pub fn property_flag(&self, name: &str) -> Option<bool> {
        self.mpv.get_property::<bool>(name).ok()
    }

    /// Read an mpv numeric property (`None` if unavailable right now).
    pub fn property_f64(&self, name: &str) -> Option<f64> {
        self.mpv.get_property::<f64>(name).ok()
    }
}

impl Player for MpvPlayer {
    fn play(&mut self, d: &StreamDescriptor) -> Result<(), EngineError> {
        let url = d.urls.first().ok_or(EngineError::NoStreamUrl)?;
        // `-append` takes a single item verbatim, so header values may
        // contain commas (mpv manual, "List Options").
        self.mpv
            .command("change-list", &["http-header-fields", "clr", ""])?;
        for (name, value) in &d.headers {
            let field = format!("{name}: {value}");
            self.mpv
                .command("change-list", &["http-header-fields", "append", &field])?;
        }
        self.mpv.command("loadfile", &[url, "replace"])?;
        self.mpv.set_property("pause", false)?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), EngineError> {
        Ok(self.mpv.command("stop", &[])?)
    }

    fn pause(&mut self, on: bool) -> Result<(), EngineError> {
        Ok(self.mpv.set_property("pause", on)?)
    }

    fn set_volume(&mut self, pct: f32) -> Result<(), EngineError> {
        Ok(self
            .mpv
            .set_property("volume", f64::from(clamp_volume(pct)))?)
    }

    fn set_device(&mut self, dev: Option<&str>) -> Result<(), EngineError> {
        Ok(self
            .mpv
            .set_property("audio-device", dev.unwrap_or("auto"))?)
    }

    fn take_events(&mut self) -> Option<EngineEvents> {
        self.events.take()
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        // `quit` makes mpv emit MPV_EVENT_SHUTDOWN, which ends the event
        // loop. If the command fails the core is already gone and the loop
        // has exited (or will on its own); never block on join then.
        let quit = self.mpv.command("quit", &[]);
        if let Some(handle) = self.event_thread.take()
            && quit.is_ok()
        {
            let _ = handle.join();
        }
    }
}

/// Ask mpv to deliver log messages at `level` and above as events.
/// `libmpv2` 7.0 has no safe wrapper for this (docs/RESEARCH.md).
fn request_log_messages(mpv: &Mpv, level: &std::ffi::CStr) -> Result<(), EngineError> {
    // SAFETY: `mpv.ctx` is a valid handle for the lifetime of `mpv`;
    // `level` is a NUL-terminated string that mpv copies before returning.
    let rc = unsafe { libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), level.as_ptr()) };
    if rc < 0 {
        return Err(EngineError::Mpv(libmpv2::Error::Raw(rc)));
    }
    Ok(())
}

/// Result of one `mpv_wait_event` call.
enum Next {
    Event(RawEvent),
    /// An event we do not translate.
    Skip,
    /// Timed out (or woken up) with no event.
    Timeout,
}

fn next_raw(mpv: &Mpv, timeout: f64) -> Next {
    match mpv.wait_event(timeout) {
        None => Next::Timeout,
        Some(Ok(event)) => to_raw(event).map_or(Next::Skip, Next::Event),
        // libmpv2 reports a failed file (end-file with an error code)
        // as `Err(Raw(code))`, dropping the reason.
        Some(Err(libmpv2::Error::Raw(code))) => {
            Next::Event(RawEvent::EndFile(EndReason::Error(code)))
        }
        Some(Err(e)) => {
            tracing::debug!(error = ?e, "unreadable mpv event");
            Next::Skip
        }
    }
}

/// Block on mpv events until shutdown, forwarding translated events.
///
/// After each event the queue is drained without blocking; once it is
/// empty, accumulated stream-info changes are flushed as one event.
fn event_loop(mpv: &Mpv, tx: &UnboundedSender<EngineEvent>) {
    let mut translator = Translator::default();
    let mut timeout = -1.0;
    loop {
        match next_raw(mpv, timeout) {
            Next::Timeout => {
                for event in [translator.flush(), translator.flush_tags()]
                    .into_iter()
                    .flatten()
                {
                    let _ = tx.send(event);
                }
                timeout = -1.0;
            }
            Next::Skip => timeout = 0.0,
            Next::Event(raw) => {
                if handle(mpv, &mut translator, tx, raw) {
                    return;
                }
                timeout = 0.0;
            }
        }
    }
}

/// Translate and forward one raw event; `true` on shutdown.
fn handle(
    mpv: &Mpv,
    translator: &mut Translator,
    tx: &UnboundedSender<EngineEvent>,
    raw: RawEvent,
) -> bool {
    let mut forward = |raw: RawEvent| -> bool {
        if let RawEvent::Log { prefix, text } = &raw {
            tracing::warn!(target: "mpv", "[{prefix}] {text}");
        }
        let shutdown = raw == RawEvent::Shutdown;
        for event in translator.translate(raw) {
            // A dropped receiver only means nobody listens; keep draining.
            let _ = tx.send(event);
        }
        shutdown
    };
    if raw == RawEvent::MetadataChanged {
        for name in TAG_PROPS {
            forward(RawEvent::Prop {
                name: (*name).to_owned(),
                value: read_prop(mpv, name, Format::String),
            });
        }
        return false;
    }
    if matches!(
        raw,
        RawEvent::FileLoaded | RawEvent::PlaybackRestart | RawEvent::AudioReconfig
    ) {
        if forward(raw) {
            return true;
        }
        for name in TAG_PROPS {
            forward(RawEvent::Prop {
                name: (*name).to_owned(),
                value: read_prop(mpv, name, Format::String),
            });
        }
        // mpv 0.41.0 does not always re-notify observed properties for a
        // new file: after one AAC file, the next one's `audio-params` stay
        // "unavailable" to the observer while readable (docs/RESEARCH.md).
        // Re-read them at these points so the mirror is always current.
        for (name, format) in STREAM_INFO_PROPS {
            forward(RawEvent::Prop {
                name: (*name).to_owned(),
                value: read_prop(mpv, name, *format),
            });
        }
        return false;
    }
    if !matches!(raw, RawEvent::EndFile(EndReason::Error(_))) {
        return forward(raw);
    }
    // mpv delivers queued events before log messages (player/client.c,
    // `mpv_wait_event`), so the FFmpeg line explaining this failure may
    // still be pending. Drain what is ready: logs first, then the error,
    // then the other drained events in their original order.
    let mut later = Vec::new();
    loop {
        match next_raw(mpv, 0.0) {
            Next::Event(log @ RawEvent::Log { .. }) => {
                forward(log);
            }
            Next::Event(other) => later.push(other),
            Next::Skip => {}
            Next::Timeout => break,
        }
    }
    if forward(raw) {
        return true;
    }
    later.into_iter().any(forward)
}

/// Read one stream-info property now (`None` if unavailable).
fn read_prop(mpv: &Mpv, name: &str, format: Format) -> PropValue {
    match format {
        Format::String => mpv.get_property::<String>(name).map(PropValue::Str),
        Format::Int64 => mpv.get_property::<i64>(name).map(PropValue::Int),
        Format::Double => mpv.get_property::<f64>(name).map(PropValue::Double),
        Format::Flag => mpv
            .get_property::<bool>(name)
            .map(|b| PropValue::Int(i64::from(b))),
        _ => return PropValue::None,
    }
    .unwrap_or(PropValue::None)
}

/// Map a libmpv2 event to our owned [`RawEvent`]; `None` = not interesting.
fn to_raw(event: Event<'_>) -> Option<RawEvent> {
    Some(match event {
        Event::Shutdown => RawEvent::Shutdown,
        Event::StartFile => RawEvent::StartFile,
        Event::FileLoaded => RawEvent::FileLoaded,
        Event::PlaybackRestart => RawEvent::PlaybackRestart,
        Event::AudioReconfig => RawEvent::AudioReconfig,
        Event::EndFile(reason) => RawEvent::EndFile(match reason {
            mpv_end_file_reason::Eof => EndReason::Eof,
            mpv_end_file_reason::Redirect => EndReason::Redirect,
            // Stop, Quit; Error arrives as `Err(Raw(code))` instead.
            _ => EndReason::Stop,
        }),
        Event::LogMessage { prefix, text, .. } => RawEvent::Log {
            prefix: prefix.to_owned(),
            text: text.trim_end().to_owned(),
        },
        Event::PropertyChange {
            name,
            change,
            reply_userdata: STREAM_INFO_ID,
        } => RawEvent::Prop {
            name: name.to_owned(),
            value: match change {
                PropertyData::Str(s) | PropertyData::OsdStr(s) => PropValue::Str(s.to_owned()),
                PropertyData::Int64(i) => PropValue::Int(i),
                PropertyData::Double(d) => PropValue::Double(d),
                PropertyData::Flag(b) => PropValue::Int(i64::from(b)),
                PropertyData::None => PropValue::None,
            },
        },
        Event::PropertyChange { name, change, .. } => match (name, change) {
            ("pause", PropertyData::Flag(on)) => RawEvent::Pause(on),
            ("paused-for-cache", PropertyData::Flag(on)) => RawEvent::PausedForCache(on),
            ("metadata", _) => RawEvent::MetadataChanged,
            ("audio-device-list", PropertyData::Str(json)) => RawEvent::DeviceList(json.to_owned()),
            ("cache-buffering-state", PropertyData::Int64(p)) => RawEvent::CacheBufferingState(p),
            _ => return None,
        },
        _ => return None,
    })
}

/// Clamp to `0..=100`; NaN becomes 0 (silence is the safe failure).
fn clamp_volume(pct: f32) -> f32 {
    if pct.is_nan() {
        0.0
    } else {
        pct.clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_volume_bounds() {
        assert_eq!(clamp_volume(-5.0), 0.0);
        assert_eq!(clamp_volume(42.5), 42.5);
        assert_eq!(clamp_volume(250.0), 100.0);
        assert_eq!(clamp_volume(f32::NAN), 0.0);
    }

    #[test]
    fn libmpv_links_and_reports_version() {
        let v = libmpv_version().unwrap();
        assert!(v.starts_with("mpv "), "unexpected mpv-version: {v}");
    }
}
