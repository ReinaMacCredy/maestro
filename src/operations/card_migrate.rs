//! Fold the four legacy artifact trees (features, tasks, decisions, harness
//! backlog) into the unified flat card store at `.maestro/cards/` (SPEC
//! beads-model, P1 slice 2).
//!
//! Additive and idempotent. It reads the four trees and mints one
//! `cards/<id>/card.yaml` per artifact, KEEPING every existing id (feature slug
//! / task-NNN / decision-NNN / harness-backlog id) so current verbs keep
//! resolving -- this slice is a zero-behavior reorg, the hash remint + ref
//! rewrite are deferred to P3. The source trees are left untouched: nothing
//! reads `cards/` until cutover, so leaving features/tasks/decisions/harness in
//! place keeps the old verbs reading exactly what they read before.
//!
//! Each source record's whole YAML mapping is copied verbatim into the card's
//! `extra` carrier while the card's identity fields are derived copies. Cutover
//! then reconstructs the original typed record with one
//! `serde_yaml::from_value(card.extra)`, so the migration never has to be kept
//! byte-synced with a separate reconstruction step.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{card_path, load_with_snapshot, save_with_snapshot};
use crate::domain::decisions::normalize_decision_id;
use crate::foundation::core::fs::{child_dirs as fs_child_dirs, ensure_dir};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::CARD_SCHEMA_VERSION;

/// Per-type tally of a card-fold run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CardMigrateReport {
    pub features: usize,
    pub tasks: usize,
    pub decisions: usize,
    pub ideas: usize,
    /// Artifacts already present in `cards/` from a prior run (idempotent skip).
    pub skipped: usize,
    /// Pre-fold snapshot directory under `.maestro/backups/`, when one was taken.
    pub backup: Option<PathBuf>,
}

/// One outgoing cross-artifact reference, captured for the post-fold
/// dangling-ref check. `from`/`field` are carried only to name the offender.
struct CardRef {
    from: String,
    field: String,
    target: String,
}

/// Fold the four legacy trees into `.maestro/cards/`. `now` stamps the backup
/// directory and supplies the fallback timestamp for harness items that predate
/// `first_seen`/`last_seen`.
pub fn run(paths: &MaestroPaths, now: &str) -> Result<CardMigrateReport> {
    // O1: snapshot .maestro/ (minus backups/) before touching anything, so a
    // rollback is a wholesale restore with no reverse-transform code.
    let mut report = CardMigrateReport {
        backup: backup_maestro(paths, now)?,
        ..Default::default()
    };

    let mut minted: HashSet<String> = HashSet::new();
    let mut refs: Vec<CardRef> = Vec::new();

    fold_features(paths, now, &mut minted, &mut report)?;
    fold_tasks(paths, now, &mut minted, &mut refs, &mut report)?;
    fold_decisions(paths, now, &mut minted, &mut refs, &mut report)?;
    fold_ideas(paths, now, &mut minted, &mut refs, &mut report)?;

    validate_refs(&minted, &refs)?;

    Ok(report)
}

fn fold_features(
    paths: &MaestroPaths,
    now: &str,
    minted: &mut HashSet<String>,
    report: &mut CardMigrateReport,
) -> Result<()> {
    for feature_dir in sorted_child_dirs(&paths.features_dir())? {
        let yaml = feature_dir.join("feature.yaml");
        if !yaml.is_file() {
            continue;
        }
        let source = read_yaml_mapping(&yaml)?;
        let id = string_field(&source, "id")
            .or_else(|| dir_name(&feature_dir))
            .with_context(|| format!("feature missing id: {}", yaml.display()))?;
        let card = Card {
            schema_version: CARD_SCHEMA_VERSION.to_string(),
            card_type: CardType::Feature,
            title: title_or_id(&source, &id),
            id,
            status: string_field(&source, "status").unwrap_or_default(),
            parent: None,
            deps: Vec::new(),
            lane: None,
            claimed_by: None,
            claimed_at: None,
            created_at: created_at_or(&source, "created_at", now),
            updated_at: updated_at_or(&source, "updated_at", "created_at", now),
            description: None,
            extra: source,
        };
        if mint(paths, minted, report, card, Some(&feature_dir))? {
            report.features += 1;
        }
    }
    Ok(())
}

fn fold_tasks(
    paths: &MaestroPaths,
    now: &str,
    minted: &mut HashSet<String>,
    refs: &mut Vec<CardRef>,
    report: &mut CardMigrateReport,
) -> Result<()> {
    // Flat tasks (`tasks/<task>/`): no feature parent.
    for task_dir in sorted_child_dirs(&paths.tasks_dir())? {
        fold_one_task(paths, &task_dir, None, now, minted, refs, report)?;
    }
    // Nested tasks (`features/<feat>/tasks/<task>/`): the dir IS the only carrier
    // of the feature link, because TaskRecord.feature_id is never serialized.
    for feature_dir in sorted_child_dirs(&paths.features_dir())? {
        let parent = dir_name(&feature_dir);
        for task_dir in sorted_child_dirs(&feature_dir.join("tasks"))? {
            fold_one_task(paths, &task_dir, parent.clone(), now, minted, refs, report)?;
        }
    }
    Ok(())
}

fn fold_one_task(
    paths: &MaestroPaths,
    task_dir: &Path,
    parent_from_dir: Option<String>,
    now: &str,
    minted: &mut HashSet<String>,
    refs: &mut Vec<CardRef>,
    report: &mut CardMigrateReport,
) -> Result<()> {
    let yaml = task_dir.join("task.yaml");
    if !yaml.is_file() {
        return Ok(());
    }
    let source = read_yaml_mapping(&yaml)?;
    let id = string_field(&source, "id")
        .with_context(|| format!("task missing id: {}", yaml.display()))?;
    collect_task_refs(&id, &source, refs);
    let card = Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        id: id.clone(),
        card_type: CardType::Task,
        title: title_or_id(&source, &id),
        // Task lifecycle lives under `state`, not `status`; keep the word verbatim.
        status: string_field(&source, "state").unwrap_or_default(),
        parent: parent_from_dir.or_else(|| string_field(&source, "feature_id")),
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        created_at: created_at_or(&source, "created_at", now),
        updated_at: updated_at_or(&source, "updated_at", "created_at", now),
        description: None,
        extra: source,
    };
    if mint(paths, minted, report, card, None)? {
        report.tasks += 1;
    }
    Ok(())
}

fn fold_decisions(
    paths: &MaestroPaths,
    now: &str,
    minted: &mut HashSet<String>,
    refs: &mut Vec<CardRef>,
    report: &mut CardMigrateReport,
) -> Result<()> {
    fold_decision_store(
        paths,
        &paths.decisions_file(),
        None,
        now,
        minted,
        refs,
        report,
    )?;
    for feature_dir in sorted_child_dirs(&paths.features_dir())? {
        let store = feature_dir.join("decisions.yaml");
        fold_decision_store(
            paths,
            &store,
            dir_name(&feature_dir),
            now,
            minted,
            refs,
            report,
        )?;
    }
    Ok(())
}

fn fold_decision_store(
    paths: &MaestroPaths,
    store_path: &Path,
    feature_parent: Option<String>,
    now: &str,
    minted: &mut HashSet<String>,
    refs: &mut Vec<CardRef>,
    report: &mut CardMigrateReport,
) -> Result<()> {
    if !store_path.is_file() {
        return Ok(());
    }
    let store = read_yaml_mapping(store_path)?;
    let Some(items) = sequence_field(&store, "decisions") else {
        return Ok(());
    };
    for item in items {
        let Some(record) = item.as_mapping() else {
            continue;
        };
        let id = string_field(record, "id")
            .with_context(|| format!("decision missing id in {}", store_path.display()))?;
        collect_decision_refs(&id, record, refs);
        let card = Card {
            schema_version: CARD_SCHEMA_VERSION.to_string(),
            id: id.clone(),
            card_type: CardType::Decision,
            title: title_or_id(record, &id),
            status: string_field(record, "status").unwrap_or_default(),
            // DecisionRecord carries no updated_at; lean on locked_at, else created_at.
            parent: string_field(record, "feature").or_else(|| feature_parent.clone()),
            deps: Vec::new(),
            lane: None,
            claimed_by: None,
            claimed_at: None,
            created_at: created_at_or(record, "created_at", now),
            updated_at: updated_at_or(record, "locked_at", "created_at", now),
            description: None,
            extra: record.clone(),
        };
        if mint(paths, minted, report, card, None)? {
            report.decisions += 1;
        }
    }
    Ok(())
}

fn fold_ideas(
    paths: &MaestroPaths,
    now: &str,
    minted: &mut HashSet<String>,
    refs: &mut Vec<CardRef>,
    report: &mut CardMigrateReport,
) -> Result<()> {
    let backlog = paths.harness_dir().join("backlog.yaml");
    if !backlog.is_file() {
        return Ok(());
    }
    let store = read_yaml_mapping(&backlog)?;
    let Some(items) = sequence_field(&store, "items") else {
        return Ok(());
    };
    for item in items {
        let Some(record) = item.as_mapping() else {
            continue;
        };
        let id = string_field(record, "id")
            .with_context(|| format!("harness backlog item missing id in {}", backlog.display()))?;
        collect_idea_refs(&id, record, refs);
        // first_seen/last_seen are skip-if-empty, so fall back rather than store "".
        let created_at = nonempty_field(record, "first_seen")
            .or_else(|| nonempty_field(record, "last_seen"))
            .unwrap_or_else(|| now.to_string());
        let updated_at = nonempty_field(record, "last_seen").unwrap_or_else(|| created_at.clone());
        let card = Card {
            schema_version: CARD_SCHEMA_VERSION.to_string(),
            id: id.clone(),
            // Every harness item maps to `idea`; its detector category stays in
            // `extra.type`, it is not the card type (SPEC keep-ids reconciliation).
            card_type: CardType::Idea,
            title: title_or_id(record, &id),
            status: string_field(record, "status").unwrap_or_default(),
            parent: None,
            deps: Vec::new(),
            lane: None,
            claimed_by: None,
            claimed_at: None,
            created_at,
            updated_at,
            description: None,
            extra: record.clone(),
        };
        if mint(paths, minted, report, card, None)? {
            report.ideas += 1;
        }
    }
    Ok(())
}

/// Write one card, guarding id collisions and skipping cards a prior run already
/// minted. Returns whether a new card was written.
fn mint(
    paths: &MaestroPaths,
    minted: &mut HashSet<String>,
    report: &mut CardMigrateReport,
    card: Card,
    prose_src: Option<&Path>,
) -> Result<bool> {
    if !minted.insert(card.id.clone()) {
        bail!(
            "card id collision: two source artifacts both map to id '{}'; resolve the duplicate before migrating",
            card.id
        );
    }
    let path = card_path(paths, &card.id);
    let snapshot = load_with_snapshot(&path)
        .with_context(|| format!("failed to read card snapshot {}", path.display()))?;
    if snapshot.card.is_some() {
        report.skipped += 1;
        return Ok(false);
    }
    save_with_snapshot(&path, &card, &snapshot)
        .with_context(|| format!("failed to write card {}", card.id))?;
    if let Some(src) = prose_src {
        let card_dir = path
            .parent()
            .with_context(|| format!("card path missing parent: {}", path.display()))?;
        copy_feature_prose(src, card_dir)?;
    }
    Ok(true)
}

/// Fail loud when any captured reference does not resolve to a minted card
/// (SPEC E5, P1 form). Decision ids carry legacy aliasing (`decision-7` ==
/// `decision-007` == `7`), so both the minted-id set and every ref pass through
/// `decisions::query::normalize_decision_id` before comparison -- mirroring the
/// existing dangling-ref checker so the migration never aborts on a ref maestro
/// itself resolves. Normalization is a no-op for task/feature/idea ids, so only
/// the decision alias collapses. The hash remint + canonical rewrite are P3's job.
fn validate_refs(minted: &HashSet<String>, refs: &[CardRef]) -> Result<()> {
    let resolvable: HashSet<String> = minted.iter().map(|id| normalize_ref(id)).collect();
    for reference in refs {
        if !resolvable.contains(&normalize_ref(&reference.target)) {
            bail!(
                "dangling reference: card '{}' {} points at '{}', which no migrated card provides",
                reference.from,
                reference.field,
                reference.target
            );
        }
    }
    Ok(())
}

/// Normalize an id the way the decision system resolves references, falling back
/// to the raw id for a form it cannot normalize (mirrors `decisions::query`,
/// which uses the same `unwrap_or_else` fallback on both sides of its check).
fn normalize_ref(id: &str) -> String {
    normalize_decision_id(id).unwrap_or_else(|_| id.to_string())
}

fn collect_task_refs(from: &str, record: &Mapping, refs: &mut Vec<CardRef>) {
    let Some(blockers) = sequence_field(record, "blockers") else {
        return;
    };
    for blocker in blockers {
        let target = blocker
            .as_mapping()
            .and_then(|blocker| blocker.get(Value::String("blocked_ref".to_string())))
            .and_then(Value::as_mapping)
            .and_then(|blocked_ref| string_field(blocked_ref, "id"));
        push_ref(refs, from, "blocker", target);
    }
}

fn collect_decision_refs(from: &str, record: &Mapping, refs: &mut Vec<CardRef>) {
    if let Some(targets) = sequence_field(record, "supersedes") {
        for target in targets {
            push_ref(
                refs,
                from,
                "supersedes",
                target.as_str().map(str::to_string),
            );
        }
    }
    push_ref(
        refs,
        from,
        "superseded_by",
        string_field(record, "superseded_by"),
    );
}

fn collect_idea_refs(from: &str, record: &Mapping, refs: &mut Vec<CardRef>) {
    push_ref(
        refs,
        from,
        "spawned_task",
        string_field(record, "spawned_task"),
    );
    let Some(history) = sequence_field(record, "history") else {
        return;
    };
    for entry in history {
        let target = entry
            .as_mapping()
            .and_then(|entry| string_field(entry, "task"));
        push_ref(refs, from, "history.task", target);
    }
}

fn push_ref(refs: &mut Vec<CardRef>, from: &str, field: &str, target: Option<String>) {
    if let Some(target) = target.filter(|target| !target.is_empty()) {
        refs.push(CardRef {
            from: from.to_string(),
            field: field.to_string(),
            target,
        });
    }
}

/// Snapshot `.maestro/` (minus its own `backups/`) into a timestamped backup
/// directory. Symlinks are skipped rather than followed. Returns the snapshot
/// path, or `None` when there is no `.maestro/` to snapshot. A snapshot that
/// already exists for this instant is reused, keeping re-runs idempotent.
fn backup_maestro(paths: &MaestroPaths, now: &str) -> Result<Option<PathBuf>> {
    let maestro_dir = paths.maestro_dir();
    if !maestro_dir.is_dir() {
        return Ok(None);
    }
    let backups_dir = paths.backups_dir();
    let target = backups_dir.join(format!("{}-card-migrate", sanitize_label(now)));
    if target.exists() {
        return Ok(Some(target));
    }
    ensure_dir(&target)?;
    for entry in fs::read_dir(&maestro_dir)
        .with_context(|| format!("failed to read {}", maestro_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", maestro_dir.display()))?;
        let source = entry.path();
        if source == backups_dir {
            continue;
        }
        copy_recursive(&source, &target.join(entry.file_name()))?;
    }
    Ok(Some(target))
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(src)
        .with_context(|| format!("failed to inspect {}", src.display()))?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_dir() {
        ensure_dir(dst)?;
        for entry in
            fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))?
        {
            let entry = entry.with_context(|| format!("failed to list {}", src.display()))?;
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            ensure_dir(parent)?;
        }
        fs::copy(src, dst)
            .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))?;
    }
    Ok(())
}

/// Copy a feature's prose sidecars next to its card, when present.
fn copy_feature_prose(feature_dir: &Path, card_dir: &Path) -> Result<()> {
    for name in ["spec.md", "notes.md", "qa.md"] {
        let source = feature_dir.join(name);
        if source.is_file() {
            ensure_dir(card_dir)?;
            fs::copy(&source, card_dir.join(name)).with_context(|| {
                format!(
                    "failed to copy {} into {}",
                    source.display(),
                    card_dir.display()
                )
            })?;
        }
    }
    Ok(())
}

fn sanitize_label(now: &str) -> String {
    now.chars()
        .map(|c| {
            if matches!(c, ':' | '/' | '\\') {
                '-'
            } else {
                c
            }
        })
        .collect()
}

fn sorted_child_dirs(parent: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs: Vec<PathBuf> = fs_child_dirs(parent)?
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    dirs.sort();
    Ok(dirs)
}

fn dir_name(dir: &Path) -> Option<String> {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn title_or_id(record: &Mapping, id: &str) -> String {
    string_field(record, "title").unwrap_or_else(|| id.to_string())
}

fn created_at_or(record: &Mapping, key: &str, now: &str) -> String {
    string_field(record, key).unwrap_or_else(|| now.to_string())
}

fn updated_at_or(record: &Mapping, key: &str, fallback_key: &str, now: &str) -> String {
    string_field(record, key).unwrap_or_else(|| created_at_or(record, fallback_key, now))
}

fn read_yaml_mapping(path: &Path) -> Result<Mapping> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    value
        .as_mapping()
        .cloned()
        .with_context(|| format!("expected mapping in {}", path.display()))
}

fn string_field(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn nonempty_field(map: &Mapping, key: &str) -> Option<String> {
    string_field(map, key).filter(|value| !value.is_empty())
}

fn sequence_field<'a>(map: &'a Mapping, key: &str) -> Option<&'a Vec<Value>> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_sequence)
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::card::schema::Card;
    use crate::domain::card::store::load as load_card;
    use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus, DecisionStore};
    use crate::domain::feature::FeatureStatus;
    use crate::domain::feature::schema::FeatureRecord;
    use crate::domain::harness::{BacklogConfig, BacklogItem, HistoryEntry};
    use crate::domain::task::{
        Blocker, BlockerKind, BlockerRef, BlockerSource, TaskRecord, TaskState,
    };

    const NOW: &str = "2026-06-08T12:00:00Z";

    fn temp_repo(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("maestro-cardmig-{name}-{}-{nanos}", process::id()))
    }

    fn write_record<T: serde::Serialize>(path: &Path, value: &T) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent dir");
        }
        fs::write(
            path,
            serde_yaml::to_string(value).expect("serialize fixture"),
        )
        .expect("write fixture");
    }

    fn read_value(path: &Path) -> Value {
        serde_yaml::from_str(&fs::read_to_string(path).expect("read fixture"))
            .expect("parse fixture")
    }

    fn store_element(store_path: &Path, key: &str, id: &str) -> Value {
        let store = read_value(store_path);
        store
            .as_mapping()
            .and_then(|store| store.get(Value::String(key.to_string())))
            .and_then(Value::as_sequence)
            .and_then(|items| {
                items.iter().find(|item| {
                    item.as_mapping()
                        .and_then(|item| item.get(Value::String("id".to_string())))
                        .and_then(Value::as_str)
                        == Some(id)
                })
            })
            .cloned()
            .unwrap_or_else(|| panic!("element {id} not found in {}", store_path.display()))
    }

    fn load(paths: &MaestroPaths, id: &str) -> Card {
        load_card(&card_path(paths, id))
            .expect("load card")
            .unwrap_or_else(|| panic!("card {id} missing"))
    }

    fn decision(id: &str, status: DecisionStatus, feature: Option<&str>) -> DecisionRecord {
        DecisionRecord {
            id: id.to_string(),
            title: format!("Decision {id}"),
            status,
            feature: feature.map(str::to_string),
            context: None,
            decision: None,
            rejected: Vec::new(),
            preview: None,
            supersedes: Vec::new(),
            superseded_by: None,
            created_at: "2026-06-01T04:00:00Z".to_string(),
            locked_at: None,
        }
    }

    /// Build a brownfield `.maestro/` with all four trees and every cross-ref
    /// resolving: task-001 -> decision-001, decision-002 -> decision-001,
    /// hb-1.spawned_task -> task-002, hb-1.history -> task-001.
    fn brownfield(root: &Path) -> MaestroPaths {
        let paths = MaestroPaths::new(root);
        let feat_dir = paths.features_dir().join("csv-export");

        let mut feature =
            FeatureRecord::proposed("csv-export", "Add CSV export", "2026-06-01T00:00:00Z");
        feature.status = FeatureStatus::InProgress;
        feature.description = Some("Stream rows to stdout".to_string());
        feature.acceptance = vec!["exports a header row".to_string()];
        feature.updated_at = "2026-06-02T00:00:00Z".to_string();
        write_record(&feat_dir.join("feature.yaml"), &feature);
        fs::write(feat_dir.join("spec.md"), "# CSV export\n").expect("spec");
        fs::write(feat_dir.join("notes.md"), "design notes\n").expect("notes");
        fs::write(feat_dir.join("qa.md"), "# QA\nbaseline\n").expect("qa");

        let mut task1 = TaskRecord::draft("task-001", "Implement writer", "2026-06-01T01:00:00Z");
        task1.state = TaskState::InProgress;
        task1.updated_at = "2026-06-02T01:00:00Z".to_string();
        task1.blockers = vec![Blocker {
            id: "b1".to_string(),
            kind: BlockerKind::Decision,
            blocked_ref: Some(BlockerRef {
                kind: BlockerKind::Decision,
                id: "decision-001".to_string(),
            }),
            title: "awaits writer choice".to_string(),
            reason: "need the decision".to_string(),
            source: BlockerSource::Command,
            created_at: "2026-06-01T02:00:00Z".to_string(),
            resolved_at: None,
        }];
        write_record(
            &feat_dir
                .join("tasks")
                .join(task1.directory_name())
                .join("task.yaml"),
            &task1,
        );

        let mut task2 = TaskRecord::draft("task-002", "Add tests", "2026-06-01T03:00:00Z");
        task2.state = TaskState::Verified;
        task2.updated_at = "2026-06-02T03:00:00Z".to_string();
        write_record(
            &paths
                .tasks_dir()
                .join(task2.directory_name())
                .join("task.yaml"),
            &task2,
        );

        let mut global = DecisionStore::empty();
        let mut d1 = decision("decision-001", DecisionStatus::Superseded, None);
        d1.superseded_by = Some("decision-002".to_string());
        d1.locked_at = Some("2026-06-01T05:00:00Z".to_string());
        let mut d2 = decision("decision-002", DecisionStatus::Locked, Some("csv-export"));
        d2.supersedes = vec!["decision-001".to_string()];
        global.decisions = vec![d1, d2];
        write_record(&paths.decisions_file(), &global);

        let mut feature_store = DecisionStore::empty();
        feature_store.decisions = vec![decision("decision-003", DecisionStatus::Open, None)];
        write_record(&feat_dir.join("decisions.yaml"), &feature_store);

        let mut backlog = BacklogConfig::empty();
        backlog.items = vec![BacklogItem {
            id: "hb-1".to_string(),
            fingerprint: "missing_skill:csv".to_string(),
            source: "session-1".to_string(),
            provenance: "detector".to_string(),
            topic: "csv".to_string(),
            item_type: "missing_skill".to_string(),
            title: "Document CSV export".to_string(),
            priority: "medium".to_string(),
            occurrences: 2,
            sessions_hit: vec!["session-1".to_string()],
            first_seen: "2026-06-01T09:00:00Z".to_string(),
            last_seen: "2026-06-02T09:00:00Z".to_string(),
            status: "proposed".to_string(),
            evidence: vec!["seen twice".to_string()],
            spawned_task: Some("task-002".to_string()),
            dismissal_reason: None,
            history: vec![HistoryEntry {
                result: "accepted".to_string(),
                task: Some("task-001".to_string()),
                note: None,
                at: "2026-06-02T09:30:00Z".to_string(),
            }],
        }];
        write_record(&paths.harness_dir().join("backlog.yaml"), &backlog);

        paths
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migrates_brownfield_repo_into_cards() {
        let root = temp_repo("brownfield");
        let paths = brownfield(&root);

        let report = run(&paths, NOW).expect("clean migrate succeeds");
        assert_eq!(report.features, 1, "one feature card");
        assert_eq!(report.tasks, 2, "two task cards");
        assert_eq!(report.decisions, 3, "three decision cards");
        assert_eq!(report.ideas, 1, "one idea card");
        assert_eq!(report.skipped, 0, "nothing skipped on a fresh run");
        let backup = report.backup.clone().expect("backup taken");

        for id in [
            "csv-export",
            "task-001",
            "task-002",
            "decision-001",
            "decision-002",
            "decision-003",
            "hb-1",
        ] {
            assert!(
                card_path(&paths, id).is_file(),
                "card {id} written under its kept id"
            );
        }

        // Feature card: derived identity + prose sidecars copied beside it.
        let feature_card = load(&paths, "csv-export");
        assert_eq!(feature_card.card_type, CardType::Feature);
        assert_eq!(
            feature_card.status, "in_progress",
            "status kept verbatim, not coarsened"
        );
        assert_eq!(feature_card.parent, None);
        let feature_card_dir = card_path(&paths, "csv-export")
            .parent()
            .expect("card dir")
            .to_path_buf();
        for prose in ["spec.md", "notes.md", "qa.md"] {
            assert!(
                feature_card_dir.join(prose).is_file(),
                "{prose} preserved as a sidecar"
            );
        }

        // Task parent: nested task captures its feature; flat task has none.
        let task1_card = load(&paths, "task-001");
        assert_eq!(task1_card.parent.as_deref(), Some("csv-export"));
        assert_eq!(
            task1_card.status, "in_progress",
            "task status comes from `state`"
        );
        let task2_card = load(&paths, "task-002");
        assert_eq!(task2_card.parent, None);
        assert_eq!(task2_card.card_type, CardType::Task);

        // Decision parent: explicit `feature` field, then per-feature store dir.
        assert_eq!(
            load(&paths, "decision-002").parent.as_deref(),
            Some("csv-export")
        );
        assert_eq!(
            load(&paths, "decision-003").parent.as_deref(),
            Some("csv-export")
        );
        assert_eq!(load(&paths, "decision-001").card_type, CardType::Decision);

        // Idea: harness item maps to idea; first_seen/last_seen drive timestamps.
        let idea = load(&paths, "hb-1");
        assert_eq!(idea.card_type, CardType::Idea);
        assert_eq!(idea.created_at, "2026-06-01T09:00:00Z");
        assert_eq!(idea.updated_at, "2026-06-02T09:00:00Z");

        // Losslessness: extra is the verbatim source mapping after the card disk
        // round-trip, and still deserializes to the original typed record.
        let feature_src = read_value(&paths.features_dir().join("csv-export").join("feature.yaml"));
        assert_eq!(
            Value::Mapping(feature_card.extra.clone()),
            feature_src,
            "feature extra lossless"
        );
        let _: FeatureRecord = serde_yaml::from_value(Value::Mapping(feature_card.extra.clone()))
            .expect("feature extra reconstructs a FeatureRecord");

        // Task typed round-trip is the cutover guarantee: old code reads `extra`.
        let task_src_path = paths
            .features_dir()
            .join("csv-export")
            .join("tasks")
            .join("task-001-implement-writer")
            .join("task.yaml");
        let task_original: TaskRecord =
            serde_yaml::from_str(&fs::read_to_string(&task_src_path).expect("read task"))
                .expect("parse task");
        let task_reconstructed: TaskRecord =
            serde_yaml::from_value(Value::Mapping(task1_card.extra.clone()))
                .expect("reconstruct task");
        assert_eq!(
            task_reconstructed, task_original,
            "task extra reconstructs the exact record"
        );

        // Decision and idea elements also survive verbatim.
        let decision_src = store_element(&paths.decisions_file(), "decisions", "decision-001");
        assert_eq!(
            Value::Mapping(load(&paths, "decision-001").extra.clone()),
            decision_src
        );
        let idea_src = store_element(&paths.harness_dir().join("backlog.yaml"), "items", "hb-1");
        assert_eq!(Value::Mapping(idea.extra.clone()), idea_src);

        // Backup is a restorable snapshot of the pre-fold trees, minus backups/.
        assert!(
            backup
                .join("features")
                .join("csv-export")
                .join("feature.yaml")
                .is_file(),
            "backup snapshots the feature tree"
        );
        assert!(
            !backup.join("backups").exists(),
            "backup excludes the backups dir itself"
        );

        cleanup(&root);
    }

    #[test]
    fn rerun_is_idempotent() {
        let root = temp_repo("idempotent");
        let paths = brownfield(&root);

        let first = run(&paths, NOW).expect("first migrate");
        assert_eq!(
            first.features + first.tasks + first.decisions + first.ideas,
            7
        );

        let second = run(&paths, "2026-06-08T13:00:00Z").expect("second migrate is a no-op");
        assert_eq!(second.features, 0);
        assert_eq!(second.tasks, 0);
        assert_eq!(second.decisions, 0);
        assert_eq!(second.ideas, 0);
        assert_eq!(second.skipped, 7, "every artifact already had a card");

        assert_eq!(
            sorted_child_dirs(&paths.cards_dir()).expect("cards").len(),
            7,
            "no duplicate cards"
        );

        cleanup(&root);
    }

    #[test]
    fn dangling_reference_fails_loud() {
        let root = temp_repo("dangling");
        let paths = brownfield(&root);

        let mut bad = TaskRecord::draft("task-003", "Bad ref", "2026-06-01T10:00:00Z");
        bad.blockers = vec![Blocker {
            id: "b9".to_string(),
            kind: BlockerKind::Decision,
            blocked_ref: Some(BlockerRef {
                kind: BlockerKind::Decision,
                id: "decision-999".to_string(),
            }),
            title: "awaits a ghost".to_string(),
            reason: "no such decision".to_string(),
            source: BlockerSource::Command,
            created_at: "2026-06-01T10:00:00Z".to_string(),
            resolved_at: None,
        }];
        write_record(
            &paths
                .tasks_dir()
                .join(bad.directory_name())
                .join("task.yaml"),
            &bad,
        );

        let error = run(&paths, NOW).expect_err("dangling reference must fail loud");
        assert!(
            error.to_string().contains("decision-999"),
            "error names the dangling target: {error}"
        );

        cleanup(&root);
    }

    #[test]
    fn colliding_ids_fail_loud() {
        let root = temp_repo("collision");
        let paths = brownfield(&root);

        // A decision whose id collides with the feature slug minted earlier.
        let mut global = DecisionStore::empty();
        global.decisions = vec![
            decision("decision-001", DecisionStatus::Locked, None),
            decision("csv-export", DecisionStatus::Open, None),
        ];
        write_record(&paths.decisions_file(), &global);

        let error = run(&paths, NOW).expect_err("id collision must fail loud");
        let message = error.to_string();
        assert!(
            message.contains("collision") && message.contains("csv-export"),
            "error names the collision: {error}"
        );

        cleanup(&root);
    }

    #[test]
    fn short_form_decision_ref_resolves() {
        // A `supersedes` in the legacy short form (`decision-1`) must resolve
        // against the canonical minted id `decision-001` -- migration mirrors the
        // decision system's id normalization rather than aborting on exact-match.
        let root = temp_repo("short-ref");
        let paths = MaestroPaths::new(&root);

        let mut global = DecisionStore::empty();
        let d1 = decision("decision-001", DecisionStatus::Superseded, None);
        let mut d2 = decision("decision-002", DecisionStatus::Locked, None);
        d2.supersedes = vec!["decision-1".to_string()];
        global.decisions = vec![d1, d2];
        write_record(&paths.decisions_file(), &global);

        let report =
            run(&paths, NOW).expect("short-form decision ref resolves, no false dangling abort");
        assert_eq!(report.decisions, 2, "both decisions migrated");

        cleanup(&root);
    }
}
