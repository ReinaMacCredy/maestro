use anyhow::{Context, Result, bail};

use crate::domain::decisions;
use crate::domain::feature;
use crate::domain::install::{InstallLock, InstallState, MirrorKind};
use crate::domain::task;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::schema::{
    BACKLOG_SCHEMA_VERSION, Compat, HARNESS_SCHEMA_VERSION, classify,
};
use crate::harness::schema::{BacklogConfig, HarnessConfig};

/// Execute `maestro doctor`.
pub fn run() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let report = doctor_report(&paths)?;

    for check in &report.checks {
        println!("check {}: ok ({})", check.name, check.detail);
    }

    if report.errors.is_empty() {
        println!("doctor: ok");
        return Ok(());
    }

    for error in &report.errors {
        eprintln!("error: {error}");
    }
    bail!("doctor found {} error(s)", report.errors.len())
}

#[derive(Debug)]
struct DoctorReport {
    checks: Vec<DoctorCheck>,
    errors: Vec<String>,
}

#[derive(Debug)]
struct DoctorCheck {
    name: &'static str,
    detail: String,
}

fn doctor_report(paths: &MaestroPaths) -> Result<DoctorReport> {
    let mut checks = Vec::new();
    let mut errors = Vec::new();

    if paths.maestro_dir().is_dir() {
        checks.push(DoctorCheck {
            name: "maestro-dir",
            detail: ".maestro present".to_string(),
        });
    } else {
        errors.push(format!("{} is missing", paths.maestro_dir().display()));
    }

    check_harness(paths, &mut checks, &mut errors);
    check_features(paths, &mut checks, &mut errors);
    check_backlog(paths, &mut checks, &mut errors);
    check_decisions(paths, &mut checks, &mut errors);
    check_install(paths, &mut checks, &mut errors);

    let task_report = task::check_blocker_graph(&paths.tasks_dir())?;
    if task_report.is_ok() {
        checks.push(DoctorCheck {
            name: "task-blockers",
            detail: format!("{} tasks scanned", task_report.tasks_scanned),
        });
    } else {
        errors.extend(task_report.errors);
    }

    Ok(DoctorReport { checks, errors })
}

fn check_harness(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let path = paths.harness_dir().join("harness.yml");
    match read_yaml::<HarnessConfig>(&path) {
        Ok(config) if classify(&config.schema_version, HARNESS_SCHEMA_VERSION) == Compat::Exact => {
            checks.push(DoctorCheck {
                name: "harness",
                detail: path.display().to_string(),
            })
        }
        Ok(config) => errors.push(schema_diagnostic(
            &path,
            HARNESS_SCHEMA_VERSION,
            &config.schema_version,
        )),
        Err(error) => errors.push(error.to_string()),
    }
}

fn check_features(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
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
        Err(error) => errors.push(error.to_string()),
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

fn check_decisions(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let dir = paths.decisions_dir();
    if !dir.is_dir() {
        errors.push(format!("{} is missing", dir.display()));
        return;
    }

    match decisions::decision_entries(&dir) {
        Ok(entries) => checks.push(DoctorCheck {
            name: "decisions",
            detail: format!("{} decision file(s)", entries.len()),
        }),
        Err(error) => errors.push(format!("{error:#}")),
    }
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
            errors.push(error.to_string());
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
                MirrorKind::Symlink => {
                    std::fs::symlink_metadata(&path).is_ok() && path.exists()
                }
                // Every other mirror lives inside a real file on disk.
                _ => path.exists(),
            };
            if !intact {
                errors.push(format!(
                    "{agent} mirror is missing or broken: {relative}; run `maestro init` to repair"
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
            "hook recorder is missing: {}; run `maestro init` to repair",
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
