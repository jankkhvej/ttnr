//! `ttnr play <url>`: headless playback with plain status lines on stdout.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use ttnr_core::{ResolveCache, ResolveCtx, SourceRegistry, StreamDescriptor};
use ttnr_engine::{EngineConfig, EngineEvent, ErrorKind, MpvPlayer, Player};
use ttnr_sources::{ReqwestHttp, UrlSource};

use crate::paths;

/// Resolve `url` (expanding playlists), then play it until it ends, fails,
/// or Ctrl-C is pressed. If a playlist lists several streams and one fails,
/// the next is tried.
pub async fn run(url: &str, cfg: &EngineConfig) -> Result<()> {
    let mut player = MpvPlayer::new(cfg).context("cannot start the audio engine")?;
    let mut events = player
        .take_events()
        .context("engine events already taken")?;
    tracing::info!(url, "play");
    println!("ttnr: {url}");

    let resolved = resolve(url).await?;
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

/// Resolve through the source registry (only the `url` source so far).
async fn resolve(url: &str) -> Result<StreamDescriptor> {
    let cache = match paths::cache_dir() {
        Some(dir) => ResolveCache::load(dir.join("resolve.json")),
        None => ResolveCache::in_memory(),
    };
    let http = ReqwestHttp::new().context("cannot create the HTTP client")?;
    let ctx = ResolveCtx::new(Arc::new(http), Arc::new(cache));
    let mut registry = SourceRegistry::new();
    registry.register(Arc::new(UrlSource));
    let d = registry.resolve(&UrlSource::station(url), &ctx).await?;
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
