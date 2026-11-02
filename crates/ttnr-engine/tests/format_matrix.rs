//! Format matrix (SPEC F2, ROADMAP M1): every fixture from
//! `scripts/gen-fixtures.zsh` plays through `MpvPlayer` (`ao=null`) and
//! reports the expected `StreamInfo`. Generates fixtures on first run
//! (needs Homebrew ffmpeg); no network.
//!
//! DASH is absent on purpose: Homebrew FFmpeg 9.0.2 has no `dash` demuxer
//! (built without libxml2) and mpv has none of its own (docs/RESEARCH.md).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use common::null_player;
use ttnr_core::{StreamDescriptor, StreamInfo};
use ttnr_engine::{EngineEvent, Player};

struct Case {
    file: &'static str,
    codec: &'static str,
    profile: Option<&'static str>,
    rate: u32,
    channels: &'static str,
    count: u32,
    container: &'static str,
    bit_depth: Option<u8>,
}

const fn case(
    file: &'static str,
    codec: &'static str,
    rate: u32,
    channels: &'static str,
    count: u32,
    container: &'static str,
) -> Case {
    Case {
        file,
        codec,
        profile: None,
        rate,
        channels,
        count,
        container,
        bit_depth: None,
    }
}

const MP4: &str = "mov,mp4,m4a,3gp,3g2,mj2";

fn cases() -> Vec<Case> {
    vec![
        case("mp3.mp3", "mp3", 44_100, "stereo", 2, "mp3"),
        case("mono.mp3", "mp3", 22_050, "mono", 1, "mp3"),
        // Keep aac_lc → aac_adts → he_aac → he_aac_v2 adjacent: AAC after
        // AAC is the regression case for mpv not re-notifying properties
        // across files (ADR-013).
        Case {
            profile: Some("LC"),
            ..case("aac_lc.m4a", "aac", 48_000, "stereo", 2, MP4)
        },
        Case {
            profile: Some("LC"),
            ..case("aac_adts.aac", "aac", 44_100, "stereo", 2, "aac")
        },
        Case {
            profile: Some("HE-AAC"),
            ..case("he_aac.m4a", "aac", 48_000, "stereo", 2, MP4)
        },
        Case {
            profile: Some("HE-AACv2"),
            ..case("he_aac_v2.m4a", "aac", 48_000, "stereo", 2, MP4)
        },
        case("opus.opus", "opus", 48_000, "stereo", 2, "ogg"),
        case("vorbis.ogg", "vorbis", 44_100, "stereo", 2, "ogg"),
        Case {
            bit_depth: Some(16),
            ..case("flac_16_44.flac", "flac", 44_100, "stereo", 2, "flac")
        },
        // 24-bit decodes to s32: bit depth unknown by design (ADR-011).
        case("flac_24_96.flac", "flac", 96_000, "stereo", 2, "flac"),
        case("alac_24.m4a", "alac", 48_000, "stereo", 2, MP4),
        case("pcm_s24.wav", "pcm_s24le", 48_000, "stereo", 2, "wav"),
        case("ac3_51.ac3", "ac3", 48_000, "5.1(side)", 6, "ac3"),
        case("eac3_51.eac3", "eac3", 48_000, "5.1(side)", 6, "eac3"),
        Case {
            profile: Some("LC"),
            ..case("aac_71.m4a", "aac", 48_000, "7.1", 8, MP4)
        },
        case("opus_71.opus", "opus", 48_000, "7.1", 8, "ogg"),
        Case {
            bit_depth: Some(16),
            ..case(
                "channel_check_51.flac",
                "flac",
                48_000,
                "5.1(side)",
                6,
                "flac",
            )
        },
        Case {
            bit_depth: Some(16),
            ..case("channel_check_71.flac", "flac", 48_000, "7.1", 8, "flac")
        },
        Case {
            profile: Some("LC"),
            ..case("hls/index.m3u8", "aac", 48_000, "stereo", 2, "hls")
        },
    ]
}

fn fixtures_dir() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("zsh")
        .arg(root.join("scripts/gen-fixtures.zsh"))
        .status()
        .expect("cannot run scripts/gen-fixtures.zsh (zsh missing?)");
    assert!(status.success(), "scripts/gen-fixtures.zsh failed");
    root.join("tests/fixtures/generated")
}

type Facts<'a> = (
    Option<&'a str>,
    Option<&'a str>,
    Option<u32>,
    Option<&'a str>,
    Option<u32>,
    Option<&'a str>,
    Option<u8>,
);

fn facts(info: &StreamInfo) -> Facts<'_> {
    (
        info.codec.as_deref(),
        info.codec_profile.as_deref(),
        info.sample_rate,
        info.channels.as_deref(),
        info.channel_count,
        info.container.as_deref(),
        info.bit_depth,
    )
}

fn expected(c: &Case) -> Facts<'static> {
    (
        Some(c.codec),
        c.profile,
        Some(c.rate),
        Some(c.channels),
        Some(c.count),
        Some(c.container),
        c.bit_depth,
    )
}

/// Play `path` until its latest `StreamInfo` matches `want`, or until EOF
/// or 10 s; returns the latest info. (Facts arrive in several events: e.g.
/// the codec profile only once the first packets are decoded.)
fn probe(
    p: &mut ttnr_engine::MpvPlayer,
    rx: &mut ttnr_engine::EngineEvents,
    path: &str,
    want: &Facts<'_>,
) -> StreamInfo {
    p.play(&StreamDescriptor::direct(path)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut started = false;
    let mut latest = StreamInfo::default();
    loop {
        match rx.try_recv() {
            Ok(EngineEvent::Loading) => started = true,
            Ok(EngineEvent::StreamInfo(info)) if started => {
                latest = info;
                if facts(&latest) == *want {
                    return latest;
                }
            }
            Ok(EngineEvent::Eof) if started => return latest,
            Ok(EngineEvent::Error { kind, detail }) if started => {
                panic!("{path}: {kind} {detail:?}")
            }
            Ok(_) => {}
            Err(_) => {
                if Instant::now() > deadline {
                    return latest;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[test]
fn format_matrix() {
    let dir = fixtures_dir();
    // One core for the whole matrix (one MpvPlayer per process, ADR-010).
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    let mut failures = Vec::new();
    for c in cases() {
        let path = dir.join(c.file);
        let want = expected(&c);
        let info = probe(&mut p, &mut rx, path.to_str().unwrap(), &want);
        let got = facts(&info);
        if got != want {
            failures.push(format!("{}:\n  got  {got:?}\n  want {want:?}", c.file));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
