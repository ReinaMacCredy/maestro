use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::store;
use crate::domain::task::template::TaskRecord;
use crate::foundation::core::fs::{
    ensure_dir, read_to_string_if_exists, write_string_if_unchanged,
};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{Compat, PROGRESS_SCHEMA_VERSION, classify};

pub const PROGRESS_FILE: &str = "progress.yml";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProgressFile {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskRecord>,
}

impl ProgressFile {
    pub fn new(agent: Option<String>, session_id: Option<String>) -> Self {
        Self {
            schema_version: PROGRESS_SCHEMA_VERSION.to_string(),
            session_id,
            agent,
            current_task: None,
            tasks: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressSnapshot {
    pub progress: Option<ProgressFile>,
    raw: Option<String>,
}

pub fn progress_path(paths: &MaestroPaths, progress_id: &str) -> PathBuf {
    paths.cards_dir().join(progress_id).join(PROGRESS_FILE)
}

pub fn load_with_snapshot(path: &Path) -> Result<ProgressSnapshot> {
    if path.parent().is_some_and(store::is_symlink) {
        bail!(
            "progress dir {} is a symlink; the card store refuses symlinked dirs",
            path.parent().unwrap_or(path).display()
        );
    }
    let Some(contents) = read_to_string_if_exists(path)? else {
        return Ok(ProgressSnapshot {
            progress: None,
            raw: None,
        });
    };
    let progress: ProgressFile = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&progress.schema_version, PROGRESS_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "unsupported progress schema {} in {}; expected {}",
            progress.schema_version,
            path.display(),
            PROGRESS_SCHEMA_VERSION
        );
    }
    Ok(ProgressSnapshot {
        progress: Some(progress),
        raw: Some(contents),
    })
}

pub fn save_with_snapshot(
    path: &Path,
    progress: &ProgressFile,
    snapshot: &ProgressSnapshot,
) -> Result<()> {
    if snapshot.raw.is_none()
        && let Some(parent) = path.parent()
    {
        ensure_dir(parent)?;
    }
    let contents = serde_yaml::to_string(progress).context("failed to serialize progress")?;
    write_string_if_unchanged(path, snapshot.raw.as_deref(), &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::domain::task::{TaskRecord, TaskState};

    use super::*;

    fn temp_paths(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        MaestroPaths::new(
            std::env::temp_dir().join(format!("maestro-{label}-{}-{nanos}", process::id())),
        )
    }

    #[test]
    fn task_progress_storage_round_trips_task_records() {
        let paths = temp_paths("task-progress-storage");
        let path = progress_path(&paths, "progress-doc-cleanup-019f");
        let snapshot = load_with_snapshot(&path).expect("missing progress file loads");
        assert_eq!(snapshot.progress, None);

        let mut progress = ProgressFile::new(Some("codex".to_string()), Some("s1".to_string()));
        progress.current_task = Some("task-fix-typo-a83f".to_string());
        let mut task = TaskRecord::draft(
            "task-fix-typo-a83f",
            "fix typo in README",
            "2026-06-26T00:00:00Z",
        );
        task.state = TaskState::Ready;
        progress.tasks.push(task);

        save_with_snapshot(&path, &progress, &snapshot).expect("save progress file");
        let loaded = load_with_snapshot(&path)
            .expect("reload progress file")
            .progress
            .expect("progress file exists");

        assert_eq!(loaded.schema_version, PROGRESS_SCHEMA_VERSION);
        assert_eq!(loaded.agent.as_deref(), Some("codex"));
        assert_eq!(loaded.session_id.as_deref(), Some("s1"));
        assert_eq!(loaded.current_task.as_deref(), Some("task-fix-typo-a83f"));
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].id, "task-fix-typo-a83f");
        assert_eq!(loaded.tasks[0].state, TaskState::Ready);

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }

    #[test]
    fn task_progress_storage_rejects_stale_writer() {
        let paths = temp_paths("task-progress-stale");
        let path = progress_path(&paths, "progress-doc-cleanup-019f");
        let first = load_with_snapshot(&path).expect("first snapshot");
        let second = load_with_snapshot(&path).expect("second snapshot");

        let mut winner = ProgressFile::new(Some("codex".to_string()), Some("s1".to_string()));
        winner.current_task = Some("task-winner".to_string());
        save_with_snapshot(&path, &winner, &second).expect("winner saves");

        let loser = ProgressFile::new(Some("codex".to_string()), Some("s1".to_string()));
        let error = save_with_snapshot(&path, &loser, &first).expect_err("stale writer rejected");
        assert!(format!("{error:#}").contains("changed since it was read"));

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }
}
