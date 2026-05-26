use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::core::diff::unified_diff;
use crate::core::fs::ensure_dir;
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_string_atomic;
use crate::core::schema::{ACCEPTANCE_SCHEMA_VERSION, TASK_SCHEMA_VERSION};
use crate::core::slug::slugify_ascii;
use crate::task::template::{
    AcceptanceFile, StateHistoryEntry, TaskRecord, TaskState, VerificationBinding,
};

/// One planned migration write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationChange {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: String,
    pub source: Option<PathBuf>,
}

/// Complete v0.106.1 to v0.8 migration plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MigrationPlan {
    pub changes: Vec<MigrationChange>,
}

/// Build a read-only migration plan.
pub fn plan(paths: &MaestroPaths) -> Result<MigrationPlan> {
    let mut plan = MigrationPlan::default();
    plan_tasks(paths, &mut plan)?;
    plan_archives(paths, &mut plan)?;
    plan_features(paths, &mut plan)?;
    plan_decisions(paths, &mut plan)?;
    plan_harness_verify(paths, &mut plan)?;
    Ok(plan)
}

/// Render a unified diff preview for `maestro migrate --check`.
pub fn render_check(plan: &MigrationPlan) -> String {
    if plan.changes.is_empty() {
        return "migration check: no v0.106.1 artifacts found\n".to_string();
    }
    let mut out = format!("migration check: {} change(s)\n", plan.changes.len());
    for change in &plan.changes {
        let path = change.path.display().to_string();
        out.push_str(&unified_diff(
            &path,
            change.before.as_deref().unwrap_or_default(),
            &change.after,
        ));
    }
    out
}

/// Apply a migration plan with source and destination backups.
pub fn apply(paths: &MaestroPaths, plan: &MigrationPlan, force: bool) -> Result<()> {
    if !force {
        reject_concurrent_writer(paths)?;
    }
    let timestamp = backup_operation_timestamp()?;
    let mut backed_up = BTreeSet::<PathBuf>::new();
    for change in &plan.changes {
        if let Some(source) = change.source.as_ref().filter(|source| source.is_file()) {
            if backed_up.insert(source.clone()) {
                backup_file_with_timestamp(paths, source, "migrate", &timestamp)?;
            }
        }
        if change.path.is_file() && backed_up.insert(change.path.clone()) {
            backup_file_with_timestamp(paths, &change.path, "migrate", &timestamp)?;
        }
        write_string_atomic(&change.path, &change.after)
            .with_context(|| format!("failed to write {}", change.path.display()))?;
    }
    Ok(())
}

fn plan_tasks(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    let tasks_dir = paths.tasks_dir();
    for path in files_with_extension(&tasks_dir, "jsonl")? {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for line in raw.lines().filter(|line| !line.trim().is_empty()) {
            let old: OldTask = serde_json::from_str(line)
                .with_context(|| format!("failed to parse task JSONL in {}", path.display()))?;
            let task = old.into_task();
            let task_dir = tasks_dir.join(task.directory_name());
            let acceptance = AcceptanceFile {
                schema_version: ACCEPTANCE_SCHEMA_VERSION.to_string(),
                task: task.id.clone(),
                checks: Vec::new(),
                locked_by: None,
                locked_at: None,
            };
            push_change(
                plan,
                task_dir.join("task.yaml"),
                serde_yaml::to_string(&task)?,
                Some(&path),
            )?;
            push_change(
                plan,
                task_dir.join("acceptance.yaml"),
                serde_yaml::to_string(&acceptance)?,
                Some(&path),
            )?;
            push_change(
                plan,
                task_dir.join("task.md"),
                migrated_task_markdown(&task),
                Some(&path),
            )?;
        }
        let archive = paths
            .maestro_dir()
            .join("raw/archived/tasks")
            .join(path.file_name().unwrap_or_default());
        push_change(plan, archive, raw, Some(&path))?;
    }
    Ok(())
}

fn plan_archives(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    for dir in ["missions", "verdicts", "handoffs", "plans", "intake"] {
        let source_dir = paths.maestro_dir().join(dir);
        for path in files_under(&source_dir)? {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let relative = path.strip_prefix(&source_dir).unwrap_or(&path);
            let target = paths
                .maestro_dir()
                .join("raw/archived")
                .join(dir)
                .join(relative);
            push_change(plan, target, raw, Some(&path))?;
        }
    }

    let evidence_dir = paths.maestro_dir().join("evidence");
    for path in files_under(&evidence_dir)? {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let relative = path.strip_prefix(&evidence_dir).unwrap_or(&path);
        let target = paths.runs_dir().join("migrated").join(relative);
        push_change(plan, target, raw, Some(&path))?;
    }
    Ok(())
}

fn plan_features(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    let path = paths.features_dir().join("features.yaml");
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let Ok(mut value) = serde_yaml::from_str::<serde_yaml::Value>(&raw) else {
        return Ok(());
    };
    if let Some(features) = value
        .get_mut("features")
        .and_then(serde_yaml::Value::as_sequence_mut)
    {
        for feature in features {
            if let Some(mapping) = feature.as_mapping_mut() {
                mapping.remove(serde_yaml::Value::String("tasks".to_string()));
            }
        }
    }
    let after = serde_yaml::to_string(&value)?;
    push_change(plan, path, after, None)
}

fn plan_decisions(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    let dir = paths.decisions_dir();
    for path in files_with_prefix(&dir, "ADR-")? {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let number = file_name
            .strip_prefix("ADR-")
            .and_then(|rest| rest.split('-').next())
            .and_then(|number| number.parse::<u32>().ok())
            .unwrap_or(1);
        let title = raw
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .unwrap_or(file_name.trim_end_matches(".md"));
        let target = dir.join(format!("decision-{number:03}-{}.md", slugify_ascii(title)));
        push_change(plan, target, raw, Some(&path))?;
    }
    Ok(())
}

fn plan_harness_verify(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    let policies = paths.maestro_dir().join("policies");
    let mut verify = Vec::<String>::new();
    for name in ["verify.yaml", "verify.yml"] {
        let path = policies.join(name);
        if !path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let value: serde_yaml::Value = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if let Some(commands) = value
            .get("verify")
            .or_else(|| value.get("commands"))
            .and_then(serde_yaml::Value::as_sequence)
        {
            verify.extend(
                commands
                    .iter()
                    .filter_map(serde_yaml::Value::as_str)
                    .map(str::to_string),
            );
        }
    }
    if verify.is_empty() {
        return Ok(());
    }
    let after = serde_yaml::to_string(&json!({
        "schema_version": "maestro.harness.v1",
        "stack": {
            "kind": "generic",
            "detected_by": [],
            "verify": verify,
        }
    }))?;
    push_change(plan, paths.harness_dir().join("harness.yml"), after, None)
}

fn reject_concurrent_writer(paths: &MaestroPaths) -> Result<()> {
    for path in files_under(&paths.maestro_dir())? {
        if path.extension().and_then(|extension| extension.to_str()) == Some("lock") {
            bail!("v0.106.1 writer evidence found: {}", path.display());
        }
    }
    Ok(())
}

fn push_change(
    plan: &mut MigrationPlan,
    path: PathBuf,
    after: String,
    source: Option<&Path>,
) -> Result<()> {
    let before = match fs::read_to_string(&path) {
        Ok(before) => Some(before),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    if before.as_deref() != Some(after.as_str()) {
        plan.changes.push(MigrationChange {
            path,
            before,
            after,
            source: source.map(Path::to_path_buf),
        });
    }
    Ok(())
}

fn files_with_extension(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    Ok(files_under(dir)?
        .into_iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(extension))
        .collect())
}

fn files_with_prefix(dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
    Ok(files_under(dir)?
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(prefix))
                .unwrap_or(false)
        })
        .collect())
}

fn files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    ensure_dir(dir)?;
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn migrated_task_markdown(task: &TaskRecord) -> String {
    format!("# {}\n\nState: {}\n", task.title, state_label(&task.state))
}

#[derive(Deserialize)]
struct OldTask {
    id: String,
    title: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    feature_id: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

impl OldTask {
    fn into_task(self) -> TaskRecord {
        let created_at = self.created_at.unwrap_or_else(|| "0".to_string());
        let updated_at = self.updated_at.unwrap_or_else(|| created_at.clone());
        let state = parse_state(self.state.as_deref());
        TaskRecord {
            schema_version: TASK_SCHEMA_VERSION.to_string(),
            slug: slugify_ascii(&self.title),
            id: self.id,
            feature_id: self.feature_id,
            title: self.title,
            task_type: None,
            lane: Some("normal".to_string()),
            risk: Some("medium".to_string()),
            raw_request: None,
            input_type: None,
            affected_areas: Vec::new(),
            open_questions: Vec::new(),
            state: state.clone(),
            acceptance_locked: matches!(
                state,
                TaskState::Ready
                    | TaskState::InProgress
                    | TaskState::NeedsVerification
                    | TaskState::Verified
            ),
            claimed_by: None,
            claimed_at: None,
            blockers: Vec::new(),
            state_history: vec![StateHistoryEntry {
                state,
                at: created_at.clone(),
                by: "migrate".to_string(),
                to: None,
                summary: Some("migrated from v0.106.1".to_string()),
                claims: Vec::new(),
                open_items: Vec::new(),
            }],
            verification: VerificationBinding::default(),
            created_at,
            updated_at,
        }
    }
}

fn parse_state(state: Option<&str>) -> TaskState {
    match state.unwrap_or("draft") {
        "exploring" => TaskState::Exploring,
        "ready" => TaskState::Ready,
        "in_progress" => TaskState::InProgress,
        "needs_verification" => TaskState::NeedsVerification,
        "verified" => TaskState::Verified,
        "rejected" => TaskState::Rejected,
        "abandoned" => TaskState::Abandoned,
        "superseded" => TaskState::Superseded,
        _ => TaskState::Draft,
    }
}

fn state_label(state: &TaskState) -> &'static str {
    match state {
        TaskState::Draft => "draft",
        TaskState::Exploring => "exploring",
        TaskState::Ready => "ready",
        TaskState::InProgress => "in_progress",
        TaskState::NeedsVerification => "needs_verification",
        TaskState::Verified => "verified",
        TaskState::Rejected => "rejected",
        TaskState::Abandoned => "abandoned",
        TaskState::Superseded => "superseded",
    }
}
