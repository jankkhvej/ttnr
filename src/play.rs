//! `ttnr play <target>`: headless playback with plain status lines on stdout.

use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc;
use ttnr_core::{
    Action, Failover, Merger, NowPlaying, ResolveCache, ResolveCtx, SourceRegistry, Station,
    StreamDescriptor,
};
use ttnr_engine::{EngineConfig, EngineEvent, MpvPlayer, Player};
use ttnr_sources::{BbcSource, ReqwestHttp, StationsSource, UrlSource};

use crate::{config, paths, stations};

/// Play `target` – a URL, a file path or a station id from
/// `stations.toml` – until it ends, fails, or Ctrl-C is pressed. Playlists
/// are expanded; if one of several streams fails, the next is tried.
/// Now-playing info from the stream (ICY, tags) and from the station's
/// metadata provider is merged by priority.
/// `media` connects to the system's Now Playing / media keys (macOS):
/// play/pause work, next/previous are ignored (one target).
pub async fn run(
    target: &str,
    cfg: &EngineConfig,
    media: Option<ttnr_platform::MediaLink>,
) -> Result<()> {
    let station = station_for(target)?;
    let mut player = MpvPlayer::new(cfg).context("cannot start the audio engine")?;
    let mut events = player
        .take_events()
        .context("engine events already taken")?;
    tracing::info!(station = %station.key(), "play");
    println!("ttnr: {}", station.name);

    let (registry, ctx) = setup()?;
    let resolved = resolve(&registry, &ctx, &station).await?;
    if resolved.urls.len() > 1 {
        println!("playlist: {} streams", resolved.urls.len());
    }
    let own_url = station.url.clone().unwrap_or_default();
    let mut failover = Failover::new(&resolved);
    let start = |player: &mut MpvPlayer, failover: &Failover, url: String| -> Result<()> {
        if url != own_url {
            println!("stream: {url}");
        }
        player.play(&StreamDescriptor {
            urls: vec![url],
            headers: failover.headers().to_vec(),
            ..StreamDescriptor::default()
        })?;
        Ok(())
    };
    let first = failover.next_url().context("nothing to play")?;
    start(&mut player, &failover, first)?;

    // The provider stops by itself when `np_rx` is dropped.
    let (np_tx, mut np_rx) = mpsc::channel::<NowPlaying>(8);
    if let Some(provider) = registry
        .get(&station.source)
        .and_then(|s| s.metadata(&station, &ctx))
    {
        let offset = config::load()
            .map(|c| {
                c.sync_offsets()
                    .get(&station.source)
                    .copied()
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let (raw_tx, mut raw_rx) = mpsc::channel::<NowPlaying>(8);
        tokio::spawn(async move {
            if let Err(e) = provider.run(raw_tx).await {
                tracing::warn!(error = %e, "metadata provider stopped");
            }
        });
        // Show provider updates when they become true on air plus the
        // source's sync offset (same rule as the TUI).
        tokio::spawn(async move {
            while let Some(np) = raw_rx.recv().await {
                let tx = np_tx.clone();
                let wait = (np.at + offset)
                    .duration_since(std::time::SystemTime::now())
                    .unwrap_or_default();
                tokio::spawn(async move {
                    tokio::time::sleep(wait).await;
                    let _ = tx.send(np).await;
                });
            }
        });
    }
    let mut merger = Merger::default();
    let mut programme: Option<String> = None;
    let mut show = |np: NowPlaying, merger: &mut Merger, latest: &mut Option<NowPlaying>| {
        let Some(best) = merger.update(np) else {
            return;
        };
        *latest = Some(best.clone());
        if best.programme.is_some() && best.programme != programme {
            programme.clone_from(&best.programme);
            println!("on air: {}", programme.as_deref().unwrap_or_default());
        }
        if let Some(line) = now_line(best) {
            println!("{line}");
        }
    };

    let mut latest: Option<NowPlaying> = None;
    let mut paused = false;
    let (remote_tx, mut remote_rx) = mpsc::unbounded_channel::<ttnr_platform::SystemEvent>();
    let updater = media.map(|link| {
        let updater = link.updater();
        std::thread::spawn(move || {
            while let Ok(cmd) = link.events.recv() {
                if remote_tx.send(cmd).is_err() {
                    break;
                }
            }
        });
        updater
    });
    let mut published = None;
    loop {
        if let Some(updater) = &updater {
            let info = Some(crate::media::info(
                &station.name,
                latest.as_ref(),
                !paused,
                false,
            ));
            if info != published {
                updater.update(info.clone());
                published = info;
            }
        }
        tokio::select! {
            Some(event) = remote_rx.recv() => {
                tracing::info!(?event, "system event");
                use ttnr_platform::RemoteCommand::{Pause, Play, TogglePause};
                use ttnr_platform::SystemEvent::Remote;
                let want = match event {
                    Remote(TogglePause) => !paused,
                    Remote(Play) => false,
                    Remote(Pause) => true,
                    _ => continue,
                };
                if want != paused {
                    player.pause(want)?;
                }
            }
            event = events.recv() => {
                let Some(event) = event else {
                    bail!("audio engine stopped unexpectedly");
                };
                tracing::info!(?event, "engine");
                if let Some(line) = describe(&event) {
                    println!("{line}");
                }
                match event {
                    EngineEvent::Playing => tracing::info!(
                        ao = player.property_string("current-ao").as_deref().unwrap_or("?"),
                        "audio output"
                    ),
                    EngineEvent::Tags(tags) => {
                        if let Some(np) = NowPlaying::from_tags(&tags, SystemTime::now()) {
                            show(np, &mut merger, &mut latest);
                        }
                    }
                    EngineEvent::Paused(on) => paused = on,
                    EngineEvent::Eof => return Ok(()),
                    EngineEvent::Error { kind, detail } => {
                        let action = if kind.is_retryable() {
                            failover.on_failure(kind.is_stale_hint())
                        } else {
                            Action::GiveUp
                        };
                        let url = match action {
                            Action::Play(url) => {
                                println!("[{kind}; trying the next stream]");
                                url
                            }
                            Action::Refresh => {
                                println!("[{kind}; re-resolving the station]");
                                let fresh = registry.refresh(&station, &ctx).await.with_context(|| {
                                    format!("{kind}, and re-resolving failed")
                                })?;
                                tracing::info!(trace = %fresh.trace, urls = ?fresh.urls, "refreshed");
                                failover.refreshed(&fresh).with_context(|| {
                                    format!("{kind}, and re-resolving found no stream")
                                })?
                            }
                            Action::GiveUp => match detail {
                                Some(detail) => bail!("{kind} ({detail})"),
                                None => bail!("{kind}"),
                            },
                        };
                        merger.clear();
                        latest = None;
                        start(&mut player, &failover, url)?;
                    }
                    EngineEvent::Shutdown => bail!("audio engine shut down"),
                    _ => {}
                }
            }
            Some(np) = np_rx.recv() => {
                tracing::info!(?np, "provider");
                show(np, &mut merger, &mut latest);
            }
            _ = tokio::signal::ctrl_c() => {
                println!("[stopped]");
                tracing::info!("stopped by Ctrl-C");
                return Ok(());
            }
        }
    }
}

/// A URL or path plays as is; anything else must be a station id.
fn station_for(target: &str) -> Result<Station> {
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
                println!("{}: {}", st.id, d.urls.join(" | "));
                println!("  {}", d.trace);
            }
            Err(e) => {
                failed += 1;
                println!("{}: error: {e}", st.id);
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
    let cache = match paths::cache_dir() {
        Some(dir) => ResolveCache::load(dir.join("resolve.json")),
        None => ResolveCache::in_memory(),
    };
    let http = ReqwestHttp::new().context("cannot create the HTTP client")?;
    let ctx = ResolveCtx::new(Arc::new(http), Arc::new(cache));
    let mut registry = SourceRegistry::new();
    registry.register(Arc::new(UrlSource));
    registry.register(Arc::new(StationsSource::default()));
    registry.register(Arc::new(BbcSource));
    Ok((registry, ctx))
}

/// Resolve `station` through its source.
async fn resolve(
    registry: &SourceRegistry,
    ctx: &ResolveCtx,
    station: &Station,
) -> Result<StreamDescriptor> {
    let d = registry.resolve(station, ctx).await.map_err(|e| match e {
        ttnr_core::ResolveError::UnknownSource(id) => anyhow::anyhow!(
            "station {:?} needs the `{id}` source, which is not available yet",
            station.id
        ),
        other => other.into(),
    })?;
    tracing::info!(trace = %d.trace, urls = ?d.urls, "resolved");
    Ok(d)
}

/// `now: Artist – Title` (or just the title).
fn now_line(np: &NowPlaying) -> Option<String> {
    match (&np.artist, &np.title) {
        (Some(artist), Some(title)) => Some(format!("now: {artist} – {title}")),
        (None, Some(title)) => Some(format!("now: {title}")),
        _ => None,
    }
}

/// The status line for an engine event, if it deserves one. Errors are
/// reported by the caller (stderr, exit code); now-playing goes through the
/// merger.
fn describe(event: &EngineEvent) -> Option<String> {
    Some(match event {
        EngineEvent::Loading => "[loading]".into(),
        EngineEvent::Playing => "[playing]".into(),
        EngineEvent::Buffering { percent: Some(p) } => format!("[buffering {p}%]"),
        EngineEvent::Buffering { percent: None } => "[buffering]".into(),
        EngineEvent::Paused(true) => "[paused]".into(),
        EngineEvent::Paused(false) => "[resumed]".into(),
        EngineEvent::StreamInfo(info) => {
            let summary = info.summary();
            if summary.is_empty() {
                return None;
            }
            format!("format: {summary}")
        }
        EngineEvent::Eof => "[end of stream]".into(),
        EngineEvent::Tags(_)
        | EngineEvent::Devices(_)
        | EngineEvent::Stopped
        | EngineEvent::Error { .. }
        | EngineEvent::Shutdown => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttnr_core::{InfoSource, StreamInfo};
    use ttnr_engine::ErrorKind;

    #[test]
    fn status_lines() {
        assert_eq!(
            describe(&EngineEvent::Loading).as_deref(),
            Some("[loading]")
        );
        assert_eq!(
            describe(&EngineEvent::Buffering { percent: Some(40) }).as_deref(),
            Some("[buffering 40%]")
        );
        assert_eq!(
            describe(&EngineEvent::Eof).as_deref(),
            Some("[end of stream]")
        );
        let info = StreamInfo {
            codec: Some("mp3".into()),
            sample_rate: Some(44_100),
            ..StreamInfo::default()
        };
        assert_eq!(
            describe(&EngineEvent::StreamInfo(info)).as_deref(),
            Some("format: MP3 · 44.1 kHz")
        );
    }

    #[test]
    fn silent_events() {
        assert_eq!(
            describe(&EngineEvent::StreamInfo(StreamInfo::default())),
            None
        );
        assert_eq!(describe(&EngineEvent::Stopped), None);
        let err = EngineEvent::Error {
            kind: ErrorKind::Http403,
            detail: None,
        };
        assert_eq!(describe(&err), None);
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
