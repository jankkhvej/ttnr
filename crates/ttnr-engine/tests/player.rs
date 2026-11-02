//! `MpvPlayer` against a generated local WAV file with `ao=null`
//! (no network, no audible output).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ttnr_core::StreamDescriptor;
use ttnr_engine::{AudioOutput, EngineConfig, EngineError, MpvPlayer, Player};

const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

/// Write `secs` seconds of a 440 Hz 16-bit PCM sine as a WAV file.
fn write_wav(path: &Path, secs: u32) {
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

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sine.wav");
    write_wav(&path, 5);
    (dir, path)
}

fn null_player() -> MpvPlayer {
    MpvPlayer::new(&EngineConfig {
        audio_output: AudioOutput::Null,
        ..EngineConfig::default()
    })
    .unwrap()
}

/// Poll `cond` until it holds or 5 s pass.
fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn start(p: &mut MpvPlayer, path: &Path) {
    p.play(&StreamDescriptor::direct(path.to_str().unwrap()))
        .unwrap();
    wait_for("audio decoding", || {
        p.property_string("audio-codec-name").is_some()
    });
}

#[test]
fn plays_local_file_audio_only() {
    let (_dir, path) = fixture();
    let mut p = null_player();
    start(&mut p, &path);
    assert_eq!(
        p.property_string("audio-codec-name").as_deref(),
        Some("pcm_s16le")
    );
    assert_eq!(p.property_string("current-ao").as_deref(), Some("null"));
    assert_eq!(p.property_string("vid").as_deref(), Some("no"));
    assert_eq!(p.property_string("current-vo"), None, "no video output");
    assert_eq!(p.property_flag("pause"), Some(false));
}

#[test]
fn pause_and_resume() {
    let (_dir, path) = fixture();
    let mut p = null_player();
    start(&mut p, &path);
    p.pause(true).unwrap();
    assert_eq!(p.property_flag("pause"), Some(true));
    p.pause(false).unwrap();
    assert_eq!(p.property_flag("pause"), Some(false));
}

#[test]
fn volume_is_clamped_software_volume() {
    let mut p = null_player();
    assert_eq!(p.property_f64("volume"), Some(100.0));
    p.set_volume(30.0).unwrap();
    assert_eq!(p.property_f64("volume"), Some(30.0));
    p.set_volume(150.0).unwrap();
    assert_eq!(p.property_f64("volume"), Some(100.0));
    p.set_volume(-1.0).unwrap();
    assert_eq!(p.property_f64("volume"), Some(0.0));
}

#[test]
fn initial_volume_from_config() {
    let p = MpvPlayer::new(&EngineConfig {
        audio_output: AudioOutput::Null,
        volume: 40.0,
        ..EngineConfig::default()
    })
    .unwrap();
    assert_eq!(p.property_f64("volume"), Some(40.0));
}

#[test]
fn stop_returns_to_idle() {
    let (_dir, path) = fixture();
    let mut p = null_player();
    start(&mut p, &path);
    assert_eq!(p.property_flag("idle-active"), Some(false));
    p.stop().unwrap();
    wait_for("idle", || p.property_flag("idle-active") == Some(true));
}

#[test]
fn empty_descriptor_is_an_error() {
    let mut p = null_player();
    let err = p.play(&StreamDescriptor::default()).unwrap_err();
    assert!(matches!(err, EngineError::NoStreamUrl));
}

#[test]
fn headers_are_passed_verbatim_and_replaced_per_play() {
    let (_dir, path) = fixture();
    let mut p = null_player();
    let mut d = StreamDescriptor::direct(path.to_str().unwrap());
    d.headers = vec![("X-Test".into(), "a, b".into())];
    p.play(&d).unwrap();
    let fields = p.property_string("http-header-fields").unwrap();
    assert!(fields.contains("X-Test: a"), "got {fields:?}");

    p.play(&StreamDescriptor::direct(path.to_str().unwrap()))
        .unwrap();
    assert_eq!(p.property_string("http-header-fields").as_deref(), Some(""));
}

#[test]
fn device_can_be_pinned_and_released() {
    let mut p = null_player();
    p.set_device(Some("null/test")).unwrap();
    assert_eq!(
        p.property_string("audio-device").as_deref(),
        Some("null/test")
    );
    p.set_device(None).unwrap();
    assert_eq!(p.property_string("audio-device").as_deref(), Some("auto"));
}

/// Opens the real CoreAudio output (volume 0, so silent). Ignored by
/// default: needs an audio device. Run: `cargo test -- --ignored coreaudio`.
#[test]
#[ignore = "needs a CoreAudio output device"]
fn coreaudio_output_opens() {
    let (_dir, path) = fixture();
    let mut p = MpvPlayer::new(&EngineConfig {
        volume: 0.0,
        ..EngineConfig::default()
    })
    .unwrap();
    start(&mut p, &path);
    wait_for("coreaudio ao", || {
        p.property_string("current-ao").as_deref() == Some("coreaudio")
    });
}
