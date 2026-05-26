//! Temporary compatibility alias for the Task domain facade.

pub mod blockers {
    use anyhow::Result;

    pub use crate::domain::task::has_unresolved_blockers;
    use crate::domain::task::{BlockerKind, BlockerRef, TaskRecord};

    pub fn add_blocker(
        task: &mut TaskRecord,
        id: String,
        kind: BlockerKind,
        blocked_ref: Option<BlockerRef>,
        title: String,
        reason: String,
        created_at: String,
    ) {
        crate::domain::task::blockers::add_blocker(
            task,
            id,
            kind,
            blocked_ref,
            title,
            reason,
            created_at,
        );
    }

    pub fn resolve_blocker(
        task: &mut TaskRecord,
        blocker_id: &str,
        resolved_at: String,
    ) -> Result<()> {
        crate::domain::task::blockers::resolve_blocker(task, blocker_id, resolved_at)
    }
}

pub mod display {
    use crate::domain::task::TaskRecord;

    pub fn render_task(task: &TaskRecord) -> String {
        crate::domain::task::render_task(task)
    }

    pub fn render_task_list(tasks: &[TaskRecord]) -> String {
        crate::domain::task::render_task_list(tasks)
    }
}

pub mod doctor {
    use std::path::Path;

    use anyhow::Result;

    use crate::domain::task::TaskRecord;
    pub use crate::domain::task::{TaskDoctorReport, TaskEntry};

    pub fn load_task_records(tasks_dir: &Path) -> Result<Vec<TaskRecord>> {
        crate::domain::task::load_task_records(tasks_dir)
    }

    pub fn load_task_entries(tasks_dir: &Path) -> Result<Vec<TaskEntry>> {
        crate::domain::task::load_task_entries(tasks_dir)
    }

    pub fn check_blocker_graph(tasks_dir: &Path) -> Result<TaskDoctorReport> {
        crate::domain::task::check_blocker_graph(tasks_dir)
    }

    pub fn render_report(report: &TaskDoctorReport) -> String {
        crate::domain::task::render_report(report)
    }
}

pub mod lifecycle {
    use anyhow::Result;

    pub use crate::domain::task::TransitionDetails;
    use crate::domain::task::{TaskRecord, TaskState};

    pub fn transition(
        task: &mut TaskRecord,
        to: TaskState,
        by: &str,
        at: &str,
        details: TransitionDetails,
    ) -> Result<()> {
        crate::domain::task::lifecycle::transition(task, to, by, at, details)
    }
}

pub mod lookup {
    use std::fs;
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use crate::domain::task::template::TaskSnapshot;
    use crate::domain::task::TaskRecord;

    pub fn resolve_task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
        crate::domain::task::lookup::resolve_task_yaml_path(tasks_dir, id)
    }

    pub fn task_yaml_path_for_entry(entry: &fs::DirEntry) -> Result<Option<PathBuf>> {
        crate::domain::task::lookup::task_yaml_path_for_entry(entry)
    }

    pub fn valid_task_yaml_path(path: &Path) -> Result<bool> {
        crate::domain::task::lookup::valid_task_yaml_path(path)
    }

    pub fn load_task_with_snapshot(
        tasks_dir: &Path,
        id: &str,
    ) -> Result<(TaskRecord, TaskSnapshot, PathBuf)> {
        crate::domain::task::lookup::load_task_with_snapshot(tasks_dir, id)
    }
}

pub mod template {
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    pub use crate::domain::task::template::{StateHistoryEntry, TaskSnapshot};
    pub use crate::domain::task::{
        AcceptanceFile, Blocker, BlockerKind, BlockerRef, BlockerSource, TaskRecord, TaskState,
        VerificationBinding,
    };

    /// Legacy binding-only proof state retained for `crate::task` compatibility.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ProofState {
        Missing,
        Failed,
        Accepted,
        Stale,
    }

    /// Temporary legacy extension trait for binding-only proof-state
    /// classification without adding this method to the target domain type.
    pub trait LegacyProofStateExt {
        /// Compute the legacy binding-only proof state.
        fn proof_state(
            &self,
            current_commit: Option<&str>,
            current_acceptance_hash: Option<&str>,
            current_checks_hash: Option<&str>,
            latest_failed: bool,
        ) -> ProofState;
    }

    impl LegacyProofStateExt for VerificationBinding {
        fn proof_state(
            &self,
            current_commit: Option<&str>,
            current_acceptance_hash: Option<&str>,
            current_checks_hash: Option<&str>,
            latest_failed: bool,
        ) -> ProofState {
            if latest_failed {
                return ProofState::Failed;
            }
            let Some(verified_commit) = self.verified_commit.as_deref() else {
                return ProofState::Missing;
            };
            if Some(verified_commit) != current_commit
                || self.acceptance_hash.as_deref() != current_acceptance_hash
                || self.checks_hash.as_deref() != current_checks_hash
            {
                return ProofState::Stale;
            }
            ProofState::Accepted
        }
    }

    pub fn write_task_artifacts(
        tasks_dir: &Path,
        task: &TaskRecord,
        acceptance: &AcceptanceFile,
    ) -> Result<PathBuf> {
        crate::domain::task::template::write_task_artifacts(tasks_dir, task, acceptance)
    }

    pub fn load_task(path: &Path) -> Result<(TaskRecord, TaskSnapshot)> {
        crate::domain::task::template::load_task(path)
    }

    pub fn save_task_with_snapshot(task: &TaskRecord, snapshot: &TaskSnapshot) -> Result<()> {
        crate::domain::task::template::save_task_with_snapshot(task, snapshot)
    }
}
