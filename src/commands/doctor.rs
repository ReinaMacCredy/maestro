use anyhow::{bail, Context, Result};

use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::schema::{BACKLOG_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION};
use crate::feature::schema::FeatureRegistry;
use crate::harness::schema::{BacklogConfig, HarnessConfig};
use crate::task::doctor::check_blocker_graph;

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

    let task_report = check_blocker_graph(&paths.tasks_dir())?;
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
        Ok(config) if config.schema_version == HARNESS_SCHEMA_VERSION => checks.push(DoctorCheck {
            name: "harness",
            detail: path.display().to_string(),
        }),
        Ok(config) => errors.push(format!(
            "{} schema mismatch: expected {}, found {}",
            path.display(),
            HARNESS_SCHEMA_VERSION,
            config.schema_version
        )),
        Err(error) => errors.push(error.to_string()),
    }
}

fn check_features(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let path = paths.features_dir().join("features.yaml");
    match read_yaml::<FeatureRegistry>(&path) {
        Ok(registry) if registry.schema_version == FEATURE_SCHEMA_VERSION => {
            checks.push(DoctorCheck {
                name: "features",
                detail: format!("{} feature(s)", registry.features.len()),
            });
        }
        Ok(registry) => errors.push(format!(
            "{} schema mismatch: expected {}, found {}",
            path.display(),
            FEATURE_SCHEMA_VERSION,
            registry.schema_version
        )),
        Err(error) => errors.push(error.to_string()),
    }
}

fn check_backlog(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let path = paths.harness_dir().join("backlog.yaml");
    match read_yaml::<BacklogConfig>(&path) {
        Ok(backlog) if backlog.schema_version == BACKLOG_SCHEMA_VERSION => {
            checks.push(DoctorCheck {
                name: "backlog",
                detail: format!("{} item(s)", backlog.items.len()),
            });
        }
        Ok(backlog) => errors.push(format!(
            "{} schema mismatch: expected {}, found {}",
            path.display(),
            BACKLOG_SCHEMA_VERSION,
            backlog.schema_version
        )),
        Err(error) => errors.push(error.to_string()),
    }
}

fn check_decisions(paths: &MaestroPaths, checks: &mut Vec<DoctorCheck>, errors: &mut Vec<String>) {
    let dir = paths.decisions_dir();
    if !dir.is_dir() {
        errors.push(format!("{} is missing", dir.display()));
        return;
    }

    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            let count = entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.path().is_file()
                        && entry
                            .file_name()
                            .to_str()
                            .map(|name| name.starts_with("decision-") && name.ends_with(".md"))
                            .unwrap_or(false)
                })
                .count();
            checks.push(DoctorCheck {
                name: "decisions",
                detail: format!("{count} decision file(s)"),
            });
        }
        Err(error) => errors.push(format!("failed to read {}: {error}", dir.display())),
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
