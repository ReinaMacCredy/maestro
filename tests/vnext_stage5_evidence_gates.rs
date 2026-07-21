use std::collections::BTreeSet;
use std::fs;

use maestro::domain::vnext::authority::{ExecutionProducerV1, PrincipalIdV1, SessionIdV1};
use maestro::domain::vnext::contract::runtime::ContractGenerationIdV1;
use maestro::domain::vnext::evidence::{
    ClaimError, ClaimSubjectV1, ClaimV1, EvidenceClaimPublicationV1, EvidencePayloadManifestV1,
    EvidenceRedactionPolicyV1, EvidenceRetentionClassV1, EvidenceRetentionPolicyV1,
    EvidenceSecretScanReceiptV1, NominalObservationPayloadV1, ObservationAcquisitionV1,
    ObservationDraftV1, ObservationError, ObservationKindV1, ObservationPayloadCommonV1,
    ObservationPayloadDetailV1, ObservationPayloadFieldTypeV1, ObservationPayloadFieldV1,
    ObservationPayloadV1, ObservationPublicationRouteV1, ObservationRecordIdV1,
    ObservationSubjectKindV1, ObservationSubjectV1, ObservationV1, SubmissionRefV1,
};
use maestro::domain::vnext::gate::{
    GateError, GateEvaluationInputV1, GateEvaluationResultV1, GateEvaluatorContractV1,
    GateInputClassV1, GateLeafRuleV1, GateNodeIdV1, GateNodeV1, GateOperatorV1, GateScopeV1,
    GateSnapshotV1, PureGateEvaluatorV1,
};
use maestro::domain::vnext::identity::{
    ContractComponentIdV1, ContractRootIdV1, StoreDomainIdV1, StoreObjectIdV1,
};
use maestro::domain::vnext::work::{WorkIdV1, WorkSubmissionIdV1};
use maestro::foundation::core::deterministic_cbor::{self, CborValue};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn hash(byte: u8) -> [u8; 32] {
    Sha256::digest([byte]).into()
}

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn deterministic_detail(seed: u8) -> ObservationPayloadDetailV1 {
    ObservationPayloadDetailV1::Deterministic {
        executable_bytes_hash: hash(seed),
        executable_version_hash: hash(seed + 1),
        arguments_hash: hash(seed + 2),
        working_directory_hash: hash(seed + 3),
        relevant_environment_hash: hash(seed + 4),
        subject_revision_hash: hash(seed + 5),
        dirty_state_hash: hash(seed + 6),
        exit_status_hash: hash(seed + 7),
        stdout_hash: hash(seed + 8),
        stderr_hash: hash(seed + 9),
    }
}

fn nominal_fields(kind: ObservationKindV1, seed: u8) -> Vec<ObservationPayloadFieldV1> {
    kind.contract()
        .unwrap()
        .payload_fields()
        .into_iter()
        .enumerate()
        .map(|(index, field)| match field.field_type() {
            ObservationPayloadFieldTypeV1::Digest => {
                ObservationPayloadFieldV1::Digest(hash(seed.wrapping_add(index as u8)))
            }
            ObservationPayloadFieldTypeV1::Count => {
                ObservationPayloadFieldV1::Count(index as u64 + 1)
            }
            ObservationPayloadFieldTypeV1::Timestamp => {
                ObservationPayloadFieldV1::Timestamp(1_000 + index as u64)
            }
            ObservationPayloadFieldTypeV1::Tag => ObservationPayloadFieldV1::Tag(index as u64 + 1),
            ObservationPayloadFieldTypeV1::Boolean => {
                ObservationPayloadFieldV1::Boolean(index % 2 == 0)
            }
        })
        .collect()
}

fn payload_manifest(
    kind: ObservationKindV1,
    object_id: StoreObjectIdV1,
    payload: &ObservationPayloadV1,
    producer: ExecutionProducerV1,
    recorded_at: u64,
    _seed: u8,
) -> EvidencePayloadManifestV1 {
    let redaction = EvidenceRedactionPolicyV1::prohibit_secrets_v1(1_048_576).unwrap();
    let scan =
        EvidenceSecretScanReceiptV1::scan(object_id, payload, redaction, producer, recorded_at)
            .unwrap();
    let retention = EvidenceRetentionPolicyV1::new(
        EvidenceRetentionClassV1::ExplicitSecurityErasureEligible,
        recorded_at + 1_000,
    )
    .unwrap();
    EvidencePayloadManifestV1::new(
        kind,
        object_id,
        payload,
        "application/json",
        redaction,
        scan,
        retention,
    )
    .unwrap()
}

fn leaf(scope: GateScopeV1, input_class: GateInputClassV1, seed: u8) -> GateNodeV1 {
    let rule = match input_class {
        GateInputClassV1::Evidence => GateLeafRuleV1::EvidenceSetPresent,
        GateInputClassV1::Authority => GateLeafRuleV1::AuthoritySetPresent,
        GateInputClassV1::Mixed => GateLeafRuleV1::MixedSetPresent,
        GateInputClassV1::Composite => panic!("leaf input class cannot be composite"),
    };
    GateNodeV1::new(
        scope,
        input_class,
        GateOperatorV1::Leaf,
        GateEvaluatorContractV1::leaf(rule, hash(seed + 4)).unwrap(),
        hash(seed + 5),
        None,
        vec![],
    )
    .unwrap()
}

fn composite(operator: GateOperatorV1, children: &[GateNodeV1], seed: u8) -> GateNodeV1 {
    GateNodeV1::new(
        GateScopeV1::Work,
        GateInputClassV1::Composite,
        operator,
        GateEvaluatorContractV1::composite(operator, hash(seed + 4)).unwrap(),
        hash(seed + 5),
        None,
        children.iter().map(GateNodeV1::id).collect(),
    )
    .unwrap()
}

fn snapshot(roots: Vec<GateNodeIdV1>, nodes: Vec<GateNodeV1>) -> GateSnapshotV1 {
    GateSnapshotV1::new(
        WorkIdV1::derive("stage5-evidence-work").unwrap(),
        ContractGenerationIdV1::parse(&rendered(20)).unwrap(),
        ContractRootIdV1::parse(&rendered(21)).unwrap(),
        ContractComponentIdV1::parse(&rendered(22)).unwrap(),
        hash(22),
        hash(23),
        roots,
        nodes,
    )
    .unwrap()
}

fn observation(
    seed: u8,
    lineage: Vec<ObservationRecordIdV1>,
    acquisition: ObservationAcquisitionV1,
) -> ObservationV1 {
    let kind = ObservationKindV1::DeterministicProcedure;
    let generation = ContractGenerationIdV1::parse(&rendered(20)).unwrap();
    let root = ContractRootIdV1::parse(&rendered(21)).unwrap();
    let repository = StoreDomainIdV1::parse(&rendered(30)).unwrap();
    let subjects = vec![
        ObservationSubjectV1::for_work(
            *WorkIdV1::derive("stage5-evidence-work").unwrap().as_bytes(),
            generation,
            *root.as_bytes(),
        )
        .unwrap(),
        ObservationSubjectV1::new(
            ObservationSubjectKindV1::Repository,
            *repository.as_bytes(),
            *generation.as_bytes(),
        )
        .unwrap(),
    ];
    let procedure_hash = hash(seed + 2);
    let environment_hash = hash(seed + 3);
    let toolchain_hash = hash(seed + 4);
    let observed_at = 100 + u64::from(seed);
    let recorded_at = 101 + u64::from(seed);
    let clock_basis_hash = hash(seed + 5);
    let typed_payload = ObservationPayloadV1::new(
        kind,
        ObservationPayloadCommonV1::new(
            &subjects,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
        )
        .unwrap(),
        ObservationPayloadDetailV1::Deterministic {
            executable_bytes_hash: hash(seed + 10),
            executable_version_hash: hash(seed + 11),
            arguments_hash: hash(seed + 12),
            working_directory_hash: hash(seed + 13),
            relevant_environment_hash: hash(seed + 14),
            subject_revision_hash: hash(seed + 15),
            dirty_state_hash: hash(seed + 16),
            exit_status_hash: hash(seed + 17),
            stdout_hash: hash(seed + 18),
            stderr_hash: hash(seed + 19),
        },
    )
    .unwrap();
    let producer = ExecutionProducerV1::SessionBound {
        principal_id: PrincipalIdV1::derive(&format!("stage5-observer-{seed}")).unwrap(),
        session_id: SessionIdV1::derive(&format!("stage5-session-{seed}")).unwrap(),
    };
    let object_id = StoreObjectIdV1::parse(&rendered(seed + 6)).unwrap();
    ObservationV1::new(ObservationDraftV1 {
        kind,
        store_domain_id: StoreDomainIdV1::parse(&rendered(30)).unwrap(),
        subjects,
        producer,
        procedure_hash,
        environment_hash,
        toolchain_hash,
        observed_at,
        recorded_at,
        clock_basis_hash,
        lineage,
        payload: payload_manifest(
            kind,
            object_id,
            &typed_payload,
            producer,
            recorded_at,
            seed + 7,
        ),
        acquisition,
        publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
    })
    .unwrap()
}

#[test]
fn observation_kind_runtime_matches_all_frozen_catalog_semantics() {
    let catalog: Value = serde_json::from_slice(
        &fs::read("contracts/vnext/catalogs/generated/catalog-01-observation.json").unwrap(),
    )
    .unwrap();
    let expected: Vec<_> = catalog["descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let value = row["value"].as_array().unwrap();
            let action = value[3].as_array().unwrap()[0].as_u64().unwrap();
            let routes = value[4]
                .as_array()
                .unwrap()
                .iter()
                .map(|route| route.as_u64().unwrap())
                .collect::<Vec<_>>();
            let compatibility = value[5]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let pair = pair.as_array().unwrap();
                    (pair[0].as_u64().unwrap(), pair[1].as_u64().unwrap())
                })
                .collect::<Vec<_>>();
            (
                value[0].as_u64().unwrap(),
                value[1].as_str().unwrap().to_owned(),
                action,
                routes,
                compatibility,
            )
        })
        .collect();
    let actual: Vec<_> = ObservationKindV1::ALL
        .iter()
        .map(|kind| {
            (
                kind.tag(),
                kind.name().to_owned(),
                kind.producer_action_tag(),
                kind.source_route_tags().to_vec(),
                kind.cma_compatibility().to_vec(),
            )
        })
        .collect();

    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 43);
    assert!(ObservationKindV1::from_tag(0).is_err());
    assert!(ObservationKindV1::from_tag(44).is_err());

    let contract_rows = ObservationKindV1::ALL
        .iter()
        .map(|kind| {
            let contract = kind.contract().unwrap();
            serde_json::json!([
                kind.tag(),
                kind.name(),
                contract.payload_schema_id().render(),
                contract.required_subject_mask(),
                contract.allowed_subject_mask(),
                contract.allowed_acquisition_mask(),
                contract.payload_detail_tag(),
                hex(contract.contract_hash()),
            ])
        })
        .collect::<Vec<_>>();
    let contract_table_identity = hex(&Sha256::digest(serde_json::to_vec(&contract_rows).unwrap()));
    assert_eq!(
        contract_table_identity,
        "a5f0e9137c091972802cb7084d86070a930091f0570cefcc7df445074478a676"
    );
    assert_eq!(
        contract_rows
            .iter()
            .map(|row| row[2].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        43
    );
    assert_eq!(
        contract_rows
            .iter()
            .map(|row| row[7].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        43
    );
    for (index, kind) in ObservationKindV1::ALL.iter().enumerate() {
        let contract = kind.contract().unwrap();
        let payload_fields = contract.payload_fields();
        assert!(!payload_fields.is_empty());
        assert_eq!(
            payload_fields
                .iter()
                .map(|field| field.name())
                .collect::<BTreeSet<_>>()
                .len(),
            payload_fields.len()
        );
        assert_eq!(
            contract.required_subject_mask() & !contract.allowed_subject_mask(),
            0
        );
        assert_ne!(contract.allowed_subject_mask(), 0);
        assert_ne!(contract.allowed_acquisition_mask(), 0);
        assert_eq!(contract.allowed_acquisition_mask() & !0b111, 0);
        let subjects = vec![
            ObservationSubjectV1::new(ObservationSubjectKindV1::Repository, hash(100), hash(101))
                .unwrap(),
        ];
        let common = ObservationPayloadCommonV1::new(
            &subjects,
            hash(102),
            hash(103),
            hash(104),
            200,
            201,
            hash(105),
        )
        .unwrap();
        if contract.payload_detail_tag() == 9 {
            let fields = nominal_fields(*kind, 130);
            let nominal = NominalObservationPayloadV1::new(*kind, fields.clone()).unwrap();
            let payload = ObservationPayloadV1::new(
                *kind,
                common,
                ObservationPayloadDetailV1::Nominal(nominal),
            )
            .unwrap();
            let encoded = payload.canonical_bytes().unwrap();
            assert_eq!(
                ObservationPayloadV1::from_canonical_bytes(&encoded).unwrap(),
                payload
            );

            let mut wrong_count = fields.clone();
            wrong_count.pop();
            assert_eq!(
                NominalObservationPayloadV1::new(*kind, wrong_count).unwrap_err(),
                ObservationError::InvalidPayloadSemantics
            );
            let mut wrong_type = fields;
            wrong_type[0] = ObservationPayloadFieldV1::Boolean(true);
            assert_eq!(
                NominalObservationPayloadV1::new(*kind, wrong_type).unwrap_err(),
                ObservationError::InvalidPayloadSemantics
            );
        }
        let wrong_detail = if contract.payload_detail_tag() == 9 {
            deterministic_detail(110)
        } else {
            ObservationPayloadDetailV1::Nominal(
                NominalObservationPayloadV1::new(
                    ObservationKindV1::RemoteReadback,
                    nominal_fields(ObservationKindV1::RemoteReadback, 120),
                )
                .unwrap(),
            )
        };
        assert_eq!(
            ObservationPayloadV1::new(*kind, common, wrong_detail).unwrap_err(),
            ObservationError::InvalidPayloadSemantics,
            "kind {} accepted a foreign payload detail at row {index}",
            kind.name()
        );
    }
}

#[test]
fn payload_manifest_requires_current_authenticated_zero_secret_scan() {
    let kind = ObservationKindV1::DeterministicProcedure;
    let work_id = WorkIdV1::derive("stage5-secret-policy-work").unwrap();
    let contract_root = ContractRootIdV1::parse(&rendered(21)).unwrap();
    let subjects = vec![
        ObservationSubjectV1::for_work(
            *work_id.as_bytes(),
            ContractGenerationIdV1::parse(&rendered(20)).unwrap(),
            *contract_root.as_bytes(),
        )
        .unwrap(),
        ObservationSubjectV1::new(ObservationSubjectKindV1::Repository, hash(30), hash(20))
            .unwrap(),
    ];
    let observed_at = 200;
    let recorded_at = 201;
    let procedure_hash = hash(32);
    let environment_hash = hash(33);
    let toolchain_hash = hash(34);
    let clock_basis_hash = hash(35);
    let payload = ObservationPayloadV1::new(
        kind,
        ObservationPayloadCommonV1::new(
            &subjects,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
        )
        .unwrap(),
        deterministic_detail(40),
    )
    .unwrap();
    let object_id = StoreObjectIdV1::parse(&rendered(50)).unwrap();
    let producer = ExecutionProducerV1::SessionBound {
        principal_id: PrincipalIdV1::derive("stage5-secret-scanner-principal").unwrap(),
        session_id: SessionIdV1::derive("stage5-secret-scanner-session").unwrap(),
    };
    let redaction = EvidenceRedactionPolicyV1::prohibit_secrets_v1(1_048_576).unwrap();
    let secret_payload = ObservationPayloadV1::new(
        kind,
        ObservationPayloadCommonV1::new(
            &subjects,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
        )
        .unwrap(),
        ObservationPayloadDetailV1::Deterministic {
            executable_bytes_hash: *b"raw-secret-canary-in-digest-slot",
            executable_version_hash: hash(41),
            arguments_hash: hash(42),
            working_directory_hash: hash(43),
            relevant_environment_hash: hash(44),
            subject_revision_hash: hash(45),
            dirty_state_hash: hash(46),
            exit_status_hash: hash(47),
            stdout_hash: hash(48),
            stderr_hash: hash(49),
        },
    )
    .unwrap();
    let canary = EvidenceSecretScanReceiptV1::scan(
        object_id,
        &secret_payload,
        redaction,
        producer,
        recorded_at,
    )
    .unwrap();
    let retention = EvidenceRetentionPolicyV1::new(
        EvidenceRetentionClassV1::ExplicitSecurityErasureEligible,
        recorded_at + 1_000,
    )
    .unwrap();
    assert_eq!(
        EvidencePayloadManifestV1::new(
            kind,
            object_id,
            &secret_payload,
            "application/cbor",
            redaction,
            canary,
            retention,
        )
        .unwrap_err(),
        ObservationError::InvalidSecretScanReceipt
    );

    let stale_scan = EvidenceSecretScanReceiptV1::scan(
        object_id,
        &payload,
        redaction,
        producer,
        observed_at - 1,
    )
    .unwrap();
    let stale_manifest = EvidencePayloadManifestV1::new(
        kind,
        object_id,
        &payload,
        "application/cbor",
        redaction,
        stale_scan,
        retention,
    )
    .unwrap();
    assert_eq!(
        ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: StoreDomainIdV1::parse(&rendered(30)).unwrap(),
            subjects,
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
            lineage: vec![],
            payload: stale_manifest,
            acquisition: ObservationAcquisitionV1::effect_free(hash(53), hash(54)).unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap_err(),
        ObservationError::KindSemanticMismatch
    );
}

#[test]
fn observation_publication_route_rejects_wrong_action_route_and_profile() {
    assert_eq!(
        ObservationPublicationRouteV1::new(
            ObservationKindV1::DeterministicProcedure,
            40,
            None,
            None,
        )
        .unwrap_err(),
        ObservationError::InvalidPublicationRoute
    );
    assert_eq!(
        ObservationPublicationRouteV1::new(
            ObservationKindV1::TrustedTimeAtomicUnit,
            45,
            Some(2),
            Some(1),
        )
        .unwrap_err(),
        ObservationError::InvalidPublicationRoute
    );
    ObservationPublicationRouteV1::new(
        ObservationKindV1::TrustedTimeAtomicUnit,
        45,
        Some(1),
        Some(1),
    )
    .unwrap();
}

#[test]
fn observations_bind_effect_free_and_exact_derivation_provenance() {
    let first = observation(
        40,
        vec![],
        ObservationAcquisitionV1::effect_free(hash(50), hash(51)).unwrap(),
    );
    assert!(!first.acquisition().has_run());
    assert_eq!(first.acquisition().acquisition_id(), Some(&hash(50)));

    let derived = observation(
        60,
        vec![first.id()],
        ObservationAcquisitionV1::declared_derivation(vec![first.id()]).unwrap(),
    );
    assert!(!derived.acquisition().has_run());
    assert_ne!(first.id(), derived.id());

    let mismatch = ObservationV1::new(ObservationDraftV1 {
        kind: ObservationKindV1::DeterministicProcedure,
        store_domain_id: derived.store_domain_id(),
        subjects: derived.subjects().to_vec(),
        producer: ExecutionProducerV1::SessionBound {
            principal_id: PrincipalIdV1::derive("stage5-mismatch").unwrap(),
            session_id: SessionIdV1::derive("stage5-mismatch-session").unwrap(),
        },
        procedure_hash: hash(70),
        environment_hash: hash(71),
        toolchain_hash: hash(72),
        observed_at: 200,
        recorded_at: 201,
        clock_basis_hash: hash(73),
        lineage: vec![],
        payload: derived.payload().clone(),
        acquisition: ObservationAcquisitionV1::declared_derivation(vec![first.id()]).unwrap(),
        publication_route: ObservationPublicationRouteV1::new(
            ObservationKindV1::DeterministicProcedure,
            39,
            None,
            None,
        )
        .unwrap(),
    });
    assert_eq!(
        mismatch.unwrap_err(),
        ObservationError::DerivationLineageMismatch
    );
}

#[test]
fn gate_snapshot_is_canonical_closed_and_root_reachable() {
    let first = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10);
    let second = leaf(GateScopeV1::Work, GateInputClassV1::Authority, 20);
    let all = composite(GateOperatorV1::All, &[first.clone(), second.clone()], 30);
    let canonical = snapshot(
        vec![all.id()],
        vec![all.clone(), second.clone(), first.clone()],
    );
    let reordered = snapshot(
        vec![all.id()],
        vec![first.clone(), all.clone(), second.clone()],
    );
    assert_eq!(canonical.id(), reordered.id());
    assert_eq!(
        canonical.canonical_bytes().unwrap(),
        reordered.canonical_bytes().unwrap()
    );
    let canonical_bytes = canonical.canonical_bytes().unwrap();
    assert_eq!(
        GateSnapshotV1::from_canonical_bytes(&canonical_bytes).unwrap(),
        canonical
    );

    let mut identity_mutant = deterministic_cbor::decode(&canonical_bytes).unwrap();
    let CborValue::Array(record) = &mut identity_mutant else {
        unreachable!("Gate Snapshot record is an array");
    };
    let CborValue::Bytes(stored_id) = &mut record[0] else {
        unreachable!("Gate Snapshot identity is bytes");
    };
    stored_id[0] ^= 1;
    assert_eq!(
        GateSnapshotV1::from_canonical_bytes(
            &deterministic_cbor::encode(&identity_mutant).unwrap()
        )
        .unwrap_err(),
        GateError::InvalidStoredSnapshot
    );

    let mut order_mutant = deterministic_cbor::decode(&canonical_bytes).unwrap();
    let CborValue::Array(record) = &mut order_mutant else {
        unreachable!("Gate Snapshot record is an array");
    };
    let CborValue::Array(snapshot_fields) = &mut record[1] else {
        unreachable!("Gate Snapshot material is an array");
    };
    let CborValue::Array(nodes) = &mut snapshot_fields[7] else {
        unreachable!("Gate Snapshot nodes are an array");
    };
    nodes.reverse();
    assert_eq!(
        GateSnapshotV1::from_canonical_bytes(&deterministic_cbor::encode(&order_mutant).unwrap())
            .unwrap_err(),
        GateError::InvalidStoredSnapshot
    );

    let mut tag_mutant = deterministic_cbor::decode(&canonical_bytes).unwrap();
    let CborValue::Array(record) = &mut tag_mutant else {
        unreachable!("Gate Snapshot record is an array");
    };
    let CborValue::Array(snapshot_fields) = &mut record[1] else {
        unreachable!("Gate Snapshot material is an array");
    };
    let CborValue::Array(nodes) = &mut snapshot_fields[7] else {
        unreachable!("Gate Snapshot nodes are an array");
    };
    let CborValue::Array(node_record) = &mut nodes[0] else {
        unreachable!("Gate node record is an array");
    };
    let CborValue::Array(node_fields) = &mut node_record[1] else {
        unreachable!("Gate node material is an array");
    };
    node_fields[0] = CborValue::Unsigned(99);
    assert_eq!(
        GateSnapshotV1::from_canonical_bytes(&deterministic_cbor::encode(&tag_mutant).unwrap())
            .unwrap_err(),
        GateError::InvalidStoredSnapshot
    );

    assert_eq!(
        GateSnapshotV1::new(
            WorkIdV1::derive("stage5-detached").unwrap(),
            ContractGenerationIdV1::parse(&rendered(20)).unwrap(),
            ContractRootIdV1::parse(&rendered(21)).unwrap(),
            ContractComponentIdV1::parse(&rendered(22)).unwrap(),
            hash(22),
            hash(23),
            vec![first.id()],
            vec![first, second],
        )
        .unwrap_err(),
        GateError::DetachedNode
    );

    let step_leaf = leaf(GateScopeV1::Step, GateInputClassV1::Evidence, 40);
    let work_parent = composite(GateOperatorV1::All, std::slice::from_ref(&step_leaf), 50);
    assert_eq!(
        GateSnapshotV1::new(
            WorkIdV1::derive("stage5-cross-scope").unwrap(),
            ContractGenerationIdV1::parse(&rendered(20)).unwrap(),
            ContractRootIdV1::parse(&rendered(21)).unwrap(),
            ContractComponentIdV1::parse(&rendered(22)).unwrap(),
            hash(22),
            hash(23),
            vec![work_parent.id()],
            vec![step_leaf, work_parent],
        )
        .unwrap_err(),
        GateError::CrossScopeEdge
    );
}

#[test]
fn composite_gate_grammars_are_fail_closed_and_order_independent() {
    let children = vec![
        leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10),
        leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 20),
        leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 30),
    ];
    let operators = [
        GateOperatorV1::All,
        GateOperatorV1::Any,
        GateOperatorV1::Quorum { required: 1 },
        GateOperatorV1::Quorum { required: 2 },
        GateOperatorV1::Quorum { required: 3 },
        GateOperatorV1::Veto,
        GateOperatorV1::DenyOverrides,
    ];
    for (index, operator) in operators.into_iter().enumerate() {
        let gate = composite(operator, &children, 40 + index as u8 * 6);
        let mut nodes = children.clone();
        nodes.push(gate.clone());
        let graph = snapshot(vec![gate.id()], nodes);
        for first in GateEvaluationResultV1::ALL {
            for second in GateEvaluationResultV1::ALL {
                for third in GateEvaluationResultV1::ALL {
                    let results = [first, second, third];
                    let expected = composite_result_oracle(operator, &results);
                    let inputs = children
                        .iter()
                        .zip(results)
                        .enumerate()
                        .map(|(item, (child, result))| {
                            GateEvaluationInputV1::child(child.id(), hash(80 + item as u8), result)
                                .unwrap()
                        })
                        .collect::<Vec<_>>();
                    let forward = PureGateEvaluatorV1
                        .evaluate(&graph, gate.id(), inputs.clone())
                        .unwrap()
                        .result();
                    let reverse = PureGateEvaluatorV1
                        .evaluate(&graph, gate.id(), inputs.into_iter().rev().collect())
                        .unwrap()
                        .result();
                    assert_eq!(
                        forward, expected,
                        "operator {operator:?}, inputs {results:?}"
                    );
                    assert_eq!(
                        reverse, expected,
                        "operator {operator:?}, inputs {results:?}"
                    );
                }
            }
        }
    }
}

fn composite_result_oracle(
    operator: GateOperatorV1,
    results: &[GateEvaluationResultV1],
) -> GateEvaluationResultV1 {
    let count = |expected| results.iter().filter(|result| **result == expected).count();
    let passes = count(GateEvaluationResultV1::Pass);
    let failures = count(GateEvaluationResultV1::Fail);
    let unknowns = count(GateEvaluationResultV1::Indeterminate);
    let errors = count(GateEvaluationResultV1::Error);
    match operator {
        GateOperatorV1::Leaf => panic!("composite oracle cannot evaluate a leaf"),
        GateOperatorV1::All => match (errors, failures, unknowns) {
            (1.., _, _) => GateEvaluationResultV1::Error,
            (0, 1.., _) => GateEvaluationResultV1::Fail,
            (0, 0, 1..) => GateEvaluationResultV1::Indeterminate,
            (0, 0, 0) => GateEvaluationResultV1::Pass,
        },
        GateOperatorV1::Any => match (passes, errors, unknowns) {
            (1.., _, _) => GateEvaluationResultV1::Pass,
            (0, 1.., _) => GateEvaluationResultV1::Error,
            (0, 0, 1..) => GateEvaluationResultV1::Indeterminate,
            (0, 0, 0) => GateEvaluationResultV1::Fail,
        },
        GateOperatorV1::Quorum { required } => {
            let required = required as usize;
            if passes >= required {
                GateEvaluationResultV1::Pass
            } else if passes + unknowns + errors < required {
                GateEvaluationResultV1::Fail
            } else if errors > 0 {
                GateEvaluationResultV1::Error
            } else {
                GateEvaluationResultV1::Indeterminate
            }
        }
        GateOperatorV1::Veto | GateOperatorV1::DenyOverrides => {
            match (failures, errors, unknowns) {
                (1.., _, _) => GateEvaluationResultV1::Fail,
                (0, 1.., _) => GateEvaluationResultV1::Error,
                (0, 0, 1..) => GateEvaluationResultV1::Indeterminate,
                (0, 0, 0) => GateEvaluationResultV1::Pass,
            }
        }
    }
}

#[test]
fn pure_composite_evaluator_refuses_leaf_self_attestation() {
    let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10);
    let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
    assert_eq!(
        PureGateEvaluatorV1
            .evaluate(
                &graph,
                gate.id(),
                vec![GateEvaluationInputV1::leaf(hash(90), GateEvaluationResultV1::Pass,).unwrap()],
            )
            .unwrap_err(),
        GateError::LeafRequiresPinnedEvaluator
    );
}

#[test]
fn claim_publication_requires_exact_resolved_observation_records() {
    let resolved = observation(
        40,
        vec![],
        ObservationAcquisitionV1::effect_free(hash(50), hash(51)).unwrap(),
    );
    let extra = observation(
        60,
        vec![],
        ObservationAcquisitionV1::effect_free(hash(70), hash(71)).unwrap(),
    );
    let submission = SubmissionRefV1::for_work(
        WorkSubmissionIdV1::derive("stage5-resolved-claim-publication").unwrap(),
    )
    .unwrap();
    let claim = ClaimV1::new(
        submission,
        ClaimSubjectV1::for_work(
            WorkIdV1::derive("stage5-evidence-work").unwrap(),
            ContractRootIdV1::parse(&rendered(21)).unwrap(),
            vec![],
        )
        .unwrap(),
        hash(80),
        vec![resolved.id()],
    )
    .unwrap();

    EvidenceClaimPublicationV1::new(submission, vec![claim.clone()], vec![resolved.clone()])
        .unwrap();
    assert_eq!(
        EvidenceClaimPublicationV1::new(submission, vec![claim.clone()], vec![]).unwrap_err(),
        ClaimError::UnresolvedObservationReference
    );
    assert_eq!(
        EvidenceClaimPublicationV1::new(submission, vec![claim], vec![resolved.clone(), extra],)
            .unwrap_err(),
        ClaimError::UnreferencedObservationRecord
    );
    assert_eq!(
        EvidenceClaimPublicationV1::new(
            submission,
            vec![
                ClaimV1::new(
                    submission,
                    ClaimSubjectV1::for_work(
                        WorkIdV1::derive("stage5-resolved-claim-work").unwrap(),
                        ContractRootIdV1::parse(&rendered(21)).unwrap(),
                        vec![],
                    )
                    .unwrap(),
                    hash(81),
                    vec![resolved.id()],
                )
                .unwrap()
            ],
            vec![resolved.clone(), resolved],
        )
        .unwrap_err(),
        ClaimError::DuplicateObservationRecord
    );
}
