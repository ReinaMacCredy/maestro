use anyhow::Result;

use crate::domain::harness::BacklogItem;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{HarnessArgs, HarnessCommand};
use crate::operations::harness;

/// Execute `maestro harness`.
pub fn run(args: HarnessArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        HarnessCommand::List { all } => list(&paths, all),
        HarnessCommand::Show { id } => show(&paths, &id),
        HarnessCommand::Apply { id } => apply(&paths, &id),
        HarnessCommand::Measure { id, force } => measure(&paths, &id, force),
    }
}

fn list(paths: &MaestroPaths, all: bool) -> Result<()> {
    let (backlog, ready) = harness::refresh(paths)?;
    let visible = backlog
        .items
        .iter()
        .filter(|item| is_visible(item, all))
        .collect::<Vec<_>>();
    if visible.is_empty() {
        println!("no improvement proposals found");
        return Ok(());
    }
    println!("ID\tSTATUS\tTYPE\tTITLE");
    for item in visible {
        let hint = if ready.contains(&item.id) {
            "\t(ready to measure)"
        } else {
            ""
        };
        println!(
            "{}\t{}\t{}\t{}{}",
            item.id,
            field_or_default(&item.status, "proposed"),
            field_or_default(&item.item_type, "unknown"),
            item.title,
            hint
        );
    }
    Ok(())
}

fn show(paths: &MaestroPaths, id: &str) -> Result<()> {
    let (backlog, _) = harness::refresh(paths)?;
    print_item(backlog.find(id)?);
    Ok(())
}

fn apply(paths: &MaestroPaths, id: &str) -> Result<()> {
    let item = harness::apply(paths, id)?;
    match &item.spawned_task {
        Some(task) => println!("accepted {} (spawned {task})", item.id),
        None => println!("accepted {}", item.id),
    }
    Ok(())
}

fn measure(paths: &MaestroPaths, id: &str, force: bool) -> Result<()> {
    let item = harness::measure(paths, id, force)?;
    println!(
        "{} is now {}",
        item.id,
        field_or_default(&item.status, "proposed")
    );
    Ok(())
}

/// Default list shows the active set (proposed + accepted); `--all` adds the
/// `measured` ledger.
fn is_visible(item: &BacklogItem, all: bool) -> bool {
    all || field_or_default(&item.status, "proposed") != "measured"
}

fn print_item(item: &BacklogItem) {
    println!("id: {}", item.id);
    println!("title: {}", item.title);
    println!("type: {}", field_or_default(&item.item_type, "unknown"));
    println!("status: {}", field_or_default(&item.status, "proposed"));
    println!("priority: {}", field_or_default(&item.priority, "medium"));
    if !item.source.is_empty() {
        println!("source: {}", item.source);
    }
    if let Some(task) = &item.spawned_task {
        println!("spawned_task: {task}");
    }
    if !item.evidence.is_empty() {
        println!("evidence:");
        for entry in &item.evidence {
            println!("- {entry}");
        }
    }
    if !item.history.is_empty() {
        println!("history:");
        for entry in &item.history {
            match &entry.task {
                Some(task) => println!("- {} ({}) {}", entry.result, task, entry.at),
                None => println!("- {} {}", entry.result, entry.at),
            }
        }
    }
}

fn field_or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}
