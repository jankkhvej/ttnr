//! Views render at any size without panicking and show the essentials at
//! the smallest supported size (32×16). Snapshots per theme and size follow
//! in M3 task 5.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::SystemTime;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ttnr_core::app::{AppState, Device, Status, View};
use ttnr_core::{InfoSource, NowPlaying, Station, StreamInfo};
use ttnr_themes::Theme;

fn np(a: &str, t: &str) -> NowPlaying {
    NowPlaying {
        artist: Some(a.into()),
        title: Some(t.into()),
        ..NowPlaying::empty(InfoSource::Icy, SystemTime::UNIX_EPOCH)
    }
}

fn station(name: &str, url: &str) -> Station {
    Station {
        name: name.into(),
        ..Station::from_url(url)
    }
}

fn state() -> AppState {
    AppState {
        station: Some(station(
            "SomaFM Groove Salad",
            "https://somafm.com/groovesalad.pls",
        )),
        status: Status::Playing,
        now: Some(NowPlaying {
            programme: Some("Morning Ambient".into()),
            ..np("State Azure", "Converging")
        }),
        history: [np("Kick Bong", "Vaporum")].into(),
        stream: Some(StreamInfo {
            codec: Some("mp3".into()),
            sample_rate: Some(44_100),
            channels: Some("stereo".into()),
            bitrate: Some(128_000),
            ..StreamInfo::default()
        }),
        volume: 80,
        devices: vec![Device {
            name: "coreaudio/x".into(),
            description: "MAJOR IV".into(),
        }],
        stations: vec![
            station("SomaFM Groove Salad", "https://somafm.com/groovesalad.pls"),
            station("FIP", "https://fip"),
        ],
        ..AppState::default()
    }
}

fn views() -> Vec<View> {
    vec![
        View::Main,
        View::Stations { selected: 1 },
        View::OpenUrl {
            input: "https://example.org/stream.m3u8".into(),
        },
        View::Help,
        View::Devices { selected: 1 },
    ]
}

fn render(s: &AppState, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| ttnr_tui::draw(f, s, &Theme::plain())).unwrap();
    t.backend().to_string()
}

#[test]
fn any_size_renders() {
    let mut s = state();
    for view in views() {
        s.view = view;
        for w in [1, 2, 5, 10, 31, 32, 40, 80, 120] {
            for h in [1, 2, 3, 5, 15, 16, 25, 40] {
                render(&s, w, h);
            }
        }
    }
}

#[test]
fn essentials_at_32x16() {
    let out = render(&state(), 32, 16);
    for needle in [
        "SomaFM Groove Salad",
        "State Azure – Converging",
        "▶ playing",
        "vol 80% · system default",
        "MP3 · 44.1 kHz · stereo · 128…",
        "? help · q quit",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
}

#[test]
fn footer_grows_with_width() {
    let narrow = render(&state(), 32, 16);
    let wide = render(&state(), 120, 30);
    assert!(narrow.contains("spc play · ? help · q quit"), "{narrow}");
    assert!(!narrow.contains("+- vol"));
    assert!(wide.contains("spc play · +- vol · m mute"), "{wide}");
    assert!(wide.contains("? help · q quit"));
}

#[test]
fn low_height_keeps_station_track_status() {
    let out = render(&state(), 40, 7);
    for needle in [
        "SomaFM Groove Salad",
        "State Azure – Converging",
        "▶ playing",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
    assert!(!out.contains("Kick Bong"), "history goes first\n{out}");
}

#[test]
fn errors_and_empty_station_list() {
    let mut s = state();
    s.status = Status::Error("HTTP 404 Not Found".into());
    assert!(render(&s, 40, 16).contains("✕ error: HTTP 404 Not Found"));
    s.stations.clear();
    s.view = View::Stations { selected: 0 };
    assert!(render(&s, 60, 16).contains("no stations – see examples/stations.toml"));
}
