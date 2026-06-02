//! Task verification orchestration, report DTOs, and hashing.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::attempts::write_task_report_attempt;
use super::claims::{check_claims, collect_evidence};
use super::commands::run_verify_commands;
use super::stale::{FreshnessInputs, StoredFreshness};
use crate::domain::task::{self, AcceptanceFile, TaskRecord, TaskState, VerificationBinding};
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::VERIFICATION_SCHEMA_VERSION;

static ATTEMPT_COUNTER: AtomicU64 = AtomicU64::new(0);
pub(super) const EVENT_PROOF_SOURCE_KIND: &str = "event";

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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerificationCommand {
    pub cmd: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

impl<'de> Deserialize<'de> for VerificationCommand {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(object) = value else {
            return Err(serde::de::Error::custom(
                "verification command must be an object",
            ));
        };
        let cmd = match object.get("cmd") {
            Some(Value::String(cmd)) => cmd.clone(),
            _ => {
                return Err(serde::de::Error::custom(
                    "verification command object missing string cmd",
                ));
            }
        };
        let exit_code = match object.get("exit_code").and_then(Value::as_i64) {
            Some(exit_code) => i32::try_from(exit_code)
                .map_err(|_| serde::de::Error::custom("exit_code out of range"))?,
            None => 0,
        };
        let duration_ms = match object.get("duration_ms") {
            Some(Value::Number(number)) => number.as_u64().map(u128::from).ok_or_else(|| {
                serde::de::Error::custom("duration_ms must be a positive integer")
            })?,
            Some(Value::String(duration)) => duration
                .parse::<u128>()
                .map_err(|_| serde::de::Error::custom("duration_ms must parse"))?,
            Some(_) => {
                return Err(serde::de::Error::custom(
                    "duration_ms must be a number or string",
                ));
            }
            None => 0,
        };
        Ok(Self {
            cmd,
            exit_code,
            duration_ms,
        })
    }
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
    let standalone_without_checks = if task.feature_id.is_none() {
        let (acceptance, _) = load_acceptance_with_hash(task_dir)?;
        acceptance.checks.is_empty()
    } else {
        false
    };
    let failures = failures_for(
        task,
        &claims,
        &claim_checks,
        &evidence,
        &commands,
        standalone_without_checks,
    );
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
                hash: sha256_hex(source.text.as_bytes()),
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
///
/// L6b: a proof read crosses the live/archive boundary, so an archived task's
/// proof status still resolves (matching `task show`). Only `proof_status` reads
/// through here; the verify transition uses a separate path, so archived tasks
/// stay immutable.
pub fn load_task_by_id(paths: &MaestroPaths, task_id: &str) -> Result<LoadedTask> {
    let handle = match task::load_task_for_update(&paths.tasks_dir(), task_id) {
        Ok(handle) => handle,
        Err(live_err) => {
            task::load_task_for_update(&paths.archive_tasks_dir(), task_id).map_err(|_| live_err)?
        }
    };

    Ok(LoadedTask {
        task: handle.task().clone(),
        task_dir: handle.task_dir().to_path_buf(),
    })
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
    standalone_without_checks: bool,
) -> Vec<String> {
    let mut failures = Vec::new();

    if standalone_without_checks {
        failures.push(format!(
            "standalone task {} requires at least one check; add one with `maestro task set {} --check \"...\"`",
            task.id, task.id
        ));
    }

    if task.state != TaskState::NeedsVerification && task.state != TaskState::Verified {
        failures.push(format!(
            "task is {}, expected needs_verification; submit it first with `maestro task complete {} --summary \"...\" --claim \"...\"`",
            task.state.as_str(),
            task.id
        ));
    }
    if claims.is_empty() {
        failures.push(format!(
            "no completion claims found in task history; record one with `maestro task complete {} --claim \"...\"`",
            task.id
        ));
    }
    if evidence.is_empty() {
        failures.push(format!(
            "missing proof: no task events or proof artifacts found; hooks record proof during agent runs, or add one with `maestro event create --task-id {} --claim \"...\"`",
            task.id
        ));
    }

    for check in claim_checks.iter().filter(|check| !check.matched) {
        failures.push(format!(
            "claim not backed by events/proof: {}; record matching proof with `maestro event create --task-id {} --claim \"{}\"`",
            check.claim, task.id, check.claim
        ));
    }
    for command in commands.iter().filter(|command| command.exit_code != 0) {
        failures.push(format!(
            "verify command failed: {} (exit {})",
            command.cmd, command.exit_code
        ));
    }

    failures
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
pub(super) struct EvidenceText {
    pub(super) kind: String,
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) claims: Vec<String>,
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
    Ok((acceptance, sha256_hex(&bytes)))
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
        "acceptance_locked": task.acceptance_locked,
        "claims": claims,
    });
    sha256_hex(contract.to_string().as_bytes())
}

fn checks_hash(acceptance: &AcceptanceFile) -> String {
    sha256_hex(
        serde_json::to_string(&acceptance.checks)
            .expect("invariant: acceptance checks should serialize")
            .as_bytes(),
    )
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn new_attempt_id(task: &TaskRecord, verified_at: &str) -> String {
    let counter = ATTEMPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{verified_at}-{}-{counter}", task.id, process::id())
}
