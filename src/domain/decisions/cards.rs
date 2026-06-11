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
//! The envelope carries shared fields (`id`, `title`, `status`, parent feature,
//! context, timestamps); `card.extra` carries only the decision-specific payload.
//! The home (global vs per-feature) is read back from `card.parent`, which the
//! migration set from the source `feature` field (falling back to the per-feature
//! store dir). `DecisionRecord` carries no schema version of its own (the version
//! lives on the enclosing `DecisionStore`), so the card-store load already
//! validates the only version present.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::fold;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardHome, ResolvedCard};
use crate::domain::decisions::query::DecisionSource;
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// Reconstruct a [`DecisionRecord`] from a decision card's slim `extra` payload
/// plus the envelope fields it omits. No per-record schema check: the version
/// lives on `DecisionStore`, not the record, and the card-store load already
/// validated `card.schema_version`.
pub(crate) fn record_from_card(card: Card, artifact: String) -> Result<DecisionRecord> {
    // A card minted natively by the card model (DN9 `maestro create`) carries no
    // `extra`, so the slim-payload read below has nothing to parse. Synthesize
    // the record from the card's own fields instead, so `status`/`doctor` can read
    // a canonically-created decision card without crashing. This bridge retires in
    // S4 (E7), when the decision lifecycle moves onto the native fields.
    if card.extra.is_empty() {
        return Ok(record_from_native_card(card));
    }
    let Card {
        id,
        title,
        status,
        parent,
        description,
        created_at,
        extra,
        ..
    } = card;
    let mut extra = extra;
    fold::seed_string_if_absent(&mut extra, "id", &id);
    fold::seed_string_if_absent(&mut extra, "title", &title);
    let record_status = decision_status_from_word(&status).unwrap_or(DecisionStatus::Open);
    fold::seed_string_if_absent(&mut extra, "status", record_status.as_str());
    fold::seed_optional_string_if_absent(&mut extra, "feature", parent.as_deref());
    fold::seed_optional_string_if_absent(&mut extra, "context", description.as_deref());
    fold::seed_string_if_absent(&mut extra, "created_at", &created_at);
    let mut record: DecisionRecord = serde_yaml::from_value(Value::Mapping(extra))
        .with_context(|| format!("failed to parse {artifact}"))?;
    // The card verbs (`update`) write only the top-level copy fields, so they
    // are the freshest source for what they own (SPEC DN3: the card status is
    // the single source of truth). The overlay is conservative: an unrecognized
    // status word and an absent description keep the record's own. Identity is
    // never the payload's to override: a hand-edited `extra.id`/`extra.feature`
    // would otherwise route later saves at a different logical record.
    record.id = id;
    record.feature = parent;
    record.title = title;
    if let Some(mapped) = decision_status_from_word(&status) {
        record.status = mapped;
    }
    if description.is_some() {
        record.context = description;
    }
    Ok(record)
}

/// Map a card status word to the decision status it denotes. `closed` -- the
/// DN3b uniform terminal word a pre-guard `card close` may have written -- folds
/// onto `locked`: a settled decision must not silently read as open. An unknown
/// word maps to `None` so callers keep a better source.
fn decision_status_from_word(status: &str) -> Option<DecisionStatus> {
    Some(match status {
        "open" => DecisionStatus::Open,
        "locked" | "closed" => DecisionStatus::Locked,
        "superseded" => DecisionStatus::Superseded,
        _ => return None,
    })
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
        status: decision_status_from_word(&card.status).unwrap_or(DecisionStatus::Open),
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

/// Serialize a decision record to the mapping the card builder folds into the
/// envelope plus slim `extra`.
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

/// Load one decision for a read-modify-write: `Some((record, source,
/// resolved))` when a `Decision`-typed card exists for `id` -- in any home the
/// resolver covers (a container `decisions.yaml` entry or a pre-migration
/// flat dir) -- else `None`. The resolved card is the CAS basis the matching
/// save checks (SPEC D1).
pub(crate) fn load_one(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(DecisionRecord, DecisionSource, ResolvedCard)>> {
    let Some(resolved) = card_store::resolve(paths, id)? else {
        return Ok(None);
    };
    if resolved.card.card_type != CardType::Decision {
        return Ok(None);
    }
    let source = source_from_parent(resolved.card.parent.as_deref());
    let record = record_from_card(resolved.card.clone(), resolved.path().display().to_string())?;
    Ok(Some((record, source, resolved)))
}

/// Persist a decision record back to the home it was resolved from (the
/// card-store CAS rejects a racing writer, SPEC D1).
pub(crate) fn save(record: &DecisionRecord, resolved: &ResolvedCard) -> Result<()> {
    let card = card_for(record)?;
    card_store::save_folded_resolved(card, resolved)
}

/// Reconstruct every live `Decision`-typed card with its home and backing
/// path (the container file for an entry-backed decision), sorted by id.
/// `tolerant` skips a card that fails to load or parse (the strict callers
/// surface the first such error), mirroring `scan_feature_cards`.
pub(crate) fn scan(
    paths: &MaestroPaths,
    tolerant: bool,
) -> Result<Vec<(DecisionRecord, DecisionSource, PathBuf)>> {
    let cards = if tolerant {
        crate::domain::card::query::scan_with_failures(paths)?.cards
    } else {
        crate::domain::card::query::scan_with_paths(paths)?
    };
    let mut decisions = Vec::new();
    for (card, path) in cards {
        if card.card_type != CardType::Decision {
            continue;
        }
        let source = source_from_parent(card.parent.as_deref());
        match record_from_card(card, path.display().to_string()) {
            Ok(record) => decisions.push((record, source, path)),
            Err(error) if tolerant => {
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
    decisions.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    Ok(decisions)
}

/// [`scan`] from an already-loaded card set, so the card-aware doctor walks the
/// store once for every check. Envelope failures never reach this (the shared
/// scan's caller owns them); `tolerant` only governs conversion errors on
/// `Decision`-typed cards.
pub(crate) fn records_in_cards(
    cards: &[(Card, PathBuf)],
    tolerant: bool,
) -> Result<Vec<(DecisionRecord, DecisionSource, PathBuf)>> {
    let mut decisions = Vec::new();
    for (card, path) in cards {
        if card.card_type != CardType::Decision {
            continue;
        }
        let source = source_from_parent(card.parent.as_deref());
        match record_from_card(card.clone(), path.display().to_string()) {
            Ok(record) => decisions.push((record, source, path.clone())),
            Err(_) if tolerant => {}
            Err(error) => return Err(error),
        }
    }
    Ok(decisions)
}

/// Create a new decision card from a record, landing in the container home its
/// parent dictates (a feature's `decisions.yaml` or the global one). The write
/// is a CAS create, so a concurrent create of the same id is rejected. Returns
/// the home so callers can report the landing path.
pub(crate) fn create(paths: &MaestroPaths, record: &DecisionRecord) -> Result<CardHome> {
    let card = card_for(record)?;
    card_store::create_card(paths, &card)
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

    /// Fidelity: a record folded into a slim card and read back is identical,
    /// and the home derives from `card.parent`.
    #[test]
    fn record_round_trips_through_the_card() {
        let record = feature_decision();
        let card = card_for(&record).expect("fold record into card");

        assert_eq!(card.card_type, CardType::Decision);
        assert_eq!(card.id, "decision-002");
        assert_eq!(card.parent.as_deref(), Some("csv-export"));
        assert_eq!(card.status, "locked", "status derives from the record");
        for key in ["id", "title", "status", "feature", "context", "created_at"] {
            assert!(
                !card
                    .extra
                    .contains_key(serde_yaml::Value::String(key.to_string())),
                "extra omits envelope-owned {key}"
            );
        }
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
            "every field survives the round-trip through the slim card"
        );
    }

    /// The card verbs write only the top-level copy; the typed read treats it
    /// as the freshest source (SPEC DN3). A pre-guard `card close` wrote the
    /// uniform terminal word on decision cards -- it folds onto locked so a
    /// settled decision never silently reads as open.
    #[test]
    fn typed_read_overlays_card_verb_writes() {
        let mut record = feature_decision();
        record.status = DecisionStatus::Open;
        let mut card = card_for(&record).expect("fold record into card");
        card.status = "closed".to_string();
        card.title = "retitled".to_string();
        card.description = Some("fresh context".to_string());

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the record");
        assert_eq!(reconstructed.status, DecisionStatus::Locked);
        assert_eq!(reconstructed.title, "retitled");
        assert_eq!(reconstructed.context.as_deref(), Some("fresh context"));
    }

    /// Identity is the envelope's: a hand-edited `extra.id`/`extra.feature`
    /// must not route the typed view (and any save keyed off it) at a
    /// different logical record than the card the resolver actually loaded.
    #[test]
    fn typed_read_keeps_the_envelope_identity() {
        let record = feature_decision();
        let mut card = card_for(&record).expect("fold record into card");
        card.extra.insert(
            serde_yaml::Value::String("id".to_string()),
            serde_yaml::Value::String("decision-999".to_string()),
        );
        card.extra.insert(
            serde_yaml::Value::String("feature".to_string()),
            serde_yaml::Value::String("other-feature".to_string()),
        );

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the record");
        assert_eq!(reconstructed.id, "decision-002");
        assert_eq!(reconstructed.feature.as_deref(), Some("csv-export"));
    }

    /// SPEC D1 in card mode: two readers each take a load-time snapshot; the
    /// first save wins, the second is rejected because the card store's
    /// raw-string CAS checks the snapshot read at load time, not a fresh one.
    #[test]
    fn card_mode_save_rejects_a_stale_decision_writer() {
        let paths = card_mode_repo("stale-writer");
        // A global decision (no feature parent) so the create needs no feature
        // container; it lands as an entry in the root decisions.yaml.
        let mut record = feature_decision();
        record.feature = None;
        let home = create(&paths, &record).expect("create the decision card");
        assert!(
            home.path().ends_with("decisions.yaml"),
            "a global decision lands in the root decisions.yaml: {}",
            home.path().display()
        );

        let (mut winner, _, winner_resolved) = load_one(&paths, "decision-002")
            .expect("first read")
            .expect("card exists");
        let (mut loser, _, loser_resolved) = load_one(&paths, "decision-002")
            .expect("second read")
            .expect("card exists");

        winner.title = "winner".to_string();
        save(&winner, &winner_resolved).expect("first writer commits");

        loser.title = "stale writer".to_string();
        let error = save(&loser, &loser_resolved).expect_err("the stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }
}
