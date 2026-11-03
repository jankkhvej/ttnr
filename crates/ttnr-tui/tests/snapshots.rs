//! Snapshot tests: every view in several states at the three reference
//! sizes (32×16, 40×25, 100×30; THEMES.md), per theme. Snapshots hold the
//! characters only – colours are covered by `views.rs`. Review changes with
//! `cargo insta review` (or inspect `*.snap.new`).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::SystemTime;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ttnr_core::app::{AppState, Device, Status, View};
use ttnr_core::{InfoSource, NowPlaying, Station, StreamInfo};
use ttnr_themes::{ColorDepth, ThemeSet};
use ttnr_tui::Keymap;

const SIZES: [(u16, u16); 3] = [(32, 16), (40, 25), (100, 30)];

fn np(artist: &str, title: &str) -> NowPlaying {
    NowPlaying {
        artist: Some(artist.into()),
        title: Some(title.into()),
        ..NowPlaying::empty(InfoSource::Icy, SystemTime::UNIX_EPOCH)
    }
}

fn station(id: &str, name: &str, source: &str) -> Station {
    Station {
        id: id.into(),
        name: name.into(),
        source: source.into(),
        url: None,
        params: Default::default(),
    }
}

fn stations() -> Vec<Station> {
    vec![
        station("groovesalad", "SomaFM Groove Salad", "url"),
        station("fip", "FIP (Radio France)", "url"),
        station("bbc_6music", "BBC Radio 6 Music", "bbc"),
        station("bbc_radio_three", "BBC Radio 3", "bbc"),
    ]
}

fn devices() -> Vec<Device> {
    vec![
        Device {
            name: "coreaudio/BuiltInSpeakerDevice".into(),
            description: "MacBook Air Speakers".into(),
        },
        Device {
            name: "coreaudio/1C-6E-4C-8E-4A-5B:output".into(),
            description: "MAJOR IV".into(),
        },
    ]
}

fn idle() -> AppState {
    AppState {
        volume: 100,
        stations: stations(),
        devices: devices(),
        ..AppState::default()
    }
}

fn playing() -> AppState {
    AppState {
        station: Some(stations()[2].clone()),
        status: Status::Playing,
        now: Some(NowPlaying {
            programme: Some("Lauren Laverne".into()),
            album: Some("Is This It (20th Anniversary Edition)".into()),
            ..np("The Strokes", "Someday")
        }),
        history: [
            np("Wet Leg", "Chaise Longue"),
            np("Khruangbin", "Time (You and I)"),
            np("Fontaines D.C.", "Starburster"),
        ]
        .into(),
        stream: Some(StreamInfo {
            codec: Some("aac".into()),
            codec_profile: Some("HE-AAC".into()),
            sample_rate: Some(48_000),
            channels: Some("stereo".into()),
            bitrate: Some(96_000),
            output_sample_rate: Some(48_000),
            output_channels: Some("stereo".into()),
            ..StreamInfo::default()
        }),
        volume: 80,
        device: Some("coreaudio/1C-6E-4C-8E-4A-5B:output".into()),
        ..idle()
    }
}

fn buffering() -> AppState {
    AppState {
        status: Status::Reconnecting("HTTP 404 Not Found; next stream".into()),
        now: None,
        history: Default::default(),
        stream: None,
        muted: true,
        ..playing()
    }
}

fn error() -> AppState {
    AppState {
        station: Some(stations()[1].clone()),
        status: Status::Error("server unreachable (tcp: Connection refused)".into()),
        now: None,
        history: Default::default(),
        stream: None,
        ..idle()
    }
}

fn cases() -> Vec<(&'static str, AppState)> {
    let with = |mut s: AppState, view| {
        s.view = view;
        s
    };
    vec![
        ("main_idle", idle()),
        ("main_playing", playing()),
        ("main_reconnecting", buffering()),
        ("main_error", error()),
        ("stations", with(playing(), View::Stations { selected: 1 })),
        (
            "open_url",
            with(
                playing(),
                View::OpenUrl {
                    input: "https://somafm.com/groovesalad.pls".into(),
                },
            ),
        ),
        ("help", with(playing(), View::Help)),
        ("devices", with(playing(), View::Devices { selected: 0 })),
    ]
}

#[test]
fn all_views_all_sizes() {
    let themes = ThemeSet::builtin();
    let keymap = Keymap::default();
    for i in 0..themes.len() {
        let theme = themes.get(i).for_depth(ColorDepth::Ansi256);
        for (name, state) in cases() {
            for (w, h) in SIZES {
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| ttnr_tui::draw(f, &state, &theme, &keymap))
                    .unwrap();
                insta::with_settings!({
                    snapshot_suffix => format!("{}_{name}_{w}x{h}", theme.name),
                    omit_expression => true,
                    prepend_module_to_snapshot => false,
                }, {
                    insta::assert_snapshot!("view", t.backend());
                });
            }
        }
    }
}

/// Printout themes (Teletype) append lines instead of redrawing: feed a
/// session idle → playing → reconnecting → error and snapshot the paper.
#[test]
fn printout_session() {
    let themes = ThemeSet::builtin();
    let keymap = Keymap::default();
    for i in 0..themes.len() {
        let theme = themes.get(i).for_depth(ColorDepth::Ansi256);
        if !theme.chrome.printout {
            continue;
        }
        let mut paper = ttnr_tui::Printout::new(&theme);
        for state in [idle(), playing(), buffering(), error()] {
            paper.observe(&state, &theme);
        }
        for (w, h) in SIZES {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| ttnr_tui::draw_with_printout(f, &error(), &theme, &keymap, paper.lines()))
                .unwrap();
            insta::with_settings!({
                snapshot_suffix => format!("{}_printout_{w}x{h}", theme.name),
                omit_expression => true,
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!("view", t.backend());
            });
        }
    }
}

/// Animation frames: boot sequences at characteristic moments and the
/// tape-loading border (characters only; colours in the fx unit tests).
#[test]
fn animation_frames() {
    use std::time::Duration;
    use ttnr_themes::BootEffect;
    use ttnr_tui::{Extras, Fx, FxFrame};
    let themes = ThemeSet::builtin();
    let keymap = Keymap::default();
    let fx = Fx::new(true, ColorDepth::TrueColor);
    for i in 0..themes.len() {
        let theme = themes.get(i).for_depth(ColorDepth::TrueColor);
        let mut frames: Vec<(String, FxFrame)> = Vec::new();
        let boot = theme.effects.boot;
        if boot != BootEffect::None {
            let moments: &[u64] = match boot {
                BootEffect::ZxSpectrum => &[100, 600, 2000],
                BootEffect::Vt100 => &[100, 600, 1500, 3000],
                _ => &[100, 1000],
            };
            for ms in moments {
                frames.push((
                    format!("boot_{ms}ms"),
                    FxFrame::Boot {
                        effect: boot,
                        t: Duration::from_millis(*ms),
                        seed: 1,
                    },
                ));
            }
        }
        if theme.effects.loading == ttnr_themes::LoadingEffect::Tape {
            for (name, pilot) in [("tape_pilot", true), ("tape_data", false)] {
                frames.push((
                    name.into(),
                    FxFrame::Tape {
                        pilot,
                        tape: 0.0,
                        seed: 1,
                    },
                ));
            }
        }
        for (name, f) in frames {
            let mut t = Terminal::new(TestBackend::new(48, 30)).unwrap();
            t.draw(|frame| {
                let extras = Extras {
                    printout: &[],
                    fx: Some((&fx, f)),
                };
                ttnr_tui::draw_with(frame, &playing(), &theme, &keymap, &extras);
            })
            .unwrap();
            insta::with_settings!({
                snapshot_suffix => format!("{}_{name}", theme.name),
                omit_expression => true,
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!("fx", t.backend());
            });
        }
    }
}
