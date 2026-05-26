//! Verification proof status helpers.

use anyhow::Result;

use crate::foundation::core::paths::MaestroPaths;
use crate::verification::stale::{stale_reasons, StaleReason};
use crate::verification::verify_task::{
    freshness_inputs, load_task_by_id, read_report, verification_path, ClaimCheck, ProofSource,
    VerificationReport, VerificationStatus,
};

/// Derived status for a task's persisted proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofStatusKind {
    Missing,
    Failed,
    Accepted,
    Stale,
}

/// User-facing proof status loaded from `verification.json`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofStatus {
    pub task_id: String,
    pub kind: ProofStatusKind,
    pub verification_path: String,
    pub report: Option<VerificationReport>,
    pub stale_reasons: Vec<StaleReason>,
}

/// Load persisted proof and derive freshness against current task inputs.
pub fn proof_status(paths: &MaestroPaths, task_id: &str) -> Result<ProofStatus> {
    let loaded = load_task_by_id(paths, task_id)?;
    let report = read_report(&loaded.task_dir)?;
    let verification_file = verification_path(&loaded.task_dir);
    let path = verification_file
        .strip_prefix(paths.repo_root())
        .unwrap_or(&verification_file)
        .display()
        .to_string();

    let Some(report) = report else {
        return Ok(ProofStatus {
            task_id: loaded.task.id,
            kind: ProofStatusKind::Missing,
            verification_path: path,
            report: None,
            stale_reasons: Vec::new(),
        });
    };

    let current = freshness_inputs(paths, &loaded)?;
    let stale = stale_reasons(&current, &report.freshness);
    let kind = match report.status {
        VerificationStatus::Failed => ProofStatusKind::Failed,
        VerificationStatus::Passed if stale.is_empty() => ProofStatusKind::Accepted,
        VerificationStatus::Passed => ProofStatusKind::Stale,
    };

    Ok(ProofStatus {
        task_id: loaded.task.id,
        kind,
        verification_path: path,
        report: Some(report),
        stale_reasons: stale,
    })
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

    let Some(report) = status.report.as_ref() else {
        out.push_str("reason: missing verification.json\n");
        return out;
    };

    out.push_str(&format!("verified_at: {}\n", report.verified_at));
    out.push_str(&format!(
        "commit: {}\n",
        report
            .freshness
            .verified_commit
            .as_deref()
            .unwrap_or("<none>")
    ));
    out.push_str(&format_claims(&report.claims));
    out.push_str(&format_sources(&report.proof_sources));

    if !status.stale_reasons.is_empty() {
        out.push_str("stale_reasons:\n");
        for reason in &status.stale_reasons {
            out.push_str(&format!(
                "- {} expected {} found {}\n",
                reason.field, reason.expected, reason.actual
            ));
        }
    }
    if !report.failures.is_empty() {
        out.push_str("failures:\n");
        for failure in &report.failures {
            out.push_str(&format!("- {failure}\n"));
        }
    }

    out
}

impl ProofStatusKind {
    fn label(&self) -> &'static str {
        match self {
            ProofStatusKind::Missing => "missing",
            ProofStatusKind::Failed => "failed",
            ProofStatusKind::Accepted => "accepted",
            ProofStatusKind::Stale => "stale",
        }
    }
}

fn format_claims(claims: &[ClaimCheck]) -> String {
    let matched = claims.iter().filter(|claim| claim.matched).count();
    format!("claims: {matched}/{}\n", claims.len())
}

fn format_sources(sources: &[ProofSource]) -> String {
    if sources.is_empty() {
        return "sources: 0\n".to_string();
    }

    let mut out = format!("sources: {}\n", sources.len());
    for source in sources {
        out.push_str(&format!("- {} {}\n", source.kind, source.path));
    }
    out
}
