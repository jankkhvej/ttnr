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
use ttnr_tui::Keymap;

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
    t.draw(|f| ttnr_tui::draw(f, s, &Theme::plain(), &Keymap::default()))
        .unwrap();
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
    assert!(!narrow.contains("+/- vol"));
    assert!(wide.contains("spc play · +/- vol · m mute"), "{wide}");
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

/// Snapshots carry no colours, so check styles directly: the station name
/// uses the accent colour, downgraded per colour depth.
#[test]
fn palette_follows_colour_depth() {
    use ratatui::style::Color;
    use ttnr_themes::ColorDepth;
    let accent_of = |depth| {
        let theme = Theme::plain().for_depth(depth);
        let mut t = Terminal::new(TestBackend::new(40, 16)).unwrap();
        t.draw(|f| ttnr_tui::draw(f, &state(), &theme, &Keymap::default()))
            .unwrap();
        // Row 1 (inside the frame), first letter of the station name.
        let buf = t.backend().buffer();
        let cell = (0..40)
            .map(|x| &buf[(x, 1)])
            .find(|c| c.symbol() == "S")
            .unwrap();
        cell.fg
    };
    assert_eq!(accent_of(ColorDepth::Ansi16), Color::Cyan);
    assert_eq!(accent_of(ColorDepth::TrueColor), Color::Cyan);
    assert_eq!(accent_of(ColorDepth::Mono), Color::Reset);
}

/// `plain` draws secondary text faint in the terminal's own colour, so it
/// fits any colour scheme (a palette colour like bright_black does not).
#[test]
fn plain_dim_is_faint_default_colour() {
    use ratatui::style::{Color, Modifier};
    let theme = Theme::plain().for_depth(ttnr_themes::ColorDepth::TrueColor);
    let mut t = Terminal::new(TestBackend::new(60, 16)).unwrap();
    t.draw(|f| ttnr_tui::draw(f, &state(), &theme, &Keymap::default()))
        .unwrap();
    let buf = t.backend().buffer();
    // The footer's first letter ("spc play …") on the second-to-last row.
    let cell = (0..60)
        .map(|x| &buf[(x, 14)])
        .find(|c| c.symbol() == "s")
        .unwrap();
    assert_eq!(cell.fg, Color::Reset);
    assert!(cell.modifier.contains(Modifier::DIM));
}

#[test]
fn sync_offset_is_shown_when_set() {
    let mut s = state();
    assert!(!render(&s, 60, 16).contains("sync"));
    s.sync_offset = std::time::Duration::from_secs(25);
    assert!(render(&s, 60, 16).contains("vol 80% · system default · sync 25s"));
}

/// The ZX80/BBC title bar is coloured where colour exists and falls back to
/// inverse video in mono, so it never disappears.
#[test]
fn title_bar_survives_mono() {
    use ratatui::style::{Color, Modifier};
    use ttnr_themes::{ColorDepth, ThemeSet};
    let set = ThemeSet::builtin();
    let zx80 = set.get(set.position("zx80").unwrap());
    let bar_of = |depth| {
        let theme = zx80.for_depth(depth);
        let mut t = Terminal::new(TestBackend::new(40, 25)).unwrap();
        t.draw(|f| ttnr_tui::draw(f, &state(), &theme, &Keymap::default()))
            .unwrap();
        let buf = t.backend().buffer();
        let cell = (0..25)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .map(|p| buf[p].clone())
            .find(|c| c.symbol() == "S")
            .unwrap();
        (cell.bg, cell.modifier.contains(Modifier::REVERSED))
    };
    assert_eq!(bar_of(ColorDepth::Ansi16), (Color::Black, false));
    assert_eq!(bar_of(ColorDepth::Mono), (Color::Reset, true));
}

/// Animation frames cover the UI completely: nothing of the view below
/// (e.g. the station list's reverse-video highlight) shows through.
#[test]
fn boot_frames_hide_the_ui_below() {
    use ratatui::style::Modifier;
    use std::time::Duration;
    use ttnr_themes::{BootEffect, ColorDepth, ThemeSet};
    use ttnr_tui::{Extras, Fx, FxFrame};
    let set = ThemeSet::builtin();
    let fx = Fx::new(true, ColorDepth::TrueColor);
    let mut s = state();
    s.view = View::Stations { selected: 0 };
    for i in 0..set.len() {
        let theme = set.get(i).for_depth(ColorDepth::TrueColor);
        let effect = theme.effects.boot;
        if effect == BootEffect::None {
            continue;
        }
        let f = FxFrame::Boot {
            effect,
            t: Duration::from_millis(600),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|frame| {
            let extras = Extras {
                printout: &[],
                fx: Some((&fx, f)),
            };
            ttnr_tui::draw_with(frame, &s, &theme, &Keymap::default(), &extras);
        })
        .unwrap();
        let buf = t.backend().buffer();
        let reversed = buf
            .content()
            .iter()
            .filter(|c| c.modifier.contains(Modifier::REVERSED))
            .count();
        // At most the boot's own cursor cell.
        assert!(reversed <= 1, "{}: {reversed} reversed cells", theme.name);
        assert!(
            !buf.content()
                .iter()
                .any(|c| c.symbol() == "S" && c.modifier.contains(Modifier::REVERSED))
        );
    }
}

#[test]
fn line_speed_is_shown_on_serial_themes_only() {
    use ttnr_themes::{ColorDepth, ThemeSet};
    let set = ThemeSet::builtin();
    let mut s = state();
    s.baud = 9600;
    let shown = |name: &str| {
        let theme = set
            .get(set.position(name).unwrap())
            .for_depth(ColorDepth::TrueColor);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| ttnr_tui::draw(f, &s, &theme, &Keymap::default()))
            .unwrap();
        t.backend().to_string()
    };
    assert!(shown("pdp11-vt100").contains("9600 BAUD"));
    assert!(!shown("plain").contains("baud"));
}

/// Each diagnostics indicator can be shown alone (M9 acceptance).
#[test]
fn each_diagnostics_indicator_alone() {
    use ttnr_core::app::{DiagSelection, Diagnostics, HttpActivity};
    let base = DiagSelection::default();
    let cases = [
        (
            DiagSelection {
                network: true,
                ..base
            },
            "net active",
        ),
        (
            DiagSelection {
                download: true,
                ..base
            },
            "16 kB/s buf 12 s",
        ),
        (
            DiagSelection {
                metadata: true,
                ..base
            },
            "rms.api.bbc.co.uk 3 s ago ok",
        ),
        (
            DiagSelection {
                sync_note: true,
                ..base
            },
            "may lag",
        ),
    ];
    for (shown, expected) in cases {
        let mut s = state();
        s.now.as_mut().unwrap().source = ttnr_core::InfoSource::External;
        s.diag = Diagnostics {
            shown,
            bytes_per_sec: Some(16_000),
            buffered_secs: Some(12),
            http: Some(HttpActivity {
                host: "rms.api.bbc.co.uk".into(),
                ago_secs: 3,
                outcome: "ok".into(),
            }),
        };
        let text = render(&s, 100, 30);
        assert!(text.contains(expected), "{shown:?}: {text}");
        let others = ["net ", "kB/s", "ago", "may lag"]
            .iter()
            .filter(|o| text.contains(**o))
            .count();
        assert_eq!(others, 1, "only one indicator for {shown:?}");
    }
    // Hidden: no diagnostics line at all.
    assert!(!render(&state(), 100, 30).contains("diag:"));
}
