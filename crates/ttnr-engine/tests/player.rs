//! `MpvPlayer` against a generated local WAV file with `ao=null`
//! (no network, no audible output).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use common::{fixture, null_player, wait_for};
use ttnr_core::StreamDescriptor;
use ttnr_engine::{AudioOutput, EngineConfig, EngineError, MpvPlayer, Player};

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
    // The AO opens shortly after the decoder; wait rather than race it.
    wait_for("null ao", || {
        p.property_string("current-ao").as_deref() == Some("null")
    });
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

/// A 44.1 kHz MP3 (float planar) through the real output: must open
/// CoreAudio or the AVFoundation fallback, never fail. Ignored by default:
/// needs an audio device and generated fixtures.
#[test]
#[ignore = "needs an audio device and scripts/gen-fixtures.zsh output"]
fn real_output_opens_for_44k_mp3() {
    let mp3 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated/mp3.mp3");
    let mut p = MpvPlayer::new(&EngineConfig {
        volume: 0.0,
        ..EngineConfig::default()
    })
    .unwrap();
    start(&mut p, &mp3);
    wait_for("an audio output", || {
        matches!(
            p.property_string("current-ao").as_deref(),
            Some("coreaudio" | "avfoundation")
        )
    });
}
