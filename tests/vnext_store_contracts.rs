use maestro::domain::identity::{ContractRootIdV1, SchemaIdV1, StoreObjectIdV1};
use maestro::domain::persistence::{
    CollectionPlanV1, ExportError, GenerationError, ReachabilitySnapshotV1, RetentionError,
    RetentionRootKindV1, RetentionRootV1, SealedExportEntryV1, SealedExportV1,
    StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreHeadV1, StoreObjectError,
    StoreObjectV1, StoreRoleV1,
};
use maestro::foundation::core::deterministic_cbor::{self, CborValue};

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn schema(byte: u8) -> SchemaIdV1 {
    SchemaIdV1::parse(&rendered(byte)).expect("valid schema identity")
}

fn contract_root(byte: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&rendered(byte)).expect("valid Contract Root identity")
}

fn compatibility() -> StoreCompatibilityV1 {
    StoreCompatibilityV1::stage0_successor().expect("Stage 0 successor compatibility")
}

fn object(seed: u64, references: Vec<StoreObjectIdV1>) -> StoreObjectV1 {
    StoreObjectV1::new(
        schema(1),
        CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(seed)]),
        references,
    )
    .expect("valid Store Object")
}

#[test]
fn deterministic_cbor_decode_round_trips_the_frozen_subset() {
    let value = CborValue::Array(vec![
        CborValue::Unsigned(24),
        CborValue::Bool(true),
        CborValue::Bytes(vec![0, 1, 2]),
        CborValue::text("store").expect("ASCII text"),
        CborValue::optional(None),
    ]);
    let bytes = deterministic_cbor::encode(&value).expect("canonical bytes");
    assert_eq!(
        deterministic_cbor::decode(&bytes).expect("canonical decode"),
        value
    );
    assert!(deterministic_cbor::decode(&[0x18, 0x01]).is_err());
    assert!(deterministic_cbor::decode(&[0x01, 0x01]).is_err());
}

#[test]
fn store_object_identity_is_canonical_and_path_independent() {
    let first = object(7, vec![]);
    let repeated = object(7, vec![]);
    let changed = object(8, vec![]);

    assert_eq!(first.id(), repeated.id());
    assert_eq!(first.canonical_bytes(), repeated.canonical_bytes());
    assert_ne!(first.id(), changed.id());
    assert_eq!(
        StoreObjectV1::decode(first.canonical_bytes()).expect("decode"),
        first
    );
}

#[test]
fn store_object_references_are_strictly_sorted_unique_and_non_self_selecting() {
    let a = object(1, vec![]).id();
    let b = object(2, vec![]).id();
    let mut sorted = vec![a, b];
    sorted.sort();
    object(3, sorted.clone());

    let mut reversed = sorted.clone();
    reversed.reverse();
    assert_eq!(
        StoreObjectV1::new(schema(1), CborValue::Unsigned(3), reversed),
        Err(StoreObjectError::ReferencesNotStrictlySorted)
    );
    assert_eq!(
        StoreObjectV1::new(schema(1), CborValue::Unsigned(3), vec![a, a]),
        Err(StoreObjectError::ReferencesNotStrictlySorted)
    );
}

#[test]
fn store_role_and_domain_identity_are_closed_and_separate() {
    assert_eq!(
        StoreRoleV1::ALL,
        [StoreRoleV1::Repository, StoreRoleV1::Installation]
    );
    let repository =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"same").expect("repository domain");
    let installation =
        StoreDomainV1::derive(StoreRoleV1::Installation, b"same").expect("installation domain");
    assert_ne!(repository.id(), installation.id());
}

#[test]
fn store_compatibility_binds_the_current_stage0_successor() {
    let current = compatibility();
    assert!(current.is_stage0_successor());

    let predecessor = StoreCompatibilityV1::new(
        maestro::domain::identity::ManifestIdV1::parse(
            "sha256:60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6",
        )
        .expect("predecessor Manifest identity"),
        current.association_schema_id(),
        current.finality_edge_manifest_id(),
        current.schema_read_write_set_descriptor_id(),
        current.writer_protocol_epoch_id(),
        current.migration_epoch_id(),
    );
    assert!(!predecessor.is_stage0_successor());
}

#[test]
fn store_generation_and_head_form_a_monotonic_non_aba_chain() {
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"repo-a").expect("Store domain");
    let root_a = object(10, vec![]).id();
    let generation_a = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        contract_root(2),
        compatibility(),
        vec![root_a],
    )
    .expect("first Generation");
    let head_a = StoreHeadV1::new(&generation_a, 1, None).expect("first Head");
    assert_eq!(
        StoreGenerationV1::decode(&generation_a.canonical_bytes().expect("Generation bytes"))
            .expect("Generation decode"),
        generation_a
    );

    let root_b = object(11, vec![]).id();
    let generation_b = StoreGenerationV1::new(
        domain.clone(),
        2,
        Some(generation_a.id()),
        contract_root(2),
        compatibility(),
        vec![root_b],
    )
    .expect("second Generation");
    let head_b = StoreHeadV1::new(&generation_b, 2, Some(head_a.id())).expect("second Head");

    let generation_a_again = StoreGenerationV1::new(
        domain,
        3,
        Some(generation_b.id()),
        contract_root(2),
        compatibility(),
        vec![root_a],
    )
    .expect("third Generation with prior visible root");
    let head_a_again =
        StoreHeadV1::new(&generation_a_again, 3, Some(head_b.id())).expect("third non-ABA Head");

    assert_ne!(generation_a.id(), generation_a_again.id());
    assert_ne!(head_a.id(), head_a_again.id());
    assert_ne!(head_a.revision(), head_a_again.revision());
}

#[test]
fn store_generation_rejects_rewind_shapes() {
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"repo-b").expect("Store domain");
    let root = object(1, vec![]).id();
    assert_eq!(
        StoreGenerationV1::new(
            domain.clone(),
            2,
            None,
            contract_root(2),
            compatibility(),
            vec![root]
        ),
        Err(GenerationError::InvalidPreviousGeneration)
    );
    assert_eq!(
        StoreGenerationV1::new(
            domain,
            1,
            Some(
                StoreGenerationV1::new(
                    StoreDomainV1::derive(StoreRoleV1::Repository, b"repo-c")
                        .expect("Store domain"),
                    1,
                    None,
                    contract_root(2),
                    compatibility(),
                    vec![root]
                )
                .expect("Generation")
                .id()
            ),
            contract_root(2),
            compatibility(),
            vec![root]
        ),
        Err(GenerationError::InvalidPreviousGeneration)
    );
}

#[test]
fn reachability_requires_complete_sorted_roots_and_tombstones_before_collection() {
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"repo-retention").expect("Store domain");
    let live = object(40, vec![]).id();
    let tombstoned = object(41, vec![]).id();
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        contract_root(2),
        compatibility(),
        vec![live],
    )
    .expect("Generation");
    let head = StoreHeadV1::new(&generation, 1, None).expect("Head");
    let roots = vec![RetentionRootV1::new(
        RetentionRootKindV1::ActiveGeneration,
        live,
    )];
    let snapshot = ReachabilitySnapshotV1::new(head.id(), 1, roots, vec![live], vec![tombstoned])
        .expect("complete reachability");
    let plan = CollectionPlanV1::new(&snapshot, vec![tombstoned]).expect("collection plan");
    assert_eq!(plan.candidates(), &[tombstoned]);

    assert_eq!(
        CollectionPlanV1::new(&snapshot, vec![live]),
        Err(RetentionError::CandidateNotCollectable)
    );
    assert_eq!(
        ReachabilitySnapshotV1::new(
            head.id(),
            1,
            vec![RetentionRootV1::new(
                RetentionRootKindV1::ActiveGeneration,
                tombstoned
            )],
            vec![live],
            vec![tombstoned]
        ),
        Err(RetentionError::RootNotReachable)
    );
}

#[test]
fn sealed_export_is_deterministic_complete_and_round_trippable() {
    let leaf = object(50, vec![]);
    let root = object(51, vec![leaf.id()]);
    let mut reachable = vec![leaf.id(), root.id()];
    reachable.sort();
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"repo-export").expect("Store domain");
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        contract_root(2),
        compatibility(),
        vec![root.id()],
    )
    .expect("Generation");
    let head = StoreHeadV1::new(&generation, 1, None).expect("Head");
    let snapshot = ReachabilitySnapshotV1::new(
        head.id(),
        1,
        vec![RetentionRootV1::new(
            RetentionRootKindV1::ActiveGeneration,
            root.id(),
        )],
        reachable,
        vec![],
    )
    .expect("reachability");
    let mut entries = vec![
        SealedExportEntryV1::Available(root),
        SealedExportEntryV1::Available(leaf),
    ];
    entries.sort_by_key(SealedExportEntryV1::object_id);
    let export =
        SealedExportV1::new(generation, head, snapshot, vec![], entries).expect("sealed export");
    let decoded = SealedExportV1::decode(export.canonical_bytes()).expect("offline verification");
    assert_eq!(decoded, export);
    assert_eq!(decoded.id(), export.id());

    let mut tampered = export.canonical_bytes().to_vec();
    let last = tampered.last_mut().expect("nonempty export");
    *last ^= 1;
    assert!(SealedExportV1::decode(&tampered).is_err());
}

#[test]
fn sealed_export_refuses_an_incomplete_reference_closure() {
    let missing = object(60, vec![]);
    let root = object(61, vec![missing.id()]);
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"repo-export-missing")
        .expect("Store domain");
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        contract_root(2),
        compatibility(),
        vec![root.id()],
    )
    .expect("Generation");
    let head = StoreHeadV1::new(&generation, 1, None).expect("Head");
    let snapshot = ReachabilitySnapshotV1::new(
        head.id(),
        1,
        vec![RetentionRootV1::new(
            RetentionRootKindV1::ActiveGeneration,
            root.id(),
        )],
        vec![root.id()],
        vec![],
    )
    .expect("syntactically complete snapshot");
    assert_eq!(
        SealedExportV1::new(
            generation,
            head,
            snapshot,
            vec![],
            vec![SealedExportEntryV1::Available(root)]
        ),
        Err(ExportError::ReferenceClosureIncomplete)
    );
}
