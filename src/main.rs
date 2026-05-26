use std::process;

use clap::Parser;

fn main() {
    let cli = maestro::commands::Cli::parse();
    let auto_check = should_auto_check_after(&cli.command);
    if let Err(error) = maestro::commands::run(cli) {
        if !error.is::<maestro::commands::update::ReportedError>() {
            eprintln!("Error: {error:?}");
        }
        process::exit(1);
    }
    if auto_check {
        let _ = maestro::commands::update::run_auto_check();
    }
}

fn should_auto_check_after(command: &maestro::commands::RootCommand) -> bool {
    !matches!(
        command,
        maestro::commands::RootCommand::Update(_)
            | maestro::commands::RootCommand::Mcp(_)
            | maestro::commands::RootCommand::Hook(_)
            | maestro::commands::RootCommand::ShellInit
    )
}
