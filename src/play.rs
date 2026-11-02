//! `ttnr play <url>`: headless playback with plain status lines on stdout.

use anyhow::{Context, Result, bail};
use ttnr_core::StreamDescriptor;
use ttnr_engine::{EngineConfig, EngineEvent, MpvPlayer, Player};

/// Play `url` until it ends, fails, or Ctrl-C is pressed.
pub async fn run(url: &str, cfg: &EngineConfig) -> Result<()> {
    let mut player = MpvPlayer::new(cfg).context("cannot start the audio engine")?;
    let mut events = player
        .take_events()
        .context("engine events already taken")?;
    tracing::info!(url, "play");
    println!("ttnr: {url}");
    player.play(&StreamDescriptor::direct(url))?;
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
                    EngineEvent::Eof => return Ok(()),
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
