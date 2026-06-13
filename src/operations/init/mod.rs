use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::domain::extraction::{
    ExtractMode, FolderDecision, FolderPreview, extract_all, preview_all, render_preview,
    validate_all,
};
use crate::domain::harness::schema::HarnessConfig;
use crate::domain::harness::templates::harness_yml;
use crate::foundation::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::{MaestroPaths, announce_repo_root, discover_repo_root};
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
    Applied {
        /// Bundled folders that merge preserved but that are behind this
        /// binary's shipped versions (resolvable with `maestro sync`). Always 0
        /// outside merge mode, where Create/Force leave nothing drifted.
        behind: usize,
        /// The repository root init operated on, for the success line (T6.1).
        root: PathBuf,
    },
    /// Init only planned the artifact tree and previewed bundled extraction.
    DryRun {
        /// The startup directories and files init would create.
        plan: InitPlan,
        /// The whole-folder fate of every bundled resource under the chosen mode.
        preview: Vec<FolderPreview>,
    },
}

/// Coordinate startup artifact creation through owning domain contracts.
pub fn run(options: &InitOptions) -> Result<InitOutcome> {
    let repo_root = init_repo_root()?;
    announce_repo_root(&repo_root);
    let paths = MaestroPaths::new(repo_root);
    managed_path(&paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    let plan = InitPlan::new(&paths)?;
    validate_plan_paths(&paths, &plan)?;

    if options.dry_run {
        // Preview bundled extraction under the same mode init would apply. The
        // timestamp is a dummy: preview is read-only and never backs up or
        // writes, so it never consults it.
        let preview = preview_all(&paths, extract_mode(options, Some(""))?)?;
        return Ok(InitOutcome::DryRun { plan, preview });
    }

    // Bare `init` (no --merge/--force) is strict-create. On an already-initialized
    // repo it would otherwise bail per-file deep in extraction ("<...>/SKILL.md
    // already exists"); pre-empt that with one clean repo-level message naming the
    // two ways forward (T6.2). The anchor is the init-written harness.yml -- the
    // completed-init marker -- not mere `.maestro` existence, which a partial or
    // interrupted init could leave behind.
    if !options.merge && !options.force && paths.harness_dir().join("harness.yml").exists() {
        bail!(
            "maestro is already initialized in {}; use --force to refresh or --merge to fill gaps",
            paths.repo_root().display()
        );
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

    // Merge preserves existing folders without comparing versions, so a merged
    // project can be left behind this binary's shipped resources. Surface that
    // by counting the folders an Update-mode resync would refresh. Create/Force
    // just wrote everything current, so only merge can leave drift.
    let behind = if options.merge {
        preview_all(
            &paths,
            ExtractMode::Update {
                backup_timestamp: "",
            },
        )?
        .iter()
        .filter(|folder| folder.decision == FolderDecision::Refresh)
        .count()
    } else {
        0
    };

    Ok(InitOutcome::Applied {
        behind,
        root: paths.repo_root().to_path_buf(),
    })
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
            directories: vec![paths.harness_dir(), paths.cards_dir()],
            files: vec![InitFile {
                path: paths.harness_dir().join("harness.yml"),
                contents: harness_yml(&harness_config)?,
            }],
        })
    }
}

/// Render the dry-run artifact tree, then the bundled-resource extraction
/// preview (the hook script, the harness) the same run would apply.
pub fn render_dry_run(plan: &InitPlan, preview: &[FolderPreview]) -> String {
    let mut out = String::from("maestro init would create:\n");
    for directory in &plan.directories {
        out.push_str(&format!("dir  {}\n", directory.display()));
    }
    for file in &plan.files {
        out.push_str(&format!("file {}\n", file.path.display()));
    }
    out.push_str(&render_preview(preview));
    out.push_str("after init:\n");
    out.push_str("  run: maestro init --yes\n");
    out.push_str("  check setup: maestro doctor\n");
    out.push_str("  resume: maestro status\n");
    out.push_str(
        "safety: dry-run writes nothing; --yes keeps existing files; --force backs up first\n",
    );
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
