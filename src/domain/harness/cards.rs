//! Harness-backlog <-> card glue.
//!
//! Unlike feature/task/decision -- per-record stores whose verbs each touch one
//! record -- the harness backlog is an AGGREGATE: its verbs load a whole
//! `BacklogConfig`, run an item-retaining merge, and save it back. Each item is
//! an `idea`-typed card: an entry in `.maestro/cards/ideas.yaml` (the
//! container-layout home), or a pre-migration flat
//! `.maestro/cards/<id>/card.yaml` straggler dir. The card store is the only
//! store (D7), with no metadata file beside it. The detect-skip evidence stamp
//! lives in `.maestro/harness/detect-stamp` (a cache, not a store).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::fold;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardSnapshot};
use crate::domain::harness::schema::BacklogItem;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// Reconstruct a [`BacklogItem`] from an idea card's slim `extra` payload plus
/// the envelope fields it omits. No per-item schema check: the version lives on
/// `BacklogConfig`, not the item, and the card-store load already validated
/// `card.schema_version`.
pub(crate) fn item_from_card(card: Card, artifact: &str) -> Result<BacklogItem> {
    // A card minted natively by the card model (DN9 `maestro create -t idea`)
    // carries no `extra`, so the slim-payload read below has nothing to parse.
    // Synthesize a minimal item from the card's own fields so `harness list` can
    // read it without crashing. The detector metadata a migrated item carries
    // (fingerprint, occurrences, evidence, history) has no native home yet (the S4
    // gap), so it stays empty; the aggregate merge is fingerprint-keyed, so an
    // empty-fingerprint native idea is the "non-harness idea source" D7/P5 owns.
    if card.extra.is_empty() {
        return Ok(item_from_native_card(card));
    }
    let Card {
        id,
        title,
        status,
        extra,
        ..
    } = card;
    let mut extra = extra;
    fold::seed_string_if_absent(&mut extra, "id", &id);
    fold::seed_string_if_absent(&mut extra, "title", &title);
    fold::seed_string_if_absent(&mut extra, "status", &status);
    let mut item: BacklogItem = serde_yaml::from_value(Value::Mapping(extra))
        .with_context(|| format!("failed to parse {artifact}"))?;
    // The card verbs (`update`) write only the top-level copy fields, so they
    // are the freshest source for the title and status they own (SPEC DN3).
    // Identity is the envelope's, never the payload's: a divergent `extra.id`
    // would desync the item from the card home later saves resolve by id.
    item.id = id;
    item.title = title;
    if !status.is_empty() {
        item.status = status;
    }
    Ok(item)
}

/// Build a [`BacklogItem`] from a native idea card's own fields (no `extra`
/// carrier). Only `id`/`title` are required; every detector field is
/// skip-if-empty, so a minimal item round-trips and reads cleanly.
fn item_from_native_card(card: Card) -> BacklogItem {
    BacklogItem {
        id: card.id,
        title: card.title,
        status: card.status,
        first_seen: card.created_at,
        last_seen: card.updated_at,
        fingerprint: String::new(),
        source: String::new(),
        provenance: String::new(),
        topic: String::new(),
        item_type: String::new(),
        priority: String::new(),
        occurrences: 0,
        sessions_hit: Vec::new(),
        evidence: Vec::new(),
        spawned_task: None,
        dismissal_reason: None,
        history: Vec::new(),
    }
}

/// Serialize a backlog item to the mapping the card builder folds into the
/// envelope plus slim `extra`.
fn item_to_mapping(item: &BacklogItem) -> Result<Mapping> {
    match serde_yaml::to_value(item).context("failed to serialize backlog item")? {
        Value::Mapping(map) => Ok(map),
        _ => bail!("backlog item did not serialize to a mapping"),
    }
}

/// Fold a backlog item into its idea card.
pub(crate) fn card_for(item: &BacklogItem) -> Result<Card> {
    Ok(fold::idea_card(
        item.id.clone(),
        item_to_mapping(item)?,
        &utc_now_timestamp(),
    ))
}

/// E7's `idea` arm, dispatched from [`CardType::reconcile`]: merge a
/// fingerprint-matched re-detection into the existing card by reconstructing
/// both items, running the pair-level backlog merge, and folding the result
/// back. The card round-trip is lossless (see `item_round_trips_through_the_card`).
pub(crate) fn reconcile_idea(existing: Card, incoming: Card) -> Result<Card> {
    let mut existing_item = item_from_card(existing, "existing idea card")?;
    let incoming_item = item_from_card(incoming, "incoming idea card")?;
    crate::domain::harness::backlog::reconcile_item(&mut existing_item, &incoming_item);
    card_for(&existing_item)
}

/// Reconstruct every PRE-MIGRATION flat-dir `Idea` card with its load-time CAS
/// snapshot and card path, sorted by id; entry-backed ideas live in
/// `ideas.yaml` and are read by `backlog::load_with_snapshot` alongside this.
/// The first card that fails to load or parse surfaces its error: an aggregate
/// save must see the whole backlog, so a partial scan would silently drop
/// items.
pub(crate) fn scan(paths: &MaestroPaths) -> Result<Vec<(BacklogItem, CardSnapshot, PathBuf)>> {
    let mut items = Vec::new();
    for id in card_store::card_dir_ids(&paths.cards_dir())? {
        let path = card_store::card_path(paths, &id);
        let snapshot = card_store::load_with_snapshot(&path)?;
        let Some(card) = snapshot.card.clone() else {
            continue;
        };
        if card.card_type != CardType::Idea {
            continue;
        }
        let item = item_from_card(card, &path.display().to_string())?;
        items.push((item, snapshot, path));
    }
    Ok(items)
}

/// Persist a backlog item to an explicit card path against its load-time snapshot
/// (the card-store CAS rejects a racing writer, SPEC D1). An item minted by the
/// merge has no card yet, so its snapshot is the absent one `scan` never returned
/// -- pass [`card_store::load_with_snapshot`] of its path, which a concurrent
/// create of the same id will then fail.
pub(crate) fn save_at(path: &Path, item: &BacklogItem, snapshot: &CardSnapshot) -> Result<()> {
    let card = card_for(item)?;
    card_store::save_folded_with_snapshot(path, card, snapshot)
}

/// Remove the card for a backlog item dropped by the merge (D4 ephemeral
/// reconciliation). Absence is fine: a prior partial save may have removed it.
pub(crate) fn remove(paths: &MaestroPaths, id: &str) -> Result<()> {
    let dir = paths.cards_dir().join(id);
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", dir.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::card::store::card_path;
    use crate::foundation::core::fs::ensure_dir;

    fn card_mode_repo(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-harness-cards-{label}-{}-{nanos}",
            process::id()
        ));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    fn item() -> BacklogItem {
        // A fully-populated item: the optional/list/skip-if-empty fields exercise
        // serde's round-trip so a dropped one is caught.
        BacklogItem {
            id: "hb-001".to_string(),
            fingerprint: "missing_verification:cargo test".to_string(),
            source: "task-002".to_string(),
            provenance: "detector".to_string(),
            topic: "verification".to_string(),
            item_type: "missing_verification".to_string(),
            title: "task-002 verified outside harness.yml".to_string(),
            priority: "high".to_string(),
            occurrences: 3,
            sessions_hit: vec!["task-002".to_string(), "task-005".to_string()],
            first_seen: "2026-06-08T00:00:00Z".to_string(),
            last_seen: "2026-06-09T00:00:00Z".to_string(),
            status: "proposed".to_string(),
            evidence: vec!["task-002 used verification command 1 outside harness.yml".to_string()],
            spawned_task: None,
            dismissal_reason: None,
            history: Vec::new(),
        }
    }

    /// Fidelity: an item folded into a slim card and read back is identical.
    #[test]
    fn item_round_trips_through_the_card() {
        let item = item();
        let card = card_for(&item).expect("fold item into card");

        assert_eq!(card.card_type, CardType::Idea);
        assert_eq!(card.id, "hb-001");
        assert_eq!(card.parent, None, "harness ideas are global, never docked");
        assert_eq!(card.status, "proposed", "status derives from the item");
        for key in ["id", "title", "status"] {
            assert!(
                !card
                    .extra
                    .contains_key(serde_yaml::Value::String(key.to_string())),
                "extra omits envelope-owned {key}"
            );
        }

        let reconstructed = item_from_card(card, "test").expect("reconstruct the item");
        assert_eq!(
            reconstructed, item,
            "every field survives the round-trip through the slim card"
        );
    }

    /// SPEC E7 through the card-level hook: a fingerprint-matched re-detection
    /// merges into the existing idea card -- recurrence and evidence refresh from
    /// the incoming detection while identity, status, and the spawned-task link
    /// stay with the existing item.
    #[test]
    fn reconcile_merges_a_re_detection_into_the_existing_idea_card() {
        let mut existing = item();
        existing.item_type = "explicit_intervention".to_string();
        existing.status = "accepted".to_string();
        existing.spawned_task = Some("card-abc123".to_string());

        let mut incoming = item();
        incoming.item_type = "explicit_intervention".to_string();
        incoming.title = "re-detected title".to_string();
        incoming.last_seen = "2026-06-10T00:00:00Z".to_string();
        incoming.occurrences = 5;
        incoming.sessions_hit.push("task-009".to_string());
        incoming.evidence.push("a fresh correction".to_string());

        let merged = CardType::Idea
            .reconcile(
                card_for(&existing).expect("fold the existing item"),
                card_for(&incoming).expect("fold the incoming item"),
            )
            .expect("reconcile the idea cards");
        let merged = item_from_card(merged, "merged idea card").expect("reconstruct the item");

        assert_eq!(
            merged.id, existing.id,
            "identity stays with the existing card"
        );
        assert_eq!(merged.title, existing.title, "the existing title is kept");
        assert_eq!(
            merged.status, "accepted",
            "a non-regressed status is preserved"
        );
        assert_eq!(merged.spawned_task.as_deref(), Some("card-abc123"));
        assert_eq!(merged.last_seen, "2026-06-10T00:00:00Z");
        assert_eq!(merged.occurrences, 5);
        assert_eq!(
            merged.sessions_hit,
            vec!["task-002", "task-005", "task-009"],
            "recurrence refreshes from the fresh detection"
        );
        assert!(
            merged.evidence.contains(&"a fresh correction".to_string()),
            "new evidence accumulates: {:?}",
            merged.evidence
        );
        assert!(
            merged.evidence.contains(&existing.evidence[0]),
            "existing evidence survives: {:?}",
            merged.evidence
        );
    }

    /// D6 through the hook: a re-detected `measured` state-detector note reopens
    /// to `proposed`, logs the regression, and drops the spawned-task link so the
    /// next accept spawns a fresh task.
    #[test]
    fn reconcile_reopens_a_regressed_measured_state_note() {
        let mut existing = item();
        existing.status = "measured".to_string();
        existing.spawned_task = Some("card-abc123".to_string());

        let merged = CardType::Idea
            .reconcile(
                card_for(&existing).expect("fold the existing item"),
                card_for(&item()).expect("fold the incoming item"),
            )
            .expect("reconcile the idea cards");
        let merged = item_from_card(merged, "merged idea card").expect("reconstruct the item");

        assert_eq!(merged.status, "proposed", "the regressed note reopens");
        assert_eq!(merged.spawned_task, None);
        assert_eq!(merged.history.len(), 1, "{:?}", merged.history);
        assert_eq!(merged.history[0].result, "regressed");
        assert_eq!(merged.history[0].task.as_deref(), Some("card-abc123"));
    }

    /// SPEC D1 in card mode: two readers each take a load-time snapshot; the first
    /// save wins, the second is rejected because the card store's raw-string CAS
    /// checks the snapshot read at load time, not a fresh one.
    #[test]
    fn card_mode_save_rejects_a_stale_item_writer() {
        let paths = card_mode_repo("stale-writer");
        let path = card_path(&paths, "hb-001");
        let fresh = card_store::load_with_snapshot(&path).expect("absent snapshot");
        save_at(&path, &item(), &fresh).expect("create the idea card");

        let winner_snapshot = card_store::load_with_snapshot(&path).expect("first read");
        let loser_snapshot = card_store::load_with_snapshot(&path).expect("second read");

        let mut winner = item();
        winner.title = "winner".to_string();
        save_at(&path, &winner, &winner_snapshot).expect("first writer commits");

        let mut loser = item();
        loser.title = "stale writer".to_string();
        let error =
            save_at(&path, &loser, &loser_snapshot).expect_err("the stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }
}
