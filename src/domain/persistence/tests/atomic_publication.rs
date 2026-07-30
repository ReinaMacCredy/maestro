use std::fs;
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

use crate as maestro;
use maestro::domain::identity::{ContractRootIdV1, SchemaIdV1, StoreGenerationIdV1, StoreHeadIdV1};
use maestro::domain::persistence::{
    AtomicGenerationPublicationV1, LogicalTombstoneV1, StoreCompatibilityV1, StoreDomainV1,
    StoreGenerationV1, StoreIdempotencyProbeV1, StoreIdempotencyV1, StoreObjectV1,
    StorePublicationOutcomeV1, StoreRoleV1, StoreV1,
};
use maestro::foundation::core::deterministic_cbor::CborValue;
use rusqlite::Connection;

use super::super::store::{
    fail_next_atomic_publication_after_staging_for_test,
    install_before_failed_publication_cleanup_test_hook,
};
use super::TestTempDir;

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn object(seed: u64, references: Vec<maestro::domain::identity::StoreObjectIdV1>) -> StoreObjectV1 {
    StoreObjectV1::new(
        SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
        CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(seed)]),
        references,
    )
    .expect("Store Object")
}

fn object_file_path(object_id: maestro::domain::identity::StoreObjectIdV1) -> std::path::PathBuf {
    let rendered = object_id.to_string();
    let hex = rendered
        .strip_prefix("sha256:")
        .expect("Store Object identity rendering");
    std::path::Path::new("objects")
        .join(&hex[..2])
        .join(format!("{hex}.cbor"))
}

fn publication(
    domain: StoreDomainV1,
    result: StoreObjectV1,
    root: StoreObjectV1,
    meaning: u8,
) -> AtomicGenerationPublicationV1 {
    publication_with_lineage(
        domain,
        result,
        root,
        TestPublicationLineage {
            key: [7; 32],
            meaning,
            ordinal: 1,
            previous: None,
            expected_old: None,
        },
    )
}

struct TestPublicationLineage {
    key: [u8; 32],
    meaning: u8,
    ordinal: u64,
    previous: Option<StoreGenerationIdV1>,
    expected_old: Option<StoreHeadIdV1>,
}

fn publication_with_lineage(
    domain: StoreDomainV1,
    result: StoreObjectV1,
    root: StoreObjectV1,
    lineage: TestPublicationLineage,
) -> AtomicGenerationPublicationV1 {
    let mut roots = vec![root.id(), result.id()];
    roots.sort_by_key(|id| id.to_string());
    let generation = StoreGenerationV1::new(
        domain,
        lineage.ordinal,
        lineage.previous,
        ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
        StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
        roots,
    )
    .expect("Generation");
    let idempotency = StoreIdempotencyV1::new(
        "authority.issue-bootstrap-mandate",
        lineage.key,
        [lineage.meaning; 32],
        result.id(),
    )
    .expect("idempotency contract");
    AtomicGenerationPublicationV1::new(
        generation,
        lineage.expected_old,
        vec![result, root],
        idempotency,
    )
    .expect("atomic publication")
}

fn relative_file_inventory(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        inventory: &mut Vec<std::path::PathBuf>,
    ) {
        for entry in fs::read_dir(directory).expect("read Store directory") {
            let path = entry.expect("Store entry").path();
            if path.is_dir() {
                visit(root, &path, inventory);
            } else {
                inventory.push(
                    path.strip_prefix(root)
                        .expect("Store-relative path")
                        .to_owned(),
                );
            }
        }
    }
    let mut inventory = Vec::new();
    visit(root, root, &mut inventory);
    inventory.sort();
    inventory
}

#[test]
fn atomic_publication_commits_objects_head_result_and_idempotency_once() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-atomic-publication");
        let path = fs::canonicalize(temp.path())
            .expect("canonical temp")
            .join(role.as_str());
        let domain = StoreDomainV1::derive(role, b"atomic-publication").expect("Store domain");
        let mut store = StoreV1::create(&path, domain.clone()).expect("Store");
        let result = object(1, vec![]);
        let root = object(2, vec![result.id()]);
        let request = publication(domain, result.clone(), root, 8);

        let committed = store
            .publish_generation_atomically(&request)
            .expect("atomic commit");
        assert!(matches!(
            committed,
            StorePublicationOutcomeV1::Committed { .. }
        ));
        assert_eq!(
            store.read_object(result.id()).expect("committed result"),
            result
        );

        let connection = Connection::open(path.join("store.sqlite3")).expect("metadata");
        let committed_clock: i64 = connection
            .query_row(
                "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("publication clock");
        let probe =
            StoreIdempotencyProbeV1::new("authority.issue-bootstrap-mandate", [7; 32], [8; 32])
                .expect("idempotency probe");
        assert!(matches!(
            store
                .replay_idempotency(&probe)
                .expect("pre-evaluation replay probe"),
            Some(StorePublicationOutcomeV1::Replayed { .. })
        ));
        let replayed = store
            .publish_generation_atomically(&request)
            .expect("same-key replay");
        assert!(matches!(
            replayed,
            StorePublicationOutcomeV1::Replayed { .. }
        ));
        let replay_clock: i64 = connection
            .query_row(
                "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("replay publication clock");
        assert_eq!(
            replay_clock, committed_clock,
            "replay performs zero publication writes"
        );
    }
}

#[test]
fn historical_idempotency_result_is_a_durable_replay_horizon_after_head_advance() {
    let temp = TestTempDir::new("maestro-vnext-idempotency-replay-retention");
    let path = fs::canonicalize(temp.path())
        .expect("canonical temp")
        .join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"idempotency-replay-retention")
        .expect("Store domain");
    let mut store = StoreV1::create(&path, domain.clone()).expect("Store");
    let first_result = object(101, vec![]);
    let first_result_id = first_result.id();
    let first_root = object(102, vec![first_result_id]);
    let first = store
        .publish_generation_atomically(&publication(
            domain.clone(),
            first_result.clone(),
            first_root,
            101,
        ))
        .expect("first publication");
    let first_head = first.head().clone();

    let second_result = object(103, vec![]);
    let second_root = object(104, vec![second_result.id()]);
    store
        .publish_generation_atomically(&publication_with_lineage(
            domain.clone(),
            second_result,
            second_root,
            TestPublicationLineage {
                key: [103; 32],
                meaning: 103,
                ordinal: 2,
                previous: Some(first_head.generation_id()),
                expected_old: Some(first_head.id()),
            },
        ))
        .expect("advance active Head");

    let snapshot = store
        .snapshot_reachability()
        .expect("replay-aware snapshot");
    assert!(snapshot.roots().iter().any(|root| {
        root.kind() == maestro::domain::persistence::RetentionRootKindV1::ReplayHorizon
            && root.object_id() == first_result_id
    }));
    let tombstone =
        LogicalTombstoneV1::new(snapshot.head_id(), first_result_id, [105; 32], [106; 32])
            .expect("historical result tombstone");
    assert!(matches!(
        store.tombstone(&tombstone, snapshot.retention_revision()),
        Err(maestro::domain::persistence::StoreError::ObjectStillReachable(id))
            if id == first_result_id
    ));
    drop(store);

    let reopened = StoreV1::open(&path, domain).expect("reopen Store");
    let probe =
        StoreIdempotencyProbeV1::new("authority.issue-bootstrap-mandate", [7; 32], [101; 32])
            .expect("historical replay probe");
    let replayed = reopened
        .replay_idempotency(&probe)
        .expect("replay read")
        .expect("historical replay");
    assert!(matches!(
        replayed,
        StorePublicationOutcomeV1::Replayed { .. }
    ));
    assert_eq!(replayed.result(), &first_result);
}

#[test]
fn same_key_changed_meaning_conflicts_without_writes() {
    let temp = TestTempDir::new("maestro-vnext-atomic-meaning-conflict");
    let path = fs::canonicalize(temp.path())
        .expect("canonical temp")
        .join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"atomic-meaning-conflict")
        .expect("Store domain");
    let mut store = StoreV1::create(&path, domain.clone()).expect("Store");
    let result = object(3, vec![]);
    let root = object(4, vec![result.id()]);
    store
        .publish_generation_atomically(&publication(
            domain.clone(),
            result.clone(),
            root.clone(),
            9,
        ))
        .expect("first commit");

    let connection = Connection::open(path.join("store.sqlite3")).expect("metadata");
    let before: i64 = connection
        .query_row(
            "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .expect("clock before conflict");
    assert!(
        store
            .publish_generation_atomically(&publication(domain, result, root, 10))
            .is_err()
    );
    let after: i64 = connection
        .query_row(
            "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .expect("clock after conflict");
    assert_eq!(after, before);
}

#[test]
fn concurrent_same_key_publications_have_one_commit_and_one_zero_write_replay() {
    let temp = TestTempDir::new("maestro-vnext-atomic-publication-race");
    let path = fs::canonicalize(temp.path())
        .expect("canonical temp")
        .join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Installation, b"atomic-publication-race")
        .expect("Store domain");
    drop(StoreV1::create(&path, domain.clone()).expect("Store"));
    let result = object(11, vec![]);
    let root = object(12, vec![result.id()]);
    let request = publication(domain.clone(), result, root, 13);
    let start = Arc::new(Barrier::new(3));

    let contenders = (0..2)
        .map(|_| {
            let path = path.clone();
            let domain = domain.clone();
            let request = request.clone();
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                let mut store = StoreV1::open(path, domain).expect("contender Store");
                start.wait();
                store
                    .publish_generation_atomically(&request)
                    .expect("contended publication")
            })
        })
        .collect::<Vec<_>>();
    start.wait();
    let outcomes = contenders
        .into_iter()
        .map(|contender| contender.join().expect("contender thread"))
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, StorePublicationOutcomeV1::Committed { .. }))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, StorePublicationOutcomeV1::Replayed { .. }))
            .count(),
        1
    );

    let connection = Connection::open(path.join("store.sqlite3")).expect("metadata");
    let (clock, heads, idempotency): (i64, i64, i64) = connection
        .query_row(
            "SELECT
                 (SELECT publication_clock FROM store_publication_clock WHERE singleton = 1),
                 (SELECT COUNT(*) FROM store_heads),
                 (SELECT COUNT(*) FROM store_idempotency)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("publication counts");
    assert_eq!((clock, heads, idempotency), (1, 1, 1));
}

#[test]
fn stale_different_key_publication_leaves_no_raw_object_debt() {
    let temp = TestTempDir::new("maestro-vnext-atomic-stale-cleanup");
    let path = fs::canonicalize(temp.path())
        .expect("canonical temp")
        .join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"atomic-stale-cleanup")
        .expect("Store domain");
    let mut store = StoreV1::create(&path, domain.clone()).expect("Store");

    let first_result = object(20, vec![]);
    let first_root = object(21, vec![first_result.id()]);
    let first = store
        .publish_generation_atomically(&publication(domain.clone(), first_result, first_root, 20))
        .expect("first publication");
    let first_head = first.head().clone();

    let second_result = object(22, vec![]);
    let second_root = object(23, vec![second_result.id()]);
    store
        .publish_generation_atomically(&publication_with_lineage(
            domain.clone(),
            second_result,
            second_root,
            TestPublicationLineage {
                key: [22; 32],
                meaning: 22,
                ordinal: 2,
                previous: Some(first_head.generation_id()),
                expected_old: Some(first_head.id()),
            },
        ))
        .expect("second publication");

    let before_files = relative_file_inventory(&path);
    let connection = Connection::open(path.join("store.sqlite3")).expect("metadata");
    let before_counts: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
                 (SELECT publication_clock FROM store_publication_clock WHERE singleton = 1),
                 (SELECT COUNT(*) FROM store_objects),
                 (SELECT COUNT(*) FROM store_heads),
                 (SELECT COUNT(*) FROM store_idempotency)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("before counts");

    let stale_result = object(24, vec![]);
    let stale_root = object(25, vec![stale_result.id()]);
    let error = store
        .publish_generation_atomically(&publication_with_lineage(
            domain,
            stale_result,
            stale_root,
            TestPublicationLineage {
                key: [24; 32],
                meaning: 24,
                ordinal: 2,
                previous: Some(first_head.generation_id()),
                expected_old: Some(first_head.id()),
            },
        ))
        .expect_err("stale expected Head must fail");
    assert!(matches!(
        error,
        maestro::domain::persistence::StoreError::HeadCasMismatch
    ));

    let after_counts: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
                 (SELECT publication_clock FROM store_publication_clock WHERE singleton = 1),
                 (SELECT COUNT(*) FROM store_objects),
                 (SELECT COUNT(*) FROM store_heads),
                 (SELECT COUNT(*) FROM store_idempotency)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("after counts");
    assert_eq!(after_counts, before_counts);
    assert_eq!(relative_file_inventory(&path), before_files);
}

#[test]
fn failed_writer_cleanup_cannot_unlink_an_object_committed_by_a_waiting_writer() {
    let temp = TestTempDir::new("maestro-vnext-atomic-cleanup-race");
    let path = fs::canonicalize(temp.path())
        .expect("canonical temp")
        .join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"atomic-cleanup-race")
        .expect("Store domain");
    let mut setup = StoreV1::create(&path, domain.clone()).expect("Store");
    let initial_result = object(31, vec![]);
    let initial_root = object(32, vec![initial_result.id()]);
    let initial = setup
        .publish_generation_atomically(&publication(
            domain.clone(),
            initial_result,
            initial_root,
            31,
        ))
        .expect("initial publication");
    let initial_head = initial.head().clone();
    drop(setup);

    let failed_only = object(30, vec![]);
    let failed_only_id = failed_only.id();
    let shared_result = object(33, vec![]);
    let mut failing_references = vec![shared_result.id(), failed_only.id()];
    failing_references.sort_unstable();
    let failing_root = object(34, failing_references);
    let mut failing_roots = vec![failing_root.id(), shared_result.id()];
    failing_roots.sort_unstable();
    let failing_root_id = failing_root.id();
    let failing_generation = StoreGenerationV1::new(
        domain.clone(),
        2,
        Some(initial_head.generation_id()),
        ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
        StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
        failing_roots,
    )
    .expect("failing Generation");
    let failing_idempotency = StoreIdempotencyV1::new(
        "authority.issue-bootstrap-mandate",
        [34; 32],
        [34; 32],
        shared_result.id(),
    )
    .expect("failing idempotency");
    let failing_publication = AtomicGenerationPublicationV1::new(
        failing_generation,
        Some(initial_head.id()),
        vec![shared_result.clone(), failed_only, failing_root],
        failing_idempotency,
    )
    .expect("failing publication shape");
    let committed_root = object(35, vec![shared_result.id()]);
    let committed_publication = publication_with_lineage(
        domain.clone(),
        shared_result.clone(),
        committed_root,
        TestPublicationLineage {
            key: [35; 32],
            meaning: 35,
            ordinal: 2,
            previous: Some(initial_head.generation_id()),
            expected_old: Some(initial_head.id()),
        },
    );

    let race_timeout = Duration::from_secs(10);
    let (cleanup_ready_tx, cleanup_ready_rx) = mpsc::channel();
    let (commit_done_tx, commit_done_rx) = mpsc::channel();
    let (failure_tx, failure_rx) = mpsc::sync_channel(1);
    let failing_path = path.clone();
    let failing_domain = domain.clone();
    let failing_writer = std::thread::spawn(move || {
        let mut store = StoreV1::open(failing_path, failing_domain).expect("failing Store");
        fail_next_atomic_publication_after_staging_for_test();
        install_before_failed_publication_cleanup_test_hook(move || {
            cleanup_ready_tx
                .send(())
                .expect("cleanup readiness receiver");
            commit_done_rx
                .recv_timeout(race_timeout)
                .expect("waiting writer must commit before cleanup resumes");
        });
        let failure = store
            .publish_generation_atomically(&failing_publication)
            .expect_err("injected post-staging failure must fail publication");
        drop(store);
        failure_tx
            .send(failure)
            .expect("failed writer completion receiver");
    });

    cleanup_ready_rx
        .recv_timeout(race_timeout)
        .expect("failed writer must reach cleanup after staging");
    let mut committed_store = StoreV1::open(&path, domain.clone()).expect("committing Store");
    let committed = committed_store
        .publish_generation_atomically(&committed_publication)
        .expect("waiting writer commits shared object");
    let connection = Connection::open(path.join("store.sqlite3")).expect("metadata");
    let clock_before_cleanup: i64 = connection
        .query_row(
            "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .expect("clock before guarded cleanup");
    commit_done_tx
        .send(())
        .expect("failed writer cleanup sender");
    let failure = failure_rx
        .recv_timeout(race_timeout)
        .expect("failed writer must finish cleanup within the race bound");
    failing_writer.join().expect("failing writer thread");
    assert!(matches!(
        failure,
        maestro::domain::persistence::StoreError::Metadata(_)
    ));
    let clock_after_cleanup: i64 = connection
        .query_row(
            "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .expect("clock after guarded cleanup");
    assert_eq!(clock_after_cleanup, clock_before_cleanup);
    assert!(
        !path.join(object_file_path(failed_only_id)).exists(),
        "bytes staged only by the failed writer must be removed"
    );
    assert!(
        !path.join(object_file_path(failing_root_id)).exists(),
        "failed-publication-only bytes must be removed"
    );
    assert!(
        path.join(object_file_path(shared_result.id())).is_file(),
        "bytes committed by the waiting writer must survive cleanup"
    );
    let committed_head = committed.head().clone();
    drop(committed_store);

    let reopened = StoreV1::open(&path, domain).expect("committed Store remains reopenable");
    assert_eq!(
        reopened
            .publication_generation(committed_head.id())
            .expect("committed Generation")
            .id(),
        committed_head.generation_id()
    );
    assert_eq!(
        reopened
            .read_object(shared_result.id())
            .expect("shared committed object file survives cleanup"),
        shared_result
    );
}
