//! Task verification execution helpers.

use std::collections::BTreeSet;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::events::managed_event_files;
use super::stale::{FreshnessInputs, StoredFreshness};
use crate::domain::harness::HarnessConfig;
use crate::domain::task::{self, AcceptanceFile, TaskRecord, TaskState, VerificationBinding};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::git;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{EVENT_SCHEMA_VERSION, VERIFICATION_SCHEMA_VERSION};

static ATTEMPT_COUNTER: AtomicU64 = AtomicU64::new(0);
const EVENT_PROOF_SOURCE_KIND: &str = "event";
const LATEST_ATTEMPT_REPORT_FILE: &str = "latest.json";
const MAX_STORED_ATTEMPT_REPORTS: usize = 20;
const CANONICAL_REPORT_RESTORE_FILE: &str = "verification.json.restore";
const CANONICAL_REPORT_RESTORE_SCHEMA: &str = "maestro.verification.restore.v1";

/// High-level result returned by the Proof facade after task verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskVerification {
    pub task_id: String,
    pub status: TaskVerificationStatus,
    pub claim_count: usize,
    pub proof_source_count: usize,
    pub failures: Vec<String>,
}

/// High-level pass/fail status returned by the Proof facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskVerificationStatus {
    Passed,
    Failed,
}

/// Result status written to verification reports.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
}

/// Per-claim evidence matching result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClaimCheck {
    pub claim: String,
    pub matched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Proof source considered during verification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProofSource {
    pub kind: String,
    pub path: String,
    pub hash: String,
}

/// One verification command result captured from `harness.yml.verify`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationCommand {
    pub cmd: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

/// Verification artifact persisted in a task directory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationReport {
    pub schema_version: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_snapshot: Option<VerificationTaskSnapshot>,
    pub status: VerificationStatus,
    pub verified_at: String,
    #[serde(flatten)]
    pub freshness: StoredFreshness,
    pub claims: Vec<ClaimCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<VerificationCommand>,
    pub proof_sources: Vec<ProofSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
}

/// Task snapshot identity used when Proof evaluated a report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationTaskSnapshot {
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CanonicalReportRestoreJournal {
    schema_version: String,
    previous: Option<String>,
}

/// Paths and records for a loaded task.
#[derive(Clone, Debug)]
pub struct LoadedTask {
    pub task: TaskRecord,
    pub task_dir: PathBuf,
}

/// Evaluate task proof and persist the non-canonical attempt report.
pub(crate) fn evaluate_and_write_task_report_attempt(
    paths: &MaestroPaths,
    task: &TaskRecord,
    task_dir: &Path,
    verified_at: &str,
) -> Result<VerificationReport> {
    let report = evaluate_task_report(paths, task, task_dir, verified_at)?;
    write_task_report_attempt(task_dir, &report)?;
    Ok(report)
}

/// Evaluate task proof and return the report that should be written.
pub(crate) fn evaluate_task_report(
    paths: &MaestroPaths,
    task: &TaskRecord,
    task_dir: &Path,
    verified_at: &str,
) -> Result<VerificationReport> {
    let commands = run_verify_commands(paths)?;
    let inputs =
        freshness_inputs_for_task(task, task_dir, git::head(paths.repo_root()).unwrap_or(None))?;
    let claims = completion_claims(task);
    let evidence = collect_evidence(paths, task_dir, &task.id)?;
    let claim_checks = check_claims(&claims, &evidence);
    let failures = failures_for(task, &claims, &claim_checks, &evidence, &commands);
    let status = if failures.is_empty() {
        VerificationStatus::Passed
    } else {
        VerificationStatus::Failed
    };

    let report = VerificationReport {
        schema_version: VERIFICATION_SCHEMA_VERSION.to_string(),
        task_id: task.id.clone(),
        attempt_id: Some(new_attempt_id(task, verified_at)),
        task_snapshot: Some(VerificationTaskSnapshot {
            updated_at: task.updated_at.clone(),
        }),
        status,
        verified_at: verified_at.to_string(),
        freshness: StoredFreshness {
            verified_commit: inputs.commit.clone(),
            task_contract_hash: inputs.task_contract_hash,
            acceptance_hash: inputs.acceptance_hash,
            checks_hash: inputs.checks_hash,
        },
        claims: claim_checks,
        commands,
        proof_sources: evidence
            .iter()
            .map(|source| ProofSource {
                kind: source.kind.clone(),
                path: display_path(paths.repo_root(), &source.path),
                hash: hash_bytes(source.text.as_bytes()),
            })
            .collect(),
        failures,
    };

    Ok(report)
}

impl TaskVerification {
    pub(crate) fn from_report(report: &VerificationReport) -> Self {
        Self {
            task_id: report.task_id.clone(),
            status: match report.status {
                VerificationStatus::Passed => TaskVerificationStatus::Passed,
                VerificationStatus::Failed => TaskVerificationStatus::Failed,
            },
            claim_count: report.claims.len(),
            proof_source_count: report.proof_sources.len(),
            failures: report.failures.clone(),
        }
    }
}

/// Load a task by id or id prefix directory name.
pub fn load_task_by_id(paths: &MaestroPaths, task_id: &str) -> Result<LoadedTask> {
    let handle = task::load_task_for_update(&paths.tasks_dir(), task_id)?;

    Ok(LoadedTask {
        task: handle.task().clone(),
        task_dir: handle.task_dir().to_path_buf(),
    })
}

/// Return the path to the verification artifact for a loaded task.
pub fn verification_path(task_dir: &Path) -> PathBuf {
    task_dir.join("verification.json")
}

/// Return the directory that stores non-canonical verification attempts.
pub(crate) fn verification_attempts_dir(task_dir: &Path) -> PathBuf {
    task_dir.join("verification.attempts")
}

/// Read `verification.json` when it exists.
pub fn read_report(task_dir: &Path) -> Result<Option<VerificationReport>> {
    let path = verification_path(task_dir);
    read_managed_report_file_if_exists(&path)
}

/// Compute current proof freshness inputs for a loaded task.
pub fn freshness_inputs(paths: &MaestroPaths, loaded: &LoadedTask) -> Result<FreshnessInputs> {
    let commit = git::head(paths.repo_root()).unwrap_or(None);
    freshness_inputs_for_task(&loaded.task, &loaded.task_dir, commit)
}

/// Compute current proof freshness inputs for a task artifact directory.
pub fn freshness_inputs_for_task(
    task: &TaskRecord,
    task_dir: &Path,
    commit: Option<String>,
) -> Result<FreshnessInputs> {
    let (acceptance, acceptance_hash) = load_acceptance_with_hash(task_dir)?;

    Ok(FreshnessInputs {
        commit,
        task_contract_hash: task_contract_hash(task),
        acceptance_hash,
        checks_hash: checks_hash(&acceptance),
    })
}

pub(crate) fn verification_outcome_for_report(
    report: &VerificationReport,
) -> Result<task::VerificationOutcome> {
    let receipt = applied_receipt_for_report(report)
        .context("verification report missing task snapshot identity")?;
    match report.status {
        VerificationStatus::Passed => Ok(task::VerificationOutcome::Passed(
            task::VerificationPassed {
                binding: verification_binding_for_report(report),
                receipt,
                summary: report_summary(report),
            },
        )),
        VerificationStatus::Failed => Ok(task::VerificationOutcome::Failed(
            task::VerificationFailed {
                receipt,
                summary: report_summary(report),
                failures: report.failures.clone(),
            },
        )),
    }
}

pub(crate) fn applied_receipt_for_report(
    report: &VerificationReport,
) -> Option<task::AppliedVerificationReceipt> {
    report
        .task_snapshot
        .as_ref()
        .map(|snapshot| task::AppliedVerificationReceipt {
            task_snapshot_updated_at: snapshot.updated_at.clone(),
            verified_at: report.verified_at.clone(),
            attempt_id: report.attempt_id.clone(),
        })
}

pub(crate) fn verification_binding_for_report(report: &VerificationReport) -> VerificationBinding {
    VerificationBinding {
        verified_at: Some(report.verified_at.clone()),
        verified_commit: report.freshness.verified_commit.clone(),
        verified_by_run: event_source_path(report),
        task_contract_hash: Some(report.freshness.task_contract_hash.clone()),
        acceptance_hash: Some(report.freshness.acceptance_hash.clone()),
        checks_hash: Some(report.freshness.checks_hash.clone()),
        applied_report: None,
    }
}

pub(crate) fn passed_binding_matches_report(
    binding: &VerificationBinding,
    report: &VerificationReport,
) -> bool {
    binding.verified_at.as_deref() == Some(report.verified_at.as_str())
        && binding.verified_commit == report.freshness.verified_commit
        && binding.verified_by_run == event_source_path(report)
        && binding.task_contract_hash.as_deref()
            == Some(report.freshness.task_contract_hash.as_str())
        && binding.acceptance_hash.as_deref() == Some(report.freshness.acceptance_hash.as_str())
        && binding.checks_hash.as_deref() == Some(report.freshness.checks_hash.as_str())
}

pub(crate) fn report_summary(report: &VerificationReport) -> String {
    match report.status {
        VerificationStatus::Passed => format!(
            "verification passed: {} claim(s), {} proof source(s)",
            report.claims.len(),
            report.proof_sources.len()
        ),
        VerificationStatus::Failed => {
            let first = report
                .failures
                .first()
                .map(String::as_str)
                .unwrap_or("unknown verification failure");
            format!("verification failed: {first}")
        }
    }
}

pub(crate) fn write_task_report(task_dir: &Path, report: &VerificationReport) -> Result<()> {
    let path = verification_path(task_dir);
    write_report_file(&path, report)
}

pub(crate) fn write_task_report_attempt(
    task_dir: &Path,
    report: &VerificationReport,
) -> Result<PathBuf> {
    let attempts_dir = managed_attempts_dir(task_dir)?;
    let path = attempts_dir.join(format!("{}.json", report_file_stem(report)));
    write_report_file(&path, report)?;
    write_report_file(&attempts_dir.join(LATEST_ATTEMPT_REPORT_FILE), report)?;
    prune_old_attempt_reports(&attempts_dir)?;
    Ok(path)
}

fn prune_old_attempt_reports(attempts_dir: &Path) -> Result<()> {
    let entries = fs::read_dir(attempts_dir)
        .with_context(|| format!("failed to read {}", attempts_dir.display()))?;
    let mut attempts = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", attempts_dir.display()))?;
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(LATEST_ATTEMPT_REPORT_FILE) {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_file() && !file_type.is_symlink() {
            attempts.push(path);
        }
    }

    attempts.sort();
    let remove_count = attempts.len().saturating_sub(MAX_STORED_ATTEMPT_REPORTS);
    for path in attempts.into_iter().take(remove_count) {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to prune {}", path.display()));
            }
        }
    }
    Ok(())
}

fn existing_managed_attempts_dir(task_dir: &Path) -> Result<Option<PathBuf>> {
    let attempts_dir = verification_attempts_dir(task_dir);
    match fs::symlink_metadata(&attempts_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "managed verification attempts path must not be a symlink: {}",
                attempts_dir.display()
            );
        }
        Ok(metadata) if !metadata.is_dir() => {
            bail!(
                "managed verification attempts path must be a directory: {}",
                attempts_dir.display()
            );
        }
        Ok(_) => Ok(Some(attempts_dir)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect {}", attempts_dir.display()))
        }
    }
}

fn managed_attempts_dir(task_dir: &Path) -> Result<PathBuf> {
    let attempts_dir = verification_attempts_dir(task_dir);
    match existing_managed_attempts_dir(task_dir)? {
        Some(path) => Ok(path),
        None => {
            match fs::create_dir(&attempts_dir) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to create {}", attempts_dir.display()));
                }
            }
            match existing_managed_attempts_dir(task_dir)? {
                Some(path) => Ok(path),
                None => bail!(
                    "managed verification attempts path was not created: {}",
                    attempts_dir.display()
                ),
            }
        }
    }
}

pub(crate) fn replace_task_report_preserving_previous(
    task_dir: &Path,
    report: &VerificationReport,
) -> Result<CanonicalReportRestore> {
    let path = verification_path(task_dir);
    let journal_path = canonical_report_restore_path(task_dir);
    if read_canonical_report_restore_journal(task_dir)?.is_some() {
        bail!(
            "pending canonical verification report restore journal exists: {}",
            journal_path.display()
        );
    }
    let previous = read_managed_report_file_text_if_exists(&path)?;
    write_canonical_report_restore_journal(task_dir, previous.as_ref())?;
    write_task_report(task_dir, report)?;
    Ok(CanonicalReportRestore {
        path,
        journal_path,
        committed: false,
    })
}

fn read_managed_report_file_text_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "managed verification report path must not be a symlink: {}",
                path.display()
            );
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!(
                "managed verification report path must be a file: {}",
                path.display()
            );
        }
        Ok(_) => fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(Some),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

pub(crate) fn read_managed_report_file_if_exists(
    path: &Path,
) -> Result<Option<VerificationReport>> {
    let Some(raw) = read_managed_report_file_text_if_exists(path)? else {
        return Ok(None);
    };
    parse_report_file(path, &raw)
}

pub(crate) fn read_report_file_if_exists(path: &Path) -> Result<Option<VerificationReport>> {
    let Some(raw) = read_to_string_if_exists(path)? else {
        return Ok(None);
    };
    parse_report_file(path, &raw)
}

fn parse_report_file(path: &Path, raw: &str) -> Result<Option<VerificationReport>> {
    let report: VerificationReport =
        serde_json::from_str(raw).with_context(|| format!("failed to parse {}", path.display()))?;
    if report.schema_version != VERIFICATION_SCHEMA_VERSION {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            VERIFICATION_SCHEMA_VERSION,
            report.schema_version
        );
    }
    Ok(Some(report))
}

pub(crate) fn latest_attempt_report(
    task_dir: &Path,
) -> Result<Option<(VerificationReport, PathBuf)>> {
    let Some(attempts_dir) = existing_managed_attempts_dir(task_dir)? else {
        return Ok(None);
    };
    let marker_path = attempts_dir.join(LATEST_ATTEMPT_REPORT_FILE);
    let marker = read_managed_report_file_if_exists(&marker_path)?;

    let entries = fs::read_dir(&attempts_dir)
        .with_context(|| format!("failed to read {}", attempts_dir.display()))?;
    let mut latest_path = None;
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", attempts_dir.display()))?;
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(LATEST_ATTEMPT_REPORT_FILE) {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_file()
            && !file_type.is_symlink()
            && latest_path
                .as_ref()
                .map(|current: &PathBuf| path > *current)
                .unwrap_or(true)
        {
            latest_path = Some(path);
        }
    }
    let Some(path) = latest_path else {
        return Ok(marker.map(|report| (report, marker_path)));
    };
    let report = read_report_file_if_exists(&path)?
        .with_context(|| format!("missing verification attempt {}", path.display()))?;
    Ok(Some((report, path)))
}
pub(crate) struct CanonicalReportRestore {
    path: PathBuf,
    journal_path: PathBuf,
    committed: bool,
}

impl CanonicalReportRestore {
    pub(crate) fn commit(mut self) {
        self.committed = true;
        let _ = remove_canonical_report_restore_journal(&self.journal_path);
    }

    fn rollback_promoted_report(&mut self) -> Result<()> {
        if self.committed {
            return Ok(());
        }
        restore_canonical_report_from_journal(&self.path, &self.journal_path)?;
        self.committed = true;
        Ok(())
    }
}

impl task::template::SaveTaskHook for CanonicalReportRestore {
    fn commit(self) {
        CanonicalReportRestore::commit(self);
    }

    fn rollback(&mut self) -> Result<()> {
        self.rollback_promoted_report()
    }
}

impl Drop for CanonicalReportRestore {
    fn drop(&mut self) {
        let _ = self.committed;
    }
}

fn write_report_file(path: &Path, report: &VerificationReport) -> Result<()> {
    let raw = serde_json::to_string_pretty(report)?;
    write_string_atomic(path, &format!("{raw}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn recover_canonical_report_for_task(
    task: &TaskRecord,
    task_dir: &Path,
    report_reflected: impl Fn(&TaskRecord, &VerificationReport) -> bool,
) -> Result<()> {
    let Some(_) = read_canonical_report_restore_journal(task_dir)? else {
        return Ok(());
    };
    let path = verification_path(task_dir);
    match read_managed_report_file_if_exists(&path) {
        Ok(Some(report)) if report_reflected(task, &report) => {
            remove_canonical_report_restore_journal(&canonical_report_restore_path(task_dir))
        }
        _ => restore_canonical_report_from_journal(&path, &canonical_report_restore_path(task_dir)),
    }
}

fn canonical_report_restore_path(task_dir: &Path) -> PathBuf {
    task_dir.join(CANONICAL_REPORT_RESTORE_FILE)
}

fn write_canonical_report_restore_journal(
    task_dir: &Path,
    previous: Option<&String>,
) -> Result<()> {
    let journal = CanonicalReportRestoreJournal {
        schema_version: CANONICAL_REPORT_RESTORE_SCHEMA.to_string(),
        previous: previous.cloned(),
    };
    let raw = serde_json::to_string_pretty(&journal)?;
    write_string_atomic(canonical_report_restore_path(task_dir), &format!("{raw}\n"))
}

fn read_canonical_report_restore_journal(
    task_dir: &Path,
) -> Result<Option<CanonicalReportRestoreJournal>> {
    let path = canonical_report_restore_path(task_dir);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => bail!(
            "managed verification restore journal must not be a symlink: {}",
            path.display()
        ),
        Ok(metadata) if !metadata.is_file() => bail!(
            "managed verification restore journal must be a file: {}",
            path.display()
        ),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }

    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let journal: CanonicalReportRestoreJournal = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if journal.schema_version != CANONICAL_REPORT_RESTORE_SCHEMA {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            CANONICAL_REPORT_RESTORE_SCHEMA,
            journal.schema_version
        );
    }
    Ok(Some(journal))
}

fn restore_canonical_report_from_journal(path: &Path, journal_path: &Path) -> Result<()> {
    let Some(journal) = read_canonical_report_restore_journal(
        journal_path.parent().unwrap_or_else(|| Path::new("")),
    )?
    else {
        return Ok(());
    };
    match journal.previous {
        Some(previous) => {
            write_string_atomic(path, &previous)
                .with_context(|| format!("failed to restore {}", path.display()))?;
        }
        None => match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to remove {}", path.display()));
            }
        },
    }
    remove_canonical_report_restore_journal(journal_path)
}

fn remove_canonical_report_restore_journal(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn report_file_stem(report: &VerificationReport) -> String {
    report
        .attempt_id
        .as_deref()
        .unwrap_or(report.verified_at.as_str())
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn new_attempt_id(task: &TaskRecord, verified_at: &str) -> String {
    let counter = ATTEMPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{verified_at}-{}-{counter}", task.id, process::id())
}

fn event_source_path(report: &VerificationReport) -> Option<String> {
    report
        .proof_sources
        .iter()
        .find(|source| source.kind == EVENT_PROOF_SOURCE_KIND)
        .map(|source| source.path.clone())
}

fn failures_for(
    task: &TaskRecord,
    claims: &[String],
    claim_checks: &[ClaimCheck],
    evidence: &[EvidenceText],
    commands: &[VerificationCommand],
) -> Vec<String> {
    let mut failures = Vec::new();

    if task.state != TaskState::NeedsVerification && task.state != TaskState::Verified {
        failures.push(format!(
            "task is {}, expected needs_verification",
            state_name(&task.state)
        ));
    }
    if claims.is_empty() {
        failures.push("no completion claims found in task history".to_string());
    }
    if evidence.is_empty() {
        failures.push("missing proof: no task events or proof artifacts found".to_string());
    }

    for check in claim_checks.iter().filter(|check| !check.matched) {
        failures.push(format!("claim not backed by events/proof: {}", check.claim));
    }
    for command in commands.iter().filter(|command| command.exit_code != 0) {
        failures.push(format!(
            "verify command failed: {} (exit {})",
            command.cmd, command.exit_code
        ));
    }

    failures
}

fn run_verify_commands(paths: &MaestroPaths) -> Result<Vec<VerificationCommand>> {
    let commands = harness_verify_commands(paths)?;
    let mut results = Vec::new();
    for command in commands {
        let started = Instant::now();
        let status = shell_command(&command)
            .current_dir(paths.repo_root())
            .status()
            .with_context(|| format!("failed to run verify command `{command}`"))?;
        results.push(VerificationCommand {
            cmd: command,
            exit_code: status.code().unwrap_or(1),
            duration_ms: started.elapsed().as_millis(),
        });
    }
    Ok(results)
}

fn harness_verify_commands(paths: &MaestroPaths) -> Result<Vec<String>> {
    let path = match managed_path(
        paths,
        ".maestro/harness/harness.yml",
        SymlinkPolicy::RejectAllComponents,
    ) {
        Ok(path) => path,
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::ManagedPathContainsSymlink { .. })
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    let Some(raw) = read_to_string_if_exists(&path)? else {
        return Ok(Vec::new());
    };
    let config: HarnessConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config.stack.verify)
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    shell
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

fn check_claims(claims: &[String], evidence: &[EvidenceText]) -> Vec<ClaimCheck> {
    claims
        .iter()
        .map(|claim| {
            let normalized_claim = normalize_claim(claim);
            let source = evidence
                .iter()
                .find(|source| {
                    source
                        .claims
                        .iter()
                        .any(|candidate| normalize_claim(candidate) == normalized_claim)
                })
                .map(|source| source.path.display().to_string());
            ClaimCheck {
                claim: claim.clone(),
                matched: source.is_some(),
                source,
            }
        })
        .collect()
}

fn completion_claims(task: &TaskRecord) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut claims = Vec::new();
    for claim in task
        .state_history
        .iter()
        .flat_map(|entry| entry.claims.iter())
        .map(|claim| claim.trim())
        .filter(|claim| !claim.is_empty())
    {
        if seen.insert(claim.to_string()) {
            claims.push(claim.to_string());
        }
    }
    claims
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvidenceText {
    kind: String,
    path: PathBuf,
    text: String,
    claims: Vec<String>,
}

fn collect_evidence(
    paths: &MaestroPaths,
    task_dir: &Path,
    task_id: &str,
) -> Result<Vec<EvidenceText>> {
    let mut evidence = Vec::new();
    collect_task_artifact_text(task_dir, "evidence", &mut evidence)?;
    collect_task_artifact_text(task_dir, "proof", &mut evidence)?;
    collect_event_text(paths, task_id, &mut evidence)?;
    Ok(evidence)
}

fn collect_task_artifact_text(
    task_dir: &Path,
    dirname: &str,
    evidence: &mut Vec<EvidenceText>,
) -> Result<()> {
    let dir = task_dir.join(dirname);
    if !dir.is_dir() {
        return Ok(());
    }

    for path in text_files_under(&dir)? {
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let claims = proof_text_claims(&text);
        evidence.push(EvidenceText {
            kind: dirname.to_string(),
            path,
            text,
            claims,
        });
    }
    Ok(())
}

fn collect_event_text(
    paths: &MaestroPaths,
    task_id: &str,
    evidence: &mut Vec<EvidenceText>,
) -> Result<()> {
    for path in managed_event_files(paths)? {
        let mut matched = Vec::new();
        let mut claims = Vec::new();
        for line in event_lines(&path)? {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if event.get("task_id").and_then(Value::as_str) == Some(task_id)
                && is_proof_event(&event)
            {
                claims.extend(event_claims(&event));
                matched.push(line);
            }
        }
        if !matched.is_empty() {
            evidence.push(EvidenceText {
                kind: EVENT_PROOF_SOURCE_KIND.to_string(),
                path,
                text: matched.join("\n"),
                claims,
            });
        }
    }
    Ok(())
}

fn is_proof_event(event: &Value) -> bool {
    matches!(event_kind(event), Some("proof" | "Proof" | "task_proof"))
        || is_phase4_tool_proof_event(event)
}

fn event_kind(event: &Value) -> Option<&str> {
    event
        .get("kind")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("type"))
        .and_then(Value::as_str)
}

fn event_claims(event: &Value) -> Vec<String> {
    let mut claims = Vec::new();
    if let Some(claim) = event.get("claim").and_then(Value::as_str) {
        claims.push(claim.to_string());
    }
    if let Some(message) = event.get("message").and_then(Value::as_str) {
        claims.push(message.to_string());
    }
    if let Some(values) = event.get("claims").and_then(Value::as_array) {
        claims.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if is_phase4_tool_proof_event(event) {
        claims.extend(phase4_tool_claims(event));
    }
    claims
}

fn is_phase4_tool_proof_event(event: &Value) -> bool {
    event.get("schema_version").and_then(Value::as_str) == Some(EVENT_SCHEMA_VERSION)
        && event.get("event_type").and_then(Value::as_str) == Some("PostToolUse")
        && event.get("status").and_then(Value::as_str) == Some("ok")
}

fn phase4_tool_claims(event: &Value) -> Vec<String> {
    let mut claims = Vec::new();
    let tool_name = event.get("tool_name").and_then(Value::as_str);
    let tool_input_hash = event.get("tool_input_hash").and_then(Value::as_str);

    if let (Some(tool_name), Some(tool_input_hash)) = (tool_name, tool_input_hash) {
        claims.push(format!("{tool_name} {tool_input_hash}"));
    }
    claims
}

fn proof_text_claims(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("claim:")
                .or_else(|| trimmed.strip_prefix("Claim:"))
                .map(str::trim)
                .filter(|claim| !claim.is_empty())
                .map(str::to_string)
        })
        .collect()
}

fn normalize_claim(claim: &str) -> String {
    claim.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn event_lines(path: &Path) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::to_string)
        .collect())
}

fn text_files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("failed to inspect {}", path.display()))?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    collect_files(&path, files)?;
                } else if file_type.is_file() {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}

fn load_acceptance_with_hash(task_dir: &Path) -> Result<(AcceptanceFile, String)> {
    let path = task_dir.join("acceptance.yaml");
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!(
            "managed task acceptance path must not be a symlink: {}",
            path.display()
        );
    }
    if !metadata.is_file() {
        bail!(
            "managed task acceptance path must be a file: {}",
            path.display()
        );
    }
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let raw = String::from_utf8(bytes.clone())
        .with_context(|| format!("failed to read {} as UTF-8", path.display()))?;
    let acceptance = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok((acceptance, hash_bytes(&bytes)))
}

fn task_contract_hash(task: &TaskRecord) -> String {
    let claims = completion_claims(task);
    let contract = json!({
        "schema_version": task.schema_version,
        "id": task.id,
        "slug": task.slug,
        "feature_id": task.feature_id,
        "title": task.title,
        "task_type": task.task_type,
        "lane": task.lane,
        "risk": task.risk,
        "raw_request": task.raw_request,
        "input_type": task.input_type,
        "affected_areas": task.affected_areas,
        "open_questions": task.open_questions,
        "acceptance_locked": task.acceptance_locked,
        "claims": claims,
    });
    hash_bytes(contract.to_string().as_bytes())
}

fn checks_hash(acceptance: &AcceptanceFile) -> String {
    hash_bytes(
        serde_json::to_string(&acceptance.checks)
            .expect("invariant: acceptance checks should serialize")
            .as_bytes(),
    )
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn state_name(state: &TaskState) -> &'static str {
    match state {
        TaskState::Draft => "draft",
        TaskState::Exploring => "exploring",
        TaskState::Ready => "ready",
        TaskState::InProgress => "in_progress",
        TaskState::NeedsVerification => "needs_verification",
        TaskState::Verified => "verified",
        TaskState::Rejected => "rejected",
        TaskState::Abandoned => "abandoned",
        TaskState::Superseded => "superseded",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        recover_canonical_report_for_task, replace_task_report_preserving_previous,
        verification_outcome_for_report, verification_path, VerificationReport, VerificationStatus,
        VerificationTaskSnapshot,
    };
    use crate::domain::task::{self, AcceptanceFile, TaskRecord};
    use crate::foundation::core::schema::VERIFICATION_SCHEMA_VERSION;

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn canonical_report_restores_previous_when_task_save_fails_after_promotion() {
        let temp = TestTempDir::new("maestro-proof-report-rollback");
        let tasks_dir = temp.path().join(".maestro/tasks");
        fs::create_dir_all(&tasks_dir).expect("invariant: tasks dir should be creatable");
        let task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        let acceptance = AcceptanceFile::new("task-001", Vec::new());
        let task_dir = task::template::write_task_artifacts(&tasks_dir, &task, &acceptance)
            .expect("invariant: task artifacts should be writable");
        let previous = report("task-001", "old-attempt", "old-time");
        let next = report("task-001", "new-attempt", "new-time");
        super::write_task_report(&task_dir, &previous)
            .expect("invariant: previous report should be writable");
        let mut handle = task::load_task_for_update(&tasks_dir, "task-001")
            .expect("invariant: task should load for update");
        let outcome =
            verification_outcome_for_report(&next).expect("invariant: outcome should build");

        let result = task::apply_verification_outcome_to_handle_after(
            &mut handle,
            outcome,
            "test",
            "new-time",
            || {
                let restore = replace_task_report_preserving_previous(&task_dir, &next)?;
                fs::remove_file(task_dir.join("task.yaml"))
                    .expect("invariant: task.yaml should be removable");
                fs::create_dir(task_dir.join("task.yaml"))
                    .expect("invariant: task.yaml directory should be creatable");
                Ok(restore)
            },
        );

        assert!(result.is_err());
        let restored = fs::read_to_string(verification_path(&task_dir))
            .expect("invariant: verification report should remain readable");
        let restored: VerificationReport =
            serde_json::from_str(&restored).expect("invariant: report should parse");
        assert_eq!(restored.attempt_id.as_deref(), Some("old-attempt"));
    }

    #[test]
    fn canonical_report_restore_journal_recovers_after_interrupted_promotion() {
        let temp = TestTempDir::new("maestro-proof-report-recovery");
        let tasks_dir = temp.path().join(".maestro/tasks");
        fs::create_dir_all(&tasks_dir).expect("invariant: tasks dir should be creatable");
        let task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        let acceptance = AcceptanceFile::new("task-001", Vec::new());
        let task_dir = task::template::write_task_artifacts(&tasks_dir, &task, &acceptance)
            .expect("invariant: task artifacts should be writable");
        let previous = report("task-001", "old-attempt", "old-time");
        let next = report("task-001", "new-attempt", "new-time");
        super::write_task_report(&task_dir, &previous)
            .expect("invariant: previous report should be writable");

        let guard = replace_task_report_preserving_previous(&task_dir, &next)
            .expect("invariant: canonical promotion should write");
        drop(guard);

        recover_canonical_report_for_task(&task, &task_dir, |_, _| false)
            .expect("invariant: interrupted promotion should recover");

        let restored = fs::read_to_string(verification_path(&task_dir))
            .expect("invariant: verification report should remain readable");
        let restored: VerificationReport =
            serde_json::from_str(&restored).expect("invariant: report should parse");
        assert_eq!(restored.attempt_id.as_deref(), Some("old-attempt"));
        assert!(!task_dir.join(super::CANONICAL_REPORT_RESTORE_FILE).exists());
    }

    fn report(task_id: &str, attempt_id: &str, verified_at: &str) -> VerificationReport {
        VerificationReport {
            schema_version: VERIFICATION_SCHEMA_VERSION.to_string(),
            task_id: task_id.to_string(),
            attempt_id: Some(attempt_id.to_string()),
            task_snapshot: Some(VerificationTaskSnapshot {
                updated_at: "t0".to_string(),
            }),
            status: VerificationStatus::Passed,
            verified_at: verified_at.to_string(),
            freshness: super::StoredFreshness {
                verified_commit: None,
                task_contract_hash: "task-hash".to_string(),
                acceptance_hash: "acceptance-hash".to_string(),
                checks_hash: "checks-hash".to_string(),
            },
            claims: Vec::new(),
            commands: Vec::new(),
            proof_sources: Vec::new(),
            failures: Vec::new(),
        }
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "{prefix}-{}-{timestamp}-{counter}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
