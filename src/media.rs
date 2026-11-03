//! Mapping between ttnr's state and the system's Now Playing / media keys
//! (ADR-036): what Control Center shows, and what a media key does.

use ttnr_core::NowPlaying;
use ttnr_core::app::{AppState, Command, Status};
use ttnr_platform::{NowPlayingInfo, RemoteCommand};

/// Now Playing for a station: the track if known (title, artist, station as
/// album), else the station with its programme.
/// `can_skip`: next/previous station are available.
pub fn info(
    station: &str,
    now: Option<&NowPlaying>,
    playing: bool,
    can_skip: bool,
) -> NowPlayingInfo {
    let programme = now.and_then(|n| n.programme.clone());
    match now.and_then(|n| n.title.clone()) {
        Some(title) => NowPlayingInfo {
            title,
            artist: now.and_then(|n| n.artist.clone()).or(programme),
            album: Some(station.to_owned()),
            playing,
            can_skip,
        },
        None => NowPlayingInfo {
            title: station.to_owned(),
            artist: programme,
            album: None,
            playing,
            can_skip,
        },
    }
}

/// Now Playing for the TUI state; `None` when nothing is (about to be)
/// playing.
pub fn from_state(s: &AppState) -> Option<NowPlayingInfo> {
    let station = s.station.as_ref()?;
    let playing = match s.status {
        Status::Idle | Status::Ended | Status::Error(_) => return None,
        Status::Paused => false,
        Status::Resolving
        | Status::Loading
        | Status::Buffering(_)
        | Status::Playing
        | Status::Reconnecting(_) => true,
    };
    Some(info(
        &station.name,
        s.now.as_ref(),
        playing,
        !s.stations.is_empty(),
    ))
}

/// The app command for a media key in state `s` (play/pause only when they
/// change something; next/previous switch stations).
pub fn command_for(remote: RemoteCommand, s: &AppState) -> Option<Command> {
    let paused = matches!(s.status, Status::Paused);
    match remote {
        RemoteCommand::TogglePause => Some(Command::TogglePause),
        RemoteCommand::Play => paused.then_some(Command::TogglePause),
        RemoteCommand::Pause => {
            (!paused && from_state(s).is_some()).then_some(Command::TogglePause)
        }
        RemoteCommand::Next => Some(Command::NextStation),
        RemoteCommand::Previous => Some(Command::PrevStation),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use ttnr_core::{InfoSource, Station};

    fn np(artist: Option<&str>, title: Option<&str>, programme: Option<&str>) -> NowPlaying {
        NowPlaying {
            artist: artist.map(Into::into),
            title: title.map(Into::into),
            programme: programme.map(Into::into),
            ..NowPlaying::empty(InfoSource::External, SystemTime::UNIX_EPOCH)
        }
    }

    fn state(status: Status, now: Option<NowPlaying>) -> AppState {
        AppState {
            station: Some(Station {
                name: "BBC Radio 6 Music".into(),
                ..Station::from_url("http://x")
            }),
            status,
            now,
            ..AppState::default()
        }
    }

    #[test]
    fn track_station_and_programme_lines() {
        let full = np(Some("The Strokes"), Some("Someday"), Some("Lauren Laverne"));
        assert_eq!(
            info("6 Music", Some(&full), true, true),
            NowPlayingInfo {
                title: "Someday".into(),
                artist: Some("The Strokes".into()),
                album: Some("6 Music".into()),
                playing: true,
                can_skip: true,
            }
        );
        let title_only = np(None, Some("Someday"), Some("Lauren Laverne"));
        assert_eq!(
            info("6 Music", Some(&title_only), true, true)
                .artist
                .as_deref(),
            Some("Lauren Laverne")
        );
        let speech = np(None, None, Some("Today"));
        assert_eq!(
            info("Radio 4", Some(&speech), false, false),
            NowPlayingInfo {
                title: "Radio 4".into(),
                artist: Some("Today".into()),
                album: None,
                playing: false,
                can_skip: false,
            }
        );
    }

    #[test]
    fn state_mapping() {
        assert_eq!(from_state(&AppState::default()), None);
        assert_eq!(from_state(&state(Status::Error("x".into()), None)), None);
        assert!(from_state(&state(Status::Loading, None)).unwrap().playing);
        assert!(!from_state(&state(Status::Paused, None)).unwrap().playing);
        // Next/previous only with a station list.
        let mut s = state(Status::Playing, None);
        assert!(!from_state(&s).unwrap().can_skip);
        s.stations = vec![ttnr_core::Station::from_url("http://y")];
        assert!(from_state(&s).unwrap().can_skip);
    }

    #[test]
    fn media_keys() {
        let playing = state(Status::Playing, None);
        let paused = state(Status::Paused, None);
        let idle = AppState::default();
        use RemoteCommand::*;
        assert_eq!(
            command_for(TogglePause, &playing),
            Some(Command::TogglePause)
        );
        assert_eq!(command_for(Play, &playing), None);
        assert_eq!(command_for(Play, &paused), Some(Command::TogglePause));
        assert_eq!(command_for(Pause, &playing), Some(Command::TogglePause));
        assert_eq!(command_for(Pause, &paused), None);
        assert_eq!(command_for(Pause, &idle), None);
        assert_eq!(command_for(Next, &idle), Some(Command::NextStation));
        assert_eq!(command_for(Previous, &playing), Some(Command::PrevStation));
    }
}
