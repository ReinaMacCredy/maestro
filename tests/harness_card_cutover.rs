//! Slice 3e end-to-end: after migration the harness backlog verbs read and write
//! the card store. The backlog is an AGGREGATE -- store-level `evidence_stamp`
//! plus a whole-list merge -- not a per-record store, so this proves a real verb
//! (`unapply`) round-trips an item through its `idea` card while the store-level
//! `evidence_stamp` survives in the metadata file that keeps `items: []`
//! (SPEC-beads-model P1 dual-read cutover; D7/P5 collapses the two stores).

mod support;

use maestro::domain::card::schema::{Card, CardType};
use maestro::domain::card::store::card_path;
use maestro::domain::card::{StoreMode, store_mode};
use maestro::domain::harness::backlog;
use maestro::domain::harness::schema::{BacklogConfig, BacklogItem, HistoryEntry};
use maestro::foundation::core::paths::MaestroPaths;
use maestro::operations::{card_migrate, harness};
use support::TestTempDir;

const NOW: &str = "2026-06-09T12:00:00Z";

/// An accepted, durable item: `status != "proposed"` so the merge never
/// reconciles it away, and `spawned_task: None` so `unapply` needs no live task.
fn accepted_item() -> BacklogItem {
    BacklogItem {
        id: "hb-001".to_string(),
        fingerprint: "agent_audit:csv-export".to_string(),
        source: "task-002".to_string(),
        provenance: "agent-audit".to_string(),
        topic: "verification".to_string(),
        item_type: "agent_audit".to_string(),
        title: "Add a verification command".to_string(),
        priority: "high".to_string(),
        occurrences: 2,
        sessions_hit: vec!["task-002".to_string()],
        first_seen: "2026-06-08T00:00:00Z".to_string(),
        last_seen: "2026-06-08T00:00:00Z".to_string(),
        status: "accepted".to_string(),
        evidence: vec!["task-002 had no verify leg".to_string()],
        spawned_task: None,
        dismissal_reason: None,
        history: vec![HistoryEntry {
            result: "accepted".to_string(),
            task: None,
            note: None,
            at: "2026-06-08T01:00:00Z".to_string(),
        }],
    }
}

/// The persisted card, parsed straight off disk.
fn card(paths: &MaestroPaths, id: &str) -> Card {
    let contents = std::fs::read_to_string(card_path(paths, id)).expect("card.yaml readable");
    serde_yaml::from_str(&contents).expect("card.yaml parses")
}

/// The store-metadata file, parsed straight off disk.
fn metadata(paths: &MaestroPaths) -> BacklogConfig {
    let contents = std::fs::read_to_string(paths.harness_dir().join("backlog.yaml"))
        .expect("backlog.yaml readable");
    serde_yaml::from_str(&contents).expect("backlog.yaml parses")
}

#[test]
fn unapply_after_migration_writes_the_card_and_keeps_the_evidence_stamp() {
    let temp = TestTempDir::new("harness-card-cutover");
    let paths = MaestroPaths::new(temp.path());
    std::fs::create_dir_all(paths.harness_dir()).expect("create harness dir");

    // Author a legacy backlog with a store-level evidence_stamp and one accepted
    // item, then migrate. The item folds to an hb-001 idea card; the evidence_stamp
    // is store-level state with no card home.
    let seeded = BacklogConfig {
        schema_version: BacklogConfig::empty().schema_version,
        evidence_stamp: "seed-stamp".to_string(),
        items: vec![accepted_item()],
    };
    backlog::save(&paths, &seeded).expect("seed legacy backlog");
    assert_eq!(
        store_mode(&paths),
        StoreMode::Legacy,
        "no cards/ yet -> legacy"
    );

    card_migrate::run(&paths, NOW).expect("migration succeeds");
    assert_eq!(store_mode(&paths), StoreMode::Cards, "cards/ -> card store");
    assert_eq!(card(&paths, "hb-001").card_type, CardType::Idea);

    // READ: the backlog verbs reconstruct the item from the idea card and the
    // store-level evidence_stamp from the metadata file.
    let loaded = harness::load_backlog(&paths).expect("load backlog in card mode");
    let item = loaded.find("hb-001").expect("hb-001 present");
    assert_eq!(item.status, "accepted", "item read from the card");
    assert_eq!(
        loaded.evidence_stamp, "seed-stamp",
        "store-level stamp survives the cutover"
    );

    // WRITE through a real verb: unapply is a whole-store RMW that never touches the
    // evidence_stamp, so it isolates the item-card write from the metadata file.
    let unapplied = harness::unapply(&paths, "hb-001", Some("reverting")).expect("unapply");
    assert_eq!(unapplied.item.status, "proposed");

    // The change landed in the idea card (its derived status flipped)...
    assert_eq!(card(&paths, "hb-001").status, "proposed");

    // ...the items moved to cards, and the metadata file kept the stamp...
    let meta = metadata(&paths);
    assert!(
        meta.items.is_empty(),
        "items live as cards, not in backlog.yaml"
    );
    assert_eq!(
        meta.evidence_stamp, "seed-stamp",
        "unapply leaves the store-level stamp untouched"
    );

    // ...and a fresh read round-trips the unapplied item from the card.
    let reloaded = harness::load_backlog(&paths).expect("reload");
    let item = reloaded.find("hb-001").expect("hb-001 still present");
    assert_eq!(item.status, "proposed");
    assert!(item.spawned_task.is_none(), "unapply cleared the task link");
}
