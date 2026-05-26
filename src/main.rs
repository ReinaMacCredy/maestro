use std::process;

use clap::Parser;

fn main() {
    let cli = maestro::commands::Cli::parse();
    if let Err(error) = maestro::commands::run(cli) {
        if !error.is::<maestro::commands::update::ReportedError>() {
            eprintln!("Error: {error:?}");
        }
        process::exit(1);
    }
}
