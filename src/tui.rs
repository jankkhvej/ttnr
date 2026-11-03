//! The interactive TUI: runs the [`App`] state machine, performs its effects
//! (player, resolution, metadata providers) and renders on a separate task
//! that redraws only when the state changes or the terminal is resized.

use std::sync::Arc;

use anyhow::{Context, Result};
use crossterm::cursor::MoveTo;
use crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use futures::StreamExt;
use ratatui::crossterm::event::{Event, EventStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use ttnr_core::app::{App, AppState, Command, DEFAULT_HISTORY, Device, Effect, Msg, PlayerEvent};
use ttnr_core::{ResolveCtx, SourceRegistry, StreamDescriptor};
use ttnr_engine::{EngineConfig, EngineEvent, MpvPlayer, Player};

use crate::{config, paths, play, stations};

/// Run the TUI until the user quits.
pub async fn run(cfg: &EngineConfig) -> Result<()> {
    // Config problems are reported before the terminal is taken over.
    let config = config::load()?;
    let keymap = ttnr_tui::Keymap::with_overrides(&config.key_overrides())
        .context("invalid [keys] in config.toml")?;
    let themes_dir = paths::config_dir().map(|d| d.join("themes"));
    let (themes, theme_errors) = match &themes_dir {
        Some(dir) => ttnr_themes::ThemeSet::load(dir),
        None => (ttnr_themes::ThemeSet::builtin(), Vec::new()),
    };
    let theme_index = match &config.theme {
        None => 0,
        Some(name) => themes.position(name).with_context(|| {
            format!(
                "unknown theme `{name}` in config.toml (available: {})",
                themes.names().join(", ")
            )
        })?,
    };
    let depth = config.color.depth();
    tracing::info!(?depth, themes = ?themes.names(), "themes");
    let (stations, load_error) = match stations::load() {
        Ok(list) => (list.stations().to_vec(), None),
        Err(e) => (Vec::new(), Some(format!("{e:#}"))),
    };
    let volume = cfg.volume.clamp(0.0, 100.0).round() as u8;
    let mut app =
        App::new(stations, volume, DEFAULT_HISTORY).with_sync_offsets(config.sync_offsets());
    app.state.theme_index = theme_index;
    for e in &theme_errors {
        tracing::warn!(error = %e, "skipping theme");
    }
    if let Some(e) = theme_errors.first() {
        app.state.status = ttnr_core::app::Status::Error(e.to_string());
    }
    if let Some(e) = load_error {
        app.state.status = ttnr_core::app::Status::Error(e);
    } else if !app.state.stations.is_empty() {
        app.update(Msg::Command(Command::OpenStations));
    }

    let (registry, ctx) = play::setup()?;
    let registry = Arc::new(registry);
    let mut player = MpvPlayer::new(cfg).context("cannot start the audio engine")?;
    let mut engine = player
        .take_events()
        .context("engine events already taken")?;

    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Msg>();
    let (state_tx, state_rx) = watch::channel(app.state.clone());
    let quadrants = config.mosaic.use_quadrants();
    let resolved: Vec<ttnr_themes::Theme> = (0..themes.len())
        .map(|i| {
            let t = themes.get(i).for_depth(depth);
            if quadrants {
                t.with_mosaic_fallback()
            } else {
                t
            }
        })
        .collect();
    let ui = tokio::spawn(ui_loop(state_rx, msg_tx.clone(), keymap, resolved));
    let mut runtime = Runtime {
        registry,
        ctx,
        msg_tx,
        metadata: None,
    };
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("cannot listen for SIGTERM")?;

    while !app.state.quit {
        let msg = tokio::select! {
            Some(ev) = engine.recv() => match engine_msg(ev) {
                Some(msg) => msg,
                None => continue,
            },
            Some(msg) = msg_rx.recv() => msg,
            _ = sigterm.recv() => Msg::Command(Command::Quit),
            else => break,
        };
        tracing::debug!(?msg, "msg");
        for effect in app.update(msg) {
            runtime.perform(effect, &mut player);
        }
        state_tx.send_if_modified(|shown| {
            let changed = *shown != app.state;
            if changed {
                shown.clone_from(&app.state);
            }
            changed
        });
        if ui.is_finished() {
            break;
        }
    }
    // Make sure the UI sees `quit` even if the loop ended otherwise.
    state_tx.send_modify(|s| s.quit = true);
    runtime.stop_metadata();
    ui.await.context("UI task failed")?
}

/// Performs [`Effect`]s; async work reports back through `msg_tx`.
struct Runtime {
    registry: Arc<SourceRegistry>,
    ctx: ResolveCtx,
    msg_tx: mpsc::UnboundedSender<Msg>,
    metadata: Option<JoinHandle<()>>,
}

impl Runtime {
    fn perform(&mut self, effect: Effect, player: &mut MpvPlayer) {
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
                ttnr_core::ResolveError::UnknownSource(id) => {
                    format!("the `{id}` source is not available yet")
                }
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

    fn stop_metadata(&mut self) {
        if let Some(task) = self.metadata.take() {
            task.abort();
        }
    }
}

/// Map an engine event to an app message (`None` = not interesting).
fn engine_msg(ev: EngineEvent) -> Option<Msg> {
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
            message: match detail {
                Some(d) => format!("{kind} ({d})"),
                None => kind.to_string(),
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
        EngineEvent::Stopped => None,
    }
}

/// The UI task: owns the terminal; redraws when the state changes or the
/// terminal is resized; turns keys into commands. Restores the terminal on
/// every exit path (ratatui's init also installs a panic hook for that).
async fn ui_loop(
    mut state_rx: watch::Receiver<AppState>,
    msg_tx: mpsc::UnboundedSender<Msg>,
    keymap: ttnr_tui::Keymap,
    themes: Vec<ttnr_themes::Theme>,
) -> Result<()> {
    let mut terminal = ratatui::try_init().context("cannot initialise the terminal")?;
    let result = async {
        let mut events = EventStream::new();
        let mut dirty = true;
        let mut printout = ttnr_tui::Printout::new(&themes[0]);
        loop {
            let state = state_rx.borrow_and_update().clone();
            if state.quit {
                return Ok(());
            }
            if dirty {
                let theme = &themes[state.theme_index % themes.len()];
                // Synchronized output (DEC mode 2026): the terminal shows the
                // frame only when complete – no blank flash from ratatui's
                // clear-on-resize. Terminals without it ignore the sequence.
                crossterm::execute!(std::io::stdout(), BeginSynchronizedUpdate)?;
                if theme.chrome.printout {
                    printout.observe(&state, theme);
                }
                terminal.draw(|frame| {
                    ttnr_tui::draw_with_printout(frame, &state, theme, &keymap, printout.lines());
                })?;
                // Park the (hidden) cursor top-left: some terminals
                // (Terminal.app) scroll the screen to keep the cursor visible
                // when the window shrinks, shifting the UI by a line.
                crossterm::execute!(std::io::stdout(), MoveTo(0, 0), EndSynchronizedUpdate)?;
                dirty = false;
            }
            tokio::select! {
                changed = state_rx.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                    dirty = true;
                }
                event = events.next() => match event {
                    Some(Ok(Event::Key(key))) => {
                        if let Some(cmd) = keymap.command_for(key, &state.view) {
                            let _ = msg_tx.send(Msg::Command(cmd));
                        }
                    }
                    Some(Ok(Event::Resize(w, h))) => {
                        // The terminal may have reflowed or scrolled the old
                        // frame (iTerm2 rewraps lines on width changes), so
                        // the diff renderer's idea of the screen is stale:
                        // clear and repaint everything. (`resize` does not
                        // query the cursor, unlike `clear`, which would race
                        // with the input stream.)
                        // Inside a synchronized update (closed by the next
                        // draw) so the clear never shows on its own.
                        crossterm::execute!(std::io::stdout(), BeginSynchronizedUpdate)?;
                        terminal.resize(ratatui::layout::Rect::new(0, 0, w, h))?;
                        dirty = true;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(anyhow::Error::from(e).context("terminal input")),
                    None => return Ok(()),
                },
            }
        }
    }
    .await;
    ratatui::restore();
    result
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
                message: "HTTP 404 Not Found (http: HTTP error 404 Not Found)".into(),
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
