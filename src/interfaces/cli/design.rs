use anyhow::{Context, Result, bail};

use crate::domain::design;
use crate::foundation::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::interfaces::cli::{DesignArgs, DesignCommand};

/// Execute `maestro design`: list shipped DESIGN.md styles or copy one into the repo.
pub fn run(args: DesignArgs) -> Result<()> {
    match args.command {
        DesignCommand::List => list(),
        DesignCommand::Init {
            style,
            dry_run,
            force,
        } => init(style.as_deref(), dry_run, force),
    }
}

fn list() -> Result<()> {
    let manifest = design::awesome_manifest()?;
    println!("default: {}", design::default_style());
    println!(
        "awesome-design-md: {} @ {} ({} files, {})",
        manifest.source.repository,
        manifest.source.commit,
        manifest.copied_files.len(),
        manifest.source.license
    );
    println!("styles:");
    for style in design::styles() {
        println!("  {}\t{}", style.token, style.source_label);
    }
    Ok(())
}

fn init(style: Option<&str>, dry_run: bool, force: bool) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let target = paths.repo_root().join("DESIGN.md");
    let served = design::serve(style)?;
    let exists = target.exists();

    if dry_run {
        print_init_preview(&paths, &target, &served, exists)?;
        return Ok(());
    }

    if exists && !force {
        bail!(
            "{} already exists; use --force to overwrite it with a backup",
            target.display()
        );
    }

    let backup = if exists {
        let timestamp = backup_operation_timestamp()?;
        Some(backup_file_with_timestamp(
            &paths,
            &target,
            "design-init",
            &timestamp,
        )?)
    } else {
        None
    };
    write_string_atomic(&target, served.contents)
        .with_context(|| format!("failed to write {}", target.display()))?;

    println!("wrote: {}", target.display());
    println!("style: {}", served.style.token);
    println!("source: {}", served.style.source_label);
    if let Some(backup) = backup {
        println!("backup: {}", backup.display());
    }
    Ok(())
}

fn print_init_preview(
    paths: &MaestroPaths,
    target: &std::path::Path,
    served: &design::ServedDesign,
    exists: bool,
) -> Result<()> {
    println!("action: dry-run");
    println!("target: {}", target.display());
    println!("repo: {}", paths.repo_root().display());
    println!("style: {}", served.style.token);
    println!("source: {}", served.style.source_label);
    println!("available_styles: {}", design::styles().len());
    println!("design_md_exists: {exists}");
    if served.style.is_vendor {
        let manifest = design::awesome_manifest()?;
        println!("upstream_repository: {}", manifest.source.repository);
        println!("upstream_commit: {}", manifest.source.commit);
        println!("upstream_license: {}", manifest.source.license);
        println!("upstream_copied_files: {}", manifest.copied_files.len());
    }
    println!(
        "would_write: {}",
        if exists {
            "overwrite only with --force"
        } else {
            "create DESIGN.md"
        }
    );
    Ok(())
}
