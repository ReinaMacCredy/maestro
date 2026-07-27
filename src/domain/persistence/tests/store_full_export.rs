use std::fs;
use std::path::{Path, PathBuf};

use crate as maestro;
use maestro::domain::identity::{ContractRootIdV1, SchemaIdV1};
use maestro::domain::persistence::{
    LogicalTombstoneV1, RetentionPinV1, RetentionRootKindV1, RetentionRootV1, SealedExportV1,
    StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreHeadV1, StoreObjectV1,
    StoreRoleV1, StoreStateV1, StoreV1,
};
use maestro::foundation::core::deterministic_cbor::{self, CborValue};
use rusqlite::{Connection, params};

use super::TestTempDir;

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn object(seed: u64) -> StoreObjectV1 {
    StoreObjectV1::new(
        SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
        CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(seed)]),
        vec![],
    )
    .expect("Store Object")
}

fn canonical_temp(temp: &TestTempDir) -> PathBuf {
    fs::canonicalize(temp.path()).expect("canonical temporary directory")
}

fn snapshot_closure_relative_path(root_id: &[u8; 32]) -> PathBuf {
    let hex = root_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Path::new("exports")
        .join("snapshot-closures")
        .join(format!("{hex}.cbor"))
}

fn source(
    path: &Path,
    domain: StoreDomainV1,
    seed: u64,
) -> (StoreV1, StoreObjectV1, StoreGenerationV1, StoreHeadV1) {
    let mut store = StoreV1::create(path, domain.clone()).expect("create Store");
    let root = object(seed);
    store.put_object(&root).expect("persist root");
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
        StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
        vec![root.id()],
    )
    .expect("Generation");
    let head = store
        .publish_generation(&generation, None)
        .expect("publish Generation");
    (store, root, generation, head)
}

fn insert_idempotency_rows(
    path: &Path,
    root: &StoreObjectV1,
    generation: &StoreGenerationV1,
    count: usize,
    reverse: bool,
) {
    let connection = Connection::open(path.join("store.sqlite3")).expect("metadata connection");
    let head_id: Vec<u8> = connection
        .query_row("SELECT head_id FROM store_heads", [], |row| row.get(0))
        .expect("Head identity");
    let keys = if reverse {
        (0..count).rev().collect::<Vec<_>>()
    } else {
        (0..count).collect::<Vec<_>>()
    };
    for key in keys {
        connection
            .execute(
                "INSERT INTO store_idempotency
                 (namespace, key_digest, meaning_digest, result_object_id, generation_id, head_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "test.full-export",
                    [key as u8; 32].as_slice(),
                    [(key as u8).wrapping_add(1); 32].as_slice(),
                    root.id().as_bytes(),
                    generation.id().as_bytes(),
                    head_id,
                ],
            )
            .expect("historical idempotency row");
    }
}

fn activate_with_sqlite(store_path: &Path) {
    let mut connection =
        Connection::open(store_path.join("store.sqlite3")).expect("Store metadata");
    let transaction = connection.transaction().expect("activation transaction");
    let (head_id, head_revision): (Vec<u8>, i64) = transaction
        .query_row(
            "SELECT head_id, head_revision FROM store_heads ORDER BY head_revision DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("staged Head");
    transaction
        .execute(
            "INSERT INTO store_active_head(singleton, head_id, head_revision) VALUES (1, ?1, ?2)",
            params![head_id, head_revision],
        )
        .expect("test-only active Head publication");
    transaction
        .execute(
            "UPDATE store_retention_revision SET retention_revision = 1 WHERE singleton = 1 AND retention_revision = 0",
            [],
        )
        .expect("test-only retention activation");
    let changed = transaction
        .execute(
            "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
            [],
        )
        .expect("test-only activation transition");
    assert_eq!(changed, 1);
    transaction
        .execute(
            "UPDATE store_publication_clock SET publication_clock = publication_clock + 1 WHERE singleton = 1",
            [],
        )
        .expect("test-only activation publication clock");
    transaction.commit().expect("activation commit");
}

fn mutate_block_closure(
    export: &SealedExportV1,
    mutate: impl FnOnce(&mut Vec<CborValue>),
) -> Vec<u8> {
    let mut export_value =
        deterministic_cbor::decode(export.canonical_bytes()).expect("export CBOR");
    let CborValue::Array(export_fields) = &mut export_value else {
        panic!("export shape");
    };
    let Some(CborValue::Bytes(closure_bytes)) = export_fields.last_mut() else {
        panic!("V2 closure bytes");
    };
    let mut closure = deterministic_cbor::decode(closure_bytes).expect("closure CBOR");
    let CborValue::Array(closure_fields) = &mut closure else {
        panic!("closure shape");
    };
    let Some(CborValue::Array(blocks)) = closure_fields.last_mut() else {
        panic!("block set");
    };
    mutate(blocks);
    *closure_bytes = deterministic_cbor::encode(&closure).expect("mutated closure CBOR");
    deterministic_cbor::encode(&export_value).expect("mutated export CBOR")
}

fn splice_export_closure(top_level: &SealedExportV1, closure_source: &SealedExportV1) -> Vec<u8> {
    let mut top_level_value =
        deterministic_cbor::decode(top_level.canonical_bytes()).expect("top-level export CBOR");
    let closure_source_value = deterministic_cbor::decode(closure_source.canonical_bytes())
        .expect("closure-source export CBOR");
    let CborValue::Array(top_level_fields) = &mut top_level_value else {
        panic!("top-level export shape");
    };
    let CborValue::Array(closure_source_fields) = closure_source_value else {
        panic!("closure-source export shape");
    };
    let closure = closure_source_fields
        .last()
        .expect("V2 closure-source bytes")
        .clone();
    *top_level_fields.last_mut().expect("V2 top-level closure") = closure;
    deterministic_cbor::encode(&top_level_value).expect("spliced export CBOR")
}

#[test]
fn full_snapshot_identity_changes_with_idempotency_history_and_restores_it_inactive() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-full-export-history");
        let base = canonical_temp(&temp);
        let domain = StoreDomainV1::derive(role, b"full-export-history").expect("Store domain");
        let (mut with_history, root, generation, _) =
            source(&base.join("with-history"), domain.clone(), 10);
        insert_idempotency_rows(&base.join("with-history"), &root, &generation, 1, false);
        let with_history = with_history.seal_export().expect("full export");

        let (mut without_history, _, _, _) =
            source(&base.join("without-history"), domain.clone(), 10);
        let without_history = without_history.seal_export().expect("full export");
        assert_eq!(with_history.head().id(), without_history.head().id());
        assert_ne!(
            with_history.snapshot_root().expect("V2 root").id(),
            without_history.snapshot_root().expect("V2 root").id()
        );
        assert_ne!(with_history.id(), without_history.id());

        let destination_path = base.join("destination");
        let mut destination =
            StoreV1::create(&destination_path, domain).expect("inactive destination");
        destination
            .import_inactive(with_history.canonical_bytes())
            .expect("full-history import");
        assert_eq!(
            destination.state().expect("destination state").0,
            StoreStateV1::Inactive
        );
        assert_eq!(
            destination.active_head().expect("inactive currentness"),
            None
        );
        let connection =
            Connection::open(destination_path.join("store.sqlite3")).expect("destination metadata");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM store_idempotency", [], |row| {
                row.get(0)
            })
            .expect("idempotency count");
        assert_eq!(count, 1);
    }
}

#[test]
fn full_history_import_supports_two_local_seals_and_retains_every_prior_root() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-full-export-import-reseal");
        let base = canonical_temp(&temp);
        let domain =
            StoreDomainV1::derive(role, b"full-export-import-reseal").expect("Store domain");
        let (mut source, _, _, _) = source(&base.join("source"), domain.clone(), 11);
        let source_first = source
            .seal_export()
            .expect("first source full-history export");
        let imported = source
            .seal_export()
            .expect("second source full-history export");
        let source_first_root = *source_first
            .snapshot_root()
            .expect("first source V2 root")
            .id()
            .as_bytes();
        let imported_root = *imported
            .snapshot_root()
            .expect("second source V2 root")
            .id()
            .as_bytes();

        let destination_path = base.join("destination");
        let mut destination =
            StoreV1::create(&destination_path, domain).expect("inactive destination");
        destination
            .import_inactive(imported.canonical_bytes())
            .expect("full-history import");
        activate_with_sqlite(&destination_path);
        let first_local = destination.seal_export().expect("first local seal");
        let second_local = destination.seal_export().expect("second local seal");
        let first_local_root = *first_local
            .snapshot_root()
            .expect("first local V2 root")
            .id()
            .as_bytes();
        let second_local_root = *second_local
            .snapshot_root()
            .expect("second local V2 root")
            .id()
            .as_bytes();
        assert_ne!(imported_root, first_local_root);
        assert_ne!(first_local_root, second_local_root);

        let connection =
            Connection::open(destination_path.join("store.sqlite3")).expect("destination metadata");
        let retained_roots = connection
            .prepare("SELECT snapshot_root_id FROM store_sealed_exports")
            .expect("sealed-export roots query")
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .expect("sealed-export roots")
            .collect::<Result<Vec<_>, _>>()
            .expect("sealed-export root rows");
        assert_eq!(retained_roots.len(), 4);
        for root in [
            source_first_root,
            imported_root,
            first_local_root,
            second_local_root,
        ] {
            assert!(
                retained_roots
                    .iter()
                    .any(|stored| stored.as_slice() == root),
                "every imported or locally sealed root remains authoritative"
            );
            assert!(
                destination_path
                    .join(snapshot_closure_relative_path(&root))
                    .is_file(),
                "every retained root keeps its verified snapshot closure"
            );
        }
    }
}

#[test]
fn seal_fails_closed_when_a_required_prior_snapshot_closure_is_missing() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-full-export-missing-prior-closure");
        let path = canonical_temp(&temp).join(role.as_str());
        let domain = StoreDomainV1::derive(role, b"full-export-missing-prior-closure")
            .expect("Store domain");
        let (mut store, _, _, _) = source(&path, domain, 12);
        let first = store.seal_export().expect("first full-history export");
        let first_root = first.snapshot_root().expect("first V2 root").id();
        fs::remove_file(path.join(snapshot_closure_relative_path(first_root.as_bytes())))
            .expect("remove required prior snapshot closure");

        assert!(
            store.seal_export().is_err(),
            "a seal must not omit a required prior snapshot closure"
        );
        let connection = Connection::open(path.join("store.sqlite3")).expect("Store metadata");
        let receipt_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM store_sealed_exports", [], |row| {
                row.get(0)
            })
            .expect("sealed-export receipt count");
        assert_eq!(receipt_count, 1, "the stale seal receipt must not commit");
    }
}

#[test]
fn restored_restore_candidate_history_round_trips_through_another_full_export() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-full-export-candidate-round-trip");
        let base = canonical_temp(&temp);
        let domain =
            StoreDomainV1::derive(role, b"full-export-candidate-round-trip").expect("Store domain");
        let (mut source, _, _, _) = source(&base.join("source"), domain.clone(), 13);
        let source_export = source.seal_export().expect("source full-history export");

        let first_restore_path = base.join("first-restore");
        let mut first_restore =
            StoreV1::create(&first_restore_path, domain.clone()).expect("first restore Store");
        let original_candidate = first_restore
            .import_inactive(source_export.canonical_bytes())
            .expect("first inactive import");
        activate_with_sqlite(&first_restore_path);
        let round_trip_export = first_restore
            .seal_export()
            .expect("full export containing restored candidate history");

        let second_restore_path = base.join("second-restore");
        let mut second_restore =
            StoreV1::create(&second_restore_path, domain.clone()).expect("second restore Store");
        second_restore
            .import_inactive(round_trip_export.canonical_bytes())
            .expect("second inactive import");
        assert_eq!(
            second_restore
                .restore_candidate(original_candidate.id())
                .expect("restored historical candidate"),
            original_candidate
        );
        drop(second_restore);

        let reopened = StoreV1::open(second_restore_path, domain).expect("reopen second restore");
        assert_eq!(
            reopened
                .restore_candidate(original_candidate.id())
                .expect("durable historical candidate"),
            original_candidate
        );
    }
}

#[test]
fn canonical_row_order_is_insertion_independent_and_crosses_chunk_boundary() {
    let temp = TestTempDir::new("maestro-vnext-full-export-chunks");
    let base = canonical_temp(&temp);
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"full-export-chunks").expect("domain");
    let mut exports = Vec::new();
    for (name, reverse) in [("forward", false), ("reverse", true)] {
        let path = base.join(name);
        let (mut store, root, generation, _) = source(&path, domain.clone(), 20);
        insert_idempotency_rows(&path, &root, &generation, 129, reverse);
        exports.push(store.seal_export().expect("chunked export"));
    }
    assert_eq!(
        exports[0].snapshot_root().expect("V2 root").id(),
        exports[1].snapshot_root().expect("V2 root").id()
    );
    assert_eq!(exports[0].id(), exports[1].id());
    assert!(
        exports[0]
            .snapshot_root()
            .expect("V2 root")
            .chunk_ids()
            .len()
            >= 7
    );
}

#[test]
fn the_next_export_captures_the_prior_lag_one_receipt() {
    let temp = TestTempDir::new("maestro-vnext-full-export-lag-one");
    let path = canonical_temp(&temp).join("store");
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Installation, b"full-export-lag-one").expect("domain");
    let (mut store, _, _, _) = source(&path, domain, 30);
    let first = store.seal_export().expect("first export");
    let second = store.seal_export().expect("second export");
    assert_ne!(first.id(), second.id());
    assert_ne!(
        first.snapshot_root().expect("first V2 root").id(),
        second.snapshot_root().expect("second V2 root").id()
    );
    let connection = Connection::open(path.join("store.sqlite3")).expect("metadata");
    let receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM store_sealed_exports", [], |row| {
            row.get(0)
        })
        .expect("receipt count");
    assert_eq!(receipts, 2);
}

#[test]
fn released_pin_tombstone_and_collection_history_restore_exactly_but_inactive() {
    let temp = TestTempDir::new("maestro-vnext-full-export-retention-history");
    let base = canonical_temp(&temp);
    let source_path = base.join("source");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"full-export-retention-history")
        .expect("domain");
    let (mut source, root, _, head) = source(&source_path, domain.clone(), 40);
    let orphan = object(41);
    source.put_object(&orphan).expect("persist orphan");
    let pin = RetentionPinV1::new(
        head.id(),
        RetentionRootV1::new(RetentionRootKindV1::LegalHold, root.id()),
        [11; 32],
    )
    .expect("retention pin");
    let revision = source.add_retention_pin(&pin, 1).expect("add pin");
    let revision = source
        .release_retention_pin(pin.id(), [12; 32], revision)
        .expect("release pin");
    let tombstone =
        LogicalTombstoneV1::new(head.id(), orphan.id(), [13; 32], [14; 32]).expect("tombstone");
    source
        .tombstone(&tombstone, revision)
        .expect("commit tombstone");
    let reachability = source.snapshot_reachability().expect("reachability");
    let plan = source.plan_collection(&reachability).expect("GC plan");
    assert_eq!(source.collect(&plan).expect("collect"), 1);
    let export = source.seal_export().expect("full-history export");

    let destination_path = base.join("destination");
    let mut destination = StoreV1::create(&destination_path, domain).expect("inactive destination");
    destination
        .import_inactive(export.canonical_bytes())
        .expect("full-history import");
    assert_eq!(
        destination.state().expect("destination state").0,
        StoreStateV1::Inactive
    );
    assert_eq!(destination.active_head().expect("active Head"), None);
    let connection =
        Connection::open(destination_path.join("store.sqlite3")).expect("destination metadata");
    for (table, expected) in [
        ("store_retention_pins", 1_i64),
        ("store_retention_pin_releases", 1),
        ("store_logical_tombstones", 1),
        ("store_gc_plans", 1),
        ("store_gc_plan_objects", 1),
        ("store_gc_collection_occurrences", 1),
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("history count");
        assert_eq!(count, expected, "history table {table}");
    }
}

#[test]
fn v2_flat_block_closure_rejects_missing_extra_reordered_and_malformed_blocks() {
    let temp = TestTempDir::new("maestro-vnext-full-export-block-tamper");
    let path = canonical_temp(&temp).join("source");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"full-export-block-tamper")
        .expect("domain");
    let (mut source, _, _, _) = source(&path, domain, 50);
    let export = source.seal_export().expect("full-history export");

    let missing = mutate_block_closure(&export, |blocks| {
        blocks.pop().expect("at least one block");
    });
    assert!(SealedExportV1::decode(&missing).is_err());

    let extra = mutate_block_closure(&export, |blocks| {
        blocks.push(blocks[0].clone());
    });
    assert!(SealedExportV1::decode(&extra).is_err());

    let reordered = mutate_block_closure(&export, |blocks| {
        blocks.swap(0, 1);
    });
    assert!(SealedExportV1::decode(&reordered).is_err());

    let malformed = mutate_block_closure(&export, |blocks| {
        let block = blocks
            .iter_mut()
            .find(|block| {
                matches!(block, CborValue::Array(fields) if matches!(fields.first(), Some(CborValue::Unsigned(1..=3))))
            })
            .expect("schema, family, or chunk block");
        let CborValue::Array(fields) = block else {
            unreachable!("selected Array block");
        };
        let CborValue::Bytes(payload) = fields.last_mut().expect("block payload") else {
            panic!("block payload bytes");
        };
        payload[0] ^= 0xff;
    });
    assert!(SealedExportV1::decode(&malformed).is_err());
}

#[test]
fn canonical_export_refuses_a_later_same_domain_snapshot_closure_before_import_writes() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-full-export-closure-splice");
        let base = canonical_temp(&temp);
        let domain =
            StoreDomainV1::derive(role, b"full-export-closure-splice").expect("Store domain");
        let (mut source, _, _, _) = source(&base.join("source"), domain.clone(), 60);
        let first = source.seal_export().expect("first canonical V2 export");
        let second = source.seal_export().expect("later canonical V2 export");
        let spliced = splice_export_closure(&first, &second);
        assert!(
            SealedExportV1::decode(&spliced).is_err(),
            "canonical export A must not accept same-domain closure B"
        );

        let destination_path = base.join("destination");
        let mut destination =
            StoreV1::create(&destination_path, domain).expect("inactive destination");
        assert!(destination.import_inactive(&spliced).is_err());
        let connection =
            Connection::open(destination_path.join("store.sqlite3")).expect("destination metadata");
        for table in [
            "store_objects",
            "store_heads",
            "store_sealed_exports",
            "store_restore_candidates",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("destination row count");
            assert_eq!(count, 0, "rejected splice must not write {table}");
        }
        assert_eq!(
            fs::read_dir(destination_path.join("exports"))
                .expect("destination exports directory")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .count(),
            0,
            "rejected splice must not persist export bytes"
        );
    }
}
