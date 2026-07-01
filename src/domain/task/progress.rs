use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::live_db;
use crate::domain::card::query::{self, Coarse};
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self, CardHome};
use crate::domain::task::lifecycle::{self, TransitionDetails};
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
    db: Option<DbProgressSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressTaskSnapshot {
    pub path: PathBuf,
    progress: ProgressFile,
    raw: Option<String>,
    db: Option<DbProgressSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DbProgressSnapshot {
    paths: MaestroPaths,
    card_id: String,
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
            db: None,
        });
    };
    let progress = parse_progress(&contents, &path.display().to_string())?;
    Ok(ProgressSnapshot {
        progress: Some(progress),
        raw: Some(contents),
        db: None,
    })
}

fn load_db_with_snapshot(paths: &MaestroPaths, card_id: &str) -> Result<ProgressSnapshot> {
    let display = live_db::synthetic_card_path(paths, card_id, PROGRESS_FILE)
        .display()
        .to_string();
    let Some(contents) = live_db::read_text_file(paths, card_id, PROGRESS_FILE)? else {
        return Ok(ProgressSnapshot {
            progress: None,
            raw: None,
            db: Some(DbProgressSnapshot {
                paths: paths.clone(),
                card_id: card_id.to_string(),
            }),
        });
    };
    let progress = parse_progress(&contents, &display)?;
    Ok(ProgressSnapshot {
        progress: Some(progress),
        raw: Some(contents),
        db: Some(DbProgressSnapshot {
            paths: paths.clone(),
            card_id: card_id.to_string(),
        }),
    })
}

fn parse_progress(contents: &str, display: &str) -> Result<ProgressFile> {
    let progress: ProgressFile =
        serde_yaml::from_str(contents).with_context(|| format!("failed to parse {display}"))?;
    if classify(&progress.schema_version, PROGRESS_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "unsupported progress schema {} in {}; expected {}",
            progress.schema_version,
            display,
            PROGRESS_SCHEMA_VERSION
        );
    }
    Ok(progress)
}

pub fn save_with_snapshot(
    path: &Path,
    progress: &ProgressFile,
    snapshot: &ProgressSnapshot,
) -> Result<()> {
    if let Some(db) = &snapshot.db {
        let contents = serde_yaml::to_string(progress).context("failed to serialize progress")?;
        return live_db::write_text_file_if_unchanged(
            &db.paths,
            &db.card_id,
            PROGRESS_FILE,
            snapshot.raw.as_deref(),
            &contents,
        );
    }
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

pub fn setup_simple_tasks(
    paths: &MaestroPaths,
    titles: &[String],
    project: Option<String>,
    start: bool,
    created_at: String,
    actor: &str,
) -> Result<Vec<TaskRecord>> {
    if titles.is_empty() {
        bail!("task setup requires at least one --task");
    }
    if titles.iter().any(|title| title.trim().is_empty()) {
        bail!("task title must not be empty");
    }

    let (path, mut progress, snapshot) =
        load_or_create_actor_progress(paths, project, actor, &created_at)?;
    let mut tasks = Vec::with_capacity(titles.len());
    for title in titles {
        let id = store::mint_card_id(paths, CardType::Task, title);
        let mut task = TaskRecord::draft(&id, title, &created_at);
        task.state = TaskState::Ready;
        task.acceptance_locked = true;
        task.acceptance.locked_by = Some(actor.to_string());
        task.acceptance.locked_at = Some(created_at.clone());
        tasks.push(task);
    }

    if start {
        let first = tasks
            .first_mut()
            .context("task setup requires at least one --task")?;
        lifecycle::transition(
            first,
            TaskState::InProgress,
            actor,
            &created_at,
            TransitionDetails {
                summary: Some("started from task setup".to_string()),
                ..TransitionDetails::default()
            },
        )?;
        progress.current_task = Some(first.id.clone());
    }

    progress.tasks.extend(tasks.iter().cloned());
    save_with_snapshot(&path, &progress, &snapshot)?;
    Ok(tasks)
}

pub fn ensure_started_simple_task(
    paths: &MaestroPaths,
    title: &str,
    project: Option<String>,
    created_at: String,
    actor: &str,
) -> Result<TaskRecord> {
    if title.trim().is_empty() {
        bail!("task title must not be empty");
    }
    let (path, mut progress, snapshot) =
        load_or_create_actor_progress(paths, project, actor, &created_at)?;
    if let Some(task) = progress
        .tasks
        .iter()
        .find(|task| task.state == TaskState::InProgress)
    {
        return Ok(task.clone());
    }

    let id = store::mint_card_id(paths, CardType::Task, title);
    let mut task = TaskRecord::draft(&id, title, &created_at);
    task.state = TaskState::Ready;
    task.acceptance_locked = true;
    task.acceptance.locked_by = Some(actor.to_string());
    task.acceptance.locked_at = Some(created_at.clone());
    lifecycle::transition(
        &mut task,
        TaskState::InProgress,
        actor,
        &created_at,
        TransitionDetails {
            summary: Some("auto-started from write-like hook".to_string()),
            ..TransitionDetails::default()
        },
    )?;
    progress.current_task = Some(task.id.clone());
    progress.tasks.push(task.clone());
    save_with_snapshot(&path, &progress, &snapshot)?;
    Ok(task)
}

pub fn load_task_with_snapshot(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, ProgressTaskSnapshot, PathBuf)>> {
    store::validate_card_id(id)?;
    for (card, card_path) in query::scan_with_paths(paths)? {
        if card.card_type != CardType::Progress {
            continue;
        }
        let (path, snapshot) = load_card_snapshot(paths, &card, &card_path)?;
        let Some(progress) = snapshot.progress.clone() else {
            continue;
        };
        if let Some(task) = progress.tasks.iter().find(|task| task.id == id) {
            let progress_dir = path
                .parent()
                .context("progress sidecar path is missing parent directory")?
                .to_path_buf();
            return Ok(Some((
                task.clone(),
                ProgressTaskSnapshot {
                    path,
                    progress,
                    raw: snapshot.raw,
                    db: snapshot.db,
                },
                progress_dir,
            )));
        }
    }
    Ok(None)
}

pub fn scan(paths: &MaestroPaths) -> Result<Vec<(TaskRecord, PathBuf)>> {
    let mut records = Vec::new();
    for (card, card_path) in query::scan_with_paths(paths)? {
        collect_tasks_from_progress_card(&mut records, paths, &card, &card_path)?;
    }
    Ok(records)
}

pub fn scan_with_cards(paths: &MaestroPaths) -> Result<Vec<(TaskRecord, Card, PathBuf)>> {
    let mut records = Vec::new();
    for (card, card_path) in query::scan_with_paths(paths)? {
        if card.card_type != CardType::Progress {
            continue;
        }
        let (path, snapshot) = load_card_snapshot(paths, &card, &card_path)?;
        if let Some(progress) = snapshot.progress {
            let progress_dir = path
                .parent()
                .context("progress sidecar path is missing parent directory")?
                .to_path_buf();
            records.extend(
                progress
                    .tasks
                    .into_iter()
                    .map(|task| (task, card.clone(), progress_dir.clone())),
            );
        }
    }
    Ok(records)
}

pub(crate) fn scan_in_cards(
    paths: &MaestroPaths,
    cards: &[(Card, PathBuf)],
) -> Result<Vec<(TaskRecord, PathBuf)>> {
    let mut records = Vec::new();
    for (card, card_path) in cards {
        collect_tasks_from_progress_card(&mut records, paths, card, card_path)?;
    }
    Ok(records)
}

fn collect_tasks_from_progress_card(
    records: &mut Vec<(TaskRecord, PathBuf)>,
    paths: &MaestroPaths,
    card: &Card,
    card_path: &Path,
) -> Result<()> {
    if card.card_type != CardType::Progress {
        return Ok(());
    }
    let (path, snapshot) = load_card_snapshot(paths, card, card_path)?;
    if let Some(progress) = snapshot.progress {
        let progress_dir = path
            .parent()
            .context("progress sidecar path is missing parent directory")?
            .to_path_buf();
        records.extend(
            progress
                .tasks
                .into_iter()
                .map(|task| (task, progress_dir.clone())),
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
            db: snapshot.db.clone(),
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
        let (path, snapshot) = load_card_snapshot(paths, &card, &card_path)?;
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
        CardHome::Entry(_) | CardHome::Db(_) => bail!("progress cards must be dir-backed"),
    };
    let snapshot = load_with_snapshot(&path)?;
    let progress = ProgressFile::new(agent, session_id);
    Ok((path, progress, snapshot))
}

fn load_card_snapshot(
    paths: &MaestroPaths,
    card: &Card,
    card_path: &Path,
) -> Result<(PathBuf, ProgressSnapshot)> {
    let path = card_path
        .parent()
        .context("progress card path is missing parent directory")?
        .join(PROGRESS_FILE);
    if live_db::contains_card_id(paths, &card.id)? {
        return Ok((path, load_db_with_snapshot(paths, &card.id)?));
    }
    Ok((path.clone(), load_with_snapshot(&path)?))
}

fn find_actor_progress(
    paths: &MaestroPaths,
    actor: &str,
    project: Option<&str>,
) -> Result<Option<(Card, PathBuf)>> {
    for (card, card_path) in query::scan_with_paths(paths)? {
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

    #[test]
    fn db_backed_progress_sidecar_scans_saves_and_rejects_stale_writers() {
        let paths = temp_paths("task-progress-db");
        let now = "2026-07-01T00:00:00Z";
        let mut card = Card::new(
            "progress-codex-db-019f",
            CardType::Progress,
            "Progress for codex#s1",
            "in_progress",
            now,
        );
        card.claimed_by = Some("codex#s1".to_string());
        store::create_card(&paths, &card).expect("create progress card");
        let card_dir = paths.cards_dir().join(&card.id);
        let path = card_dir.join(PROGRESS_FILE);
        let snapshot = load_with_snapshot(&path).expect("load empty progress sidecar");
        let mut progress = ProgressFile::new(Some("codex".to_string()), Some("s1".to_string()));
        let mut task = TaskRecord::draft("task-db-progress-019f", "db progress task", now);
        task.state = TaskState::Ready;
        progress.tasks.push(task);
        save_with_snapshot(&path, &progress, &snapshot).expect("write progress sidecar");
        live_db::import_card_dir(&paths, &card.id, &card_dir, true)
            .expect("import progress card into DB");

        assert!(
            !card_dir.exists(),
            "DB import removes the progress card dir"
        );
        let scanned = scan(&paths).expect("scan DB-backed progress tasks");
        assert!(
            scanned
                .iter()
                .any(|(task, _)| task.id == "task-db-progress-019f"),
            "DB-backed progress sidecar contributes its tasks"
        );

        let (mut winner, winner_snapshot, _) =
            load_task_with_snapshot(&paths, "task-db-progress-019f")
                .expect("load DB-backed progress task")
                .expect("DB-backed progress task exists");
        let (mut loser, loser_snapshot, _) =
            load_task_with_snapshot(&paths, "task-db-progress-019f")
                .expect("load stale DB-backed progress task snapshot")
                .expect("DB-backed progress task exists");
        winner.state = TaskState::InProgress;
        save_task_with_snapshot(&winner, &winner_snapshot)
            .expect("save DB-backed progress task through sidecar");

        loser.title = "stale writer loses".to_string();
        let error = save_task_with_snapshot(&loser, &loser_snapshot)
            .expect_err("stale DB sidecar writer rejected");
        assert!(format!("{error:#}").contains("changed since it was read"));

        let (reloaded, _, _) = load_task_with_snapshot(&paths, "task-db-progress-019f")
            .expect("reload DB-backed progress task")
            .expect("DB-backed progress task remains");
        assert_eq!(reloaded.state, TaskState::InProgress);

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }
}
