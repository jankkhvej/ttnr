//! `ttnr` – minimal, themeable terminal player for network music streams.

mod config;
mod logging;
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
    /// .asx), a local file, or a station id from stations.toml.
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
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    match cli.command {
        None => runtime.block_on(tui::run(&EngineConfig::default(), cli.theme.as_deref())),
        Some(Command::Play(args)) => {
            let cfg = EngineConfig {
                audio_output: match args.audio_output {
                    Output::Coreaudio => AudioOutput::CoreAudio,
                    Output::Null => AudioOutput::Null,
                },
                volume: args.volume,
                ..EngineConfig::default()
            };
            runtime.block_on(play::run(&args.target, &cfg))
        }
        Some(Command::Refresh { target }) => runtime.block_on(play::refresh(target.as_deref())),
        Some(Command::Stations { search }) => stations::print(search),
    }
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
