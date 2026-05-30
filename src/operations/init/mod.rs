use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::domain::extraction::{extract_all, validate_all, ExtractMode};
use crate::domain::harness::schema::HarnessConfig;
use crate::domain::harness::templates::{backlog_yaml, features_yaml, harness_yml};
use crate::foundation::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::foundation::core::safe_write::write_string_atomic;

/// Options for one `maestro init` operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InitOptions {
    /// Print the planned tree without writing files.
    pub dry_run: bool,
    /// Preserve existing files while creating missing artifacts.
    pub merge: bool,
    /// Overwrite existing files after backing them up.
    pub force: bool,
}

/// Result of a complete init operation.
#[derive(Debug)]
pub enum InitOutcome {
    /// Init applied the artifact plan.
    Applied,
    /// Init only planned the artifact tree.
    DryRun(InitPlan),
}

/// Coordinate startup artifact creation through owning domain contracts.
pub fn run(options: &InitOptions) -> Result<InitOutcome> {
    let repo_root = init_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    managed_path(&paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    let plan = InitPlan::new(&paths)?;
    validate_plan_paths(&paths, &plan)?;

    if options.dry_run {
        return Ok(InitOutcome::DryRun(plan));
    }

    let backup_timestamp = if options.force {
        Some(backup_operation_timestamp()?)
    } else {
        None
    };
    let extract_mode = extract_mode(options, backup_timestamp.as_deref())?;
    validate_all(&paths, extract_mode)?;

    for directory in plan.directories {
        ensure_dir(directory)?;
    }
    for file in plan.files {
        write_init_file(&paths, file, options, backup_timestamp.as_deref())?;
    }
    extract_all(&paths, extract_mode)?;

    Ok(InitOutcome::Applied)
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

/// Startup artifact plan for `maestro init`.
#[derive(Debug)]
pub struct InitPlan {
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

/// Render the dry-run artifact tree.
pub fn render_dry_run(plan: &InitPlan) -> String {
    let mut out = String::from("maestro init would create:\n");
    for directory in &plan.directories {
        out.push_str(&format!("dir  {}\n", directory.display()));
    }
    for file in &plan.files {
        out.push_str(&format!("file {}\n", file.path.display()));
    }
    out
}

fn write_init_file(
    paths: &MaestroPaths,
    file: InitFile,
    options: &InitOptions,
    backup_timestamp: Option<&str>,
) -> Result<()> {
    if file.path.exists() {
        if options.merge {
            return Ok(());
        }
        if options.force {
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

fn extract_mode<'a>(
    options: &InitOptions,
    backup_timestamp: Option<&'a str>,
) -> Result<ExtractMode<'a>> {
    if options.merge {
        return Ok(ExtractMode::Merge);
    }
    if options.force {
        let backup_timestamp =
            backup_timestamp.context("force init must have a backup timestamp")?;
        return Ok(ExtractMode::Force { backup_timestamp });
    }

    Ok(ExtractMode::Create)
}
