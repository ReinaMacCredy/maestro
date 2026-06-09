mod support;

use std::fs;

use maestro::domain::harness::backlog;
use maestro::domain::harness::schema::{BacklogConfig, BacklogItem, HistoryEntry};
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

fn paths_for(temp: &TestTempDir) -> MaestroPaths {
    let paths = MaestroPaths::new(temp.path());
    fs::create_dir_all(paths.harness_dir()).expect("invariant: harness dir should be creatable");
    paths
}

fn proposal(source: &str, item_type: &str, title: &str) -> BacklogItem {
    BacklogItem {
        id: String::new(),
        fingerprint: format!("{item_type}:{source}"),
        source: source.to_string(),
        provenance: "detector".to_string(),
        topic: source.to_string(),
        item_type: item_type.to_string(),
        title: title.to_string(),
        priority: "medium".to_string(),
        occurrences: 0,
        sessions_hit: Vec::new(),
        first_seen: String::new(),
        last_seen: String::new(),
        status: "proposed".to_string(),
        evidence: vec![format!("{source} evidence")],
        spawned_task: None,
        dismissal_reason: None,
        history: Vec::new(),
    }
}

#[test]
fn refresh_assigns_stable_ids_and_deduplicates_by_key() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);

    let backlog = backlog::refresh(
        &paths,
        vec![
            proposal("source-b", "missing_skill", "Add skill B"),
            proposal("source-a", "missing_skill", "Add skill A"),
            proposal("source-a", "missing_skill", "Add skill A"),
        ],
    )
    .expect("invariant: backlog refresh should succeed");

    assert_eq!(backlog.items.len(), 2);
    for item in &backlog.items {
        assert!(
            item.id.starts_with("card-"),
            "merge-minted ideas use card ids: {}",
            item.id
        );
    }
    assert_ne!(backlog.items[0].id, backlog.items[1].id);

    let refreshed = backlog::refresh(
        &paths,
        vec![
            proposal("source-b", "missing_skill", "Add skill B"),
            proposal("source-a", "missing_skill", "Add skill A"),
        ],
    )
    .expect("invariant: duplicate refresh should succeed");

    // The fingerprint match keeps each item's minted id stable across refreshes.
    let ids = |items: &[BacklogItem]| {
        let mut pairs: Vec<(String, String)> = items
            .iter()
            .map(|item| (item.source.clone(), item.id.clone()))
            .collect();
        pairs.sort();
        pairs
    };
    assert_eq!(ids(&refreshed.items), ids(&backlog.items));
}

#[test]
fn refresh_preserves_existing_ids_and_mints_card_ids() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let mut existing = BacklogConfig::empty();
    let mut existing_item = proposal("existing", "recurring_blocker", "Fix existing blocker");
    existing_item.id = "hb-007".to_string();
    // accepted so D4 ephemeral reconcile keeps it when the fresh run no longer detects it.
    existing_item.status = "accepted".to_string();
    existing.items.push(existing_item);
    backlog::save(&paths, &existing).expect("invariant: existing backlog should save");

    let refreshed = backlog::refresh(
        &paths,
        vec![proposal("new", "recurring_blocker", "Fix new blocker")],
    )
    .expect("invariant: backlog refresh should succeed");

    let existing = refreshed
        .items
        .iter()
        .find(|item| item.source == "existing")
        .expect("invariant: existing item survives the refresh");
    assert_eq!(existing.id, "hb-007", "a pre-minted id is never rewritten");
    let minted = refreshed
        .items
        .iter()
        .find(|item| item.source == "new")
        .expect("invariant: new proposal lands");
    assert!(
        minted.id.starts_with("card-"),
        "new proposals mint card ids: {}",
        minted.id
    );
}

#[test]
fn refresh_drops_undetected_proposed_note_without_history() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let mut existing = BacklogConfig::empty();
    let mut stale = proposal("stale", "missing_skill", "Add stale skill");
    stale.id = "hb-005".to_string();
    // proposed, no spawned task, no history: a pure evidence note with nothing durable.
    existing.items.push(stale);
    backlog::save(&paths, &existing).expect("invariant: existing backlog should save");

    // A refresh that no longer detects it reconciles the ephemeral note away (D4).
    let refreshed =
        backlog::refresh(&paths, Vec::new()).expect("invariant: backlog refresh should succeed");

    assert!(refreshed.items.is_empty());
}

#[test]
fn refresh_keeps_undetected_proposed_note_with_history() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let mut existing = BacklogConfig::empty();
    let mut durable = proposal("durable", "missing_skill", "Add durable skill");
    durable.id = "hb-005".to_string();
    // A prior measure-fail left history: durable, so D4 must NOT drop it even when undetected.
    durable.history.push(HistoryEntry {
        result: "ineffective".to_string(),
        task: None,
        note: None,
        at: "1970-01-01T00:00:00Z".to_string(),
    });
    existing.items.push(durable);
    backlog::save(&paths, &existing).expect("invariant: existing backlog should save");

    let refreshed =
        backlog::refresh(&paths, Vec::new()).expect("invariant: backlog refresh should succeed");

    assert_eq!(refreshed.items.len(), 1);
    assert_eq!(refreshed.items[0].id, "hb-005");
}

#[test]
fn refresh_updates_existing_proposal_evidence_without_changing_status() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let mut existing = BacklogConfig::empty();
    let mut existing_item = proposal("existing", "missing_verification", "Add existing check");
    existing_item.id = "hb-003".to_string();
    existing_item.status = "applied".to_string();
    existing_item.evidence = vec![
        "manual note: keep this context".to_string(),
        "verification.json used `api_key='old secret' cargo test` outside harness.yml".to_string(),
    ];
    existing.items.push(existing_item);
    backlog::save(&paths, &existing).expect("invariant: existing backlog should save");

    let mut refreshed_proposal = proposal("existing", "missing_verification", "Add existing check");
    refreshed_proposal.evidence =
        vec!["verification.json used verification command 1 outside harness.yml".to_string()];
    let refreshed = backlog::refresh(&paths, vec![refreshed_proposal])
        .expect("invariant: backlog refresh should succeed");

    assert_eq!(refreshed.items.len(), 1);
    assert_eq!(refreshed.items[0].id, "hb-003");
    assert_eq!(refreshed.items[0].status, "applied");
    assert_eq!(
        refreshed.items[0].evidence,
        vec![
            "manual note: keep this context",
            "verification.json used verification command 1 outside harness.yml",
        ]
    );
}

#[test]
fn refresh_sanitizes_orphaned_legacy_missing_verification_evidence() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let mut existing = BacklogConfig::empty();
    let mut existing_item = proposal(
        "task-001",
        "missing_verification",
        "Add legacy verification",
    );
    existing_item.id = "hb-003".to_string();
    // accepted (durable) so D4 reconcile keeps the orphan; evidence is still scrubbed in place.
    existing_item.status = "accepted".to_string();
    existing_item.evidence = vec![
        "manual note: keep this context".to_string(),
        "verification.attempts/api_key=top_secret.json used `api_key='top secret' cargo test` outside harness.yml"
            .to_string(),
    ];
    existing.items.push(existing_item);
    backlog::save(&paths, &existing).expect("invariant: existing backlog should save");

    let refreshed =
        backlog::refresh(&paths, Vec::new()).expect("invariant: backlog refresh should succeed");

    assert_eq!(
        refreshed.items[0].evidence,
        vec![
            "manual note: keep this context",
            "verification.attempts/archived attempt used verification command 1 outside harness.yml",
        ]
    );
}

#[test]
fn refresh_preserves_manual_missing_verification_evidence() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let mut existing = BacklogConfig::empty();
    let mut existing_item = proposal(
        "task-001",
        "missing_verification",
        "Add manual verification",
    );
    existing_item.id = "hb-003".to_string();
    // accepted (durable) so D4 reconcile keeps the orphan and the manual note survives.
    existing_item.status = "accepted".to_string();
    existing_item.evidence = vec!["manual note: keep this context".to_string()];
    existing.items.push(existing_item);
    backlog::save(&paths, &existing).expect("invariant: existing backlog should save");

    let refreshed =
        backlog::refresh(&paths, Vec::new()).expect("invariant: backlog refresh should succeed");

    assert_eq!(
        refreshed.items[0].evidence,
        vec!["manual note: keep this context"]
    );
}

#[test]
fn refresh_does_not_apply_harness_config_changes() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    let harness_yml = paths.harness_dir().join("harness.yml");
    fs::write(
        &harness_yml,
        "schema_version: maestro.harness.v1\nsentinel: keep\n",
    )
    .expect("invariant: harness config should be writable");
    let before = fs::read_to_string(&harness_yml).expect("invariant: harness config should read");

    backlog::refresh(
        &paths,
        vec![proposal("source", "missing_skill", "Add skill")],
    )
    .expect("invariant: backlog refresh should succeed");

    assert_eq!(
        fs::read_to_string(&harness_yml).expect("invariant: harness config should read"),
        before
    );
}

#[test]
fn load_ignores_legacy_backlog_yaml() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    // A migration leftover, any schema: the card store is the only store (D7),
    // so the legacy file is never read and never an error.
    fs::write(
        paths.harness_dir().join("backlog.yaml"),
        "schema_version: maestro.backlog.v0\nitems:\n  - id: hb-001\n    title: ghost\n",
    )
    .expect("invariant: backlog should be writable");

    let backlog = backlog::load(&paths).expect("invariant: legacy file must not break the load");

    assert!(
        backlog.items.is_empty(),
        "items come from idea cards only, never backlog.yaml"
    );
}
