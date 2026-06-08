//! Slice 3b end-to-end: after migration, the feature record verbs read and
//! write the card store, and the legacy `feature.yaml` is frozen -- proving the
//! card became the source of truth, not merely that the two happen to agree
//! (SPEC-beads-model P1 dual-read cutover).

mod support;

use maestro::domain::card::schema::Card;
use maestro::domain::card::store::card_path;
use maestro::domain::card::{StoreMode, store_mode};
use maestro::domain::feature::{self, ContractEdits, FeatureStatus};
use maestro::foundation::core::paths::MaestroPaths;
use maestro::operations::card_migrate;
use support::TestTempDir;

const NOW: &str = "2026-06-08T12:00:00Z";

/// Read the persisted card and return its real status string.
fn card_status(paths: &MaestroPaths, id: &str) -> String {
    let contents = std::fs::read_to_string(card_path(paths, id)).expect("card.yaml readable");
    let card: Card = serde_yaml::from_str(&contents).expect("card.yaml parses");
    card.status
}

/// Read the legacy feature record's status string straight off disk.
fn legacy_status(paths: &MaestroPaths, id: &str) -> String {
    let path = paths.features_dir().join(id).join("feature.yaml");
    let contents = std::fs::read_to_string(path).expect("legacy feature.yaml readable");
    let value: serde_yaml::Value = serde_yaml::from_str(&contents).expect("feature.yaml parses");
    value["status"]
        .as_str()
        .expect("status is a string")
        .to_string()
}

#[test]
fn feature_verbs_read_and_write_the_card_after_migration() {
    let temp = TestTempDir::new("feature-card-cutover");
    let paths = MaestroPaths::new(temp.path());

    // Author a complete Proposed contract through the real verbs while the repo
    // is still legacy (no cards/ yet), so the migrated card carries a faithful
    // record rather than a hand-crafted one.
    let id = feature::create(&paths, "Csv export").expect("create feature");
    assert_eq!(id, "csv-export");
    feature::set(
        &paths,
        &id,
        ContractEdits {
            acceptance: Some(vec!["exports a header row".to_string()]),
            affected_areas: Some(vec!["cli".to_string()]),
            ..Default::default()
        },
    )
    .expect("author contract");
    assert_eq!(
        store_mode(&paths),
        StoreMode::Legacy,
        "no cards/ yet -> legacy store"
    );

    // Fold the legacy trees into cards/. Migration is non-destructive: the
    // legacy feature.yaml is left in place, frozen at `proposed`.
    card_migrate::run(&paths, NOW).expect("migration succeeds");
    assert_eq!(
        store_mode(&paths),
        StoreMode::Cards,
        "cards/ present -> card store authoritative"
    );
    assert!(card_path(&paths, &id).is_file(), "feature card written");
    assert_eq!(card_status(&paths, &id), "proposed");
    assert_eq!(legacy_status(&paths, &id), "proposed");

    // accept: Proposed -> Ready (a gated verb whose contract gate reads the
    // record reconstructed from the card; qa: none skips the sidecar baseline).
    let report = feature::accept_with_qa_none(&paths, &id, "scaffolding only", false)
        .expect("accept the migrated feature");
    assert_eq!(report.status, FeatureStatus::Ready);

    // The read-back goes through the dispatched load -> reads the card, which
    // now reflects the write; the legacy feature.yaml is untouched.
    assert_eq!(
        feature::show(&paths, &id).expect("show").status,
        FeatureStatus::Ready
    );
    assert_eq!(
        card_status(&paths, &id),
        "ready",
        "card is the write target"
    );
    assert_eq!(
        legacy_status(&paths, &id),
        "proposed",
        "legacy feature.yaml stays frozen -> the card, not the legacy file, is authoritative"
    );

    // start: Ready -> InProgress. The second verb's load must read the card the
    // first verb wrote (not a stale snapshot), proving the CAS round-trip.
    let report = feature::start(&paths, &id).expect("start the migrated feature");
    assert_eq!(report.status, FeatureStatus::InProgress);
    assert_eq!(
        feature::show(&paths, &id).expect("show").status,
        FeatureStatus::InProgress
    );
    assert_eq!(card_status(&paths, &id), "in_progress");
    assert_eq!(
        legacy_status(&paths, &id),
        "proposed",
        "legacy feature.yaml still frozen after a second card write"
    );
}
