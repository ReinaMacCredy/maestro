mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

use maestro::domain::vnext::identity::{
    ContractRootIdV1, DescriptorIdV1, ManifestIdV1, SchemaIdV1, StoreObjectIdV1,
};
use maestro::domain::vnext::persistence::{
    ReachabilitySnapshotV1, RetentionRootKindV1, RetentionRootV1, SealedBackupV1,
    SealedExportEntryV1, SealedExportV1, StoreCompatibilityV1, StoreDomainV1, StoreError,
    StoreGenerationV1, StoreHeadV1, StoreObjectV1, StoreRoleV1, StoreStateV1, StoreV1,
};
use maestro::foundation::core::deterministic_cbor::CborValue;
use rusqlite::Connection;

use support::TestTempDir;

const DURABILITY_CHILD_PATH: &str = "MAESTRO_VNEXT_DURABILITY_CHILD_PATH";
const DURABILITY_CHILD_ROLE: &str = "MAESTRO_VNEXT_DURABILITY_CHILD_ROLE";

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn schema(byte: u8) -> SchemaIdV1 {
    SchemaIdV1::parse(&rendered(byte)).expect("valid Schema identity")
}

fn contract_root() -> ContractRootIdV1 {
    ContractRootIdV1::parse(&rendered(2)).expect("valid Contract Root identity")
}

fn compatibility() -> StoreCompatibilityV1 {
    StoreCompatibilityV1::stage0_successor().expect("frozen Stage 0 successor bindings")
}

fn predecessor_compatibility() -> StoreCompatibilityV1 {
    StoreCompatibilityV1::new(
        ManifestIdV1::parse(
            "sha256:60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6",
        )
        .expect("frozen predecessor writer compatibility Manifest"),
        SchemaIdV1::parse(
            "sha256:fddd9d43b7f8662187b834a64ef5fb0ba96b2182b6218c1a2c1b5aaca0e26808",
        )
        .expect("frozen predecessor association Schema"),
        ManifestIdV1::parse(
            "sha256:026b61dd18923e40917167af14737124ec11b1cabdb69fdb2422bb50d4a80466",
        )
        .expect("frozen predecessor finality-edge Manifest"),
        DescriptorIdV1::parse(
            "sha256:99333b038139e952f55ae22bd82383679a978ce8c2559ac44eeaebc15b3addec",
        )
        .expect("frozen predecessor read-write-set Descriptor"),
        DescriptorIdV1::parse(
            "sha256:f3e6d7c105193f278bcfdd744d7b715358a59ffc8b7b02c3f17fe1592d1c6e6b",
        )
        .expect("frozen predecessor writer-protocol Descriptor"),
        DescriptorIdV1::parse(
            "sha256:95d517009025279d79108c8cf81418cf101ff77fedd333326fde03ac223e0a69",
        )
        .expect("frozen predecessor migration-epoch Descriptor"),
    )
}

fn mixed_compatibility() -> StoreCompatibilityV1 {
    let current = compatibility();
    let predecessor = predecessor_compatibility();
    StoreCompatibilityV1::new(
        current.writer_compatibility_manifest_id(),
        current.association_schema_id(),
        current.finality_edge_manifest_id(),
        current.schema_read_write_set_descriptor_id(),
        current.writer_protocol_epoch_id(),
        predecessor.migration_epoch_id(),
    )
}

fn domain(role: StoreRoleV1, key: &str) -> StoreDomainV1 {
    StoreDomainV1::derive(role, key.as_bytes()).expect("valid Store domain")
}

fn object(seed: u64) -> StoreObjectV1 {
    StoreObjectV1::new(
        schema(1),
        CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(seed)]),
        vec![],
    )
    .expect("valid Store Object")
}

fn canonical_temp(temp: &TestTempDir) -> PathBuf {
    fs::canonicalize(temp.path()).expect("canonical temporary directory")
}

fn object_relative_path(object_id: StoreObjectIdV1) -> PathBuf {
    let rendered = object_id.render();
    let hex = rendered
        .strip_prefix("sha256:")
        .expect("identity rendering prefix");
    Path::new("objects")
        .join(&hex[..2])
        .join(format!("{hex}.cbor"))
}

fn export_relative_path(export: &SealedExportV1) -> PathBuf {
    let rendered = export.id().render();
    let hex = rendered
        .strip_prefix("sha256:")
        .expect("identity rendering prefix");
    Path::new("exports").join(format!("{hex}.cbor"))
}

fn generation(
    domain: StoreDomainV1,
    ordinal: u64,
    previous: Option<maestro::domain::vnext::identity::StoreGenerationIdV1>,
    root: StoreObjectIdV1,
) -> StoreGenerationV1 {
    StoreGenerationV1::new(
        domain,
        ordinal,
        previous,
        contract_root(),
        compatibility(),
        vec![root],
    )
    .expect("valid Store Generation")
}

fn publish_initial(
    store: &mut StoreV1,
    domain: StoreDomainV1,
    seed: u64,
) -> (StoreObjectV1, StoreGenerationV1, StoreHeadV1) {
    let root = object(seed);
    store.put_object(&root).expect("persist root object");
    let generation = generation(domain, 1, None, root.id());
    let head = store
        .publish_generation(&generation, None)
        .expect("publish initial Generation");
    (root, generation, head)
}

fn sealed_export(path: &Path, domain: StoreDomainV1, seed: u64) -> SealedBackupV1 {
    let mut store = StoreV1::create(path, domain.clone()).expect("create export source Store");
    publish_initial(&mut store, domain, seed);
    store.seal_export().expect("seal source export")
}

fn carrier_with_compatibility(
    domain: StoreDomainV1,
    seed: u64,
    compatibility: StoreCompatibilityV1,
) -> SealedExportV1 {
    let root = object(seed);
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        contract_root(),
        compatibility,
        vec![root.id()],
    )
    .expect("valid Generation carrier");
    let head = StoreHeadV1::new(&generation, 1, None).expect("valid Head carrier");
    let reachability = ReachabilitySnapshotV1::new(
        head.id(),
        1,
        vec![RetentionRootV1::new(
            RetentionRootKindV1::ActiveGeneration,
            root.id(),
        )],
        vec![root.id()],
        vec![],
    )
    .expect("valid reachability carrier");
    SealedExportV1::new(
        generation,
        head,
        reachability,
        vec![],
        vec![SealedExportEntryV1::Available(root)],
    )
    .expect("valid sealed export carrier")
}

fn activate_with_sqlite(store_path: &Path) {
    let connection = Connection::open(store_path.join("store.sqlite3")).expect("Store metadata");
    let changed = connection
        .execute(
            "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
            [],
        )
        .expect("test-only activation transition");
    assert_eq!(changed, 1);
}

#[test]
fn two_contenders_have_exactly_one_non_aba_generation_winner() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-two-contenders");
        let store_path = canonical_temp(&temp).join(role.as_str());
        let domain = domain(role, "two-contenders");
        let mut setup = StoreV1::create(&store_path, domain.clone()).expect("create Store");
        let (_, first_generation, first_head) = publish_initial(&mut setup, domain.clone(), 10);
        let roots = [object(11), object(12)];
        for root in &roots {
            setup.put_object(root).expect("persist contender root");
        }
        let root_ids = roots.each_ref().map(StoreObjectV1::id);
        let contenders =
            root_ids.map(|root| generation(domain.clone(), 2, Some(first_generation.id()), root));
        let expected_head_id = first_head.id();
        drop(setup);

        let barrier = Arc::new(Barrier::new(3));
        let handles = contenders
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, contender)| {
                let barrier = Arc::clone(&barrier);
                let store_path = store_path.clone();
                let domain = domain.clone();
                thread::spawn(move || {
                    let mut store =
                        StoreV1::open(&store_path, domain).expect("open contender Store");
                    barrier.wait();
                    (
                        index,
                        store.publish_generation(&contender, Some(expected_head_id)),
                    )
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("contender thread"))
            .collect::<Vec<_>>();

        let winners = results
            .iter()
            .filter_map(|(index, result)| result.as_ref().ok().map(|head| (*index, head.clone())))
            .collect::<Vec<_>>();
        assert_eq!(winners.len(), 1, "exactly one contender must publish");
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_err()).count(),
            1,
            "the stale expected-old contender must be refused"
        );

        let (winner_index, winner_head) = &winners[0];
        let mut reopened = StoreV1::open(&store_path, domain).expect("reopen Store after race");
        let exported = reopened.seal_export().expect("read winning lineage");
        assert_eq!(exported.head().id(), winner_head.id());
        assert_eq!(exported.generation(), &contenders[*winner_index]);
        assert_eq!(exported.generation().roots(), &[root_ids[*winner_index]]);
    }
}

#[test]
fn inactive_store_reopens_without_runtime_currentness_for_both_roles() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-inactive-reopen");
        let store_path = canonical_temp(&temp).join(role.as_str());
        let domain = domain(role, "inactive-reopen");
        let mut store = StoreV1::create(&store_path, domain.clone()).expect("create Store");
        let (_, generation, _) = publish_initial(&mut store, domain.clone(), 20);
        drop(store);

        let mut reopened = StoreV1::open(&store_path, domain).expect("reopen inactive Store");
        assert_eq!(
            reopened.state().expect("Store state").0,
            StoreStateV1::Inactive
        );
        assert_eq!(reopened.active_head().expect("runtime currentness"), None);
        assert_eq!(
            reopened
                .seal_export()
                .expect("staged lineage remains durable")
                .generation(),
            &generation
        );
    }
}

#[cfg(unix)]
#[test]
fn open_handle_refuses_root_rename_and_same_path_replacement() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-root-binding");
        let base = canonical_temp(&temp);
        let store_path = base.join(role.as_str());
        let moved_path = base.join(format!("{}-moved", role.as_str()));
        let domain = domain(role, "root-binding");
        let original = StoreV1::create(&store_path, domain.clone()).expect("create Store");

        fs::rename(&store_path, &moved_path).expect("rename open Store root");
        assert!(
            original.state().is_err(),
            "renamed root binding must be refused"
        );

        let replacement =
            StoreV1::create(&store_path, domain).expect("create same-path replacement Store");
        assert!(
            original.state().is_err(),
            "same-path replacement must not rebind the open handle"
        );
        drop(replacement);
    }
}

#[cfg(unix)]
#[test]
fn open_store_refuses_hard_link_substitution_of_live_sqlite_sidecars() {
    for suffix in ["-wal", "-shm"] {
        let temp = TestTempDir::new("maestro-vnext-store-live-sidecar-hard-link");
        let base = canonical_temp(&temp);
        let store_path = base.join(suffix.trim_start_matches('-'));
        let domain = domain(StoreRoleV1::Repository, &format!("live-sidecar{suffix}"));
        let mut store = StoreV1::create(&store_path, domain.clone()).expect("create Store");
        let (root, _, _) = publish_initial(&mut store, domain, 29);
        let sidecar = store_path.join(format!("store.sqlite3{suffix}"));
        assert!(
            sidecar.is_file(),
            "SQLite must expose live {suffix} sidecar"
        );
        fs::hard_link(&sidecar, base.join(format!("sidecar-alias{suffix}")))
            .expect("create unsafe sidecar hard-link alias");

        assert!(
            store.state().is_err(),
            "Store metadata reads must refuse aliased {suffix}"
        );
        assert!(
            store.read_object(root.id()).is_err(),
            "Store object reads must refuse aliased {suffix}"
        );
    }
}

#[test]
fn active_store_reopen_refuses_missing_or_corrupt_root_bytes() {
    for role in StoreRoleV1::ALL {
        for corruption in ["missing", "corrupt"] {
            let temp = TestTempDir::new("maestro-vnext-store-active-root-corruption");
            let store_path = canonical_temp(&temp).join(format!("{}-{corruption}", role.as_str()));
            let domain = domain(role, &format!("active-root-{corruption}"));
            let mut store = StoreV1::create(&store_path, domain.clone()).expect("create Store");
            let (root, _, _) = publish_initial(&mut store, domain.clone(), 30);
            drop(store);
            activate_with_sqlite(&store_path);

            let root_path = store_path.join(object_relative_path(root.id()));
            if corruption == "missing" {
                fs::remove_file(&root_path).expect("remove active root bytes");
            } else {
                fs::write(&root_path, b"corrupt active root bytes")
                    .expect("corrupt active root bytes");
            }
            assert!(
                StoreV1::open(&store_path, domain).is_err(),
                "active Store must not reopen with {corruption} root bytes"
            );
        }
    }
}

#[test]
fn mixed_and_predecessor_compatibility_exports_are_refused_on_import() {
    for role in StoreRoleV1::ALL {
        for (label, incompatible) in [
            ("predecessor", predecessor_compatibility()),
            ("mixed", mixed_compatibility()),
        ] {
            let temp = TestTempDir::new("maestro-vnext-store-compatibility-refusal");
            let domain = domain(role, &format!("compatibility-{label}"));
            let export = carrier_with_compatibility(domain.clone(), 40, incompatible);
            assert_eq!(
                SealedExportV1::decode(export.canonical_bytes()).expect("public export carrier"),
                export
            );
            let mut destination = StoreV1::create(
                canonical_temp(&temp).join(format!("{}-{label}", role.as_str())),
                domain,
            )
            .expect("create import destination");
            assert!(matches!(
                destination.import_inactive(export.canonical_bytes()),
                Err(StoreError::Export(_))
            ));
        }
    }
}

#[test]
fn compatible_legacy_partial_export_decodes_but_is_refused_on_import() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-legacy-partial-refusal");
        let domain = domain(role, "legacy-partial-refusal");
        let legacy = carrier_with_compatibility(domain.clone(), 41, compatibility());
        let decoded =
            SealedExportV1::decode(legacy.canonical_bytes()).expect("compatible legacy V1 decode");
        assert_eq!(decoded, legacy);
        assert!(decoded.snapshot_root().is_none());

        let mut destination = StoreV1::create(canonical_temp(&temp).join(role.as_str()), domain)
            .expect("create import destination");
        assert!(matches!(
            destination.import_inactive(legacy.canonical_bytes()),
            Err(StoreError::Export(_))
        ));
    }
}

#[test]
fn import_refuses_destination_domain_mismatch_for_both_roles() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-domain-mismatch");
        let base = canonical_temp(&temp);
        let source_domain = domain(role, "domain-mismatch-source");
        let export = sealed_export(&base.join("source"), source_domain, 50);
        let mut destination = StoreV1::create(
            base.join("destination"),
            domain(role, "domain-mismatch-destination"),
        )
        .expect("create mismatched destination");
        assert!(matches!(
            destination.import_inactive(export.canonical_bytes()),
            Err(StoreError::DomainMismatch)
        ));
    }
}

#[test]
fn import_exact_replay_is_idempotent_but_changed_export_conflicts() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-import-replay");
        let base = canonical_temp(&temp);
        let domain = domain(role, "import-replay");
        let first = sealed_export(&base.join("source-first"), domain.clone(), 60);
        let changed = sealed_export(&base.join("source-changed"), domain.clone(), 61);
        assert_ne!(first.id(), changed.id());

        let mut destination =
            StoreV1::create(base.join("destination"), domain).expect("create destination Store");
        let candidate = destination
            .import_inactive(first.canonical_bytes())
            .expect("first import");
        assert_eq!(
            destination
                .import_inactive(first.canonical_bytes())
                .expect("exact import replay"),
            candidate
        );
        assert!(matches!(
            destination.import_inactive(changed.canonical_bytes()),
            Err(StoreError::RestoreRequiresEmptyStore)
        ));
    }
}

#[test]
fn corrupt_database_never_falls_back_to_persisted_export_bytes() {
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-no-db-fallback");
        let store_path = canonical_temp(&temp).join(role.as_str());
        let domain = domain(role, "no-db-fallback");
        let mut store = StoreV1::create(&store_path, domain.clone()).expect("create Store");
        publish_initial(&mut store, domain.clone(), 70);
        let export = store.seal_export().expect("persist sealed export bytes");
        let export_path = store_path.join(export_relative_path(&export));
        assert_eq!(
            fs::read(&export_path).expect("persisted export"),
            export.canonical_bytes()
        );
        drop(store);

        for suffix in ["-wal", "-shm"] {
            let sidecar = store_path.join(format!("store.sqlite3{suffix}"));
            if sidecar.exists() {
                fs::remove_file(sidecar).expect("remove closed SQLite sidecar");
            }
        }
        fs::write(store_path.join("store.sqlite3"), b"not a SQLite database")
            .expect("corrupt Store metadata");

        assert!(StoreV1::open(&store_path, domain.clone()).is_err());
        assert!(StoreV1::create(&store_path, domain).is_err());
        assert_eq!(
            fs::read(export_path).expect("export remains present"),
            export.canonical_bytes(),
            "Store admission must not reconstruct corrupt metadata from export bytes"
        );
    }
}

#[test]
fn subprocess_durability_worker() {
    let Some(store_path) = std::env::var_os(DURABILITY_CHILD_PATH) else {
        return;
    };
    let role = match std::env::var(DURABILITY_CHILD_ROLE).as_deref() {
        Ok("repository") => StoreRoleV1::Repository,
        Ok("installation") => StoreRoleV1::Installation,
        other => panic!("invalid durability child role: {other:?}"),
    };
    let domain = domain(role, "subprocess-durability");
    let mut store = StoreV1::create(store_path, domain.clone()).expect("child creates Store");
    publish_initial(&mut store, domain, 80 + role.tag());
    drop(store);
}

#[test]
fn clean_subprocess_exit_is_durable_and_reopens_for_both_roles() {
    let test_binary = std::env::current_exe().expect("current integration test binary");
    for role in StoreRoleV1::ALL {
        let temp = TestTempDir::new("maestro-vnext-store-subprocess-durability");
        let store_path = canonical_temp(&temp).join(role.as_str());
        let output = Command::new(&test_binary)
            .args(["--exact", "subprocess_durability_worker", "--nocapture"])
            .env(DURABILITY_CHILD_PATH, &store_path)
            .env(DURABILITY_CHILD_ROLE, role.as_str())
            .output()
            .expect("spawn durability child test process");
        assert!(
            output.status.success(),
            "durability child failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let domain = domain(role, "subprocess-durability");
        let mut reopened = StoreV1::open(&store_path, domain).expect("parent reopens child Store");
        assert_eq!(
            reopened.state().expect("durable Store state").0,
            StoreStateV1::Inactive
        );
        assert_eq!(reopened.active_head().expect("runtime currentness"), None);
        assert_eq!(
            reopened
                .seal_export()
                .expect("durable child lineage")
                .generation()
                .roots(),
            &[object(80 + role.tag()).id()]
        );
    }
}
