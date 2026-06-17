//! Task verification orchestration, report DTOs, and hashing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::claims::{check_claims, collect_evidence};
use super::commands::run_verify_commands;
use super::stale::{FreshnessInputs, StoredFreshness};
use crate::domain::task::{self, TaskRecord, TaskState, VerificationBinding};
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::VERIFICATION_SCHEMA_VERSION;

pub(super) const EVENT_PROOF_SOURCE_KIND: &str = "event";

/// High-level result returned by the Proof facade after task verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskVerification {
    pub task_id: String,
    pub status: TaskVerificationStatus,
    pub claim_count: usize,
    pub proof_source_count: usize,
    pub command_count: usize,
    pub claims_only: bool,
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
    #[serde(default, skip_serializing_if = "is_false")]
    pub claims_only: bool,
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

/// Evaluate task proof and return the report that should be written.
pub(crate) fn evaluate_task_report(
    paths: &MaestroPaths,
    task: &TaskRecord,
    task_dir: &Path,
    verified_at: &str,
) -> Result<VerificationReport> {
    let command_run = run_verify_commands(paths, task.verify_command.as_deref())?;
    let inputs = freshness_inputs_for_task(task, git::head(paths.repo_root()).unwrap_or(None))?;
    let claims = completion_claims(task);
    let evidence = collect_evidence(paths, task_dir, &task.id)?;
    let claim_checks = check_claims(&claims, &evidence, paths.repo_root());
    let standalone_without_checks = task.feature_id.is_none() && task.acceptance.checks.is_empty();
    let failures = failures_for(
        task,
        &claims,
        &claim_checks,
        &evidence,
        VerificationCommandPolicy {
            commands: &command_run.commands,
            claims_only: command_run.claims_only,
            stack_kind: &command_run.stack_kind,
        },
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
        attempt_id: None,
        task_snapshot: Some(VerificationTaskSnapshot {
            updated_at: task.updated_at.clone(),
        }),
        status,
        verified_at: verified_at.to_string(),
        freshness: StoredFreshness {
            verified_commit: inputs.commit.clone(),
            contract_hash: inputs.contract_hash,
        },
        claims: claim_checks,
        commands: command_run.commands,
        claims_only: command_run.claims_only,
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
            command_count: report.commands.len(),
            claims_only: report.claims_only,
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
    match task::load_task_for_update(&paths.tasks_dir(), task_id) {
        Ok(handle) => Ok(LoadedTask {
            task: handle.task().clone(),
            task_dir: handle.task_dir().to_path_buf(),
        }),
        Err(live_err) => match task::load_archived_task_record(paths, task_id) {
            Ok(Some((task, task_dir))) => Ok(LoadedTask { task, task_dir }),
            _ => Err(live_err),
        },
    }
}

/// Compute current proof freshness inputs for a task.
pub fn freshness_inputs_for_task(
    task: &TaskRecord,
    commit: Option<String>,
) -> Result<FreshnessInputs> {
    Ok(FreshnessInputs {
        commit,
        contract_hash: task_contract_hash(task),
    })
}

pub(crate) fn verification_outcome_for_report(
    report: &VerificationReport,
) -> Result<task::VerificationOutcome> {
    match report.status {
        VerificationStatus::Passed => Ok(task::VerificationOutcome::Passed(
            task::VerificationPassed {
                binding: verification_binding_for_report(report),
                summary: report_summary(report),
            },
        )),
        VerificationStatus::Failed => Ok(task::VerificationOutcome::Failed(
            task::VerificationFailed {
                binding: verification_binding_for_report(report),
                summary: report_summary(report),
                failures: report.failures.clone(),
            },
        )),
    }
}

pub(crate) fn verification_binding_for_report(report: &VerificationReport) -> VerificationBinding {
    VerificationBinding {
        status: Some(match report.status {
            VerificationStatus::Passed => task::VerificationStatus::Passed,
            VerificationStatus::Failed => task::VerificationStatus::Failed,
        }),
        verified_at: Some(report.verified_at.clone()),
        verified_commit: report.freshness.verified_commit.clone(),
        verified_by_run: event_source_path(report),
        contract_hash: Some(report.freshness.contract_hash.clone()),
        claim_checks: report
            .claims
            .iter()
            .map(|claim| task::ClaimCheckReceipt {
                claim: claim.claim.clone(),
                matched: claim.matched,
                source: claim.source.clone(),
            })
            .collect(),
        commands: report
            .commands
            .iter()
            .map(|command| task::VerificationCommandReceipt {
                cmd: command.cmd.clone(),
                exit_code: command.exit_code,
                duration_ms: command.duration_ms,
            })
            .collect(),
        claims_only: report.claims_only,
        proof_sources: report
            .proof_sources
            .iter()
            .map(|source| task::ProofSourceReceipt {
                kind: source.kind.clone(),
                path: source.path.clone(),
                hash: source.hash.clone(),
            })
            .collect(),
        failures: report.failures.clone(),
    }
}

pub(crate) fn report_summary(report: &VerificationReport) -> String {
    match report.status {
        VerificationStatus::Passed => format!(
            "verification passed: {} claim(s), {} proof source(s){}",
            report.claims.len(),
            report.proof_sources.len(),
            claims_only_suffix(report)
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
    command_policy: VerificationCommandPolicy<'_>,
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
    if command_policy.commands.is_empty() && !command_policy.claims_only {
        failures.push(format!(
            "cannot verify {} -- no verify commands configured (stack: {})\n  fix: add commands to .maestro/harness/harness.yml stack.verify\n  or: accept claims-only verification: maestro harness set --claims-only\n  retry: maestro task verify {}",
            task.id, command_policy.stack_kind, task.id
        ));
    }

    for check in claim_checks.iter().filter(|check| !check.matched) {
        failures.push(format!(
            "claim not backed by events/proof: {}; record matching proof with `maestro event create --task-id {} --claim \"{}\"`",
            check.claim, task.id, check.claim
        ));
    }
    for command in command_policy
        .commands
        .iter()
        .filter(|command| command.exit_code != 0)
    {
        failures.push(format!(
            "verify command failed: {} (exit {})",
            command.cmd, command.exit_code
        ));
    }

    failures
}

struct VerificationCommandPolicy<'a> {
    commands: &'a [VerificationCommand],
    claims_only: bool,
    stack_kind: &'a str,
}

fn claims_only_suffix(report: &VerificationReport) -> String {
    if report.claims_only {
        format!(", claims-only, {} command(s)", report.commands.len())
    } else {
        String::new()
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn completion_claims(task: &TaskRecord) -> Vec<String> {
    if !task.claims.is_empty() {
        return task.claims.clone();
    }
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

fn task_contract_hash(task: &TaskRecord) -> String {
    let mut contract = json!({
        "id": task.id,
        "title": task.title,
        "acceptance": task.acceptance,
        "claims": completion_claims(task),
    });
    // Only fold the falsifier into the hash when one is set, so tasks verified
    // before this field existed keep their original hash and are not re-staled
    // on upgrade. Setting/clearing the command still changes the hash.
    if let Some(command) = &task.verify_command {
        contract["verify_command"] = json!(command);
    }
    sha256_hex(contract.to_string().as_bytes())
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_the_per_task_verify_command_invalidates_freshness() {
        let mut task = TaskRecord::draft("task-001", "Slice", "2026-06-13T00:00:00Z");
        let before = freshness_inputs_for_task(&task, None)
            .expect("invariant: freshness inputs should compute")
            .contract_hash;

        task.verify_command = Some("cargo test --test resources_version_guard".to_string());
        let after = freshness_inputs_for_task(&task, None)
            .expect("invariant: freshness inputs should compute")
            .contract_hash;

        assert_ne!(
            before, after,
            "the per-task verify command must be part of the contract hash so editing it invalidates freshness"
        );
    }

    #[test]
    fn a_task_with_no_falsifier_hashes_as_if_the_field_never_existed() {
        // Guards against re-staling every pre-upgrade verified task: the hash for
        // a task without a falsifier must omit the key entirely (not serialize a
        // `verify_command: null`), so it equals the hash computed before the field
        // was added.
        let task = TaskRecord::draft("task-001", "Slice", "2026-06-13T00:00:00Z");
        let with_skip = task_contract_hash(&task);

        let legacy = json!({
            "id": task.id,
            "title": task.title,
            "acceptance": task.acceptance,
            "claims": completion_claims(&task),
        });
        let legacy_hash = sha256_hex(legacy.to_string().as_bytes());

        assert_eq!(
            with_skip, legacy_hash,
            "a task without a falsifier must keep its pre-field contract hash"
        );
    }
}
