//! Playback engine for tuner: the `Player` trait and its libmpv-backed
//! implementation, configured for audio only (video is never decoded).
//!
//! M0 only verifies that libmpv links and initialises; playback arrives in
//! M1 (see `docs/ROADMAP.md`).

use libmpv2::Mpv;

/// Errors raised by the playback engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// libmpv refused to create or initialise a player instance.
    #[error("libmpv: {0}")]
    Mpv(#[from] libmpv2::Error),
}

/// Version string of the linked libmpv, e.g. `"mpv 0.41.0"`.
///
/// Creates and drops a throw-away mpv core (no file loaded, no audio
/// output opened).
pub fn libmpv_version() -> Result<String, EngineError> {
    let mpv = Mpv::new()?;
    Ok(mpv.get_property::<String>("mpv-version")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libmpv_links_and_reports_version() {
        let v = libmpv_version().unwrap();
        assert!(v.starts_with("mpv "), "unexpected mpv-version: {v}");
    }
}
