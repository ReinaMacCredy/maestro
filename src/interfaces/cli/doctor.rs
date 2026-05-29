use anyhow::{bail, Context, Result};

use crate::domain::decisions;
use crate::domain::feature;
use crate::domain::task;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::foundation::core::schema::{
    classify, Compat, BACKLOG_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION,
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
    let diagnostic = feature::diagnose(paths);
    match (diagnostic.found, diagnostic.compatibility) {
        (Ok((_, count)), Some(Compat::Exact)) => checks.push(DoctorCheck {
            name: "features",
            detail: format!("{count} feature(s)"),
        }),
        (Ok((found, _)), Some(Compat::NeedsMigration)) => errors.push(format!(
            "features schema needs migration: expected {}, found {found}; run `maestro migrate`",
            diagnostic.expected
        )),
        (Ok((found, _)), _) => errors.push(format!(
            "features schema incompatible: expected {}, found {found}",
            diagnostic.expected
        )),
        (Err(error), _) => errors.push(error),
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

/// Build a doctor diagnostic for a schema gap, routing the message by
/// classification: a migratable gap points at `maestro migrate`; anything
/// unknown is reported as incompatible (stop).
fn schema_diagnostic(path: &std::path::Path, expected: &str, found: &str) -> String {
    match classify(found, expected) {
        Compat::Exact => format!("{} schema ok ({expected})", path.display()),
        Compat::NeedsMigration => format!(
            "{} schema needs migration: expected {expected}, found {found}; run `maestro migrate`",
            path.display()
        ),
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

fn read_yaml<T>(path: &std::path::Path) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}
