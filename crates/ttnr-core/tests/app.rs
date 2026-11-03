//! The application state machine, driven by messages only.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::SystemTime;

use ttnr_core::app::{App, Command, Device, Effect, Msg, PlayerEvent, Status, View};
use ttnr_core::{InfoSource, NowPlaying, Station, StreamDescriptor, StreamTags};

fn stations() -> Vec<Station> {
    ["a", "b", "c"]
        .iter()
        .map(|id| Station {
            name: format!("Station {id}"),
            ..Station::from_url(&format!("http://{id}/s"))
        })
        .collect()
}

fn app() -> App {
    App::new(stations(), 50, 2)
}

fn resolved(app: &App, urls: &[&str]) -> Msg {
    Msg::Resolved {
        key: app.state.station.as_ref().unwrap().key(),
        result: Ok(StreamDescriptor {
            urls: urls.iter().map(|u| (*u).to_owned()).collect(),
            ..StreamDescriptor::default()
        }),
    }
}

fn play(url: &str) -> Effect {
    Effect::Play {
        url: url.into(),
        headers: vec![],
    }
}

fn failed(stale: bool) -> Msg {
    Msg::Player(PlayerEvent::Failed {
        retryable: true,
        stale_hint: stale,
        message: "HTTP 404 Not Found".into(),
    })
}

#[test]
fn play_resolve_and_start() {
    let mut a = app();
    let st = a.state.stations[1].clone();
    assert_eq!(
        a.update(Msg::Play(st.clone())),
        [Effect::Stop, Effect::Resolve(st.clone())]
    );
    assert_eq!(a.state.status, Status::Resolving);
    let fx = a.update(resolved(&a, &["http://m1", "http://m2"]));
    assert_eq!(fx, [play("http://m1"), Effect::StartMetadata(st)]);
    assert_eq!(a.state.status, Status::Loading);
    a.update(Msg::Player(PlayerEvent::Playing));
    assert_eq!(a.state.status, Status::Playing);
}

#[test]
fn failover_mirror_then_refresh_then_error() {
    let mut a = app();
    a.update(Msg::Play(a.state.stations[0].clone()));
    a.update(resolved(&a, &["http://m1", "http://m2"]));
    assert_eq!(a.update(failed(true)), [play("http://m2")]);
    assert!(matches!(a.state.status, Status::Reconnecting(_)));
    let st = a.state.station.clone().unwrap();
    assert_eq!(a.update(failed(true)), [Effect::Refresh(st.clone())]);
    let fx = a.update(Msg::Refreshed {
        key: st.key(),
        result: Ok(StreamDescriptor::direct("http://fresh")),
    });
    assert_eq!(fx, [play("http://fresh")], "no second metadata start");
    assert_eq!(a.update(failed(true)), []);
    assert_eq!(a.state.status, Status::Error("HTTP 404 Not Found".into()));
}

#[test]
fn late_resolution_for_another_station_is_ignored() {
    let mut a = app();
    a.update(Msg::Play(a.state.stations[0].clone()));
    let stale = resolved(&a, &["http://old"]);
    a.update(Msg::Play(a.state.stations[1].clone()));
    assert_eq!(a.update(stale), []);
    assert_eq!(a.state.status, Status::Resolving);
}

#[test]
fn resolve_error_is_shown() {
    let mut a = app();
    a.update(Msg::Play(a.state.stations[0].clone()));
    let key = a.state.station.as_ref().unwrap().key();
    a.update(Msg::Resolved {
        key,
        result: Err("cannot resolve".into()),
    });
    assert_eq!(a.state.status, Status::Error("cannot resolve".into()));
}

#[test]
fn now_playing_history_and_priority() {
    let mut a = app();
    a.update(Msg::Play(a.state.stations[0].clone()));
    a.update(resolved(&a, &["http://m1"]));
    let icy = |t: &str| {
        Msg::Player(PlayerEvent::Tags(StreamTags {
            icy_title: Some(t.into()),
            ..StreamTags::default()
        }))
    };
    for t in ["A - 1", "B - 2", "C - 3", "D - 4"] {
        a.update(icy(t));
    }
    assert_eq!(a.state.now.as_ref().unwrap().title.as_deref(), Some("4"));
    let hist: Vec<_> = a
        .state
        .history
        .iter()
        .map(|n| n.title.clone().unwrap())
        .collect();
    assert_eq!(hist, ["3", "2"], "most recent first, capped at 2");
    // A provider with higher priority takes over.
    let ext = NowPlaying {
        title: Some("External".into()),
        ..NowPlaying::empty(InfoSource::External, SystemTime::now())
    };
    a.update(Msg::NowPlaying(ext));
    assert_eq!(a.state.now.as_ref().unwrap().source, InfoSource::External);
    assert_eq!(a.state.history[0].title.as_deref(), Some("4"));
}

#[test]
fn volume_mute_pause() {
    let mut a = app();
    assert_eq!(
        a.update(Msg::Command(Command::VolumeUp)),
        [Effect::SetVolume(55)]
    );
    assert_eq!(
        a.update(Msg::Command(Command::ToggleMute)),
        [Effect::SetVolume(0)]
    );
    assert!(a.state.muted);
    assert_eq!(
        a.update(Msg::Command(Command::VolumeDown)),
        [Effect::SetVolume(50)],
        "volume key unmutes"
    );
    for _ in 0..30 {
        a.update(Msg::Command(Command::VolumeUp));
    }
    assert_eq!(a.state.volume, 100);
    assert_eq!(
        a.update(Msg::Command(Command::TogglePause)),
        [],
        "nothing playing"
    );
    a.state.status = Status::Playing;
    assert_eq!(
        a.update(Msg::Command(Command::TogglePause)),
        [Effect::Pause(true)]
    );
    a.update(Msg::Player(PlayerEvent::Paused(true)));
    assert_eq!(
        a.update(Msg::Command(Command::TogglePause)),
        [Effect::Pause(false)]
    );
}

#[test]
fn next_and_previous_station_wrap() {
    let mut a = app();
    a.update(Msg::Command(Command::NextStation));
    assert_eq!(a.state.station.as_ref().unwrap().name, "Station a");
    a.update(Msg::Command(Command::PrevStation));
    assert_eq!(a.state.station.as_ref().unwrap().name, "Station c");
    a.update(Msg::Command(Command::NextStation));
    assert_eq!(a.state.station.as_ref().unwrap().name, "Station a");
}

#[test]
fn station_list_navigation() {
    let mut a = app();
    a.update(Msg::Command(Command::OpenStations));
    assert_eq!(a.state.view, View::Stations { selected: 0 });
    for _ in 0..5 {
        a.update(Msg::Command(Command::Down));
    }
    assert_eq!(a.state.view, View::Stations { selected: 2 }, "clamped");
    a.update(Msg::Command(Command::Up));
    let fx = a.update(Msg::Command(Command::Select));
    assert_eq!(fx[1], Effect::Resolve(a.state.stations[1].clone()));
    assert_eq!(a.state.view, View::Main);
}

#[test]
fn open_url_prompt_takes_text() {
    let mut a = app();
    a.update(Msg::Command(Command::OpenUrl));
    for c in "http://x/s.plsq".chars() {
        a.update(Msg::Command(Command::Input(c)));
    }
    a.update(Msg::Command(Command::Backspace));
    // Command keys are text inside the prompt.
    a.update(Msg::Command(Command::Quit));
    assert!(!a.state.quit);
    let fx = a.update(Msg::Command(Command::Select));
    assert_eq!(fx[1], Effect::Resolve(Station::from_url("http://x/s.pls")));
}

#[test]
fn device_picker() {
    let mut a = app();
    a.update(Msg::Devices(vec![
        Device {
            name: "coreaudio/A".into(),
            description: "Speakers".into(),
        },
        Device {
            name: "coreaudio/B".into(),
            description: "AirPods".into(),
        },
    ]));
    a.update(Msg::Command(Command::OpenDevices));
    assert_eq!(a.state.view, View::Devices { selected: 0 });
    a.update(Msg::Command(Command::Down));
    a.update(Msg::Command(Command::Down));
    assert_eq!(
        a.update(Msg::Command(Command::Select)),
        [Effect::SetDevice(Some("coreaudio/B".into()))]
    );
    assert_eq!(a.state.device.as_deref(), Some("coreaudio/B"));
    a.update(Msg::Command(Command::OpenDevices));
    assert_eq!(
        a.state.view,
        View::Devices { selected: 2 },
        "opens on the pinned device"
    );
    a.update(Msg::Command(Command::Up));
    a.update(Msg::Command(Command::Up));
    assert_eq!(
        a.update(Msg::Command(Command::Select)),
        [Effect::SetDevice(None)]
    );
}

#[test]
fn cycle_theme_changes_state_only() {
    let mut a = app();
    assert_eq!(a.update(Msg::Command(Command::CycleTheme)), []);
    assert_eq!(a.state.theme_index, 1);
}

#[test]
fn help_back_quit() {
    let mut a = app();
    a.update(Msg::Command(Command::ToggleHelp));
    assert_eq!(a.state.view, View::Help);
    a.update(Msg::Command(Command::Back));
    assert_eq!(a.state.view, View::Main);
    assert_eq!(
        a.update(Msg::Command(Command::Quit)),
        [Effect::Stop, Effect::Quit]
    );
    assert!(a.state.quit);
}

#[test]
fn provider_now_playing_is_delayed_by_the_sync_offset() {
    use std::collections::BTreeMap;
    use std::time::Duration;
    let mut a = App::new(stations(), 50, 2)
        .with_sync_offsets(BTreeMap::from([("url".into(), Duration::from_secs(20))]));
    a.update(Msg::Play(a.state.stations[0].clone()));
    assert_eq!(a.state.sync_offset, Duration::from_secs(20));
    let np = NowPlaying {
        title: Some("Later".into()),
        ..NowPlaying::empty(InfoSource::External, SystemTime::now())
    };
    let fx = a.update(Msg::NowPlaying(np.clone()));
    let [Effect::Delay { after, msg }] = fx.as_slice() else {
        panic!("{fx:?}")
    };
    assert_eq!(*after, Duration::from_secs(20));
    assert!(a.state.now.is_none(), "not shown yet");
    a.update((**msg).clone());
    assert_eq!(
        a.state.now.as_ref().unwrap().title.as_deref(),
        Some("Later")
    );

    // A due update for a station we left is dropped.
    let Msg::NowPlayingDue { np: due_np, .. } = (**msg).clone() else {
        panic!()
    };
    a.update(Msg::Play(a.state.stations[1].clone()));
    a.update(Msg::NowPlayingDue {
        key: "url:http://a/s".into(),
        np: due_np,
    });
    assert!(a.state.now.is_none());
}

#[test]
fn sync_offset_keys_adjust_per_source() {
    use std::time::Duration;
    let mut a = app();
    a.update(Msg::Play(a.state.stations[0].clone()));
    a.update(Msg::Command(Command::SyncEarlier));
    assert_eq!(a.state.sync_offset, Duration::ZERO, "not below zero");
    for _ in 0..3 {
        a.update(Msg::Command(Command::SyncLater));
    }
    assert_eq!(a.state.sync_offset, Duration::from_secs(3));
    // Same source keeps the tuned value on the next station.
    a.update(Msg::Command(Command::NextStation));
    assert_eq!(a.state.sync_offset, Duration::from_secs(3));
    // Zero offset: shown immediately.
    a.state.sync_offset = Duration::ZERO;
    let np = NowPlaying {
        title: Some("Now".into()),
        ..NowPlaying::empty(InfoSource::External, SystemTime::now())
    };
    assert_eq!(a.update(Msg::NowPlaying(np)), []);
    assert_eq!(a.state.now.as_ref().unwrap().title.as_deref(), Some("Now"));
}
