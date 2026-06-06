use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::domain::decisions;
use crate::domain::feature;
use crate::domain::install::{InstallLock, InstallState, MirrorKind};
use crate::domain::task;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::schema::{
    BACKLOG_SCHEMA_VERSION, Compat, HARNESS_SCHEMA_VERSION, classify,
};
use crate::harness::schema::{BacklogConfig, HarnessConfig};

/// Execute `maestro doctor`.
pub fn run() -> Result<()> {
    let report = match discover_repo_root() {
        Ok(repo_root) => {
            let paths = MaestroPaths::new(repo_root);
            doctor_report(&paths)?
        }
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::RepoRootNotFound { .. })
            ) =>
        {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            DoctorReport::not_initialized(cwd)
        }
        Err(error) => return Err(error),
    };

    for check in &report.checks {
        println!("check {}: ok ({})", check.name, check.detail);
    }
    for warning in &report.warnings {
        println!("warning: {warning}");
    }

    if report.errors.is_empty() {
        println!("doctor: ok");
        print_ok_handoff(&report);
        return Ok(());
    }

    for error in &report.errors {
        eprintln!("error: {error}");
    }
    bail!("doctor found {} error(s)", report.errors.len())
}

fn print_ok_handoff(report: &DoctorReport) {
    if report.checks.iter().any(|check| check.name == "install") {
        println!("next: maestro status");
    } else {
        println!(
            "next: maestro install --agent {}",
            super::detected_agent_hint()
        );
        println!("then: maestro status");
    }
}

#[derive(Debug)]
struct DoctorReport {
    checks: Vec<DoctorCheck>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl DoctorReport {
    fn not_initialized(cwd: PathBuf) -> Self {
        Self {
            checks: Vec::new(),
            warnings: Vec::new(),
            errors: vec![format!(
                "{} is not initialized for Maestro; run `maestro init --yes` to create .maestro",
                cwd.display()
            )],
        }
    }
}

#[derive(Debug)]
struct DoctorCheck {
    name: &'static str,
    detail: String,
}

fn doctor_report(paths: &MaestroPaths) -> Result<DoctorReport> {
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    if paths.maestro_dir().is_dir() {
        checks.push(DoctorCheck {
            name: "maestro-dir",
            detail: ".maestro present".to_string(),
        });
    } else {
        errors.push(missing_resource(&paths.maestro_dir()));
    }

    check_harness(paths, &mut checks, &mut errors);
    check_features(paths, &mut checks, &mut errors);
    check_backlog(paths, &mut checks, &mut errors);
    check_decisions(paths, &mut checks, &mut warnings, &mut errors);
    check_install(paths, &mut checks, &mut errors);

    // Collect a corrupt-task error into the report rather than aborting via `?`: a
    // single malformed task.yaml must not suppress every other doctor check, and
    // its full cause should surface like the other corrupt-artifact diagnostics.
    match task::check_blocker_graph(&paths.tasks_dir()) {
        Ok(task_report) if task_report.is_ok() => checks.push(DoctorCheck {
            name: "task-blockers",
            detail: format!("{} tasks scanned", task_report.tasks_scanned),
        }),
        Ok(task_report) => errors.extend(task_report.errors),
        Err(error) => errors.push(format!("{error:#}")),
    }

    Ok(DoctorReport {
        checks,
        warnings,
        errors,
    })
}

/// One vocabulary for every "a scaffolded resource is gone" error: the `{path}
/// is missing` phrasing the directory checks already use, plus the repair the
/// recorder check already names. `init --merge` restores any deleted piece
/// (verified: harness.yml, backlog.yaml, the features/decisions dirs, and a
/// fully-removed `.maestro`), so the hint is honest at every site.
fn missing_resource(path: &std::path::Path) -> String {
    format!(
        "{} is missing; run `maestro init --merge` to repair",
        path.display()
    )
}

fn check_harness(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let path = paths.harness_dir().join("harness.yml");
    if !path.exists() {
        errors.push(missing_resource(&path));
        return;
    }
    match read_yaml::<HarnessConfig>(&path) {
        Ok(config) if classify(&config.schema_version, HARNESS_SCHEMA_VERSION) == Compat::Exact => {
            checks.push(DoctorCheck {
                name: "harness",
                detail: format!("schema {}", config.schema_version),
            })
        }
        Ok(config) => errors.push(schema_diagnostic(
            &path,
            HARNESS_SCHEMA_VERSION,
            &config.schema_version,
        )),
        Err(error) => errors.push(format!("{error:#}")),
    }
}

fn check_features(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    // diagnose returns "{dir} is missing" for an absent dir and a scan error for a
    // present-but-corrupt one; only the former is `init --merge`-repairable, so
    // catch the missing dir here (mirroring check_decisions) and leave scan errors
    // to surface unchanged.
    let dir = paths.features_dir();
    if !dir.is_dir() {
        errors.push(missing_resource(&dir));
        return;
    }
    match feature::diagnose(paths).found {
        Ok(count) => checks.push(DoctorCheck {
            name: "features",
            detail: format!("{count} feature(s)"),
        }),
        Err(error) => errors.push(error),
    }
}

fn check_backlog(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let path = paths.harness_dir().join("backlog.yaml");
    if !path.exists() {
        errors.push(missing_resource(&path));
        return;
    }
    match read_yaml::<BacklogConfig>(&path) {
        Ok(backlog)
            if classify(&backlog.schema_version, BACKLOG_SCHEMA_VERSION) == Compat::Exact =>
        {
            checks.push(DoctorCheck {
                name: "backlog",
                detail: format!("{} item(s)", backlog.items.len()),
            });
        }
        Ok(backlog) => errors.push(schema_diagnostic(
            &path,
            BACKLOG_SCHEMA_VERSION,
            &backlog.schema_version,
        )),
        Err(error) => errors.push(format!("{error:#}")),
    }
}

/// Build a doctor diagnostic for a schema gap: an exact match is reported ok;
/// any other version is incompatible (stop). This is a clean-rewrite binary
/// with no migration path.
fn schema_diagnostic(path: &std::path::Path, expected: &str, found: &str) -> String {
    match classify(found, expected) {
        Compat::Exact => format!("{} schema ok ({expected})", path.display()),
        Compat::Incompatible => format!(
            "{} schema incompatible: expected {expected}, found {found}",
            path.display()
        ),
    }
}

fn check_decisions(
    paths: &MaestroPaths,
    checks: &mut Vec<DoctorCheck>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let dir = paths.decisions_dir();
    if !dir.is_dir() {
        errors.push(missing_resource(&dir));
        return;
    }

    let report = decisions::diagnose(paths);
    warnings.extend(report.warnings);
    errors.extend(report.errors);
    checks.push(DoctorCheck {
        name: "decisions",
        detail: format!(
            "{} structured decision(s), {} legacy file(s)",
            report.structured_count, report.legacy_count
        ),
    });
}

/// Verify that the files an installed agent owns still exist on disk. A bare
/// `init` with no integration installed has no committed agents, so this is a
/// no-op there and `doctor` stays ok; once an agent is installed, a deleted
/// CLAUDE.md / codex hook config / skill symlink / record.sh is reported as an
/// error (T4).
fn check_install(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let lock = match InstallLock::load(&paths.install_lock_file()) {
        Ok(lock) => lock,
        Err(error) => {
            // Surface the full parse cause for a corrupt install lock, matching the
            // other corrupt-artifact diagnostics; a missing lock is handled upstream.
            errors.push(format!("{error:#}"));
            return;
        }
    };

    let committed = lock
        .agents
        .iter()
        .filter(|(_, install)| install.state == InstallState::Committed)
        .collect::<Vec<_>>();
    if committed.is_empty() {
        // init'd but never installed (or only pending/removing): nothing to verify.
        return;
    }

    let root = paths.repo_root();
    let mut missing = 0_usize;
    for (agent, install) in &committed {
        for (relative, ownership) in &install.files {
            let path = root.join(relative);
            let intact = match ownership.kind {
                // A managed symlink must exist and resolve to a live target.
                MirrorKind::Symlink => std::fs::symlink_metadata(&path).is_ok() && path.exists(),
                // Every other mirror lives inside a real file on disk.
                _ => path.exists(),
            };
            if !intact {
                errors.push(format!(
                    "{agent} mirror is missing or broken: {relative}; run `maestro install --agent {agent}` to repair"
                ));
                missing += 1;
            }
        }
    }

    // The hook recorder lives in the extraction manifest, not the lock, but
    // install always extracts it and every installed hook entry runs it, so a
    // committed agent implies it must be present.
    let recorder = paths.hooks_dir().join("record.sh");
    if !recorder.exists() {
        errors.push(format!(
            "hook recorder is missing: {}; run `maestro init --merge` to repair",
            recorder.display()
        ));
        missing += 1;
    }

    if missing == 0 {
        checks.push(DoctorCheck {
            name: "install",
            detail: format!("{} agent(s) intact", committed.len()),
        });
    }
}

fn read_yaml<T>(path: &std::path::Path) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}
