//! Slice 3d end-to-end: after migration the decision verbs read and write the
//! card store. The critical case is `lock` with `supersedes` crossing the
//! global<->feature boundary -- the superseding decision and its target lived in
//! DIFFERENT legacy stores before migration, so this proves the card-mode lock
//! resolves and mutates both cards (not just same-store records) while the legacy
//! YAML stores stay frozen (SPEC-beads-model P1 dual-read cutover).

mod support;

use maestro::domain::card::schema::{Card, CardType};
use maestro::domain::card::store::card_path;
use maestro::domain::card::{StoreMode, store_mode};
use maestro::domain::decisions::schema::{DecisionRecord, DecisionStatus};
use maestro::domain::decisions::{self, DecisionContent, DecisionSource};
use maestro::foundation::core::paths::MaestroPaths;
use maestro::operations::card_migrate;
use support::TestTempDir;

const NOW: &str = "2026-06-08T12:00:00Z";

/// The persisted card, parsed straight off disk.
fn card(paths: &MaestroPaths, id: &str) -> Card {
    let contents = std::fs::read_to_string(card_path(paths, id)).expect("card.yaml readable");
    serde_yaml::from_str(&contents).expect("card.yaml parses")
}

/// The reconstructed decision record the read verbs return.
fn structured(paths: &MaestroPaths, id: &str) -> DecisionRecord {
    match decisions::show(paths, id).expect("show decision") {
        DecisionContent::Structured { record, .. } => *record,
        other => panic!("expected a structured decision, got {other:?}"),
    }
}

/// The status string a legacy YAML store still shows for a decision, read off
/// disk to prove migration left it frozen.
fn legacy_store_status(store_path: &std::path::Path, id: &str) -> String {
    let contents = std::fs::read_to_string(store_path).expect("legacy decision store readable");
    let value: serde_yaml::Value = serde_yaml::from_str(&contents).expect("store parses");
    value["decisions"]
        .as_sequence()
        .expect("decisions is a sequence")
        .iter()
        .find(|record| record["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("{id} present in {}", store_path.display()))["status"]
        .as_str()
        .expect("status is a string")
        .to_string()
}

#[test]
fn lock_supersedes_across_the_global_feature_boundary_after_migration() {
    let temp = TestTempDir::new("decision-card-cutover");
    let paths = MaestroPaths::new(temp.path());

    // Author the records through the real verbs while the repo is still legacy,
    // so the migrated cards carry faithful records. The target (decision-001)
    // lands in the GLOBAL store; the superseding decision (decision-002) lands in
    // a PER-FEATURE store -- two different files before migration.
    let feature_id =
        maestro::domain::feature::create(&paths, "Csv export").expect("create feature");
    assert_eq!(feature_id, "csv-export");

    let global = decisions::create_open(&paths, "Use fire-and-forget hooks", None, None)
        .expect("create global decision");
    assert_eq!(global.record.id, "decision-001");
    assert_eq!(global.source, DecisionSource::Global);

    let feature = decisions::create_open(
        &paths,
        "Use a replay queue for hooks",
        Some("hooks dropped events under load"),
        Some(&feature_id),
    )
    .expect("create feature decision");
    assert_eq!(feature.record.id, "decision-002");
    assert_eq!(
        feature.source,
        DecisionSource::Feature {
            feature_id: feature_id.clone()
        }
    );

    let global_store = paths.decisions_file();
    let feature_store = paths
        .features_dir()
        .join(&feature_id)
        .join("decisions.yaml");
    assert_eq!(
        store_mode(&paths),
        StoreMode::Legacy,
        "no cards/ yet -> legacy"
    );

    // Fold the legacy trees into cards/. Non-destructive: both YAML stores stay
    // in place, frozen at `open`.
    card_migrate::run(&paths, NOW).expect("migration succeeds");
    assert_eq!(store_mode(&paths), StoreMode::Cards, "cards/ -> card store");
    assert_eq!(
        card(&paths, "decision-001").parent,
        None,
        "global -> no parent"
    );
    assert_eq!(
        card(&paths, "decision-002").parent.as_deref(),
        Some(feature_id.as_str()),
        "per-feature -> parent recovered"
    );

    // Lock the FEATURE decision, superseding the GLOBAL one. The card-mode lock
    // must load decision-002's card, validate the cross-boundary target against
    // the union (cards + legacy markdown), write the locked card, and flip the
    // superseded target's card -- all without a store-file search.
    let report = decisions::lock(
        &paths,
        "decision-002",
        "buffer to a replay queue",
        &["fire-and-forget".to_string()],
        Some("queue depth gauge"),
        &["decision-001".to_string()],
    )
    .expect("lock crosses the boundary");
    assert_eq!(report.path, card_path(&paths, "decision-002"));
    assert_eq!(
        report.source,
        DecisionSource::Feature {
            feature_id: feature_id.clone()
        }
    );
    assert!(
        report.note_line.as_deref().is_some_and(
            |line| line.contains("decision-002 locked -- Use a replay queue for hooks")
        ),
        "the lock notes the owning feature: {:?}",
        report.note_line
    );

    // The superseding decision's card is locked with its decision text, preview,
    // supersedes target, and lock timestamp.
    let locked = structured(&paths, "decision-002");
    assert_eq!(locked.status, DecisionStatus::Locked);
    assert_eq!(locked.decision.as_deref(), Some("buffer to a replay queue"));
    assert_eq!(locked.preview.as_deref(), Some("queue depth gauge"));
    assert_eq!(locked.supersedes, vec!["decision-001".to_string()]);
    assert!(locked.locked_at.is_some(), "locked_at stamped");
    assert_eq!(card(&paths, "decision-002").status, "locked");

    // The cross-store target's card is flipped to superseded -- the proof the
    // card-mode mark_superseded reached a card that began in a different store.
    let target = structured(&paths, "decision-001");
    assert_eq!(target.status, DecisionStatus::Superseded);
    assert_eq!(target.superseded_by.as_deref(), Some("decision-002"));
    assert_eq!(card(&paths, "decision-001").status, "superseded");

    // Both legacy YAML stores stay frozen at `open` -> the cards, not the YAML,
    // are authoritative.
    assert_eq!(
        legacy_store_status(&global_store, "decision-001"),
        "open",
        "global store frozen across the supersede write"
    );
    assert_eq!(
        legacy_store_status(&feature_store, "decision-002"),
        "open",
        "feature store frozen across the lock write"
    );

    // The owning feature's notes.md (still the legacy feature dir in card mode)
    // carries the lock note.
    let notes = std::fs::read_to_string(paths.features_dir().join(&feature_id).join("notes.md"))
        .expect("feature notes.md written");
    assert!(
        notes.contains("decision-002 locked"),
        "feature note recorded: {notes}"
    );

    // create_open in card mode mints the next id off the card store + legacy
    // markdown (here decision-003), exercises feature::ensure_exists, and writes a
    // Decision card with the parent recovered from the feature home.
    let third = decisions::create_open(&paths, "Stream rows lazily", None, Some(&feature_id))
        .expect("create a decision in card mode");
    assert_eq!(third.record.id, "decision-003");
    assert_eq!(
        third.source,
        DecisionSource::Feature {
            feature_id: feature_id.clone()
        }
    );
    let third_card = card(&paths, "decision-003");
    assert_eq!(third_card.card_type, CardType::Decision);
    assert_eq!(third_card.parent.as_deref(), Some(feature_id.as_str()));

    // Read verbs see the card store: the feature's decisions list the two
    // per-feature cards, and the global list spans all three.
    let for_feature =
        decisions::decisions_for_feature(&paths, &feature_id).expect("decisions for feature");
    let feature_ids: Vec<&str> = for_feature
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    assert_eq!(feature_ids, vec!["decision-002", "decision-003"]);

    let all = decisions::list(&paths).expect("list decisions");
    let listed: Vec<&str> = all.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(listed, vec!["decision-001", "decision-002", "decision-003"]);
}
