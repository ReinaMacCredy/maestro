use anyhow::Result;

use crate::domain::harness::{BacklogConfig, BacklogItem};
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::improver::propose;
use crate::interfaces::cli::{ImproveArgs, ImproveCommand};

/// Execute `maestro improve`.
pub fn run(args: ImproveArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        ImproveCommand::List => list(&paths),
        ImproveCommand::Show { id } => show(&paths, &id),
        ImproveCommand::Apply { id } => apply(&paths, &id),
    }
}

fn list(paths: &MaestroPaths) -> Result<()> {
    let backlog = propose::refresh(paths)?;
    if backlog.items.is_empty() {
        println!("no improvement proposals found");
        return Ok(());
    }
    println!("ID\tSTATUS\tTYPE\tTITLE");
    for item in backlog.items {
        println!(
            "{}\t{}\t{}\t{}",
            item.id,
            field_or_default(&item.status, "proposed"),
            field_or_default(&item.item_type, "unknown"),
            item.title
        );
    }
    Ok(())
}

fn show(paths: &MaestroPaths, id: &str) -> Result<()> {
    let backlog = propose::refresh(paths)?;
    let item = find_item(&backlog, id)?;
    print_item(item);
    Ok(())
}

fn apply(paths: &MaestroPaths, id: &str) -> Result<()> {
    let item = propose::apply(paths, id)?;
    println!("applied {}", item.id);
    Ok(())
}

fn find_item<'a>(backlog: &'a BacklogConfig, id: &str) -> Result<&'a BacklogItem> {
    backlog
        .items
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| anyhow::anyhow!("backlog item not found: {id}"))
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
    if !item.evidence.is_empty() {
        println!("evidence:");
        for entry in &item.evidence {
            println!("- {entry}");
        }
    }
}

fn field_or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() {
        fallback
    } else {
        value
    }
}
