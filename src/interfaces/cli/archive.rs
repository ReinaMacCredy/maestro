use anyhow::{Result, bail};

use crate::domain::card;
use crate::foundation::core::paths::discover_repo_root;
use crate::interfaces::cli::card as card_cli;
use crate::interfaces::cli::{
    ArchiveArgs, ArchiveCleanupArgs, ArchiveCommand, ArchiveMigrateDbArgs, CardArchiveArgs,
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
        ArchiveCommand::MigrateDb(args) => migrate_db(&paths, args),
        ArchiveCommand::Doctor => doctor(&paths),
        ArchiveCommand::Cleanup(args) => cleanup(&paths, args),
        ArchiveCommand::Stats => stats(&paths),
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
