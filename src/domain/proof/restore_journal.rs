//! Canonical verification report restore journal (write-ahead log).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::attempts::{
    read_managed_report_file_if_exists, read_managed_report_file_text_if_exists, verification_path,
    write_task_report,
};
use super::verify_task::VerificationReport;
use crate::domain::task;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{classify, Compat, VERIFICATION_RESTORE_SCHEMA_VERSION};

const CANONICAL_REPORT_RESTORE_FILE: &str = "verification.json.restore";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CanonicalReportRestoreJournal {
    schema_version: String,
    previous: Option<String>,
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

pub(crate) fn recover_canonical_report_for_task(
    task: &task::TaskRecord,
    task_dir: &Path,
    report_reflected: impl Fn(&task::TaskRecord, &VerificationReport) -> bool,
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
        schema_version: VERIFICATION_RESTORE_SCHEMA_VERSION.to_string(),
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
    if classify(&journal.schema_version, VERIFICATION_RESTORE_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            VERIFICATION_RESTORE_SCHEMA_VERSION,
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::attempts::{verification_path, write_task_report};
    use super::super::stale::StoredFreshness;
    use super::super::verify_task::{
        verification_outcome_for_report, VerificationReport, VerificationStatus,
        VerificationTaskSnapshot,
    };
    use super::{recover_canonical_report_for_task, replace_task_report_preserving_previous};
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
        write_task_report(&task_dir, &previous)
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
        write_task_report(&task_dir, &previous)
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
            freshness: StoredFreshness {
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
