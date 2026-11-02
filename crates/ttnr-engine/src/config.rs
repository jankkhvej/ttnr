//! Engine configuration (the subset of mpv options ttnr exposes).

/// Which mpv audio output driver to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioOutput {
    /// macOS CoreAudio (`ao=coreaudio`).
    #[default]
    CoreAudio,
    /// Discard audio (`ao=null`); for tests and headless probing.
    Null,
}

impl AudioOutput {
    /// mpv's name for this driver.
    pub fn mpv_name(self) -> &'static str {
        match self {
            Self::CoreAudio => "coreaudio",
            Self::Null => "null",
        }
    }
}

/// Settings applied when the mpv core is created.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    /// Audio output driver.
    pub audio_output: AudioOutput,
    /// mpv `audio-channels` value: `auto-safe` (system's preferred layout,
    /// mpv default), `auto`, or a layout list such as `stereo,5.1`.
    pub audio_channels: String,
    /// Initial app volume in percent (`0.0..=100.0`).
    pub volume: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            audio_output: AudioOutput::default(),
            audio_channels: "auto-safe".to_owned(),
            volume: 100.0,
        }
    }
}
