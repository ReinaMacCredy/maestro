use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = maestro::commands::Cli::parse();
    maestro::commands::run(cli)
}
