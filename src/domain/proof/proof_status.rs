//! Verification proof status helpers.

use std::path::Path;

use anyhow::Result;

use super::stale::{StaleReason, StoredFreshness, stale_reasons};
use super::verify_task::{freshness_inputs_for_task, load_task_by_id};
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::render_timestamp;

/// Derived status for a task's persisted proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofStatusKind {
    Missing,
    Failed,
    Accepted,
    Stale,
}

/// One proof source included in a user-facing status read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofStatusSource {
    pub kind: String,
    pub path: String,
}

/// One stale-proof reason included in a user-facing status read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofStaleReason {
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

/// User-facing proof status loaded from task.yaml.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofStatus {
    pub task_id: String,
    pub kind: ProofStatusKind,
    pub verification_path: String,
    pub verified_at: Option<String>,
    pub verified_commit: Option<String>,
    pub matched_claims: usize,
    pub total_claims: usize,
    pub claims_only: bool,
    pub sources: Vec<ProofStatusSource>,
    pub stale_reasons: Vec<ProofStaleReason>,
    pub failures: Vec<String>,
}

/// Proof-owned outcome for Improve's verification command read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationCommandRead {
    pub(crate) commands: Vec<VerificationCommandEvidence>,
    pub(crate) source: VerificationCommandSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationCommandEvidence {
    command: String,
    safe_summary: String,
}

impl VerificationCommandEvidence {
    pub(crate) fn command(&self) -> &str {
        &self.command
    }

    pub(crate) fn safe_summary(&self) -> &str {
        &self.safe_summary
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationCommandSource {
    evidence_name: String,
}

impl VerificationCommandSource {
    pub(crate) fn evidence_name(&self) -> &str {
        &self.evidence_name
    }
}

/// Load persisted proof and derive freshness against current task inputs.
pub fn proof_status(paths: &MaestroPaths, task_id: &str) -> Result<ProofStatus> {
    let loaded = load_task_by_id(paths, task_id)?;
    proof_status_for_task(
        paths,
        &loaded.task,
        &loaded.task_dir,
        git::head(paths.repo_root()).unwrap_or(None),
    )
}

/// Load only the persisted proof classification for an already loaded task.
pub fn proof_status_kind_for_task(
    task: &task::TaskRecord,
    current_commit: Option<String>,
) -> Result<ProofStatusKind> {
    classify_binding(task, current_commit, FailedStalePolicy::Skip)
        .map(|classification| classification.kind)
}

/// Load only the persisted proof classification needed by needs-verification UI rows.
pub fn needs_verification_proof_status_kind_for_task(
    task: &task::TaskRecord,
) -> Result<ProofStatusKind> {
    Ok(match task.verification.status {
        None => ProofStatusKind::Missing,
        Some(task::VerificationStatus::Failed) => ProofStatusKind::Failed,
        Some(task::VerificationStatus::Passed) => ProofStatusKind::Accepted,
    })
}

/// Read verification commands from task.yaml.
pub(crate) fn verification_command_read_for_task(
    task: &task::TaskRecord,
) -> Result<VerificationCommandRead> {
    Ok(VerificationCommandRead {
        commands: task
            .verification
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| VerificationCommandEvidence {
                command: command.cmd.clone(),
                safe_summary: format!("verification command {}", index + 1),
            })
            .collect(),
        source: VerificationCommandSource {
            evidence_name: "task.yaml#verification".to_string(),
        },
    })
}

/// Load persisted proof for an already loaded task and derive its status.
pub fn proof_status_for_task(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<ProofStatus> {
    let verification_path = display_verification_path(paths, &task_dir.join("task.yaml"));
    if task.verification.status.is_none() {
        return Ok(ProofStatus {
            task_id: task.id.clone(),
            kind: ProofStatusKind::Missing,
            verification_path,
            verified_at: None,
            verified_commit: None,
            matched_claims: 0,
            total_claims: 0,
            claims_only: false,
            sources: Vec::new(),
            stale_reasons: Vec::new(),
            failures: Vec::new(),
        });
    }

    let classification = classify_binding(task, current_commit, FailedStalePolicy::BestEffort)?;
    let stale = classification
        .stale
        .into_iter()
        .map(ProofStaleReason::from)
        .collect();
    Ok(status_from_binding(
        task.id.clone(),
        classification.kind,
        verification_path,
        &task.verification,
        stale,
    ))
}

/// Render proof status for CLI output.
pub fn render_proof_status(status: &ProofStatus) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "proof {}: {}\n",
        status.task_id,
        status.kind.label()
    ));
    out.push_str(&format!("verification: {}\n", status.verification_path));

    let Some(verified_at) = status.verified_at.as_ref() else {
        out.push_str("reason: missing task.yaml verification block\n");
        return out;
    };

    out.push_str(&format!("verified_at: {}\n", render_timestamp(verified_at)));
    out.push_str(&format!(
        "commit: {}\n",
        status.verified_commit.as_deref().unwrap_or("<none>")
    ));
    out.push_str(&format_claims(status.matched_claims, status.total_claims));
    if status.claims_only {
        out.push_str("claims_only: true\n");
    }
    out.push_str(&format_sources(&status.sources));

    if !status.stale_reasons.is_empty() {
        out.push_str("stale_reasons:\n");
        for reason in &status.stale_reasons {
            out.push_str(&format!(
                "- {} expected {} found {}\n",
                reason.field, reason.expected, reason.actual
            ));
        }
    }
    if !status.failures.is_empty() {
        out.push_str("failures:\n");
        for failure in &status.failures {
            out.push_str(&format!("- {failure}\n"));
        }
    }

    out
}

impl ProofStatusKind {
    pub fn label(&self) -> &'static str {
        match self {
            ProofStatusKind::Missing => "missing",
            ProofStatusKind::Failed => "failed",
            ProofStatusKind::Accepted => "accepted",
            ProofStatusKind::Stale => "stale",
        }
    }
}

impl From<StaleReason> for ProofStaleReason {
    fn from(reason: StaleReason) -> Self {
        Self {
            field: reason.field,
            expected: reason.expected,
            actual: reason.actual,
        }
    }
}

fn status_from_binding(
    task_id: String,
    kind: ProofStatusKind,
    verification_path: String,
    binding: &task::VerificationBinding,
    stale_reasons: Vec<ProofStaleReason>,
) -> ProofStatus {
    ProofStatus {
        task_id,
        kind,
        verification_path,
        verified_at: binding.verified_at.clone(),
        verified_commit: binding.verified_commit.clone(),
        matched_claims: binding
            .claim_checks
            .iter()
            .filter(|claim| claim.matched)
            .count(),
        total_claims: binding.claim_checks.len(),
        claims_only: binding.claims_only,
        sources: binding
            .proof_sources
            .iter()
            .map(|source| ProofStatusSource {
                kind: source.kind.clone(),
                path: source.path.clone(),
            })
            .collect(),
        stale_reasons,
        failures: binding.failures.clone(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailedStalePolicy {
    Skip,
    BestEffort,
}

struct ProofClassification {
    kind: ProofStatusKind,
    stale: Vec<StaleReason>,
}

fn classify_binding(
    task: &task::TaskRecord,
    current_commit: Option<String>,
    failed_stale_policy: FailedStalePolicy,
) -> Result<ProofClassification> {
    let Some(status) = task.verification.status.clone() else {
        return Ok(ProofClassification {
            kind: ProofStatusKind::Missing,
            stale: Vec::new(),
        });
    };
    let stale = match (status, failed_stale_policy) {
        (task::VerificationStatus::Failed, FailedStalePolicy::Skip) => Vec::new(),
        _ => full_status_stale_reasons(task, current_commit)?,
    };
    let kind = match task.verification.status {
        Some(task::VerificationStatus::Failed) => ProofStatusKind::Failed,
        Some(task::VerificationStatus::Passed) if stale.is_empty() => ProofStatusKind::Accepted,
        Some(task::VerificationStatus::Passed) => ProofStatusKind::Stale,
        None => ProofStatusKind::Missing,
    };
    Ok(ProofClassification { kind, stale })
}

fn full_status_stale_reasons(
    task: &task::TaskRecord,
    current_commit: Option<String>,
) -> Result<Vec<StaleReason>> {
    let current = freshness_inputs_for_task(task, current_commit)?;
    let stored = StoredFreshness {
        verified_commit: task.verification.verified_commit.clone(),
        contract_hash: task.verification.contract_hash.clone().unwrap_or_default(),
    };
    Ok(stale_reasons(&current, &stored))
}

fn display_verification_path(paths: &MaestroPaths, task_yaml: &Path) -> String {
    format!(
        "{}#verification",
        task_yaml
            .strip_prefix(paths.repo_root())
            .unwrap_or(task_yaml)
            .display()
    )
}

fn format_claims(matched: usize, total: usize) -> String {
    format!("claims: {matched}/{total}\n")
}

fn format_sources(sources: &[ProofStatusSource]) -> String {
    if sources.is_empty() {
        return "sources: 0\n".to_string();
    }

    let mut out = format!("sources: {}\n", sources.len());
    for source in sources {
        out.push_str(&format!("- {} {}\n", source.kind, source.path));
    }
    out
}
