//! Verification report persistence, reading, and attempt selection.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::verify_task::VerificationReport;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{classify, Compat, VERIFICATION_SCHEMA_VERSION};
use crate::foundation::core::time::parse_utc_timestamp;

pub(super) const LATEST_ATTEMPT_REPORT_FILE: &str = "latest.json";
const MAX_STORED_ATTEMPT_REPORTS: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum VerificationReportRead {
    Missing,
    Malformed,
    Report {
        report: Box<VerificationReport>,
        source: VerificationReportSource,
        path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VerificationReportSource {
    Canonical,
    LatestAttempt,
}

/// Return the path to the verification artifact for a loaded task.
pub fn verification_path(task_dir: &Path) -> PathBuf {
    task_dir.join("verification.json")
}

/// Return the directory that stores non-canonical verification attempts.
pub(crate) fn verification_attempts_dir(task_dir: &Path) -> PathBuf {
    task_dir.join("verification.attempts")
}

pub(super) fn write_task_report(task_dir: &Path, report: &VerificationReport) -> Result<()> {
    let path = verification_path(task_dir);
    write_report_file(&path, report)
}

pub(crate) fn write_task_report_attempt(
    task_dir: &Path,
    report: &VerificationReport,
) -> Result<PathBuf> {
    let attempts_dir = managed_attempts_dir(task_dir)?;
    let path = attempts_dir.join(format!("{}.json", report_file_stem(report)));
    write_report_file(&path, report)?;
    write_report_file(&attempts_dir.join(LATEST_ATTEMPT_REPORT_FILE), report)?;
    prune_old_attempt_reports(&attempts_dir)?;
    Ok(path)
}

fn prune_old_attempt_reports(attempts_dir: &Path) -> Result<()> {
    let entries = fs::read_dir(attempts_dir)
        .with_context(|| format!("failed to read {}", attempts_dir.display()))?;
    let mut attempts = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", attempts_dir.display()))?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name == LATEST_ATTEMPT_REPORT_FILE {
            continue;
        }
        if !is_archived_attempt_file_name(file_name) {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_file() && !file_type.is_symlink() {
            attempts.push(path);
        }
    }

    attempts.sort();
    let remove_count = attempts.len().saturating_sub(MAX_STORED_ATTEMPT_REPORTS);
    for path in attempts.into_iter().take(remove_count) {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to prune {}", path.display()));
            }
        }
    }
    Ok(())
}

fn existing_managed_attempts_dir(task_dir: &Path) -> Result<Option<PathBuf>> {
    let attempts_dir = verification_attempts_dir(task_dir);
    match fs::symlink_metadata(&attempts_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "managed verification attempts path must not be a symlink: {}",
                attempts_dir.display()
            );
        }
        Ok(metadata) if !metadata.is_dir() => {
            bail!(
                "managed verification attempts path must be a directory: {}",
                attempts_dir.display()
            );
        }
        Ok(_) => Ok(Some(attempts_dir)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect {}", attempts_dir.display()))
        }
    }
}

fn managed_attempts_dir(task_dir: &Path) -> Result<PathBuf> {
    let attempts_dir = verification_attempts_dir(task_dir);
    match existing_managed_attempts_dir(task_dir)? {
        Some(path) => Ok(path),
        None => {
            match fs::create_dir(&attempts_dir) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to create {}", attempts_dir.display()));
                }
            }
            match existing_managed_attempts_dir(task_dir)? {
                Some(path) => Ok(path),
                None => bail!(
                    "managed verification attempts path was not created: {}",
                    attempts_dir.display()
                ),
            }
        }
    }
}

pub(super) fn read_managed_report_file_text_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "managed verification report path must not be a symlink: {}",
                path.display()
            );
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!(
                "managed verification report path must be a file: {}",
                path.display()
            );
        }
        Ok(_) => fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(Some),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

pub(crate) fn read_managed_report_file_if_exists(
    path: &Path,
) -> Result<Option<VerificationReport>> {
    let Some(raw) = read_managed_report_file_text_if_exists(path)? else {
        return Ok(None);
    };
    parse_report_file(path, &raw)
}

pub(super) fn read_managed_report_file_for_command_read(
    path: &Path,
    source: VerificationReportSource,
) -> Result<VerificationReportRead> {
    let Some(raw) = read_managed_report_file_text_if_exists(path)? else {
        return Ok(VerificationReportRead::Missing);
    };
    Ok(parse_report_file_for_command_read(path, &raw, source))
}

fn parse_report_file(path: &Path, raw: &str) -> Result<Option<VerificationReport>> {
    let report: VerificationReport =
        serde_json::from_str(raw).with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&report.schema_version, VERIFICATION_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            VERIFICATION_SCHEMA_VERSION,
            report.schema_version
        );
    }
    Ok(Some(report))
}

fn parse_report_file_for_command_read(
    path: &Path,
    raw: &str,
    source: VerificationReportSource,
) -> VerificationReportRead {
    let Ok(mut value) = serde_json::from_str::<Value>(raw) else {
        return VerificationReportRead::Malformed;
    };
    normalize_command_read_report(&mut value);
    let Ok(report) = serde_json::from_value::<VerificationReport>(value) else {
        return VerificationReportRead::Malformed;
    };
    if classify(&report.schema_version, VERIFICATION_SCHEMA_VERSION) != Compat::Exact {
        return VerificationReportRead::Malformed;
    }
    VerificationReportRead::Report {
        report: Box::new(report),
        source,
        path: path.to_path_buf(),
    }
}

fn normalize_command_read_report(value: &mut Value) {
    let Some(commands) = value.get_mut("commands").and_then(Value::as_array_mut) else {
        return;
    };
    for command in commands {
        let legacy_command = match command {
            Value::String(command) => Some(command.clone()),
            _ => None,
        };
        if let Some(command_text) = legacy_command {
            *command = json!({
                "cmd": command_text,
                "exit_code": 0,
                "duration_ms": 0,
            });
        }
    }
}

struct AttemptReportPaths {
    marker_path: PathBuf,
    archived_paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttemptReportReader {
    Strict,
    CommandRead,
}

fn attempt_report_paths(task_dir: &Path) -> Result<Option<AttemptReportPaths>> {
    let Some(attempts_dir) = existing_managed_attempts_dir(task_dir)? else {
        return Ok(None);
    };
    let marker_path = attempts_dir.join(LATEST_ATTEMPT_REPORT_FILE);
    let entries = fs::read_dir(&attempts_dir)
        .with_context(|| format!("failed to read {}", attempts_dir.display()))?;
    let mut archived_paths = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", attempts_dir.display()))?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name == LATEST_ATTEMPT_REPORT_FILE {
            continue;
        }
        if !is_archived_attempt_file_name(file_name) {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_symlink() {
            bail!(
                "managed verification attempt path must not be a symlink: {}",
                path.display()
            );
        }
        if !file_type.is_file() {
            bail!(
                "managed verification attempt path must be a file: {}",
                path.display()
            );
        }
        archived_paths.push(path);
    }
    archived_paths.sort_by(|left, right| right.cmp(left));
    Ok(Some(AttemptReportPaths {
        marker_path,
        archived_paths,
    }))
}

fn is_archived_attempt_file_name(file_name: &str) -> bool {
    !file_name.starts_with('.') && file_name.ends_with(".json")
}

pub(crate) fn latest_attempt_report(
    task_dir: &Path,
) -> Result<Option<(VerificationReport, PathBuf)>> {
    match latest_attempt_report_candidate(task_dir, AttemptReportReader::Strict)? {
        VerificationReportRead::Missing => Ok(None),
        VerificationReportRead::Malformed => bail!("malformed verification attempt report"),
        VerificationReportRead::Report { report, path, .. } => Ok(Some((*report, path))),
    }
}

pub(super) fn latest_attempt_report_for_command_read(
    task_dir: &Path,
) -> Result<VerificationReportRead> {
    latest_attempt_report_candidate_for_command_read(task_dir)
}

pub(super) fn latest_attempt_report_candidate_for_command_read(
    task_dir: &Path,
) -> Result<VerificationReportRead> {
    latest_attempt_report_candidate(task_dir, AttemptReportReader::CommandRead)
}

fn latest_attempt_report_candidate(
    task_dir: &Path,
    reader: AttemptReportReader,
) -> Result<VerificationReportRead> {
    let Some(paths) = attempt_report_paths(task_dir)? else {
        return Ok(VerificationReportRead::Missing);
    };
    let marker = read_attempt_report_candidate(
        &paths.marker_path,
        VerificationReportSource::LatestAttempt,
        reader,
        true,
    )?;
    if reader == AttemptReportReader::Strict && matches!(marker, VerificationReportRead::Malformed)
    {
        return Ok(VerificationReportRead::Malformed);
    }
    let mut saw_malformed = matches!(marker, VerificationReportRead::Malformed);
    let mut selected = match marker {
        report @ VerificationReportRead::Report { .. } => Some(report),
        VerificationReportRead::Missing | VerificationReportRead::Malformed => None,
    };

    for path in paths.archived_paths {
        match read_attempt_report_candidate(
            &path,
            VerificationReportSource::LatestAttempt,
            reader,
            false,
        )? {
            report @ VerificationReportRead::Report { .. } => {
                if selected
                    .as_ref()
                    .map(|selected| report_is_newer(&report, selected))
                    .unwrap_or(true)
                {
                    selected = Some(report);
                }
            }
            VerificationReportRead::Malformed => {
                saw_malformed = true;
                if reader == AttemptReportReader::Strict
                    && malformed_archive_may_be_newer_than_selected(&path, selected.as_ref())
                {
                    return Ok(VerificationReportRead::Malformed);
                }
            }
            VerificationReportRead::Missing => {}
        }
    }
    if let Some(report) = selected {
        Ok(report)
    } else if saw_malformed {
        Ok(VerificationReportRead::Malformed)
    } else {
        Ok(VerificationReportRead::Missing)
    }
}

fn malformed_archive_may_be_newer_than_selected(
    archive_path: &Path,
    selected: Option<&VerificationReportRead>,
) -> bool {
    let Some(VerificationReportRead::Report { report, .. }) = selected else {
        return true;
    };
    let Some(archive_stem) = archive_path.file_stem().and_then(|name| name.to_str()) else {
        return true;
    };
    archive_stem > report_file_stem(report).as_str()
}

fn report_is_newer(candidate: &VerificationReportRead, selected: &VerificationReportRead) -> bool {
    match (candidate, selected) {
        (
            VerificationReportRead::Report {
                report: candidate, ..
            },
            VerificationReportRead::Report {
                report: selected, ..
            },
        ) => verification_report_is_newer(candidate, selected),
        _ => false,
    }
}

pub(super) fn verification_report_is_newer(
    candidate: &VerificationReport,
    selected: &VerificationReport,
) -> bool {
    report_ordering(candidate, selected) == std::cmp::Ordering::Greater
}

fn report_order_key(report: &VerificationReport) -> (&str, &str) {
    (report.verified_at.as_str(), attempt_id_or_empty(report))
}

fn report_ordering(left: &VerificationReport, right: &VerificationReport) -> std::cmp::Ordering {
    match (report_timestamp_nanos(left), report_timestamp_nanos(right)) {
        (Some(left), Some(right)) => match left.cmp(&right) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        },
        (Some(_), None) => return std::cmp::Ordering::Greater,
        (None, Some(_)) => return std::cmp::Ordering::Less,
        (None, None) => {}
    }
    report_order_key(left).cmp(&report_order_key(right))
}

fn report_timestamp_nanos(report: &VerificationReport) -> Option<i128> {
    parse_report_timestamp_nanos(&report.verified_at)
}

fn parse_report_timestamp_nanos(value: &str) -> Option<i128> {
    if value.chars().all(|character| character.is_ascii_digit()) {
        return parse_numeric_report_timestamp_nanos(value);
    }
    parse_utc_timestamp(value).map(|timestamp| timestamp.nanos_since_epoch)
}

fn parse_numeric_report_timestamp_nanos(value: &str) -> Option<i128> {
    let timestamp = value.parse::<i128>().ok()?;
    match value.len() {
        0..=10 => timestamp.checked_mul(1_000_000_000),
        11..=13 => timestamp.checked_mul(1_000_000),
        14..=16 => timestamp.checked_mul(1_000),
        _ => Some(timestamp),
    }
}

fn attempt_id_or_empty(report: &VerificationReport) -> &str {
    report.attempt_id.as_deref().unwrap_or_default()
}

fn read_attempt_report_candidate(
    path: &Path,
    source: VerificationReportSource,
    reader: AttemptReportReader,
    managed: bool,
) -> Result<VerificationReportRead> {
    let raw = if managed {
        read_managed_report_file_text_if_exists(path)?
    } else {
        match read_to_string_if_exists(path) {
            Ok(raw) => raw,
            Err(_) => return Ok(VerificationReportRead::Malformed),
        }
    };
    let Some(raw) = raw else {
        return Ok(VerificationReportRead::Missing);
    };
    Ok(match reader {
        AttemptReportReader::Strict => parse_report_file_for_strict_candidate(path, &raw, source),
        AttemptReportReader::CommandRead => parse_report_file_for_command_read(path, &raw, source),
    })
}

fn parse_report_file_for_strict_candidate(
    path: &Path,
    raw: &str,
    source: VerificationReportSource,
) -> VerificationReportRead {
    let Ok(Some(report)) = parse_report_file(path, raw) else {
        return VerificationReportRead::Malformed;
    };
    VerificationReportRead::Report {
        report: Box::new(report),
        source,
        path: path.to_path_buf(),
    }
}

pub(super) fn write_report_file(path: &Path, report: &VerificationReport) -> Result<()> {
    let raw = serde_json::to_string_pretty(report)?;
    write_string_atomic(path, &format!("{raw}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn report_file_stem(report: &VerificationReport) -> String {
    report
        .attempt_id
        .as_deref()
        .unwrap_or(report.verified_at.as_str())
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::stale::StoredFreshness;
    use super::super::verify_task::{
        VerificationReport, VerificationStatus, VerificationTaskSnapshot,
    };
    use super::{
        latest_attempt_report, latest_attempt_report_for_command_read, verification_attempts_dir,
        VerificationReportRead,
    };
    use crate::foundation::core::schema::VERIFICATION_SCHEMA_VERSION;

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn latest_attempt_selection_uses_parsed_timestamp_order() {
        let temp = TestTempDir::new("maestro-proof-attempt-timestamp-order");
        let task_dir = temp.path().join("task-001");
        let attempts_dir = verification_attempts_dir(&task_dir);
        fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be creatable");
        let stale_marker = report("task-001", "stale-marker", "900");
        let newer_archive = report("task-001", "newer-archive", "1000");
        super::write_report_file(&attempts_dir.join("latest.json"), &stale_marker)
            .expect("invariant: marker report should be writable");
        super::write_report_file(&attempts_dir.join("zz-newer-archive.json"), &newer_archive)
            .expect("invariant: archived report should be writable");

        let (status_report, status_path) = latest_attempt_report(&task_dir)
            .expect("invariant: status selector should not fail")
            .expect("invariant: status selector should find an attempt");
        assert_eq!(status_report.attempt_id.as_deref(), Some("newer-archive"));
        assert_eq!(
            status_path.file_name().and_then(|name| name.to_str()),
            Some("zz-newer-archive.json")
        );

        let command_read = latest_attempt_report_for_command_read(&task_dir)
            .expect("invariant: command-read selector should not fail");
        match command_read {
            VerificationReportRead::Report { report, path, .. } => {
                assert_eq!(report.attempt_id.as_deref(), Some("newer-archive"));
                assert_eq!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("zz-newer-archive.json")
                );
            }
            other => panic!("expected newer archived report, got {other:?}"),
        }
    }

    #[test]
    fn latest_attempt_selection_ignores_older_malformed_archived_attempt() {
        let temp = TestTempDir::new("maestro-proof-attempt-older-malformed");
        let task_dir = temp.path().join("task-001");
        let attempts_dir = verification_attempts_dir(&task_dir);
        fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be creatable");
        let current = report("task-001", "zz-current-marker", "2000");
        super::write_report_file(&attempts_dir.join("latest.json"), &current)
            .expect("invariant: marker report should be writable");
        fs::write(attempts_dir.join("aa-old-archive.json"), "{not-json")
            .expect("invariant: malformed archive should be writable");

        let (status_report, status_path) = latest_attempt_report(&task_dir)
            .expect("invariant: status selector should not fail")
            .expect("invariant: status selector should find an attempt");
        assert_eq!(
            status_report.attempt_id.as_deref(),
            Some("zz-current-marker")
        );
        assert_eq!(
            status_path.file_name().and_then(|name| name.to_str()),
            Some("latest.json")
        );

        let command_read = latest_attempt_report_for_command_read(&task_dir)
            .expect("invariant: command-read selector should not fail");
        match command_read {
            VerificationReportRead::Report { report, path, .. } => {
                assert_eq!(report.attempt_id.as_deref(), Some("zz-current-marker"));
                assert_eq!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("latest.json")
                );
            }
            other => panic!("expected marker report, got {other:?}"),
        }
    }

    #[test]
    fn latest_attempt_selection_reports_malformed_newer_archive_before_valid_archive() {
        let temp = TestTempDir::new("maestro-proof-attempt-newer-malformed");
        let task_dir = temp.path().join("task-001");
        let attempts_dir = verification_attempts_dir(&task_dir);
        fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be creatable");
        fs::write(attempts_dir.join("zz-newer-malformed.json"), "{not-json")
            .expect("invariant: malformed archive should be writable");
        let archived = report("task-001", "aa-valid-archive", "1000");
        super::write_report_file(&attempts_dir.join("aa-valid-archive.json"), &archived)
            .expect("invariant: archived report should be writable");

        let error = latest_attempt_report(&task_dir)
            .expect_err("invariant: strict selector should report malformed attempts");
        assert!(error
            .to_string()
            .contains("malformed verification attempt report"));

        let command_read = latest_attempt_report_for_command_read(&task_dir)
            .expect("invariant: command-read selector should not fail");
        match command_read {
            VerificationReportRead::Report { report, .. } => {
                assert_eq!(report.attempt_id.as_deref(), Some("aa-valid-archive"));
            }
            other => panic!("expected archived report, got {other:?}"),
        }
    }

    #[test]
    fn command_read_attempt_selection_falls_back_to_valid_archive_after_malformed_marker() {
        let temp = TestTempDir::new("maestro-proof-attempt-malformed-marker");
        let task_dir = temp.path().join("task-001");
        let attempts_dir = verification_attempts_dir(&task_dir);
        fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be creatable");
        fs::write(attempts_dir.join("latest.json"), "{not-json")
            .expect("invariant: malformed marker should be writable");
        let archived = report("task-001", "archived-attempt", "1000");
        super::write_report_file(&attempts_dir.join("zz-archived-attempt.json"), &archived)
            .expect("invariant: archived report should be writable");

        let error = latest_attempt_report(&task_dir)
            .expect_err("invariant: strict selector should report malformed attempts");
        assert!(error
            .to_string()
            .contains("malformed verification attempt report"));

        let command_read = latest_attempt_report_for_command_read(&task_dir)
            .expect("invariant: command-read selector should not fail");
        match command_read {
            VerificationReportRead::Report { report, .. } => {
                assert_eq!(report.attempt_id.as_deref(), Some("archived-attempt"));
            }
            other => panic!("expected archived report, got {other:?}"),
        }
    }

    #[test]
    fn latest_attempt_selection_reports_malformed_when_no_valid_attempt_exists() {
        let temp = TestTempDir::new("maestro-proof-attempt-all-malformed");
        let task_dir = temp.path().join("task-001");
        let attempts_dir = verification_attempts_dir(&task_dir);
        fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be creatable");
        fs::write(attempts_dir.join("latest.json"), "{not-json")
            .expect("invariant: malformed marker should be writable");

        let error = latest_attempt_report(&task_dir)
            .expect_err("invariant: status selector should report malformed attempts");
        assert!(error
            .to_string()
            .contains("malformed verification attempt report"));

        let command_read = latest_attempt_report_for_command_read(&task_dir)
            .expect("invariant: command-read selector should not fail");
        assert!(matches!(command_read, VerificationReportRead::Malformed));
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
