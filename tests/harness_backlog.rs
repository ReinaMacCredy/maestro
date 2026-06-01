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
        item_type: item_type.to_string(),
        title: title.to_string(),
        priority: "medium".to_string(),
        status: "proposed".to_string(),
        evidence: vec![format!("{source} evidence")],
        spawned_task: None,
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
    assert_eq!(backlog.items[0].id, "hb-001");
    assert_eq!(backlog.items[0].source, "source-a");
    assert_eq!(backlog.items[1].id, "hb-002");
    assert_eq!(backlog.items[1].source, "source-b");

    let refreshed = backlog::refresh(
        &paths,
        vec![
            proposal("source-b", "missing_skill", "Add skill B"),
            proposal("source-a", "missing_skill", "Add skill A"),
        ],
    )
    .expect("invariant: duplicate refresh should succeed");

    assert_eq!(refreshed.items, backlog.items);
}

#[test]
fn refresh_preserves_existing_ids_and_uses_next_number() {
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

    assert_eq!(refreshed.items[0].id, "hb-007");
    assert_eq!(refreshed.items[1].id, "hb-008");
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
fn load_rejects_backlog_schema_mismatch() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let paths = paths_for(&temp);
    fs::write(
        paths.harness_dir().join("backlog.yaml"),
        "schema_version: maestro.backlog.v0\nitems: []\n",
    )
    .expect("invariant: backlog should be writable");

    let error = backlog::load(&paths).expect_err("invariant: schema mismatch should fail");

    assert!(error.to_string().contains("schema mismatch"));
}

#[cfg(unix)]
#[test]
fn refresh_rejects_symlinked_backlog_paths() {
    let temp = TestTempDir::new("maestro-harness-backlog");
    let external = TestTempDir::new("maestro-harness-backlog-external");
    let paths = MaestroPaths::new(temp.path());
    fs::create_dir_all(paths.maestro_dir()).expect("invariant: maestro dir should be creatable");
    std::os::unix::fs::symlink(external.path(), paths.harness_dir())
        .expect("invariant: symlinked harness dir should be creatable");

    let error = backlog::refresh(
        &paths,
        vec![proposal("source", "missing_skill", "Add skill")],
    )
    .expect_err("invariant: symlinked backlog path should fail");

    assert!(error.to_string().contains("symlink"));
}
