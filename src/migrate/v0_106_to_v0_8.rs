use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::core::diff::unified_diff;
use crate::core::fs::ensure_dir;
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_atomic;
use crate::core::schema::{ACCEPTANCE_SCHEMA_VERSION, TASK_SCHEMA_VERSION};
use crate::core::slug::slugify_ascii;
use crate::task::template::{
    AcceptanceFile, StateHistoryEntry, TaskRecord, TaskState, VerificationBinding,
};

/// One planned migration write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationChange {
    pub path: PathBuf,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
    pub source: Option<PathBuf>,
}

/// Complete v0.106.1 to v0.8 migration plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MigrationPlan {
    pub changes: Vec<MigrationChange>,
}

/// Build a read-only migration plan.
pub fn plan(paths: &MaestroPaths) -> Result<MigrationPlan> {
    reject_symlinked_migration_roots(paths)?;
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
        let before = change
            .before
            .as_deref()
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        let after = change
            .after
            .as_deref()
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        out.push_str(&unified_diff(&path, before.as_ref(), after.as_ref()));
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
    let mut applied = Vec::<AppliedChange>::new();
    for change in &plan.changes {
        if let Some(source) = change.source.as_ref().filter(|source| source.is_file()) {
            if backed_up.insert(source.clone()) {
                backup_file_with_timestamp(paths, source, "migrate", &timestamp)?;
            }
        }
        if change.path.is_file() && backed_up.insert(change.path.clone()) {
            backup_file_with_timestamp(paths, &change.path, "migrate", &timestamp)?;
        }
        let write_result = match change.after.as_deref() {
            Some(after) => write_atomic(&change.path, after)
                .with_context(|| format!("failed to write {}", change.path.display())),
            None => remove_migrated_file(&change.path),
        };
        if let Err(error) = write_result {
            rollback_applied(applied)?;
            return Err(error);
        }
        applied.push(AppliedChange {
            path: change.path.clone(),
            before: change.before.clone(),
        });
    }
    Ok(())
}

fn remove_migrated_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

struct AppliedChange {
    path: PathBuf,
    before: Option<Vec<u8>>,
}

fn rollback_applied(applied: Vec<AppliedChange>) -> Result<()> {
    for change in applied.into_iter().rev() {
        match change.before {
            Some(before) => write_atomic(&change.path, &before)
                .with_context(|| format!("failed to restore {}", change.path.display()))?,
            None => match fs::remove_file(&change.path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to remove migrated file {}", change.path.display())
                    });
                }
            },
        }
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
            let task = old.into_task(paths)?;
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
                paths,
                task_dir.join("task.yaml"),
                serde_yaml::to_string(&task)?.into_bytes(),
                Some(&path),
            )?;
            push_change(
                plan,
                paths,
                task_dir.join("acceptance.yaml"),
                serde_yaml::to_string(&acceptance)?.into_bytes(),
                Some(&path),
            )?;
            push_change(
                plan,
                paths,
                task_dir.join("task.md"),
                migrated_task_markdown(&task).into_bytes(),
                Some(&path),
            )?;
        }
        let archive = paths
            .maestro_dir()
            .join("raw/archived/tasks")
            .join(path.file_name().unwrap_or_default());
        push_change(plan, paths, archive, raw.into_bytes(), Some(&path))?;
    }
    Ok(())
}

fn plan_archives(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    for dir in ["missions", "verdicts", "handoffs", "plans", "intake"] {
        let source_dir = paths.maestro_dir().join(dir);
        for path in files_under(&source_dir)? {
            let raw =
                fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
            let relative = path.strip_prefix(&source_dir).unwrap_or(&path);
            let target = paths
                .maestro_dir()
                .join("raw/archived")
                .join(dir)
                .join(relative);
            push_change(plan, paths, target, raw, Some(&path))?;
        }
    }

    let evidence_dir = paths.maestro_dir().join("evidence");
    for path in files_under(&evidence_dir)? {
        let raw = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let relative = path.strip_prefix(&evidence_dir).unwrap_or(&path);
        let target = paths.runs_dir().join("migrated").join(relative);
        push_change(plan, paths, target, raw, Some(&path))?;
    }
    Ok(())
}

fn plan_features(paths: &MaestroPaths, plan: &mut MigrationPlan) -> Result<()> {
    let legacy_path = paths.maestro_dir().join("features.yaml");
    if legacy_path.is_file() {
        let raw = fs::read_to_string(&legacy_path)
            .with_context(|| format!("failed to read {}", legacy_path.display()))?;
        let after = normalize_features_yaml(&raw)?;
        push_change(
            plan,
            paths,
            paths.features_dir().join("features.yaml"),
            after.into_bytes(),
            Some(&legacy_path),
        )?;
        push_change(
            plan,
            paths,
            paths
                .maestro_dir()
                .join("raw/archived/features/features.yaml"),
            raw.into_bytes(),
            Some(&legacy_path),
        )?;
        push_change(
            plan,
            paths,
            paths.maestro_dir().join("archive/features/features.yaml"),
            fs::read(&legacy_path)
                .with_context(|| format!("failed to read {}", legacy_path.display()))?,
            Some(&legacy_path),
        )?;
        push_delete(plan, paths, legacy_path, None)?;
        return Ok(());
    }

    let path = paths.features_dir().join("features.yaml");
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let after = normalize_features_yaml(&raw)?;
    push_change(plan, paths, path, after.into_bytes(), None)
}

fn normalize_features_yaml(raw: &str) -> Result<String> {
    let Ok(mut value) = serde_yaml::from_str::<serde_yaml::Value>(raw) else {
        return Ok(raw.to_string());
    };
    if let Some(features) = value
        .get_mut("features")
        .and_then(serde_yaml::Value::as_sequence_mut)
    {
        for feature in features {
            if let Some(mapping) = feature.as_mapping_mut() {
                mapping.remove(serde_yaml::Value::String("tasks".to_string()));
                normalize_feature_mapping(mapping);
            }
        }
    } else if let Some(mapping) = value.as_mapping_mut() {
        let mut feature = serde_yaml::Mapping::new();
        for (key, value) in mapping.iter() {
            if key.as_str() != Some("schema_version") {
                feature.insert(key.clone(), value.clone());
            }
        }
        feature.remove(serde_yaml::Value::String("tasks".to_string()));
        normalize_feature_mapping(&mut feature);
        let mut registry = serde_yaml::Mapping::new();
        registry.insert(
            serde_yaml::Value::String("features".to_string()),
            serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(feature)]),
        );
        value = serde_yaml::Value::Mapping(registry);
    }
    if let Some(mapping) = value.as_mapping_mut() {
        mapping.insert(
            serde_yaml::Value::String("schema_version".to_string()),
            serde_yaml::Value::String("maestro.feature.v1".to_string()),
        );
    }
    Ok(serde_yaml::to_string(&value)?)
}

fn normalize_feature_mapping(mapping: &mut serde_yaml::Mapping) {
    let title = mapping
        .get(serde_yaml::Value::String("title".to_string()))
        .cloned()
        .or_else(|| {
            mapping
                .get(serde_yaml::Value::String("id".to_string()))
                .cloned()
        })
        .unwrap_or_else(|| serde_yaml::Value::String("Migrated feature".to_string()));
    mapping
        .entry(serde_yaml::Value::String("title".to_string()))
        .or_insert(title);
    mapping
        .entry(serde_yaml::Value::String("status".to_string()))
        .or_insert_with(|| serde_yaml::Value::String("proposed".to_string()));
    normalize_feature_status(mapping);
    mapping
        .entry(serde_yaml::Value::String("created_at".to_string()))
        .or_insert_with(|| serde_yaml::Value::String("0".to_string()));
    mapping
        .entry(serde_yaml::Value::String("updated_at".to_string()))
        .or_insert_with(|| serde_yaml::Value::String("0".to_string()));
}

fn normalize_feature_status(mapping: &mut serde_yaml::Mapping) {
    let key = serde_yaml::Value::String("status".to_string());
    let Some(status) = mapping.get_mut(&key) else {
        return;
    };
    let Some(status_text) = status.as_str() else {
        *status = serde_yaml::Value::String("proposed".to_string());
        return;
    };
    let normalized = match status_text {
        "proposed" | "in_progress" | "shipped" | "cancelled" => status_text,
        "active" | "started" | "in-progress" | "in progress" => "in_progress",
        "done" | "complete" | "completed" | "merged" => "shipped",
        "canceled" => "cancelled",
        _ => "proposed",
    };
    *status = serde_yaml::Value::String(normalized.to_string());
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
        push_change(plan, paths, target, raw.into_bytes(), Some(&path))?;
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
    let workflows = workflow_defaults(paths)?;
    if verify.is_empty() && workflows.is_none() {
        return Ok(());
    }
    let mut stack = serde_yaml::Mapping::new();
    stack.insert(
        serde_yaml::Value::String("kind".to_string()),
        serde_yaml::Value::String("generic".to_string()),
    );
    stack.insert(
        serde_yaml::Value::String("detected_by".to_string()),
        serde_yaml::Value::Sequence(Vec::new()),
    );
    stack.insert(
        serde_yaml::Value::String("verify".to_string()),
        serde_yaml::to_value(verify)?,
    );
    let mut after = serde_yaml::Mapping::new();
    after.insert(
        serde_yaml::Value::String("schema_version".to_string()),
        serde_yaml::Value::String("maestro.harness.v1".to_string()),
    );
    after.insert(
        serde_yaml::Value::String("stack".to_string()),
        serde_yaml::Value::Mapping(stack),
    );
    if let Some(workflows) = workflows {
        after.insert(serde_yaml::Value::String("workflow".to_string()), workflows);
    }
    let after = serde_yaml::to_string(&serde_yaml::Value::Mapping(after))?;
    push_change(
        plan,
        paths,
        paths.harness_dir().join("harness.yml"),
        after.into_bytes(),
        None,
    )
}

fn workflow_defaults(paths: &MaestroPaths) -> Result<Option<serde_yaml::Value>> {
    let workflows_dir = paths.maestro_dir().join("workflows");
    let workflow_files = files_with_extension(&workflows_dir, "yaml")?;
    if workflow_files.is_empty() {
        return Ok(None);
    }

    let mut defaults = serde_yaml::Mapping::new();
    for path in workflow_files {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let value: serde_yaml::Value = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        defaults.insert(serde_yaml::Value::String(stem.to_string()), value);
    }

    let mut workflow = serde_yaml::Mapping::new();
    workflow.insert(
        serde_yaml::Value::String("default".to_string()),
        serde_yaml::Value::Mapping(defaults),
    );
    Ok(Some(serde_yaml::Value::Mapping(workflow)))
}

fn reject_concurrent_writer(paths: &MaestroPaths) -> Result<()> {
    for path in [
        paths.maestro_dir().join("writer.lock"),
        paths.maestro_dir().join("migrate.lock"),
        paths.maestro_dir().join("v0.106.lock"),
    ] {
        if path.exists() {
            bail!("v0.106.1 writer evidence found: {}", path.display());
        }
    }
    Ok(())
}

fn push_change(
    plan: &mut MigrationPlan,
    paths: &MaestroPaths,
    path: PathBuf,
    after: Vec<u8>,
    source: Option<&Path>,
) -> Result<()> {
    ensure_managed_target(paths, &path)?;
    reject_symlinked_target(&path)?;
    let before = match fs::read(&path) {
        Ok(before) => Some(before),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    if before.as_deref() != Some(after.as_slice()) {
        plan.changes.push(MigrationChange {
            path,
            before,
            after: Some(after),
            source: source.map(Path::to_path_buf),
        });
    }
    Ok(())
}

fn push_delete(
    plan: &mut MigrationPlan,
    paths: &MaestroPaths,
    path: PathBuf,
    source: Option<&Path>,
) -> Result<()> {
    ensure_managed_target(paths, &path)?;
    reject_symlinked_target(&path)?;
    let before = match fs::read(&path) {
        Ok(before) => Some(before),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    if before.is_some() {
        plan.changes.push(MigrationChange {
            path,
            before,
            after: None,
            source: source.map(Path::to_path_buf),
        });
    }
    Ok(())
}

fn reject_symlinked_migration_roots(paths: &MaestroPaths) -> Result<()> {
    for path in [
        paths.maestro_dir(),
        paths.tasks_dir(),
        paths.features_dir(),
        paths.decisions_dir(),
        paths.harness_dir(),
        paths.runs_dir(),
        paths.maestro_dir().join("raw"),
        paths.maestro_dir().join("missions"),
        paths.maestro_dir().join("verdicts"),
        paths.maestro_dir().join("handoffs"),
        paths.maestro_dir().join("plans"),
        paths.maestro_dir().join("intake"),
        paths.maestro_dir().join("evidence"),
        paths.maestro_dir().join("policies"),
        paths.maestro_dir().join("workflows"),
    ] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("migration path contains symlink: {}", path.display());
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
            }
        }
    }
    Ok(())
}

fn ensure_managed_target(paths: &MaestroPaths, path: &Path) -> Result<()> {
    if !path.starts_with(paths.maestro_dir()) {
        bail!("migration target escapes .maestro: {}", path.display());
    }
    for component in path
        .strip_prefix(paths.maestro_dir())
        .unwrap_or(path)
        .components()
    {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                bail!("migration target escapes .maestro: {}", path.display());
            }
        }
    }
    Ok(())
}

fn reject_symlinked_target(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("migration target contains symlink: {}", current.display());
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", current.display()));
            }
        }
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
    fn into_task(self, paths: &MaestroPaths) -> Result<TaskRecord> {
        validate_old_task_id(&self.id)?;
        let created_at = self.created_at.unwrap_or_else(|| "0".to_string());
        let updated_at = self.updated_at.unwrap_or_else(|| created_at.clone());
        let state = parse_state(self.state.as_deref());
        let mut task = TaskRecord {
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
        };
        apply_intake(paths, &mut task)?;
        apply_handoff(paths, &mut task)?;
        apply_verdict(paths, &mut task)?;
        Ok(task)
    }
}

fn validate_old_task_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    let Some(Component::Normal(component)) = components.next() else {
        bail!("invalid migrated task id: {id}");
    };
    if components.next().is_some() || component.is_empty() || component.to_str() != Some(id) {
        bail!("invalid migrated task id: {id}");
    }
    Ok(())
}

#[derive(Deserialize)]
struct OldIntake {
    #[serde(default)]
    raw_request: Option<String>,
    #[serde(default)]
    request: Option<String>,
    #[serde(default)]
    input_type: Option<String>,
    #[serde(default)]
    affected_areas: Vec<String>,
    #[serde(default)]
    open_questions: Vec<String>,
}

fn apply_intake(paths: &MaestroPaths, task: &mut TaskRecord) -> Result<()> {
    let path = paths
        .maestro_dir()
        .join("intake")
        .join(format!("{}.yaml", task.id));
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let intake: OldIntake = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    task.raw_request = intake.raw_request.or(intake.request);
    task.input_type = intake.input_type;
    task.affected_areas = intake.affected_areas;
    task.open_questions = intake.open_questions;
    Ok(())
}

fn apply_handoff(paths: &MaestroPaths, task: &mut TaskRecord) -> Result<()> {
    let path = paths
        .maestro_dir()
        .join("handoffs")
        .join(format!("{}.md", task.id));
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let title = raw
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or("handoff");
    task.state_history.push(StateHistoryEntry {
        state: task.state.clone(),
        at: task.updated_at.clone(),
        by: "migrate".to_string(),
        to: None,
        summary: Some(format!("migrated handoff: {title}")),
        claims: Vec::new(),
        open_items: Vec::new(),
    });
    Ok(())
}

#[derive(Default, Deserialize)]
struct OldVerdict {
    #[serde(default)]
    verdict: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    verified_at: Option<String>,
    #[serde(default)]
    verified_commit: Option<String>,
    #[serde(default)]
    commit: Option<String>,
    #[serde(default)]
    verified_by_run: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    task_contract_hash: Option<String>,
    #[serde(default)]
    acceptance_hash: Option<String>,
    #[serde(default)]
    checks_hash: Option<String>,
}

fn apply_verdict(paths: &MaestroPaths, task: &mut TaskRecord) -> Result<()> {
    let path = paths
        .maestro_dir()
        .join("verdicts")
        .join(format!("{}.json", task.id));
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let verdict: OldVerdict = serde_json::from_str(&raw).unwrap_or_default();
    let status = verdict
        .verdict
        .as_deref()
        .or(verdict.status.as_deref())
        .unwrap_or("");
    if !matches!(status, "pass" | "passed" | "verified" | "ok") {
        return Ok(());
    }
    task.verification = VerificationBinding {
        verified_at: verdict.verified_at,
        verified_commit: verdict.verified_commit.or(verdict.commit),
        verified_by_run: verdict.verified_by_run.or(verdict.run_id),
        task_contract_hash: verdict.task_contract_hash,
        acceptance_hash: verdict.acceptance_hash,
        checks_hash: verdict.checks_hash,
    };
    Ok(())
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
