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

fn external(title: &str, at: SystemTime) -> NowPlaying {
    NowPlaying {
        title: Some(title.into()),
        ..NowPlaying::empty(InfoSource::External, at)
    }
}

fn delay_of(fx: &[Effect]) -> (std::time::Duration, Msg) {
    match fx {
        [Effect::Delay { after, msg }] => (*after, (**msg).clone()),
        other => panic!("expected one delay, got {other:?}"),
    }
}

#[test]
fn provider_update_is_shown_at_its_time_plus_offset() {
    use std::collections::BTreeMap;
    use std::time::Duration;
    let mut a = App::new(stations(), 50, 2)
        .with_sync_offsets(BTreeMap::from([("url".into(), Duration::from_secs(20))]));
    a.update(Msg::Play(a.state.stations[0].clone()));
    assert_eq!(a.state.sync_offset, Duration::from_secs(20));

    // Track started 5 s ago: due in ~15 s.
    let started = SystemTime::now() - Duration::from_secs(5);
    let (after, due) = delay_of(&a.update(Msg::NowPlaying(external("Song", started))));
    assert!(
        (Duration::from_secs(14)..=Duration::from_secs(15)).contains(&after),
        "{after:?}"
    );
    assert!(a.state.now.is_none(), "not shown yet");

    // Lowering the offset re-times it: now overdue → shown at once.
    for _ in 0..20 {
        a.update(Msg::Command(Command::SyncEarlier));
    }
    assert_eq!(a.state.now.as_ref().unwrap().title.as_deref(), Some("Song"));
    // The old timer fires later: stale generation, nothing happens.
    assert_eq!(a.update(due), []);

    // An update that is already overdue is shown immediately.
    let mut b = App::new(stations(), 50, 2)
        .with_sync_offsets(BTreeMap::from([("url".into(), Duration::from_secs(20))]));
    b.update(Msg::Play(b.state.stations[0].clone()));
    let long_ago = SystemTime::now() - Duration::from_secs(60);
    assert_eq!(b.update(Msg::NowPlaying(external("Old", long_ago))), []);
    assert_eq!(b.state.now.as_ref().unwrap().title.as_deref(), Some("Old"));
}

#[test]
fn raising_the_offset_postpones_and_station_change_cancels() {
    use std::time::Duration;
    let mut a = app();
    a.update(Msg::Play(a.state.stations[0].clone()));
    a.update(Msg::Command(Command::SyncLater)); // 1 s
    let (_, first) = delay_of(&a.update(Msg::NowPlaying(external("Song", SystemTime::now()))));
    let (after, _) = delay_of(&a.update(Msg::Command(Command::SyncLater))); // 2 s
    assert!(after > Duration::from_millis(1500), "{after:?}");
    assert_eq!(a.update(first), [], "superseded timer");
    // Switching station drops the pending update.
    a.update(Msg::Command(Command::NextStation));
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

#[test]
fn cycle_baud_steps_and_wraps() {
    use ttnr_core::app::BAUD_STEPS;
    let mut a = App::new(Vec::new(), 80, 5);
    let mut seen = Vec::new();
    for _ in 0..BAUD_STEPS.len() {
        assert_eq!(a.update(Msg::Command(Command::CycleBaud)), []);
        seen.push(a.state.baud);
    }
    assert_eq!(seen, [9600, 1200, 300, 110, 0]);
    a.state.baud = 2400; // from config, not a step
    a.update(Msg::Command(Command::CycleBaud));
    assert_eq!(a.state.baud, 9600);
}

#[test]
fn pinned_device_falls_back_to_default_and_returns() {
    let speakers = Device {
        name: "coreaudio/A".into(),
        description: "Speakers".into(),
    };
    let airpods = Device {
        name: "coreaudio/B".into(),
        description: "AirPods".into(),
    };
    let mut a = app();
    // Pinned from config by description, before the list is known.
    a.state.device = Some("AirPods".into());
    assert_eq!(
        a.update(Msg::Devices(vec![speakers.clone()])),
        [],
        "absent: default stays"
    );
    assert_eq!(
        a.update(Msg::Devices(vec![speakers.clone(), airpods.clone()])),
        [Effect::SetDevice(Some("coreaudio/B".into()))],
        "appeared: switch to it"
    );
    assert_eq!(a.update(Msg::Devices(vec![speakers.clone(), airpods])), []);
    assert_eq!(
        a.update(Msg::Devices(vec![speakers])),
        [Effect::SetDevice(None)],
        "vanished: back to the system default"
    );
    assert_eq!(a.state.device.as_deref(), Some("AirPods"), "pin kept");
    a.update(Msg::DefaultDevice(Some("Speakers".into())));
    assert_eq!(a.state.default_device.as_deref(), Some("Speakers"));
}

/// Starts station 0 on one URL and reports it playing.
fn playing_app() -> App {
    let mut a = app();
    let st = a.state.stations[0].clone();
    a.update(Msg::Play(st));
    a.update(resolved(&a, &["http://m1"]));
    a.update(Msg::Player(PlayerEvent::Playing));
    a
}

/// The delay and the scheduled message of a reconnection.
fn retry_of(fx: &[Effect]) -> (u64, Msg) {
    assert_eq!(fx[0], Effect::Stop, "{fx:?}");
    match &fx[1] {
        Effect::Delay { after, msg } => (after.as_secs(), (**msg).clone()),
        other => panic!("expected a delay, got {other:?}"),
    }
}

fn unreachable() -> Msg {
    Msg::Player(PlayerEvent::Failed {
        retryable: true,
        stale_hint: false,
        message: "server unreachable".into(),
    })
}

#[test]
fn lost_connection_reconnects_with_backoff() {
    let mut a = playing_app();
    let st = a.state.stations[0].clone();
    let mut delays = Vec::new();
    for _ in 0..8 {
        let (secs, retry) = retry_of(&a.update(unreachable()));
        delays.push(secs);
        assert!(matches!(&a.state.status, Status::Reconnecting(m) if m.contains("retry in")));
        assert_eq!(a.update(retry), [Effect::Stop, Effect::Resolve(st.clone())]);
        a.update(resolved(&a, &["http://m1"]));
    }
    assert_eq!(delays, [1, 2, 4, 8, 16, 30, 60, 60]);
    // Playing again resets the backoff.
    a.update(Msg::Player(PlayerEvent::Playing));
    assert_eq!(retry_of(&a.update(unreachable())).0, 1);
}

#[test]
fn a_station_that_never_played_fails_as_before() {
    let mut a = app();
    let st = a.state.stations[0].clone();
    a.update(Msg::Play(st));
    a.update(resolved(&a, &["http://m1"]));
    assert_eq!(a.update(unreachable()), []);
    assert!(matches!(a.state.status, Status::Error(_)));
}

#[test]
fn live_stream_end_reconnects_but_a_file_ends() {
    let mut a = playing_app();
    a.update(Msg::Player(PlayerEvent::StreamInfo(
        ttnr_core::StreamInfo {
            seekable: Some(false),
            ..ttnr_core::StreamInfo::default()
        },
    )));
    assert_eq!(retry_of(&a.update(Msg::Player(PlayerEvent::Ended))).0, 1);

    let mut f = playing_app();
    f.update(Msg::Player(PlayerEvent::StreamInfo(
        ttnr_core::StreamInfo {
            seekable: Some(true),
            ..ttnr_core::StreamInfo::default()
        },
    )));
    assert_eq!(f.update(Msg::Player(PlayerEvent::Ended)), []);
    assert_eq!(f.state.status, Status::Ended);
}

#[test]
fn stale_or_foreign_retries_are_ignored() {
    let mut a = playing_app();
    let (_, retry) = retry_of(&a.update(unreachable()));
    // The user picks another station meanwhile.
    let other = a.state.stations[1].clone();
    a.update(Msg::Play(other));
    assert_eq!(a.update(retry), [], "station changed");

    let mut b = playing_app();
    let (_, first) = retry_of(&b.update(unreachable()));
    b.update(Msg::Woke); // reconnects now, supersedes the scheduled one
    assert_eq!(b.update(first), [], "superseded");
}

#[test]
fn wake_reconnects_only_a_live_session() {
    let mut a = playing_app();
    let st = a.state.stations[0].clone();
    assert_eq!(a.update(Msg::Woke), [Effect::Stop, Effect::Resolve(st)]);
    assert!(matches!(&a.state.status, Status::Reconnecting(m) if m.contains("woke")));
    let mut idle = app();
    assert_eq!(idle.update(Msg::Woke), []);
    let mut paused = playing_app();
    paused.update(Msg::Player(PlayerEvent::Paused(true)));
    assert_eq!(paused.update(Msg::Woke), [], "a paused stream stays paused");
}

#[test]
fn resolution_failure_while_reconnecting_retries() {
    let mut a = playing_app();
    let (_, retry) = retry_of(&a.update(unreachable()));
    a.update(retry);
    let key = a.state.station.as_ref().unwrap().key();
    let fx = a.update(Msg::Resolved {
        key,
        result: Err("dns: no network".into()),
    });
    assert_eq!(retry_of(&fx).0, 2);
}

#[test]
fn directory_search_flow() {
    let mut a = app();
    a.update(Msg::Command(Command::OpenSearch));
    assert_eq!(
        a.state.view,
        View::Search {
            input: String::new()
        }
    );
    for c in "jazz".chars() {
        a.update(Msg::Command(Command::Input(c)));
    }
    a.update(Msg::Command(Command::Input('n'))); // keys are text in the prompt
    a.update(Msg::Command(Command::Backspace));
    assert_eq!(
        a.update(Msg::Command(Command::Select)),
        [Effect::Search("jazz".into())]
    );
    assert_eq!(a.state.view, View::Results { selected: 0 });
    assert_eq!(a.state.search.note.as_deref(), Some("searching…"));
    // An older search's answer is ignored.
    a.update(Msg::SearchResults {
        query: "rock".into(),
        result: Ok(stations()),
    });
    assert!(a.state.search.results.is_empty());
    let found: Vec<Station> = stations().into_iter().take(2).collect();
    a.update(Msg::SearchResults {
        query: "jazz".into(),
        result: Ok(found.clone()),
    });
    assert_eq!(a.state.search.results, found);
    assert_eq!(a.state.search.note, None);
    a.update(Msg::Command(Command::Down));
    a.update(Msg::Command(Command::Down));
    assert_eq!(a.state.view, View::Results { selected: 1 }, "clamped");
    assert_eq!(
        a.update(Msg::Command(Command::Select)),
        [Effect::Stop, Effect::Resolve(found[1].clone())]
    );
    // Reopening the prompt offers the last query.
    a.update(Msg::Command(Command::OpenSearch));
    assert_eq!(
        a.state.view,
        View::Search {
            input: "jazz".into()
        }
    );
}

#[test]
fn directory_search_errors_and_empty_results() {
    let mut a = app();
    a.update(Msg::Command(Command::OpenSearch));
    a.update(Msg::Command(Command::Input('x')));
    a.update(Msg::Command(Command::Select));
    a.update(Msg::SearchResults {
        query: "x".into(),
        result: Ok(Vec::new()),
    });
    assert_eq!(a.state.search.note.as_deref(), Some("no stations found"));
    assert_eq!(
        a.update(Msg::Command(Command::Select)),
        [],
        "nothing to play"
    );
    a.update(Msg::SearchResults {
        query: "x".into(),
        result: Err("http: timeout".into()),
    });
    assert_eq!(a.state.search.note.as_deref(), Some("http: timeout"));
}

#[test]
fn added_stations_join_the_list_once() {
    let mut a = app();
    let extra = Station {
        name: "Script FM".into(),
        ..Station::from_url("http://script/fm")
    };
    a.update(Msg::AddStations(vec![extra.clone(), stations()[0].clone()]));
    assert_eq!(
        a.state.stations.len(),
        4,
        "duplicate of a listed station skipped"
    );
    a.update(Msg::AddStations(vec![extra]));
    assert_eq!(a.state.stations.len(), 4);
}

#[test]
fn set_volume_clamps_and_unmutes() {
    let mut a = app();
    a.update(Msg::Command(Command::ToggleMute));
    assert_eq!(
        a.update(Msg::Command(Command::SetVolume(130))),
        [Effect::SetVolume(100)]
    );
    assert!(!a.state.muted);
    assert_eq!(a.state.volume, 100);
}

#[test]
fn an_icecast_stream_without_size_reconnects() {
    let mut a = playing_app();
    a.update(Msg::Player(PlayerEvent::StreamInfo(
        ttnr_core::StreamInfo {
            container: Some("mp3".into()),
            seekable: Some(false),
            ..ttnr_core::StreamInfo::default()
        },
    )));
    assert_eq!(retry_of(&a.update(Msg::Player(PlayerEvent::Ended))).0, 1);
}

#[test]
fn an_http_file_of_known_size_ends() {
    let mut a = playing_app();
    a.update(Msg::Player(PlayerEvent::StreamInfo(
        ttnr_core::StreamInfo {
            container: Some("wav".into()),
            seekable: Some(false),
            size_bytes: Some(192_044),
            ..ttnr_core::StreamInfo::default()
        },
    )));
    assert_eq!(a.update(Msg::Player(PlayerEvent::Ended)), []);
    assert_eq!(a.state.status, Status::Ended);
}
