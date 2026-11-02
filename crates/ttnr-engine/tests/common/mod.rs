//! Shared helpers for engine integration tests.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ttnr_engine::{AudioOutput, EngineConfig, MpvPlayer};

const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

/// Write `secs` seconds of a 440 Hz 16-bit PCM sine as a WAV file.
pub fn write_wav(path: &Path, secs: u32) {
    let frames = RATE * secs;
    let data_len = frames * u32::from(CHANNELS) * 2;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&CHANNELS.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * u32::from(CHANNELS) * 2).to_le_bytes());
    b.extend_from_slice(&(CHANNELS * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let t = i as f32 / RATE as f32;
        let s = ((t * 440.0 * std::f32::consts::TAU).sin() * 8000.0) as i16;
        for _ in 0..CHANNELS {
            b.extend_from_slice(&s.to_le_bytes());
        }
    }
    std::fs::write(path, b).unwrap();
}

/// A temp dir holding `sine.wav` of `secs` seconds.
pub fn fixture_secs(secs: u32) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sine.wav");
    write_wav(&path, secs);
    (dir, path)
}

pub fn null_player() -> MpvPlayer {
    MpvPlayer::new(&EngineConfig {
        audio_output: AudioOutput::Null,
        ..EngineConfig::default()
    })
    .unwrap()
}

/// Poll `cond` until it holds or 5 s pass.
pub fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A 5-second fixture.
pub fn fixture() -> (tempfile::TempDir, PathBuf) {
    fixture_secs(5)
}

/// Serialise tests that need FFmpeg log lines: mpv delivers them only to
/// the first mpv core in the process (`common/av_log.c`).
pub fn single_core() -> MutexGuard<'static, ()> {
    static CORE: Mutex<()> = Mutex::new(());
    CORE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
