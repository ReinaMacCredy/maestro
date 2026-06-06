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
        HarnessCommand::Set { claims_only } => set(&paths, claims_only),
        HarnessCommand::Propose {
            title,
            evidence,
            topic,
        } => propose(&paths, &title, &evidence, topic.as_deref()),
        HarnessCommand::Apply { id } => apply(&paths, &id),
        HarnessCommand::Dismiss { id, reason } => dismiss(&paths, &id, &reason),
        HarnessCommand::Measure { id, force } => measure(&paths, &id, force),
    }
}

fn propose(paths: &MaestroPaths, title: &str, evidence: &str, topic: Option<&str>) -> Result<()> {
    let item = harness::propose_agent_audit(paths, title, evidence, topic, &super::cli_run_id())?;
    println!("proposed {} ({})", item.id, item.title);
    println!(
        "provenance: {}",
        field_or_default(&item.provenance, "agent-audit")
    );
    if !item.topic.is_empty() {
        println!("topic: {}", item.topic);
    }
    println!("seen: {}", seen_label(&item));
    Ok(())
}

fn set(paths: &MaestroPaths, claims_only: bool) -> Result<()> {
    if !claims_only {
        anyhow::bail!("no harness policy field selected; pass --claims-only");
    }
    harness::set_claims_only_verification(paths)?;
    println!("claims-only verification accepted for this repo");
    Ok(())
}

fn list(paths: &MaestroPaths, all: bool) -> Result<()> {
    let (backlog, ready, over_threshold_items) = harness::refresh(paths)?;
    let over_threshold = over_threshold_items
        .into_iter()
        .map(|item| item.id)
        .collect::<std::collections::BTreeSet<_>>();
    // Terminal ledger items are hidden by default; surface the count so they
    // don't appear to have vanished (UX-3).
    let hidden = backlog
        .items
        .iter()
        .filter(|item| !is_visible(item, false))
        .count();
    let visible = backlog
        .items
        .iter()
        .filter(|item| is_visible(item, all))
        .collect::<Vec<_>>();
    if visible.is_empty() {
        println!("no improvement proposals found");
        if !all && hidden > 0 {
            println!("# {hidden} terminal proposal(s) hidden; use --all to include");
        }
        return Ok(());
    }
    println!("ID\t!\tSTATUS\tTYPE\tSEEN\tTITLE");
    for item in visible {
        let hint = if ready.contains(&item.id) {
            "\t(ready to measure)"
        } else {
            ""
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}{}",
            item.id,
            if over_threshold.contains(&item.id) {
                "!"
            } else {
                ""
            },
            field_or_default(&item.status, "proposed"),
            field_or_default(&item.item_type, "unknown"),
            seen_label(item),
            item.title,
            hint
        );
    }
    if !all && hidden > 0 {
        println!("# {hidden} terminal proposal(s) hidden; use --all to include");
    }
    Ok(())
}

fn show(paths: &MaestroPaths, id: &str) -> Result<()> {
    let (backlog, _, _) = harness::refresh(paths)?;
    print_item(backlog.find(id)?);
    Ok(())
}

fn apply(paths: &MaestroPaths, id: &str) -> Result<()> {
    let applied = harness::apply(paths, id)?;
    match &applied.item.spawned_task {
        Some(task) => {
            println!("accepted {} (spawned {task})", applied.item.id);
            println!("  check preset: \"{}\"", applied.check);
            println!("next: maestro task claim {task}");
        }
        None => println!("accepted {}", applied.item.id),
    }
    Ok(())
}

fn dismiss(paths: &MaestroPaths, id: &str, reason: &str) -> Result<()> {
    let item = harness::dismiss(paths, id, reason)?;
    println!("dismissed {}", item.id);
    println!(
        "reason: {}",
        item.dismissal_reason.as_deref().unwrap_or(reason)
    );
    Ok(())
}

fn measure(paths: &MaestroPaths, id: &str, force: bool) -> Result<()> {
    let (item, friction_live) = harness::measure(paths, id, force)?;
    let status = field_or_default(&item.status, "proposed");
    if status == "proposed" {
        // A state detector still emitting reverts instead of closing (D2); frame it
        // as ineffective rather than a bare status line (T9.2).
        println!(
            "{} reverted to proposed: the improvement was ineffective (friction still detected); \
             re-run `maestro harness apply {}` to try again",
            item.id, item.id
        );
    } else {
        println!("{} is now {status}", item.id);
        if friction_live {
            // Behavioral item closed by your judgment, not an automatic silence
            // check, while its friction is still detected (T9).
            println!(
                "note: friction is still detected; this behavioral item was closed by judgment, \
                 not by a silence check"
            );
        }
    }
    Ok(())
}

/// Default list shows the active set (proposed + accepted); `--all` adds the
/// terminal ledger.
fn is_visible(item: &BacklogItem, all: bool) -> bool {
    let status = field_or_default(&item.status, "proposed");
    all || !matches!(status, "measured" | "dismissed")
}

fn print_item(item: &BacklogItem) {
    println!("id: {}", item.id);
    println!("title: {}", item.title);
    println!("type: {}", field_or_default(&item.item_type, "unknown"));
    println!("status: {}", field_or_default(&item.status, "proposed"));
    println!("priority: {}", field_or_default(&item.priority, "medium"));
    println!("seen: {}", seen_label(item));
    if !item.sessions_hit.is_empty() {
        println!("sessions_hit: {}", item.sessions_hit.join(", "));
    }
    if !item.first_seen.is_empty() {
        println!("first_seen: {}", item.first_seen);
    }
    if !item.last_seen.is_empty() {
        println!("last_seen: {}", item.last_seen);
    }
    if !item.source.is_empty() {
        println!("source: {}", item.source);
    }
    if !item.provenance.is_empty() {
        println!("provenance: {}", item.provenance);
    }
    if !item.topic.is_empty() {
        println!("topic: {}", item.topic);
    }
    if let Some(task) = &item.spawned_task {
        println!("spawned_task: {task}");
    }
    if let Some(reason) = &item.dismissal_reason {
        println!("dismissal_reason: {reason}");
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

fn seen_label(item: &BacklogItem) -> String {
    format!("{}x/{}s", item.occurrences, item.sessions_hit.len())
}
