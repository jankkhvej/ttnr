//! OS integration for ttnr: Now Playing / Control Center and media keys
//! (ADR-036), the default output device and its changes (ADR-037);
//! sleep/wake follows in a later M6 task.
//!
//! [`run_app`] runs the application. On macOS with media controls enabled,
//! the main thread hosts the AppKit run loop (MediaPlayer delivers remote
//! commands there) and the application runs on a worker thread with a
//! [`MediaLink`]; elsewhere the application simply runs on the calling
//! thread without one.

use std::sync::mpsc;

#[cfg(target_os = "macos")]
mod devices_macos;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use devices_macos::{DeviceWatch, default_output_device, watch as watch_output_devices};

/// No device watching on this OS.
#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct DeviceWatch;

/// The system default output device's name (not available on this OS).
#[cfg(not(target_os = "macos"))]
pub fn default_output_device() -> Option<String> {
    None
}

/// Device change notifications (not available on this OS).
#[cfg(not(target_os = "macos"))]
pub fn watch_output_devices(_on_change: impl Fn() + Send + Sync + 'static) -> Option<DeviceWatch> {
    None
}

/// What the system's Now Playing shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlayingInfo {
    /// Main line (track title, or the station when no track is known).
    pub title: String,
    /// Second line (artist, or the programme).
    pub artist: Option<String>,
    /// Third line (the station).
    pub album: Option<String>,
    /// Playing (true) or paused.
    pub playing: bool,
    /// Next/previous are available (greyed out in Control Center if not).
    pub can_skip: bool,
}

/// A command from media keys, headphones or Control Center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCommand {
    /// Play/pause toggle (F8, headphone button).
    TogglePause,
    /// Play.
    Play,
    /// Pause.
    Pause,
    /// Next track (here: next station).
    Next,
    /// Previous track (here: previous station).
    Previous,
}

/// The application's side of the system integration.
#[derive(Debug)]
pub struct MediaLink {
    updates: mpsc::Sender<Option<NowPlayingInfo>>,
    /// Commands from the system; blocking `recv` ends when the host stops.
    pub commands: mpsc::Receiver<RemoteCommand>,
}

impl MediaLink {
    /// A link not connected to any system (tests): returns the link, the
    /// receiver of its updates and the sender of commands.
    pub fn detached() -> (
        Self,
        mpsc::Receiver<Option<NowPlayingInfo>>,
        mpsc::Sender<RemoteCommand>,
    ) {
        let (updates, updates_rx) = mpsc::channel();
        let (commands_tx, commands) = mpsc::channel();
        (Self { updates, commands }, updates_rx, commands_tx)
    }

    /// A sender for Now Playing updates (`None` = nothing playing).
    pub fn updater(&self) -> Updater {
        Updater(self.updates.clone())
    }
}

/// Sends Now Playing updates to the system (cloneable, `Send`).
#[derive(Debug, Clone)]
pub struct Updater(mpsc::Sender<Option<NowPlayingInfo>>);

impl Updater {
    /// Publish `info` (`None` clears Now Playing). Never fails: if the host
    /// is gone the update is dropped.
    pub fn update(&self, info: Option<NowPlayingInfo>) {
        let _ = self.0.send(info);
    }
}

/// Run `app`. With `media_controls` on macOS (and when called on the main
/// thread), `app` runs on a worker thread and receives a [`MediaLink`]
/// while the main thread hosts the system integration until `app`
/// returns; otherwise `app` runs here with `None`. A panic in `app` is
/// propagated.
pub fn run_app<R, F>(media_controls: bool, app: F) -> R
where
    F: FnOnce(Option<MediaLink>) -> R + Send + 'static,
    R: Send + 'static,
{
    #[cfg(target_os = "macos")]
    if media_controls {
        return macos::host(app);
    }
    let _ = media_controls;
    app(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_link_carries_updates_and_commands() {
        let (link, updates, commands) = MediaLink::detached();
        let info = NowPlayingInfo {
            title: "Someday".into(),
            artist: Some("The Strokes".into()),
            album: Some("BBC Radio 6 Music".into()),
            playing: true,
            can_skip: true,
        };
        link.updater().update(Some(info.clone()));
        assert_eq!(updates.recv().unwrap(), Some(info));
        commands.send(RemoteCommand::Next).unwrap();
        assert_eq!(link.commands.recv().unwrap(), RemoteCommand::Next);
    }

    #[test]
    fn without_media_controls_the_app_runs_here() {
        let here = std::thread::current().id();
        let (same_thread, had_link) = run_app(false, move |link| {
            (std::thread::current().id() == here, link.is_some())
        });
        assert!(same_thread);
        assert!(!had_link);
    }
}
