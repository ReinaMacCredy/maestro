use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::query::{self, Coarse};
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self, CardHome};
use crate::domain::task::template::{TaskRecord, TaskState};
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressTaskSnapshot {
    pub path: PathBuf,
    progress: ProgressFile,
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

pub fn add_simple_task(
    paths: &MaestroPaths,
    title: &str,
    project: Option<String>,
    created_at: String,
    actor: &str,
) -> Result<TaskRecord> {
    if title.trim().is_empty() {
        bail!("task title must not be empty");
    }
    let id = store::mint_card_id(paths, CardType::Task, title);
    let mut task = TaskRecord::draft(&id, title, &created_at);
    task.state = TaskState::Ready;
    task.acceptance_locked = true;
    task.acceptance.locked_by = Some(actor.to_string());
    task.acceptance.locked_at = Some(created_at.clone());

    let (path, mut progress, snapshot) =
        load_or_create_actor_progress(paths, project, actor, &created_at)?;
    progress.tasks.push(task.clone());
    save_with_snapshot(&path, &progress, &snapshot)?;
    Ok(task)
}

pub fn load_task_with_snapshot(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, ProgressTaskSnapshot, PathBuf)>> {
    store::validate_card_id(id)?;
    for (card, card_path) in query::scan_dir_with_paths(&paths.cards_dir())? {
        if card.card_type != CardType::Progress {
            continue;
        }
        let Some(progress_dir) = card_path.parent() else {
            continue;
        };
        let path = progress_dir.join(PROGRESS_FILE);
        let snapshot = load_with_snapshot(&path)?;
        let Some(progress) = snapshot.progress.clone() else {
            continue;
        };
        if let Some(task) = progress.tasks.iter().find(|task| task.id == id) {
            return Ok(Some((
                task.clone(),
                ProgressTaskSnapshot {
                    path,
                    progress,
                    raw: snapshot.raw,
                },
                progress_dir.to_path_buf(),
            )));
        }
    }
    Ok(None)
}

pub fn scan(paths: &MaestroPaths) -> Result<Vec<(TaskRecord, PathBuf)>> {
    let mut records = Vec::new();
    for (card, card_path) in query::scan_dir_with_paths(&paths.cards_dir())? {
        collect_tasks_from_progress_card(&mut records, &card, &card_path)?;
    }
    Ok(records)
}

pub fn scan_with_cards(paths: &MaestroPaths) -> Result<Vec<(TaskRecord, Card, PathBuf)>> {
    let mut records = Vec::new();
    for (card, card_path) in query::scan_dir_with_paths(&paths.cards_dir())? {
        if card.card_type != CardType::Progress {
            continue;
        }
        let Some(progress_dir) = card_path.parent() else {
            continue;
        };
        let path = progress_dir.join(PROGRESS_FILE);
        if let Some(progress) = load_with_snapshot(&path)?.progress {
            records.extend(
                progress
                    .tasks
                    .into_iter()
                    .map(|task| (task, card.clone(), progress_dir.to_path_buf())),
            );
        }
    }
    Ok(records)
}

pub(crate) fn scan_in_cards(cards: &[(Card, PathBuf)]) -> Result<Vec<(TaskRecord, PathBuf)>> {
    let mut records = Vec::new();
    for (card, card_path) in cards {
        collect_tasks_from_progress_card(&mut records, card, card_path)?;
    }
    Ok(records)
}

fn collect_tasks_from_progress_card(
    records: &mut Vec<(TaskRecord, PathBuf)>,
    card: &Card,
    card_path: &Path,
) -> Result<()> {
    if card.card_type != CardType::Progress {
        return Ok(());
    }
    let Some(progress_dir) = card_path.parent() else {
        return Ok(());
    };
    let path = progress_dir.join(PROGRESS_FILE);
    if let Some(progress) = load_with_snapshot(&path)?.progress {
        records.extend(
            progress
                .tasks
                .into_iter()
                .map(|task| (task, progress_dir.to_path_buf())),
        );
    }
    Ok(())
}

pub fn save_task_with_snapshot(task: &TaskRecord, snapshot: &ProgressTaskSnapshot) -> Result<()> {
    let mut progress = snapshot.progress.clone();
    let Some(slot) = progress
        .tasks
        .iter_mut()
        .find(|candidate| candidate.id == task.id)
    else {
        bail!(
            "task {} no longer exists in {}",
            task.id,
            snapshot.path.display()
        );
    };
    *slot = task.clone();
    if task.state == TaskState::InProgress {
        progress.current_task = Some(task.id.clone());
    } else if progress.current_task.as_deref() == Some(task.id.as_str()) && !task.state.is_live() {
        progress.current_task = None;
    }
    save_with_snapshot(
        &snapshot.path,
        &progress,
        &ProgressSnapshot {
            progress: Some(snapshot.progress.clone()),
            raw: snapshot.raw.clone(),
        },
    )
}

fn load_or_create_actor_progress(
    paths: &MaestroPaths,
    project: Option<String>,
    actor: &str,
    now: &str,
) -> Result<(PathBuf, ProgressFile, ProgressSnapshot)> {
    if let Some((card, card_path)) = find_actor_progress(paths, actor, project.as_deref())? {
        let path = card_path
            .parent()
            .context("progress card path is missing parent directory")?
            .join(PROGRESS_FILE);
        let snapshot = load_with_snapshot(&path)?;
        let (agent, session_id) = card
            .claimed_by
            .as_deref()
            .map(actor_parts)
            .unwrap_or((None, None));
        let progress = snapshot
            .progress
            .clone()
            .unwrap_or_else(|| ProgressFile::new(agent, session_id));
        return Ok((path, progress, snapshot));
    }

    let (agent, session_id) = actor_parts(actor);
    let title = progress_title(actor, project.as_deref());
    let id = store::mint_card_id(paths, CardType::Progress, &title);
    let mut card = Card::new(&id, CardType::Progress, &title, "in_progress", now);
    card.claimed_by = Some(actor.to_string());
    card.claimed_at = Some(now.to_string());
    card.project = project;
    let home = store::create_card(paths, &card)?;
    let path = match home {
        CardHome::Dir(path) => path
            .parent()
            .context("progress card path is missing parent directory")?
            .join(PROGRESS_FILE),
        CardHome::Entry(_) => bail!("progress cards must be dir-backed"),
    };
    let snapshot = load_with_snapshot(&path)?;
    let progress = ProgressFile::new(agent, session_id);
    Ok((path, progress, snapshot))
}

fn find_actor_progress(
    paths: &MaestroPaths,
    actor: &str,
    project: Option<&str>,
) -> Result<Option<(Card, PathBuf)>> {
    for (card, card_path) in query::scan_dir_with_paths(&paths.cards_dir())? {
        if card.card_type != CardType::Progress {
            continue;
        }
        if card.claimed_by.as_deref() != Some(actor) {
            continue;
        }
        if card.project.as_deref() != project {
            continue;
        }
        if query::coarse_of(&card.status) == Some(Coarse::Closed) {
            continue;
        }
        return Ok(Some((card, card_path)));
    }
    Ok(None)
}

fn progress_title(actor: &str, project: Option<&str>) -> String {
    match project {
        Some(project) => format!("Progress for {actor} in {project}"),
        None => format!("Progress for {actor}"),
    }
}

fn actor_parts(actor: &str) -> (Option<String>, Option<String>) {
    let actor = actor.trim();
    if actor.is_empty() {
        return (None, None);
    }
    match actor.split_once('#') {
        Some((agent, session)) => (
            nonempty(agent).map(ToOwned::to_owned),
            nonempty(session).map(ToOwned::to_owned),
        ),
        None => (Some(actor.to_string()), None),
    }
}

fn nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
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
