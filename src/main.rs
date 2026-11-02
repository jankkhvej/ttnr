//! `ttnr` – minimal, themeable terminal player for network music streams.

mod logging;
mod paths;
mod play;
mod stations;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ttnr_engine::{AudioOutput, EngineConfig};

/// Minimal, themeable terminal player for network music streams.
#[derive(Debug, Parser)]
#[command(name = "ttnr", version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Play a URL, file or station id with plain status output (Ctrl-C stops).
    Play(PlayArgs),
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
        Command::Play(args) => {
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
        Command::Stations { search } => stations::print(search),
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
    fn volume_bounds() {
        assert_eq!(parse_volume("40"), Ok(40.0));
        assert!(parse_volume("101").is_err());
        assert!(parse_volume("-1").is_err());
        assert!(parse_volume("loud").is_err());
    }
}
