use std::process;

use clap::Parser;

fn main() {
    let cli = maestro::interfaces::cli::Cli::parse();
    let auto_check = should_auto_check_after(&cli.command);
    if let Err(error) = maestro::interfaces::cli::run(cli) {
        if !error.is::<maestro::interfaces::cli::update::ReportedError>() {
            eprintln!("Error: {error:?}");
            if let Some(hint) = error_hint(&error) {
                eprintln!("fix: {hint}");
            }
        }
        process::exit(1);
    }
    if auto_check {
        let _ = maestro::interfaces::cli::update::run_auto_check();
    }
}

fn error_hint(error: &anyhow::Error) -> Option<String> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<maestro::foundation::core::error::MaestroError>()
            .and_then(|error| error.hint())
    })
}

fn should_auto_check_after(command: &maestro::interfaces::cli::RootCommand) -> bool {
    !matches!(
        command,
        maestro::interfaces::cli::RootCommand::Init(_)
            | maestro::interfaces::cli::RootCommand::Update(_)
            | maestro::interfaces::cli::RootCommand::Sync(_)
            | maestro::interfaces::cli::RootCommand::MigrateV2
            | maestro::interfaces::cli::RootCommand::Resume(_)
            | maestro::interfaces::cli::RootCommand::Mcp(_)
            | maestro::interfaces::cli::RootCommand::Hook(_)
            | maestro::interfaces::cli::RootCommand::ShellInit
    )
}
