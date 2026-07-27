use std::collections::BTreeSet;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::domain::vnext::authority::test_support::{
    AuthorityFixtureModeV1, RepositoryAuthorityFixtureV1, repository_owner_family_authority_fixture,
};
use crate::domain::vnext::authority::{
    ActionRequestIdV1, AuthorityContextIdV1, PrincipalIdV1, RepositoryDownstreamActionLeafV1,
    SessionIdV1,
};
use crate::domain::vnext::identity::{ContractRootIdV1, SchemaIdV1};
use crate::domain::vnext::persistence::{
    StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreIdempotencyProbeV1, StoreObjectV1,
    StorePublicationOutcomeV1, StoreRoleV1, StoreV1,
};
use crate::domain::vnext::repository::RepositoryStoreSchemaV1;
use crate::foundation::core::deterministic_cbor::{self, CborValue};

use super::evaluation::classify_policy_diff;
use super::state::{PlanningStateErrorV1, PlanningTransitionDispositionV1, test_adapter};
use super::*;

fn actor() -> (PrincipalIdV1, SessionIdV1) {
    (
        PrincipalIdV1::derive("stage7-planning-actor").expect("stage7 planning test invariant"),
        SessionIdV1::derive("stage7-planning-session").expect("stage7 planning test invariant"),
    )
}

fn digest(seed: &str) -> [u8; 32] {
    Sha256::digest(seed.as_bytes()).into()
}

fn test_root() -> std::path::PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("stage7 planning test invariant")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "maestro-vnext-stage7-planning-{}-{nonce}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&path).expect("stage7 planning test invariant");
    fs::canonicalize(path).expect("stage7 planning test invariant")
}

fn put_objects_in_reference_order(store: &mut StoreV1, objects: Vec<StoreObjectV1>) {
    let mut pending = objects;
    let mut inserted = BTreeSet::new();
    while !pending.is_empty() {
        let index = pending
            .iter()
            .position(|object| {
                object
                    .references()
                    .iter()
                    .all(|reference| inserted.contains(reference))
            })
            .expect("Store Object references must form an acyclic closed set");
        let object = pending.remove(index);
        store
            .put_object(&object)
            .expect("stage7 planning test invariant");
        inserted.insert(object.id());
    }
}

// Authority owns the tag-25 repository-governance-floor snapshot that every
// Action-105 publication reads live, and exposes no crate-visible fixture for
// it, so the Stage-7 store setup mirrors the frozen genesis encoding here.
// Authority decodes, revalidates and recomputes the semantic hash inside the
// transaction, so any drift from the frozen schema fails this test loudly.
const GOVERNANCE_FLOOR_SCHEMA_FIELDS: [&str; 19] = [
    "repository_store_domain",
    "authority_context",
    "floor_revision",
    "predecessor",
    "activation_basis",
    "activation_generation_ordinal",
    "authority_epoch",
    "trust_root_revision",
    "trust_root_binding_commitment",
    "authority_transition_protocol_identity",
    "authority_transition_protocol_version",
    "minimum_assurance",
    "requirement_grammar_identity",
    "requirement_evaluator_identity",
    "requirement_evaluator_revision",
    "requirement_rows",
    "semantic_hash",
    "canonicalization_version",
    "protocol_version",
];

const GOVERNANCE_FLOOR_SCHEMA_VARIANTS: [&str; 3] = [
    "repository_genesis",
    "explicit_legacy_migration",
    "guarded_rotation",
];

const GOVERNANCE_FLOOR_SCHEMA_INVARIANTS: [&str; 12] = [
    "append_only_authority_schema_tag_25",
    "internal_non_public_schema",
    "exactly_one_direct_floor_root",
    "repository_governance_head_class_8_exact_closure",
    "gap_free_predecessor_history",
    "same_repository_and_authority_context",
    "action_105_requirement_exactly_once",
    "planning_authority_persistence_participants",
    "semantic_hash_recomputed",
    "old_writer_preserves_unknown_root",
    "restore_requires_exact_same_domain_chain",
    "immutable_non_authorizing_record",
];

fn governance_floor_schema_id() -> SchemaIdV1 {
    let descriptor = CborValue::Array(vec![
        CborValue::Unsigned(25),
        CborValue::text("RepositoryGovernanceFloorSnapshotV1")
            .expect("stage7 planning test invariant"),
        CborValue::Array(
            GOVERNANCE_FLOOR_SCHEMA_FIELDS
                .iter()
                .map(|field| CborValue::text(*field).expect("stage7 planning test invariant"))
                .collect(),
        ),
        CborValue::Array(
            GOVERNANCE_FLOOR_SCHEMA_VARIANTS
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(index as u64 + 1),
                        CborValue::text(*variant).expect("stage7 planning test invariant"),
                    ])
                })
                .collect(),
        ),
        CborValue::Array(
            GOVERNANCE_FLOOR_SCHEMA_INVARIANTS
                .iter()
                .map(|invariant| {
                    CborValue::text(*invariant).expect("stage7 planning test invariant")
                })
                .collect(),
        ),
    ]);
    let envelope = CborValue::Array(vec![
        CborValue::text("maestro.vnext.stage2.authority.schema-descriptor.v1")
            .expect("stage7 planning test invariant"),
        descriptor,
    ]);
    SchemaIdV1::from_digest(canonical_digest(&envelope))
}

fn canonical_digest(value: &CborValue) -> [u8; 32] {
    Sha256::digest(deterministic_cbor::encode(value).expect("stage7 planning test invariant"))
        .into()
}

fn length_prefixed_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut writer = Sha256::new();
    writer.update((domain.len() as u64).to_be_bytes());
    writer.update(domain);
    for field in fields {
        writer.update((field.len() as u64).to_be_bytes());
        writer.update(field);
    }
    writer.finalize().into()
}

fn governance_action_105_requirement_row() -> CborValue {
    let participants = || {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Unsigned(7),
            CborValue::Unsigned(14),
        ])
    };
    CborValue::Array(vec![
        CborValue::Unsigned(105),
        CborValue::Unsigned(12),
        participants(),
        CborValue::Unsigned(1),
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Array(vec![
                CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(12)]),
                CborValue::Array(vec![CborValue::Unsigned(3), participants()]),
                CborValue::Array(vec![CborValue::Unsigned(4), CborValue::Unsigned(1)]),
                CborValue::Array(vec![CborValue::Unsigned(5)]),
                CborValue::Array(vec![CborValue::Unsigned(6)]),
            ]),
        ]),
    ])
}

fn repository_governance_floor_genesis_object(
    domain: &StoreDomainV1,
    fixture: &RepositoryAuthorityFixtureV1,
) -> StoreObjectV1 {
    let context = AuthorityContextIdV1::derive("stage3-repository-context")
        .expect("stage7 planning test invariant");
    let trust_root_binding_commitment = length_prefixed_digest(
        b"maestro.authority.repository-governance-trust-root-binding.v1\0",
        &[
            context.as_bytes(),
            fixture.selection.actor_binding_id().as_bytes(),
            fixture.selection.actor_session_id().as_bytes(),
            &11_u64.to_be_bytes(),
        ],
    );
    let protocol_identity = digest("repository-governance-transition-protocol");
    let grammar_identity = digest("repository-governance-requirement-grammar");
    let evaluator_identity = digest("repository-governance-requirement-evaluator");
    let requirement_rows = CborValue::Array(vec![governance_action_105_requirement_row()]);
    let semantic_hash = canonical_digest(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-governance-floor-semantic.v1")
            .expect("stage7 planning test invariant"),
        CborValue::Bytes(protocol_identity.to_vec()),
        CborValue::Unsigned(1),
        CborValue::Unsigned(1),
        CborValue::Bytes(grammar_identity.to_vec()),
        CborValue::Bytes(evaluator_identity.to_vec()),
        CborValue::Unsigned(1),
        requirement_rows.clone(),
    ]));
    let value = CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-governance-floor-snapshot.v1")
            .expect("stage7 planning test invariant"),
        CborValue::Bytes(domain.id().as_bytes().to_vec()),
        CborValue::Bytes(context.as_bytes().to_vec()),
        CborValue::Unsigned(1),
        CborValue::optional(None),
        CborValue::Unsigned(1),
        CborValue::Unsigned(1),
        CborValue::Unsigned(fixture.authority_epoch),
        CborValue::Unsigned(11),
        CborValue::Bytes(trust_root_binding_commitment.to_vec()),
        CborValue::Bytes(protocol_identity.to_vec()),
        CborValue::Unsigned(1),
        CborValue::Unsigned(1),
        CborValue::Bytes(grammar_identity.to_vec()),
        CborValue::Bytes(evaluator_identity.to_vec()),
        CborValue::Unsigned(1),
        requirement_rows,
        CborValue::Bytes(semantic_hash.to_vec()),
        CborValue::Unsigned(1),
        CborValue::Unsigned(1),
    ]);
    StoreObjectV1::new(governance_floor_schema_id(), value, vec![])
        .expect("stage7 planning test invariant")
}

fn opportunity(action_ref: &str, stamp: u8) -> SchedulingOpportunityRefV1 {
    SchedulingOpportunityRefV1::Action {
        action_ref: action_ref.into(),
        material_dependency_stamp: [stamp; 32],
    }
}

fn opportunity_set() -> SchedulingOpportunitySetV1 {
    SchedulingOpportunitySetV1::new(
        [1; 32],
        vec![opportunity("action:a", 2), opportunity("action:b", 3)],
        [4; 32],
        true,
    )
    .expect("stage7 planning test invariant")
}

fn floor() -> SchedulingSafetyFloorV1 {
    SchedulingSafetyFloorV1::new(
        "safety-floor:v1".into(),
        "core:evaluator:v1".into(),
        [5; 32],
        1,
        1000,
        1000,
        1000,
        1000,
    )
    .expect("stage7 planning test invariant")
}

fn policy(
    name: &str,
    foundation: u64,
    fairness: u64,
    hysteresis: u64,
    overload: u64,
) -> SchedulingPolicySnapshotV1 {
    SchedulingPolicySnapshotV1::new(
        format!("policy:{name}"),
        "evaluator:v1".into(),
        1,
        "core:evaluator:v1".into(),
        foundation,
        fairness,
        hysteresis,
        overload,
    )
    .expect("stage7 planning test invariant")
}

fn binding(
    policy: SchedulingPolicySnapshotV1,
    old: Option<&SchedulingPolicyBindingV1>,
    floor: &SchedulingSafetyFloorV1,
) -> SchedulingPolicyBindingV1 {
    let diff = classify_policy_diff(old.map(|row| &row.policy), &policy, floor)
        .expect("stage7 planning test invariant");
    SchedulingPolicyBindingV1::new(
        "repo:stage7".into(),
        "generation:7".into(),
        old.map_or(1, |row| row.revision + 1),
        old.map(|row| row.semantic_hash),
        policy,
        diff,
    )
    .expect("stage7 planning test invariant")
}

fn scheduling_policy_transition(
    actor_principal: PrincipalIdV1,
    actor_session: SessionIdV1,
    binding: SchedulingPolicyBindingV1,
    safety_floor: SchedulingSafetyFloorV1,
) -> PlanningTransitionV1 {
    apply_planning_mutation(
        &mut PlanningStateV1::default(),
        PlanningMutationV1::PublishSchedulingPolicyBinding {
            actor_principal,
            actor_session,
            binding,
            safety_floor,
        },
    )
    .expect("stage7 planning test invariant")
}

fn proposal(seed: &str, opportunities: &SchedulingOpportunitySetV1) -> PlanningProposalV1 {
    PlanningProposalV1::new(
        PlanningProposalIdV1::derive(seed).expect("stage7 planning test invariant"),
        "repo:stage7".into(),
        "generation:7".into(),
        opportunities.frontier_hash,
        vec!["work-root:one".into()],
        opportunities.semantic_hash,
        vec![ProposalAdviceUnitV1 {
            semantic_claim_hash: [6; 32],
            covered_opportunity_refs: vec!["action:a".into()],
            rationale_ref: "rationale:one".into(),
        }],
        vec!["assumption:one".into()],
        vec!["observation:one".into()],
        "producer:one".into(),
        "acquisition:one".into(),
        "privacy:one".into(),
        "redaction:one".into(),
        10,
        100,
    )
    .expect("stage7 planning test invariant")
}

fn evaluation(
    opportunities: SchedulingOpportunitySetV1,
    policy_binding: SchedulingPolicyBindingV1,
    safety_floor: SchedulingSafetyFloorV1,
    active_harm: ActiveHarmDispositionV1,
    applicable_proposals: Vec<PlanningProposalV1>,
) -> SchedulingEvaluationInputV1 {
    let opportunity_facts = vec![
        OpportunityFactsV1 {
            opportunity: opportunities.opportunities[0].clone(),
            facts_hash: [12; 32],
            feasible: true,
            containment: true,
            hard_deadline_safe: true,
            foundation: true,
            foundation_total_time: Some(5),
            fairness_deferral: 10,
            feasible_load: true,
            switching_cost: 0,
            currently_active_or_uncertain: false,
        },
        OpportunityFactsV1 {
            opportunity: opportunities.opportunities[1].clone(),
            facts_hash: [13; 32],
            feasible: true,
            containment: false,
            hard_deadline_safe: true,
            foundation: false,
            foundation_total_time: None,
            fairness_deferral: 1,
            feasible_load: true,
            switching_cost: 50,
            currently_active_or_uncertain: false,
        },
    ];
    let observation_closure_member_hashes = vec![[14; 32]];
    let key = SchedulingAssessmentInputKeyV1::new(
        "repo:stage7".into(),
        "generation:7".into(),
        "projection:all".into(),
        opportunities.frontier_hash,
        opportunities.semantic_hash,
        policy_binding.semantic_hash,
        policy_binding.policy.semantic_hash,
        [7; 32],
        safety_floor.classifier_hash,
        safety_floor.semantic_hash,
        proposal_closure_hash(&applicable_proposals).expect("stage7 planning test invariant"),
        owner_fact_closure_hash(&opportunity_facts).expect("stage7 planning test invariant"),
        observation_closure_hash(&observation_closure_member_hashes)
            .expect("stage7 planning test invariant"),
        20,
        30,
        [11; 32],
    )
    .expect("stage7 planning test invariant");
    SchedulingEvaluationInputV1 {
        key,
        opportunity_facts,
        opportunity_set: opportunities,
        policy_binding,
        safety_floor,
        active_harm,
        applicable_proposals,
        observation_closure_member_hashes,
        complete_owner_fact_closure: true,
        complete_proposal_closure: true,
    }
}

#[test]
fn exact_four_planning_actions_apply_and_assessment_replay_deduplicates() {
    let (actor_principal, actor_session) = actor();
    let opportunities = opportunity_set();
    let proposal = proposal("proposal:one", &opportunities);
    let safety_floor = floor();
    let policy_binding = binding(policy("initial", 10, 20, 5, 100), None, &safety_floor);
    let assessment_input = evaluation(
        opportunities.clone(),
        policy_binding.clone(),
        safety_floor.clone(),
        ActiveHarmDispositionV1::ConfirmedAbsent,
        Vec::new(),
    );
    let mut state = PlanningStateV1::default();
    let actions = vec![
        apply_planning_mutation(
            &mut state,
            PlanningMutationV1::PublishPlanningProposal {
                actor_principal,
                actor_session,
                proposal: proposal.clone(),
            },
        )
        .expect("stage7 planning test invariant")
        .action_literal(),
        apply_planning_mutation(
            &mut state,
            PlanningMutationV1::DisposePlanningProposal {
                actor_principal,
                actor_session,
                disposition: PlanningProposalDispositionV1::new(
                    proposal.proposal_id,
                    proposal.semantic_hash,
                    PlanningProposalDispositionKindV1::Retracted,
                    21,
                    "reason:retracted".into(),
                )
                .expect("stage7 planning test invariant"),
            },
        )
        .expect("stage7 planning test invariant")
        .action_literal(),
        apply_planning_mutation(
            &mut state,
            PlanningMutationV1::PublishSchedulingPolicyBinding {
                actor_principal,
                actor_session,
                binding: policy_binding,
                safety_floor,
            },
        )
        .expect("stage7 planning test invariant")
        .action_literal(),
        apply_planning_mutation(
            &mut state,
            PlanningMutationV1::PublishSchedulingAssessment {
                actor_principal,
                actor_session,
                evaluation: Box::new(assessment_input.clone()),
            },
        )
        .expect("stage7 planning test invariant")
        .action_literal(),
    ];
    let replay = apply_planning_mutation(
        &mut state,
        PlanningMutationV1::PublishSchedulingAssessment {
            actor_principal,
            actor_session,
            evaluation: Box::new(assessment_input),
        },
    )
    .expect("stage7 planning test invariant");
    assert_eq!(
        replay.disposition(),
        PlanningTransitionDispositionV1::Deduplicated
    );
    assert!(replay.records().is_empty());
    assert_eq!(
        actions,
        [
            "PublishPlanningProposal",
            "DisposePlanningProposal",
            "PublishSchedulingPolicyBinding",
            "PublishSchedulingAssessment",
        ]
    );
}

#[test]
fn assessment_is_pure_explainable_and_never_selects_a_winner() {
    let opportunities = opportunity_set();
    let safety_floor = floor();
    let policy_binding = binding(policy("ranking", 10, 20, 5, 100), None, &safety_floor);
    let applicable_proposal = proposal("proposal:assessment-advice", &opportunities);
    let input = evaluation(
        opportunities,
        policy_binding,
        safety_floor,
        ActiveHarmDispositionV1::ConfirmedUncontained,
        vec![applicable_proposal],
    );
    let first = evaluate_scheduling(&input).expect("stage7 planning test invariant");
    let second = evaluate_scheduling(&input).expect("stage7 planning test invariant");
    assert_eq!(first, second);
    let SchedulingAssessmentResultV1::OrderedEquivalenceClasses(classes) = first.result else {
        panic!("expected ordered equivalence classes");
    };
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].ordered_opportunity_refs, ["action:a"]);
    assert!(
        classes[0]
            .reasons
            .contains(&SchedulingReasonV1::ActiveHarmContainment)
    );
    assert!(
        classes[0]
            .reasons
            .contains(&SchedulingReasonV1::ProposalAdvice)
    );
}

#[test]
fn policy_binding_keeps_downgrade_authority_outside_its_identity() {
    let safety_floor = floor();
    let initial = binding(policy("strong", 10, 20, 5, 10), None, &safety_floor);
    let weaker_policy = policy("weak", 20, 30, 10, 20);
    let diff = classify_policy_diff(Some(&initial.policy), &weaker_policy, &safety_floor)
        .expect("stage7 planning test invariant");
    assert_eq!(diff.kind, SemanticPolicyDiffKindV1::Weakening);
    let downgraded = SchedulingPolicyBindingV1::new(
        "repo:stage7".into(),
        "generation:7".into(),
        2,
        Some(initial.semantic_hash),
        weaker_policy,
        diff,
    )
    .expect("stage7 planning test invariant");
    assert_eq!(downgraded.diff.kind, SemanticPolicyDiffKindV1::Weakening);
}

#[test]
fn safety_floor_enforces_its_numeric_ceilings_and_reports_real_strength() {
    let safety_floor = floor();
    assert_eq!(safety_floor.strength(), [u64::MAX - 1000; 4]);
    assert!(
        safety_floor
            .admits(&policy("at-floor", 1000, 1000, 1000, 1000))
            .is_ok()
    );
    for breach in [
        policy("over-foundation", 1001, 20, 5, 100),
        policy("over-fairness", 10, 1001, 5, 100),
        policy("over-hysteresis", 10, 20, 1001, 100),
        policy("over-overload", 10, 20, 5, 1001),
    ] {
        assert!(matches!(
            safety_floor.admits(&breach),
            Err(PlanningErrorV1::InvalidPolicy)
        ));
        assert!(classify_policy_diff(None, &breach, &safety_floor).is_err());
    }
}

#[test]
fn scheduling_owner_revisions_are_nonzero_and_part_of_the_typed_facts() {
    let revision_one = SchedulingPolicySnapshotV1::new(
        "policy:revision".into(),
        "evaluator:revision".into(),
        1,
        "core:evaluator:v1".into(),
        10,
        20,
        5,
        100,
    )
    .expect("stage7 planning test invariant");
    let revision_two = SchedulingPolicySnapshotV1::new(
        "policy:revision".into(),
        "evaluator:revision".into(),
        2,
        "core:evaluator:v1".into(),
        10,
        20,
        5,
        100,
    )
    .expect("stage7 planning test invariant");
    assert_ne!(revision_one.semantic_hash, revision_two.semantic_hash);
    assert!(matches!(
        SchedulingPolicySnapshotV1::new(
            "policy:revision".into(),
            "evaluator:revision".into(),
            0,
            "core:evaluator:v1".into(),
            10,
            20,
            5,
            100,
        ),
        Err(PlanningErrorV1::InvalidPolicy)
    ));
    assert!(matches!(
        SchedulingSafetyFloorV1::new(
            "safety-floor:v1".into(),
            "core:evaluator:v1".into(),
            [5; 32],
            0,
            1000,
            1000,
            1000,
            1000,
        ),
        Err(PlanningErrorV1::InvalidPolicy)
    ));
    assert!(matches!(
        SchedulingSafetyFloorV1::new(
            "safety-floor:v1".into(),
            "core:evaluator:v1".into(),
            [5; 32],
            1,
            1000,
            u64::MAX,
            1000,
            1000,
        ),
        Err(PlanningErrorV1::InvalidPolicy)
    ));
}

#[test]
fn scheduling_publication_uses_live_authority_facts_in_one_store_transaction() {
    let safety_floor = floor();
    let policy_binding = binding(policy("atomic", 10, 20, 5, 100), None, &safety_floor);
    let placeholder_actor = actor();
    let placeholder = scheduling_policy_transition(
        placeholder_actor.0,
        placeholder_actor.1,
        policy_binding.clone(),
        safety_floor.clone(),
    );
    let action = RepositoryDownstreamActionLeafV1::from_global_tag(105)
        .expect("stage7 planning test invariant");
    let mut fixture = repository_owner_family_authority_fixture(
        vec![(action.literal(), placeholder.subject_commitment())],
        AuthorityFixtureModeV1::Valid,
    );
    let selection = fixture.selection;
    let actor_principal = fixture.actor_principal;
    let actor_session = selection.actor_session_id();
    let authority_root_id = fixture.authority_root_id;

    let root = test_root();
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"stage7-scheduling")
        .expect("stage7 planning test invariant");
    let governance_floor = repository_governance_floor_genesis_object(&domain, &fixture);
    fixture.objects.push(governance_floor.clone());
    let mut store = StoreV1::create(&root, domain.clone()).expect("stage7 planning test invariant");
    put_objects_in_reference_order(&mut store, fixture.objects);
    let mut roots = vec![authority_root_id, governance_floor.id()];
    roots.sort_unstable();
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        ContractRootIdV1::from_digest(digest("stage7-scheduling-contract")),
        StoreCompatibilityV1::stage0_successor().expect("stage7 planning test invariant"),
        roots,
    )
    .expect("stage7 planning test invariant");
    store
        .publish_generation(&generation, None)
        .expect("stage7 planning test invariant");
    let connection =
        Connection::open(root.join("store.sqlite3")).expect("stage7 planning test invariant");
    assert_eq!(
        connection
            .execute(
                "UPDATE store_state SET state = 'active', state_revision = state_revision + 1
                 WHERE singleton = 1",
                [],
            )
            .expect("stage7 planning test invariant"),
        1
    );

    let schema = RepositoryStoreSchemaV1::WorkRecord
        .schema_id()
        .expect("stage7 planning test invariant");
    let request_object = StoreObjectV1::new(
        schema,
        CborValue::text("stage7-scheduling-request").expect("stage7 planning test invariant"),
        vec![],
    )
    .expect("stage7 planning test invariant");
    let binding_object = StoreObjectV1::new(schema, policy_binding.canonical_value(), vec![])
        .expect("stage7 planning test invariant");
    let probe = StoreIdempotencyProbeV1::new(
        "maestro.test.stage7-scheduling.v1",
        digest("stage7-scheduling-key"),
        digest("stage7-scheduling-meaning"),
    )
    .expect("stage7 planning test invariant");

    let unchanged_head = store
        .active_head()
        .expect("stage7 planning test invariant")
        .expect("stage7 planning test invariant");
    let forged_binding_object = StoreObjectV1::new(
        schema,
        CborValue::text("forged-stage7-scheduling-binding")
            .expect("stage7 planning test invariant"),
        vec![],
    )
    .expect("stage7 planning test invariant");
    let forged = publish_scheduling_policy_binding(
        &mut store,
        &probe,
        SchedulingPolicyPublicationInputV1 {
            request_id: ActionRequestIdV1::derive("stage7-scheduling-request")
                .expect("stage7 planning test invariant"),
            authority_selection: selection,
            transition: scheduling_policy_transition(
                actor_principal,
                actor_session,
                policy_binding.clone(),
                safety_floor.clone(),
            ),
            request_object: request_object.clone(),
            binding_object: forged_binding_object,
            current_binding: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        forged,
        SchedulingPolicyPublicationErrorV1::BindingObjectMismatch
    ));
    assert_eq!(
        store
            .active_head()
            .expect("stage7 planning test invariant")
            .expect("stage7 planning test invariant"),
        unchanged_head
    );

    let committed = publish_scheduling_policy_binding(
        &mut store,
        &probe,
        SchedulingPolicyPublicationInputV1 {
            request_id: ActionRequestIdV1::derive("stage7-scheduling-request")
                .expect("stage7 planning test invariant"),
            authority_selection: selection,
            transition: scheduling_policy_transition(
                actor_principal,
                actor_session,
                policy_binding,
                safety_floor.clone(),
            ),
            request_object: request_object.clone(),
            binding_object: binding_object.clone(),
            current_binding: None,
        },
    )
    .expect("stage7 planning test invariant");
    assert!(matches!(
        committed,
        StorePublicationOutcomeV1::Committed { .. }
    ));
    assert_ne!(committed.head().id(), unchanged_head.id());
    assert_eq!(
        store
            .read_object(binding_object.id())
            .expect("stage7 planning test invariant"),
        binding_object
    );
    assert!(
        committed
            .result()
            .references()
            .contains(&request_object.id())
    );
    assert!(
        committed
            .result()
            .references()
            .contains(&binding_object.id())
    );
    assert!(
        store
            .publication_generation(committed.head().id())
            .expect("stage7 planning test invariant")
            .roots()
            .contains(&binding_object.id())
    );

    drop(connection);
    drop(store);
    fs::remove_dir_all(root).expect("stage7 planning test invariant");
}

#[test]
fn stale_proposal_disposition_and_policy_binding_races_are_refused() {
    let (actor_principal, actor_session) = actor();
    let opportunities = opportunity_set();
    let proposal = proposal("proposal:race", &opportunities);
    let (mut state, _) = test_adapter::apply(
        &PlanningStateV1::default(),
        PlanningMutationV1::PublishPlanningProposal {
            actor_principal,
            actor_session,
            proposal: proposal.clone(),
        },
    )
    .expect("stage7 planning test invariant");
    let disposition = PlanningProposalDispositionV1::new(
        proposal.proposal_id,
        proposal.semantic_hash,
        PlanningProposalDispositionKindV1::Retracted,
        21,
        "reason:race".into(),
    )
    .expect("stage7 planning test invariant");
    apply_planning_mutation(
        &mut state,
        PlanningMutationV1::DisposePlanningProposal {
            actor_principal,
            actor_session,
            disposition: disposition.clone(),
        },
    )
    .expect("stage7 planning test invariant");
    assert_eq!(
        apply_planning_mutation(
            &mut state,
            PlanningMutationV1::DisposePlanningProposal {
                actor_principal,
                actor_session,
                disposition,
            },
        )
        .unwrap_err(),
        PlanningStateErrorV1::ProposalAlreadyDisposed
    );
}
