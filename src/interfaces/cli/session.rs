use anyhow::Result;

use crate::domain::run;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{SessionArgs, SessionCommand, SessionShowArgs};

pub fn run(args: SessionArgs) -> Result<()> {
    match args.command {
        SessionCommand::Show(args) => show(args),
    }
}

fn show(args: SessionShowArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let readout = run::session_readout(&paths, &args.session_id)?;
    if args.json {
        println!("{}", serde_json::to_string(&readout)?);
        return Ok(());
    }
    render_text(&readout);
    Ok(())
}

fn render_text(readout: &run::SessionReadout) {
    println!("Session: {}", readout.session_id);
    println!("Outcome: {}", readout.outcome);
    println!("Ownership: {}", readout.ownership);
    println!();
    println!("Activity:");
    println!("- commands: {}", readout.activity.commands);
    println!("- activity events: {}", readout.activity.events);
    println!("- compactions: {}", readout.activity.compactions);
    if !readout.activity.counts.is_empty() {
        let counts = readout
            .activity
            .counts
            .iter()
            .map(|(kind, count)| format!("{kind}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("- kinds: {counts}");
    }
    println!();
    println!("Lifecycle:");
    println!("- events: {}", readout.lifecycle.events);
    if let Some(last_action) = &readout.lifecycle.last_action {
        println!("- last action: {last_action}");
    }
    println!();
    println!("Tasks:");
    if readout.tasks.is_empty() {
        println!("- none");
    } else {
        for task in &readout.tasks {
            println!(
                "- {} [{}] {} (proof events: {})",
                task.id, task.status, task.title, task.proof_events
            );
        }
    }
    println!();
    println!("Proof:");
    println!("- proof events: {}", readout.proof.events);
    println!();
    println!("Sources:");
    println!("- activity: {}", readout.sources.activity);
    println!("- lifecycle: {}", readout.sources.lifecycle);
    println!("- proof: {}", readout.sources.proof);
    println!("- transcript: {}", readout.sources.transcript);
    if !readout.gaps.is_empty() {
        println!();
        println!("Gaps:");
        for gap in &readout.gaps {
            println!("- {gap}");
        }
    }
}
