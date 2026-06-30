//! Feature aggregate facade.

pub(crate) mod archive;
mod qa;
pub mod query;
pub(crate) mod registry;
pub mod schema;
mod staleness;
mod verification;
pub(crate) mod worktree;

pub use archive::{
    AutoArchiveReceipt, FeatureArchiveReport, LooseSweepReport, append_auto_archive_receipt,
    archive_feature, archive_feature_with_expected_hash, archive_loose, unarchive_feature,
};
pub use registry::{
    AcceptanceTextEdit, AmendReport, CancelReport, ContractAdditions, ContractChangeCounts,
    ContractEdits, FeatureDiagnostic, FeatureRosterEntry, FeatureView, FinalizeReport, NoteReport,
    ReopenReport, SetReport, SpecSectionReport, TransitionReport, accept, accept_with_qa_none,
    amend, amend_log_position, cancel, close, close_gaps, create, diagnose, ensure_exists,
    feature_sidecar_dir, finalize, handoff_gap, list, list_archived, list_tolerant,
    list_tolerant_with_entries, list_with_entries, note, read_sidecar_text, reopen, set,
    set_with_report, show, show_archived, start, status, status_label, titles,
    verified_child_commit_drift, write_sidecar_text, write_spec_section,
};
pub use schema::{FeatureStatus, normalize_acceptance_id};
pub use staleness::{RETIRE_REMINDER, STALE_PROPOSED_THRESHOLD_DAYS, age_days, is_stale_proposed};
pub use verification::{
    AcceptanceCoverage, AcceptanceProof, AcceptanceSweepItem, AcceptanceSweepReport,
    FeatureProofUpdate, FeatureVerifyReport, acceptance_coverage, acceptance_coverage_archived,
    acceptance_id, uncovered_acceptance, verify_feature,
};
pub(crate) use worktree::{
    WorktreeCleanupReceipt, WorktreeComputedState, WorktreeIntent, WorktreeLaneStatus,
    WorktreeMilestoneKind, WorktreeRecordReport, lane_statuses, mark_lane, plan_lane,
    record_cleanup,
};
