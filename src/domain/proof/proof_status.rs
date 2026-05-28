//! Verification proof status helpers.

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::stale::{stale_reasons, StaleReason};
use super::verify_task::{
    applied_receipt_for_report, freshness_inputs_for_task, latest_attempt_report,
    latest_attempt_report_for_command_read, load_task_by_id, passed_binding_matches_report,
    read_managed_report_file_for_command_read, read_managed_report_file_if_exists,
    recover_canonical_report_for_task, verification_path, verification_report_is_newer,
    VerificationReport, VerificationReportRead, VerificationReportSource, VerificationStatus,
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
    Unapplied,
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

/// Proof-owned outcome for Improve's verification command read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum VerificationCommandRead {
    Commands {
        commands: Vec<VerificationCommandEvidence>,
        source: VerificationCommandSource,
    },
    SkippedMalformedReport,
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
///
/// The paths argument is retained to keep this facade symmetric with the full
/// read-model helper; kind-only classification does not need repo-local paths.
pub fn proof_status_kind_for_task(
    _paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<ProofStatusKind> {
    let Some(selected) = read_report_for_task(task, task_dir)? else {
        return Ok(ProofStatusKind::Missing);
    };
    Ok(classify_report(
        task,
        task_dir,
        current_commit,
        &selected.report,
        FailedStalePolicy::Skip,
    )?
    .kind)
}

/// Load only the persisted proof classification needed by needs-verification UI rows.
pub fn needs_verification_proof_status_kind_for_task(
    task: &task::TaskRecord,
    task_dir: &Path,
) -> Result<ProofStatusKind> {
    let Some(selected) = read_report_for_task(task, task_dir)? else {
        return Ok(ProofStatusKind::Missing);
    };
    Ok(classify_without_freshness(task, &selected.report))
}

/// Read verification commands without repairing or promoting proof reports.
pub(crate) fn verification_command_read_for_task(
    task: &task::TaskRecord,
    task_dir: &Path,
) -> Result<VerificationCommandRead> {
    let canonical_path = verification_path(task_dir);
    let canonical = read_managed_report_file_for_command_read(
        &canonical_path,
        VerificationReportSource::Canonical,
    )?;
    match report_for_command_read(task, task_dir, canonical)? {
        VerificationReportRead::Report {
            report,
            source: _,
            path,
        } => {
            let report = *report;
            Ok(VerificationCommandRead::Commands {
                commands: report
                    .commands
                    .into_iter()
                    .enumerate()
                    .map(|(index, command)| VerificationCommandEvidence {
                        command: command.cmd,
                        safe_summary: format!("verification command {}", index + 1),
                    })
                    .collect(),
                source: VerificationCommandSource {
                    evidence_name: evidence_name(task_dir, &path),
                },
            })
        }
        VerificationReportRead::Missing => Ok(VerificationCommandRead::Commands {
            commands: Vec::new(),
            source: VerificationCommandSource {
                evidence_name: "verification.json".to_string(),
            },
        }),
        VerificationReportRead::Malformed => Ok(VerificationCommandRead::SkippedMalformedReport),
    }
}

fn evidence_name(task_dir: &Path, path: &Path) -> String {
    if path == verification_path(task_dir) {
        return "verification.json".to_string();
    }
    if path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("verification.attempts")
    {
        if path.file_name().and_then(|name| name.to_str()) == Some("latest.json") {
            return "verification.attempts/latest.json".to_string();
        }
        return "verification.attempts/archived attempt".to_string();
    }
    "verification evidence".to_string()
}

fn report_for_command_read(
    task: &task::TaskRecord,
    task_dir: &Path,
    canonical: VerificationReportRead,
) -> Result<VerificationReportRead> {
    match canonical {
        VerificationReportRead::Report {
            report,
            source,
            path,
        } if report_reflected_in_task(task, &report) => {
            if canonical_report_can_short_circuit(&report) {
                return Ok(VerificationReportRead::Report {
                    report,
                    source,
                    path,
                });
            }
            match latest_attempt_report_for_command_read(task_dir)? {
                attempt @ VerificationReportRead::Report { .. } => {
                    let is_newer = match &attempt {
                        VerificationReportRead::Report {
                            report: attempt_report,
                            ..
                        } => verification_report_is_newer(attempt_report, &report),
                        _ => false,
                    };
                    if is_newer {
                        Ok(attempt)
                    } else {
                        Ok(VerificationReportRead::Report {
                            report,
                            source,
                            path,
                        })
                    }
                }
                VerificationReportRead::Malformed => Ok(VerificationReportRead::Malformed),
                VerificationReportRead::Missing => Ok(VerificationReportRead::Report {
                    report,
                    source,
                    path,
                }),
            }
        }
        VerificationReportRead::Report {
            report,
            source,
            path,
        } => match latest_attempt_report_for_command_read(task_dir)? {
            VerificationReportRead::Missing => Ok(VerificationReportRead::Report {
                report,
                source,
                path,
            }),
            VerificationReportRead::Malformed => Ok(VerificationReportRead::Malformed),
            attempt => Ok(attempt),
        },
        VerificationReportRead::Missing => latest_attempt_report_for_command_read(task_dir),
        VerificationReportRead::Malformed => {
            match latest_attempt_report_for_command_read(task_dir)? {
                VerificationReportRead::Missing | VerificationReportRead::Malformed => {
                    Ok(VerificationReportRead::Malformed)
                }
                attempt => Ok(attempt),
            }
        }
    }
}

/// Load persisted proof for an already loaded task and derive its status.
pub fn proof_status_for_task(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<ProofStatus> {
    let selected = read_report_for_task(task, task_dir)?;

    let Some(selected) = selected else {
        return Ok(ProofStatus {
            task_id: task.id.clone(),
            kind: ProofStatusKind::Missing,
            verification_path: display_verification_path(paths, &verification_path(task_dir)),
            verified_at: None,
            verified_commit: None,
            matched_claims: 0,
            total_claims: 0,
            sources: Vec::new(),
            stale_reasons: Vec::new(),
            failures: Vec::new(),
        });
    };

    let classification = classify_report(
        task,
        task_dir,
        current_commit,
        &selected.report,
        FailedStalePolicy::BestEffort,
    )?;
    let stale = classification
        .stale
        .into_iter()
        .map(ProofStaleReason::from)
        .collect();

    Ok(status_from_report(
        task.id.clone(),
        classification.kind,
        display_verification_path(paths, &selected.path),
        selected.report,
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
    if status.kind == ProofStatusKind::Unapplied {
        out.push_str("reason: verification report was not applied to task.yaml\n");
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
            ProofStatusKind::Unapplied => "unapplied",
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailedStalePolicy {
    Skip,
    BestEffort,
}

struct ProofClassification {
    kind: ProofStatusKind,
    stale: Vec<StaleReason>,
}

fn classify_report(
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
    report: &VerificationReport,
    failed_stale_policy: FailedStalePolicy,
) -> Result<ProofClassification> {
    let report_reflected = report_reflected_in_task(task, report);
    if !report_reflected {
        return Ok(ProofClassification {
            kind: ProofStatusKind::Unapplied,
            stale: Vec::new(),
        });
    }
    if report.status == VerificationStatus::Failed {
        let stale = match failed_stale_policy {
            FailedStalePolicy::Skip => Vec::new(),
            FailedStalePolicy::BestEffort => {
                full_status_stale_reasons(task, task_dir, current_commit, report)
                    .unwrap_or_default()
            }
        };
        return Ok(ProofClassification {
            kind: ProofStatusKind::Failed,
            stale,
        });
    }

    let stale = full_status_stale_reasons(task, task_dir, current_commit, report)?;
    Ok(ProofClassification {
        kind: if stale.is_empty() {
            ProofStatusKind::Accepted
        } else {
            ProofStatusKind::Stale
        },
        stale,
    })
}

fn classify_without_freshness(
    task: &task::TaskRecord,
    report: &VerificationReport,
) -> ProofStatusKind {
    if !report_reflected_in_task(task, report) {
        return ProofStatusKind::Unapplied;
    }
    match report.status {
        VerificationStatus::Failed => ProofStatusKind::Failed,
        VerificationStatus::Passed => ProofStatusKind::Accepted,
    }
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

fn report_reflected_in_task(task: &task::TaskRecord, report: &VerificationReport) -> bool {
    if let Some(receipt) = applied_receipt_for_report(report) {
        return task.verification.applied_report.as_ref() == Some(&receipt)
            && (report.status == VerificationStatus::Failed
                || passed_binding_matches_report(&task.verification, report));
    }
    match report.status {
        VerificationStatus::Passed => passed_binding_matches_report(&task.verification, report),
        VerificationStatus::Failed => true,
    }
}

fn canonical_report_can_short_circuit(report: &VerificationReport) -> bool {
    report.status != VerificationStatus::Failed || applied_receipt_for_report(report).is_some()
}

struct SelectedReport {
    report: VerificationReport,
    path: PathBuf,
}

fn read_report_for_task(
    task: &task::TaskRecord,
    task_dir: &Path,
) -> Result<Option<SelectedReport>> {
    recover_canonical_report_for_task(task, task_dir, report_reflected_in_task)?;
    let canonical_path = verification_path(task_dir);
    match read_managed_report_file_if_exists(&canonical_path)? {
        Some(report) if report_reflected_in_task(task, &report) => {
            if !canonical_report_can_short_circuit(&report) {
                if let Some((attempt, path)) = latest_attempt_report(task_dir)? {
                    if verification_report_is_newer(&attempt, &report) {
                        return Ok(Some(SelectedReport {
                            report: attempt,
                            path,
                        }));
                    }
                }
            }
            Ok(Some(SelectedReport {
                report,
                path: canonical_path,
            }))
        }
        canonical => {
            if let Some((report, path)) = latest_attempt_report(task_dir)? {
                return Ok(Some(SelectedReport { report, path }));
            }
            Ok(canonical.map(|report| SelectedReport {
                report,
                path: canonical_path,
            }))
        }
    }
}

fn display_verification_path(paths: &MaestroPaths, verification_file: &Path) -> String {
    verification_file
        .strip_prefix(paths.repo_root())
        .unwrap_or(verification_file)
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
