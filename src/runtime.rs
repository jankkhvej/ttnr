//! Shared runtime of the TUI and `ttnr play`: performs the [`App`]'s
//! effects (player, resolution, metadata providers, search, delays) and maps
//! engine events to app messages (ADR-043).
//!
//! [`App`]: ttnr_core::app::App

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use ttnr_core::app::{Device, Effect, Msg, PlayerEvent};
use ttnr_core::{ResolveCtx, SourceRegistry, StreamDescriptor};
use ttnr_engine::{EngineEvent, MpvPlayer, Player};

/// Performs [`Effect`]s; async work reports back through `msg_tx`.
pub struct Runtime {
    registry: Arc<SourceRegistry>,
    ctx: ResolveCtx,
    msg_tx: mpsc::UnboundedSender<Msg>,
    metadata: Option<JoinHandle<()>>,
}

impl Runtime {
    /// A runtime resolving through `registry`; results arrive on `msg_tx`.
    pub fn new(
        registry: Arc<SourceRegistry>,
        ctx: ResolveCtx,
        msg_tx: mpsc::UnboundedSender<Msg>,
    ) -> Self {
        Self {
            registry,
            ctx,
            msg_tx,
            metadata: None,
        }
    }

    /// Perform one effect (async work reports back as messages).
    pub fn perform(&mut self, effect: Effect, player: &mut MpvPlayer) {
        tracing::debug!(?effect, "effect");
        let result = match effect {
            Effect::Resolve(st) => {
                self.resolve(st, false);
                Ok(())
            }
            Effect::Refresh(st) => {
                self.resolve(st, true);
                Ok(())
            }
            Effect::Play { url, headers } => player.play(&StreamDescriptor {
                urls: vec![url],
                headers,
                ..StreamDescriptor::default()
            }),
            Effect::Stop => player.stop(),
            Effect::Pause(on) => player.pause(on),
            Effect::SetVolume(v) => player.set_volume(f32::from(v)),
            Effect::SetDevice(d) => player.set_device(d.as_deref()),
            Effect::StartMetadata(st) => {
                self.start_metadata(&st);
                Ok(())
            }
            Effect::Search(query) => {
                self.search(query);
                Ok(())
            }
            Effect::Delay { after, msg } => {
                let tx = self.msg_tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(after).await;
                    let _ = tx.send(*msg);
                });
                Ok(())
            }
            // Quitting is handled by the loop.
            Effect::Quit => Ok(()),
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "player command failed");
            let _ = self.msg_tx.send(Msg::Player(PlayerEvent::Failed {
                retryable: false,
                stale_hint: false,
                message: e.to_string(),
            }));
        }
    }

    /// Search the station directory; the answer comes back as a message.
    fn search(&self, query: String) {
        let (registry, ctx, tx) = (self.registry.clone(), self.ctx.clone(), self.msg_tx.clone());
        tokio::spawn(async move {
            let q = ttnr_core::ListQuery {
                search: Some(query.clone()),
                limit: Some(50),
            };
            let result = match registry.get(ttnr_sources::radio_browser::ID) {
                Some(source) => source.list(&q, &ctx).await.map_err(|e| e.to_string()),
                None => Err("the radio-browser source is not available".to_owned()),
            };
            let _ = tx.send(Msg::SearchResults { query, result });
        });
    }

    fn resolve(&self, st: ttnr_core::Station, refresh: bool) {
        let (registry, ctx, tx) = (self.registry.clone(), self.ctx.clone(), self.msg_tx.clone());
        tokio::spawn(async move {
            let key = st.key();
            let result = if refresh {
                registry.refresh(&st, &ctx).await
            } else {
                registry.resolve(&st, &ctx).await
            };
            let result = result.map_err(|e| match e {
                ttnr_core::ResolveError::UnknownSource(id) => format!(
                    "station {:?} needs the `{id}` source, which is not available",
                    st.id
                ),
                other => other.to_string(),
            });
            let msg = if refresh {
                Msg::Refreshed { key, result }
            } else {
                Msg::Resolved { key, result }
            };
            let _ = tx.send(msg);
        });
    }

    fn start_metadata(&mut self, st: &ttnr_core::Station) {
        self.stop_metadata();
        let Some(provider) = self
            .registry
            .get(&st.source)
            .and_then(|s| s.metadata(st, &self.ctx))
        else {
            return;
        };
        let tx = self.msg_tx.clone();
        self.metadata = Some(tokio::spawn(async move {
            let (np_tx, mut np_rx) = mpsc::channel(8);
            // The provider ends once `np_rx` is dropped (task aborted).
            tokio::spawn(async move {
                if let Err(e) = provider.run(np_tx).await {
                    tracing::warn!(error = %e, "metadata provider stopped");
                }
            });
            while let Some(np) = np_rx.recv().await {
                if tx.send(Msg::NowPlaying(np)).is_err() {
                    break;
                }
            }
        }));
    }

    /// Stop the metadata provider, if any.
    pub fn stop_metadata(&mut self) {
        if let Some(task) = self.metadata.take() {
            task.abort();
        }
    }
}

/// Map an engine event to an app message (`None` = not interesting).
pub fn engine_msg(ev: EngineEvent) -> Option<Msg> {
    let player = |p| Some(Msg::Player(p));
    match ev {
        EngineEvent::Loading => player(PlayerEvent::Loading),
        EngineEvent::Playing => player(PlayerEvent::Playing),
        EngineEvent::Buffering { percent } => player(PlayerEvent::Buffering(percent)),
        EngineEvent::Paused(on) => player(PlayerEvent::Paused(on)),
        EngineEvent::StreamInfo(info) => player(PlayerEvent::StreamInfo(info)),
        EngineEvent::Tags(tags) => player(PlayerEvent::Tags(tags)),
        EngineEvent::Eof => player(PlayerEvent::Ended),
        EngineEvent::Error { kind, detail } => player(PlayerEvent::Failed {
            retryable: kind.is_retryable(),
            stale_hint: kind.is_stale_hint(),
            // FFmpeg's detail only repeats an HTTP status; keep it for the
            // rest (e.g. which host was unreachable).
            message: match detail {
                Some(d) if !kind.is_http() => format!("{kind} ({d})"),
                _ => kind.to_string(),
            },
        }),
        EngineEvent::Devices(list) => Some(Msg::Devices(
            list.into_iter()
                .map(|d| Device {
                    name: d.name,
                    description: d.description,
                })
                .collect(),
        )),
        EngineEvent::Shutdown => player(PlayerEvent::Failed {
            retryable: false,
            stale_hint: false,
            message: "audio engine shut down".into(),
        }),
        EngineEvent::Transfer {
            bytes_per_sec,
            buffered_secs,
        } => Some(Msg::Transfer {
            bytes_per_sec,
            buffered_secs,
        }),
        EngineEvent::Stopped => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttnr_engine::ErrorKind;

    #[test]
    fn engine_errors_map_to_failover_hints() {
        let msg = engine_msg(EngineEvent::Error {
            kind: ErrorKind::Http404,
            detail: Some("http: HTTP error 404 Not Found".into()),
        });
        assert_eq!(
            msg,
            Some(Msg::Player(PlayerEvent::Failed {
                retryable: true,
                stale_hint: true,
                message: "HTTP 404 Not Found".into(),
            }))
        );
        let Some(Msg::Player(PlayerEvent::Failed { retryable, .. })) =
            engine_msg(EngineEvent::Error {
                kind: ErrorKind::AudioOutput,
                detail: None,
            })
        else {
            panic!()
        };
        assert!(!retryable);
        assert_eq!(engine_msg(EngineEvent::Stopped), None);
    }
}
