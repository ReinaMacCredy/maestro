use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::*;
use crate::domain::vnext::authority::test_support::{
    AuthorityFixtureModeV1, repository_authority_fixture,
};
use crate::domain::vnext::contract::assembly::{
    candidate_root_schema_closure_v1, facet_schema_id_v1, fixture_facet_value_v1,
    normative_inputs_schema_id_v1,
};
use crate::domain::vnext::contract::component::CandidateContractComponentV1;
use crate::domain::vnext::contract::component_kind::ContractComponentKindV1;
use crate::domain::vnext::contract::materialization::{
    ContractConsequencePlanV1, PlannedContractComponentV1, PlannedContractSlotV1,
};
use crate::domain::vnext::contract::provenance::ComponentProvenanceV1;
use crate::domain::vnext::contract::runtime::ContractGenerationIdV1;
use crate::domain::vnext::design::{
    AlternativeConsequenceV1, AlternativeRejectionV1, AlternativeV1, DecisionIdV1,
    DecisionRevisionV1, ExactRecordRefV1,
};
use crate::domain::vnext::identity::{DesignRevisionIdV1, DesignSourceBindingIdV1};
use crate::domain::vnext::persistence::StoreDomainV1;
use crate::domain::vnext::step::{
    StepBindingV1, StepGraphNodeV1, StepIdV1, StepRevisionIdV1, StepScopeV1,
};

#[test]
fn repository_action_closure_is_exactly_the_seven_stage3_leaves() {
    assert_eq!(RepositoryActionKindV1::ALL.len(), 7);
    assert_eq!(
        RepositoryActionKindV1::ALL.map(RepositoryActionKindV1::authority_leaf),
        [
            RepositoryActionLeafV1::CreateDraftWork,
            RepositoryActionLeafV1::CancelWork,
            RepositoryActionLeafV1::AbsorbWork,
            RepositoryActionLeafV1::PublishInitialContract,
            RepositoryActionLeafV1::AmendContract,
            RepositoryActionLeafV1::AppendDesignRevision,
            RepositoryActionLeafV1::ResolveDecision,
        ]
    );
    assert_eq!(
        RepositoryActionKindV1::ALL.map(RepositoryActionKindV1::tag),
        [1, 2, 4, 12, 13, 15, 20]
    );
}

#[test]
fn retained_actions_use_the_production_authority_fixture_closure() {
    let _revoked_mode_is_covered_by_authority_tests = AuthorityFixtureModeV1::RevokedGrant;
    let fixture = repository_authority_fixture(
        vec![("CreateDraftWork", [7; 32])],
        AuthorityFixtureModeV1::Valid,
    );
    assert!(!fixture.objects.is_empty());
    assert_ne!(fixture.authority_root_id.as_bytes(), &[0; 32]);
    assert_ne!(fixture.authenticated_human.identity(), [0; 32]);
    assert!(fixture.leaf_authority_expires_at > 0);
}

#[test]
fn create_draft_work_roots_the_new_record_in_one_authorized_commit() {
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"create-draft-work").expect("test fixture");
    let work_id = WorkIdV1::derive("created-draft").expect("test fixture");
    let fixture = repository_authority_fixture(
        vec![(
            "CreateDraftWork",
            work_subject_commitment(work_id).expect("test fixture"),
        )],
        AuthorityFixtureModeV1::Valid,
    );
    let contract_root = ContractRootIdV1::parse(&render_digest([55; 32])).expect("test fixture");
    let (mut store, head, generation) = active_store_with_roots(
        domain.clone(),
        contract_root,
        fixture.objects,
        vec![fixture.authority_root_id],
    );
    let publication = CreateDraftWorkPublicationV1::new(
        RepositoryActionIdentityV1::new(
            ActionRequestIdV1::derive("create-draft-request").expect("test fixture"),
            IdempotencyKeyIdV1::derive("create-draft-key").expect("test fixture"),
        ),
        basis(&head, &generation),
        fixture.selection,
        work_id,
    )
    .expect("test fixture");
    let expected = work_record_object(publication.work())
        .expect("test fixture")
        .id();

    let outcome = RepositoryStoreV1::new(&mut store)
        .create_draft_work(publication)
        .expect("test fixture");
    let committed = store
        .publication_generation(outcome.head().id())
        .expect("test fixture");
    assert!(committed.roots().contains(&expected));
    assert_eq!(
        store.read_object(expected).expect("test fixture"),
        work_record_object(
            &WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id).expect("test fixture")
        )
        .expect("test fixture")
    );
}

#[test]
fn repository_schema_closure_contains_only_stage3_store_carriers() {
    assert_eq!(RepositoryStoreSchemaV1::ALL.len(), 14);
    assert_eq!(
        RepositoryStoreSchemaV1::ALL.map(RepositoryStoreSchemaV1::domain),
        [
            REPOSITORY_ACTION_REQUEST_DOMAIN_V1,
            REPOSITORY_WORK_RECORD_DOMAIN_V1,
            REPOSITORY_DESIGN_STREAM_DOMAIN_V1,
            REPOSITORY_CONTRACT_REVISION_DOMAIN_V1,
            REPOSITORY_CONTRACT_GENERATION_DOMAIN_V1,
            REPOSITORY_FINALIZATION_MANIFEST_DOMAIN_V1,
            REPOSITORY_CONTRACT_ROOT_DOMAIN_V1,
            REPOSITORY_DECISION_DOMAIN_V1,
            REPOSITORY_STEP_GRAPH_DOMAIN_V1,
            REPOSITORY_STEP_STATE_DOMAIN_V1,
            REPOSITORY_STEP_AMENDMENT_AUDIT_DOMAIN_V1,
            REPOSITORY_DECISION_MATERIALIZATION_AUDIT_DOMAIN_V1,
            REPOSITORY_EXACT_EQUIVALENCE_RECEIPT_DOMAIN_V1,
            REPOSITORY_COMPONENT_INVALIDATION_RECEIPT_DOMAIN_V1,
        ]
    );
    for schema in RepositoryStoreSchemaV1::ALL {
        let domain = schema.domain().to_ascii_lowercase();
        assert!(!domain.contains("execution"));
        assert!(!domain.contains("evidence"));
        assert!(!domain.contains("gate"));
        schema.schema_id().expect("closed Stage 3 schema id");
    }
}

#[test]
fn contract_idempotency_meaning_includes_the_complete_step_subject_basis() {
    let fixture = repository_authority_fixture(
        vec![("PublishInitialContract", [7; 32])],
        AuthorityFixtureModeV1::Valid,
    );
    let identity = RepositoryActionIdentityV1::new(
        ActionRequestIdV1::derive("same-contract-request").expect("test fixture"),
        IdempotencyKeyIdV1::derive("same-contract-idempotency-key").expect("test fixture"),
    );
    let basis = RepositoryStoreBasisV1::new(
        StoreHeadIdV1::parse(&render_digest([11; 32])).expect("test fixture"),
        StoreGenerationIdV1::parse(&render_digest([12; 32])).expect("test fixture"),
        7,
        ContractRootIdV1::parse(&render_digest([13; 32])).expect("test fixture"),
    )
    .expect("test fixture");
    let request = |step_basis| {
        action_request_object(
            RepositoryActionKindV1::PublishInitialContract,
            identity,
            basis,
            fixture.selection,
            [14; 32],
            step_basis,
            CborValue::Bytes(vec![15; 32]),
        )
        .expect("test fixture")
    };
    let graph_a = request([16; 32]);
    let graph_b = request([17; 32]);
    assert_ne!(graph_a.id(), graph_b.id());
    assert_ne!(
        Sha256::digest(graph_a.canonical_bytes()),
        Sha256::digest(graph_b.canonical_bytes())
    );
}

#[test]
fn equal_root_contract_amendment_accepts_only_an_exact_zero_write_shape() {
    let reason = WorkTransitionReasonV1::new("contract amendment fixture").expect("test fixture");
    let amend = WorkTransitionV1::AmendContract {
        invalidated_submission_id: None,
        reason: reason.clone(),
    };
    let cancel = WorkTransitionV1::CancelWork { reason };

    assert!(validate_contract_amendment_mode(
        true, None, false, true, true,
    ));
    assert!(!validate_contract_amendment_mode(
        true,
        Some(&amend),
        false,
        true,
        true,
    ));
    assert!(!validate_contract_amendment_mode(
        true, None, true, true, true,
    ));
    assert!(!validate_contract_amendment_mode(
        true, None, false, false, true,
    ));
    assert!(!validate_contract_amendment_mode(
        true, None, false, true, false,
    ));

    assert!(validate_contract_amendment_mode(
        false,
        Some(&amend),
        true,
        true,
        false,
    ));
    assert!(!validate_contract_amendment_mode(
        false, None, true, true, false,
    ));
    assert!(!validate_contract_amendment_mode(
        false,
        Some(&cancel),
        true,
        true,
        false,
    ));
    assert!(!validate_contract_amendment_mode(
        false,
        Some(&amend),
        false,
        true,
        false,
    ));
    assert!(!validate_contract_amendment_mode(
        false,
        Some(&amend),
        true,
        true,
        true,
    ));
}

#[test]
fn contract_amendment_current_step_states_are_an_exact_graph_partition() {
    let repository =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"contract-amendment-state-set")
            .expect("test fixture");
    let work_id = WorkIdV1::derive("contract-amendment-state-work").expect("test fixture");
    let scope = StepScopeV1::new(repository.id(), work_id);
    let generation_id =
        ContractGenerationIdV1::parse(&render_digest([21; 32])).expect("test fixture");
    let root_id = ContractRootIdV1::parse(&render_digest([22; 32])).expect("test fixture");
    let binding = |key: &str, revision: u8| {
        StepBindingV1::new(
            scope,
            generation_id,
            root_id,
            StepIdV1::new(scope, key).expect("test fixture"),
            StepRevisionIdV1::from_bytes([revision; 32]).expect("test fixture"),
        )
        .expect("test fixture")
    };
    let first = binding("first", 31);
    let second = binding("second", 32);
    let outsider = binding("outsider", 33);
    let graph = StepGraphSnapshotV1::new(
        scope,
        generation_id,
        root_id,
        vec![
            StepGraphNodeV1::new(first, true).expect("test fixture"),
            StepGraphNodeV1::new(second, true).expect("test fixture"),
        ],
        vec![],
    )
    .expect("test fixture");
    let first_state = StepStateV1::new_open(first);
    let second_state = StepStateV1::new_open(second);

    assert!(validate_current_step_state_set(
        &graph,
        &[second_state, first_state],
    ));
    assert!(!validate_current_step_state_set(&graph, &[first_state]));
    assert!(!validate_current_step_state_set(
        &graph,
        &[first_state, first_state],
    ));
    assert!(!validate_current_step_state_set(
        &graph,
        &[first_state, StepStateV1::new_open(outsider)],
    ));
}

#[test]
fn rooted_step_state_partition_rejects_hidden_current_states_but_allows_other_history() {
    let domain = StoreDomainV1::derive(
        StoreRoleV1::Repository,
        b"rooted-contract-amendment-state-partition",
    )
    .expect("test fixture");
    let work_id = WorkIdV1::derive("rooted-step-partition-work").expect("test fixture");
    let other_work_id = WorkIdV1::derive("other-rooted-step-work").expect("test fixture");
    let scope = StepScopeV1::new(domain.id(), work_id);
    let other_scope = StepScopeV1::new(domain.id(), other_work_id);
    let generation_id =
        ContractGenerationIdV1::parse(&render_digest([41; 32])).expect("test fixture");
    let historical_generation_id =
        ContractGenerationIdV1::parse(&render_digest([40; 32])).expect("test fixture");
    let root_id = ContractRootIdV1::parse(&render_digest([42; 32])).expect("test fixture");
    let binding =
        |scope: StepScopeV1, generation_id: ContractGenerationIdV1, key: &str, revision: u8| {
            StepBindingV1::new(
                scope,
                generation_id,
                root_id,
                StepIdV1::new(scope, key).expect("test fixture"),
                StepRevisionIdV1::from_bytes([revision; 32]).expect("test fixture"),
            )
            .expect("test fixture")
        };
    let first = binding(scope, generation_id, "first", 51);
    let second = binding(scope, generation_id, "second", 52);
    let outsider = binding(scope, generation_id, "outsider", 53);
    let historical = binding(scope, historical_generation_id, "historical", 54);
    let other_work = binding(other_scope, generation_id, "other-work", 55);
    let graph = StepGraphSnapshotV1::new(
        scope,
        generation_id,
        root_id,
        vec![
            StepGraphNodeV1::new(first, true).expect("test fixture"),
            StepGraphNodeV1::new(second, true).expect("test fixture"),
        ],
        vec![],
    )
    .expect("test fixture");
    let expected = vec![
        step_state_object(&StepStateV1::new_open(first)).expect("test fixture"),
        step_state_object(&StepStateV1::new_open(second)).expect("test fixture"),
    ];
    let historical_object =
        step_state_object(&StepStateV1::new_open(historical)).expect("test fixture");
    let other_work_object =
        step_state_object(&StepStateV1::new_open(other_work)).expect("test fixture");
    let generation = |objects: &[StoreObjectV1]| {
        let mut roots = objects.iter().map(StoreObjectV1::id).collect::<Vec<_>>();
        roots.sort_unstable();
        roots.dedup();
        StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root_id,
            StoreCompatibilityV1::stage0_successor().expect("test fixture"),
            roots,
        )
        .expect("test fixture")
    };
    let allowed_objects = vec![
        expected[0].clone(),
        expected[1].clone(),
        historical_object,
        other_work_object,
    ];
    validate_rooted_step_state_partition(
        &generation(&allowed_objects),
        &allowed_objects,
        &graph,
        &expected,
    )
    .expect("historical and other-Work states are outside the current Contract partition");

    let mut outsider_objects = allowed_objects.clone();
    outsider_objects
        .push(step_state_object(&StepStateV1::new_open(outsider)).expect("test fixture"));
    assert!(matches!(
        validate_rooted_step_state_partition(
            &generation(&outsider_objects),
            &outsider_objects,
            &graph,
            &expected,
        ),
        Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch)
    ));

    let duplicate_lifecycle = StepStateV1::new_open(first)
        .cancel([56; 32])
        .expect("test fixture");
    let mut duplicate_objects = allowed_objects.clone();
    duplicate_objects.push(step_state_object(&duplicate_lifecycle).expect("test fixture"));
    assert!(matches!(
        validate_rooted_step_state_partition(
            &generation(&duplicate_objects),
            &duplicate_objects,
            &graph,
            &expected,
        ),
        Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch)
    ));

    let candidate_generation_id =
        ContractGenerationIdV1::parse(&render_digest([43; 32])).expect("test fixture");
    let candidate_root_id =
        ContractRootIdV1::parse(&render_digest([44; 32])).expect("test fixture");
    let candidate_binding = |key: &str, revision: u8| {
        StepBindingV1::new(
            scope,
            candidate_generation_id,
            candidate_root_id,
            StepIdV1::new(scope, key).expect("test fixture"),
            StepRevisionIdV1::from_bytes([revision; 32]).expect("test fixture"),
        )
        .expect("test fixture")
    };
    let candidate_first = candidate_binding("first", 61);
    let candidate_second = candidate_binding("second", 62);
    let pre_rooted_candidate_outsider = candidate_binding("pre-rooted-outsider", 63);
    let candidate_graph = StepGraphSnapshotV1::new(
        scope,
        candidate_generation_id,
        candidate_root_id,
        vec![
            StepGraphNodeV1::new(candidate_first, true).expect("test fixture"),
            StepGraphNodeV1::new(candidate_second, true).expect("test fixture"),
        ],
        vec![],
    )
    .expect("test fixture");
    let next_objects = vec![
        step_state_object(&StepStateV1::new_open(candidate_first)).expect("test fixture"),
        step_state_object(&StepStateV1::new_open(candidate_second)).expect("test fixture"),
    ];
    let mut successor_objects = allowed_objects.clone();
    successor_objects.extend(next_objects.iter().cloned());
    successor_objects.push(
        step_state_object(&StepStateV1::new_open(pre_rooted_candidate_outsider))
            .expect("test fixture"),
    );
    let mut successor_roots = successor_objects
        .iter()
        .map(StoreObjectV1::id)
        .collect::<Vec<_>>();
    successor_roots.sort_unstable();
    successor_roots.dedup();
    let successor_generation = StoreGenerationV1::new(
        domain.clone(),
        2,
        Some(generation(&allowed_objects).id()),
        candidate_root_id,
        StoreCompatibilityV1::stage0_successor().expect("test fixture"),
        successor_roots,
    )
    .expect("test fixture");
    assert!(matches!(
        validate_rooted_step_state_partition(
            &successor_generation,
            &successor_objects,
            &candidate_graph,
            &next_objects,
        ),
        Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch)
    ));
}

#[test]
fn equal_root_materialization_is_store_validated_without_an_action() {
    let (schemas, base) = materialization_schema_and_root(31);
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"equal-root-no-op").expect("test fixture");
    let work_id = WorkIdV1::derive("equal-root-work").expect("test fixture");
    let resolved = resolved_materialization_decision(
        domain.id(),
        work_id,
        &base,
        MaterializationFixtureV1::EqualRoot,
    );
    let work = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id).expect("test fixture");
    let work_object = work_record_object(&work).expect("test fixture");
    let decision = decision_object(&resolved).expect("test fixture");
    let base_object = contract_root_object(&base).expect("test fixture");
    let (mut store, head, generation) = active_store(
        domain,
        *base.root_id(),
        vec![work_object, decision.clone(), base_object.clone()],
    );

    let prepared = DecisionMaterializationCandidateV1::prepare(
        basis(&head, &generation),
        work,
        resolved.clone(),
        &schemas,
        &base,
        vec![],
    )
    .expect("test fixture");
    let DecisionMaterializationRepositoryPreparationV1::EqualRoot(no_op) = prepared else {
        panic!("retaining the exact component closure must be equal-root")
    };
    let materialization = RepositoryStoreV1::new(&mut store)
        .validate_equal_root_materialization(&no_op)
        .expect("test fixture");
    assert!(matches!(
        materialization.disposition(),
        crate::domain::vnext::design::DecisionMaterializationDispositionV1::NoOpEqualRoot
    ));
    assert_eq!(
        store
            .active_head()
            .expect("test fixture")
            .expect("test fixture"),
        head
    );
    assert_eq!(
        store
            .publication_generation(head.id())
            .expect("test fixture"),
        generation
    );
}

#[test]
fn terminal_work_rejects_equal_and_distinct_materialization_before_store_access() {
    let (schemas, base) = materialization_schema_and_root(36);
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"terminal-materialization")
        .expect("test fixture");
    let work_id = WorkIdV1::derive("terminal-materialization-work").expect("test fixture");
    let terminal_work = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id)
        .expect("test fixture")
        .apply(
            WorkRecordWriterV1::Work,
            crate::domain::vnext::work::WorkRevisionV1::new(1).expect("test fixture"),
            WorkTransitionV1::CancelWork {
                reason: WorkTransitionReasonV1::new("terminal work cannot be rematerialized")
                    .expect("test fixture"),
            },
        )
        .expect("test fixture");
    let basis = RepositoryStoreBasisV1::new(
        StoreHeadIdV1::parse(&render_digest([36; 32])).expect("test fixture"),
        StoreGenerationIdV1::parse(&render_digest([37; 32])).expect("test fixture"),
        1,
        *base.root_id(),
    )
    .expect("test fixture");

    for fixture in [
        MaterializationFixtureV1::EqualRoot,
        MaterializationFixtureV1::ChangedReplacement,
    ] {
        let resolved = resolved_materialization_decision(domain.id(), work_id, &base, fixture);
        assert!(matches!(
            DecisionMaterializationCandidateV1::prepare(
                basis,
                terminal_work.clone(),
                resolved,
                &schemas,
                &base,
                vec![],
            ),
            Err(RepositoryPublicationErrorV1::MaterializationBasisMismatch)
        ));
    }
}

#[test]
fn materialization_chain_accepts_replacement_and_rejects_duplicate_resolution() {
    let (schemas, base) = materialization_schema_and_root(38);
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"materialization-chain")
        .expect("test fixture");
    let work_id = WorkIdV1::derive("materialization-chain-work").expect("test fixture");
    let work = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id).expect("test fixture");
    let resolved = resolved_materialization_decision(
        domain.id(),
        work_id,
        &base,
        MaterializationFixtureV1::ChangedReplacement,
    );
    let store_basis = RepositoryStoreBasisV1::new(
        StoreHeadIdV1::parse(&render_digest([38; 32])).expect("test fixture"),
        StoreGenerationIdV1::parse(&render_digest([39; 32])).expect("test fixture"),
        1,
        *base.root_id(),
    )
    .expect("test fixture");
    let prepared = DecisionMaterializationCandidateV1::prepare(
        store_basis,
        work.clone(),
        resolved.clone(),
        &schemas,
        &base,
        vec![],
    )
    .expect("test fixture");
    let DecisionMaterializationRepositoryPreparationV1::Candidate(candidate) = prepared else {
        panic!("changed replacement must produce a materialization candidate")
    };
    let candidate_root = candidate.preflight.candidate_root().clone();
    assert!(validate_materialization_chain(
        store_basis,
        &work,
        &base,
        &candidate_root,
        std::slice::from_ref(&candidate),
    ));

    let duplicate = DecisionMaterializationCandidateV1::prepare(
        store_basis,
        work.clone(),
        resolved,
        &schemas,
        &base,
        vec![],
    )
    .expect("test fixture");
    let DecisionMaterializationRepositoryPreparationV1::Candidate(duplicate) = duplicate else {
        panic!("changed replacement must produce a materialization candidate")
    };
    assert!(!validate_materialization_chain(
        store_basis,
        &work,
        &base,
        &candidate_root,
        &[*candidate, *duplicate],
    ));
}

#[test]
fn distinct_exactly_equivalent_root_is_receipt_proven_and_zero_write() {
    let (schemas, base) = materialization_schema_and_root(41);
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"equivalent-root-no-op")
        .expect("test fixture");
    let work_id = WorkIdV1::derive("equivalent-root-work").expect("test fixture");
    let changed = resolved_materialization_decision(
        domain.id(),
        work_id,
        &base,
        MaterializationFixtureV1::ChangedReplacement,
    );
    let changed_preflight = DecisionMaterializationV1::preflight(
        changed.resolution().expect("test fixture"),
        &schemas,
        &base,
    )
    .expect("test fixture");
    assert!(matches!(
        exact_equivalence_receipt_object(
            &changed,
            &changed_preflight,
            &decision_object(&changed).expect("test fixture"),
            &contract_root_object(&base).expect("test fixture"),
            &contract_root_object(changed_preflight.candidate_root()).expect("test fixture"),
        ),
        Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch)
    ));
    let resolved = resolved_materialization_decision(
        domain.id(),
        work_id,
        &base,
        MaterializationFixtureV1::EquivalentReplacement,
    );
    let work = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id).expect("test fixture");
    let work_object = work_record_object(&work).expect("test fixture");
    let preflight = DecisionMaterializationV1::preflight(
        resolved.resolution().expect("test fixture"),
        &schemas,
        &base,
    )
    .expect("test fixture");
    assert!(!preflight.is_equal_root());
    let decision = decision_object(&resolved).expect("test fixture");
    let base_object = contract_root_object(&base).expect("test fixture");
    let candidate_object = contract_root_object(preflight.candidate_root()).expect("test fixture");
    let receipt = exact_equivalence_receipt_object(
        &resolved,
        &preflight,
        &decision,
        &base_object,
        &candidate_object,
    )
    .expect("pinned evaluator proves exact semantic equivalence");
    let roots = vec![work_object.id(), decision.id(), base_object.id()];
    let (mut store, head, generation) = active_store_with_roots(
        domain.clone(),
        *base.root_id(),
        vec![work_object, decision, base_object, candidate_object],
        roots,
    );
    let prepared = DecisionMaterializationCandidateV1::prepare(
        basis(&head, &generation),
        work.clone(),
        resolved.clone(),
        &schemas,
        &base,
        vec![],
    )
    .expect("test fixture");
    let DecisionMaterializationRepositoryPreparationV1::Candidate(candidate) = prepared else {
        panic!("distinct candidate roots require receipt validation")
    };

    let stale = RepositoryStoreBasisV1::new(
        StoreHeadIdV1::parse(&render_digest([99; 32])).expect("test fixture"),
        generation.id(),
        generation.ordinal(),
        generation.contract_root_id(),
    )
    .expect("test fixture");
    let stale_candidate = DecisionMaterializationCandidateV1 {
        store_basis: stale,
        current_work: candidate.current_work.clone(),
        current_work_object: candidate.current_work_object.clone(),
        resolved_decision: candidate.resolved_decision.clone(),
        preflight: DecisionMaterializationV1::preflight(
            candidate
                .resolved_decision
                .resolution()
                .expect("test fixture"),
            &schemas,
            &base,
        )
        .expect("test fixture"),
        base_root_object: candidate.base_root_object.clone(),
        candidate_root_object: candidate.candidate_root_object.clone(),
        decision_object: candidate.decision_object.clone(),
        invalidation_receipts: vec![],
    };
    let expected_receipt_id = candidate
        .evaluate_exact_equivalence_receipt()
        .expect("pinned evaluator receipt")
        .id();
    assert_eq!(expected_receipt_id, receipt.id());
    let materialization = RepositoryStoreV1::new(&mut store)
        .validate_exactly_equivalent_materialization(*candidate)
        .expect("test fixture");
    assert!(matches!(
        materialization.disposition(),
        crate::domain::vnext::design::DecisionMaterializationDispositionV1::NoOpEquivalent { .. }
    ));
    assert!(materialization.authorization().is_none());
    assert_eq!(
        store
            .active_head()
            .expect("test fixture")
            .expect("test fixture"),
        head
    );
    assert_eq!(
        store
            .publication_generation(head.id())
            .expect("test fixture"),
        generation
    );

    assert!(matches!(
        RepositoryStoreV1::new(&mut store)
            .validate_exactly_equivalent_materialization(stale_candidate),
        Err(RepositoryPublicationErrorV1::StaleStoreBasis)
    ));
    assert_eq!(
        store
            .active_head()
            .expect("test fixture")
            .expect("test fixture"),
        head
    );
}

fn active_store(
    domain: StoreDomainV1,
    contract_root: ContractRootIdV1,
    objects: Vec<StoreObjectV1>,
) -> (StoreV1, StoreHeadV1, StoreGenerationV1) {
    let roots = objects.iter().map(StoreObjectV1::id).collect();
    active_store_with_roots(domain, contract_root, objects, roots)
}

fn active_store_with_roots(
    domain: StoreDomainV1,
    contract_root: ContractRootIdV1,
    objects: Vec<StoreObjectV1>,
    mut roots: Vec<StoreObjectIdV1>,
) -> (StoreV1, StoreHeadV1, StoreGenerationV1) {
    let root = test_root();
    let mut store = StoreV1::create(&root, domain.clone()).expect("test fixture");
    put_objects_in_reference_order(&mut store, objects);
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        contract_root,
        StoreCompatibilityV1::stage0_successor().expect("test fixture"),
        roots,
    )
    .expect("test fixture");
    store
        .publish_generation(&generation, None)
        .expect("test fixture");
    activate_store(&root);
    let head = store
        .active_head()
        .expect("test fixture")
        .expect("test fixture");
    (store, head, generation)
}

fn put_objects_in_reference_order(store: &mut StoreV1, objects: Vec<StoreObjectV1>) {
    let mut pending = objects;
    let mut inserted = std::collections::BTreeSet::new();
    while !pending.is_empty() {
        let index = pending
            .iter()
            .position(|object| {
                object
                    .references()
                    .iter()
                    .all(|reference| inserted.contains(reference))
            })
            .expect("fixture Store objects form a closed DAG");
        let object = pending.remove(index);
        store.put_object(&object).expect("test fixture");
        inserted.insert(object.id());
    }
}

fn basis(head: &StoreHeadV1, generation: &StoreGenerationV1) -> RepositoryStoreBasisV1 {
    RepositoryStoreBasisV1::new(
        head.id(),
        generation.id(),
        generation.ordinal(),
        generation.contract_root_id(),
    )
    .expect("test fixture")
}

fn materialization_schema_and_root(seed: u8) -> (SchemaClosureV1, CandidateContractRootV1) {
    let schemas = candidate_root_schema_closure_v1().expect("test fixture");
    let rendered = |value: u8| format!("sha256:{}", format!("{value:02x}").repeat(32));
    let design_revision_id = DesignRevisionIdV1::parse(&rendered(seed)).expect("test fixture");
    let source_binding_id =
        DesignSourceBindingIdV1::parse(&rendered(seed.saturating_add(1))).expect("test fixture");
    let components = ContractComponentKindV1::ALL
        .into_iter()
        .map(|kind| {
            let (schema_id, value) = if kind == ContractComponentKindV1::NormativeInputs {
                (
                    normative_inputs_schema_id_v1(&schemas).expect("test fixture"),
                    CborValue::Array(vec![
                        CborValue::Unsigned(1),
                        bytes(&[seed; 32]),
                        CborValue::Array(Vec::new()),
                    ]),
                )
            } else {
                (
                    facet_schema_id_v1(&schemas, kind).expect("test fixture"),
                    fixture_facet_value_v1(kind, [seed; 32], vec![[seed; 32]]),
                )
            };
            CandidateContractComponentV1::new(
                &schemas,
                kind,
                schema_id,
                value,
                vec![],
                ComponentProvenanceV1::design_slot(
                    design_revision_id,
                    kind.tag(),
                    source_binding_id,
                )
                .expect("test fixture"),
            )
            .expect("test fixture")
        })
        .collect();
    let root = CandidateContractRootV1::new(&schemas, components).expect("test fixture");
    (schemas, root)
}

fn resolved_materialization_decision(
    repository_id: crate::domain::vnext::identity::StoreDomainIdV1,
    work_id: WorkIdV1,
    base: &CandidateContractRootV1,
    fixture: MaterializationFixtureV1,
) -> DecisionV1 {
    let schemas = candidate_root_schema_closure_v1().expect("test fixture");
    let (retained, additions) = if fixture != MaterializationFixtureV1::EqualRoot {
        let replaced = base
            .components()
            .iter()
            .find(|component| component.kind() == ContractComponentKindV1::IntendedOutcome)
            .expect("test fixture");
        let retained = base
            .components()
            .iter()
            .filter(|component| component.component_id() != replaced.component_id())
            .map(|component| *component.component_id())
            .collect();
        let addition = PlannedContractComponentV1::new(
            PlannedContractSlotV1::new(1).expect("test fixture"),
            ContractComponentKindV1::IntendedOutcome,
            facet_schema_id_v1(&schemas, ContractComponentKindV1::IntendedOutcome)
                .expect("test fixture"),
            if fixture == MaterializationFixtureV1::EquivalentReplacement {
                replaced.value().clone()
            } else {
                fixture_facet_value_v1(ContractComponentKindV1::IntendedOutcome, [99; 32], vec![])
            },
            vec![],
        )
        .expect("test fixture");
        (retained, vec![addition])
    } else {
        (
            base.components()
                .iter()
                .map(|component| *component.component_id())
                .collect(),
            vec![],
        )
    };
    let plan = ContractConsequencePlanV1::new(7, base, retained, additions).expect("test fixture");
    let decision_id = DecisionIdV1::new(if fixture != MaterializationFixtureV1::EqualRoot {
        "repository-distinct-materialization"
    } else {
        "repository-equal-materialization"
    })
    .expect("test fixture");
    let alternatives = vec![
        AlternativeV1::new(
            b"no effect".to_vec(),
            b"no Contract effect".to_vec(),
            AlternativeConsequenceV1::NoContractEffect,
        )
        .expect("test fixture"),
        AlternativeV1::new(
            b"apply exact plan".to_vec(),
            b"derive candidate closure".to_vec(),
            AlternativeConsequenceV1::typed_plan(plan.clone()),
        )
        .expect("test fixture"),
    ];
    let revision = DecisionRevisionV1::new(
        decision_id.clone(),
        1,
        None,
        b"which exact transformation?".to_vec(),
        ExactRecordRefV1::from_digest([31; 32]),
        *plan.base_root_id(),
        alternatives,
    )
    .expect("test fixture");
    let selected = *revision.alternatives()[1].alternative_id();
    let rejected = AlternativeRejectionV1::new(
        *revision.alternatives()[0].alternative_id(),
        b"the exact transformation is required".to_vec(),
    )
    .expect("test fixture");
    let decision =
        DecisionV1::new(repository_id, work_id, decision_id, revision).expect("test fixture");
    decision
        .resolve(
            decision.head().revision_id(),
            &selected,
            b"apply the selected exact plan".to_vec(),
            vec![rejected],
            &AdmittedCommittedActionV1::fixture("repository-materialization-resolution"),
            WorkDecisionEligibilityV1::Eligible,
        )
        .expect("test fixture")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MaterializationFixtureV1 {
    EqualRoot,
    EquivalentReplacement,
    ChangedReplacement,
}

fn activate_store(root: &std::path::Path) {
    let connection = Connection::open(root.join("store.sqlite3")).expect("test fixture");
    assert_eq!(
        connection
            .execute(
                "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                [],
            )
            .expect("test fixture"),
        1
    );
}

fn test_root() -> std::path::PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test fixture")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "maestro-vnext-stage3-repository-{}-{nonce}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&path).expect("test fixture");
    fs::canonicalize(path).expect("test fixture")
}
