use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card;
use crate::domain::decisions;
use crate::domain::feature;
use crate::domain::harness::HarnessConfig;
use crate::domain::install::{InstallLock, InstallState, MirrorKind};
use crate::domain::run;
use crate::domain::skills;
use crate::domain::task;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{ALLOC_MARKER_PREFIX, child_dirs};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::schema::{Compat, HARNESS_SCHEMA_VERSION, classify};
use crate::operations::harness;

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

    // One walk of the card store backs every card-typed check below (features,
    // backlog, decisions, task blockers). A card that fails to load has an
    // unknowable type, so it is reported here exactly once instead of once per
    // per-type scan; the typed checks then see only the loadable cards.
    let scan = match card::query::scan_with_failures(paths) {
        Ok(scan) => {
            for failure in &scan.failures {
                errors.push(failure.error.clone());
            }
            Some(scan)
        }
        Err(error) => {
            errors.push(format!("{error:#}"));
            None
        }
    };

    if let Some(scan) = &scan {
        check_features(paths, &scan.cards, &mut checks, &mut warnings, &mut errors);
        check_archive_backlog(&scan.cards, &mut checks, &mut warnings);
        check_backlog(&scan.cards, &mut checks, &mut errors);
        check_decisions(paths, &scan.cards, &mut checks, &mut warnings, &mut errors);
        warnings.extend(card::query::integrity_warnings(paths, &scan.cards));
        warnings.extend(card::query::unknown_field_warnings(paths, &scan.cards));
    }
    check_install(paths, &mut checks, &mut errors);
    check_global_skills(&mut checks, &mut warnings, &mut errors);

    match recordless_task_dir_warnings(paths) {
        Ok(found) => warnings.extend(found),
        Err(error) => errors.push(format!("{error:#}")),
    }
    if let Some(scan) = &scan {
        // Collect a corrupt-task error into the report rather than aborting via
        // `?`: a single malformed task record must not suppress every other
        // doctor check, and its full cause should surface like the other
        // corrupt-artifact diagnostics.
        match task::check_blocker_graph_in_cards(paths, &scan.cards) {
            Ok(task_report) if task_report.is_ok() => checks.push(DoctorCheck {
                name: "task-blockers",
                detail: format!("{} tasks scanned", task_report.tasks_scanned),
            }),
            Ok(task_report) => errors.extend(task_report.errors),
            Err(error) => errors.push(format!("{error:#}")),
        }
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
/// (verified: harness.yml and a fully-removed `.maestro`), so the hint is
/// honest at every site.
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

fn check_features(
    paths: &MaestroPaths,
    cards: &[(card::schema::Card, PathBuf)],
    checks: &mut Vec<DoctorCheck>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    // Feature cards live in the flat card store, so there is no per-entity
    // `features/` directory to require. The recordless-dir sweep still catches a
    // legacy ghost dir left behind by a brownfield migration (it reads as empty
    // when the dir is absent), and `diagnose` counts feature cards from the store.
    match recordless_dir_warnings(paths.repo_root(), &paths.features_dir(), "feature.yaml") {
        Ok(found) => warnings.extend(found),
        Err(error) => errors.push(format!("{error:#}")),
    }
    match feature::diagnose(cards).found {
        Ok(count) => checks.push(DoctorCheck {
            name: "features",
            detail: format!("{count} feature(s)"),
        }),
        Err(error) => errors.push(error),
    }
}

/// Closed features still in the live store are an archive backlog: the lid in
/// `.maestro/archive/cards/INDEX.md` only remembers what gets archived, so the
/// backlog is surfaced as an advisory (never an error -- nothing is broken).
fn check_archive_backlog(
    cards: &[(card::schema::Card, PathBuf)],
    checks: &mut Vec<DoctorCheck>,
    warnings: &mut Vec<String>,
) {
    let closed = cards
        .iter()
        .filter(|(card, _)| {
            // The same predicate `feature archive --closed` sweeps with, so the
            // advisory count and the sweep's reach cannot drift apart.
            card.card_type == card::schema::CardType::Feature
                && feature::FeatureStatus::parse(&card.status)
                    .is_some_and(|status| status.is_terminal())
        })
        .count();
    if closed == 0 {
        checks.push(DoctorCheck {
            name: "archive",
            detail: "no closed features awaiting archive".to_string(),
        });
    } else {
        warnings.push(format!(
            "{closed} closed feature(s) not archived; sweep with `maestro feature archive --closed`"
        ));
    }
}

fn recordless_task_dir_warnings(paths: &MaestroPaths) -> Result<Vec<String>> {
    // Task cards live in the flat card store; like the features sweep above,
    // this only catches a legacy ghost `tasks/` dir left behind by a brownfield
    // migration (it reads as empty when the dir is absent).
    recordless_dir_warnings(paths.repo_root(), &paths.tasks_dir(), "task.yaml")
}

fn recordless_dir_warnings(
    repo_root: &Path,
    root: &Path,
    record_name: &str,
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    for (path, _) in child_dirs(root)? {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(ALLOC_MARKER_PREFIX))
        {
            continue;
        }
        if path.join(record_name).is_file() {
            continue;
        }
        let display_path = display_relative(repo_root, &path);
        warnings.push(format!(
            "{display_path} has no {record_name} (likely an aborted create); remove it: rm -r {display_path}"
        ));
    }
    warnings.sort();
    Ok(warnings)
}

fn display_relative(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn check_backlog(
    cards: &[(card::schema::Card, PathBuf)],
    checks: &mut Vec<DoctorCheck>,
    errors: &mut Vec<String>,
) {
    // The backlog has no file of its own (D7): items live as idea cards, so the
    // check counts them through the same conversion every harness verb uses.
    match harness::load_backlog_in_cards(cards) {
        Ok(backlog) => {
            checks.push(DoctorCheck {
                name: "backlog",
                detail: format!("{} item(s)", backlog.items.len()),
            });
        }
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
    cards: &[(card::schema::Card, PathBuf)],
    checks: &mut Vec<DoctorCheck>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    // Decisions are decision-typed cards in the flat store (plus any frozen
    // legacy markdown), so there is no `decisions/` directory to require;
    // `diagnose` and the dangling-ref scan both read an absent legacy dir as
    // empty.
    let report = decisions::diagnose(paths, cards);
    warnings.extend(report.warnings);
    warnings.extend(decisions::dangling_reference_warnings(paths, cards));
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
/// CLAUDE.md / codex hook config / record.sh is reported as an error (T4).
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
    let recorder_script = run::hook_event_contract().script();
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
                continue;
            }
            // A present settings file is not enough: its maestro-managed hook
            // entries can be stripped while the file lives on (e.g. a user edit
            // that drops the `hooks` key), which silently darkens run-event
            // recording. For the JSON mirror that owns `hooks`, confirm the
            // recorder script is still wired into the file's hook entries.
            if ownership.kind == MirrorKind::JsonManagedKeys
                && ownership.managed_keys.iter().any(|key| key == "hooks")
                && !hook_entries_wired(&path, recorder_script)
            {
                errors.push(format!(
                    "{agent} hook entries are missing from {relative} (run-event recording is off); run `maestro install --agent {agent}` to repair"
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

/// True when the agent's settings file still wires the recorder script into its
/// hook entries. Reads the `hooks` value as opaque JSON and looks for a
/// reference to the script name (e.g. the `record.sh` in the installed
/// `sh "$CLAUDE_PROJECT_DIR/.maestro/hooks/record.sh"` command). A file that no
/// longer parses, has no `hooks`, or whose `hooks` never names the recorder is
/// treated as not wired -- run-event recording would be dark either way.
fn hook_entries_wired(path: &Path, recorder_script: &str) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    value
        .get("hooks")
        .map(|entry| entry.to_string().contains(recorder_script))
        .unwrap_or(false)
}

/// Compare the user-level global skill cache against the skills embedded in
/// this binary. Silent when the user never adopted global skills (no lock);
/// drift is a warning rather than an error because agents resolve repo-local
/// skills first, so a stale cache degrades discovery without breaking it.
fn check_global_skills(
    checks: &mut Vec<DoctorCheck>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let status = match skills::global_skills_status() {
        Ok(Some(status)) => status,
        Ok(None) => return,
        Err(error) => {
            errors.push(format!("{error:#}"));
            return;
        }
    };

    if status.stale.is_empty() && status.retired.is_empty() {
        checks.push(DoctorCheck {
            name: "skills",
            detail: format!("{} global skill(s) match binary", status.matched),
        });
        return;
    }

    for drift in &status.stale {
        let installed = drift.installed.as_deref().unwrap_or("missing");
        let embedded = drift.embedded.as_deref().unwrap_or("unversioned");
        warnings.push(format!(
            "global skill {} is {installed} in the cache, binary ships {embedded}; run `maestro sync --global-skills`",
            drift.name
        ));
    }
    if !status.retired.is_empty() {
        warnings.push(format!(
            "{} retired global skill(s) linger in the cache ({}); run `maestro sync --global-skills`",
            status.retired.len(),
            status.retired.join(", ")
        ));
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
