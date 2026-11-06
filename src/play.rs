//! `ttnr play <target>`: headless playback with plain status lines on stdout.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc;
use ttnr_core::app::{App, AppState, DEFAULT_HISTORY, Effect, Msg, Status};
use ttnr_core::{NowPlaying, ResolveCache, ResolveCtx, SourceRegistry, Station, StreamInfo};
use ttnr_engine::{EngineConfig, EngineEvent, MpvPlayer, Player};
use ttnr_sources::{BbcSource, RadioBrowserSource, ReqwestHttp, StationsSource, UrlSource};

use crate::runtime::{Runtime, engine_msg};
use crate::{config, paths, stations};

/// Play `target` – a URL, a file path, a station id from `stations.toml`
/// or `radio-browser:<uuid>` – until it ends, fails, or Ctrl-C is pressed.
/// Runs the same state machine as the TUI (ADR-043): playlists fail over to
/// the next stream, a stale URL is re-resolved once, a live stream that
/// drops reconnects with backoff, now-playing is merged by priority and
/// timed by the sync offset. Status lines come from state changes.
pub async fn run(
    target: &str,
    cfg: &EngineConfig,
    media: Option<ttnr_platform::MediaLink>,
) -> Result<()> {
    let station = station_for(target)?;
    let mut player = MpvPlayer::new(cfg).context("cannot start the audio engine")?;
    let mut engine = player
        .take_events()
        .context("engine events already taken")?;
    tracing::info!(station = %station.key(), "play");
    crate::term::say(&format!("ttnr: {}", station.name));

    let (registry, ctx) = setup()?;
    let offsets = config::load().map(|c| c.sync_offsets()).unwrap_or_default();
    // `cfg.volume` is validated to 0–100 by the CLI.
    let volume = cfg.volume.round().clamp(0.0, 100.0) as u8;
    let mut app = App::new(Vec::new(), volume, DEFAULT_HISTORY).with_sync_offsets(offsets);
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Msg>();
    // The control socket (`ttnr now`, `ttnr ctl`) reads the published state.
    let (state_tx, state_rx) = tokio::sync::watch::channel(app.state.clone());
    let _socket_cleanup = crate::runtime::serve_control(state_rx, msg_tx.clone()).await;
    let mut runtime = Runtime::new(Arc::new(registry), ctx, msg_tx);
    let mut printer = Printer::new(&station);

    let (updater, mut remote_rx) = crate::runtime::forward_system_events(media);
    let mut published = None;

    // One future for the whole loop: a new `ctrl_c()` per `select!` misses
    // a signal delivered while another branch wins (issue #7).
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    let mut next = Some(Msg::Play(station));
    loop {
        if let Some(msg) = next.take() {
            if let Some(line) = printer.msg(&msg) {
                crate::term::say(&line);
            }
            let effects = app.update(msg);
            // What happened first ("[…; trying the next stream]"), then
            // what starts ("stream: …").
            for line in printer.state(&app.state) {
                crate::term::say(&line);
            }
            for effect in effects {
                if let Some(line) = printer.effect(&effect) {
                    crate::term::say(&line);
                }
                runtime.perform(effect, &mut player);
            }
            state_tx.send_if_modified(|shown| {
                let changed = *shown != app.state;
                if changed {
                    shown.clone_from(&app.state);
                }
                changed
            });
            if let Some(updater) = &updater {
                let info = crate::media::from_state(&app.state);
                if info != published {
                    updater.update(info.clone());
                    published = info;
                }
            }
            if app.state.quit {
                println!("[stopped]");
                break;
            }
            match &app.state.status {
                Status::Ended => break,
                Status::Error(message) => {
                    runtime.stop_metadata();
                    bail!("{message}");
                }
                _ => {}
            }
        }
        next = tokio::select! {
            event = engine.recv() => {
                let Some(event) = event else {
                    bail!("audio engine stopped unexpectedly");
                };
                tracing::info!(?event, "engine");
                if matches!(event, EngineEvent::Playing) {
                    tracing::info!(
                        ao = player.property_string("current-ao").as_deref().unwrap_or("?"),
                        "audio output"
                    );
                }
                engine_msg(event)
            }
            Some(msg) = msg_rx.recv() => Some(msg),
            Some(event) = remote_rx.recv() => {
                tracing::info!(?event, "system event");
                match event {
                    ttnr_platform::SystemEvent::Woke => Some(Msg::Woke),
                    ttnr_platform::SystemEvent::Remote(remote) => {
                        crate::media::command_for(remote, &app.state).map(Msg::Command)
                    }
                }
            }
            _ = &mut ctrl_c => {
                println!("[stopped]");
                tracing::info!("stopped by Ctrl-C");
                break;
            }
        };
    }
    runtime.stop_metadata();
    state_tx.send_modify(|s| s.quit = true);
    Ok(())
}

/// Turns the app's state changes (and a few effects/messages) into
/// `ttnr play`'s status lines.
#[derive(Debug, Default)]
struct Printer {
    /// The station's own URL (not repeated as `stream:`).
    own_url: Option<String>,
    status: Option<Status>,
    /// Key of the station the last status belonged to.
    station: Option<String>,
    format: String,
    programme: Option<String>,
    track: Option<String>,
}

impl Printer {
    fn new(station: &Station) -> Self {
        Self {
            own_url: station.url.clone(),
            ..Self::default()
        }
    }

    /// `playlist: N streams` when a resolution yields mirrors.
    fn msg(&self, msg: &Msg) -> Option<String> {
        match msg {
            Msg::Resolved { result: Ok(d), .. } | Msg::Refreshed { result: Ok(d), .. }
                if d.urls.len() > 1 =>
            {
                Some(format!("playlist: {} streams", d.urls.len()))
            }
            _ => None,
        }
    }

    /// `stream: <url>` when another URL than the station's own starts.
    fn effect(&self, effect: &Effect) -> Option<String> {
        match effect {
            Effect::Play { url, .. } if self.own_url.as_deref() != Some(url.as_str()) => {
                Some(format!("stream: {url}"))
            }
            _ => None,
        }
    }

    /// Lines for what changed since the last call.
    fn state(&mut self, s: &AppState) -> Vec<String> {
        let mut lines = Vec::new();
        let station = s.station.as_ref().map(Station::key);
        if self.status.as_ref() != Some(&s.status) {
            // A station change is no resume, even from `Paused`.
            let prev = self.status.as_ref().filter(|_| station == self.station);
            if let Some(line) = status_line(prev, &s.status) {
                lines.push(line);
            }
            self.status = Some(s.status.clone());
        }
        self.station = station;
        let format = s
            .stream
            .as_ref()
            .map(StreamInfo::summary)
            .unwrap_or_default();
        if format != self.format {
            if !format.is_empty() {
                lines.push(format!("format: {format}"));
            }
            self.format = format;
        }
        let now = s.now.as_ref();
        let programme = now.and_then(|n| n.programme.clone());
        if programme.is_some() && programme != self.programme {
            lines.push(format!(
                "on air: {}",
                programme.as_deref().unwrap_or_default()
            ));
            self.programme = programme;
        }
        let track = now.and_then(now_line);
        if track.is_some() && track != self.track {
            lines.extend(track.clone());
            self.track = track;
        }
        lines
    }
}

/// The line for a status change; errors are reported by the caller
/// (stderr, exit code).
fn status_line(prev: Option<&Status>, status: &Status) -> Option<String> {
    Some(match status {
        // A live stream resumes by connecting afresh (ADR-048).
        Status::Resolving if prev == Some(&Status::Paused) => "[resumed]".into(),
        Status::Idle | Status::Resolving | Status::Error(_) => return None,
        Status::Loading => "[loading]".into(),
        Status::Playing if prev == Some(&Status::Paused) => "[resumed]".into(),
        Status::Playing => "[playing]".into(),
        Status::Buffering(Some(p)) => format!("[buffering {p}%]"),
        Status::Buffering(None) => "[buffering]".into(),
        Status::Paused => "[paused]".into(),
        Status::Reconnecting(m) => format!("[{m}]"),
        Status::Ended => "[end of stream]".into(),
    })
}

/// A URL or path plays as is; anything else must be a station id.
fn station_for(target: &str) -> Result<Station> {
    if let Some(uuid) = target.strip_prefix("radio-browser:") {
        return Ok(RadioBrowserSource::station(uuid, None));
    }
    if looks_like_url_or_path(target) {
        return Ok(UrlSource::station(target));
    }
    let list = stations::load()?;
    match list.find(target) {
        Some(st) => Ok(st.clone()),
        None if list.stations().is_empty() => bail!(
            "{target:?} is neither a URL nor a station id (no stations configured; see examples/stations.toml)"
        ),
        None => {
            let ids: Vec<_> = list.stations().iter().map(|s| s.id.as_str()).collect();
            bail!("unknown station {target:?}; known: {}", ids.join(", "))
        }
    }
}

fn looks_like_url_or_path(target: &str) -> bool {
    target.contains("://")
        || target.starts_with(['/', '.', '~'])
        || std::path::Path::new(target).exists()
}

/// `ttnr refresh [target]`: re-discover stream URLs (bypassing and updating
/// the cache) for one station / URL, or for every configured station.
pub async fn refresh(target: Option<&str>) -> Result<()> {
    let stations = match target {
        Some(t) => vec![station_for(t)?],
        None => stations::load()?.stations().to_vec(),
    };
    if stations.is_empty() {
        println!("no stations to refresh");
        return Ok(());
    }
    let (registry, ctx) = setup()?;
    let mut failed = 0;
    for st in &stations {
        match registry.refresh(st, &ctx).await {
            Ok(d) => {
                crate::term::say(&format!("{}: {}", st.id, d.urls.join(" | ")));
                crate::term::say(&format!("  {}", d.trace));
            }
            Err(e) => {
                failed += 1;
                crate::term::say(&format!("{}: error: {e}", st.id));
            }
        }
    }
    if failed > 0 {
        bail!(
            "{failed} of {} station(s) could not be resolved",
            stations.len()
        );
    }
    Ok(())
}

/// The source registry and the resolve context (HTTP client, URL cache).
pub fn setup() -> Result<(SourceRegistry, ResolveCtx)> {
    setup_observed().map(|(registry, ctx, _)| (registry, ctx))
}

/// [`setup`] plus the log of the last HTTP request (diagnostics).
pub fn setup_observed() -> Result<(SourceRegistry, ResolveCtx, ttnr_core::HttpLog)> {
    let cache = match paths::cache_dir() {
        Some(dir) => ResolveCache::load(dir.join("resolve.json")),
        None => ResolveCache::in_memory(),
    };
    let http = ReqwestHttp::new().context("cannot create the HTTP client")?;
    let (http, log) = ttnr_core::ObservedHttp::new(Arc::new(http));
    let ctx = ResolveCtx::new(Arc::new(http), Arc::new(cache));
    let mut registry = SourceRegistry::new();
    registry.register(Arc::new(UrlSource));
    registry.register(Arc::new(StationsSource::default()));
    registry.register(Arc::new(BbcSource));
    registry.register(Arc::new(RadioBrowserSource));
    // External-command sources from config.toml (a broken config is
    // reported by whoever loads it for other settings).
    if let Ok(config) = config::load() {
        for source in config.exec_sources(paths::config_dir().as_deref())? {
            registry.register(Arc::new(source));
        }
    }
    Ok((registry, ctx, log))
}

/// `now: Artist – Title` (or just the title).
fn now_line(np: &NowPlaying) -> Option<String> {
    match (&np.artist, &np.title) {
        (Some(artist), Some(title)) => Some(format!("now: {artist} – {title}")),
        (None, Some(title)) => Some(format!("now: {title}")),
        _ => None,
    }
}

/// `ttnr search <query>`: search radio-browser.info and print the stations
/// with a ready-to-paste `stations.toml` entry for each.
pub async fn search(query: &str, limit: usize, source_id: &str) -> Result<()> {
    let (registry, ctx) = setup()?;
    let ids: Vec<_> = registry.ids().collect();
    let source = registry.get(source_id).with_context(|| {
        format!(
            "unknown source `{source_id}` (available: {})",
            ids.join(", ")
        )
    })?;
    let q = ttnr_core::ListQuery {
        search: Some(query.to_owned()),
        limit: Some(limit),
    };
    let list = source
        .list(&q, &ctx)
        .await
        .with_context(|| format!("{source_id} search failed"))?;
    if list.is_empty() {
        crate::term::say(&format!("no stations found for {query:?}"));
        return Ok(());
    }
    for st in &list {
        match st.params.get("detail") {
            Some(detail) => crate::term::say(&format!("{}  {}  [{detail}]", st.id, st.name)),
            None => crate::term::say(&format!("{}  {}", st.id, st.name)),
        }
    }
    println!();
    if source_id == ttnr_sources::radio_browser::ID {
        println!("play: ttnr play radio-browser:<uuid>   ·   or add to stations.toml:");
        if let Some(st) = list.first() {
            println!(
                // `{:?}` escapes control characters from the directory.
                "[[station]]\nid     = \"{}\"\nname   = {:?}\nsource = \"radio-browser\"\nparams = {{ uuid = {:?} }}",
                slug(&st.name),
                st.name,
                st.id
            );
        }
    } else if let Some(st) = list.first() {
        println!("add to stations.toml:");
        println!(
            "[[station]]\nid     = {:?}\nname   = {:?}\nsource = {source_id:?}",
            st.id, st.name
        );
    }
    Ok(())
}

/// A short id from a station name ("SomaFM Groove Salad" → "somafm-groove-salad").
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use ttnr_core::InfoSource;

    #[test]
    fn printer_lines_follow_state_changes() {
        let st = Station::from_url("http://own/stream");
        let mut p = Printer::new(&st);
        let mut s = AppState {
            station: Some(st),
            status: Status::Loading,
            ..AppState::default()
        };
        assert_eq!(p.state(&s), ["[loading]"]);
        assert!(p.state(&s).is_empty(), "no change, no line");
        s.status = Status::Playing;
        s.stream = Some(StreamInfo {
            codec: Some("mp3".into()),
            sample_rate: Some(44_100),
            ..StreamInfo::default()
        });
        assert_eq!(p.state(&s), ["[playing]", "format: MP3 · 44.1 kHz"]);
        s.status = Status::Paused;
        assert_eq!(p.state(&s), ["[paused]"]);
        s.status = Status::Playing;
        assert_eq!(p.state(&s), ["[resumed]"]);
        // Live radio resumes by connecting afresh (ADR-048).
        s.status = Status::Paused;
        p.state(&s);
        s.status = Status::Resolving;
        assert_eq!(p.state(&s), ["[resumed]"]);
        s.status = Status::Playing;
        assert_eq!(p.state(&s), ["[playing]"]);
        // Another station started while paused is not resumed.
        s.status = Status::Paused;
        p.state(&s);
        s.station = Some(Station::from_url("http://other/s"));
        s.status = Status::Resolving;
        assert!(p.state(&s).is_empty(), "{:?}", p.state(&s));
        s.now = Some(NowPlaying {
            programme: Some("Morning".into()),
            artist: Some("A".into()),
            title: Some("T".into()),
            ..NowPlaying::empty(InfoSource::External, SystemTime::UNIX_EPOCH)
        });
        assert_eq!(p.state(&s), ["on air: Morning", "now: A – T"]);
        s.status = Status::Reconnecting("HTTP 404 Not Found; trying the next stream".into());
        assert_eq!(
            p.state(&s),
            ["[HTTP 404 Not Found; trying the next stream]"]
        );
        s.status = Status::Ended;
        assert_eq!(p.state(&s), ["[end of stream]"]);
        assert_eq!(
            p.effect(&Effect::Play {
                url: "http://own/stream".into(),
                headers: vec![]
            }),
            None,
            "own URL not repeated"
        );
        assert_eq!(
            p.effect(&Effect::Play {
                url: "http://mirror".into(),
                headers: vec![]
            })
            .as_deref(),
            Some("stream: http://mirror")
        );
    }

    #[test]
    fn now_lines() {
        let mut np = NowPlaying::empty(InfoSource::Icy, SystemTime::UNIX_EPOCH);
        assert_eq!(now_line(&np), None);
        np.title = Some("Song".into());
        assert_eq!(now_line(&np).as_deref(), Some("now: Song"));
        np.artist = Some("Band".into());
        assert_eq!(now_line(&np).as_deref(), Some("now: Band – Song"));
    }
}
