use anyhow::{Result, anyhow};

use crate::domain::card;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{ArchiveArgs, DepArgs, DepCommand, ListArgs, ReadyArgs};

/// Execute `maestro ready`: workable cards with no open blockers.
pub fn ready(args: ReadyArgs) -> Result<()> {
    let paths = repo_paths()?;
    if card::store_mode(&paths) == card::StoreMode::Legacy {
        legacy_notice();
        return Ok(());
    }
    let cards = card::query::scan(&paths)?;
    let mut ready = card::query::ready(&cards);
    if let Some(feature) = args.feature.as_deref() {
        ready.retain(|c| c.parent.as_deref() == Some(feature));
    }
    render_ready(&ready);
    Ok(())
}

/// Execute `maestro list`: cards filtered by parent, type, assignee, or coarse status.
pub fn list(args: ListArgs) -> Result<()> {
    let paths = repo_paths()?;
    if card::store_mode(&paths) == card::StoreMode::Legacy {
        legacy_notice();
        return Ok(());
    }

    let card_type = args
        .card_type
        .as_deref()
        .map(|word| {
            card::schema::CardType::parse(word).ok_or_else(|| {
                anyhow!(
                    "unknown --type {word:?}; expected feature, task, bug, chore, idea, or decision"
                )
            })
        })
        .transpose()?;
    let status = args
        .status
        .as_deref()
        .map(|word| {
            card::query::Coarse::parse(word).ok_or_else(|| {
                anyhow!("unknown --status {word:?}; expected open, in_progress, or closed")
            })
        })
        .transpose()?;

    let filter = card::query::ListFilter {
        parent: args.parent.as_deref(),
        card_type,
        assignee: args.assignee.as_deref(),
        status,
    };
    let cards = card::query::scan(&paths)?;
    render_list(&card::query::query(&cards, &filter));
    Ok(())
}

/// Execute `maestro dep add <child> <parent>`: author a blocking edge so the
/// child waits on the parent (SPEC E1/DN6).
pub fn dep(args: DepArgs) -> Result<()> {
    let paths = repo_paths()?;
    if card::store_mode(&paths) == card::StoreMode::Legacy {
        legacy_notice();
        return Ok(());
    }
    match args.command {
        DepCommand::Add { child, parent } => {
            let added = card::edit::add_blocks_dep(&paths, &child, &parent, &utc_now_timestamp())?;
            if added {
                println!("{child} is now blocked by {parent}");
            } else {
                println!("{child} is already blocked by {parent}");
            }
        }
    }
    Ok(())
}

/// Execute `maestro archive <feature>`: move the feature card and its
/// `parent=<feature>` children to the archive sibling tree (SPEC E4/D5).
pub fn archive(args: ArchiveArgs) -> Result<()> {
    let paths = repo_paths()?;
    if card::store_mode(&paths) == card::StoreMode::Legacy {
        legacy_notice();
        return Ok(());
    }
    let report = card::archive::archive_feature(&paths, &args.feature)?;
    if report.children.is_empty() {
        println!("archived feature {} (no child cards)", report.feature);
    } else {
        println!(
            "archived feature {} + {} child card(s): {}",
            report.feature,
            report.children.len(),
            report.children.join(", ")
        );
    }
    Ok(())
}

fn repo_paths() -> Result<MaestroPaths> {
    Ok(MaestroPaths::new(discover_repo_root()?))
}

/// The card verbs read `.maestro/cards/`; an unmigrated repo has none. Exit 0
/// with one guiding line rather than a dead-end error: no cards is a state.
fn legacy_notice() {
    println!(
        "this repo has no card store yet (.maestro/cards/); the card verbs apply once it is migrated to the card model"
    );
}

fn render_ready(cards: &[&card::schema::Card]) {
    if cards.is_empty() {
        println!("No ready work.");
        return;
    }
    println!("Ready work ({} cards):", cards.len());
    let id_width = cards.iter().map(|c| c.id.len()).max().unwrap_or(0);
    let type_width = cards
        .iter()
        .map(|c| c.card_type.as_str().len())
        .max()
        .unwrap_or(0);
    for c in cards {
        println!(
            "  {:<id_width$}  {:<type_width$}  {}  {}",
            c.id,
            c.card_type.as_str(),
            c.title,
            claim_label(c),
        );
    }
}

fn render_list(cards: &[&card::schema::Card]) {
    if cards.is_empty() {
        println!("no cards match");
        return;
    }
    println!("ID\tTYPE\tSTATUS\tPARENT\tTITLE");
    for c in cards {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            c.id,
            c.card_type.as_str(),
            c.status,
            c.parent.as_deref().unwrap_or("-"),
            c.title,
        );
    }
}

fn claim_label(c: &card::schema::Card) -> String {
    match c.claimed_by.as_deref() {
        Some(who) => format!("@{who}"),
        None => "(unclaimed)".to_string(),
    }
}
