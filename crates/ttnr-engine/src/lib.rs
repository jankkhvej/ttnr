//! Playback engine for ttnr: the [`Player`] trait and its libmpv-backed
//! implementation [`MpvPlayer`], configured for audio only (video is never
//! decoded).

mod config;
mod event;
mod mpv;

pub use config::{AudioOutput, EngineConfig};
pub use event::{EngineEvent, EngineEvents, ErrorKind};
pub use mpv::{MpvPlayer, libmpv_version};
use ttnr_core::StreamDescriptor;

/// Errors raised by the playback engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// libmpv rejected a call (init, option, property or command).
    #[error("libmpv: {0}")]
    Mpv(#[from] libmpv2::Error),
    /// The stream descriptor contained no URL to play.
    #[error("stream descriptor has no URL")]
    NoStreamUrl,
    /// The engine's event thread could not be started.
    #[error("cannot start engine thread: {0}")]
    Thread(#[source] std::io::Error),
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
