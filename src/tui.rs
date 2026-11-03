//! The interactive TUI: runs the [`App`] state machine, performs its effects
//! (player, resolution, metadata providers) and renders on a separate task
//! that redraws only when the state changes or the terminal is resized.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use crossterm::cursor::MoveTo;
use crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use futures::StreamExt;
use ratatui::crossterm::event::{Event, EventStream};
use tokio::sync::{mpsc, watch};
use ttnr_core::app::{App, AppState, Command, DEFAULT_HISTORY, Msg};
use ttnr_engine::{EngineConfig, MpvPlayer, Player};

use crate::runtime::{Runtime, engine_msg};
use crate::{config, paths, play, stations};

/// Run the TUI until the user quits.
/// `theme` (from `--theme`) overrides `theme` in config.toml.
/// `media` connects to the system's Now Playing / media keys (macOS).
pub async fn run(
    cfg: &EngineConfig,
    theme: Option<&str>,
    media: Option<ttnr_platform::MediaLink>,
) -> Result<()> {
    // Config problems are reported before the terminal is taken over.
    let config = config::load()?;
    let keymap = ttnr_tui::Keymap::with_overrides(&config.key_overrides())
        .context("invalid [keys] in config.toml")?;
    let themes_dir = paths::config_dir().map(|d| d.join("themes"));
    let (themes, theme_errors) = match &themes_dir {
        Some(dir) => ttnr_themes::ThemeSet::load(dir),
        None => (ttnr_themes::ThemeSet::builtin(), Vec::new()),
    };
    let (theme_name, theme_origin) = match theme {
        Some(name) => (Some(name), "--theme"),
        None => (config.theme.as_deref(), "config.toml"),
    };
    let theme_index = match theme_name {
        None => 0,
        Some(name) => themes.position(name).with_context(|| {
            format!(
                "unknown theme `{name}` in {theme_origin} (available: {})",
                themes.names().join(", ")
            )
        })?,
    };
    let depth = config.color.depth();
    tracing::info!(
        terminal = std::env::var("TERM_PROGRAM").as_deref().unwrap_or("?"),
        term = std::env::var("TERM").as_deref().unwrap_or("?"),
        "terminal"
    );
    tracing::info!(?depth, themes = ?themes.names(), "themes");
    let (stations, load_error) = match stations::load() {
        Ok(list) => (list.stations().to_vec(), None),
        Err(e) => (Vec::new(), Some(format!("{e:#}"))),
    };
    let volume = cfg.volume.clamp(0.0, 100.0).round() as u8;
    let mut app =
        App::new(stations, volume, DEFAULT_HISTORY).with_sync_offsets(config.sync_offsets());
    app.state.theme_index = theme_index;
    app.state.baud = config.baud.unwrap_or(0);
    app.state.device.clone_from(&config.output_device);
    app.state.default_device = ttnr_platform::default_output_device();
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
    let exec_ids: Vec<&'static str> = config
        .sources
        .keys()
        .filter_map(|id| registry.ids().find(|r| r == id))
        .collect();
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
    // Control socket for `ttnr now`, `ttnr ctl` and other tools; another
    // running ttnr keeps its socket.
    let socket = crate::control::socket_path();
    let mut _socket_cleanup = None;
    match crate::control::bind(&socket).await {
        Ok(Some(listener)) => {
            _socket_cleanup = Some(crate::control::SocketCleanup(socket.clone()));
            tokio::spawn(crate::control::serve(
                listener,
                socket,
                state_rx.clone(),
                msg_tx.clone(),
            ));
        }
        Ok(None) => {
            tracing::warn!(path = %socket.display(), "another ttnr owns the control socket")
        }
        Err(e) => tracing::warn!(error = %e, "no control socket"),
    }
    let ui_options = UiOptions {
        effects: config.effects_enabled(),
        depth,
        baud: config.baud.unwrap_or(0),
    };
    let ui = tokio::spawn(ui_loop(
        state_rx,
        msg_tx.clone(),
        keymap,
        resolved,
        ui_options,
    ));
    // The UI shows which device "system default" is; CoreAudio tells us
    // when it changes (mpv itself follows it).
    let device_tx = msg_tx.clone();
    let _device_watch = ttnr_platform::watch_output_devices(move || {
        let _ = device_tx.send(Msg::DefaultDevice(ttnr_platform::default_output_device()));
    });
    // Stations of external-command sources join the list when they answer.
    for id in exec_ids {
        let (registry, ctx, tx) = (registry.clone(), ctx.clone(), msg_tx.clone());
        tokio::spawn(async move {
            let Some(source) = registry.get(id) else {
                return;
            };
            match source.list(&ttnr_core::ListQuery::default(), &ctx).await {
                Ok(list) => {
                    let _ = tx.send(Msg::AddStations(list));
                }
                Err(e) => tracing::warn!(source = id, error = %e, "cannot list stations"),
            }
        });
    }
    let mut runtime = Runtime::new(registry, ctx, msg_tx);
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("cannot listen for SIGTERM")?;
    // Media keys arrive on a blocking channel; forward them into the loop.
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
    let mut shown_media = None;

    while !app.state.quit {
        let msg = tokio::select! {
            Some(ev) = engine.recv() => match engine_msg(ev) {
                Some(msg) => msg,
                None => continue,
            },
            Some(msg) = msg_rx.recv() => msg,
            Some(event) = remote_rx.recv() => {
                tracing::info!(?event, "system event");
                match event {
                    ttnr_platform::SystemEvent::Woke => Msg::Woke,
                    ttnr_platform::SystemEvent::Remote(remote) => {
                        match crate::media::command_for(remote, &app.state) {
                            Some(cmd) => Msg::Command(cmd),
                            None => continue,
                        }
                    }
                }
            }
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
        if let Some(updater) = &updater {
            let info = crate::media::from_state(&app.state);
            if info != shown_media {
                updater.update(info.clone());
                shown_media = info;
            }
        }
        if ui.is_finished() {
            break;
        }
    }
    // Make sure the UI sees `quit` even if the loop ended otherwise.
    state_tx.send_modify(|s| s.quit = true);
    runtime.stop_metadata();
    ui.await.context("UI task failed")?
}

/// The UI task: owns the terminal; redraws when the state changes or the
/// terminal is resized; turns keys into commands. Restores the terminal on
/// every exit path (ratatui's init also installs a panic hook for that).
/// UI settings from the config.
struct UiOptions {
    /// Theme animations on/off.
    effects: bool,
    /// Colour depth (animations use fixed machine colours).
    depth: ttnr_themes::ColorDepth,
    /// Simulated line speed for printout themes (0 = instant).
    baud: u32,
}

async fn ui_loop(
    mut state_rx: watch::Receiver<AppState>,
    msg_tx: mpsc::UnboundedSender<Msg>,
    keymap: ttnr_tui::Keymap,
    themes: Vec<ttnr_themes::Theme>,
    options: UiOptions,
) -> Result<()> {
    let mut terminal = ratatui::try_init().context("cannot initialise the terminal")?;
    let result = async {
        let mut events = EventStream::new();
        let mut dirty = true;
        let mut printout = ttnr_tui::Printout::with_baud(&themes[0], options.baud);
        let mut fx = ttnr_tui::Fx::new(options.effects, options.depth);
        let mut last_tick = Instant::now();
        let mut slow = ttnr_tui::SlowScreen::default();
        loop {
            let state = state_rx.borrow_and_update().clone();
            if state.quit {
                return Ok(());
            }
            let theme = &themes[state.theme_index % themes.len()];
            let now = Instant::now();
            fx.observe(&state, theme, now);
            if dirty {
                // Synchronized output (DEC mode 2026): the terminal shows the
                // frame only when complete – no blank flash from ratatui's
                // clear-on-resize. Terminals without it ignore the sequence.
                crossterm::execute!(std::io::stdout(), BeginSynchronizedUpdate)?;
                if theme.chrome.printout {
                    printout.observe(&state, theme);
                }
                let fx_frame = fx.frame(now);
                // Serial terminal themes (not printout ones, which print at
                // their own speed) receive the screen at the line speed.
                let line = theme.chrome.serial && !theme.chrome.printout;
                slow.set_baud(if line { state.baud } else { 0 });
                let booting = fx.booting(now);
                terminal.draw(|frame| {
                    let extras = ttnr_tui::Extras {
                        printout: printout.lines(),
                        fx: fx_frame.map(|f| (&fx, f)),
                    };
                    ttnr_tui::draw_with(frame, &state, theme, &keymap, &extras);
                    if booting {
                        // Boot sequences run at their own pace.
                        if slow.active() {
                            slow.sync(frame.buffer_mut());
                        }
                    } else if let Some(shown) = slow.present(frame.buffer_mut(), now) {
                        *frame.buffer_mut() = shown;
                    }
                })?;
                // Park the (hidden) cursor top-left: some terminals
                // (Terminal.app) scroll the screen to keep the cursor visible
                // when the window shrinks, shifting the UI by a line.
                crossterm::execute!(std::io::stdout(), MoveTo(0, 0), EndSynchronizedUpdate)?;
                dirty = false;
            }
            // Animations and simulated printing tick only while active.
            if !printout.busy() {
                last_tick = Instant::now();
            }
            let wake = [printout.tick(), fx.tick(), slow.tick()]
                .into_iter()
                .flatten()
                .min();
            tokio::select! {
                () = sleep_or_never(wake) => {
                    let now = Instant::now();
                    printout.advance(now.saturating_duration_since(last_tick));
                    last_tick = now;
                    dirty = true;
                }
                changed = state_rx.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                    dirty = true;
                }
                event = events.next() => match event {
                    Some(Ok(Event::Key(key))) => {
                        if fx.booting(Instant::now()) {
                            // Any key ends the boot sequence.
                            fx.skip_boot();
                            dirty = true;
                        }
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

/// Sleep for `d`, or forever when `None`.
async fn sleep_or_never(d: Option<std::time::Duration>) {
    match d {
        Some(d) => tokio::time::sleep(d).await,
        None => std::future::pending().await,
    }
}
