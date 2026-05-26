//! Verification proof status helpers.

use std::path::Path;

use anyhow::Result;

use super::stale::{stale_reasons, StaleReason};
use super::verify_task::{
    freshness_inputs, freshness_inputs_for_task, load_task_by_id, read_report, verification_path,
    LoadedTask, VerificationReport, VerificationStatus,
};
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;

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

/// User-facing proof status loaded from `verification.json`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofStatus {
    pub task_id: String,
    pub kind: ProofStatusKind,
    pub verification_path: String,
    pub verified_at: Option<String>,
    pub verified_commit: Option<String>,
    pub matched_claims: usize,
    pub total_claims: usize,
    pub sources: Vec<ProofStatusSource>,
    pub stale_reasons: Vec<ProofStaleReason>,
    pub failures: Vec<String>,
}

/// Legacy proof status shape preserved for `crate::verification` callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LegacyProofStatus {
    pub(crate) task_id: String,
    pub(crate) kind: ProofStatusKind,
    pub(crate) verification_path: String,
    pub(crate) report: Option<VerificationReport>,
    pub(crate) stale_reasons: Vec<StaleReason>,
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
    _paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<ProofStatusKind> {
    let Some(report) = read_report(task_dir)? else {
        return Ok(ProofStatusKind::Missing);
    };
    proof_status_kind_for_report(task, task_dir, current_commit, &report)
}

/// Load persisted proof for an already loaded task and derive its status.
pub fn proof_status_for_task(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<ProofStatus> {
    let report = read_report(task_dir)?;
    let path = display_verification_path(paths, task_dir);

    let Some(report) = report else {
        return Ok(ProofStatus {
            task_id: task.id.clone(),
            kind: ProofStatusKind::Missing,
            verification_path: path,
            verified_at: None,
            verified_commit: None,
            matched_claims: 0,
            total_claims: 0,
            sources: Vec::new(),
            stale_reasons: Vec::new(),
            failures: Vec::new(),
        });
    };

    let stale = full_status_stale_reasons(task, task_dir, current_commit, &report)?;
    let kind = proof_status_kind_from_report(&report, &stale);
    let stale = stale.into_iter().map(ProofStaleReason::from).collect();

    Ok(status_from_report(
        task.id.clone(),
        kind,
        path,
        report,
        stale,
    ))
}

/// Return whether the latest persisted proof report failed without computing freshness.
pub fn latest_proof_failed_for_task(task_dir: &Path) -> Result<bool> {
    Ok(read_report(task_dir)?
        .map(|report| report.status == VerificationStatus::Failed)
        .unwrap_or(false))
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
        out.push_str("reason: missing verification.json\n");
        return out;
    };

    out.push_str(&format!("verified_at: {verified_at}\n"));
    out.push_str(&format!(
        "commit: {}\n",
        status.verified_commit.as_deref().unwrap_or("<none>")
    ));
    out.push_str(&format_claims(status.matched_claims, status.total_claims));
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

fn status_from_report(
    task_id: String,
    kind: ProofStatusKind,
    verification_path: String,
    report: VerificationReport,
    stale_reasons: Vec<ProofStaleReason>,
) -> ProofStatus {
    ProofStatus {
        task_id,
        kind,
        verification_path,
        verified_at: Some(report.verified_at),
        verified_commit: report.freshness.verified_commit,
        matched_claims: report.claims.iter().filter(|claim| claim.matched).count(),
        total_claims: report.claims.len(),
        sources: report
            .proof_sources
            .into_iter()
            .map(|source| ProofStatusSource {
                kind: source.kind,
                path: source.path,
            })
            .collect(),
        stale_reasons,
        failures: report.failures,
    }
}

pub(crate) fn legacy_proof_status(
    paths: &MaestroPaths,
    task_id: &str,
) -> Result<LegacyProofStatus> {
    let loaded = load_task_by_id(paths, task_id)?;
    legacy_proof_status_for_loaded(paths, loaded)
}

pub(crate) fn render_legacy_proof_status(status: &LegacyProofStatus) -> String {
    let read_model = legacy_status_as_read_model(status);
    render_proof_status(&read_model)
}

fn legacy_proof_status_for_loaded(
    paths: &MaestroPaths,
    loaded: LoadedTask,
) -> Result<LegacyProofStatus> {
    let report = read_report(&loaded.task_dir)?;
    let path = display_verification_path(paths, &loaded.task_dir);

    let Some(report) = report else {
        return Ok(LegacyProofStatus {
            task_id: loaded.task.id,
            kind: ProofStatusKind::Missing,
            verification_path: path,
            report: None,
            stale_reasons: Vec::new(),
        });
    };

    let current = freshness_inputs(paths, &loaded)?;
    let stale = stale_reasons(&current, &report.freshness);
    let kind = proof_status_kind_from_report(&report, &stale);

    Ok(LegacyProofStatus {
        task_id: loaded.task.id,
        kind,
        verification_path: path,
        report: Some(report),
        stale_reasons: stale,
    })
}

fn legacy_status_as_read_model(status: &LegacyProofStatus) -> ProofStatus {
    let Some(report) = status.report.clone() else {
        return ProofStatus {
            task_id: status.task_id.clone(),
            kind: status.kind.clone(),
            verification_path: status.verification_path.clone(),
            verified_at: None,
            verified_commit: None,
            matched_claims: 0,
            total_claims: 0,
            sources: Vec::new(),
            stale_reasons: Vec::new(),
            failures: Vec::new(),
        };
    };

    status_from_report(
        status.task_id.clone(),
        status.kind.clone(),
        status.verification_path.clone(),
        report,
        status
            .stale_reasons
            .clone()
            .into_iter()
            .map(ProofStaleReason::from)
            .collect(),
    )
}

fn proof_status_kind_for_report(
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
    report: &VerificationReport,
) -> Result<ProofStatusKind> {
    let stale = full_status_stale_reasons(task, task_dir, current_commit, report)?;
    Ok(proof_status_kind_from_report(report, &stale))
}

fn full_status_stale_reasons(
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
    report: &VerificationReport,
) -> Result<Vec<StaleReason>> {
    let current = freshness_inputs_for_task(task, task_dir, current_commit)?;
    Ok(stale_reasons(&current, &report.freshness))
}

fn proof_status_kind_from_report(
    report: &VerificationReport,
    stale_reasons: &[StaleReason],
) -> ProofStatusKind {
    match report.status {
        VerificationStatus::Failed => ProofStatusKind::Failed,
        VerificationStatus::Passed if stale_reasons.is_empty() => ProofStatusKind::Accepted,
        VerificationStatus::Passed => ProofStatusKind::Stale,
    }
}

fn display_verification_path(paths: &MaestroPaths, task_dir: &Path) -> String {
    let verification_file = verification_path(task_dir);
    verification_file
        .strip_prefix(paths.repo_root())
        .unwrap_or(&verification_file)
        .display()
        .to_string()
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
