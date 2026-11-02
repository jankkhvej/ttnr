//! `ttnr play <url>`: headless playback with plain status lines on stdout.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use ttnr_core::{ResolveCache, ResolveCtx, SourceRegistry, Station, StreamDescriptor};
use ttnr_engine::{EngineConfig, EngineEvent, ErrorKind, MpvPlayer, Player};
use ttnr_sources::{ReqwestHttp, StationsSource, UrlSource};

use crate::{paths, stations};

/// Play `target` – a URL, a file path or a station id from
/// `stations.toml` – until it ends, fails, or Ctrl-C is pressed. Playlists
/// are expanded; if one of several streams fails, the next is tried.
pub async fn run(target: &str, cfg: &EngineConfig) -> Result<()> {
    let station = station_for(target)?;
    let url = station.url.clone().unwrap_or_default();
    let url = url.as_str();
    let mut player = MpvPlayer::new(cfg).context("cannot start the audio engine")?;
    let mut events = player
        .take_events()
        .context("engine events already taken")?;
    tracing::info!(station = %station.key(), "play");
    println!("ttnr: {}", station.name);

    let resolved = resolve(&station).await?;
    if resolved.urls.len() > 1 {
        println!("playlist: {} streams", resolved.urls.len());
    }
    let mut candidates = resolved.urls.iter();
    let mut play_next = |player: &mut MpvPlayer| -> Result<bool> {
        let Some(next) = candidates.next() else {
            return Ok(false);
        };
        if next != url {
            println!("stream: {next}");
        }
        player.play(&StreamDescriptor {
            urls: vec![next.clone()],
            headers: resolved.headers.clone(),
            ..StreamDescriptor::default()
        })?;
        Ok(true)
    };
    play_next(&mut player)?;

    loop {
        tokio::select! {
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
                    EngineEvent::Eof => return Ok(()),
                    EngineEvent::Error { kind, .. } if is_stream_failure(kind) && play_next(&mut player)? => {
                        println!("[{kind}; trying the next stream]");
                    }
                    EngineEvent::Error { kind, detail } => match detail {
                        Some(detail) => bail!("{kind} ({detail})"),
                        None => bail!("{kind}"),
                    },
                    EngineEvent::Shutdown => bail!("audio engine shut down"),
                    _ => {}
                }
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

/// Resolve through the source registry.
async fn resolve(station: &Station) -> Result<StreamDescriptor> {
    let cache = match paths::cache_dir() {
        Some(dir) => ResolveCache::load(dir.join("resolve.json")),
        None => ResolveCache::in_memory(),
    };
    let http = ReqwestHttp::new().context("cannot create the HTTP client")?;
    let ctx = ResolveCtx::new(Arc::new(http), Arc::new(cache));
    let mut registry = SourceRegistry::new();
    registry.register(Arc::new(UrlSource));
    registry.register(Arc::new(StationsSource::default()));
    let d = registry.resolve(station, &ctx).await.map_err(|e| match e {
        ttnr_core::ResolveError::UnknownSource(id) => {
            anyhow::anyhow!(
                "station {:?} needs the `{id}` source, which is not available yet",
                station.id
            )
        }
        other => other.into(),
    })?;
    tracing::info!(trace = %d.trace, urls = ?d.urls, "resolved");
    Ok(d)
}

/// Failures where another mirror may well work.
fn is_stream_failure(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::Http403
            | ErrorKind::Http404
            | ErrorKind::Http(_)
            | ErrorKind::Unreachable
            | ErrorKind::LoadFailed
    )
}

/// The status line for an engine event, if it deserves one. Errors are
/// reported by the caller (stderr, exit code).
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
        EngineEvent::Stopped | EngineEvent::Error { .. } | EngineEvent::Shutdown => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttnr_core::StreamInfo;
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
}
