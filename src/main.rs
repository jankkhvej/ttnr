//! `ttnr` – minimal, themeable terminal player for network music streams.

use anyhow::Result;
use clap::Parser;

/// Minimal, themeable terminal player for network music streams.
#[derive(Debug, Parser)]
#[command(name = "ttnr", version, about, long_about = None)]
struct Cli {}

fn main() -> Result<()> {
    let _cli = Cli::parse();
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
}
