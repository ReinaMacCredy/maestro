//! Decision <-> card glue for the SPEC-beads-model P1 dual-read cutover.
//!
//! A migrated repo routes the structured decision stores (global
//! `.maestro/decisions.yaml` plus per-feature `features/<id>/decisions.yaml`)
//! through the flat `.maestro/cards/<id>/card.yaml` store: one card per decision
//! with per-card CAS, replacing the whole-file store CAS. The frozen legacy
//! markdown under `.maestro/decisions/` is orthogonal -- the migration never
//! folds it -- so every read keeps its `decision_entries` markdown loop and only
//! swaps the YAML-store loop for a `Decision`-typed card scan.
//!
//! Unlike a task, a `DecisionRecord` serializes faithfully and its `feature` is a
//! real field, so `card.extra = to_value(record)` round-trips with no field
//! recovery; the home (global vs per-feature) is read back from `card.parent`,
//! which the migration set from the source `feature` field (falling back to the
//! per-feature store dir). `DecisionRecord` carries no schema version of its own
//! (the version lives on the enclosing `DecisionStore`), so the card-store load
//! already validates the only version present.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::fold;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardSnapshot};
use crate::domain::decisions::query::DecisionSource;
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// Reconstruct a [`DecisionRecord`] from a decision card's verbatim source
/// mapping (`extra`, the COPY-design payload). No per-record schema check: the
/// version lives on `DecisionStore`, not the record, and the card-store load
/// already validated `card.schema_version`.
pub(crate) fn record_from_card(card: Card, artifact: String) -> Result<DecisionRecord> {
    // A card minted natively by the card model (DN9 `maestro create`) carries no
    // `extra`, so the verbatim-mapping read below has nothing to parse. Synthesize
    // the record from the card's own fields instead, so `status`/`doctor` can read
    // a canonically-created decision card without crashing. This bridge retires in
    // S4 (E7), when the decision lifecycle moves onto the native fields.
    if card.extra.is_empty() {
        return Ok(record_from_native_card(card));
    }
    serde_yaml::from_value(Value::Mapping(card.extra))
        .with_context(|| format!("failed to parse {artifact}"))
}

/// Build a [`DecisionRecord`] from a native card's own fields (no `extra`
/// carrier). The fork/lock prose (`decision`, `rejected`, `preview`) and the
/// supersession edges a migrated decision carries have no native home yet (the
/// S4 gap), so the record keeps `None`/empty for those; the home rides
/// `card.parent` and the status maps from the card's status word.
fn record_from_native_card(card: Card) -> DecisionRecord {
    DecisionRecord {
        id: card.id,
        title: card.title,
        status: match card.status.as_str() {
            "locked" => DecisionStatus::Locked,
            "superseded" => DecisionStatus::Superseded,
            _ => DecisionStatus::Open,
        },
        feature: card.parent,
        context: card.description,
        decision: None,
        rejected: Vec::new(),
        preview: None,
        supersedes: Vec::new(),
        superseded_by: None,
        created_at: card.created_at,
        locked_at: None,
    }
}

/// Serialize a decision record to the mapping the card builder folds into
/// `extra`. Feeding the same mapping the migration reads off `decisions.yaml`
/// keeps a saved card byte-identical to a migrated one.
fn record_to_mapping(record: &DecisionRecord) -> Result<Mapping> {
    match serde_yaml::to_value(record).context("failed to serialize decision record")? {
        Value::Mapping(map) => Ok(map),
        _ => bail!("decision record did not serialize to a mapping"),
    }
}

/// Fold a decision record into its card. `feature` rides the source mapping, so
/// the explicit `feature_parent` arg is only a fallback; passing the record's own
/// `feature` keeps live-save and migration in step.
fn card_for(record: &DecisionRecord) -> Result<Card> {
    Ok(fold::decision_card(
        record.id.clone(),
        record_to_mapping(record)?,
        record.feature.clone(),
        &utc_now_timestamp(),
    ))
}

/// The decision's home in card mode, recovered from `card.parent`: per-feature
/// when set, else the global store. Read from the parent rather than the
/// record's `feature` field so a hand-edited per-feature record whose `feature`
/// was absent still reads as `Feature` -- matching what the migration wrote.
pub(crate) fn source_from_parent(parent: Option<&str>) -> DecisionSource {
    match parent {
        Some(feature_id) => DecisionSource::Feature {
            feature_id: feature_id.to_string(),
        },
        None => DecisionSource::Global,
    }
}

/// Load one decision for a read-modify-write: `Some((record, source, snapshot,
/// card path))` when a `Decision`-typed card exists for `id`, else `None`. The
/// snapshot is the CAS basis the matching save checks (SPEC D1).
pub(crate) fn load_one(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(DecisionRecord, DecisionSource, CardSnapshot, PathBuf)>> {
    let path = card_store::card_path(paths, id);
    let snapshot = card_store::load_with_snapshot(&path)?;
    let Some(card) = snapshot.card.clone() else {
        return Ok(None);
    };
    if card.card_type != CardType::Decision {
        return Ok(None);
    }
    let source = source_from_parent(card.parent.as_deref());
    let record = record_from_card(card, path.display().to_string())?;
    Ok(Some((record, source, snapshot, path)))
}

/// Persist a decision record to an explicit card path against its load-time
/// snapshot (the card-store CAS rejects a racing writer, SPEC D1).
pub(crate) fn save_at(path: &Path, record: &DecisionRecord, snapshot: &CardSnapshot) -> Result<()> {
    let card = card_for(record)?;
    card_store::save_with_snapshot(path, &card, snapshot)
}

/// Reconstruct every live `Decision`-typed card with its home and card path,
/// sorted by id. `tolerant` skips a card that fails to load or parse (the strict
/// callers surface the first such error), mirroring `scan_feature_cards`.
pub(crate) fn scan(
    paths: &MaestroPaths,
    tolerant: bool,
) -> Result<Vec<(DecisionRecord, DecisionSource, PathBuf)>> {
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

    let mut decisions = Vec::new();
    for id in ids {
        let path = card_store::card_path(paths, &id);
        match card_store::load(&path) {
            Ok(Some(card)) if card.card_type == CardType::Decision => {
                let source = source_from_parent(card.parent.as_deref());
                match record_from_card(card, path.display().to_string()) {
                    Ok(record) => decisions.push((record, source, path)),
                    Err(error) if tolerant => {
                        let _ = error;
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(_) => {}
            Err(error) if tolerant => {
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(decisions)
}

/// Create a new decision card from a record. The write is a CAS against the
/// absent snapshot, so a concurrent create of the same id is rejected. The id is
/// reserved by the caller.
pub(crate) fn create(paths: &MaestroPaths, record: &DecisionRecord) -> Result<()> {
    let path = card_store::card_path(paths, &record.id);
    let snapshot = card_store::load_with_snapshot(&path)?;
    if snapshot.card.is_some() {
        bail!("decision {} already exists", record.id);
    }
    let card = card_for(record)?;
    card_store::save_with_snapshot(&path, &card, &snapshot)
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::decisions::schema::DecisionStatus;
    use crate::foundation::core::fs::ensure_dir;

    fn card_mode_repo(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-decision-cards-{label}-{}-{nanos}",
            process::id()
        ));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    fn feature_decision() -> DecisionRecord {
        // A per-feature, fully-populated locked decision: the `feature` field is
        // the home recovered from `card.parent`, and the optional/list fields
        // exercise serde's skip-if-empty so the round-trip catches a dropped one.
        DecisionRecord {
            id: "decision-002".to_string(),
            title: "Use a replay queue for hooks".to_string(),
            status: DecisionStatus::Locked,
            feature: Some("csv-export".to_string()),
            context: Some("hooks dropped events under load".to_string()),
            decision: Some("buffer to a replay queue".to_string()),
            rejected: vec!["fire-and-forget".to_string()],
            preview: Some("queue depth gauge".to_string()),
            supersedes: vec!["decision-001".to_string()],
            superseded_by: None,
            created_at: "2026-06-08T00:00:00Z".to_string(),
            locked_at: Some("2026-06-08T01:00:00Z".to_string()),
        }
    }

    /// Fidelity: a record folded into a card and read back is byte-identical,
    /// and the home derives from `card.parent`. This is why a migrated card and a
    /// live-saved card reconstruct the same record.
    #[test]
    fn record_round_trips_through_the_card() {
        let record = feature_decision();
        let card = card_for(&record).expect("fold record into card");

        assert_eq!(card.card_type, CardType::Decision);
        assert_eq!(card.id, "decision-002");
        assert_eq!(card.parent.as_deref(), Some("csv-export"));
        assert_eq!(card.status, "locked", "status derives from the record");
        assert_eq!(
            source_from_parent(card.parent.as_deref()),
            DecisionSource::Feature {
                feature_id: "csv-export".to_string()
            }
        );

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the record");
        assert_eq!(
            reconstructed, record,
            "every field survives the round-trip through card.extra"
        );
    }

    /// SPEC D1 in card mode: two readers each take a load-time snapshot; the
    /// first save wins, the second is rejected because the card store's
    /// raw-string CAS checks the snapshot read at load time, not a fresh one.
    #[test]
    fn card_mode_save_rejects_a_stale_decision_writer() {
        let paths = card_mode_repo("stale-writer");
        create(&paths, &feature_decision()).expect("create the decision card");

        let (mut winner, _, winner_snapshot, winner_path) = load_one(&paths, "decision-002")
            .expect("first read")
            .expect("card exists");
        let (mut loser, _, loser_snapshot, loser_path) = load_one(&paths, "decision-002")
            .expect("second read")
            .expect("card exists");

        winner.title = "winner".to_string();
        save_at(&winner_path, &winner, &winner_snapshot).expect("first writer commits");

        loser.title = "stale writer".to_string();
        let error = save_at(&loser_path, &loser, &loser_snapshot)
            .expect_err("the stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }
}
