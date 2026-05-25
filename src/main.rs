mod commands;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = commands::Cli::parse();
    commands::run(cli)
}
