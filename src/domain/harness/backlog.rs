use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, Result, bail};

use crate::domain::card::store::{self as card_store, CardSnapshot, card_path};
use crate::domain::card::{StoreMode, store_mode};
use crate::domain::harness::cards;
use crate::domain::harness::schema::{
    BacklogConfig, BacklogItem, EscalationPolicy, HistoryEntry, is_state_detector,
};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::write_string_if_unchanged;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{BACKLOG_SCHEMA_VERSION, Compat, classify};
use crate::foundation::core::time::utc_now_timestamp;

/// Load the Harness backlog, returning an empty V1 backlog when it does not exist.
pub fn load(paths: &MaestroPaths) -> Result<BacklogConfig> {
    Ok(load_with_snapshot(paths)?.backlog)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BacklogSnapshot {
    pub backlog: BacklogConfig,
    cas: BacklogCas,
}

/// The CAS basis the matching save checks, dispatched by store mode
/// (SPEC-beads-model P1 dual-read). Legacy guards the whole `backlog.yaml`; card
/// mode guards each item card plus the metadata file that holds the store-level
/// `evidence_stamp` (items move to `idea` cards).
#[derive(Clone, Debug, PartialEq)]
enum BacklogCas {
    Legacy {
        raw: Option<String>,
    },
    Cards {
        meta_raw: Option<String>,
        cards: Vec<(String, CardSnapshot)>,
    },
}

/// Load the Harness backlog with the exact bytes used for optimistic save.
pub(crate) fn load_with_snapshot(paths: &MaestroPaths) -> Result<BacklogSnapshot> {
    match store_mode(paths) {
        StoreMode::Cards => load_with_snapshot_cards(paths),
        StoreMode::Legacy => load_with_snapshot_legacy(paths),
    }
}

fn load_with_snapshot_legacy(paths: &MaestroPaths) -> Result<BacklogSnapshot> {
    let path = backlog_path(paths)?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => Some(raw),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let backlog: BacklogConfig = match raw.as_deref() {
        Some(raw) => serde_yaml::from_str(raw)
            .with_context(|| format!("failed to parse {}", path.display()))?,
        None => BacklogConfig::empty(),
    };
    validate_schema(&path, &backlog)?;
    Ok(BacklogSnapshot {
        backlog,
        cas: BacklogCas::Legacy { raw },
    })
}

/// Card mode: store-level metadata (`schema_version` + `evidence_stamp`) reads
/// from `backlog.yaml`; the items read from the `idea` cards. The frozen
/// migration leftovers in `backlog.yaml#items` are ignored -- the cards are
/// authoritative -- so the first save rewrites the metadata file with `items: []`.
fn load_with_snapshot_cards(paths: &MaestroPaths) -> Result<BacklogSnapshot> {
    let path = backlog_path(paths)?;
    let meta_raw = match fs::read_to_string(&path) {
        Ok(raw) => Some(raw),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let meta: BacklogConfig = match meta_raw.as_deref() {
        Some(raw) => serde_yaml::from_str(raw)
            .with_context(|| format!("failed to parse {}", path.display()))?,
        None => BacklogConfig::empty(),
    };
    let mut items = Vec::new();
    let mut snapshots = Vec::new();
    for (item, snapshot, _) in cards::scan(paths)? {
        snapshots.push((item.id.clone(), snapshot));
        items.push(item);
    }
    let backlog = BacklogConfig {
        schema_version: meta.schema_version,
        evidence_stamp: meta.evidence_stamp,
        items,
    };
    validate_schema(&path, &backlog)?;
    Ok(BacklogSnapshot {
        backlog,
        cas: BacklogCas::Cards {
            meta_raw,
            cards: snapshots,
        },
    })
}

/// Persist a Harness backlog through the managed Harness path policy.
pub fn save(paths: &MaestroPaths, backlog: &BacklogConfig) -> Result<()> {
    match store_mode(paths) {
        // No caller-held snapshot: load the current one and replace the store
        // against it, so card-mode save still drops reconciled-away item cards.
        StoreMode::Cards => save_with_snapshot(paths, backlog, &load_with_snapshot_cards(paths)?),
        StoreMode::Legacy => {
            let path = backlog_path(paths)?;
            validate_schema(&path, backlog)?;
            let raw = serde_yaml::to_string(backlog).context("failed to serialize backlog")?;
            write_string_atomic(&path, &raw)
                .with_context(|| format!("failed to write {}", path.display()))
        }
    }
}

/// Persist a Harness backlog only if the store still matches the loaded snapshot.
pub(crate) fn save_with_snapshot(
    paths: &MaestroPaths,
    backlog: &BacklogConfig,
    snapshot: &BacklogSnapshot,
) -> Result<()> {
    match &snapshot.cas {
        BacklogCas::Legacy { raw } => {
            let path = backlog_path(paths)?;
            validate_schema(&path, backlog)?;
            let serialized =
                serde_yaml::to_string(backlog).context("failed to serialize backlog")?;
            write_string_if_unchanged(&path, raw.as_deref(), &serialized)
                .with_context(|| format!("failed to write {}", path.display()))
        }
        BacklogCas::Cards { meta_raw, cards } => {
            save_cards(paths, backlog, meta_raw.as_deref(), cards)
        }
    }
}

/// Card-mode save: each item folds to its card under per-card CAS (a merge-minted
/// id CAS-creates against its absent snapshot, so a concurrent mint of the same id
/// loses), reconciled-away items have their cards removed, and the store metadata
/// lands under the metadata-file CAS. The multi-file write is not atomic across
/// the cards and the metadata file; D7/P5 collapses the two stores into one.
fn save_cards(
    paths: &MaestroPaths,
    backlog: &BacklogConfig,
    meta_raw: Option<&str>,
    loaded: &[(String, CardSnapshot)],
) -> Result<()> {
    let meta_path = backlog_path(paths)?;
    validate_schema(&meta_path, backlog)?;
    for item in &backlog.items {
        let path = card_path(paths, &item.id);
        let snapshot = match loaded.iter().find(|(id, _)| id == &item.id) {
            Some((_, snapshot)) => snapshot.clone(),
            None => {
                // A merge-minted id absent from the load snapshot must CAS-create
                // against an empty card. If a concurrent writer already committed
                // it, the fresh load returns a matching card and the create would
                // silently overwrite it -- bail so the racing mint loses instead.
                let fresh = card_store::load_with_snapshot(&path)?;
                if fresh.card.is_some() {
                    bail!(
                        "backlog item {} was created concurrently; reload and retry",
                        item.id
                    );
                }
                fresh
            }
        };
        cards::save_at(&path, item, &snapshot)?;
    }
    let kept: BTreeSet<&str> = backlog.items.iter().map(|item| item.id.as_str()).collect();
    for (id, _) in loaded {
        if !kept.contains(id.as_str()) {
            cards::remove(paths, id)?;
        }
    }
    let serialized = serde_yaml::to_string(&BacklogConfig {
        schema_version: backlog.schema_version.clone(),
        evidence_stamp: backlog.evidence_stamp.clone(),
        items: Vec::new(),
    })
    .context("failed to serialize backlog metadata")?;
    write_string_if_unchanged(&meta_path, meta_raw, &serialized)
        .with_context(|| format!("failed to write {}", meta_path.display()))
}

/// Refresh proposals into the Harness backlog without applying them.
pub fn refresh(paths: &MaestroPaths, proposals: Vec<BacklogItem>) -> Result<BacklogConfig> {
    let mut snapshot = load_with_snapshot(paths)?;
    merge_proposals(&mut snapshot.backlog, proposals);
    save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok(snapshot.backlog)
}

/// Merge proposals into the backlog keyed on stable fingerprint and assign
/// deterministic ids. Re-detecting a terminal `measured` state note reopens it
/// (D6); a `proposed` note with no durable history that is no longer detected
/// is reconciled away (D4).
pub fn merge_proposals(backlog: &mut BacklogConfig, proposals: Vec<BacklogItem>) {
    merge_proposals_inner(backlog, proposals, true);
}

/// Merge agent-authored proposals without reconciling away detector-authored
/// ephemeral items that are absent from this single manual proposal call.
pub fn merge_proposals_preserving_absent(backlog: &mut BacklogConfig, proposals: Vec<BacklogItem>) {
    merge_proposals_inner(backlog, proposals, false);
}

fn merge_proposals_inner(
    backlog: &mut BacklogConfig,
    mut proposals: Vec<BacklogItem>,
    reconcile_absent_ephemeral: bool,
) {
    sanitize_existing_generated_evidence(backlog);
    let fresh_fingerprints = proposals
        .iter()
        .map(|proposal| proposal.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut fingerprints = backlog
        .items
        .iter()
        .map(|item| item.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut next = next_backlog_number(&backlog.items);
    proposals.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));

    for mut proposal in proposals {
        normalize_recurrence(&mut proposal);
        let fingerprint = proposal.fingerprint.clone();
        if !fingerprint.is_empty() && fingerprints.contains(&fingerprint) {
            if let Some(existing) = backlog
                .items
                .iter_mut()
                .find(|item| item.fingerprint == fingerprint && item.status != "dismissed")
            {
                reopen_if_regressed(existing);
                refresh_existing_recurrence(existing, &proposal);
                refresh_existing_evidence(existing, &proposal);
            }
            continue;
        }
        proposal.id = format!("hb-{next:03}");
        next += 1;
        fingerprints.insert(fingerprint);
        backlog.items.push(proposal);
    }

    if reconcile_absent_ephemeral {
        backlog
            .items
            .retain(|item| !is_ephemeral_reconcilable(item, &fresh_fingerprints));
    }
}

/// D6: a re-detected, terminal `measured` state note flips back to `proposed`
/// and logs the regression. Behavioral notes are kept as-is.
fn reopen_if_regressed(existing: &mut BacklogItem) {
    if existing.status == "measured" && is_state_detector(&existing.item_type) {
        existing.status = "proposed".to_string();
        existing.history.push(HistoryEntry {
            result: "regressed".to_string(),
            task: existing.spawned_task.clone(),
            note: None,
            at: utc_now_timestamp(),
        });
        // Drop the link so the next accept spawns a fresh task (impl-default (c)),
        // mirroring the D2 ineffective path. The old task stays in history.
        existing.spawned_task = None;
    }
}

/// D4: drop a `proposed` note with no durable history that the current
/// detection run no longer produces. Durable = a spawned task or any history.
fn is_ephemeral_reconcilable(item: &BacklogItem, fresh_fingerprints: &BTreeSet<String>) -> bool {
    item.status == "proposed"
        && item.spawned_task.is_none()
        && item.history.is_empty()
        && (item.provenance.is_empty() || item.provenance == "detector")
        && !fresh_fingerprints.contains(&item.fingerprint)
}

/// Derive stored priority from the active escalation policy.
pub fn apply_escalation_policy(backlog: &mut BacklogConfig, policy: &EscalationPolicy) {
    for item in &mut backlog.items {
        item.priority = policy.priority_for(item.sessions_hit.len(), &item.priority);
    }
}

fn normalize_recurrence(item: &mut BacklogItem) {
    if item.sessions_hit.is_empty() && !item.source.is_empty() {
        item.sessions_hit.push(item.source.clone());
    }
    item.sessions_hit.sort();
    item.sessions_hit.dedup();
    if item.occurrences == 0 {
        item.occurrences = item.sessions_hit.len();
    }
}

fn refresh_existing_recurrence(existing: &mut BacklogItem, proposal: &BacklogItem) {
    if existing.first_seen.is_empty() {
        existing.first_seen = proposal.first_seen.clone();
    }
    existing.last_seen = proposal.last_seen.clone();
    existing.occurrences = proposal.occurrences;
    existing.sessions_hit = proposal.sessions_hit.clone();
    normalize_recurrence(existing);
}

fn refresh_existing_evidence(existing: &mut BacklogItem, proposal: &BacklogItem) {
    if !matches!(
        existing.item_type.as_str(),
        "missing_verification" | "explicit_intervention" | "agent_audit"
    ) {
        return;
    }

    if existing.item_type != "missing_verification" {
        for evidence in &proposal.evidence {
            if !existing.evidence.contains(evidence) {
                existing.evidence.push(evidence.clone());
            }
        }
        return;
    }

    let mut refreshed = existing
        .evidence
        .iter()
        .filter(|evidence| !is_generated_missing_verification_evidence(evidence))
        .cloned()
        .collect::<Vec<_>>();
    for (index, evidence) in proposal.evidence.iter().enumerate() {
        let evidence = sanitize_missing_verification_evidence(evidence, index);
        if !refreshed.contains(&evidence) {
            refreshed.push(evidence);
        }
    }
    existing.evidence = refreshed;
}

fn sanitize_existing_generated_evidence(backlog: &mut BacklogConfig) {
    for item in &mut backlog.items {
        if item.item_type == "missing_verification" {
            let mut generated_index = 0;
            item.evidence = item
                .evidence
                .iter()
                .map(|evidence| {
                    if is_generated_missing_verification_evidence(evidence) {
                        let sanitized =
                            sanitize_missing_verification_evidence(evidence, generated_index);
                        generated_index += 1;
                        sanitized
                    } else {
                        evidence.to_string()
                    }
                })
                .collect();
        }
    }
}

fn is_generated_missing_verification_evidence(evidence: &str) -> bool {
    is_safe_missing_verification_evidence(evidence)
        || is_legacy_generated_missing_verification_evidence(evidence)
}

fn sanitize_missing_verification_evidence(evidence: &str, index: usize) -> String {
    if let Some(source) = safe_missing_verification_source(evidence) {
        return format!(
            "{} used verification command {} outside harness.yml",
            source,
            index + 1
        );
    }
    let Some((source, detail)) = evidence.split_once(" used ") else {
        return evidence.to_string();
    };
    if !detail.ends_with(" outside harness.yml") {
        return evidence.to_string();
    }
    let source = safe_verification_source(source);
    if source == "verification evidence" {
        return evidence.to_string();
    }
    format!(
        "{} used verification command {} outside harness.yml",
        source,
        index + 1
    )
}

fn is_safe_missing_verification_evidence(evidence: &str) -> bool {
    safe_missing_verification_source(evidence).is_some()
}

fn safe_missing_verification_source(evidence: &str) -> Option<&str> {
    let (source, command) = evidence.split_once(" used ")?;
    let label = command
        .strip_prefix("verification command ")
        .and_then(|label| label.strip_suffix(" outside harness.yml"))?;
    if safe_verification_source(source) == source.trim() && label.parse::<usize>().is_ok() {
        Some(source.trim())
    } else {
        None
    }
}

fn is_legacy_generated_missing_verification_evidence(evidence: &str) -> bool {
    let Some((source, detail)) = evidence.split_once(" used ") else {
        return false;
    };
    detail.ends_with(" outside harness.yml")
        && safe_verification_source(source) != "verification evidence"
}

fn safe_verification_source(source: &str) -> &'static str {
    let source = source.trim();
    if source == "task.yaml#verification" {
        return "task.yaml#verification";
    }
    if source == "verification.json" {
        return "verification.json";
    }
    if source == "verification.attempts/latest.json" {
        return "verification.attempts/latest.json";
    }
    if source.starts_with("verification.attempts/") {
        return "verification.attempts/archived attempt";
    }
    "verification evidence"
}

fn backlog_path(paths: &MaestroPaths) -> Result<std::path::PathBuf> {
    managed_path(
        paths,
        ".maestro/harness/backlog.yaml",
        SymlinkPolicy::RejectAllComponents,
    )
}

fn validate_schema(path: &std::path::Path, backlog: &BacklogConfig) -> Result<()> {
    if classify(&backlog.schema_version, BACKLOG_SCHEMA_VERSION) != Compat::Exact {
        // Use the typed error (same Display text) so a backlog schema mismatch
        // emits the `fix: run maestro doctor` hint like feature/decision stores,
        // rather than a bare bail with no remedy.
        return Err(MaestroError::SchemaMismatch {
            artifact: path.display().to_string(),
            expected: BACKLOG_SCHEMA_VERSION,
            found: backlog.schema_version.clone(),
        }
        .into());
    }
    Ok(())
}

fn next_backlog_number(items: &[BacklogItem]) -> u32 {
    items
        .iter()
        .filter_map(|item| item.id.strip_prefix("hb-"))
        .filter_map(|number| number.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::domain::harness::backlog;
    use crate::domain::harness::schema::{BacklogConfig, BacklogItem};
    use crate::foundation::core::paths::MaestroPaths;

    fn temp_paths(name: &str) -> (PathBuf, MaestroPaths) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-{name}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        fs::create_dir_all(paths.harness_dir())
            .expect("invariant: harness dir should be creatable");
        (root, paths)
    }

    fn item(id: &str, title: &str) -> BacklogItem {
        BacklogItem {
            id: id.to_string(),
            fingerprint: id.to_string(),
            source: id.to_string(),
            provenance: "test".to_string(),
            topic: id.to_string(),
            item_type: "agent_audit".to_string(),
            title: title.to_string(),
            priority: "medium".to_string(),
            occurrences: 1,
            sessions_hit: vec![id.to_string()],
            first_seen: String::new(),
            last_seen: String::new(),
            status: "proposed".to_string(),
            evidence: vec![title.to_string()],
            spawned_task: None,
            dismissal_reason: None,
            history: Vec::new(),
        }
    }

    #[test]
    fn save_with_snapshot_rejects_stale_backlog_writer() {
        let (_root, paths) = temp_paths("backlog-stale-writer");
        let mut first = backlog::load_with_snapshot(&paths)
            .expect("invariant: first backlog load should succeed");
        let mut second = backlog::load_with_snapshot(&paths)
            .expect("invariant: second backlog load should succeed");

        second.backlog.items.push(item("hb-001", "second writer"));
        backlog::save_with_snapshot(&paths, &second.backlog, &second)
            .expect("invariant: second writer should save first");

        first.backlog.items.push(item("hb-002", "stale writer"));
        let error = backlog::save_with_snapshot(&paths, &first.backlog, &first)
            .expect_err("stale writer must be rejected");
        assert!(
            error.to_string().contains("failed to write")
                && format!("{error:#}").contains("changed since it was read; re-run"),
            "{error:#}"
        );
    }

    fn card_mode_paths(name: &str) -> (PathBuf, MaestroPaths) {
        let (root, paths) = temp_paths(name);
        fs::create_dir_all(paths.cards_dir()).expect("invariant: cards dir should be creatable");
        (root, paths)
    }

    /// Card mode: a whole-store save folds the items to cards, keeps the
    /// store-level `evidence_stamp` in the metadata file, and removes the card for
    /// an item the caller dropped (the merge's D4 reconciliation surfaces here).
    #[test]
    fn card_mode_save_moves_items_to_cards_and_drops_removed() {
        let (_root, paths) = card_mode_paths("backlog-card-roundtrip");
        let mut config = BacklogConfig::empty();
        config.evidence_stamp = "stamp-1".to_string();
        config.items = vec![item("hb-001", "first"), item("hb-002", "second")];
        backlog::save(&paths, &config).expect("save two items as cards");

        let reloaded = backlog::load(&paths).expect("reload from cards");
        assert_eq!(reloaded.items.len(), 2, "items read back from the cards");
        assert_eq!(
            reloaded.evidence_stamp, "stamp-1",
            "stamp kept in the metadata file"
        );
        assert!(paths.cards_dir().join("hb-001").join("card.yaml").is_file());
        assert!(paths.cards_dir().join("hb-002").join("card.yaml").is_file());

        let mut snapshot = backlog::load_with_snapshot(&paths).expect("load snapshot");
        snapshot.backlog.items.retain(|item| item.id == "hb-001");
        backlog::save_with_snapshot(&paths, &snapshot.backlog, &snapshot).expect("save with drop");

        assert!(paths.cards_dir().join("hb-001").join("card.yaml").is_file());
        assert!(
            !paths.cards_dir().join("hb-002").exists(),
            "the dropped item's card is removed"
        );
        let after = backlog::load(&paths).expect("reload after drop");
        assert_eq!(after.items.len(), 1);
        assert_eq!(after.items[0].id, "hb-001");
    }

    /// SPEC D1 in card mode through the aggregate snapshot: two readers each take a
    /// whole-store snapshot, the first save wins, and the second is rejected by the
    /// per-card CAS that the `BacklogSnapshot` threads through.
    #[test]
    fn card_mode_save_with_snapshot_rejects_stale_writer() {
        let (_root, paths) = card_mode_paths("backlog-card-stale-writer");
        let mut config = BacklogConfig::empty();
        config.items = vec![item("hb-001", "seed")];
        backlog::save(&paths, &config).expect("seed one item card");

        let mut first = backlog::load_with_snapshot(&paths).expect("first load");
        let mut second = backlog::load_with_snapshot(&paths).expect("second load");

        second.backlog.find_mut("hb-001").expect("item").title = "second writer".to_string();
        backlog::save_with_snapshot(&paths, &second.backlog, &second)
            .expect("second writer saves first");

        first.backlog.find_mut("hb-001").expect("item").title = "stale writer".to_string();
        let error = backlog::save_with_snapshot(&paths, &first.backlog, &first)
            .expect_err("stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );
    }

    /// SPEC D1 for the new-item branch: two readers snapshot the empty store and
    /// mint the same id. The per-card edit CAS cannot catch this -- the id is in
    /// neither snapshot -- so the create path's existence guard must reject the
    /// second mint instead of silently overwriting the first writer's card.
    #[test]
    fn card_mode_save_rejects_a_concurrent_new_item_with_the_same_id() {
        let (_root, paths) = card_mode_paths("backlog-card-concurrent-create");

        let mut first = backlog::load_with_snapshot(&paths).expect("first load");
        let mut second = backlog::load_with_snapshot(&paths).expect("second load");

        first.backlog.items.push(item("hb-001", "first writer"));
        backlog::save_with_snapshot(&paths, &first.backlog, &first).expect("first create wins");

        second.backlog.items.push(item("hb-001", "second writer"));
        let error = backlog::save_with_snapshot(&paths, &second.backlog, &second)
            .expect_err("concurrent create must be rejected");
        assert!(
            format!("{error:#}").contains("created concurrently"),
            "{error:#}"
        );

        let reloaded = backlog::load(&paths).expect("reload after the rejected create");
        assert_eq!(
            reloaded.items.len(),
            1,
            "only the first writer's card landed"
        );
        assert_eq!(reloaded.find("hb-001").expect("item").title, "first writer");
    }
}
