use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::core::error::MaestroError;
use crate::core::fs::ensure_dir;
use crate::core::managed_path::{managed_path, SymlinkPolicy};
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::safe_write::write_string_atomic;
use crate::harness::schema::HarnessConfig;
use crate::harness::templates::{backlog_yaml, features_yaml, harness_yml, HARNESS_MD};
use crate::skills::extract::{extract_bundled_skills, validate_bundled_skills, ExtractMode};

use super::InitArgs;

/// Execute `maestro init`.
pub fn run(args: InitArgs) -> Result<()> {
    let repo_root = init_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    managed_path(&paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    let plan = InitPlan::new(&paths)?;
    validate_plan_paths(&paths, &plan)?;

    if args.dry_run {
        print_dry_run(&plan);
        return Ok(());
    }

    let backup_timestamp = if args.force {
        Some(backup_operation_timestamp()?)
    } else {
        None
    };
    let extract_mode = extract_mode(&args, backup_timestamp.as_deref())?;
    validate_bundled_skills(&paths, extract_mode)?;

    for directory in plan.directories {
        ensure_dir(directory)?;
    }
    for file in plan.files {
        write_init_file(&paths, file, &args, backup_timestamp.as_deref())?;
    }
    extract_bundled_skills(&paths, extract_mode)?;

    Ok(())
}

fn init_repo_root() -> Result<PathBuf> {
    match discover_repo_root() {
        Ok(repo_root) => Ok(repo_root),
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::RepoRootNotFound { .. })
            ) =>
        {
            std::env::current_dir().context("failed to read current working directory")
        }
        Err(error) => Err(error),
    }
}

fn validate_plan_paths(paths: &MaestroPaths, plan: &InitPlan) -> Result<()> {
    for directory in &plan.directories {
        validate_managed_path(paths, directory)?;
    }
    for file in &plan.files {
        validate_managed_path(paths, &file.path)?;
    }
    Ok(())
}

fn validate_managed_path(paths: &MaestroPaths, path: &std::path::Path) -> Result<()> {
    let relative = path
        .strip_prefix(paths.repo_root())
        .with_context(|| format!("managed path is outside repo: {}", path.display()))?
        .to_str()
        .with_context(|| format!("managed path is not UTF-8: {}", path.display()))?;
    managed_path(paths, relative, SymlinkPolicy::RejectAllComponents)?;
    Ok(())
}

#[derive(Debug)]
struct InitPlan {
    directories: Vec<PathBuf>,
    files: Vec<InitFile>,
}

#[derive(Debug)]
struct InitFile {
    path: PathBuf,
    contents: String,
}

impl InitPlan {
    fn new(paths: &MaestroPaths) -> Result<Self> {
        let harness_config = HarnessConfig::detect(paths.repo_root());

        Ok(Self {
            directories: vec![
                paths.harness_dir(),
                paths.features_dir(),
                paths.decisions_dir(),
                paths.skills_dir(),
            ],
            files: vec![
                InitFile {
                    path: paths.harness_dir().join("HARNESS.md"),
                    contents: HARNESS_MD.to_string(),
                },
                InitFile {
                    path: paths.harness_dir().join("harness.yml"),
                    contents: harness_yml(&harness_config)?,
                },
                InitFile {
                    path: paths.harness_dir().join("backlog.yaml"),
                    contents: backlog_yaml()?,
                },
                InitFile {
                    path: paths.features_dir().join("features.yaml"),
                    contents: features_yaml(),
                },
            ],
        })
    }
}

fn print_dry_run(plan: &InitPlan) {
    println!("maestro init would create:");
    for directory in &plan.directories {
        println!("dir  {}", directory.display());
    }
    for file in &plan.files {
        println!("file {}", file.path.display());
    }
}

fn write_init_file(
    paths: &MaestroPaths,
    file: InitFile,
    args: &InitArgs,
    backup_timestamp: Option<&str>,
) -> Result<()> {
    if file.path.exists() {
        if args.merge {
            return Ok(());
        }
        if args.force {
            let backup_timestamp =
                backup_timestamp.context("force init must have a backup timestamp")?;
            backup_file_with_timestamp(paths, &file.path, "init", backup_timestamp)?;
        } else {
            bail!(
                "{} already exists; use --merge to keep it or --force to overwrite with backup",
                file.path.display()
            );
        }
    }

    write_string_atomic(&file.path, &file.contents)
        .with_context(|| format!("failed to write init file {}", file.path.display()))
}

fn extract_mode<'a>(args: &InitArgs, backup_timestamp: Option<&'a str>) -> Result<ExtractMode<'a>> {
    if args.merge {
        return Ok(ExtractMode::Merge);
    }
    if args.force {
        let backup_timestamp =
            backup_timestamp.context("force init must have a backup timestamp")?;
        return Ok(ExtractMode::Force { backup_timestamp });
    }

    Ok(ExtractMode::Create)
}
