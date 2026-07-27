use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    QuarantineMaterializationErrorV1, consume_foundation_census_v2, import_inactive_store,
    materialize_sealed_quarantine,
};
use crate::domain::vnext::execution::{
    ActiveStoreEffectSnapshotV1, ActiveStoreEffectWithdrawalOutcomeV1,
};
use crate::domain::vnext::identity::{ContractRootIdV1, SchemaIdV1};
use crate::domain::vnext::migration::runtime::{
    ByteTotalInventoryV1, CancellationClassificationV1, ClassificationErrorV1, ClassificationSetV1,
    ClientAdmissionV1, ClientRefusalReasonV1, ConsumerAccessV1, ConsumerCensusEntryV1,
    ConsumerClosureErrorV1, ConsumerClosureV1, ConsumerGateStageV1, ConsumerGenerationV1,
    ConsumerRecordV1, ConsumerSubjectV1, CutoverAcceptanceV1, DeclaredRootV1,
    DeterministicIdentityMapV1, EffectCrossingV1, IdentityMapEntryV1, IdentityMappingBasisV1,
    InactiveImportErrorV1, InactiveStoreImportReceiptV1, InactiveStoreImportRequestV1,
    InventoryDomainV1, InventoryNodeKindV1, InventoryPayloadV1, InventoryRowV1,
    MigrationAssociationErrorV1, MigrationAssociationMeaningV1, MigrationAssociationV1,
    MigrationDigestV1, MigrationDispositionV1, MigrationIdentityErrorV1,
    MigrationProtocolClosureV1, NativeCancellationCausalJoinV1, NormalizedLocatorV1,
    PrunePrerequisitesV1, QuarantineEntryV1, QuarantineErrorV1, RollbackAssessmentErrorV1,
    RollbackAssessmentV1, RollbackDispositionV1, SealedQuarantineManifestV1,
    SourceClassificationV1, Stage9CutoverAssociationAdapterV1,
    Stage9Stage10ConsumerCensusAdapterV1, Stage9Stage10CutoverHostAdapterV1,
    TestOnlyStage9CutoverFinalityV1,
};
use crate::domain::vnext::migration::{
    ActiveStoreAtomicParticipantV1, ActiveStoreFinalityPartsV1, ActiveStoreFinalityV1,
    ActiveStoreOwningHeadV1, ActiveStorePreconditionV1, CutoverCommitmentV1, CutoverDomainRefV1,
    CutoverDomainV1, MigrationCutoverAssociationV1, MigrationCutoverContextV1,
    MigrationCutoverMaterialV1, ReleaseBindingV1,
};
use crate::domain::vnext::persistence::{
    StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreObjectV1, StoreRoleV1,
    StoreStateV1, StoreV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

const CENSUS_V2_SOURCE: &str = include_str!("census.rs");

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_directory =
            std::fs::canonicalize(std::env::temp_dir()).expect("resolve existing temp directory");
        let path = temp_directory.join(format!(
            "maestro-vnext-stage11-inactive-import-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create Stage-11 test root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rendered(byte: u8) -> String {
    let octet = format!("{byte:02x}");
    format!("sha256:{}", octet.repeat(32))
}

fn digest(byte: u8) -> MigrationDigestV1 {
    MigrationDigestV1::from_digest([byte; 32]).expect("nonzero test digest")
}

fn locator(path: &Path) -> NormalizedLocatorV1 {
    NormalizedLocatorV1::new(path.as_os_str().as_bytes().to_vec()).expect("normalized locator")
}

#[derive(Clone)]
struct TestOnlyConsumerCensusAdapterV1 {
    entries: Vec<ConsumerCensusEntryV1>,
}

impl Stage9Stage10ConsumerCensusAdapterV1 for TestOnlyConsumerCensusAdapterV1 {
    fn authoritative_census_facts(
        &self,
    ) -> Result<(usize, [MigrationDigestV1; 3], Vec<ConsumerCensusEntryV1>), ConsumerClosureErrorV1>
    {
        Ok((
            self.entries.len(),
            [digest(90), digest(91), digest(92)],
            self.entries.clone(),
        ))
    }
}

#[derive(Clone)]
struct TestOnlyStage9AssociationAdapterV1 {
    domain_ref: CutoverDomainRefV1,
    release: ReleaseBindingV1,
    context: MigrationCutoverContextV1,
    distribution_receipt_id: MigrationDigestV1,
    candidate_store_root_id: MigrationDigestV1,
    schema_read_write_set_id: MigrationDigestV1,
    writer_protocol_epoch_id: MigrationDigestV1,
    migration_epoch_id: MigrationDigestV1,
}

impl Stage9CutoverAssociationAdapterV1 for TestOnlyStage9AssociationAdapterV1 {
    fn cutover_finality(
        &self,
        meaning: &MigrationAssociationMeaningV1,
    ) -> Result<TestOnlyStage9CutoverFinalityV1, MigrationAssociationErrorV1> {
        let association_id = CutoverCommitmentV1::new(meaning.id().into_bytes())?;
        let association = MigrationCutoverAssociationV1::new(
            self.domain_ref.clone(),
            self.release.clone(),
            self.context.clone(),
            MigrationCutoverMaterialV1 {
                association_id,
                inventory_id: CutoverCommitmentV1::new(meaning.inventory_id().into_bytes())?,
                target_set_id: CutoverCommitmentV1::new(meaning.target_set_id().into_bytes())?,
                quarantine_set_id: CutoverCommitmentV1::new(
                    meaning.quarantine_set_id().into_bytes(),
                )?,
                consumer_set_id: CutoverCommitmentV1::new(meaning.consumer_set_id().into_bytes())?,
                distribution_receipt_id: CutoverCommitmentV1::new(
                    self.distribution_receipt_id.into_bytes(),
                )?,
                candidate_store_root_id: CutoverCommitmentV1::new(
                    self.candidate_store_root_id.into_bytes(),
                )?,
                schema_read_write_set_id: CutoverCommitmentV1::new(
                    self.schema_read_write_set_id.into_bytes(),
                )?,
                writer_protocol_epoch_id: CutoverCommitmentV1::new(
                    self.writer_protocol_epoch_id.into_bytes(),
                )?,
                migration_epoch_id: CutoverCommitmentV1::new(self.migration_epoch_id.into_bytes())?,
            },
        )?;
        let MigrationCutoverContextV1::ActiveStore {
            distribution_commit_record_id,
        } = &self.context
        else {
            return Err(MigrationAssociationErrorV1::ExternalBindingMismatch);
        };
        let distribution_commit_record_id = *distribution_commit_record_id;
        let distribution_receipt_id =
            CutoverCommitmentV1::new(self.distribution_receipt_id.into_bytes())?;
        Ok(TestOnlyStage9CutoverFinalityV1::ActiveStore(
            ActiveStoreFinalityV1::new(ActiveStoreFinalityPartsV1 {
                association,
                ordered_preconditions: vec![
                    ActiveStorePreconditionV1::DistributionReceipt(distribution_receipt_id),
                    ActiveStorePreconditionV1::DistributionCommitRecord {
                        commit_record_id: distribution_commit_record_id,
                        receipt_id: distribution_receipt_id,
                    },
                ],
                atomic_participants: vec![
                    ActiveStoreAtomicParticipantV1::Association(association_id),
                    ActiveStoreAtomicParticipantV1::OwningHead(ActiveStoreOwningHeadV1 {
                        association_id,
                        distribution_commit_record_id,
                        distribution_receipt_id,
                        domain_ref: self.domain_ref.clone(),
                        release: self.release.clone(),
                        candidate_store_root_id: CutoverCommitmentV1::new(
                            self.candidate_store_root_id.into_bytes(),
                        )?,
                    }),
                ],
            })?,
        ))
    }
}

struct TestOnlyStage9Stage10CutoverHostAdapterV1 {
    observed_host_attempt_id: Option<MigrationDigestV1>,
    acceptance: CutoverAcceptanceV1,
    effect_crossing: EffectCrossingV1,
}

impl Stage9Stage10CutoverHostAdapterV1 for TestOnlyStage9Stage10CutoverHostAdapterV1 {
    fn cutover_host_facts(
        &self,
        _cutover_attempt_id: MigrationDigestV1,
    ) -> Result<
        (
            Option<MigrationDigestV1>,
            CutoverAcceptanceV1,
            EffectCrossingV1,
        ),
        RollbackAssessmentErrorV1,
    > {
        Ok((
            self.observed_host_attempt_id,
            self.acceptance,
            self.effect_crossing,
        ))
    }
}

fn test_protocol() -> MigrationProtocolClosureV1 {
    MigrationProtocolClosureV1::new(
        digest(1),
        digest(2),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
        digest(7),
        None,
    )
    .expect("test protocol")
}

fn small_inventory() -> ByteTotalInventoryV1 {
    let root_locator = NormalizedLocatorV1::new(b"/stage11/source".to_vec()).expect("root locator");
    let root = DeclaredRootV1::new(
        root_locator.clone(),
        root_locator,
        InventoryDomainV1::Repository,
        false,
    )
    .expect("declared root");
    let first_locator =
        NormalizedLocatorV1::new(b"/stage11/source/first".to_vec()).expect("first locator");
    let second_locator =
        NormalizedLocatorV1::new(b"/stage11/source/second".to_vec()).expect("second locator");
    let first = InventoryRowV1::new(
        root.id(),
        first_locator.clone(),
        first_locator,
        InventoryDomainV1::Repository,
        InventoryNodeKindV1::RegularFile,
        InventoryPayloadV1::from_bytes(b"first-v1-bytes").expect("payload"),
        digest(60),
    )
    .expect("first row");
    let second = InventoryRowV1::new(
        root.id(),
        second_locator.clone(),
        second_locator,
        InventoryDomainV1::Repository,
        InventoryNodeKindV1::RegularFile,
        InventoryPayloadV1::from_bytes(b"second-v1-bytes").expect("payload"),
        digest(61),
    )
    .expect("second row");
    ByteTotalInventoryV1::new(vec![root], vec![second, first]).expect("byte-total inventory")
}

#[test]
fn test_only_consumer_adapter_rejects_empty_membership() {
    let empty = TestOnlyConsumerCensusAdapterV1 { entries: vec![] };
    let refused = ConsumerClosureV1::evaluate_from_adapter(
        ConsumerGateStageV1::BeforeSemanticCurrentness,
        test_protocol(),
        &empty,
        PrunePrerequisitesV1::blocked(),
    );
    assert!(matches!(
        refused,
        Err(ConsumerClosureErrorV1::InvalidAuthoritativeCensus)
    ));
}

#[test]
fn test_only_h3_adapter_accepts_typed_stage4_publication_inputs() {
    let constructor: fn(
        MigrationDigestV1,
        MigrationDigestV1,
        &ActiveStoreEffectSnapshotV1,
        &ActiveStoreEffectWithdrawalOutcomeV1,
        &ActiveStoreEffectSnapshotV1,
    ) -> Result<NativeCancellationCausalJoinV1, ClassificationErrorV1> =
        NativeCancellationCausalJoinV1::test_only_from_stage4_publication;
    let _ = constructor;
}

#[test]
fn zero_migration_identities_are_unconstructible() {
    assert_eq!(
        MigrationDigestV1::from_digest([0; 32]),
        Err(MigrationIdentityErrorV1::ZeroDigest)
    );
    assert_eq!(
        MigrationDigestV1::parse_hex(&"0".repeat(64)),
        Err(MigrationIdentityErrorV1::ZeroDigest)
    );
}

#[test]
fn inventory_classification_identity_and_cancel_label_are_total_and_deterministic() {
    let inventory = small_inventory();
    assert_eq!(inventory.byte_count(), 29);
    let classifications = ClassificationSetV1::new(
        &inventory,
        inventory
            .rows()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                SourceClassificationV1::new(
                    row.source_id(),
                    MigrationDispositionV1::OpaquePreserved,
                    digest(70 + index as u8),
                    Some(digest(80 + index as u8)),
                    None,
                    false,
                    CancellationClassificationV1::NotCancellationLike,
                )
                .expect("classification")
            })
            .rev()
            .collect(),
    )
    .expect("complete classifications");
    let map = DeterministicIdentityMapV1::new(
        &classifications,
        classifications
            .rows()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                IdentityMapEntryV1::new(
                    row.source_id(),
                    row.target_id().expect("target"),
                    IdentityMappingBasisV1::HistoricalOpaque {
                        preservation_proof_id: digest(90 + index as u8),
                    },
                )
                .expect("identity-map row")
            })
            .rev()
            .collect(),
    )
    .expect("identity map");
    assert_eq!(
        map.id(),
        DeterministicIdentityMapV1::new(&classifications, map.rows().to_vec())
            .expect("identity map replay")
            .id()
    );
    assert!(matches!(
        ClassificationSetV1::new(&inventory, vec![classifications.rows()[0].clone()]),
        Err(ClassificationErrorV1::ClassificationCoverageMismatch)
    ));
    assert!(matches!(
        SourceClassificationV1::new(
            inventory.rows()[0].source_id(),
            MigrationDispositionV1::MappedNormative,
            digest(100),
            Some(digest(101)),
            None,
            false,
            CancellationClassificationV1::CancelLikeLabelNonPromoting,
        ),
        Err(ClassificationErrorV1::CancellationLabelCannotPromote)
    ));
}

#[test]
fn consumer_census_refuses_old_mixed_unknown_and_wrong_release_clients() {
    let expected = MigrationProtocolClosureV1::new(
        digest(1),
        digest(2),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
        digest(7),
        Some(digest(8)),
    )
    .expect("expected protocol");
    let client = |path: &str, subject, generation, protocol: Option<MigrationProtocolClosureV1>| {
        ConsumerRecordV1::new(
            NormalizedLocatorV1::new(path.as_bytes().to_vec()).expect("consumer locator"),
            subject,
            generation,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            protocol,
        )
        .expect("consumer")
    };
    let mixed = MigrationProtocolClosureV1::new(
        digest(11),
        digest(12),
        digest(13),
        digest(14),
        digest(15),
        digest(16),
        digest(17),
        Some(digest(8)),
    )
    .expect("mixed protocol");
    let wrong_release = MigrationProtocolClosureV1::new(
        digest(1),
        digest(2),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
        digest(7),
        Some(digest(9)),
    )
    .expect("wrong Release protocol");
    let records = vec![
        client(
            "/stage11/old",
            ConsumerSubjectV1::LegacySource,
            ConsumerGenerationV1::LegacyV1,
            None,
        ),
        client(
            "/stage11/mixed",
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::CurrentVNext,
            Some(mixed),
        ),
        client(
            "/stage11/unknown",
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::Unknown,
            None,
        ),
        client(
            "/stage11/wrong-release",
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::CurrentVNext,
            Some(wrong_release),
        ),
    ];
    let adapter = TestOnlyConsumerCensusAdapterV1 {
        entries: records
            .into_iter()
            .enumerate()
            .map(|(index, record)| {
                ConsumerCensusEntryV1::observed(digest(30 + index as u8), record)
            })
            .collect(),
    };
    let closure = ConsumerClosureV1::evaluate_from_adapter(
        ConsumerGateStageV1::BeforeSemanticCurrentness,
        expected,
        &adapter,
        PrunePrerequisitesV1::blocked(),
    )
    .expect("consumer closure");
    assert!(!closure.gate_passed());
    let admissions = closure
        .admissions()
        .iter()
        .map(|(_, admission)| *admission)
        .collect::<Vec<_>>();
    for reason in [
        ClientRefusalReasonV1::OldProtocol,
        ClientRefusalReasonV1::MixedProtocol,
        ClientRefusalReasonV1::UnknownProtocol,
        ClientRefusalReasonV1::ReleaseMismatch,
    ] {
        assert!(admissions.contains(&ClientAdmissionV1::RefusedBeforeCurrentness(reason)));
    }
}

#[test]
fn physical_pruning_requires_removed_members_and_all_external_proofs() {
    let adapter = TestOnlyConsumerCensusAdapterV1 {
        entries: vec![
            ConsumerCensusEntryV1::removed(digest(30), digest(31), digest(32))
                .expect("removed member"),
        ],
    };
    let blocked = ConsumerClosureV1::evaluate_from_adapter(
        ConsumerGateStageV1::PhysicalPruning,
        test_protocol(),
        &adapter,
        PrunePrerequisitesV1::blocked(),
    )
    .expect("blocked pruning closure");
    assert!(!blocked.gate_passed());
    let complete = PrunePrerequisitesV1::new(
        Some(digest(40)),
        Some(digest(41)),
        Some(digest(42)),
        Some(digest(43)),
        Some(digest(44)),
    )
    .expect("complete prune proofs");
    let passed = ConsumerClosureV1::evaluate_from_adapter(
        ConsumerGateStageV1::PhysicalPruning,
        test_protocol(),
        &adapter,
        complete,
    )
    .expect("complete pruning closure");
    assert!(passed.gate_passed());
}

#[test]
fn protected_noninterpreting_retention_does_not_block_currentness() {
    let hold = ConsumerRecordV1::new(
        NormalizedLocatorV1::new(b"/stage11/protected-retention".to_vec())
            .expect("retention locator"),
        ConsumerSubjectV1::LegacySource,
        ConsumerGenerationV1::LegacyV1,
        ConsumerAccessV1::ProtectedRetentionHold,
        false,
        false,
        None,
    )
    .expect("protected retention hold");
    let adapter = TestOnlyConsumerCensusAdapterV1 {
        entries: vec![ConsumerCensusEntryV1::observed(digest(45), hold)],
    };
    let closure = ConsumerClosureV1::evaluate_from_adapter(
        ConsumerGateStageV1::BeforeSemanticCurrentness,
        test_protocol(),
        &adapter,
        PrunePrerequisitesV1::blocked(),
    )
    .expect("retention closure");
    assert!(closure.gate_passed());
    assert_eq!(
        closure.admissions()[0].1,
        ClientAdmissionV1::OpaqueSealedOnly
    );
}

#[test]
fn rollback_is_pre_accept_only_and_stale_hosts_refuse() {
    let attempt = digest(50);
    let eligible_host = TestOnlyStage9Stage10CutoverHostAdapterV1 {
        observed_host_attempt_id: Some(attempt),
        acceptance: CutoverAcceptanceV1::PreAccept,
        effect_crossing: EffectCrossingV1::ProvenNotCrossed,
    };
    let eligible = RollbackAssessmentV1::from_cutover_host_adapter(attempt, &eligible_host)
        .expect("pre-accept rollback");
    assert_eq!(
        eligible.disposition(),
        RollbackDispositionV1::ProtectedExactV1RollbackEligible
    );
    assert_eq!(
        RollbackAssessmentV1::from_cutover_host_adapter(
            attempt,
            &TestOnlyStage9Stage10CutoverHostAdapterV1 {
                observed_host_attempt_id: Some(attempt),
                acceptance: CutoverAcceptanceV1::Accepted,
                effect_crossing: EffectCrossingV1::PossibleOrUnknown,
            },
        )
        .expect("accepted recovery")
        .disposition(),
        RollbackDispositionV1::VNextFreshGenerationRecoveryOnly
    );
    assert_eq!(
        RollbackAssessmentV1::from_cutover_host_adapter(
            attempt,
            &TestOnlyStage9Stage10CutoverHostAdapterV1 {
                observed_host_attempt_id: Some(digest(51)),
                acceptance: CutoverAcceptanceV1::PreAccept,
                effect_crossing: EffectCrossingV1::ProvenNotCrossed,
            },
        )
        .expect("stale host refusal")
        .disposition(),
        RollbackDispositionV1::RefusedStaleHost
    );
}

#[test]
fn production_census_consumes_only_the_foundation_v2_continuation() {
    let _ = consume_foundation_census_v2;
    for required in [
        "MigrationClassificationContinuationV2",
        "continuation.consume_for_stage11()",
        "Stage11CensusContinuationV2",
        "ProtectedLocatorLeaseV2",
        "stage11_finality_v2",
    ] {
        assert!(CENSUS_V2_SOURCE.contains(required), "missing {required}");
    }
    for forbidden in [
        "DeclaredRootScanV1",
        "recensus_declared_roots",
        "PathBuf",
        "for_admitted_root_set",
        "descriptor_census_platform::census",
    ] {
        assert!(
            !CENSUS_V2_SOURCE.contains(forbidden),
            "V1 physical census escaped the Foundation boundary: {forbidden}"
        );
    }
}

#[test]
fn sealed_quarantine_replays_exact_bytes_and_rejects_discovery_overlap() {
    let temp = TestRoot::new();
    let source_root = temp.path().join("legacy");
    let source_path = source_root.join("payload");
    let quarantine_root = temp.path().join("sealed-quarantine");
    fs::create_dir(&source_root).expect("legacy source root");
    fs::write(&source_path, b"opaque-v1-bytes").expect("legacy source bytes");
    let declared = DeclaredRootV1::new(
        locator(&source_root),
        locator(&source_root),
        InventoryDomainV1::Repository,
        false,
    )
    .expect("declared root");
    let row = InventoryRowV1::new(
        declared.id(),
        locator(&source_path),
        locator(&source_path),
        InventoryDomainV1::Repository,
        InventoryNodeKindV1::RegularFile,
        InventoryPayloadV1::from_bytes(b"opaque-v1-bytes").expect("payload"),
        digest(70),
    )
    .expect("inventory row");
    let inventory =
        ByteTotalInventoryV1::new(vec![declared], vec![row.clone()]).expect("inventory");
    let entry = QuarantineEntryV1::new(&row, b"opaque-v1-bytes".to_vec(), digest(71), digest(72))
        .expect("quarantine entry");
    let classifications = ClassificationSetV1::new(
        &inventory,
        vec![
            SourceClassificationV1::new(
                row.source_id(),
                MigrationDispositionV1::Quarantined,
                digest(71),
                None,
                Some(entry.id()),
                false,
                CancellationClassificationV1::NotCancellationLike,
            )
            .expect("classification"),
        ],
    )
    .expect("classification set");
    let manifest = SealedQuarantineManifestV1::new(
        &inventory,
        &classifications,
        locator(&quarantine_root),
        vec![entry.clone()],
    )
    .expect("sealed manifest");
    // The fence is derived from the sealed inventory's declared roots; a
    // caller can no longer under-declare the active discovery set.
    assert_eq!(
        manifest.active_discovery_roots().to_vec(),
        vec![locator(&source_root)]
    );
    assert!(matches!(
        SealedQuarantineManifestV1::new(
            &inventory,
            &classifications,
            locator(&source_root.join("quarantine")),
            vec![entry.clone()],
        ),
        Err(QuarantineErrorV1::QuarantineInsideDiscovery)
    ));
    let first =
        materialize_sealed_quarantine(&manifest, &quarantine_root).expect("materialization");
    assert_eq!(
        first,
        materialize_sealed_quarantine(&manifest, &quarantine_root).expect("exact replay")
    );
    assert_eq!(
        fs::read(
            quarantine_root
                .join("chunks")
                .join(entry.chunk_digests()[0].render_hex())
        )
        .expect("sealed chunk"),
        b"opaque-v1-bytes"
    );
    assert_eq!(
        fs::read(&source_path).expect("preserved source bytes"),
        b"opaque-v1-bytes"
    );
    fs::write(quarantine_root.join("unexpected"), b"contamination").expect("inject contamination");
    assert!(matches!(
        materialize_sealed_quarantine(&manifest, &quarantine_root),
        Err(QuarantineMaterializationErrorV1::UnexpectedEntry)
    ));
}

#[test]
fn sealed_backup_import_remains_inactive_and_associates_exact_candidate_root() {
    let temp = TestRoot::new();
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"stage11-inactive-import")
        .expect("Store domain");
    let compatibility =
        StoreCompatibilityV1::stage0_successor().expect("frozen successor compatibility");
    let mut source =
        StoreV1::create(temp.path().join("source"), domain.clone()).expect("source Store");
    let object = StoreObjectV1::new(
        SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
        CborValue::Array(vec![CborValue::Unsigned(11)]),
        vec![],
    )
    .expect("source object");
    source.put_object(&object).expect("persist source object");
    let generation = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
        compatibility.clone(),
        vec![object.id()],
    )
    .expect("source Generation");
    source
        .publish_generation(&generation, None)
        .expect("publish source Generation");
    let backup = source.seal_export().expect("sealed backup");

    let legacy_root = temp.path().join("legacy");
    let legacy_file = legacy_root.join("record");
    let declared = DeclaredRootV1::new(
        locator(&legacy_root),
        locator(&legacy_root),
        InventoryDomainV1::Repository,
        false,
    )
    .expect("legacy root");
    let row = InventoryRowV1::new(
        declared.id(),
        locator(&legacy_file),
        locator(&legacy_file),
        InventoryDomainV1::Repository,
        InventoryNodeKindV1::RegularFile,
        InventoryPayloadV1::from_bytes(b"legacy-v1-bytes").expect("legacy payload"),
        digest(30),
    )
    .expect("legacy row");
    let inventory =
        ByteTotalInventoryV1::new(vec![declared], vec![row.clone()]).expect("inventory");
    let target_id =
        MigrationDigestV1::from_digest(object.id().into_bytes()).expect("nonzero object identity");
    let classifications = ClassificationSetV1::new(
        &inventory,
        vec![
            SourceClassificationV1::new(
                row.source_id(),
                MigrationDispositionV1::OpaquePreserved,
                digest(31),
                Some(target_id),
                None,
                false,
                CancellationClassificationV1::NotCancellationLike,
            )
            .expect("classification"),
        ],
    )
    .expect("classification set");
    let target_map = DeterministicIdentityMapV1::new(
        &classifications,
        vec![
            IdentityMapEntryV1::new(
                row.source_id(),
                target_id,
                IdentityMappingBasisV1::HistoricalOpaque {
                    preservation_proof_id: digest(32),
                },
            )
            .expect("identity-map row"),
        ],
    )
    .expect("identity map");
    let quarantine = SealedQuarantineManifestV1::new(
        &inventory,
        &classifications,
        locator(&temp.path().join("sealed-quarantine")),
        vec![],
    )
    .expect("empty sealed quarantine");

    let protocol = MigrationProtocolClosureV1::new(
        MigrationDigestV1::from_digest(compatibility.association_schema_id().into_bytes())
            .expect("nonzero association schema"),
        digest(40),
        digest(41),
        MigrationDigestV1::from_digest(compatibility.finality_edge_manifest_id().into_bytes())
            .expect("nonzero finality manifest"),
        MigrationDigestV1::from_digest(
            compatibility
                .schema_read_write_set_descriptor_id()
                .into_bytes(),
        )
        .expect("nonzero schema read/write set"),
        MigrationDigestV1::from_digest(compatibility.writer_protocol_epoch_id().into_bytes())
            .expect("nonzero writer protocol epoch"),
        MigrationDigestV1::from_digest(compatibility.migration_epoch_id().into_bytes())
            .expect("nonzero migration epoch"),
        None,
    )
    .expect("protocol closure");
    let consumer = ConsumerRecordV1::new(
        locator(&temp.path().join("consumer")),
        ConsumerSubjectV1::CurrentTarget,
        ConsumerGenerationV1::CurrentVNext,
        ConsumerAccessV1::ActiveRuntime,
        true,
        true,
        Some(protocol.clone()),
    )
    .expect("current consumer");
    let consumer_adapter = TestOnlyConsumerCensusAdapterV1 {
        entries: vec![ConsumerCensusEntryV1::observed(digest(89), consumer)],
    };
    let consumers = ConsumerClosureV1::evaluate_from_adapter(
        ConsumerGateStageV1::BeforeSemanticCurrentness,
        protocol.clone(),
        &consumer_adapter,
        PrunePrerequisitesV1::blocked(),
    )
    .expect("consumer closure");
    assert!(consumers.gate_passed());
    let request = InactiveStoreImportRequestV1::new(
        &inventory,
        &classifications,
        &target_map,
        &quarantine,
        &consumers,
        backup.canonical_bytes(),
        object.id(),
    )
    .expect("inactive import request");
    let mut corrupted_backup = backup.canonical_bytes().to_vec();
    let last = corrupted_backup.last_mut().expect("nonempty sealed backup");
    *last ^= 1;
    let mut refused_destination =
        StoreV1::create(temp.path().join("refused-destination"), domain.clone())
            .expect("refused destination Store");
    assert!(import_inactive_store(&mut refused_destination, &request, &corrupted_backup).is_err());
    assert_eq!(
        refused_destination.state().expect("refused state").0,
        StoreStateV1::Inactive
    );
    assert!(
        refused_destination
            .active_head()
            .expect("refused head")
            .is_none()
    );

    let mut destination = StoreV1::create(temp.path().join("destination"), domain.clone())
        .expect("destination Store");
    let receipt = import_inactive_store(&mut destination, &request, backup.canonical_bytes())
        .expect("inactive import");
    assert_eq!(
        destination.state().expect("state").0,
        StoreStateV1::Inactive
    );
    assert!(destination.active_head().expect("head").is_none());
    assert_eq!(
        destination
            .read_object(object.id())
            .expect("imported bytes"),
        object
    );
    assert!(!receipt.activated());
    assert!(!receipt.claims_currentness());

    // The pub receipt seam re-verifies the sealed backup and binds the
    // candidate to that backup's own export bytes: a candidate restored
    // from a different backup refuses even when the backup matches the
    // request.
    assert_eq!(
        request.sealed_backup_sha256(),
        MigrationDigestV1::digest_bytes(backup.canonical_bytes()).expect("backup digest")
    );
    let mut foreign_source =
        StoreV1::create(temp.path().join("foreign-source"), domain.clone()).expect("foreign Store");
    let foreign_object = StoreObjectV1::new(
        SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
        CborValue::Array(vec![CborValue::Unsigned(12)]),
        vec![],
    )
    .expect("foreign object");
    foreign_source
        .put_object(&foreign_object)
        .expect("persist foreign object");
    let foreign_generation = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
        compatibility.clone(),
        vec![foreign_object.id()],
    )
    .expect("foreign Generation");
    foreign_source
        .publish_generation(&foreign_generation, None)
        .expect("publish foreign Generation");
    let foreign_backup = foreign_source.seal_export().expect("foreign sealed backup");
    let mut foreign_probe = StoreV1::create(temp.path().join("foreign-probe"), domain.clone())
        .expect("foreign probe Store");
    let foreign_candidate = foreign_probe
        .import_inactive(foreign_backup.canonical_bytes())
        .expect("foreign restore candidate");
    assert!(matches!(
        InactiveStoreImportReceiptV1::from_candidate(
            &request,
            &foreign_candidate,
            backup.canonical_bytes(),
            receipt.pre_import_state_revision(),
            receipt.post_import_state_revision(),
        ),
        Err(InactiveImportErrorV1::CandidateBackupMismatch)
    ));
    assert!(matches!(
        InactiveStoreImportReceiptV1::from_candidate(
            &request,
            &foreign_candidate,
            &corrupted_backup,
            receipt.pre_import_state_revision(),
            receipt.post_import_state_revision(),
        ),
        Err(InactiveImportErrorV1::SealedBackupMismatch)
    ));

    let rollback_host = TestOnlyStage9Stage10CutoverHostAdapterV1 {
        observed_host_attempt_id: Some(request.id()),
        acceptance: CutoverAcceptanceV1::PreAccept,
        effect_crossing: EffectCrossingV1::ProvenNotCrossed,
    };
    let rollback = RollbackAssessmentV1::from_cutover_host_adapter(request.id(), &rollback_host)
        .expect("rollback assessment");
    let meaning = MigrationAssociationMeaningV1::new(
        &inventory,
        &classifications,
        &target_map,
        &quarantine,
        &consumers,
        &request,
        &receipt,
        &rollback,
    )
    .expect("association meaning");
    assert_eq!(
        meaning.destination_domain_id(),
        receipt.destination_domain_id()
    );
    let meaning_id = meaning.id();
    // A rollback assessment whose verdict is anything but protected
    // exact-v1 eligibility cannot seal an association meaning.
    let stale_rollback = RollbackAssessmentV1::from_cutover_host_adapter(
        request.id(),
        &TestOnlyStage9Stage10CutoverHostAdapterV1 {
            observed_host_attempt_id: None,
            acceptance: CutoverAcceptanceV1::PreAccept,
            effect_crossing: EffectCrossingV1::ProvenNotCrossed,
        },
    )
    .expect("stale rollback assessment");
    assert_eq!(
        stale_rollback.disposition(),
        RollbackDispositionV1::RefusedStaleHost
    );
    assert!(matches!(
        MigrationAssociationMeaningV1::new(
            &inventory,
            &classifications,
            &target_map,
            &quarantine,
            &consumers,
            &request,
            &receipt,
            &stale_rollback,
        ),
        Err(MigrationAssociationErrorV1::RollbackNotEligible)
    ));
    let domain_ref = CutoverDomainRefV1::new(
        CutoverDomainV1::Repository,
        CutoverCommitmentV1::new(domain.id().into_bytes()).expect("domain commitment"),
        1,
        1,
    )
    .expect("cutover domain");
    let context = MigrationCutoverContextV1::ActiveStore {
        distribution_commit_record_id: CutoverCommitmentV1::new(digest(72).into_bytes())
            .expect("distribution commitment"),
    };
    let bindings = TestOnlyStage9AssociationAdapterV1 {
        domain_ref,
        release: ReleaseBindingV1::RepositoryAbsent,
        context,
        distribution_receipt_id: digest(71),
        candidate_store_root_id: receipt.candidate_root_id(),
        schema_read_write_set_id: protocol.schema_read_write_set_id(),
        writer_protocol_epoch_id: protocol.writer_protocol_epoch_id(),
        migration_epoch_id: protocol.migration_epoch_id(),
    };
    let wrong_bindings = TestOnlyStage9AssociationAdapterV1 {
        candidate_store_root_id: digest(99),
        ..bindings.clone()
    };
    let mismatch = MigrationAssociationV1::from_stage9_adapter(meaning.clone(), &wrong_bindings);
    assert!(matches!(
        mismatch,
        Err(MigrationAssociationErrorV1::ExternalBindingMismatch)
    ));
    let association = MigrationAssociationV1::from_stage9_adapter(meaning, &bindings)
        .expect("exact migration association");
    assert!(matches!(
        association.finality(),
        TestOnlyStage9CutoverFinalityV1::ActiveStore(_)
    ));
    assert_eq!(
        association.cutover().material().association_id.as_bytes(),
        meaning_id.as_bytes()
    );
    assert_eq!(
        association
            .cutover()
            .material()
            .candidate_store_root_id
            .as_bytes(),
        receipt.candidate_root_id().as_bytes()
    );
}

// --- Stage-11 cross-stage adapter parity proofs ------------------------------
//
// The three Stage-9/Stage-10 owner adapters stay `#[cfg(test)]` through Stage-11
// integration. The frozen consumer-gate fixture still records
// `stage9_stage10_owner_consumer_snapshot_and_closure_receipt` as a known
// upstream interface gap, so no real owner snapshot, host-acceptance effect, or
// distribution-finality source exists to bind; minting "real" non-test
// constructors would fabricate that upstream rather than integrate it. The
// fixture admits `replace_or_parity_prove_before_stage11_integration`, and these
// proofs take the parity branch: per adapter, the test-only entry admits nothing
// the production core would not also admit.

const ASSOCIATION_RUNTIME_SOURCE: &str =
    include_str!("../../../domain/vnext/migration/runtime/association.rs");
const ROLLBACK_RUNTIME_SOURCE: &str =
    include_str!("../../../domain/vnext/migration/runtime/rollback.rs");
const CONSUMER_RUNTIME_SOURCE: &str =
    include_str!("../../../domain/vnext/migration/runtime/consumer.rs");

#[test]
fn stage9_association_adapter_is_strictly_narrower_than_the_production_core() {
    // Production (`from_verified_h3_native_cancelled_members`, ungated) and the
    // test-only adapter both reach the same private `assemble`, which carries
    // every external-binding, release, and identity check. The adapter adds one
    // refusal production does not need, so it can admit strictly less.
    assert_eq!(
        ASSOCIATION_RUNTIME_SOURCE
            .matches("Self::assemble(")
            .count(),
        2
    );
    assert_eq!(
        ASSOCIATION_RUNTIME_SOURCE.matches("fn assemble(").count(),
        1
    );
    assert!(ASSOCIATION_RUNTIME_SOURCE.contains(
        "    #[cfg(test)]\n    pub(in crate::domain::vnext) fn from_verified_h3_native_cancelled_members<'tx>("
    ));
    assert!(ASSOCIATION_RUNTIME_SOURCE.contains(
        "    #[cfg(test)]\n    pub fn from_stage9_adapter<A: Stage9CutoverAssociationAdapterV1>("
    ));
    assert!(ASSOCIATION_RUNTIME_SOURCE.contains(
        "        if meaning.native_cancellation_count != 0 {\n            return Err(MigrationAssociationErrorV1::H3CarrierCountMismatch);"
    ));
}

#[test]
fn stage9_stage10_cutover_host_adapter_disposition_is_total_over_its_input_tuple() {
    // The adapter is the sole constructor of RollbackAssessmentV1, so parity is
    // proven as closure: the disposition is a total function of
    // (host match, acceptance, crossing) and the identity binds exactly that
    // tuple. No real host rebind can widen the disposition set or produce an
    // assessment whose id is not determined by its inputs.
    let attempt = digest(41);
    let host = |observed, acceptance, effect_crossing| TestOnlyStage9Stage10CutoverHostAdapterV1 {
        observed_host_attempt_id: observed,
        acceptance,
        effect_crossing,
    };
    let matched = Some(attempt);
    for (acceptance, crossing, expected) in [
        (
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::ProvenNotCrossed,
            RollbackDispositionV1::ProtectedExactV1RollbackEligible,
        ),
        (
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::PossibleOrUnknown,
            RollbackDispositionV1::RecoveryRequired,
        ),
        (
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::ConfirmedCrossed,
            RollbackDispositionV1::RecoveryRequired,
        ),
        (
            CutoverAcceptanceV1::Accepted,
            EffectCrossingV1::ProvenNotCrossed,
            RollbackDispositionV1::RecoveryRequired,
        ),
        (
            CutoverAcceptanceV1::Accepted,
            EffectCrossingV1::PossibleOrUnknown,
            RollbackDispositionV1::VNextFreshGenerationRecoveryOnly,
        ),
        (
            CutoverAcceptanceV1::Accepted,
            EffectCrossingV1::ConfirmedCrossed,
            RollbackDispositionV1::VNextFreshGenerationRecoveryOnly,
        ),
    ] {
        let assessment = RollbackAssessmentV1::from_cutover_host_adapter(
            attempt,
            &host(matched, acceptance, crossing),
        )
        .expect("total disposition");
        assert_eq!(assessment.disposition(), expected);
        let repeat = RollbackAssessmentV1::from_cutover_host_adapter(
            attempt,
            &host(matched, acceptance, crossing),
        )
        .expect("repeat disposition");
        assert_eq!(assessment.id(), repeat.id());
    }
    for observed in [None, Some(digest(42))] {
        let stale = RollbackAssessmentV1::from_cutover_host_adapter(
            attempt,
            &host(
                observed,
                CutoverAcceptanceV1::PreAccept,
                EffectCrossingV1::ProvenNotCrossed,
            ),
        )
        .expect("stale host disposition");
        assert_eq!(stale.disposition(), RollbackDispositionV1::RefusedStaleHost);
    }
    let eligible = RollbackAssessmentV1::from_cutover_host_adapter(
        attempt,
        &host(
            matched,
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::ProvenNotCrossed,
        ),
    )
    .expect("eligible");
    let crossed = RollbackAssessmentV1::from_cutover_host_adapter(
        attempt,
        &host(
            matched,
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::ConfirmedCrossed,
        ),
    )
    .expect("crossed");
    assert_ne!(eligible.id(), crossed.id());
    assert!(ROLLBACK_RUNTIME_SOURCE.contains(
        "    #[cfg(test)]\n    pub fn from_cutover_host_adapter<A: Stage9Stage10CutoverHostAdapterV1>("
    ));
}

struct ParityConsumerCensusAdapterV1 {
    declared_member_count: usize,
    authorities: [MigrationDigestV1; 3],
    entries: Vec<ConsumerCensusEntryV1>,
}

impl Stage9Stage10ConsumerCensusAdapterV1 for ParityConsumerCensusAdapterV1 {
    fn authoritative_census_facts(
        &self,
    ) -> Result<(usize, [MigrationDigestV1; 3], Vec<ConsumerCensusEntryV1>), ConsumerClosureErrorV1>
    {
        Ok((
            self.declared_member_count,
            self.authorities,
            self.entries.clone(),
        ))
    }
}

#[test]
fn stage9_stage10_consumer_census_adapter_only_narrows_the_production_closure_core() {
    // `evaluate` is the ungated production closure core; the test-only adapter
    // reaches it only through `from_owner_snapshot`, which is pure additional
    // validation. Anything the adapter can inject is therefore a subset of what
    // a real owner snapshot could present to `evaluate`.
    assert!(
        CONSUMER_RUNTIME_SOURCE
            .contains("        Self::evaluate(stage, protocol, census, prune_prerequisites)")
    );
    assert!(
        CONSUMER_RUNTIME_SOURCE.contains("    fn evaluate(\n        stage: ConsumerGateStageV1,")
    );
    assert!(CONSUMER_RUNTIME_SOURCE.contains(
        "    #[cfg(test)]\n    pub fn evaluate_from_adapter<A: Stage9Stage10ConsumerCensusAdapterV1>("
    ));
    assert!(CONSUMER_RUNTIME_SOURCE.contains("    #[cfg(test)]\n    fn from_owner_snapshot("));
    assert!(!CONSUMER_RUNTIME_SOURCE.contains("#[cfg(test)]\n    fn evaluate(\n"));

    let record = ConsumerRecordV1::new(
        NormalizedLocatorV1::new(b"/parity/consumer".to_vec()).expect("consumer locator"),
        ConsumerSubjectV1::CurrentTarget,
        ConsumerGenerationV1::CurrentVNext,
        ConsumerAccessV1::ActiveRuntime,
        true,
        true,
        Some(test_protocol()),
    )
    .expect("parity consumer record");
    let entries = vec![ConsumerCensusEntryV1::observed(digest(81), record)];
    let evaluate = |adapter: &ParityConsumerCensusAdapterV1| {
        ConsumerClosureV1::evaluate_from_adapter(
            ConsumerGateStageV1::BeforeSemanticCurrentness,
            test_protocol(),
            adapter,
            PrunePrerequisitesV1::blocked(),
        )
    };
    assert!(matches!(
        evaluate(&ParityConsumerCensusAdapterV1 {
            declared_member_count: 2,
            authorities: [digest(90), digest(91), digest(92)],
            entries: entries.clone(),
        }),
        Err(ConsumerClosureErrorV1::InvalidAuthoritativeCensus)
    ));
    assert!(matches!(
        evaluate(&ParityConsumerCensusAdapterV1 {
            declared_member_count: 1,
            authorities: [digest(90), digest(90), digest(92)],
            entries: entries.clone(),
        }),
        Err(ConsumerClosureErrorV1::InvalidAuthoritativeCensus)
    ));
    assert!(
        evaluate(&ParityConsumerCensusAdapterV1 {
            declared_member_count: 1,
            authorities: [digest(90), digest(91), digest(92)],
            entries,
        })
        .is_ok()
    );
}
