//! [`MpvPlayer`]: the libmpv implementation of [`Player`].

use std::sync::Arc;

use libmpv2::Mpv;
use ttnr_core::StreamDescriptor;

use crate::{EngineConfig, EngineError, Player};

/// Options set before `mpv_initialize`, making the core audio-only and
/// isolated from the user's mpv setup. Names and values verified against
/// the mpv 0.41.0 manual (docs/RESEARCH.md).
const AUDIO_ONLY_OPTIONS: &[(&str, &str)] = &[
    ("vid", "no"),
    ("audio-display", "no"),
    ("force-window", "no"),
    ("ytdl", "no"),
    ("config", "no"),
    ("load-scripts", "no"),
    ("terminal", "no"),
    ("osc", "no"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("input-media-keys", "no"),
    ("idle", "yes"),
    ("audio-client-name", "ttnr"),
];

/// Version string of the linked libmpv, e.g. `"mpv 0.41.0"`.
///
/// Creates and drops a throw-away mpv core (no file loaded, no audio
/// output opened).
pub fn libmpv_version() -> Result<String, EngineError> {
    let mpv = Mpv::new()?;
    Ok(mpv.get_property::<String>("mpv-version")?)
}

/// libmpv-backed [`Player`], audio only.
pub struct MpvPlayer {
    mpv: Arc<Mpv>,
}

impl MpvPlayer {
    /// Create an mpv core configured by `cfg`.
    pub fn new(cfg: &EngineConfig) -> Result<Self, EngineError> {
        let mpv = Mpv::with_initializer(|init| {
            for (name, value) in AUDIO_ONLY_OPTIONS {
                init.set_option(name, *value)?;
            }
            init.set_option("ao", cfg.audio_output.mpv_name())?;
            init.set_option("audio-channels", cfg.audio_channels.as_str())?;
            init.set_option("volume", f64::from(clamp_volume(cfg.volume)))?;
            Ok(())
        })?;
        Ok(Self { mpv: Arc::new(mpv) })
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
