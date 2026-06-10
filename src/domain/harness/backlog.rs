use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Result;

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardSnapshot, EntriesSnapshot, card_path};
use crate::domain::harness::cards;
use crate::domain::harness::schema::{
    BacklogConfig, BacklogItem, EscalationPolicy, HistoryEntry, is_state_detector,
};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// Load the Harness backlog, returning an empty backlog when no idea cards exist.
pub fn load(paths: &MaestroPaths) -> Result<BacklogConfig> {
    Ok(load_with_snapshot(paths)?.backlog)
}

/// Read the backlog from an already-loaded card set (the card-aware doctor's
/// one store walk). Read-only: it carries no CAS snapshots, so it can never
/// back a save.
pub fn items_in_cards(cards: &[(Card, PathBuf)]) -> Result<BacklogConfig> {
    let mut items = Vec::new();
    for (card, path) in cards {
        if card.card_type != CardType::Idea {
            continue;
        }
        items.push(cards::item_from_card(
            card.clone(),
            &path.display().to_string(),
        )?);
    }
    Ok(BacklogConfig { items })
}

/// The CAS basis the matching save checks: the `ideas.yaml` container file as
/// a whole, plus one card snapshot per pre-migration flat idea dir (D7 -- the
/// card store is the only store; there is no metadata file).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BacklogSnapshot {
    pub backlog: BacklogConfig,
    dirs: Vec<(String, CardSnapshot)>,
    entries: EntriesSnapshot,
}

/// Load the Harness backlog with the exact store bytes used for optimistic
/// save: flat straggler dirs plus the `ideas.yaml` entry list, sorted by id
/// (the order the per-dir store read in).
pub(crate) fn load_with_snapshot(paths: &MaestroPaths) -> Result<BacklogSnapshot> {
    let mut items = Vec::new();
    let mut dirs = Vec::new();
    for (item, snapshot, _) in cards::scan(paths)? {
        dirs.push((item.id.clone(), snapshot));
        items.push(item);
    }
    let ideas_file = paths.cards_dir().join(card_store::IDEAS_FILE);
    let entries = card_store::load_entries(&ideas_file)?;
    for card in entries.cards.clone() {
        let artifact = format!("{}#{}", ideas_file.display(), card.id);
        items.push(cards::item_from_card(card, &artifact)?);
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(BacklogSnapshot {
        backlog: BacklogConfig { items },
        dirs,
        entries,
    })
}

/// Persist a Harness backlog through the card store. With no caller-held
/// snapshot, load the current one and replace the store against it so the save
/// still drops reconciled-away item cards.
pub fn save(paths: &MaestroPaths, backlog: &BacklogConfig) -> Result<()> {
    save_with_snapshot(paths, backlog, &load_with_snapshot(paths)?)
}

/// Persist a Harness backlog only if the store still matches the loaded snapshot.
pub(crate) fn save_with_snapshot(
    paths: &MaestroPaths,
    backlog: &BacklogConfig,
    snapshot: &BacklogSnapshot,
) -> Result<()> {
    save_cards(paths, backlog, snapshot)
}

/// A pre-migration flat-dir item saves in place under per-card CAS; every
/// other item -- existing entries and merge-minted ids alike -- folds into
/// `ideas.yaml` through ONE whole-file CAS write (the F7 rider: a per-entry
/// save would self-conflict after the first write invalidated the snapshot,
/// and a concurrent writer of any entry trips the same check). A
/// reconciled-away dir item has its dir removed (D4); an entry item simply
/// leaves the list.
fn save_cards(
    paths: &MaestroPaths,
    backlog: &BacklogConfig,
    snapshot: &BacklogSnapshot,
) -> Result<()> {
    let dir_ids: BTreeSet<&str> = snapshot.dirs.iter().map(|(id, _)| id.as_str()).collect();
    let mut entry_cards = Vec::new();
    for item in &backlog.items {
        match snapshot.dirs.iter().find(|(id, _)| id == &item.id) {
            Some((_, dir_snapshot)) => {
                cards::save_at(&card_path(paths, &item.id), item, dir_snapshot)?;
            }
            None => entry_cards.push(cards::card_for(item)?),
        }
    }
    // Skip creating an empty ideas.yaml when the store had none and the save
    // adds none; an existing file is rewritten even when emptied.
    if !entry_cards.is_empty() || snapshot.entries.exists() {
        let ideas_file = paths.cards_dir().join(card_store::IDEAS_FILE);
        card_store::save_entries(&ideas_file, &entry_cards, &snapshot.entries)?;
    }
    let kept: BTreeSet<&str> = backlog.items.iter().map(|item| item.id.as_str()).collect();
    for id in dir_ids {
        if !kept.contains(id) {
            cards::remove(paths, id)?;
        }
    }
    Ok(())
}

/// Refresh proposals into the Harness backlog without applying them.
pub fn refresh(paths: &MaestroPaths, proposals: Vec<BacklogItem>) -> Result<BacklogConfig> {
    let mut snapshot = load_with_snapshot(paths)?;
    merge_proposals(paths, &mut snapshot.backlog, proposals)?;
    save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok(snapshot.backlog)
}

/// Merge proposals into the backlog keyed on stable fingerprint, minting a
/// `card-<hash>` id for each new item (E2). Re-detecting a terminal `measured`
/// state note reopens it (D6); a `proposed` note with no durable history that is
/// no longer detected is reconciled away (D4).
pub fn merge_proposals(
    paths: &MaestroPaths,
    backlog: &mut BacklogConfig,
    proposals: Vec<BacklogItem>,
) -> Result<()> {
    merge_proposals_inner(paths, backlog, proposals, true)
}

/// Merge agent-authored proposals without reconciling away detector-authored
/// ephemeral items that are absent from this single manual proposal call.
pub fn merge_proposals_preserving_absent(
    paths: &MaestroPaths,
    backlog: &mut BacklogConfig,
    proposals: Vec<BacklogItem>,
) -> Result<()> {
    merge_proposals_inner(paths, backlog, proposals, false)
}

fn merge_proposals_inner(
    paths: &MaestroPaths,
    backlog: &mut BacklogConfig,
    mut proposals: Vec<BacklogItem>,
    reconcile_absent_ephemeral: bool,
) -> Result<()> {
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
                // A re-detection of a known fingerprint merges through the
                // per-type hook (SPEC E7); the idea arm lands back in
                // `reconcile_item` below.
                let merged = CardType::Idea
                    .reconcile(cards::card_for(existing)?, cards::card_for(&proposal)?)?;
                *existing = cards::item_from_card(merged, "reconciled idea card")?;
            }
            continue;
        }
        proposal.id = card_store::mint_card_id(paths, &proposal.title);
        fingerprints.insert(fingerprint);
        backlog.items.push(proposal);
    }

    if reconcile_absent_ephemeral {
        backlog
            .items
            .retain(|item| !is_ephemeral_reconcilable(item, &fresh_fingerprints));
    }
    Ok(())
}

/// Pair-level merge for a fingerprint-matched re-detection: the implementation
/// behind `CardType::reconcile`'s idea arm (SPEC E7). Reopens a regressed
/// terminal state note (D6), then refreshes recurrence and evidence from the
/// fresh detection.
pub(crate) fn reconcile_item(existing: &mut BacklogItem, incoming: &BacklogItem) {
    reopen_if_regressed(existing);
    refresh_existing_recurrence(existing, incoming);
    refresh_existing_evidence(existing, incoming);
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
/// Only fingerprinted items qualify: a detector always stamps a fingerprint,
/// so a fingerprint-less item is a user-authored idea card (`create -t idea`)
/// that no detection run produces -- reconciling it away would delete it.
fn is_ephemeral_reconcilable(item: &BacklogItem, fresh_fingerprints: &BTreeSet<String>) -> bool {
    !item.fingerprint.is_empty()
        && item.status == "proposed"
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

    fn card_mode_paths(name: &str) -> (PathBuf, MaestroPaths) {
        let (root, paths) = temp_paths(name);
        fs::create_dir_all(paths.cards_dir()).expect("invariant: cards dir should be creatable");
        (root, paths)
    }

    /// A whole-store save folds the items into the `ideas.yaml` container file
    /// and drops the entry for an item the caller removed (the merge's D4
    /// reconciliation surfaces here).
    #[test]
    fn card_mode_save_moves_items_to_cards_and_drops_removed() {
        let (_root, paths) = card_mode_paths("backlog-card-roundtrip");
        let mut config = BacklogConfig::empty();
        config.items = vec![item("hb-001", "first"), item("hb-002", "second")];
        backlog::save(&paths, &config).expect("save two items as cards");

        let reloaded = backlog::load(&paths).expect("reload from cards");
        assert_eq!(reloaded.items.len(), 2, "items read back from the cards");
        let ideas_file = paths.cards_dir().join("ideas.yaml");
        let raw = fs::read_to_string(&ideas_file).expect("ideas.yaml holds the saved entries");
        assert!(raw.contains("hb-001") && raw.contains("hb-002"), "{raw}");
        assert!(
            !paths.cards_dir().join("hb-001").exists(),
            "a fresh save mints no flat dirs"
        );

        let mut snapshot = backlog::load_with_snapshot(&paths).expect("load snapshot");
        snapshot.backlog.items.retain(|item| item.id == "hb-001");
        backlog::save_with_snapshot(&paths, &snapshot.backlog, &snapshot).expect("save with drop");

        let raw = fs::read_to_string(&ideas_file).expect("ideas.yaml survives the drop");
        assert!(
            raw.contains("hb-001") && !raw.contains("hb-002"),
            "the dropped item's entry is removed: {raw}"
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
    /// mint the same id. The whole-file `ideas.yaml` CAS catches this without a
    /// bespoke guard -- the first writer's save changes the file, so the second
    /// writer's snapshot no longer matches and its create is rejected.
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
            format!("{error:#}").contains("changed since it was read"),
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

    /// D4 reconciliation only drops fingerprinted detector items. A
    /// fingerprint-less `proposed` item is a user-authored idea card (`create
    /// -t idea` yields no fingerprint and no provenance) that no detection run
    /// re-produces; a refresh must not delete it.
    #[test]
    fn merge_proposals_keeps_fingerprintless_user_ideas_on_reconcile() {
        let (_root, paths) = card_mode_paths("backlog-reconcile-user-idea");
        let mut config = BacklogConfig::empty();

        let mut user_idea = item("card-user1", "user idea");
        user_idea.fingerprint = String::new();
        user_idea.provenance = String::new();
        let mut stale_detected = item("card-det1", "stale detection");
        stale_detected.provenance = "detector".to_string();
        config.items = vec![user_idea, stale_detected];

        backlog::merge_proposals(
            &paths,
            &mut config,
            vec![item("card-new1", "fresh detection")],
        )
        .expect("merge fresh proposals");

        let ids: Vec<&str> = config.items.iter().map(|i| i.id.as_str()).collect();
        assert!(
            ids.contains(&"card-user1"),
            "the fingerprint-less user idea survives: {ids:?}"
        );
        assert!(
            !ids.contains(&"card-det1"),
            "the absent fingerprinted detector item is reconciled away: {ids:?}"
        );
    }
}
