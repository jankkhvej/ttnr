//! `ttnr` – minimal, themeable terminal player for network music streams.

mod config;
mod control;
mod logging;
mod media;
mod paths;
mod play;
mod stations;
mod tui;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ttnr_engine::{AudioOutput, EngineConfig};

/// Minimal, themeable terminal player for network music streams.
#[derive(Debug, Parser)]
#[command(name = "ttnr", version, about, long_about = None)]
struct Cli {
    /// Without a subcommand, the interactive terminal UI starts.
    #[command(subcommand)]
    command: Option<Command>,
    /// Theme for the terminal UI (overrides `theme` in config.toml).
    #[arg(long, global = true, value_name = "NAME")]
    theme: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Play a URL, file or station id with plain status output (Ctrl-C stops).
    Play(PlayArgs),
    /// Re-discover stream URLs (bypassing the cache) for a station or URL,
    /// or for all configured stations.
    Refresh {
        /// Station id or URL; all stations if omitted.
        target: Option<String>,
    },
    /// Search the radio-browser.info directory (or another source).
    Search {
        /// Text to look for in station names.
        query: String,
        /// Maximum number of results.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Source to search, e.g. an exec source from config.toml.
        #[arg(long, default_value = "radio-browser")]
        source: String,
    },
    /// Show what the running ttnr plays (via its control socket).
    Now {
        /// Print the full status as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Send a command to the running ttnr: toggle_pause, next, prev, mute,
    /// quit, `volume <0-100>`, `play <station-id>`.
    Ctl {
        /// Command name.
        command: String,
        /// Volume or station id.
        arg: Option<String>,
    },
    /// List the stations from stations.toml.
    Stations {
        /// Only stations whose id or name contains this text.
        #[arg(long, short)]
        search: Option<String>,
    },
}

#[derive(Debug, Args)]
struct PlayArgs {
    /// Stream or playlist URL (Icecast/Shoutcast, HLS, .pls, .m3u, .xspf,
    /// .asx), a local file, a station id from stations.toml, or
    /// `radio-browser:<uuid>` (see `ttnr search`).
    target: String,
    /// App volume in percent (0–100), independent of the system volume.
    #[arg(long, default_value_t = 100.0, value_parser = parse_volume)]
    volume: f32,
    /// Audio output driver (for testing).
    #[arg(long, hide = true, value_enum, default_value_t = Output::Coreaudio)]
    audio_output: Output,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Output {
    Coreaudio,
    Null,
}

fn parse_volume(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| format!("not a number: {s}"))?;
    if (0.0..=100.0).contains(&v) {
        Ok(v)
    } else {
        Err("volume must be between 0 and 100".into())
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    logging::init();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            eprintln!("ttnr: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    // Playing commands integrate with Now Playing / media keys; on macOS the
    // app then runs on a worker thread while the main thread hosts AppKit.
    // `--audio-output null` (tests) plays nothing audible: no integration.
    let plays = match &cli.command {
        None => true,
        Some(Command::Play(args)) => !matches!(args.audio_output, Output::Null),
        Some(_) => false,
    };
    let media = plays
        && config::load()
            .map(|c| c.media_controls_enabled())
            .unwrap_or(true);
    ttnr_platform::run_app(media, move |link| run_app(cli, link))
}

fn run_app(cli: Cli, media: Option<ttnr_platform::MediaLink>) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    // Audio options from config.toml (a broken config is reported by the
    // command itself).
    let mut engine = EngineConfig::default();
    if let Ok(c) = config::load() {
        c.apply_audio(&mut engine);
    }
    match cli.command {
        None => runtime.block_on(tui::run(&engine, cli.theme.as_deref(), media)),
        Some(Command::Play(args)) => {
            let cfg = EngineConfig {
                audio_output: match args.audio_output {
                    Output::Coreaudio => AudioOutput::CoreAudio,
                    Output::Null => AudioOutput::Null,
                },
                volume: args.volume,
                ..engine
            };
            runtime.block_on(play::run(&args.target, &cfg, media))
        }
        Some(Command::Refresh { target }) => runtime.block_on(play::refresh(target.as_deref())),
        Some(Command::Search {
            query,
            limit,
            source,
        }) => runtime.block_on(play::search(&query, limit, &source)),
        Some(Command::Now { json }) => runtime.block_on(now(json)),
        Some(Command::Ctl { command, arg }) => runtime.block_on(ctl(&command, arg.as_deref())),
        Some(Command::Stations { search }) => stations::print(search),
    }
}

/// `ttnr now [--json]`.
async fn now(json: bool) -> Result<()> {
    let answer = control::request(r#"{"cmd":"status"}"#).await?;
    if json {
        println!("{answer}");
    } else {
        let status: control::StatusJson = serde_json::from_str(&answer)?;
        println!("{}", status.line());
    }
    Ok(())
}

/// `ttnr ctl <command> [arg]`.
async fn ctl(command: &str, arg: Option<&str>) -> Result<()> {
    let request = match (command, arg) {
        ("volume", Some(v)) => {
            let v: i64 = v
                .parse()
                .map_err(|_| anyhow::anyhow!("volume must be 0-100"))?;
            serde_json::json!({ "cmd": "volume", "value": v })
        }
        ("play", Some(id)) => serde_json::json!({ "cmd": "play", "station": id }),
        (cmd, _) => serde_json::json!({ "cmd": cmd }),
    };
    let answer = control::request(&request.to_string()).await?;
    let v: serde_json::Value = serde_json::from_str(&answer)?;
    if v.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        anyhow::bail!(
            "{}",
            v.get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("refused")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn theme_flag() {
        let cli = Cli::try_parse_from(["ttnr", "--theme", "zx80"]).unwrap();
        assert_eq!(cli.theme.as_deref(), Some("zx80"));
        assert!(cli.command.is_none());
    }

    #[test]
    fn volume_bounds() {
        assert_eq!(parse_volume("40"), Ok(40.0));
        assert!(parse_volume("101").is_err());
        assert!(parse_volume("-1").is_err());
        assert!(parse_volume("loud").is_err());
    }
}
