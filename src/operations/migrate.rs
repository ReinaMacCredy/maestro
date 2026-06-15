//! One-way v1 artifact tree migration to the reduced v2 surface.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::json;
use serde_yaml::{Mapping, Value};

use crate::domain::{decisions, task};
use crate::foundation::core::fs::{ensure_dir, read_yaml_mapping, sorted_child_dirs as child_dirs};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::retention::prune_child_dirs;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{FEATURE_SCHEMA_VERSION, TASK_SCHEMA_VERSION};
use crate::foundation::core::time::render_timestamp;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MigrateReport {
    pub tasks: usize,
    pub features: usize,
    pub removed: usize,
    pub pruned_backups: usize,
    pub pruned_runs: usize,
}

pub fn run(paths: &MaestroPaths) -> Result<MigrateReport> {
    let mut report = MigrateReport::default();
    migrate_features(paths, &mut report)?;
    migrate_tasks(paths, &mut report)?;
    remove_if_exists(paths.features_dir().join("features.yaml"), &mut report)?;
    ensure_decision_store(paths)?;
    report.pruned_backups = prune_child_dirs(&paths.backups_dir(), 3)?;
    report.pruned_runs = prune_child_dirs(&paths.runs_dir(), 20)?;
    Ok(report)
}

fn ensure_decision_store(paths: &MaestroPaths) -> Result<()> {
    ensure_dir(paths.decisions_dir())?;
    let path = paths.decisions_file();
    if !path.exists() {
        write_string_atomic(path.as_path(), &decisions::empty_store_yaml()?)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn migrate_features(paths: &MaestroPaths, report: &mut MigrateReport) -> Result<()> {
    for feature_dir in child_dirs(&paths.features_dir())? {
        let feature_yaml = feature_dir.join("feature.yaml");
        if !feature_yaml.is_file() {
            continue;
        }
        let mut record = read_yaml_mapping(&feature_yaml)?;
        insert_string(&mut record, "schema_version", FEATURE_SCHEMA_VERSION);
        convert_timestamps(&mut record);
        let amend_log = feature_dir.join("amend-log.yaml");
        if amend_log.is_file() {
            let log = read_yaml_mapping(&amend_log)?;
            if let Some(entries) = log.get(Value::String("entries".to_string())).cloned() {
                record.insert(Value::String("amends".to_string()), entries);
            }
            remove_if_exists(&amend_log, report)?;
        }
        write_yaml_mapping(&feature_yaml, &record)?;
        merge_qa_files(&feature_dir, report)?;
        remove_scaffold_notes(&feature_dir, report)?;
        let draft = feature_dir.join("prepare-draft.md");
        if feature_dir.join("tasks").is_dir() {
            remove_if_exists(draft, report)?;
        }
        report.features += 1;
    }
    Ok(())
}

fn migrate_tasks(paths: &MaestroPaths, report: &mut MigrateReport) -> Result<()> {
    for task_dir in child_dirs(&paths.tasks_dir())? {
        let task_yaml = task_dir.join("task.yaml");
        if !task_yaml.is_file() {
            continue;
        }
        let mut record = read_yaml_mapping(&task_yaml)?;
        insert_string(&mut record, "schema_version", TASK_SCHEMA_VERSION);
        convert_timestamps(&mut record);
        let feature_id = take_string(&mut record, "feature_id");
        remove_key(&mut record, "slug");
        remove_key(&mut record, "task_type");
        remove_key(&mut record, "input_type");
        let acceptance = read_acceptance(&task_dir)?;
        record.insert(
            Value::String("acceptance".to_string()),
            Value::Mapping(acceptance.clone()),
        );
        promote_claims(&mut record);
        cap_history(&mut record);
        if let Some(verification) = read_verification(&task_dir, &record, &acceptance)? {
            record.insert(Value::String("verification".to_string()), verification);
        }
        write_yaml_mapping(&task_yaml, &record)?;
        write_task_markdown(&task_dir, &record)?;
        remove_if_exists(task_dir.join("acceptance.yaml"), report)?;
        remove_if_exists(task_dir.join("verification.json"), report)?;
        remove_dir_if_exists(task_dir.join("verification.attempts"), report)?;
        if let Some(feature_id) = feature_id {
            let target_root = paths.features_dir().join(feature_id).join("tasks");
            ensure_dir(&target_root)?;
            let name = task_dir
                .file_name()
                .with_context(|| format!("task dir missing name: {}", task_dir.display()))?;
            let target = target_root.join(name);
            if target != task_dir {
                if target.exists() {
                    bail!(
                        "cannot migrate task {}; target already exists",
                        target.display()
                    );
                }
                fs::rename(&task_dir, &target).with_context(|| {
                    format!(
                        "failed to move {} to {}",
                        task_dir.display(),
                        target.display()
                    )
                })?;
            }
        }
        report.tasks += 1;
    }
    Ok(())
}

fn read_acceptance(task_dir: &Path) -> Result<Mapping> {
    let path = task_dir.join("acceptance.yaml");
    if !path.is_file() {
        return Ok(Mapping::new());
    }
    let mut acceptance = read_yaml_mapping(&path)?;
    remove_key(&mut acceptance, "schema_version");
    remove_key(&mut acceptance, "task");
    convert_timestamps(&mut acceptance);
    Ok(acceptance)
}

fn read_verification(
    task_dir: &Path,
    task: &Mapping,
    acceptance: &Mapping,
) -> Result<Option<Value>> {
    let path = task_dir.join("verification.json");
    if !path.is_file() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let mut out = Mapping::new();
    copy_json_string(&mut out, &json, "status");
    copy_json_string_converted_timestamp(&mut out, &json, "verified_at");
    if let Some(commit) = json
        .get("verified_commit")
        .and_then(serde_json::Value::as_str)
    {
        insert_string(&mut out, "verified_commit", commit);
    }
    insert_string(&mut out, "contract_hash", &contract_hash(task, acceptance)?);
    copy_json_value(&mut out, &json, "claims", "claim_checks")?;
    copy_json_value(&mut out, &json, "commands", "commands")?;
    copy_json_value(&mut out, &json, "proof_sources", "proof_sources")?;
    copy_json_value(&mut out, &json, "failures", "failures")?;
    Ok(Some(Value::Mapping(out)))
}

fn contract_hash(task: &Mapping, acceptance: &Mapping) -> Result<String> {
    let contract = json!({
        "id": string_field(task, "id").unwrap_or_default(),
        "title": string_field(task, "title").unwrap_or_default(),
        "acceptance": serde_json::to_value(acceptance)?,
    });
    Ok(sha256_hex(contract.to_string().as_bytes()))
}

fn merge_qa_files(feature_dir: &Path, report: &mut MigrateReport) -> Result<()> {
    let baseline = feature_dir.join("baseline.md");
    let slices = feature_dir.join("qa-slices.yaml");
    if !baseline.is_file() && !slices.is_file() {
        return Ok(());
    }
    let mut qa = if baseline.is_file() {
        fs::read_to_string(&baseline)
            .with_context(|| format!("failed to read {}", baseline.display()))?
    } else {
        "# QA\n".to_string()
    };
    if slices.is_file() {
        let slice_yaml = fs::read_to_string(&slices)
            .with_context(|| format!("failed to read {}", slices.display()))?;
        qa.push_str("\n\n```yaml\n");
        qa.push_str(slice_yaml.trim_end());
        qa.push_str("\n```\n");
    }
    let qa_path = feature_dir.join("qa.md");
    write_string_atomic(&qa_path, &qa)
        .with_context(|| format!("failed to write {}", qa_path.display()))?;
    remove_if_exists(baseline, report)?;
    remove_if_exists(slices, report)?;
    Ok(())
}

fn remove_scaffold_notes(feature_dir: &Path, report: &mut MigrateReport) -> Result<()> {
    let path = feature_dir.join("notes.md");
    let Ok(contents) = fs::read_to_string(&path) else {
        return Ok(());
    };
    if contents.contains("Design notes: the running reasoning behind this feature")
        && contents.contains("Free-form prose, read by no gate.")
    {
        remove_if_exists(path, report)?;
    }
    Ok(())
}

fn write_task_markdown(task_dir: &Path, record: &Mapping) -> Result<()> {
    let title = string_field(record, "title").unwrap_or_else(|| "Task".to_string());
    let checks = record
        .get(Value::String("acceptance".to_string()))
        .and_then(Value::as_mapping)
        .and_then(|acceptance| acceptance.get(Value::String("checks".to_string())))
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let mut task = task::TaskRecord::draft(
        &string_field(record, "id").unwrap_or_else(|| "task".to_string()),
        &title,
        &string_field(record, "created_at").unwrap_or_default(),
    );
    task.acceptance.checks = checks
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let task_md = task_dir.join("task.md");
    write_string_atomic(&task_md, &task::task_markdown(&task))
        .with_context(|| format!("failed to write {}", task_md.display()))
}

fn promote_claims(record: &mut Mapping) {
    let mut claims = Vec::<Value>::new();
    if let Some(history) = record
        .get(Value::String("state_history".to_string()))
        .and_then(Value::as_sequence)
    {
        for entry in history {
            let Some(entry_claims) = entry
                .as_mapping()
                .and_then(|entry| entry.get(Value::String("claims".to_string())))
                .and_then(Value::as_sequence)
            else {
                continue;
            };
            for claim in entry_claims {
                if !claims.contains(claim) {
                    claims.push(claim.clone());
                }
            }
        }
    }
    if !claims.is_empty() {
        record.insert(Value::String("claims".to_string()), Value::Sequence(claims));
    }
}

fn cap_history(record: &mut Mapping) {
    let Some(history) = record
        .get_mut(Value::String("state_history".to_string()))
        .and_then(Value::as_sequence_mut)
    else {
        return;
    };
    let keep_from = history.len().saturating_sub(10);
    if keep_from > 0 {
        history.drain(0..keep_from);
    }
}

fn convert_timestamps(map: &mut Mapping) {
    for key in [
        "created_at",
        "updated_at",
        "claimed_at",
        "locked_at",
        "verified_at",
        "at",
        "resolved_at",
    ] {
        if let Some(value) = map.get_mut(Value::String(key.to_string()))
            && let Some(raw) = value.as_str()
        {
            *value = Value::String(render_timestamp(raw));
        }
    }
    for value in map.values_mut() {
        match value {
            Value::Mapping(child) => convert_timestamps(child),
            Value::Sequence(items) => {
                for item in items {
                    if let Value::Mapping(child) = item {
                        convert_timestamps(child);
                    }
                }
            }
            _ => {}
        }
    }
}

fn write_yaml_mapping(path: &Path, mapping: &Mapping) -> Result<()> {
    let contents = serde_yaml::to_string(mapping)?;
    write_string_atomic(path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn insert_string(map: &mut Mapping, key: &str, value: &str) {
    map.insert(
        Value::String(key.to_string()),
        Value::String(value.to_string()),
    );
}

fn remove_key(map: &mut Mapping, key: &str) {
    map.remove(Value::String(key.to_string()));
}

fn take_string(map: &mut Mapping, key: &str) -> Option<String> {
    map.remove(Value::String(key.to_string()))
        .and_then(|value| value.as_str().map(str::to_string))
}

fn string_field(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn copy_json_string(out: &mut Mapping, json: &serde_json::Value, key: &str) {
    if let Some(value) = json.get(key).and_then(serde_json::Value::as_str) {
        insert_string(out, key, value);
    }
}

fn copy_json_string_converted_timestamp(out: &mut Mapping, json: &serde_json::Value, key: &str) {
    if let Some(value) = json.get(key).and_then(serde_json::Value::as_str) {
        insert_string(out, key, &render_timestamp(value));
    }
}

fn copy_json_value(
    out: &mut Mapping,
    json: &serde_json::Value,
    source_key: &str,
    target_key: &str,
) -> Result<()> {
    if let Some(value) = json.get(source_key) {
        out.insert(
            Value::String(target_key.to_string()),
            serde_yaml::to_value(value)?,
        );
    }
    Ok(())
}

fn remove_if_exists(path: impl AsRef<Path>, report: &mut MigrateReport) -> Result<()> {
    match fs::remove_file(path.as_ref()) {
        Ok(()) => {
            report.removed += 1;
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove {}", path.as_ref().display()))
        }
    }
}

fn remove_dir_if_exists(path: impl AsRef<Path>, report: &mut MigrateReport) -> Result<()> {
    match fs::remove_dir_all(path.as_ref()) {
        Ok(()) => {
            report.removed += 1;
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove {}", path.as_ref().display()))
        }
    }
}
