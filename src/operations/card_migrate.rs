//! Fold the four legacy artifact trees (features, tasks, decisions, harness
//! backlog) into the unified flat card store at `.maestro/cards/` (SPEC
//! beads-model, P1 slice 2 + P3 remint).
//!
//! Additive and idempotent. It reads the four trees and mints one
//! `cards/<id>/card.yaml` per artifact. Feature cards keep their immutable
//! creation slug; every other card is reminted to a stable opaque `card-<hash>`
//! id (SPEC E2/O3). The source trees are left untouched: nothing reads `cards/`
//! until cutover, so leaving features/tasks/decisions/harness in place keeps the
//! old verbs reading exactly what they read before.
//!
//! The run is two-pass so the remint can rewrite references before any card is
//! written (SPEC E5): first collect every card and its outgoing refs, then
//! assign the `card-<hash>` ids and rewrite the five structured ref classes (the
//! dir name, each record's own `id`, the typed cross-refs in `extra`, the
//! `cards/<feature>/notes.md` decision pointers, and the `runs/**/events.jsonl`
//! `task_id`s), and only then write. The authoritative dangling-ref gate runs
//! over the rewritten ref set; human prose is never touched.
//!
//! Each source record's whole YAML mapping is copied verbatim into the card's
//! `extra` carrier while the card's identity fields are derived copies; the
//! remint then rewrites only the structured id fields inside `extra`. Cutover
//! reconstructs the original typed record with one `serde_yaml::from_value`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::fold::{self, string_field};
use crate::domain::card::query;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{
    RESERVED_CONTAINER_NAMES, card_path, hash_id, load_with_snapshot, locate, mint_hash_id,
    save_with_snapshot,
};
use crate::domain::decisions::normalize_decision_id;
use crate::domain::run::managed_event_logs;
use crate::foundation::core::fs::{ensure_dir, read_yaml_mapping, sorted_child_dirs};
use crate::foundation::core::paths::MaestroPaths;

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

/// One card collected in the first pass, before ids are assigned and writes
/// happen. `kept_id` is the legacy id the card was folded from (used to key the
/// remint and to scope the run-event rewrite); `card.id` starts equal to it and
/// is rewritten to `card-<hash>` for non-feature cards in `assign_ids`.
struct PendingCard {
    kept_id: String,
    card: Card,
    /// Feature dir whose prose sidecars (`spec.md`/`notes.md`/`qa.md`) travel
    /// with the card, for feature cards only.
    prose_src: Option<PathBuf>,
}

/// One outgoing cross-artifact reference, captured for the post-rewrite
/// dangling-ref check. `from`/`field` are carried only to name the offender;
/// `target` is the legacy id, mapped through the remint at validation time.
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

    // Pass 1: collect every card and its outgoing refs, writing nothing yet.
    let mut pending: Vec<PendingCard> = Vec::new();
    let mut refs: Vec<CardRef> = Vec::new();
    collect_features(paths, now, &mut pending)?;
    collect_tasks(paths, now, &mut pending, &mut refs)?;
    collect_decisions(paths, now, &mut pending, &mut refs)?;
    collect_ideas(paths, now, &mut pending, &mut refs)?;

    // A re-run over a partially cleaned legacy store: an artifact whose
    // reminted card already lives in the store (flat or folded) drops out
    // here, refs and all -- its card is the store's truth now and its refs
    // were validated when it migrated. The reminted id is reproducible
    // because the mint is deterministic (O6). One store scan answers every
    // membership probe (here and the run-event rewrite) instead of a per-card
    // layout walk; the scan is strict, which is the migrator's posture --
    // it refuses a corrupt store rather than migrating around it.
    let existing_ids: HashSet<String> = query::scan(paths)?
        .into_iter()
        .map(|card| card.id)
        .collect();
    let mut already_migrated: HashSet<String> = HashSet::new();
    let mut fresh: Vec<PendingCard> = Vec::new();
    for entry in pending {
        let minted = match entry.card.card_type {
            CardType::Feature => entry.kept_id.clone(),
            _ => hash_id(&entry.kept_id),
        };
        if existing_ids.contains(&minted) {
            report.skipped += 1;
            already_migrated.insert(normalize_ref(&entry.kept_id));
        } else {
            fresh.push(entry);
        }
    }
    let mut pending = fresh;
    refs.retain(|reference| !already_migrated.contains(&normalize_ref(&reference.from)));

    // E2/O3: assign stable opaque ids -- non-feature cards become `card-<hash>`,
    // feature cards keep their slug -- and build the legacy-id -> new-id remap.
    let remap = assign_ids(&mut pending)?;

    // E5: rewrite the structured refs (own id + typed cross-refs) to the new
    // ids, then run the authoritative dangling-ref gate over the rewritten set.
    rewrite_refs(&mut pending, &remap);
    let final_ids: HashSet<String> = pending.iter().map(|entry| entry.card.id.clone()).collect();
    validate_refs(
        &final_ids,
        &frozen_legacy_ids(paths)?,
        &already_migrated,
        &remap,
        &refs,
    )?;

    // Pass 2: writes happen only after the rewrite + gate pass.
    let mut minted: HashSet<String> = HashSet::new();
    for entry in &pending {
        if mint(
            paths,
            &mut minted,
            &mut report,
            &entry.card,
            entry.prose_src.as_deref(),
            &remap,
        )? {
            tally(&mut report, entry.card.card_type);
        }
    }

    // E5: rewrite run-event `task_id`s for migrated tasks. Scoped per task to
    // "no live card at the old id" so a post-migration recreate of the same id
    // keeps its own events (notes 53).
    let task_remap: Vec<(String, String)> = pending
        .iter()
        .filter(|entry| entry.card.card_type == CardType::Task)
        .map(|entry| (entry.kept_id.clone(), entry.card.id.clone()))
        .collect();
    // Pass 2 only writes reminted (`card-<hash>`) ids, so the pre-write scan
    // still answers "does a live card occupy the OLD id" exactly.
    rewrite_run_events(paths, &task_remap, &existing_ids)?;

    Ok(report)
}

fn tally(report: &mut CardMigrateReport, card_type: CardType) {
    match card_type {
        CardType::Feature => report.features += 1,
        CardType::Decision => report.decisions += 1,
        CardType::Idea => report.ideas += 1,
        CardType::Task | CardType::Bug | CardType::Chore => report.tasks += 1,
    }
}

fn collect_features(paths: &MaestroPaths, now: &str, pending: &mut Vec<PendingCard>) -> Result<()> {
    for feature_dir in sorted_child_dirs(&paths.features_dir())? {
        let yaml = feature_dir.join("feature.yaml");
        if !yaml.is_file() {
            continue;
        }
        let source = read_yaml_mapping(&yaml)?;
        let id = string_field(&source, "id")
            .or_else(|| dir_name(&feature_dir))
            .with_context(|| format!("feature missing id: {}", yaml.display()))?;
        if RESERVED_CONTAINER_NAMES.contains(&id.as_str()) {
            bail!(
                "feature id {id} is reserved by the card store layout; rename the legacy \
                 feature dir {} (and its `id:` field) before migrating",
                feature_dir.display()
            );
        }
        let card = fold::feature_card(id.clone(), source, now);
        pending.push(PendingCard {
            kept_id: id,
            card,
            prose_src: Some(feature_dir),
        });
    }
    Ok(())
}

fn collect_tasks(
    paths: &MaestroPaths,
    now: &str,
    pending: &mut Vec<PendingCard>,
    refs: &mut Vec<CardRef>,
) -> Result<()> {
    // Flat tasks (`tasks/<task>/`): no feature parent.
    for task_dir in sorted_child_dirs(&paths.tasks_dir())? {
        collect_one_task(&task_dir, None, now, pending, refs)?;
    }
    // Nested tasks (`features/<feat>/tasks/<task>/`): the dir IS the only carrier
    // of the feature link, because TaskRecord.feature_id is never serialized.
    for feature_dir in sorted_child_dirs(&paths.features_dir())? {
        let parent = dir_name(&feature_dir);
        for task_dir in sorted_child_dirs(&feature_dir.join("tasks"))? {
            collect_one_task(&task_dir, parent.clone(), now, pending, refs)?;
        }
    }
    Ok(())
}

fn collect_one_task(
    task_dir: &Path,
    parent_from_dir: Option<String>,
    now: &str,
    pending: &mut Vec<PendingCard>,
    refs: &mut Vec<CardRef>,
) -> Result<()> {
    let yaml = task_dir.join("task.yaml");
    if !yaml.is_file() {
        return Ok(());
    }
    let source = read_yaml_mapping(&yaml)?;
    let id = string_field(&source, "id")
        .with_context(|| format!("task missing id: {}", yaml.display()))?;
    collect_task_refs(&id, &source, refs);
    let card = fold::task_card(id.clone(), source, parent_from_dir, now);
    pending.push(PendingCard {
        kept_id: id,
        card,
        prose_src: None,
    });
    Ok(())
}

fn collect_decisions(
    paths: &MaestroPaths,
    now: &str,
    pending: &mut Vec<PendingCard>,
    refs: &mut Vec<CardRef>,
) -> Result<()> {
    collect_decision_store(&paths.decisions_file(), None, now, pending, refs)?;
    for feature_dir in sorted_child_dirs(&paths.features_dir())? {
        let store = feature_dir.join("decisions.yaml");
        collect_decision_store(&store, dir_name(&feature_dir), now, pending, refs)?;
    }
    Ok(())
}

fn collect_decision_store(
    store_path: &Path,
    feature_parent: Option<String>,
    now: &str,
    pending: &mut Vec<PendingCard>,
    refs: &mut Vec<CardRef>,
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
        let card = fold::decision_card(id.clone(), record.clone(), feature_parent.clone(), now);
        pending.push(PendingCard {
            kept_id: id,
            card,
            prose_src: None,
        });
    }
    Ok(())
}

fn collect_ideas(
    paths: &MaestroPaths,
    now: &str,
    pending: &mut Vec<PendingCard>,
    refs: &mut Vec<CardRef>,
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
        let card = fold::idea_card(id.clone(), record.clone(), now);
        pending.push(PendingCard {
            kept_id: id,
            card,
            prose_src: None,
        });
    }
    Ok(())
}

/// E2/O3: assign the stable opaque id of every card and return the legacy-id ->
/// new-id remap that the ref rewrite uses. Feature cards keep their slug (and so
/// seed `taken`, guaranteeing a hash can never alias a slug); every other card
/// becomes `card-<hash>`. The remap is keyed by `normalize_ref` so a legacy
/// short-form decision ref (`decision-1`) resolves against the canonical id.
/// Two source artifacts carrying the same legacy id abort loud: the salt-bump
/// would mint them apart, but every cross-ref to that id could then silently
/// rewire to whichever card claimed the remap entry last.
fn assign_ids(pending: &mut [PendingCard]) -> Result<HashMap<String, String>> {
    let mut taken: HashSet<String> = pending
        .iter()
        .filter(|entry| entry.card.card_type == CardType::Feature)
        .map(|entry| entry.card.id.clone())
        .collect();
    let mut remap: HashMap<String, String> = HashMap::new();
    for entry in pending.iter_mut() {
        if entry.card.card_type == CardType::Feature {
            continue;
        }
        // O3: salt-bump against the in-memory taken set (feature slugs + ids
        // assigned so far), then claim the result so the next mint bumps past it.
        let new_id = mint_hash_id(&entry.kept_id, |candidate| taken.contains(candidate));
        taken.insert(new_id.clone());
        if remap
            .insert(normalize_ref(&entry.kept_id), new_id.clone())
            .is_some()
        {
            bail!(
                "duplicate legacy id '{}': two source artifacts both carry this id, so references to it are ambiguous; resolve the duplicate before migrating",
                entry.kept_id
            );
        }
        entry.card.id = new_id;
    }
    Ok(remap)
}

/// E5: rewrite every structured id field in each card's `extra` to its new id --
/// the record's own `id` plus the typed cross-refs (task blockers, decision
/// supersedes/superseded_by, idea spawned_task/history). Human prose is never
/// touched. A ref whose target is not in the remap is left as-is so the
/// `validate_refs` gate can name it.
fn rewrite_refs(pending: &mut [PendingCard], remap: &HashMap<String, String>) {
    for entry in pending.iter_mut() {
        let new_id = entry.card.id.clone();
        entry
            .card
            .extra
            .insert(Value::String("id".to_string()), Value::String(new_id));
        match entry.card.card_type {
            CardType::Task | CardType::Bug | CardType::Chore => {
                rewrite_task_refs(&mut entry.card.extra, remap)
            }
            CardType::Decision => rewrite_decision_refs(&mut entry.card.extra, remap),
            CardType::Idea => rewrite_idea_refs(&mut entry.card.extra, remap),
            CardType::Feature => {}
        }
    }
}

fn rewrite_task_refs(map: &mut Mapping, remap: &HashMap<String, String>) {
    let Some(Value::Sequence(blockers)) = map.get_mut(Value::String("blockers".to_string())) else {
        return;
    };
    for blocker in blockers.iter_mut() {
        if let Value::Mapping(blocker) = blocker
            && let Some(Value::Mapping(blocked_ref)) =
                blocker.get_mut(Value::String("blocked_ref".to_string()))
        {
            rewrite_string_field(blocked_ref, "id", remap);
        }
    }
}

fn rewrite_decision_refs(map: &mut Mapping, remap: &HashMap<String, String>) {
    if let Some(Value::Sequence(targets)) = map.get_mut(Value::String("supersedes".to_string())) {
        for target in targets.iter_mut() {
            if let Value::String(id) = target
                && let Some(new) = remap.get(&normalize_ref(id))
            {
                *id = new.clone();
            }
        }
    }
    rewrite_string_field(map, "superseded_by", remap);
}

fn rewrite_idea_refs(map: &mut Mapping, remap: &HashMap<String, String>) {
    rewrite_string_field(map, "spawned_task", remap);
    let Some(Value::Sequence(history)) = map.get_mut(Value::String("history".to_string())) else {
        return;
    };
    for entry in history.iter_mut() {
        if let Value::Mapping(entry) = entry {
            rewrite_string_field(entry, "task", remap);
        }
    }
}

/// Rewrite a single string-valued `extra` field through the remap, in place.
/// Leaves the field untouched when it is absent or its value is not remapped.
fn rewrite_string_field(map: &mut Mapping, key: &str, remap: &HashMap<String, String>) {
    let current = map
        .get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(current) = current
        && let Some(new) = remap.get(&normalize_ref(&current))
    {
        map.insert(Value::String(key.to_string()), Value::String(new.clone()));
    }
}

/// Write one card, guarding id collisions and skipping cards a prior run already
/// minted. After writing a feature card, its prose sidecars are copied and their
/// structured decision pointers reminted. Returns whether a new card was written.
fn mint(
    paths: &MaestroPaths,
    minted: &mut HashSet<String>,
    report: &mut CardMigrateReport,
    card: &Card,
    prose_src: Option<&Path>,
    remap: &HashMap<String, String>,
) -> Result<bool> {
    if !minted.insert(card.id.clone()) {
        bail!(
            "card id collision: two source artifacts both map to id '{}'; resolve the duplicate before migrating",
            card.id
        );
    }
    // The idempotency probe asks the resolver, not the flat path: a prior run's
    // card may since have been folded into the container layout (an entry or a
    // pooled task dir), and re-minting it flat would resurrect a stale copy.
    if locate(paths, &card.id)?.is_some() {
        report.skipped += 1;
        return Ok(false);
    }
    let path = card_path(paths, &card.id);
    let snapshot = load_with_snapshot(&path)
        .with_context(|| format!("failed to read card snapshot {}", path.display()))?;
    save_with_snapshot(&path, card, &snapshot)
        .with_context(|| format!("failed to write card {}", card.id))?;
    if let Some(src) = prose_src {
        let card_dir = path
            .parent()
            .with_context(|| format!("card path missing parent: {}", path.display()))?;
        copy_feature_prose(src, card_dir)?;
        rewrite_note_pointers(card_dir, remap)?;
    }
    Ok(true)
}

/// E5: rewrite the structured `decision-NNN` pointers in a feature card's copied
/// `notes.md`. Mirrors `decisions::query::structured_note_decision_refs` exactly:
/// only the first decision token on a `" locked --"` / `" superseded --"` line is
/// a structured pointer, so only that token is rewritten; freeform prose mentions
/// are left untouched. The file is left byte-identical when nothing matches.
fn rewrite_note_pointers(card_dir: &Path, remap: &HashMap<String, String>) -> Result<()> {
    let notes = card_dir.join("notes.md");
    if !notes.is_file() {
        return Ok(());
    }
    let original = fs::read_to_string(&notes)
        .with_context(|| format!("failed to read {}", notes.display()))?;
    let mut changed = false;
    let mut rewritten: Vec<String> = Vec::new();
    for line in original.lines() {
        let (line, did) = rewrite_note_line(line, remap);
        changed |= did;
        rewritten.push(line);
    }
    if !changed {
        return Ok(());
    }
    let mut result = rewritten.join("\n");
    if original.ends_with('\n') {
        result.push('\n');
    }
    fs::write(&notes, result).with_context(|| format!("failed to write {}", notes.display()))?;
    Ok(())
}

fn rewrite_note_line(line: &str, remap: &HashMap<String, String>) -> (String, bool) {
    if !(line.contains(" locked --") || line.contains(" superseded --")) {
        return (line.to_string(), false);
    }
    for token in line.split_whitespace() {
        let candidate = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
        if candidate.starts_with("decision-") && normalize_decision_id(candidate).is_ok() {
            // First structured pointer on the line, mirroring the parser's break.
            return match remap.get(&normalize_ref(candidate)) {
                Some(new) => (line.replacen(candidate, new, 1), true),
                None => (line.to_string(), false),
            };
        }
    }
    (line.to_string(), false)
}

/// E5: rewrite run-event `task_id`s for migrated tasks. Scoped per task to "no
/// live card resolves at the old id": a migrated task lives at `card-<hash>`,
/// so the old id resolves to nothing and its historical events are rewritten; a
/// post-migration `task create` that reuses the old id resolves live (flat or
/// pooled), so its events are left alone; a crash-then-rerun still rewrites
/// (notes 53). The
/// token swap is targeted, not a parse-and-reserialize, so every other byte of
/// the append-only log is preserved.
fn rewrite_run_events(
    paths: &MaestroPaths,
    task_remap: &[(String, String)],
    existing_ids: &HashSet<String>,
) -> Result<()> {
    let mut active: Vec<(&str, &str)> = Vec::new();
    for (old, new) in task_remap {
        // The whole-store scan, not the flat path: a recreated task may live
        // pooled now, and its events must stay its own.
        if !existing_ids.contains(old) {
            active.push((old.as_str(), new.as_str()));
        }
    }
    if active.is_empty() {
        return Ok(());
    }
    for log in managed_event_logs(paths)? {
        let path = log.path();
        let original = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut content = original.clone();
        for (old, new) in &active {
            let needle = format!("\"task_id\":\"{old}\"");
            if content.contains(&needle) {
                content = content.replace(&needle, &format!("\"task_id\":\"{new}\""));
            }
        }
        if content != original {
            fs::write(path, content)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }
    Ok(())
}

/// Fail loud when any captured reference does not resolve after the remint (SPEC
/// E5). Each ref's legacy target is mapped through the remint the same way the
/// rewrite mapped it (`normalize_ref` collapses decision aliasing); the resulting
/// id must be one of the cards this run produced -- or an id frozen in the
/// legacy archive trees, or an artifact this run skipped as already migrated --
/// or the run aborts naming the original target. This is the authoritative
/// dangling-ref gate.
fn validate_refs(
    final_ids: &HashSet<String>,
    frozen: &HashSet<String>,
    migrated: &HashSet<String>,
    remap: &HashMap<String, String>,
    refs: &[CardRef],
) -> Result<()> {
    for reference in refs {
        let resolved = remap_target(remap, &reference.target);
        let normalized = normalize_ref(&reference.target);
        if final_ids.contains(&resolved)
            || frozen.contains(&normalized)
            || migrated.contains(&normalized)
        {
            continue;
        }
        bail!(
            "dangling reference: card '{}' {} points at '{}', which no migrated card provides",
            reference.from,
            reference.field,
            reference.target
        );
    }
    Ok(())
}

/// Ids of artifacts frozen in the legacy archive trees: `archive/tasks/`, plus
/// each archived feature's nested `tasks/` and `decisions.yaml`. A live ref to
/// one is valid history -- the target was archived, not lost -- so the gate
/// admits it and the rewrite leaves the legacy id in place. Only task and
/// decision ids can land here: External/Human blockers carry no `blocked_ref`.
fn frozen_legacy_ids(paths: &MaestroPaths) -> Result<HashSet<String>> {
    let mut ids: HashSet<String> = HashSet::new();
    let archive = paths.archive_dir();
    collect_archived_task_ids(&archive.join("tasks"), &mut ids)?;
    for feature_dir in sorted_child_dirs(&archive.join("features"))? {
        collect_archived_task_ids(&feature_dir.join("tasks"), &mut ids)?;
        let store_path = feature_dir.join("decisions.yaml");
        if !store_path.is_file() {
            continue;
        }
        let store = read_yaml_mapping(&store_path)?;
        let Some(items) = sequence_field(&store, "decisions") else {
            continue;
        };
        for item in items {
            if let Some(id) = item
                .as_mapping()
                .and_then(|record| string_field(record, "id"))
            {
                ids.insert(normalize_ref(&id));
            }
        }
    }
    Ok(ids)
}

fn collect_archived_task_ids(tree: &Path, ids: &mut HashSet<String>) -> Result<()> {
    for task_dir in sorted_child_dirs(tree)? {
        let yaml = task_dir.join("task.yaml");
        if !yaml.is_file() {
            continue;
        }
        if let Some(id) = string_field(&read_yaml_mapping(&yaml)?, "id") {
            ids.insert(normalize_ref(&id));
        }
    }
    Ok(())
}

/// Map a legacy ref target through the remint, falling back to the raw target
/// (which then fails the `final_ids` membership check as a dangling ref).
fn remap_target(remap: &HashMap<String, String>, target: &str) -> String {
    remap
        .get(&normalize_ref(target))
        .cloned()
        .unwrap_or_else(|| target.to_string())
}

/// Normalize an id the way the decision system resolves references, falling back
/// to the raw id for a form it cannot normalize (mirrors `decisions::query`,
/// which uses the same `unwrap_or_else` fallback on both sides of its check).
/// A no-op for task/feature/idea ids, so only the decision alias collapses.
fn normalize_ref(id: &str) -> String {
    normalize_decision_id(id).unwrap_or_else(|_| id.to_string())
}

fn collect_task_refs(from: &str, record: &Mapping, refs: &mut Vec<CardRef>) {
    let Some(blockers) = sequence_field(record, "blockers") else {
        return;
    };
    for blocker in blockers {
        let Some(blocker) = blocker.as_mapping() else {
            continue;
        };
        // A resolved blocker is history: nothing gates on it anymore and its
        // target may be long archived or deleted, so it never aborts the run.
        // The rewrite still remints it when the target did migrate.
        if string_field(blocker, "resolved_at").is_some_and(|at| !at.is_empty()) {
            continue;
        }
        let target = blocker
            .get(Value::String("blocked_ref".to_string()))
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

fn dir_name(dir: &Path) -> Option<String> {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
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
    use crate::domain::card::schema::{Card, CardType};
    use crate::domain::card::store::{hash_id, load as load_card};
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

    fn load(paths: &MaestroPaths, id: &str) -> Card {
        load_card(&card_path(paths, id))
            .expect("load card")
            .unwrap_or_else(|| panic!("card {id} missing"))
    }

    /// Persist a card directly, as a post-migration verb would (used to simulate
    /// a `task create` that reuses a migrated id).
    fn save_card(paths: &MaestroPaths, card: &Card) {
        let path = card_path(paths, &card.id);
        let snapshot = load_with_snapshot(&path).expect("snapshot");
        save_with_snapshot(&path, card, &snapshot).expect("save card");
    }

    /// Append one `task_proof` event line carrying `task_id`/`claim` to a
    /// managed `events.jsonl`, in the compact serde_json shape the rewrite scans.
    fn append_event(path: &Path, task_id: &str, claim: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("event dir");
        }
        let line = format!(
            "{{\"schema_version\":\"1\",\"event\":\"task_proof\",\"task_id\":\"{task_id}\",\"claim\":\"{claim}\",\"ts\":\"2026-06-01T01:00:00Z\"}}\n"
        );
        let mut existing = fs::read_to_string(path).unwrap_or_default();
        existing.push_str(&line);
        fs::write(path, existing).expect("write event");
    }

    fn read_text(path: &Path) -> String {
        fs::read_to_string(path).expect("read file")
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

        // E2: the feature card keeps its slug; every other card is reminted to
        // `card-<hash>` and lives at that dir.
        assert!(
            card_path(&paths, "csv-export").is_file(),
            "feature keeps its slug id"
        );
        for kept in [
            "task-001",
            "task-002",
            "decision-001",
            "decision-002",
            "decision-003",
            "hb-1",
        ] {
            assert!(
                !card_path(&paths, kept).is_file(),
                "non-feature card no longer lives at its kept id {kept}"
            );
            assert!(
                card_path(&paths, &hash_id(kept)).is_file(),
                "{kept} reminted to {}",
                hash_id(kept)
            );
        }

        // Feature card: derived identity + prose sidecars copied beside it.
        let feature_card = load(&paths, "csv-export");
        assert_eq!(feature_card.card_type, CardType::Feature);
        assert_eq!(feature_card.id, "csv-export");
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
        // `parent` is a feature slug, never reminted.
        let task1_card = load(&paths, &hash_id("task-001"));
        assert_eq!(task1_card.id, hash_id("task-001"));
        assert_eq!(task1_card.parent.as_deref(), Some("csv-export"));
        assert_eq!(
            task1_card.status, "in_progress",
            "task status comes from `state`"
        );
        let task2_card = load(&paths, &hash_id("task-002"));
        assert_eq!(task2_card.parent, None);
        assert_eq!(task2_card.card_type, CardType::Task);

        // Decision parent: explicit `feature` field, then per-feature store dir.
        assert_eq!(
            load(&paths, &hash_id("decision-002")).parent.as_deref(),
            Some("csv-export")
        );
        assert_eq!(
            load(&paths, &hash_id("decision-003")).parent.as_deref(),
            Some("csv-export")
        );
        assert_eq!(
            load(&paths, &hash_id("decision-001")).card_type,
            CardType::Decision
        );

        // Idea: harness item maps to idea; first_seen/last_seen drive timestamps.
        let idea = load(&paths, &hash_id("hb-1"));
        assert_eq!(idea.card_type, CardType::Idea);
        assert_eq!(idea.created_at, "2026-06-01T09:00:00Z");
        assert_eq!(idea.updated_at, "2026-06-02T09:00:00Z");

        // Losslessness for the feature: its id is unchanged and it has no
        // structured cross-refs, so `extra` stays the verbatim source mapping.
        let feature_src = read_value(&paths.features_dir().join("csv-export").join("feature.yaml"));
        assert_eq!(
            Value::Mapping(feature_card.extra.clone()),
            feature_src,
            "feature extra lossless"
        );
        let _: FeatureRecord = serde_yaml::from_value(Value::Mapping(feature_card.extra.clone()))
            .expect("feature extra reconstructs a FeatureRecord");

        // Task typed round-trip is the cutover guarantee: `extra` reconstructs the
        // exact record, with its own id and blocker ref reminted to the hash form.
        let task_src_path = paths
            .features_dir()
            .join("csv-export")
            .join("tasks")
            .join("task-001-implement-writer")
            .join("task.yaml");
        let task_original: TaskRecord =
            serde_yaml::from_str(&fs::read_to_string(&task_src_path).expect("read task"))
                .expect("parse task");
        let mut task_expected = task_original.clone();
        task_expected.id = hash_id("task-001");
        task_expected.blockers[0].blocked_ref = Some(BlockerRef {
            kind: BlockerKind::Decision,
            id: hash_id("decision-001"),
        });
        let task_reconstructed: TaskRecord =
            serde_yaml::from_value(Value::Mapping(task1_card.extra.clone()))
                .expect("reconstruct task");
        assert_eq!(
            task_reconstructed, task_expected,
            "task extra reconstructs with reminted id + blocker ref"
        );

        // Decision cross-refs are reminted: superseded_by and supersedes.
        let d1: DecisionRecord = serde_yaml::from_value(Value::Mapping(
            load(&paths, &hash_id("decision-001")).extra.clone(),
        ))
        .expect("reconstruct decision-001");
        assert_eq!(d1.id, hash_id("decision-001"));
        assert_eq!(
            d1.superseded_by.as_deref(),
            Some(hash_id("decision-002").as_str())
        );
        let d2: DecisionRecord = serde_yaml::from_value(Value::Mapping(
            load(&paths, &hash_id("decision-002")).extra.clone(),
        ))
        .expect("reconstruct decision-002");
        assert_eq!(d2.supersedes, vec![hash_id("decision-001")]);

        // Idea cross-refs are reminted: spawned_task and history.task.
        let idea_rec: BacklogItem =
            serde_yaml::from_value(Value::Mapping(idea.extra.clone())).expect("reconstruct idea");
        assert_eq!(idea_rec.id, hash_id("hb-1"));
        assert_eq!(
            idea_rec.spawned_task.as_deref(),
            Some(hash_id("task-002").as_str())
        );
        assert_eq!(
            idea_rec.history[0].task.as_deref(),
            Some(hash_id("task-001").as_str())
        );

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

    /// A re-run after the legacy feature/task trees were deleted but the
    /// harness backlog (and its cross-tree `spawned_task` refs) survived --
    /// the already-migrated cards admit those refs instead of aborting.
    #[test]
    fn rerun_over_a_partially_cleaned_legacy_store_skips_quietly() {
        let root = temp_repo("partial-clean");
        let paths = brownfield(&root);

        run(&paths, NOW).expect("first migrate");
        fs::remove_dir_all(paths.features_dir()).expect("drop legacy features");
        fs::remove_dir_all(paths.tasks_dir()).expect("drop legacy tasks");

        let report = run(&paths, "2026-06-11T00:00:00Z")
            .expect("a surviving backlog ref to a cleaned tree is not dangling");
        assert_eq!(
            report.features + report.tasks + report.decisions + report.ideas,
            0
        );
        assert_eq!(
            report.skipped, 3,
            "the surviving global decisions and backlog item count as skipped"
        );

        cleanup(&root);
    }

    /// A legacy feature whose id collides with a container-layout name would
    /// mint `cards/tasks/card.yaml` -- a record every scan skips as the work
    /// pool. The migrator must refuse it before writing anything.
    #[test]
    fn reserved_legacy_feature_id_fails_loud() {
        let root = temp_repo("reserved-id");
        let paths = brownfield(&root);

        let feature = FeatureRecord::proposed("tasks", "Shadowing slug", "2026-06-01T00:00:00Z");
        write_record(
            &paths.features_dir().join("tasks").join("feature.yaml"),
            &feature,
        );

        let error = run(&paths, NOW).expect_err("reserved feature id must fail loud");
        assert!(
            error.to_string().contains("reserved"),
            "error names the reserved-id rule: {error}"
        );
        assert!(
            !card_path(&paths, "tasks").exists(),
            "no reserved-path record was written"
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
    fn resolved_blocker_to_a_vanished_target_does_not_abort() {
        let root = temp_repo("resolved-blocker");
        let paths = brownfield(&root);

        // The blocker was resolved while its decision still existed; the
        // decision is gone now. History must not abort the migration.
        let mut task = TaskRecord::draft("task-003", "Old work", "2026-06-01T10:00:00Z");
        task.blockers = vec![Blocker {
            id: "b9".to_string(),
            kind: BlockerKind::Decision,
            blocked_ref: Some(BlockerRef {
                kind: BlockerKind::Decision,
                id: "decision-999".to_string(),
            }),
            title: "awaited a long-gone decision".to_string(),
            reason: "resolved years ago".to_string(),
            source: BlockerSource::Command,
            created_at: "2026-06-01T10:00:00Z".to_string(),
            resolved_at: Some("2026-06-02T10:00:00Z".to_string()),
        }];
        write_record(
            &paths
                .tasks_dir()
                .join(task.directory_name())
                .join("task.yaml"),
            &task,
        );

        run(&paths, NOW).expect("a resolved blocker never gates the migration");

        // The vanished target is not remapped: the historical ref is left as-is.
        let migrated: TaskRecord = serde_yaml::from_value(Value::Mapping(
            load(&paths, &hash_id("task-003")).extra.clone(),
        ))
        .expect("reconstruct task-003");
        assert_eq!(
            migrated.blockers[0]
                .blocked_ref
                .as_ref()
                .expect("ref kept")
                .id,
            "decision-999",
            "the resolved blocker's legacy ref is frozen, not rewritten"
        );

        cleanup(&root);
    }

    #[test]
    fn live_ref_to_an_archived_task_is_admitted_frozen() {
        let root = temp_repo("archived-ref");
        let paths = brownfield(&root);

        // task-009 was archived by the legacy verb; a live blocker still points
        // at it. The gate admits the frozen id instead of aborting.
        let archived = TaskRecord::draft("task-009", "Shipped long ago", "2026-05-01T00:00:00Z");
        write_record(
            &paths
                .archive_dir()
                .join("tasks")
                .join(archived.directory_name())
                .join("task.yaml"),
            &archived,
        );
        let mut task = TaskRecord::draft("task-003", "Follow-up", "2026-06-01T10:00:00Z");
        task.blockers = vec![Blocker {
            id: "b9".to_string(),
            kind: BlockerKind::Task,
            blocked_ref: Some(BlockerRef {
                kind: BlockerKind::Task,
                id: "task-009".to_string(),
            }),
            title: "depends on archived work".to_string(),
            reason: "still open".to_string(),
            source: BlockerSource::Command,
            created_at: "2026-06-01T10:00:00Z".to_string(),
            resolved_at: None,
        }];
        write_record(
            &paths
                .tasks_dir()
                .join(task.directory_name())
                .join("task.yaml"),
            &task,
        );

        run(&paths, NOW).expect("a ref into the legacy archive is valid history");

        let migrated: TaskRecord = serde_yaml::from_value(Value::Mapping(
            load(&paths, &hash_id("task-003")).extra.clone(),
        ))
        .expect("reconstruct task-003");
        assert_eq!(
            migrated.blockers[0]
                .blocked_ref
                .as_ref()
                .expect("ref kept")
                .id,
            "task-009",
            "the archived target's id stays frozen in the legacy form"
        );

        cleanup(&root);
    }

    #[test]
    fn duplicate_legacy_ids_abort_loud() {
        let root = temp_repo("dup-ids");
        let paths = brownfield(&root);

        // decision-003 already lives in the feature store; a second artifact
        // carrying the same id makes every ref to it ambiguous.
        let mut global = DecisionStore::empty();
        global.decisions = vec![
            decision("decision-001", DecisionStatus::Locked, None),
            decision("decision-003", DecisionStatus::Open, None),
        ];
        write_record(&paths.decisions_file(), &global);

        let error = run(&paths, NOW).expect_err("duplicate legacy ids must abort");
        let message = format!("{error:#}");
        assert!(
            message.contains("duplicate legacy id 'decision-003'"),
            "error names the duplicated id: {message}"
        );

        cleanup(&root);
    }

    #[test]
    fn legacy_id_colliding_with_feature_slug_is_reminted_apart() {
        let root = temp_repo("collision");
        let paths = brownfield(&root);

        // A decision whose legacy id equals the feature slug. Under keep-ids this
        // aborted as a collision; the hash remint gives the decision a
        // `card-<hash>` id while the feature keeps its slug, so both migrate.
        let mut global = DecisionStore::empty();
        global.decisions = vec![
            decision("decision-001", DecisionStatus::Locked, None),
            decision("csv-export", DecisionStatus::Open, None),
        ];
        write_record(&paths.decisions_file(), &global);

        run(&paths, NOW).expect("hash remint resolves the slug/id collision");

        // Feature keeps its slug dir; the colliding decision lands at its hash.
        assert_eq!(load(&paths, "csv-export").card_type, CardType::Feature);
        let decision_hash = hash_id("csv-export");
        assert_ne!(decision_hash, "csv-export");
        assert_eq!(load(&paths, &decision_hash).card_type, CardType::Decision);

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

        // The short-form ref is reminted to the canonical decision's hash.
        let d2_rec: DecisionRecord = serde_yaml::from_value(Value::Mapping(
            load(&paths, &hash_id("decision-002")).extra.clone(),
        ))
        .expect("reconstruct decision-002");
        assert_eq!(d2_rec.supersedes, vec![hash_id("decision-001")]);

        cleanup(&root);
    }

    #[test]
    fn migration_rewrites_run_event_attribution() {
        // E5: a proof event naming a migrated task is rewritten to the reminted
        // id, so `proof`/`claims` (run::visit_managed_events + RunEvent::task_id,
        // the claims.rs join) still attributes the proof end-to-end.
        let root = temp_repo("proof-attr");
        let paths = MaestroPaths::new(&root);

        let task = TaskRecord::draft("task-001", "Writer", "2026-06-01T01:00:00Z");
        write_record(
            &paths
                .tasks_dir()
                .join(task.directory_name())
                .join("task.yaml"),
            &task,
        );
        let events = paths
            .maestro_dir()
            .join("runs")
            .join("cli-1")
            .join("events.jsonl");
        append_event(&events, "task-001", "exports a header row");

        run(&paths, NOW).expect("migrate");
        let hash = hash_id("task-001");

        let mut attributed: Vec<String> = Vec::new();
        let mut stale = false;
        crate::domain::run::visit_managed_events(&paths, |record| {
            let event = record.event();
            if event.task_id() == Some(hash.as_str()) {
                attributed.extend(event.claim().map(str::to_string));
            }
            if event.task_id() == Some("task-001") {
                stale = true;
            }
            Ok(())
        })
        .expect("visit events");

        assert!(!stale, "no event still carries the pre-migration task id");
        assert_eq!(
            attributed,
            vec!["exports a header row".to_string()],
            "proof attributes to the reminted id"
        );

        cleanup(&root);
    }

    #[test]
    fn run_event_rewrite_skips_a_recreated_task_id() {
        // E5 scoping (notes 53): a migrated task's events are rewritten, but a
        // post-migration `task create` that reuses the old id keeps its own
        // events -- the live card at the old id guards them.
        let root = temp_repo("event-scope");
        let paths = MaestroPaths::new(&root);

        let task = TaskRecord::draft("task-001", "Writer", "2026-06-01T01:00:00Z");
        write_record(
            &paths
                .tasks_dir()
                .join(task.directory_name())
                .join("task.yaml"),
            &task,
        );
        let events = paths
            .maestro_dir()
            .join("runs")
            .join("cli-1")
            .join("events.jsonl");
        append_event(&events, "task-001", "wrote the rows");

        run(&paths, NOW).expect("first migrate");
        let hash = hash_id("task-001");
        assert!(
            read_text(&events).contains(&format!("\"task_id\":\"{hash}\"")),
            "first migrate rewrites the historical event to the reminted id"
        );
        assert!(!read_text(&events).contains("\"task_id\":\"task-001\""));

        // Simulate a post-migration recreate of task-001 (Option A keeps numbered
        // minting) plus a fresh event under that live id.
        save_card(
            &paths,
            &Card::new("task-001", CardType::Task, "New writer", "draft", NOW),
        );
        append_event(&events, "task-001", "new run output");

        run(&paths, "2026-06-09T00:00:00Z").expect("second migrate");

        let after = read_text(&events);
        assert!(
            after.contains("\"task_id\":\"task-001\""),
            "the recreated task's event is left untouched: {after}"
        );
        assert!(
            after.contains(&format!("\"task_id\":\"{hash}\"")),
            "the migrated task's historical event stays reminted: {after}"
        );

        cleanup(&root);
    }
}
