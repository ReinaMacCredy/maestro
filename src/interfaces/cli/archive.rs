use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::domain::{card, feature};
use crate::foundation::core::paths::discover_repo_root;
use crate::interfaces::cli::card as card_cli;
use crate::interfaces::cli::{
    ArchiveApplyArgs, ArchiveArgs, ArchiveCleanupArgs, ArchiveCommand, ArchiveMigrateDbArgs,
    CardArchiveArgs, OwnershipReleaseStatus,
};

pub fn run(args: ArchiveArgs) -> Result<()> {
    match args.command {
        Some(command) => run_command(command),
        None => card_cli::archive(CardArchiveArgs {
            feature: args.feature,
            loose: args.loose,
        }),
    }
}

fn run_command(command: ArchiveCommand) -> Result<()> {
    let paths = crate::foundation::core::paths::MaestroPaths::new(discover_repo_root()?);
    match command {
        ArchiveCommand::Candidates { json } => candidates(&paths, json),
        ArchiveCommand::Check { id, json } => check(&paths, &id, json),
        ArchiveCommand::Apply(args) => apply(&paths, args),
        ArchiveCommand::MigrateDb(args) => migrate_db(&paths, args),
        ArchiveCommand::Doctor => doctor(&paths),
        ArchiveCommand::Cleanup(args) => cleanup(&paths, args),
        ArchiveCommand::Stats => stats(&paths),
    }
}

fn candidates(paths: &crate::foundation::core::paths::MaestroPaths, json_out: bool) -> Result<()> {
    let candidates = feature::archive_candidates(paths, &feature::ArchiveGateEvidence::default())?;
    if json_out {
        println!(
            "{}",
            json!({
                "schema": "maestro.archive.candidates.v1",
                "candidates": candidates.iter().map(candidate_json).collect::<Vec<_>>()
            })
        );
        return Ok(());
    }
    if candidates.is_empty() {
        println!("no archive candidates");
        return Ok(());
    }
    println!("ID\tACTION\tSTATUS\tCHILD_TASKS\tREASON");
    for candidate in candidates {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            candidate.id,
            candidate.action.as_str(),
            candidate.status,
            candidate.child_tasks,
            candidate.reasons.join("; ")
        );
    }
    Ok(())
}

fn check(
    paths: &crate::foundation::core::paths::MaestroPaths,
    id: &str,
    json_out: bool,
) -> Result<()> {
    let candidate =
        feature::archive_candidate(paths, id, &feature::ArchiveGateEvidence::default())?;
    if json_out {
        println!(
            "{}",
            json!({
                "schema": "maestro.archive.check.v1",
                "candidate": candidate_json(&candidate)
            })
        );
        return Ok(());
    }
    print_candidate(&candidate);
    Ok(())
}

fn apply(
    paths: &crate::foundation::core::paths::MaestroPaths,
    args: ArchiveApplyArgs,
) -> Result<()> {
    if args.all == args.id.is_some() {
        bail!("choose exactly one: maestro archive apply <id> OR maestro archive apply --all");
    }
    let selection = match args.id {
        Some(id) => feature::ArchiveApplySelection::One(id),
        None => feature::ArchiveApplySelection::All,
    };
    let plan = feature::archive_apply_plan(
        paths,
        selection.clone(),
        &feature::ArchiveGateEvidence::default(),
    )?;
    let mut results = Vec::new();
    match selection {
        feature::ArchiveApplySelection::One(id) => {
            let Some(candidate) = plan.candidates.first() else {
                bail!("archive apply {id}: target did not produce an archive plan");
            };
            if candidate.action != feature::ArchiveCandidateAction::ArchiveNow {
                if candidate.action == feature::ArchiveCandidateAction::ReleaseOnly {
                    super::emit_ownership_release(
                        paths,
                        &id,
                        OwnershipReleaseStatus::Released,
                        Some("archive release-only"),
                    );
                    if args.json {
                        results.push(json!({
                            "id": id,
                            "action": candidate.action.as_str(),
                            "result": "released",
                            "reasons": candidate.reasons,
                            "child_tasks": candidate.child_tasks
                        }));
                        print_apply_json("one", &results);
                        return Ok(());
                    }
                    println!("released archive ownership for {id}");
                    println!("  {}", candidate.reasons.join("; "));
                    println!("next: maestro active");
                    return Ok(());
                }
                if args.json {
                    results.push(json!({
                        "id": id,
                        "action": candidate.action.as_str(),
                        "result": "skipped",
                        "reasons": candidate.reasons,
                        "child_tasks": candidate.child_tasks
                    }));
                    print_apply_json("one", &results);
                    return Ok(());
                }
                bail!(
                    "archive apply {id}: {} — {}",
                    candidate.action.as_str(),
                    candidate.reasons.join("; ")
                );
            }
            let report = feature::archive_feature(paths, &id, false)?;
            super::emit_ownership_release(
                paths,
                &id,
                OwnershipReleaseStatus::Done,
                Some("archive apply"),
            );
            if args.json {
                results.push(json!({
                    "id": id,
                    "action": feature::ArchiveCandidateAction::ArchiveNow.as_str(),
                    "result": "archived",
                    "reasons": candidate.reasons,
                    "child_tasks": report.child_tasks
                }));
                print_apply_json("one", &results);
                return Ok(());
            }
            println!("archived {id}");
            println!("  {}", report.note);
            println!("  child_tasks: {}", report.child_tasks);
            println!("next: maestro archive check {id}");
        }
        feature::ArchiveApplySelection::All => {
            let targets = plan.archive_targets();
            if targets.is_empty() {
                if args.json {
                    print_apply_json("all", &results);
                    return Ok(());
                }
                println!("archive apply --all: nothing to archive");
                return Ok(());
            }
            let mut archived = 0usize;
            let mut child_tasks = 0usize;
            for id in targets {
                let report = feature::archive_feature(paths, &id, false)?;
                super::emit_ownership_release(
                    paths,
                    &id,
                    OwnershipReleaseStatus::Done,
                    Some("archive apply --all"),
                );
                archived += 1;
                child_tasks += report.child_tasks;
                results.push(json!({
                    "id": id,
                    "action": feature::ArchiveCandidateAction::ArchiveNow.as_str(),
                    "result": "archived",
                    "reasons": [],
                    "child_tasks": report.child_tasks
                }));
                if !args.json {
                    println!("archived {id}");
                }
            }
            if args.json {
                print_apply_json("all", &results);
                return Ok(());
            }
            println!("archive summary:");
            println!("  features: {archived} archived");
            println!("  child_tasks: {child_tasks}");
            println!("next: maestro archive candidates");
        }
    }
    Ok(())
}

fn candidate_json(candidate: &feature::ArchiveCandidate) -> Value {
    json!({
        "id": candidate.id,
        "title": candidate.title,
        "status": candidate.status,
        "action": candidate.action.as_str(),
        "reasons": candidate.reasons,
        "child_tasks": candidate.child_tasks,
        "archived": candidate.archived
    })
}

fn print_apply_json(selection: &str, results: &[Value]) {
    println!(
        "{}",
        json!({
            "schema": "maestro.archive.apply.v1",
            "selection": selection,
            "results": results
        })
    );
}

fn print_candidate(candidate: &feature::ArchiveCandidate) {
    println!("archive check: {}", candidate.id);
    println!("  action: {}", candidate.action.as_str());
    println!("  status: {}", candidate.status);
    println!("  archived: {}", candidate.archived);
    println!("  child_tasks: {}", candidate.child_tasks);
    println!("  reasons:");
    for reason in &candidate.reasons {
        println!("    - {reason}");
    }
    match candidate.action {
        feature::ArchiveCandidateAction::ArchiveNow => {
            println!("next: maestro archive apply {}", candidate.id);
        }
        feature::ArchiveCandidateAction::ReleaseOnly => {
            println!(
                "next: maestro active release {} --reason \"archive check: release stale active ownership\"",
                candidate.id
            );
        }
        feature::ArchiveCandidateAction::NeedsClose => {
            println!(
                "next: maestro feature close {} --outcome \"<outcome>\"",
                candidate.id
            );
        }
        feature::ArchiveCandidateAction::NeedsDecision => {
            println!(
                "next: decide whether to close, cancel, or keep {}",
                candidate.id
            );
        }
        feature::ArchiveCandidateAction::Blocked => {
            println!("next: clear the blocker(s), then retry");
        }
    }
}

fn migrate_db(
    paths: &crate::foundation::core::paths::MaestroPaths,
    args: ArchiveMigrateDbArgs,
) -> Result<()> {
    if args.dry_run == args.apply {
        bail!("choose exactly one: maestro archive migrate-db --dry-run OR --apply");
    }
    if args.dry_run {
        let plan = card::archive_migration_plan(paths)?;
        println!("archive DB migration dry-run:");
        println!("  folder-backed archived cards: {}", plan.folder_archives);
        println!("  would import snapshots: {}", plan.importable_snapshots);
        println!(
            "  would quarantine folders under: {}",
            plan.quarantine_dir.display()
        );
        return Ok(());
    }
    let report = card::migrate_legacy_archive_folders(paths)?;
    println!("archive DB migration:");
    println!("  imported snapshots: {}", report.imported_snapshots);
    println!("  quarantined folders: {}", report.quarantined_folders);
    println!("  quarantine: {}", report.quarantine_dir.display());
    println!("next: maestro archive doctor");
    Ok(())
}

fn doctor(paths: &crate::foundation::core::paths::MaestroPaths) -> Result<()> {
    let report = card::archive_doctor(paths)?;
    println!("archive doctor:");
    println!("  schema_version: {}", report.schema_version);
    println!("  snapshots: {}", report.snapshots);
    println!("  archived cards: {}", report.cards);
    println!("  legacy quarantines: {}", report.quarantine_dirs);
    println!("archive: ok");
    Ok(())
}

fn cleanup(
    paths: &crate::foundation::core::paths::MaestroPaths,
    args: ArchiveCleanupArgs,
) -> Result<()> {
    if args.dry_run == args.apply {
        bail!("choose exactly one: maestro archive cleanup --dry-run OR --apply");
    }
    let report = card::archive_doctor(paths)?;
    if args.dry_run {
        println!("archive cleanup dry-run:");
        println!("  legacy quarantines: {}", report.quarantine_dirs);
        println!("  doctor: ok");
        return Ok(());
    }
    let removed = card::cleanup_legacy_archive_quarantine(paths)?;
    println!("archive cleanup:");
    println!("  removed legacy quarantines: {removed}");
    Ok(())
}

fn stats(paths: &crate::foundation::core::paths::MaestroPaths) -> Result<()> {
    let report = card::archive_doctor(paths)?;
    println!("archive stats:");
    println!("  snapshots: {}", report.snapshots);
    println!("  archived cards: {}", report.cards);
    println!("  legacy quarantines: {}", report.quarantine_dirs);
    Ok(())
}
