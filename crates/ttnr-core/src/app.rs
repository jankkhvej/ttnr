//! The application state machine (Elm-style): [`App::update`] takes a
//! [`Msg`], changes [`AppState`] and returns [`Effect`]s for the runtime to
//! perform (play, resolve, …). Pure and synchronous, so the TUI and the CLI
//! share it and it is tested without audio, network or a terminal (ADR-022).

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, SystemTime};

use crate::{
    Action, Failover, Merger, NowPlaying, Station, StreamDescriptor, StreamInfo, StreamTags,
};

/// Volume change per key press, in percent.
pub const VOLUME_STEP: u8 = 5;
/// Default number of recent tracks kept (SPEC F4.4).
pub const DEFAULT_HISTORY: usize = 5;
/// Sync-offset change per key press.
pub const SYNC_STEP: Duration = Duration::from_secs(1);
/// Largest sync offset.
pub const MAX_SYNC: Duration = Duration::from_secs(120);

/// Playback status shown to the user (SPEC F5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Status {
    /// Nothing selected yet.
    #[default]
    Idle,
    /// Looking up stream URLs.
    Resolving,
    /// Opening the stream.
    Loading,
    /// Waiting for network data.
    Buffering(Option<u8>),
    /// Audio is playing.
    Playing,
    /// Paused by the user.
    Paused,
    /// Recovering: next mirror or re-resolving (reason).
    Reconnecting(String),
    /// The stream ended.
    Ended,
    /// Failed (message).
    Error(String),
}

/// An audio output device (from the player's device list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Player-specific name used to select it.
    pub name: String,
    /// Human-readable description.
    pub description: String,
}

/// Which screen is shown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum View {
    /// Now playing.
    #[default]
    Main,
    /// Station list with the highlighted row.
    Stations {
        /// Highlighted index.
        selected: usize,
    },
    /// "Open URL" prompt with the text typed so far.
    OpenUrl {
        /// Input text.
        input: String,
    },
    /// Directory search prompt with the text typed so far.
    Search {
        /// Input text.
        input: String,
    },
    /// Directory search results with the highlighted row.
    Results {
        /// Highlighted index.
        selected: usize,
    },
    /// Key help overlay.
    Help,
    /// Output device picker; index 0 = "system default".
    Devices {
        /// Highlighted index.
        selected: usize,
    },
}

/// The last directory search (radio-browser).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchState {
    /// What was searched for.
    pub query: String,
    /// Stations found.
    pub results: Vec<Station>,
    /// "searching…", "no stations found", or an error.
    pub note: Option<String>,
}

/// Everything the UI shows. Plain data: compared to decide redraws.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppState {
    /// The selected station.
    pub station: Option<Station>,
    /// Playback status.
    pub status: Status,
    /// What is playing now (merged from all providers).
    pub now: Option<NowPlaying>,
    /// Earlier tracks, most recent first.
    pub history: VecDeque<NowPlaying>,
    /// Actual stream format.
    pub stream: Option<StreamInfo>,
    /// App volume 0–100 (independent of the system volume).
    pub volume: u8,
    /// Muted (volume kept for unmute).
    pub muted: bool,
    /// Pinned output device (mpv name, or a description from config);
    /// `None` = follow the system default. Used while it is in `devices`,
    /// otherwise the system default plays until it returns.
    pub device: Option<String>,
    /// The device actually requested from the player (`None` = default).
    pub output: Option<String>,
    /// The system default output device's name, if known.
    pub default_device: Option<String>,
    /// Known output devices.
    pub devices: Vec<Device>,
    /// Stations for the list and next/previous.
    pub stations: Vec<Station>,
    /// Last directory search: query, results, and a note while searching
    /// or when nothing / an error came back.
    pub search: SearchState,
    /// Current screen.
    pub view: View,
    /// Theme selection, an index into the UI's theme list (wraps around).
    pub theme_index: usize,
    /// Simulated serial line speed for terminal-like themes (bits/s,
    /// 0 = instant); cycled through [`BAUD_STEPS`].
    pub baud: u32,
    /// Delay applied to now-playing from external providers of the
    /// current station's source, to match the audio (SPEC F4.3).
    pub sync_offset: Duration,
    /// Set when the app should exit.
    pub quit: bool,
}

/// Line speeds the speed key cycles through: instant, then common serial
/// rates from fast to slow (110 = Teletype Model 33).
pub const BAUD_STEPS: [u32; 5] = [0, 9600, 1200, 300, 110];

impl AppState {
    /// Index in `devices` of the pinned device (by mpv name or description).
    pub fn pinned_index(&self) -> Option<usize> {
        let pin = self.device.as_ref()?;
        self.devices
            .iter()
            .position(|d| &d.name == pin || &d.description == pin)
    }

    /// The device to request from the player: the pinned one while it is
    /// present, else `None` (system default).
    pub fn pinned_output(&self) -> Option<String> {
        self.pinned_index().map(|i| self.devices[i].name.clone())
    }
}

/// What the player reported (the engine's events, mapped by the runtime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    /// Opening a stream.
    Loading,
    /// Audio is playing.
    Playing,
    /// Waiting for data.
    Buffering(Option<u8>),
    /// Pause state changed.
    Paused(bool),
    /// Stream format known/changed.
    StreamInfo(StreamInfo),
    /// In-stream tags changed.
    Tags(StreamTags),
    /// The stream ended.
    Ended,
    /// The stream failed.
    Failed {
        /// Another URL may work (HTTP errors, unreachable, load failure).
        retryable: bool,
        /// The URL itself is probably stale (403/404/unreachable).
        stale_hint: bool,
        /// For display.
        message: String,
    },
}

/// A user command (after the keymap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Play/pause.
    TogglePause,
    /// Louder.
    VolumeUp,
    /// Quieter.
    VolumeDown,
    /// Mute/unmute.
    ToggleMute,
    /// Next station in the list.
    NextStation,
    /// Previous station in the list.
    PrevStation,
    /// Show the station list.
    OpenStations,
    /// Show the open-URL prompt.
    OpenUrl,
    /// Show the directory search prompt.
    OpenSearch,
    /// Show the device picker.
    OpenDevices,
    /// Toggle the help overlay.
    ToggleHelp,
    /// Next theme.
    CycleTheme,
    /// Next simulated line speed (terminal-like themes).
    CycleBaud,
    /// Show external now-playing later (+1 s).
    SyncLater,
    /// Show external now-playing earlier (−1 s, not below 0).
    SyncEarlier,
    /// Exit.
    Quit,
    /// Move the highlight up.
    Up,
    /// Move the highlight down.
    Down,
    /// Confirm.
    Select,
    /// Close the current view.
    Back,
    /// Text input.
    Input(char),
    /// Delete the last input character.
    Backspace,
}

/// Input to [`App::update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// Start playing this station (CLI target, list selection, …).
    Play(Station),
    /// A player event.
    Player(PlayerEvent),
    /// Resolution finished for the station with this key.
    Resolved {
        /// [`Station::key`].
        key: String,
        /// URLs or an error message.
        result: Result<StreamDescriptor, String>,
    },
    /// A forced re-resolution finished.
    Refreshed {
        /// [`Station::key`].
        key: String,
        /// URLs or an error message.
        result: Result<StreamDescriptor, String>,
    },
    /// Now-playing from a metadata provider (subject to the sync offset).
    NowPlaying(NowPlaying),
    /// Time to re-check the pending now-playing update (scheduled by
    /// [`Effect::Delay`]); stale generations are ignored.
    NowPlayingDue {
        /// [`Station::key`] it belongs to.
        key: String,
        /// Pending-update generation it was scheduled for.
        generation: u64,
    },
    /// The output device list changed.
    Devices(Vec<Device>),
    /// The system default output device changed (its name).
    DefaultDevice(Option<String>),
    /// Time for a scheduled reconnection attempt (stale ones are ignored).
    Retry {
        /// [`Station::key`] it belongs to.
        key: String,
        /// Reconnection generation it was scheduled for.
        generation: u64,
    },
    /// The computer woke from sleep: reconnect a live stream at once.
    Woke,
    /// A directory search finished.
    SearchResults {
        /// The query it answers.
        query: String,
        /// Stations or an error message.
        result: Result<Vec<Station>, String>,
    },
    /// A user command.
    Command(Command),
}

/// Work for the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Resolve the station (cache first).
    Resolve(Station),
    /// Re-resolve the station, bypassing the cache.
    Refresh(Station),
    /// Play this URL with these headers.
    Play {
        /// Stream URL.
        url: String,
        /// HTTP headers.
        headers: Vec<(String, String)>,
    },
    /// Stop playback.
    Stop,
    /// Pause (`true`) / resume.
    Pause(bool),
    /// Set the player volume (0–100).
    SetVolume(u8),
    /// Pin a device (`None` = system default).
    SetDevice(Option<String>),
    /// Start the station's metadata provider (stop any previous one).
    StartMetadata(Station),
    /// Search the station directory for this text.
    Search(String),
    /// Deliver `msg` back to [`App::update`] after `after`.
    Delay {
        /// How long to wait.
        after: Duration,
        /// What to deliver.
        msg: Box<Msg>,
    },
    /// Exit.
    Quit,
}

/// The state machine.
#[derive(Debug)]
pub struct App {
    /// What the UI shows.
    pub state: AppState,
    failover: Option<Failover>,
    merger: Merger,
    history_len: usize,
    sync_offsets: BTreeMap<String, Duration>,
    /// Latest provider update not shown yet, and its generation.
    pending: Option<NowPlaying>,
    generation: u64,
    /// The current station has played since it was chosen (only then do
    /// failures reconnect instead of ending in an error).
    played: bool,
    /// Reconnection attempts since the last successful start.
    retry_attempt: u32,
    /// Reconnection generation (stale `Msg::Retry` are ignored).
    retry_gen: u64,
}

/// Waits between reconnection attempts; the last repeats forever.
const BACKOFF_SECS: [u64; 7] = [1, 2, 4, 8, 16, 30, 60];

impl App {
    /// A new app with `stations`, initial `volume` and history length.
    pub fn new(stations: Vec<Station>, volume: u8, history_len: usize) -> Self {
        Self {
            state: AppState {
                stations,
                volume: volume.min(100),
                ..AppState::default()
            },
            failover: None,
            merger: Merger::default(),
            history_len,
            sync_offsets: BTreeMap::new(),
            pending: None,
            generation: 0,
            played: false,
            retry_attempt: 0,
            retry_gen: 0,
        }
    }

    /// Per-source sync offsets (source id → delay), e.g. from config.
    pub fn with_sync_offsets(mut self, offsets: BTreeMap<String, Duration>) -> Self {
        self.sync_offsets = offsets;
        self
    }

    /// Apply `msg`; returns the effects to perform, in order.
    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Play(st) => self.play(st),
            Msg::Player(ev) => self.on_player(ev),
            Msg::Resolved { key, result } => self.on_resolved(&key, result, false),
            Msg::Refreshed { key, result } => self.on_resolved(&key, result, true),
            Msg::NowPlaying(np) => {
                if self.current_key().is_none() {
                    return vec![];
                }
                // Only the newest provider update matters (it describes now).
                self.pending = Some(np);
                self.generation += 1;
                self.check_pending()
            }
            Msg::NowPlayingDue { key, generation } => {
                if self.current_key().as_deref() != Some(key.as_str())
                    || generation != self.generation
                {
                    return vec![];
                }
                self.check_pending()
            }
            Msg::Devices(devices) => {
                self.state.devices = devices;
                // A pinned device that appeared or vanished changes output.
                let output = self.state.pinned_output();
                if output == self.state.output {
                    return vec![];
                }
                self.state.output.clone_from(&output);
                vec![Effect::SetDevice(output)]
            }
            Msg::DefaultDevice(name) => {
                self.state.default_device = name;
                vec![]
            }
            Msg::Retry { key, generation } => {
                if generation != self.retry_gen
                    || self.current_key().as_deref() != Some(key.as_str())
                    || !matches!(self.state.status, Status::Reconnecting(_))
                {
                    return vec![];
                }
                self.reconnect_now("reconnecting")
            }
            Msg::SearchResults { query, result } => {
                if query != self.state.search.query {
                    return vec![]; // an older search
                }
                let search = &mut self.state.search;
                match result {
                    Ok(list) => {
                        search.note = list.is_empty().then(|| "no stations found".to_owned());
                        search.results = list;
                    }
                    Err(e) => {
                        search.results.clear();
                        search.note = Some(e);
                    }
                }
                vec![]
            }
            Msg::Woke => {
                let live = matches!(
                    self.state.status,
                    Status::Playing
                        | Status::Buffering(_)
                        | Status::Loading
                        | Status::Reconnecting(_)
                );
                if !(self.played && live) {
                    return vec![];
                }
                self.retry_attempt = 0;
                self.reconnect_now("woke up; reconnecting")
            }
            Msg::Command(cmd) => self.on_command(cmd),
        }
    }

    /// Show the pending update if its time has come (`at` + sync offset),
    /// otherwise schedule a re-check for the remaining time.
    fn check_pending(&mut self) -> Vec<Effect> {
        let Some(np) = &self.pending else {
            return vec![];
        };
        let due = np.at + self.state.sync_offset;
        match due.duration_since(SystemTime::now()) {
            // Not yet (allow 50 ms of timer slack).
            Ok(left) if left > Duration::from_millis(50) => {
                let Some(key) = self.current_key() else {
                    return vec![];
                };
                vec![Effect::Delay {
                    after: left,
                    msg: Box::new(Msg::NowPlayingDue {
                        key,
                        generation: self.generation,
                    }),
                }]
            }
            _ => {
                if let Some(np) = self.pending.take() {
                    self.show_now(np);
                }
                vec![]
            }
        }
    }

    fn current_key(&self) -> Option<String> {
        self.state.station.as_ref().map(Station::key)
    }

    fn play(&mut self, st: Station) -> Vec<Effect> {
        self.state.sync_offset = self
            .sync_offsets
            .get(&st.source)
            .copied()
            .unwrap_or_default();
        self.pending = None;
        self.generation += 1;
        self.state.station = Some(st.clone());
        self.state.status = Status::Resolving;
        self.state.now = None;
        self.state.stream = None;
        self.state.view = View::Main;
        self.failover = None;
        self.merger.clear();
        self.played = false;
        self.retry_attempt = 0;
        self.retry_gen += 1;
        vec![Effect::Stop, Effect::Resolve(st)]
    }

    /// Schedule the next reconnection attempt with backoff.
    fn schedule_retry(&mut self, why: &str) -> Vec<Effect> {
        let Some(key) = self.current_key() else {
            self.state.status = Status::Error(why.to_owned());
            return vec![];
        };
        let i = usize::try_from(self.retry_attempt)
            .unwrap_or(usize::MAX)
            .min(BACKOFF_SECS.len() - 1);
        let secs = BACKOFF_SECS[i];
        self.retry_attempt = self.retry_attempt.saturating_add(1);
        self.retry_gen += 1;
        self.state.status = Status::Reconnecting(format!("{why}; retry in {secs} s"));
        vec![
            Effect::Stop,
            Effect::Delay {
                after: Duration::from_secs(secs),
                msg: Box::new(Msg::Retry {
                    key,
                    generation: self.retry_gen,
                }),
            },
        ]
    }

    /// Resolve the current station again and play it (fresh failover).
    fn reconnect_now(&mut self, why: &str) -> Vec<Effect> {
        let Some(st) = self.state.station.clone() else {
            return vec![];
        };
        self.retry_gen += 1;
        self.failover = None;
        self.state.status = Status::Reconnecting(why.to_owned());
        vec![Effect::Stop, Effect::Resolve(st)]
    }

    fn start(&mut self, url: String) -> Vec<Effect> {
        let headers = self
            .failover
            .as_ref()
            .map(|f| f.headers().to_vec())
            .unwrap_or_default();
        self.state.stream = None;
        self.merger.clear();
        self.state.now = None;
        vec![Effect::Play { url, headers }]
    }

    fn on_resolved(
        &mut self,
        key: &str,
        result: Result<StreamDescriptor, String>,
        refresh: bool,
    ) -> Vec<Effect> {
        if self.current_key().as_deref() != Some(key) {
            return vec![]; // a late answer for a station we left
        }
        let d = match result {
            Ok(d) => d,
            Err(e) if self.played && !refresh => return self.schedule_retry(&e),
            Err(e) => {
                self.state.status = Status::Error(e);
                return vec![];
            }
        };
        let url = if refresh {
            self.failover.as_mut().and_then(|f| f.refreshed(&d))
        } else {
            let mut f = Failover::new(&d);
            let url = f.next_url();
            self.failover = Some(f);
            url
        };
        let Some(url) = url else {
            self.state.status = Status::Error("no stream found".into());
            return vec![];
        };
        self.state.status = Status::Loading;
        let mut effects = self.start(url);
        if !refresh && let Some(st) = self.state.station.clone() {
            effects.push(Effect::StartMetadata(st));
        }
        effects
    }

    fn on_player(&mut self, ev: PlayerEvent) -> Vec<Effect> {
        match ev {
            PlayerEvent::Loading => self.state.status = Status::Loading,
            PlayerEvent::Playing => {
                self.state.status = Status::Playing;
                self.played = true;
                self.retry_attempt = 0;
            }
            PlayerEvent::Buffering(p) => self.state.status = Status::Buffering(p),
            PlayerEvent::Paused(true) => self.state.status = Status::Paused,
            PlayerEvent::Paused(false) => {
                if self.state.status == Status::Paused {
                    self.state.status = Status::Playing;
                }
            }
            PlayerEvent::StreamInfo(info) => self.state.stream = Some(info),
            PlayerEvent::Tags(tags) => {
                if let Some(np) = NowPlaying::from_tags(&tags, SystemTime::now()) {
                    self.show_now(np);
                }
            }
            PlayerEvent::Ended => {
                // A file ends; a live stream (not seekable) that "ends" lost
                // its connection.
                let file = self.state.stream.as_ref().and_then(|i| i.seekable) == Some(true);
                if self.played && !file {
                    return self.schedule_retry("stream ended");
                }
                self.state.status = Status::Ended;
            }
            PlayerEvent::Failed {
                retryable,
                stale_hint,
                message,
            } => {
                let action = match (&mut self.failover, retryable) {
                    (Some(f), true) => f.on_failure(stale_hint),
                    _ => Action::GiveUp,
                };
                return match action {
                    Action::Play(url) => {
                        self.state.status = Status::Reconnecting(format!("{message}; next stream"));
                        self.start(url)
                    }
                    Action::Refresh => {
                        self.state.status =
                            Status::Reconnecting(format!("{message}; re-resolving"));
                        self.state
                            .station
                            .clone()
                            .map(Effect::Refresh)
                            .into_iter()
                            .collect()
                    }
                    Action::GiveUp if retryable && self.played => self.schedule_retry(&message),
                    Action::GiveUp => {
                        self.state.status = Status::Error(message);
                        vec![]
                    }
                };
            }
        }
        vec![]
    }

    /// Merge `np`; if the shown track changes, the old one goes to history.
    fn show_now(&mut self, np: NowPlaying) {
        let Some(best) = self.merger.update(np).cloned() else {
            return;
        };
        if let Some(old) = self.state.now.take()
            && (old.artist != best.artist || old.title != best.title)
            && self.history_len > 0
        {
            self.state.history.push_front(old);
            self.state.history.truncate(self.history_len);
        }
        self.state.now = Some(best);
    }

    fn effective_volume(&self) -> u8 {
        if self.state.muted {
            0
        } else {
            self.state.volume
        }
    }

    fn on_command(&mut self, cmd: Command) -> Vec<Effect> {
        // View-specific keys first.
        match (&mut self.state.view, &cmd) {
            (View::OpenUrl { input }, Command::Input(c)) => {
                input.push(*c);
                return vec![];
            }
            (View::OpenUrl { input }, Command::Backspace) => {
                input.pop();
                return vec![];
            }
            (View::OpenUrl { input }, Command::Select) => {
                let url = input.trim().to_owned();
                if url.is_empty() {
                    self.state.view = View::Main;
                    return vec![];
                }
                return self.play(Station::from_url(&url));
            }
            (View::Search { input }, Command::Input(c)) => {
                input.push(*c);
                return vec![];
            }
            (View::Search { input }, Command::Backspace) => {
                input.pop();
                return vec![];
            }
            (View::Search { input }, Command::Select) => {
                let query = input.trim().to_owned();
                if query.is_empty() {
                    self.state.view = View::Main;
                    return vec![];
                }
                self.state.search = SearchState {
                    query: query.clone(),
                    results: Vec::new(),
                    note: Some("searching…".to_owned()),
                };
                self.state.view = View::Results { selected: 0 };
                return vec![Effect::Search(query)];
            }
            (View::Results { selected }, Command::Down) => {
                let max = self.state.search.results.len().saturating_sub(1);
                *selected = (*selected + 1).min(max);
                return vec![];
            }
            (View::Results { selected }, Command::Select) => {
                let st = self.state.search.results.get(*selected).cloned();
                return match st {
                    Some(st) => self.play(st),
                    None => vec![],
                };
            }
            (
                View::Stations { selected }
                | View::Devices { selected }
                | View::Results { selected },
                Command::Up,
            ) => {
                *selected = selected.saturating_sub(1);
                return vec![];
            }
            (View::Stations { selected }, Command::Down) => {
                let max = self.state.stations.len().saturating_sub(1);
                *selected = (*selected + 1).min(max);
                return vec![];
            }
            (View::Devices { selected }, Command::Down) => {
                *selected = (*selected + 1).min(self.state.devices.len());
                return vec![];
            }
            (View::Stations { selected }, Command::Select) => {
                let st = self.state.stations.get(*selected).cloned();
                return match st {
                    Some(st) => self.play(st),
                    None => vec![],
                };
            }
            (View::Devices { selected }, Command::Select) => {
                let device = selected
                    .checked_sub(1)
                    .and_then(|i| self.state.devices.get(i))
                    .map(|d| d.name.clone());
                self.state.device = device;
                self.state.view = View::Main;
                let output = self.state.pinned_output();
                self.state.output.clone_from(&output);
                return vec![Effect::SetDevice(output)];
            }
            (View::Main, Command::Back) => return vec![],
            (_, Command::Back) => {
                self.state.view = View::Main;
                return vec![];
            }
            (View::OpenUrl { .. } | View::Search { .. }, _) => return vec![], // other keys are text
            _ => {}
        }
        match cmd {
            Command::TogglePause => match self.state.status {
                Status::Playing | Status::Buffering(_) => vec![Effect::Pause(true)],
                Status::Paused => vec![Effect::Pause(false)],
                _ => vec![],
            },
            Command::VolumeUp | Command::VolumeDown => {
                let v = self.state.volume;
                self.state.volume = if cmd == Command::VolumeUp {
                    v.saturating_add(VOLUME_STEP).min(100)
                } else {
                    v.saturating_sub(VOLUME_STEP)
                };
                self.state.muted = false;
                vec![Effect::SetVolume(self.effective_volume())]
            }
            Command::ToggleMute => {
                self.state.muted = !self.state.muted;
                vec![Effect::SetVolume(self.effective_volume())]
            }
            Command::NextStation | Command::PrevStation => {
                let n = self.state.stations.len();
                if n == 0 {
                    return vec![];
                }
                let current = self
                    .state
                    .station
                    .as_ref()
                    .and_then(|s| self.state.stations.iter().position(|x| x.key() == s.key()));
                let next = match (current, cmd == Command::NextStation) {
                    (Some(i), true) => (i + 1) % n,
                    (Some(i), false) => (i + n - 1) % n,
                    (None, true) => 0,
                    (None, false) => n - 1,
                };
                let st = self.state.stations[next].clone();
                self.play(st)
            }
            Command::OpenStations => {
                let selected = self
                    .state
                    .station
                    .as_ref()
                    .and_then(|s| self.state.stations.iter().position(|x| x.key() == s.key()))
                    .unwrap_or(0);
                self.state.view = View::Stations { selected };
                vec![]
            }
            Command::OpenSearch => {
                self.state.view = View::Search {
                    input: self.state.search.query.clone(),
                };
                vec![]
            }
            Command::OpenUrl => {
                self.state.view = View::OpenUrl {
                    input: String::new(),
                };
                vec![]
            }
            Command::OpenDevices => {
                let selected = self.state.pinned_index().map_or(0, |i| i + 1);
                self.state.view = View::Devices { selected };
                vec![]
            }
            Command::ToggleHelp => {
                self.state.view = if self.state.view == View::Help {
                    View::Main
                } else {
                    View::Help
                };
                vec![]
            }
            Command::SyncLater | Command::SyncEarlier => {
                let now = self.state.sync_offset;
                let next = if cmd == Command::SyncLater {
                    (now + SYNC_STEP).min(MAX_SYNC)
                } else {
                    now.saturating_sub(SYNC_STEP)
                };
                self.state.sync_offset = next;
                if let Some(st) = &self.state.station {
                    self.sync_offsets.insert(st.source.clone(), next);
                }
                // Re-time the pending update with the new offset.
                self.generation += 1;
                self.check_pending()
            }
            Command::CycleTheme => {
                self.state.theme_index = self.state.theme_index.wrapping_add(1);
                vec![]
            }
            Command::CycleBaud => {
                let i = BAUD_STEPS.iter().position(|b| *b == self.state.baud);
                self.state.baud = BAUD_STEPS[i.map_or(1, |i| (i + 1) % BAUD_STEPS.len())];
                vec![]
            }
            Command::Quit => {
                self.state.quit = true;
                vec![Effect::Stop, Effect::Quit]
            }
            Command::Up
            | Command::Down
            | Command::Select
            | Command::Back
            | Command::Input(_)
            | Command::Backspace => vec![],
        }
    }
}
