//! Legacy compatibility shim for Proof.

pub mod events {
    pub use crate::domain::proof::compatibility::managed_event_files;
}

pub mod proof_status {
    use anyhow::Result;

    use crate::domain::proof::compatibility;
    pub use crate::domain::proof::ProofStatusKind;

    use super::stale::StaleReason;
    use super::verify_task::VerificationReport;
    use crate::foundation::core::paths::MaestroPaths;

    /// Legacy proof status shape preserved for `crate::verification` callers.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ProofStatus {
        pub task_id: String,
        pub kind: ProofStatusKind,
        pub verification_path: String,
        pub report: Option<VerificationReport>,
        pub stale_reasons: Vec<StaleReason>,
    }

    /// Load persisted proof using the legacy `crate::verification` status contract.
    pub fn proof_status(paths: &MaestroPaths, task_id: &str) -> Result<ProofStatus> {
        compatibility::legacy_proof_status(paths, task_id).map(from_domain_compatibility)
    }

    /// Render proof status using the legacy `crate::verification` status contract.
    pub fn render_proof_status(status: &ProofStatus) -> String {
        compatibility::render_legacy_proof_status(&to_domain_compatibility(status))
    }

    fn from_domain_compatibility(status: compatibility::LegacyProofStatus) -> ProofStatus {
        ProofStatus {
            task_id: status.task_id,
            kind: status.kind,
            verification_path: status.verification_path,
            report: status.report,
            stale_reasons: status.stale_reasons,
        }
    }

    fn to_domain_compatibility(status: &ProofStatus) -> compatibility::LegacyProofStatus {
        compatibility::LegacyProofStatus {
            task_id: status.task_id.clone(),
            kind: status.kind.clone(),
            verification_path: status.verification_path.clone(),
            report: status.report.clone(),
            stale_reasons: status.stale_reasons.clone(),
        }
    }
}

pub mod stale {
    pub use crate::domain::proof::compatibility::{
        is_fresh, stale_reasons, FreshnessInputs, StaleReason, StoredFreshness,
    };
}

pub mod verify_task {
    use anyhow::Result;

    use crate::foundation::core::paths::MaestroPaths;

    pub use crate::domain::proof::compatibility::{
        freshness_inputs, freshness_inputs_for_task, load_task_by_id, read_report,
        verification_path, ClaimCheck, LoadedTask, ProofSource, VerificationCommand,
        VerificationReport, VerificationStatus,
    };

    /// Legacy report-returning verification entrypoint.
    pub fn verify_task(
        paths: &MaestroPaths,
        task_id: &str,
        actor: &str,
    ) -> Result<VerificationReport> {
        crate::operations::verify_task_report(paths, task_id, actor)
    }
}
