//! Harness-backlog <-> card glue for the SPEC-beads-model P1 dual-read cutover.
//!
//! Unlike feature/task/decision -- per-record stores whose verbs each touch one
//! record -- the harness backlog is an AGGREGATE: its verbs load a whole
//! `BacklogConfig`, run an id-reassigning, item-retaining merge, and save it back.
//! In card mode each item folds to an `idea`-typed `.maestro/cards/hb-NNN/card.yaml`
//! (the migration's `fold_ideas` source), while the store-level metadata
//! (`schema_version` + `evidence_stamp`) stays in `.maestro/harness/backlog.yaml`
//! with `items: []`. The split is the sanctioned cost of cutting an aggregate over
//! in P1; D7/P5 collapses it into the one store.
//!
//! P1 assumption: every `idea` card IS a harness backlog item (the migration's
//! only idea source). A non-harness idea source is D7/P5's concern.

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

/// Reconstruct a [`BacklogItem`] from an idea card's verbatim source mapping
/// (`extra`, the COPY-design payload). No per-item schema check: the version lives
/// on `BacklogConfig`, not the item, and the card-store load already validated
/// `card.schema_version`.
pub(crate) fn item_from_card(card: Card, artifact: &str) -> Result<BacklogItem> {
    // A card minted natively by the card model (DN9 `maestro create -t idea`)
    // carries no `extra`, so the verbatim-mapping read below has nothing to parse.
    // Synthesize a minimal item from the card's own fields so `harness list` can
    // read it without crashing. The detector metadata a migrated item carries
    // (fingerprint, occurrences, evidence, history) has no native home yet (the S4
    // gap), so it stays empty; the aggregate merge is fingerprint-keyed, so an
    // empty-fingerprint native idea is the "non-harness idea source" D7/P5 owns.
    if card.extra.is_empty() {
        return Ok(item_from_native_card(card));
    }
    serde_yaml::from_value(Value::Mapping(card.extra))
        .with_context(|| format!("failed to parse {artifact}"))
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

/// Serialize a backlog item to the mapping the card builder folds into `extra`.
/// Feeding the same mapping the migration reads off `backlog.yaml` keeps a saved
/// card byte-identical to a migrated one.
fn item_to_mapping(item: &BacklogItem) -> Result<Mapping> {
    match serde_yaml::to_value(item).context("failed to serialize backlog item")? {
        Value::Mapping(map) => Ok(map),
        _ => bail!("backlog item did not serialize to a mapping"),
    }
}

/// Fold a backlog item into its idea card.
fn card_for(item: &BacklogItem) -> Result<Card> {
    Ok(fold::idea_card(
        item.id.clone(),
        item_to_mapping(item)?,
        &utc_now_timestamp(),
    ))
}

/// Reconstruct every live `Idea`-typed card with its load-time CAS snapshot and
/// card path, sorted by id. The first card that fails to load or parse surfaces
/// its error: an aggregate save must see the whole backlog, so a partial scan
/// would silently drop items.
pub(crate) fn scan(paths: &MaestroPaths) -> Result<Vec<(BacklogItem, CardSnapshot, PathBuf)>> {
    let cards_dir = paths.cards_dir();
    if !cards_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(&cards_dir)
        .with_context(|| format!("failed to read {}", cards_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", cards_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        if !entry.path().join("card.yaml").is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            ids.push(name.to_string());
        }
    }
    ids.sort();

    let mut items = Vec::new();
    for id in ids {
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
    card_store::save_with_snapshot(path, &card, snapshot)
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

    /// Fidelity: an item folded into a card and read back is byte-identical, which
    /// is why a migrated card and a live-saved card reconstruct the same item.
    #[test]
    fn item_round_trips_through_the_card() {
        let item = item();
        let card = card_for(&item).expect("fold item into card");

        assert_eq!(card.card_type, CardType::Idea);
        assert_eq!(card.id, "hb-001");
        assert_eq!(card.parent, None, "harness ideas are global, never docked");
        assert_eq!(card.status, "proposed", "status derives from the item");

        let reconstructed = item_from_card(card, "test").expect("reconstruct the item");
        assert_eq!(
            reconstructed, item,
            "every field survives the round-trip through card.extra"
        );
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
