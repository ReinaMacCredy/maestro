use std::fs;
use std::path::PathBuf;

use crate as maestro;
use maestro::domain::vnext::identity::{ContractRootIdV1, SchemaIdV1, StoreObjectIdV1};
use maestro::domain::vnext::persistence::{
    InstallationActivationIntentV1, LogicalTombstoneV1, RepositoryActivationIntentV1,
    RetentionPinV1, RetentionRootKindV1, RetentionRootV1, SealedBackupV1, SealedExportEntryV1,
    SealedExportV1, StoreCompatibilityV1, StoreDomainV1, StoreError, StoreGenerationV1,
    StoreObjectV1, StoreRoleV1, StoreStateV1, StoreV1,
};
use maestro::foundation::core::deterministic_cbor::{self, CborValue};
use rusqlite::Connection;

use super::TestTempDir;

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn schema() -> SchemaIdV1 {
    SchemaIdV1::parse(&rendered(1)).expect("valid Schema identity")
}

fn contract_root() -> ContractRootIdV1 {
    ContractRootIdV1::parse(&rendered(2)).expect("valid Contract Root identity")
}

fn compatibility() -> StoreCompatibilityV1 {
    StoreCompatibilityV1::stage0_successor().expect("frozen Stage 0 successor bindings")
}

fn object(seed: u64, references: Vec<StoreObjectIdV1>) -> StoreObjectV1 {
    StoreObjectV1::new(
        schema(),
        CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(seed)]),
        references,
    )
    .expect("valid Store Object")
}

fn object_relative_path(object_id: StoreObjectIdV1) -> String {
    let rendered = object_id.render();
    let hex = rendered
        .strip_prefix("sha256:")
        .expect("identity rendering prefix");
    format!("objects/{}/{hex}.cbor", &hex[..2])
}

fn canonical_temp(temp: &TestTempDir) -> PathBuf {
    fs::canonicalize(temp.path()).expect("canonical temporary directory")
}

fn rewrite_export_lineage(bytes: &[u8], rewrite: impl FnOnce(&mut Vec<CborValue>)) -> Vec<u8> {
    let CborValue::Array(mut export) =
        deterministic_cbor::decode(bytes).expect("canonical export carrier")
    else {
        panic!("export must be an array")
    };
    let CborValue::Array(lineage) = &mut export[1] else {
        panic!("export lineage must be an array")
    };
    rewrite(lineage);
    deterministic_cbor::encode(&CborValue::Array(export)).expect("rewritten canonical carrier")
}

#[test]
fn sqlite_is_truth_and_failed_reference_metadata_never_promotes_raw_bytes() {
    let temp = TestTempDir::new("maestro-vnext-store-truth");
    let store_path = canonical_temp(&temp).join("store");
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"truth").expect("Repository Store domain");
    let mut store = StoreV1::create(&store_path, domain).expect("create Store");
    for directory in ["objects", "exports", "recovery", "cache"] {
        assert!(store_path.join(directory).is_dir(), "missing {directory}");
    }

    let raw = object(1, vec![]);
    let raw_path = store_path.join(object_relative_path(raw.id()));
    fs::create_dir_all(raw_path.parent().expect("object parent")).expect("object directory");
    fs::write(&raw_path, raw.canonical_bytes()).expect("unregistered raw bytes");
    assert!(store.read_object(raw.id()).is_err());
    store
        .put_object(&raw)
        .expect("SQLite promotion after exact validation");
    assert_eq!(
        store.read_object(raw.id()).expect("authoritative object"),
        raw
    );

    let missing = object(2, vec![]);
    let referencing = object(3, vec![missing.id()]);
    assert!(store.put_object(&referencing).is_err());
    assert!(
        store_path
            .join(object_relative_path(referencing.id()))
            .is_file()
    );
    assert!(store.read_object(referencing.id()).is_err());
}

#[cfg(unix)]
#[test]
fn facade_rejects_symlink_and_hardlink_substitution_without_fallback() {
    use std::os::unix::fs::symlink;

    let temp = TestTempDir::new("maestro-vnext-store-links");
    let base = canonical_temp(&temp);
    let store_path = base.join("store");
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"links").expect("Repository Store domain");
    let mut store = StoreV1::create(&store_path, domain.clone()).expect("Store");

    let symlinked = object(5, vec![]);
    store
        .put_object(&symlinked)
        .expect("symlink target metadata");
    let symlink_path = store_path.join(object_relative_path(symlinked.id()));
    let external_symlink_target = base.join("symlink-target.cbor");
    fs::write(&external_symlink_target, symlinked.canonical_bytes()).expect("external bytes");
    fs::remove_file(&symlink_path).expect("remove canonical leaf");
    symlink(&external_symlink_target, &symlink_path).expect("substitute symlink");
    assert!(store.read_object(symlinked.id()).is_err());

    let hardlinked = object(6, vec![]);
    store
        .put_object(&hardlinked)
        .expect("hardlink target metadata");
    let hardlink_path = store_path.join(object_relative_path(hardlinked.id()));
    let external_hardlink_target = base.join("hardlink-target.cbor");
    fs::write(&external_hardlink_target, hardlinked.canonical_bytes()).expect("external bytes");
    fs::remove_file(&hardlink_path).expect("remove canonical leaf");
    fs::hard_link(&external_hardlink_target, &hardlink_path).expect("substitute hard link");
    assert!(store.read_object(hardlinked.id()).is_err());

    drop(store);
    let metadata_link = base.join("metadata-link.sqlite3");
    fs::hard_link(store_path.join("store.sqlite3"), &metadata_link).expect("metadata hard link");
    assert!(StoreV1::open(&store_path, domain).is_err());
}

#[test]
fn generation_publication_is_exact_non_aba_cas_and_failure_preserves_head() {
    let temp = TestTempDir::new("maestro-vnext-store-publish");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"publish")
        .expect("Repository Store domain");
    let mut store =
        StoreV1::create(canonical_temp(&temp).join("store"), domain.clone()).expect("Store");
    let first_root = object(10, vec![]);
    store.put_object(&first_root).expect("first object");
    let first_generation = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        contract_root(),
        compatibility(),
        vec![first_root.id()],
    )
    .expect("first Generation");
    let first_head = store
        .publish_generation(&first_generation, None)
        .expect("first publication");
    assert_eq!(
        store
            .publish_generation(&first_generation, None)
            .expect("exact first publication replay"),
        first_head
    );

    let second_root = object(11, vec![]);
    store.put_object(&second_root).expect("second object");
    let second_generation = StoreGenerationV1::new(
        domain,
        2,
        Some(first_generation.id()),
        contract_root(),
        compatibility(),
        vec![second_root.id()],
    )
    .expect("second Generation");
    assert!(store.publish_generation(&second_generation, None).is_err());
    assert_eq!(
        store.active_head().expect("inactive currentness read"),
        None
    );
    assert_eq!(
        store
            .publish_generation(&first_generation, None)
            .expect("first Head remains the staged publication"),
        first_head
    );
    let second_head = store
        .publish_generation(&second_generation, Some(first_head.id()))
        .expect("exact expected-old publication");
    assert_ne!(second_head.id(), first_head.id());
    assert_eq!(second_head.previous_head_id(), Some(first_head.id()));
}

#[test]
fn inactive_publication_reopens_without_exposing_runtime_currentness() {
    let temp = TestTempDir::new("maestro-vnext-store-reopen-inactive");
    let store_path = canonical_temp(&temp).join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"reopen-inactive")
        .expect("Repository Store domain");
    let mut store = StoreV1::create(&store_path, domain.clone()).expect("Store");
    let root = object(12, vec![]);
    store.put_object(&root).expect("root object");
    let generation = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        contract_root(),
        compatibility(),
        vec![root.id()],
    )
    .expect("Generation");
    store
        .publish_generation(&generation, None)
        .expect("staged publication");
    drop(store);

    let mut reopened = StoreV1::open(&store_path, domain.clone()).expect("reopen inactive Store");
    assert_eq!(reopened.active_head().expect("inactive currentness"), None);
    assert_eq!(
        reopened
            .seal_export()
            .expect("staged lineage remains internally usable")
            .generation(),
        &generation
    );
    drop(reopened);
    let recreated =
        StoreV1::create(&store_path, domain).expect("create uses open admission parity");
    assert_eq!(recreated.active_head().expect("inactive currentness"), None);
}

#[test]
fn sealed_export_refuses_missing_reordered_and_tampered_lineage() {
    let temp = TestTempDir::new("maestro-vnext-store-lineage-refusal");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"lineage-refusal")
        .expect("Repository Store domain");
    let mut store =
        StoreV1::create(canonical_temp(&temp).join("store"), domain.clone()).expect("Store");
    let roots = [object(13, vec![]), object(14, vec![]), object(15, vec![])];
    for root in &roots {
        store.put_object(root).expect("root object");
    }
    let mut previous_generation = None;
    let mut previous_head = None;
    for (index, root) in roots.iter().enumerate() {
        let generation = StoreGenerationV1::new(
            domain.clone(),
            index as u64 + 1,
            previous_generation,
            contract_root(),
            compatibility(),
            vec![root.id()],
        )
        .expect("Generation");
        let head = store
            .publish_generation(&generation, previous_head)
            .expect("publish Generation");
        previous_generation = Some(generation.id());
        previous_head = Some(head.id());
    }
    let export = store.seal_export().expect("sealed export");

    let missing = rewrite_export_lineage(export.export().canonical_bytes(), |lineage| {
        lineage.remove(1);
    });
    assert!(SealedExportV1::decode(&missing).is_err());

    let reordered = rewrite_export_lineage(export.export().canonical_bytes(), |lineage| {
        lineage.swap(1, 2);
    });
    assert!(SealedExportV1::decode(&reordered).is_err());

    let tampered = rewrite_export_lineage(export.export().canonical_bytes(), |lineage| {
        let CborValue::Array(member) = &mut lineage[1] else {
            panic!("lineage member must be an array")
        };
        let CborValue::Bytes(generation) = &mut member[0] else {
            panic!("Generation bytes must be bytes")
        };
        let last = generation.last_mut().expect("nonempty Generation bytes");
        *last ^= 1;
    });
    assert!(SealedExportV1::decode(&tampered).is_err());
}

#[test]
fn committed_seal_keeps_pending_carrier_until_closure_persists_and_recovers_idempotently() {
    let temp = TestTempDir::new("maestro-vnext-store-seal-closure-recovery");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"seal-closure-recovery")
        .expect("Repository Store domain");
    let store_path = canonical_temp(&temp).join("store");
    let mut store = StoreV1::create(&store_path, domain).expect("Store");
    let root = object(16, vec![]);
    store.put_object(&root).expect("root object");
    let generation = StoreGenerationV1::new(
        store.domain().clone(),
        1,
        None,
        contract_root(),
        compatibility(),
        vec![root.id()],
    )
    .expect("Generation");
    store
        .publish_generation(&generation, None)
        .expect("publish Generation");

    let closure_directory = store_path.join("exports/snapshot-closures");
    fs::remove_dir(&closure_directory).expect("remove empty closure directory");
    fs::write(
        &closure_directory,
        b"deterministic closure persistence fault",
    )
    .expect("replace closure directory with a file");

    let export_id = match store.seal_export() {
        Err(StoreError::BackupPublicationRecoveryRequired { export_id, .. }) => export_id,
        result => panic!("closure persistence fault must leave recovery debt: {result:?}"),
    };
    let export_hex = export_id
        .render()
        .strip_prefix("sha256:")
        .expect("export identity prefix")
        .to_owned();
    let pending_backup = store_path.join(format!("exports/.maestro-export-{export_hex}.pending"));
    let public_backup = store_path.join(format!("exports/{export_hex}.cbor"));
    assert!(pending_backup.is_file());
    assert!(!public_backup.exists());
    let expected_backup = SealedBackupV1::decode(
        &fs::read(&pending_backup).expect("committed pending backup carrier"),
    )
    .expect("exact pending backup carrier");
    assert_eq!(expected_backup.id(), export_id);
    let connection = Connection::open(store_path.join("store.sqlite3")).expect("metadata");
    let committed_receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM store_sealed_exports", [], |row| {
            row.get(0)
        })
        .expect("committed sealed-export receipt count");
    assert_eq!(committed_receipts, 1);
    drop(connection);

    fs::remove_file(&closure_directory).expect("remove closure fault carrier");
    fs::create_dir(&closure_directory).expect("restore closure directory");
    assert_eq!(
        store
            .recover_sealed_export_publication(export_id)
            .expect("recover pending committed backup"),
        expected_backup
    );
    assert!(public_backup.is_file());
    assert!(!pending_backup.exists());
    assert_eq!(
        fs::read(&public_backup).expect("public backup bytes"),
        expected_backup.canonical_bytes()
    );
    let closure_path = fs::read_dir(&closure_directory)
        .expect("closure directory")
        .next()
        .expect("persisted closure entry")
        .expect("persisted closure")
        .path();
    assert!(
        fs::read_dir(&closure_directory)
            .expect("closure directory")
            .nth(1)
            .is_none()
    );
    fs::remove_file(&closure_path).expect("simulate historical public-without-closure state");
    assert_eq!(
        store
            .recover_sealed_export_publication(export_id)
            .expect("already-public recovery replay"),
        expected_backup
    );
    assert!(closure_path.is_file());
}

#[test]
fn ordinal_three_export_restores_full_lineage_tombstones_and_an_inert_candidate() {
    let temp = TestTempDir::new("maestro-vnext-store-retention");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"retention")
        .expect("Repository Store domain");
    let source_path = canonical_temp(&temp).join("source");
    let mut source = StoreV1::create(&source_path, domain.clone()).expect("source Store");
    let old_root = object(20, vec![]);
    let dead = object(21, vec![old_root.id()]);
    let pinned = object(22, vec![]);
    let middle_root = object(23, vec![]);
    let current_root = object(24, vec![]);
    for value in [&old_root, &dead, &pinned, &middle_root, &current_root] {
        source.put_object(value).expect("authoritative object");
    }

    let generation_one = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        contract_root(),
        compatibility(),
        vec![old_root.id()],
    )
    .expect("Generation one");
    let head_one = source
        .publish_generation(&generation_one, None)
        .expect("publish Generation one");
    let pin = RetentionPinV1::new(
        head_one.id(),
        RetentionRootV1::new(RetentionRootKindV1::LegalHold, pinned.id()),
        [2; 32],
    )
    .expect("historical-basis pin");
    assert_eq!(source.add_retention_pin(&pin, 1).expect("pin"), 2);
    let tombstone = LogicalTombstoneV1::new(head_one.id(), dead.id(), [3; 32], [4; 32])
        .expect("historical-basis tombstone");
    assert_eq!(source.tombstone(&tombstone, 2).expect("tombstone"), 3);
    assert!(source.read_object(dead.id()).is_err());

    let generation_two = StoreGenerationV1::new(
        domain.clone(),
        2,
        Some(generation_one.id()),
        contract_root(),
        compatibility(),
        vec![middle_root.id()],
    )
    .expect("Generation two");
    let head_two = source
        .publish_generation(&generation_two, Some(head_one.id()))
        .expect("publish Generation two");
    let generation_three = StoreGenerationV1::new(
        domain.clone(),
        3,
        Some(generation_two.id()),
        contract_root(),
        compatibility(),
        vec![current_root.id()],
    )
    .expect("Generation three");
    let head_three = source
        .publish_generation(&generation_three, Some(head_two.id()))
        .expect("publish Generation three");

    let export = source.seal_export().expect("lineage-complete export");
    assert_eq!(export.lineage().len(), 3);
    assert_eq!(export.head(), &head_three);
    assert_eq!(export.object_inventory().len(), 5);
    assert!(export.entries().iter().any(|entry| {
        matches!(entry, SealedExportEntryV1::Tombstoned(object) if object.object_id() == dead.id() && object.references() == [old_root.id()])
    }));
    assert_eq!(
        SealedBackupV1::decode(export.canonical_bytes()).expect("offline verification"),
        export
    );

    let snapshot = source
        .snapshot_reachability()
        .expect("post-seal reachability snapshot");
    let plan = source.plan_collection(&snapshot).expect("collection plan");
    assert_eq!(source.collect(&plan).expect("physical collection"), 1);
    assert!(!source_path.join(object_relative_path(dead.id())).exists());

    let restored_path = canonical_temp(&temp).join("restored");
    let mut restored =
        StoreV1::create(&restored_path, domain.clone()).expect("inactive restore Store");
    let candidate = restored
        .import_inactive(export.canonical_bytes())
        .expect("inactive import");
    assert_eq!(
        candidate,
        restored
            .restore_candidate(candidate.id())
            .expect("persisted candidate")
    );
    assert_eq!(
        candidate,
        maestro::domain::vnext::persistence::RestoreCandidateV1::decode(
            candidate.canonical_bytes()
        )
        .expect("candidate canonical round trip")
    );
    assert_eq!(candidate.source_export_id(), export.id());
    assert_eq!(candidate.source_domain_id(), domain.id());
    assert_eq!(candidate.destination_domain_id(), domain.id());
    assert_eq!(candidate.candidate_generation_id(), generation_three.id());
    assert_eq!(candidate.candidate_head_id(), head_three.id());
    assert_eq!(
        candidate.candidate_snapshot_id(),
        export.reachability().id()
    );
    assert_eq!(candidate.candidate_roots(), &[current_root.id()]);
    assert_eq!(
        restored
            .import_inactive(export.canonical_bytes())
            .expect("exact import replay"),
        candidate
    );
    assert_eq!(
        restored.state().expect("restored state").0,
        StoreStateV1::Inactive
    );
    assert_eq!(restored.active_head().expect("inactive currentness"), None);
    assert_eq!(
        restored
            .read_object(current_root.id())
            .expect("current root"),
        current_root
    );
    assert!(restored.read_object(dead.id()).is_err());

    let connection = Connection::open(restored_path.join("store.sqlite3")).expect("metadata");
    let generation_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM store_generations", [], |row| {
            row.get(0)
        })
        .expect("generation count");
    let head_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM store_heads", [], |row| row.get(0))
        .expect("head count");
    let active_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM store_active_head", [], |row| {
            row.get(0)
        })
        .expect("active count");
    assert_eq!((generation_count, head_count, active_count), (3, 3, 0));
    drop(connection);

    drop(restored);
    let reopened = StoreV1::open(&restored_path, domain).expect("reopen restored Store");
    assert_eq!(reopened.active_head().expect("reopened currentness"), None);
    assert_eq!(
        reopened
            .restore_candidate(candidate.id())
            .expect("reopened candidate"),
        candidate
    );
}

#[test]
fn sealed_available_objects_are_intrinsic_retention_roots_for_both_store_roles() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-sealed-retention-root");
        let store_path = canonical_temp(&temp).join(role.as_str());
        let domain = StoreDomainV1::derive(role, b"sealed-retention-root").expect("Store domain");
        let mut store = StoreV1::create(&store_path, domain.clone()).expect("create Store");

        let first = object(80, vec![]);
        store.put_object(&first).expect("persist first root");
        let first_generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            contract_root(),
            compatibility(),
            vec![first.id()],
        )
        .expect("first Generation");
        let first_head = store
            .publish_generation(&first_generation, None)
            .expect("publish first Generation");
        let backup = store.seal_export().expect("commit first sealed backup");
        assert!(backup.entries().iter().any(
              |entry| matches!(entry, SealedExportEntryV1::Available(object) if object.id() == first.id())
          ));
        let mut missing_receipt_destination =
            StoreV1::create(store_path.with_extension("missing-receipt"), domain.clone())
                .expect("create missing-receipt destination");
        assert!(matches!(
            missing_receipt_destination.import_inactive(backup.export().canonical_bytes()),
            Err(maestro::domain::vnext::persistence::StoreError::Export(_))
        ));

        let second = object(81, vec![]);
        store.put_object(&second).expect("persist second root");
        let second_generation = StoreGenerationV1::new(
            domain,
            2,
            Some(first_generation.id()),
            contract_root(),
            compatibility(),
            vec![second.id()],
        )
        .expect("second Generation");
        let second_head = store
            .publish_generation(&second_generation, Some(first_head.id()))
            .expect("publish second Generation");
        let basis = store
            .snapshot_reachability()
            .expect("current retention basis");
        let tombstone = LogicalTombstoneV1::new(second_head.id(), first.id(), [82; 32], [83; 32])
            .expect("first-root tombstone");
        store
            .tombstone(&tombstone, basis.retention_revision())
            .expect("tombstone superseded root");

        let snapshot = store
            .snapshot_reachability()
            .expect("post-tombstone snapshot");
        let plan = store
            .plan_collection(&snapshot)
            .expect("sealed-aware collection plan");
        assert!(!plan.candidates().contains(&first.id()));
        assert!(store_path.join(object_relative_path(first.id())).is_file());
    }
}

#[test]
fn sealing_invalidates_preexisting_collection_plans_before_sweep() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-seal-invalidates-plan");
        let store_path = canonical_temp(&temp).join(role.as_str());
        let domain = StoreDomainV1::derive(role, b"seal-invalidates-plan").expect("Store domain");
        let mut store = StoreV1::create(&store_path, domain.clone()).expect("create Store");
        let first = object(90, vec![]);
        let second = object(91, vec![]);
        store.put_object(&first).expect("persist first root");
        store.put_object(&second).expect("persist second root");
        let first_generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            contract_root(),
            compatibility(),
            vec![first.id()],
        )
        .expect("first Generation");
        let first_head = store
            .publish_generation(&first_generation, None)
            .expect("publish first Generation");
        let second_generation = StoreGenerationV1::new(
            domain,
            2,
            Some(first_generation.id()),
            contract_root(),
            compatibility(),
            vec![second.id()],
        )
        .expect("second Generation");
        let second_head = store
            .publish_generation(&second_generation, Some(first_head.id()))
            .expect("publish second Generation");
        let basis = store.snapshot_reachability().expect("retention basis");
        let tombstone = LogicalTombstoneV1::new(second_head.id(), first.id(), [92; 32], [93; 32])
            .expect("first-root tombstone");
        store
            .tombstone(&tombstone, basis.retention_revision())
            .expect("tombstone first root");
        let before_seal = store.snapshot_reachability().expect("pre-seal snapshot");
        let stale_plan = store.plan_collection(&before_seal).expect("pre-seal plan");
        assert!(stale_plan.candidates().contains(&first.id()));

        store.seal_export().expect("seal current Store");
        assert!(matches!(
            store.collect(&stale_plan),
            Err(maestro::domain::vnext::persistence::StoreError::RetentionCasMismatch)
        ));

        let after_seal = store.snapshot_reachability().expect("post-seal snapshot");
        let fresh_plan = store.plan_collection(&after_seal).expect("post-seal plan");
        assert!(fresh_plan.candidates().contains(&first.id()));
        assert_eq!(store.collect(&fresh_plan).expect("fresh collection"), 1);
    }
}

#[test]
fn activation_commitments_are_nominally_distinct_by_store_role() {
    assert_ne!(
        std::any::TypeId::of::<RepositoryActivationIntentV1>(),
        std::any::TypeId::of::<InstallationActivationIntentV1>()
    );
}
