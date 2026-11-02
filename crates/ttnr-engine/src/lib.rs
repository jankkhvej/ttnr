//! Playback engine for ttnr: the [`Player`] trait and its libmpv-backed
//! implementation [`MpvPlayer`], configured for audio only (video is never
//! decoded).

mod config;
mod event;
mod mpv;

pub use config::{AudioOutput, EngineConfig};
pub use event::{AudioDevice, EngineEvent, EngineEvents, ErrorKind};
pub use mpv::{MpvPlayer, libmpv_version};
use ttnr_core::StreamDescriptor;

/// Errors raised by the playback engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// libmpv rejected a call (init, property or command).
    #[error("libmpv: {}", mpv_error_text(.0))]
    Mpv(#[from] libmpv2::Error),
    /// libmpv rejected an option while creating the player.
    #[error("libmpv option `{name}`: {}", mpv_error_text(.source))]
    Option {
        /// The mpv option name.
        name: &'static str,
        /// The libmpv error.
        source: libmpv2::Error,
    },
    /// The stream descriptor contained no URL to play.
    #[error("stream descriptor has no URL")]
    NoStreamUrl,
    /// The engine's event thread could not be started.
    #[error("cannot start engine thread: {0}")]
    Thread(#[source] std::io::Error),
}

/// Human-readable text for a libmpv2 error (`mpv_error_string` for codes).
fn mpv_error_text(e: &libmpv2::Error) -> String {
    match e {
        libmpv2::Error::Raw(code) => {
            // SAFETY: mpv_error_string accepts any int and returns a pointer
            // to a static NUL-terminated string (client.h).
            let text = unsafe { std::ffi::CStr::from_ptr(libmpv2_sys::mpv_error_string(*code)) };
            format!("{} ({code})", text.to_string_lossy())
        }
        other => format!("{other:?}"),
    }
}

/// A playback backend. Implemented by [`MpvPlayer`].
pub trait Player {
    /// Start playing the first URL of `d`, replacing whatever is playing.
    fn play(&mut self, d: &StreamDescriptor) -> Result<(), EngineError>;
    /// Stop playback; the player stays ready for the next [`Player::play`].
    fn stop(&mut self) -> Result<(), EngineError>;
    /// Pause (`true`) or resume (`false`).
    fn pause(&mut self, on: bool) -> Result<(), EngineError>;
    /// App volume in percent, clamped to `0.0..=100.0`. Independent of the
    /// system volume (SPEC F3.6).
    fn set_volume(&mut self, pct: f32) -> Result<(), EngineError>;
    /// Pin an output device by its mpv name (`audio-device-list`), or follow
    /// the system default with `None`.
    fn set_device(&mut self, dev: Option<&str>) -> Result<(), EngineError>;
    /// Take the event receiver. Returns `Some` once; later calls `None`.
    fn take_events(&mut self) -> Option<EngineEvents>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_well() {
        let e = EngineError::Option {
            name: "load-console",
            source: libmpv2::Error::Raw(libmpv2::mpv_error::OptionNotFound),
        };
        assert_eq!(
            e.to_string(),
            "libmpv option `load-console`: option not found (-5)"
        );
    }
}
