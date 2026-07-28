use std::cell::Cell;
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use super::*;
use crate::domain::authority::{
    AuthorityRevocationSetV1, AuthoritySnapshotV1, AuthorityUseConstraintV1,
    BootstrapContinuityTransitionProofV1, BootstrapG0PathV1, BootstrapInteractionSubjectV1,
    BootstrapMandateInteractionObservationJoinV1, BootstrapMandatePresentationObservationV1,
    BootstrapMandateResponseObservationV1, BootstrapMandateTargetV1,
    BootstrapResponseDispositionV1, CapacityRootIdV1, ConsentSlotEvaluationFactsV1, DelegationIdV1,
    DelegationV1, GenesisGrantIdV1, GovernedCapacityKindV1, GovernedCapacityRootV1,
    GrantActionIdentityV1, GrantAdministrationAuthorityV1, GrantDefinitionV1, GrantIdV1,
    GrantScopeV1, HalfOpenValidityV1, IdempotencyKeyIdV1, IssueBootstrapMandateInputV1,
    IssueBootstrapMandateRequestV1, IssueRootAttachedBoundedGrantPublicationV1,
    OrdinaryBoundedGrantV1, OrdinaryGrantDelegationV1, PrincipalBindingIdV1, PrincipalBindingV1,
    PrincipalIdV1, ReissueRootAttachedGrantOneToOnePublicationV1, RepositoryDownstreamActionLeafV1,
    RepositoryGovernedCapacitySlotKindV1, RevocationSetV1, RevocationTargetV1,
    RevokeGrantPublicationV1, ScopeAtomV1, SessionIdV1, SessionV1, TargetActionEffectKindV1,
    TargetActionOwnerV1, TargetActionProjectionV1, TargetActionProtocolV1, TargetExpectedHeadsV1,
    TransitionGuardKindV1, TrustedTimeV1,
};
use crate::domain::identity::ContractRootIdV1;
use crate::domain::integration::{
    TrustedHostDiagnosticTestClaimsV1, TrustedHostDiagnosticTestConnectionV1,
    TrustedHostDiagnosticTestOperatorIdentityV1,
};
use crate::domain::persistence::{
    ProtectedDiagnosticTestAnchorMutationV1, ProtectedDiagnosticTestCurrentViewProviderV1,
};
use crate::domain::persistence::{StoreDomainV1, StoreRoleV1};
use crate::domain::planning::{
    SchedulingPolicyBindingV1, SchedulingPolicySnapshotV1, SchedulingSafetyFloorV1,
    SemanticPolicyDiffKindV1, SemanticPolicyDiffV1,
};
use crate::domain::repository::RepositoryStoreSchemaV1;

fn reference(seed: &str) -> ContinuityReferenceV1 {
    ContinuityReferenceV1::derive(seed).unwrap()
}

fn digest(seed: &str) -> [u8; 32] {
    Sha256::digest(seed.as_bytes()).into()
}

fn repository_governance_floor_for_fixture(
    domain: &StoreDomainV1,
    fixture: &super::test_support::RepositoryAuthorityFixtureV1,
) -> StoreObjectV1 {
    use super::super::governance_floor::{
        RepositoryGovernanceFloorGenesisInputV1, RepositoryGovernanceFloorSnapshotV1,
    };

    let context = AuthorityContextIdV1::derive("stage3-repository-context").unwrap();
    let trust_root_binding_commitment = materialization_commitment(
        b"maestro.authority.repository-governance-trust-root-binding.v1\0",
        &[
            context.as_bytes(),
            fixture.selection.actor_binding_id().as_bytes(),
            fixture.selection.actor_session_id().as_bytes(),
            &11_u64.to_be_bytes(),
        ],
    );
    RepositoryGovernanceFloorSnapshotV1::repository_genesis(
        RepositoryGovernanceFloorGenesisInputV1 {
            repository_store_domain: *domain.id().as_bytes(),
            authority_context: *context.as_bytes(),
            activation_generation_ordinal: 1,
            authority_epoch: fixture.authority_epoch,
            trust_root_revision: 11,
            trust_root_binding_commitment,
            authority_transition_protocol_identity: digest(
                "repository-governance-transition-protocol",
            ),
            minimum_assurance: 1,
            requirement_grammar_identity: digest("repository-governance-requirement-grammar"),
            requirement_evaluator_identity: digest("repository-governance-requirement-evaluator"),
        },
    )
    .unwrap()
    .to_store_object()
    .unwrap()
}

fn scheduling_policy(maximum: u64, label: &str) -> SchedulingPolicySnapshotV1 {
    SchedulingPolicySnapshotV1::new(
        format!("policy:{label}"),
        "evaluator:scheduling-v1".to_owned(),
        1,
        "core:scheduling-v1".to_owned(),
        maximum,
        maximum,
        maximum,
        maximum,
    )
    .unwrap()
}

fn scheduling_safety_floor(maximum: u64) -> SchedulingSafetyFloorV1 {
    SchedulingSafetyFloorV1::new(
        "floor:scheduling-v1".to_owned(),
        "core:scheduling-v1".to_owned(),
        digest("classifier:scheduling-v1"),
        1,
        maximum,
        maximum,
        maximum,
        maximum,
    )
    .unwrap()
}

fn scheduling_binding(
    revision: u64,
    expected_old: Option<&SchedulingPolicyBindingV1>,
    policy: SchedulingPolicySnapshotV1,
    kind: SemanticPolicyDiffKindV1,
) -> SchedulingPolicyBindingV1 {
    SchedulingPolicyBindingV1::new(
        "repository:test".to_owned(),
        format!("generation:{revision}"),
        revision,
        expected_old.map(|binding| binding.semantic_hash),
        policy.clone(),
        SemanticPolicyDiffV1 {
            old_policy_hash: expected_old.map(|binding| binding.policy.semantic_hash),
            candidate_policy_hash: policy.semantic_hash,
            classifier_hash: digest("authority-scheduling-classifier"),
            kind,
        },
    )
    .unwrap()
}

fn test_root() -> std::path::PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "maestro-vnext-authority-facade-{}-{nonce}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&path).unwrap();
    fs::canonicalize(path).unwrap()
}

fn continuity_generation(
    manifest: &AuthorityContinuityManifestV1,
    context_id: AuthorityContextIdV1,
    generation: u64,
    prior: Option<(
        &AuthorityContinuityClosureV1,
        &SuccessVisibleAuthorityContinuityStateV1,
    )>,
) -> (
    AuthorityContinuityClosureV1,
    AdmittedTransitionGuardV1,
    SuccessVisibleAuthorityContinuityStateV1,
) {
    let accepted_time = match prior {
        None => AcceptedAuthorityTimeFloorV1::context_genesis(
            reference("stable-lineage"),
            reference("trusted-time-coordinate"),
            reference("trusted-time-stack"),
            reference("trusted-time-origin"),
            120,
            130,
        )
        .unwrap(),
        Some((_, state)) => AcceptedAuthorityTimeFloorV1::continue_from(
            state.accepted_time(),
            state.accepted_time().stable_lineage(),
            state.accepted_time().coordinate(),
            state.accepted_time().policy_stack(),
            HTimeCarryBasisV1::ExactNoLineageChange,
            HTimeContinuationContributionV1::CarryOnly,
        )
        .unwrap(),
    };
    let allocation = StoreAllocatedContinuityStateTokenV1::from_store_commitments(
        context_id,
        generation,
        prior.map(|(_, state)| state.state_token()),
        generation,
        digest(&format!("state-token-{generation}")),
        digest(&format!("allocation-{generation}")),
    )
    .unwrap();
    let predecessor = prior.map_or(
        AuthorityContinuityPredecessorV1::ContextGenesis {
            origin_commitment: reference("context-genesis-origin"),
        },
        |(closure, state)| AuthorityContinuityPredecessorV1::PriorClosure {
            closure_id: closure.id(),
            state_token: state.state_token(),
        },
    );
    let semantic_cut = AuthorityContinuitySemanticCutV1 {
        cut_sequence: generation,
        source_store_generation: generation - 1,
        successor_store_generation: generation,
        authority_epoch: 7,
        stable_lineage: reference("stable-lineage"),
        selected_trusted_time_stack: reference("trusted-time-stack"),
        carrier_profile: ContinuityCarrierProfileStatusV1::Confirmed {
            profile: reference("carrier-profile"),
            accepted_prefix: reference("accepted-prefix"),
            handoff_state: reference("handoff-state"),
            fence: reference("carrier-fence"),
            currentness: reference("carrier-currentness"),
        },
        accepted_time,
        lane_state_closure_root: reference("lane-state-root"),
        source_floor_root: reference("source-floor-root"),
        gap_companions: vec![],
        floor_provenance: vec![],
        external_revision_cells: vec![],
        cma_remaining_root: reference("cma-remaining"),
        cma_spent_root: reference("cma-spent"),
        canonical_records: vec![reference(&format!("canonical-record-{generation}"))],
        graph_nodes: vec![],
        replay_items: vec![],
        historical_spend_items: vec![],
        unresolved_effects: vec![],
    };
    let class_entries = continuity_class_entries(manifest, &semantic_cut, &[]).unwrap();
    let closure = AuthorityContinuityClosureV1::prove(
        manifest,
        AuthorityContinuityClosureInputV1 {
            manifest_id: manifest.id(),
            context_kind: manifest.context_kind(),
            context_id,
            predecessor,
            semantic_cut,
            class_entries,
            graph_edges: vec![],
            protocol_version: 1,
        },
        &allocation,
    )
    .unwrap();
    let (kind, term_facts, census) = match prior {
        None => {
            let census = TransitionGuardOwnerCensusV1::externally_rooted_genesis(
                context_id,
                generation,
                7,
                reference("context-genesis-origin"),
            )
            .unwrap();
            (
                GuardAdmissionKindV1::ExternallyRootedContextGenesis,
                Vec::new(),
                census,
            )
        }
        Some(_) => {
            let transition = TransitionGuardKindV1::RepositoryFloorOrTrustRootRotation;
            let facts = transition
                .term_bundle()
                .terms()
                .iter()
                .copied()
                .map(|term| {
                    TransitionGuardTermFactV1::owner_confirmed(
                        term,
                        reference(&format!("owner-fact-{generation}-{}", term as u8)),
                        reference(&format!("owner-revision-{generation}-{}", term as u8)),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let census = TransitionGuardOwnerCensusV1::from_owner_sources(
                transition,
                context_id,
                generation,
                7,
                reference(&format!("owner-cut-{generation}")),
                facts.clone(),
            )
            .unwrap();
            (GuardAdmissionKindV1::Established(transition), facts, census)
        }
    };
    let guard = AdmittedTransitionGuardV1::evaluate(AuthorityTransitionGuardAdmissionInputV1 {
        kind,
        context_kind: manifest.context_kind(),
        context_id,
        store_generation: generation,
        authority_epoch: 7,
        manifest_id: manifest.id(),
        closure_id: closure.id(),
        predecessor_state_token: prior.map(|(_, state)| state.state_token()),
        cut_sequence: generation,
        selected_trusted_time_stack: closure.selected_trusted_time_stack(),
        carrier_profile: closure.carrier_profile().clone(),
        accepted_time: closure.accepted_time().clone(),
        lane_state_closure_root: closure.lane_state_closure_root(),
        source_floor_root: closure.source_floor_root(),
        gap_companions: vec![],
        floor_provenance: vec![],
        external_revision_cells: vec![],
        cma_remaining_root: closure.cma_remaining_root(),
        cma_spent_root: closure.cma_spent_root(),
        unresolved_effects: vec![],
        term_facts,
        owner_census: census,
        disclosure: ContinuityDisclosureV1::ProtectedComplete,
        protocol_version: 1,
    })
    .unwrap();
    let state = SuccessVisibleAuthorityContinuityStateV1::construct(
        manifest,
        &closure,
        &guard,
        prior.map(|(_, state)| state),
    )
    .unwrap();
    (closure, guard, state)
}

fn authority_fixture(
    state: &SuccessVisibleAuthorityContinuityStateV1,
    manifest: &AuthorityContinuityManifestV1,
) -> (BootstrapAuthoritySnapshotV1, IssueBootstrapMandateRequestV1) {
    let context_id = AuthorityContextIdV1::derive("repository-context").unwrap();
    let actor_principal = PrincipalIdV1::derive("actor-principal").unwrap();
    let actor_binding_id = PrincipalBindingIdV1::derive("actor-binding").unwrap();
    let responder_binding_id = PrincipalBindingIdV1::derive("responder-binding").unwrap();
    let validity = HalfOpenValidityV1::new(100, 200).unwrap();
    let context = super::super::AuthorityContextV1::repository(
        context_id,
        "repository-installation",
        2,
        7,
        11,
    )
    .unwrap();
    let target_head = StateTokenIdV1::derive("target-head").unwrap();
    let target = TargetActionProjectionV1::new(
        BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
        "recovery-selection",
        9,
        TargetActionOwnerV1::Authority,
        TargetActionProtocolV1::RecoveryCommitmentSelection,
        TargetActionEffectKindV1::Rotate,
        "sha256:effect-closure",
        TargetExpectedHeadsV1::new(context_id, 2, 7, 11, 9, target_head).unwrap(),
        validity,
    )
    .unwrap();
    let target_commitment = target.target_action_commitment().unwrap();
    let consent = ConsentSlotEvaluationFactsV1::derive_for_target(&target, validity).unwrap();
    let actor_binding = PrincipalBindingV1::new(
        actor_binding_id,
        actor_principal,
        context_id,
        11,
        4,
        validity,
        false,
    )
    .unwrap();
    let responder_binding = PrincipalBindingV1::new(
        responder_binding_id,
        PrincipalIdV1::derive("responder-principal").unwrap(),
        context_id,
        11,
        9,
        validity,
        true,
    )
    .unwrap();
    let request_commitment = target_commitment.render();
    let actor_session = SessionV1::new(
        SessionIdV1::derive("actor-session").unwrap(),
        actor_binding_id,
        context_id,
        2,
        7,
        &request_commitment,
        validity,
    )
    .unwrap();
    let responder_session = SessionV1::new(
        SessionIdV1::derive("responder-session").unwrap(),
        responder_binding_id,
        context_id,
        2,
        7,
        &request_commitment,
        validity,
    )
    .unwrap();
    let subject = BootstrapInteractionSubjectV1::new(
        context_id,
        StateTokenIdV1::derive("interaction-plan").unwrap(),
        ActionRequestIdV1::derive("interaction-attempt").unwrap(),
        responder_binding_id,
        9,
        target_commitment,
        consent.binding().clone(),
        StateTokenIdV1::derive("option-map").unwrap(),
        StateTokenIdV1::derive("affirmative-option").unwrap(),
    );
    let carrier = StateTokenIdV1::derive("interaction-carrier").unwrap();
    let procedure = StateTokenIdV1::derive("interaction-procedure").unwrap();
    let presentation =
        BootstrapMandatePresentationObservationV1::new(subject.clone(), carrier, procedure)
            .unwrap();
    let response = BootstrapMandateResponseObservationV1::new(
        subject,
        presentation.id(),
        BootstrapResponseDispositionV1::Affirmative,
        StateTokenIdV1::derive("affirmative-option").unwrap(),
    )
    .unwrap();
    let join = BootstrapMandateInteractionObservationJoinV1::new(
        &presentation,
        &response,
        responder_session.id(),
        procedure,
    )
    .unwrap();
    let request = IssueBootstrapMandateRequestV1::try_from(IssueBootstrapMandateInputV1 {
        request_id: ActionRequestIdV1::derive("request").unwrap(),
        idempotency_key: IdempotencyKeyIdV1::derive("idempotency-key").unwrap(),
        context_id,
        actor_binding_id,
        actor_session_id: actor_session.id(),
        responder_binding_id,
        presentation_observation_id: presentation.id(),
        response_observation_id: response.id(),
        target: BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
        target_subject: "recovery-selection".to_owned(),
        target_revision: 9,
        consent_slot: consent.binding().clone(),
        supplied_mandates: vec![],
    })
    .unwrap();
    let scope = ScopeAtomV1::new(
        "IssueBootstrapMandate",
        &request_commitment,
        AuthorityEvaluatorV1::ISSUE_BOOTSTRAP_MANDATE_PROTOCOL_REVISION,
    )
    .unwrap();
    let capacity_root_id = CapacityRootIdV1::derive("repository-admin-capacity").unwrap();
    let issue_grant_scope = ScopeAtomV1::new(
        "IssueRootAttachedBoundedGrant",
        &capacity_root_id.render(),
        9,
    )
    .unwrap();
    let reissue_scope = ScopeAtomV1::new(
        "ReissueRootAttachedGrantOneToOne",
        &capacity_root_id.render(),
        9,
    )
    .unwrap();
    let revoke_scope = ScopeAtomV1::new("RevokeGrant", &capacity_root_id.render(), 9).unwrap();
    let grant = GrantDefinitionV1 {
        id: GrantIdV1::derive("genesis-grant").unwrap(),
        context_id,
        grantee_principal_id: actor_principal,
        parent_grant_id: None,
        delegation_id: None,
        terminal_scope: GrantScopeV1::new(vec![scope, issue_grant_scope]).unwrap(),
        delegable_scope: GrantScopeV1::new(vec![reissue_scope, revoke_scope]).unwrap(),
        validity,
        delegation_depth_remaining: 8,
        authority_use_constraint: AuthorityUseConstraintV1::NoLocalBoundedRoot,
    }
    .validate()
    .unwrap();
    let path = BootstrapG0PathV1::new(
        GenesisGrantIdV1::derive(&grant.id().render()).unwrap(),
        grant,
        2,
        7,
        11,
        true,
        Vec::<CapacityRootIdV1>::new(),
    )
    .unwrap();
    let facts = BootstrapAuthoritySnapshotV1::new(
        context,
        AuthoritySnapshotV1::new(
            context_id,
            2,
            7,
            11,
            9,
            TrustedTimeV1::verified(120, 130).unwrap(),
        ),
        actor_binding,
        actor_session,
        responder_binding,
        responder_session,
        vec![path],
        AuthorityRevocationSetV1::new(context_id, RevocationSetV1::empty()),
        Some(join),
        procedure,
        target,
        target_head,
        consent,
        BootstrapContinuityTransitionProofV1::new(
            context_id,
            2,
            7,
            11,
            manifest.id(),
            state.guard_kind(),
            state.state_token(),
            validity,
        ),
    )
    .unwrap();
    (facts, request)
}

fn seeded_store_with_activation(
    active: bool,
) -> (
    std::path::PathBuf,
    StoreDomainV1,
    StoreV1,
    ContractRootIdV1,
    crate::domain::persistence::StoreHeadV1,
    StoreObjectIdV1,
    IssueBootstrapMandateRequestV1,
    ProtectedDiagnosticInvocationFactsV1,
    Vec<u8>,
) {
    seeded_store_with_diagnostic_mode(active, ProtectedDiagnosticFixtureModeV1::Valid)
}

#[derive(Clone, Copy, Debug)]
enum ProtectedDiagnosticFixtureModeV1 {
    Valid,
    HeterogeneousRoot,
    MissingAuthorityRoot,
    DuplicateAuthorityRoot,
    DuplicateOperatorMapping,
    ChangedRequestCommitment,
    ForeignAuthorityRoot,
    StaleAuthorityRoot,
    NonHuman,
    RevokedBinding,
    RevokedSession,
    RevokedTrustRoot,
    WrongBindingContext,
    WrongSessionContext,
    WrongSessionBinding,
    WrongTrustRoot,
    StaleSession,
    WrongSessionEpoch,
    UnavailableTime,
    InvertedTime,
    ExpiredBinding,
    PrematureBinding,
}

struct ProtectedDiagnosticInvocationFactsV1 {
    protected_subject: ContinuityReferenceV1,
}

impl ProtectedDiagnosticInvocationFactsV1 {
    fn test_ports(
        &self,
        root: &std::path::Path,
        domain: &StoreDomainV1,
        store: &StoreV1,
        head: &crate::domain::persistence::StoreHeadV1,
        seed: &[u8],
    ) -> (
        ProtectedDiagnosticTestCurrentViewProviderV1,
        TrustedHostDiagnosticTestConnectionV1,
    ) {
        let generation = store.publication_generation(head.id()).unwrap();
        let state_revision = store.state().unwrap().1;
        let publication_clock = store.publication_clock_for_test().unwrap();
        let provider = ProtectedDiagnosticTestCurrentViewProviderV1::bound_to(
            root,
            domain,
            head,
            &generation,
            state_revision,
            publication_clock,
            seed,
        )
        .unwrap();
        let operator_identity = TrustedHostDiagnosticTestOperatorIdentityV1::authenticated_human(
            *PrincipalIdV1::derive("responder-principal")
                .unwrap()
                .as_bytes(),
            *PrincipalBindingIdV1::derive("responder-binding")
                .unwrap()
                .as_bytes(),
            *SessionIdV1::derive("responder-session").unwrap().as_bytes(),
            *AuthorityContextIdV1::derive("repository-context")
                .unwrap()
                .as_bytes(),
            11,
            9,
            100,
            200,
            100,
            200,
            2,
            7,
            *domain.id().as_bytes(),
            domain.role().tag(),
        );
        let claims = TrustedHostDiagnosticTestClaimsV1::from_seed(seed, operator_identity).unwrap();
        (
            provider,
            TrustedHostDiagnosticTestConnectionV1::authenticated(claims),
        )
    }
}

fn diagnostic_facts(
    facts: BootstrapAuthoritySnapshotV1,
    mode: ProtectedDiagnosticFixtureModeV1,
) -> BootstrapAuthoritySnapshotV1 {
    if matches!(
        mode,
        ProtectedDiagnosticFixtureModeV1::Valid
            | ProtectedDiagnosticFixtureModeV1::HeterogeneousRoot
            | ProtectedDiagnosticFixtureModeV1::MissingAuthorityRoot
            | ProtectedDiagnosticFixtureModeV1::DuplicateAuthorityRoot
    ) {
        return facts;
    }
    let mut snapshot = *facts.snapshot();
    if matches!(mode, ProtectedDiagnosticFixtureModeV1::StaleAuthorityRoot) {
        snapshot.store_generation -= 1;
    } else if matches!(mode, ProtectedDiagnosticFixtureModeV1::UnavailableTime) {
        snapshot.trusted_time = TrustedTimeV1::Unavailable;
    } else if matches!(mode, ProtectedDiagnosticFixtureModeV1::InvertedTime) {
        snapshot.trusted_time = TrustedTimeV1::Verified {
            lower_bound: 200,
            upper_bound: 100,
        };
    }
    let responder_binding = PrincipalBindingV1::new(
        facts.responder_binding().id(),
        facts.responder_binding().principal_id(),
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::WrongBindingContext) {
            AuthorityContextIdV1::derive("foreign-diagnostic-binding-context").unwrap()
        } else {
            facts.responder_binding().context_id()
        },
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::WrongTrustRoot) {
            facts.responder_binding().trust_root_revision() + 1
        } else {
            facts.responder_binding().trust_root_revision()
        },
        facts.responder_binding().assurance_revision(),
        match mode {
            ProtectedDiagnosticFixtureModeV1::ExpiredBinding => {
                HalfOpenValidityV1::new(1, 100).unwrap()
            }
            ProtectedDiagnosticFixtureModeV1::PrematureBinding => {
                HalfOpenValidityV1::new(201, 300).unwrap()
            }
            _ => facts.responder_binding().validity(),
        },
        !matches!(mode, ProtectedDiagnosticFixtureModeV1::NonHuman),
    )
    .unwrap();
    let responder_session = SessionV1::new(
        facts.responder_session().id(),
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::WrongSessionBinding) {
            PrincipalBindingIdV1::derive("foreign-diagnostic-session-binding").unwrap()
        } else {
            facts.responder_session().binding_id()
        },
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::WrongSessionContext) {
            AuthorityContextIdV1::derive("foreign-diagnostic-session-context").unwrap()
        } else {
            facts.responder_session().context_id()
        },
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::StaleSession) {
            facts.responder_session().store_generation() - 1
        } else {
            facts.responder_session().store_generation()
        },
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::WrongSessionEpoch) {
            facts.responder_session().authority_epoch() + 1
        } else {
            facts.responder_session().authority_epoch()
        },
        if matches!(
            mode,
            ProtectedDiagnosticFixtureModeV1::ChangedRequestCommitment
        ) {
            "diagnostic-request-commitment-is-snapshot-only"
        } else {
            facts.responder_session().request_commitment()
        },
        facts.responder_session().validity(),
    )
    .unwrap();
    let revocations = match mode {
        ProtectedDiagnosticFixtureModeV1::RevokedBinding => {
            RevocationSetV1::new(vec![RevocationTargetV1::PrincipalBinding(
                responder_binding.id(),
            )])
            .unwrap()
        }
        ProtectedDiagnosticFixtureModeV1::RevokedSession => {
            RevocationSetV1::new(vec![RevocationTargetV1::Session(responder_session.id())]).unwrap()
        }
        ProtectedDiagnosticFixtureModeV1::RevokedTrustRoot => {
            RevocationSetV1::new(vec![RevocationTargetV1::TrustRoot(
                snapshot.trust_root_revision,
            )])
            .unwrap()
        }
        _ => facts.revocations().revocations().clone(),
    };
    BootstrapAuthoritySnapshotV1::new(
        if matches!(mode, ProtectedDiagnosticFixtureModeV1::ForeignAuthorityRoot) {
            super::super::AuthorityContextV1::installation(
                facts.context().context_id(),
                "foreign-installation",
                "foreign-realm",
                facts.context().store_generation(),
                facts.context().authority_epoch(),
                facts.context().trust_root_revision(),
                1,
            )
            .unwrap()
        } else {
            facts.context().clone()
        },
        snapshot,
        if matches!(
            mode,
            ProtectedDiagnosticFixtureModeV1::DuplicateOperatorMapping
        ) {
            responder_binding.clone()
        } else {
            facts.actor_binding().clone()
        },
        if matches!(
            mode,
            ProtectedDiagnosticFixtureModeV1::DuplicateOperatorMapping
        ) {
            responder_session.clone()
        } else {
            facts.actor_session().clone()
        },
        responder_binding,
        responder_session,
        facts.g0_candidate_paths().to_vec(),
        AuthorityRevocationSetV1::new(facts.context().context_id(), revocations),
        facts.interaction_join().cloned(),
        facts.current_carrier_procedure_ref(),
        facts.target().clone(),
        facts.current_target_head(),
        facts.consent_slot().clone(),
        facts.continuity().clone(),
    )
    .unwrap()
}

fn seeded_store_with_diagnostic_mode(
    active: bool,
    diagnostic_mode: ProtectedDiagnosticFixtureModeV1,
) -> (
    std::path::PathBuf,
    StoreDomainV1,
    StoreV1,
    ContractRootIdV1,
    crate::domain::persistence::StoreHeadV1,
    StoreObjectIdV1,
    IssueBootstrapMandateRequestV1,
    ProtectedDiagnosticInvocationFactsV1,
    Vec<u8>,
) {
    let root = test_root();
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"authority-facade").unwrap();
    let mut store = StoreV1::create(&root, domain.clone()).unwrap();
    let manifest = AuthorityContinuityManifestV1::repository().unwrap();
    let context_id = AuthorityContextIdV1::derive("repository-context").unwrap();
    let (closure_one, guard_one, state_one) = continuity_generation(&manifest, context_id, 1, None);
    let manifest_object = authority_object(
        AuthoritySchemaV1::AuthorityContinuityManifest,
        manifest.schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let closure_one_object = authority_object(
        AuthoritySchemaV1::AuthorityContinuityClosure,
        closure_one.schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let guard_one_object = authority_object(
        AuthoritySchemaV1::AdmittedTransitionGuard,
        guard_one.schema_value().unwrap(),
        vec![closure_one_object.id()],
    )
    .unwrap();
    let state_one_object = authority_object(
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
        state_one.schema_value().unwrap(),
        vec![closure_one_object.id(), guard_one_object.id()],
    )
    .unwrap();
    for object in [
        &manifest_object,
        &closure_one_object,
        &guard_one_object,
        &state_one_object,
    ] {
        store.put_object(object).unwrap();
    }
    let contract_root = ContractRootIdV1::parse(&format!("sha256:{}", "61".repeat(32))).unwrap();
    let generation_one = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        contract_root,
        StoreCompatibilityV1::stage0_successor().unwrap(),
        vec![state_one_object.id()],
    )
    .unwrap();
    let head_one = store.publish_generation(&generation_one, None).unwrap();

    let (closure_two, guard_two, state_two) =
        continuity_generation(&manifest, context_id, 2, Some((&closure_one, &state_one)));
    let (facts, request) = authority_fixture(&state_two, &manifest);
    let facts = diagnostic_facts(facts, diagnostic_mode);
    let authenticated_carrier = b"fixture-only-nonpersisted-authentication-carrier".to_vec();
    let closure_two_object = authority_object(
        AuthoritySchemaV1::AuthorityContinuityClosure,
        closure_two.schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let guard_two_object = authority_object(
        AuthoritySchemaV1::AdmittedTransitionGuard,
        guard_two.schema_value().unwrap(),
        vec![closure_two_object.id()],
    )
    .unwrap();
    let state_two_object = authority_object(
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
        state_two.schema_value().unwrap(),
        vec![closure_two_object.id(), guard_two_object.id()],
    )
    .unwrap();
    let context_object = authority_object(
        AuthoritySchemaV1::AuthorityContext,
        facts.context().schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let actor_binding = authority_object(
        AuthoritySchemaV1::PrincipalBinding,
        facts.actor_binding().schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let responder_binding = authority_object(
        AuthoritySchemaV1::PrincipalBinding,
        facts.responder_binding().schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let actor_session = authority_object(
        AuthoritySchemaV1::Session,
        facts.actor_session().schema_value().unwrap(),
        vec![actor_binding.id()],
    )
    .unwrap();
    let responder_session = authority_object(
        AuthoritySchemaV1::Session,
        facts.responder_session().schema_value().unwrap(),
        vec![responder_binding.id()],
    )
    .unwrap();
    let grant = authority_object(
        AuthoritySchemaV1::BootstrapGenesisGrant,
        facts.g0_candidate_paths()[0]
            .genesis_grant()
            .schema_value()
            .unwrap(),
        vec![],
    )
    .unwrap();
    let revocations = authority_object(
        AuthoritySchemaV1::RevocationSet,
        facts.revocations().schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let interaction = authority_object(
        AuthoritySchemaV1::BootstrapMandateInteractionObservationJoin,
        facts.interaction_join().unwrap().schema_value().unwrap(),
        vec![responder_session.id()],
    )
    .unwrap();
    let consent_slot = authority_object(
        AuthoritySchemaV1::ConsentSlotBindingParameter,
        facts.consent_slot().binding().schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let capacity_root = GovernedCapacityRootV1::new(
        CapacityRootIdV1::derive("repository-admin-capacity").unwrap(),
        AuthorityContextKindV1::RepositoryAuthorityContext,
        facts.context().context_id(),
        GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryAuthorityAdministration,
        ),
        64,
    )
    .unwrap();
    let capacity_root = authority_object(
        AuthoritySchemaV1::GovernedCapacityRoot,
        capacity_root.schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    let references = vec![
        manifest_object.id(),
        closure_two_object.id(),
        guard_two_object.id(),
        state_two_object.id(),
        context_object.id(),
        actor_binding.id(),
        responder_binding.id(),
        actor_session.id(),
        responder_session.id(),
        grant.id(),
        revocations.id(),
        interaction.id(),
        consent_slot.id(),
        capacity_root.id(),
    ];
    let authority_root = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        facts.schema_value().unwrap(),
        references.clone(),
    )
    .unwrap();
    let heterogeneous_root = StoreObjectV1::new(
        RepositoryStoreSchemaV1::WorkRecord.schema_id().unwrap(),
        CborValue::text("unrelated-repository-root").unwrap(),
        vec![],
    )
    .unwrap();
    let mut duplicate_references = references;
    duplicate_references.push(heterogeneous_root.id());
    let duplicate_authority_root = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        facts.schema_value().unwrap(),
        duplicate_references,
    )
    .unwrap();
    for object in [
        &closure_two_object,
        &guard_two_object,
        &state_two_object,
        &context_object,
        &actor_binding,
        &responder_binding,
        &actor_session,
        &responder_session,
        &grant,
        &revocations,
        &interaction,
        &consent_slot,
        &capacity_root,
        &authority_root,
        &heterogeneous_root,
        &duplicate_authority_root,
    ] {
        store.put_object(object).unwrap();
    }
    let mut generation_roots = match diagnostic_mode {
        ProtectedDiagnosticFixtureModeV1::HeterogeneousRoot => {
            vec![authority_root.id(), heterogeneous_root.id()]
        }
        ProtectedDiagnosticFixtureModeV1::MissingAuthorityRoot => {
            vec![heterogeneous_root.id()]
        }
        ProtectedDiagnosticFixtureModeV1::DuplicateAuthorityRoot => {
            vec![authority_root.id(), duplicate_authority_root.id()]
        }
        _ => vec![authority_root.id()],
    };
    generation_roots.sort_unstable();
    let generation_two = StoreGenerationV1::new(
        domain.clone(),
        2,
        Some(generation_one.id()),
        contract_root,
        StoreCompatibilityV1::stage0_successor().unwrap(),
        generation_roots,
    )
    .unwrap();
    let head_two = store
        .publish_generation(&generation_two, Some(head_one.id()))
        .unwrap();
    let invocation_facts = ProtectedDiagnosticInvocationFactsV1 {
        protected_subject: ContinuityReferenceV1::from_digest(*authority_root.id().as_bytes()),
    };
    if active {
        let connection = Connection::open(root.join("store.sqlite3")).unwrap();
        assert_eq!(
            connection
                .execute(
                    "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                    [],
                )
                .unwrap(),
            1
        );
    }
    (
        root,
        domain,
        store,
        contract_root,
        head_two,
        authority_root.id(),
        request,
        invocation_facts,
        authenticated_carrier,
    )
}

fn seeded_store() -> (
    std::path::PathBuf,
    StoreDomainV1,
    StoreV1,
    ContractRootIdV1,
    crate::domain::persistence::StoreHeadV1,
    StoreObjectIdV1,
    IssueBootstrapMandateRequestV1,
    ProtectedDiagnosticInvocationFactsV1,
    Vec<u8>,
) {
    seeded_store_with_activation(true)
}

fn plan(
    request: IssueBootstrapMandateRequestV1,
    contract_root: ContractRootIdV1,
    head: &crate::domain::persistence::StoreHeadV1,
    authority_root: StoreObjectIdV1,
) -> IssueBootstrapMandatePublicationV1 {
    IssueBootstrapMandatePublicationV1::new(
        request,
        super::super::AuthorityPublicationLineageV1::successor(
            contract_root,
            head.generation_id(),
            head.id(),
            authority_root,
        ),
        Some([23; 32]),
    )
    .unwrap()
}

fn duplicate_request(request: &IssueBootstrapMandateRequestV1) -> IssueBootstrapMandateRequestV1 {
    IssueBootstrapMandateRequestV1::try_from(IssueBootstrapMandateInputV1 {
        request_id: ActionRequestIdV1::derive("different-request").unwrap(),
        idempotency_key: IdempotencyKeyIdV1::derive("different-idempotency-key").unwrap(),
        context_id: request.context_id(),
        actor_binding_id: request.actor_binding_id(),
        actor_session_id: request.actor_session_id(),
        responder_binding_id: request.responder_binding_id(),
        presentation_observation_id: request.presentation_observation_id(),
        response_observation_id: request.response_observation_id(),
        target: request.target(),
        target_subject: request.target_subject().to_owned(),
        target_revision: request.target_revision(),
        consent_slot: request.consent_slot().clone(),
        supplied_mandates: vec![],
    })
    .unwrap()
}

fn admin_grant_issue_plan(
    seed: &str,
    contract_root: ContractRootIdV1,
    head: &crate::domain::persistence::StoreHeadV1,
    authority_root: StoreObjectIdV1,
) -> IssueRootAttachedBoundedGrantPublicationV1 {
    let context_id = AuthorityContextIdV1::derive("repository-context").unwrap();
    let capacity_root_id = CapacityRootIdV1::derive("repository-admin-capacity").unwrap();
    let parent_grant_id = GrantIdV1::derive("genesis-grant").unwrap();
    let delegation_id = DelegationIdV1::derive(&format!("{seed}-delegation")).unwrap();
    let child_grant_id = GrantIdV1::derive(&format!("{seed}-grant")).unwrap();
    let child = GrantDefinitionV1 {
        id: child_grant_id,
        context_id,
        grantee_principal_id: PrincipalIdV1::derive("responder-principal").unwrap(),
        parent_grant_id: Some(parent_grant_id),
        delegation_id: Some(delegation_id),
        terminal_scope: GrantScopeV1::new(vec![
            ScopeAtomV1::new(
                "ReissueRootAttachedGrantOneToOne",
                &capacity_root_id.render(),
                9,
            )
            .unwrap(),
            ScopeAtomV1::new("RevokeGrant", &capacity_root_id.render(), 9).unwrap(),
        ])
        .unwrap(),
        delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
        validity: HalfOpenValidityV1::new(110, 190).unwrap(),
        delegation_depth_remaining: 7,
        authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(capacity_root_id),
    }
    .validate()
    .unwrap();
    let child = OrdinaryBoundedGrantV1::new(child).unwrap();
    let delegation = OrdinaryGrantDelegationV1::new(
        context_id,
        capacity_root_id,
        DelegationV1::new(delegation_id, parent_grant_id, child_grant_id),
        &child,
    )
    .unwrap();
    IssueRootAttachedBoundedGrantPublicationV1::new(
        GrantActionIdentityV1::new(
            ActionRequestIdV1::derive(&format!("{seed}-request")).unwrap(),
            IdempotencyKeyIdV1::derive(&format!("{seed}-key")).unwrap(),
        ),
        super::super::AuthorityPublicationLineageV1::successor(
            contract_root,
            head.generation_id(),
            head.id(),
            authority_root,
        ),
        GenesisGrantIdV1::derive(&parent_grant_id.render()).unwrap(),
        child,
        delegation,
    )
    .unwrap()
}

fn ordinary_test_grant(
    seed: &str,
    parent_grant_id: GrantIdV1,
    capacity_root_id: CapacityRootIdV1,
    terminal_scope: GrantScopeV1,
    delegable_scope: GrantScopeV1,
    delegation_depth_remaining: u8,
) -> OrdinaryBoundedGrantV1 {
    OrdinaryBoundedGrantV1::new(
        GrantDefinitionV1 {
            id: GrantIdV1::derive(&format!("{seed}-grant")).unwrap(),
            context_id: AuthorityContextIdV1::derive("repository-context").unwrap(),
            grantee_principal_id: PrincipalIdV1::derive(&format!("{seed}-principal")).unwrap(),
            parent_grant_id: Some(parent_grant_id),
            delegation_id: Some(DelegationIdV1::derive(&format!("{seed}-delegation")).unwrap()),
            terminal_scope,
            delegable_scope,
            validity: HalfOpenValidityV1::new(110, 190).unwrap(),
            delegation_depth_remaining,
            authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(capacity_root_id),
        }
        .validate()
        .unwrap(),
    )
    .unwrap()
}

fn produced_ids(result: &StoreObjectV1) -> Vec<[u8; 32]> {
    let CborValue::Array(fields) = result.value() else {
        panic!("result must use the canonical array carrier");
    };
    let CborValue::Array(produced) = &fields[7] else {
        panic!("result produced ids must use the canonical array carrier");
    };
    produced
        .iter()
        .map(|value| {
            let CborValue::Bytes(bytes) = value else {
                panic!("result produced id must be bytes");
            };
            bytes.as_slice().try_into().unwrap()
        })
        .collect()
}

fn active_objects(store: &StoreV1) -> Vec<StoreObjectV1> {
    let head = store.active_head().unwrap().unwrap();
    let generation = store.publication_generation(head.id()).unwrap();
    objects_from_roots(store, generation.roots())
}

fn objects_from_roots(store: &StoreV1, roots: &[StoreObjectIdV1]) -> Vec<StoreObjectV1> {
    let mut pending = roots.iter().copied().collect::<VecDeque<_>>();
    let mut seen = BTreeSet::new();
    let mut objects = Vec::new();
    while let Some(object_id) = pending.pop_front() {
        if !seen.insert(object_id) {
            continue;
        }
        let object = store.read_object(object_id).unwrap();
        pending.extend(object.references().iter().copied());
        objects.push(object);
    }
    objects
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
        store.put_object(&object).unwrap();
        inserted.insert(object.id());
    }
}

fn removal_consumer_binding(seed: u8) -> LegacyRemovalConsumerBindingV3 {
    LegacyRemovalConsumerBindingV3 {
        consumer_closure: [seed; 32],
        release: [seed + 1; 32],
        source_case_manifest: [seed + 2; 32],
        sighting_manifest: [seed + 3; 32],
        classification_manifest: [seed + 4; 32],
        overlap_manifest: [seed + 5; 32],
        loss_manifest: [seed + 6; 32],
        quarantine_manifest: [seed + 7; 32],
        foundation_closure: [seed + 8; 32],
        rollback_assessment: [seed + 9; 32],
        quarantine_epoch: [seed + 10; 32],
        replacement_activation: [seed + 11; 32],
        reader_closure: [seed + 12; 32],
        hold_closure: [seed + 13; 32],
        rollback_rehearsal: [seed + 14; 32],
        deletion_plan: [seed + 15; 32],
        expected_old_state: [seed + 16; 32],
    }
}

fn seeded_installation_removal_store() -> (
    std::path::PathBuf,
    StoreDomainV1,
    StoreV1,
    StoreGenerationV1,
    StoreHeadV1,
    StoreObjectIdV1,
    ContractRootIdV1,
    LegacyRemovalGuardCurrentnessV3,
) {
    use super::test_support::{AuthorityFixtureModeV1, installation_authority_fixture};

    let fixture = installation_authority_fixture(vec![], AuthorityFixtureModeV1::Valid);
    let authority_root = fixture.authority_root_id;
    let root = test_root();
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Installation, b"legacy-removal-currentness").unwrap();
    let mut store = StoreV1::create(&root, domain.clone()).unwrap();
    put_objects_in_reference_order(&mut store, fixture.objects);
    let contract_root =
        ContractRootIdV1::from_digest(digest("legacy-removal-currentness-contract"));
    let generation = StoreGenerationV1::new(
        domain.clone(),
        1,
        None,
        contract_root,
        StoreCompatibilityV1::stage0_successor().unwrap(),
        vec![authority_root],
    )
    .unwrap();
    let head = store.publish_generation(&generation, None).unwrap();
    Connection::open(root.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE store_state SET state = 'active', state_revision = state_revision + 1
             WHERE singleton = 1",
            [],
        )
        .unwrap();
    let currentness = store
        .with_serialized_active_view(observe_legacy_removal_guard_currentness_v3)
        .unwrap();
    (
        root,
        domain,
        store,
        generation,
        head,
        authority_root,
        contract_root,
        currentness,
    )
}

#[test]
fn scheduling_publication_derives_the_unique_active_root_inside_authority() {
    use super::test_support::{AuthorityFixtureModeV1, repository_owner_family_authority_fixture};

    let action = RepositoryDownstreamActionLeafV1::PUBLISH_SCHEDULING_POLICY_BINDING;
    let subject = digest("authority-owned-scheduling-root-subject");
    let owner_basis = digest("authority-owned-scheduling-root-basis");
    let mut fixture = repository_owner_family_authority_fixture(
        vec![(action.literal(), subject)],
        AuthorityFixtureModeV1::Valid,
    );
    let root = test_root();
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"authority-scheduling-root").unwrap();
    let governance_floor = repository_governance_floor_for_fixture(&domain, &fixture);
    fixture.objects.push(governance_floor.clone());
    let mut store = StoreV1::create(&root, domain.clone()).unwrap();
    put_objects_in_reference_order(&mut store, fixture.objects);
    let mut roots = vec![fixture.authority_root_id, governance_floor.id()];
    roots.sort_unstable();
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        ContractRootIdV1::from_digest(digest("authority-scheduling-root-contract")),
        StoreCompatibilityV1::stage0_successor().unwrap(),
        roots,
    )
    .unwrap();
    store.publish_generation(&generation, None).unwrap();
    Connection::open(root.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE store_state SET state = 'active', state_revision = state_revision + 1
             WHERE singleton = 1",
            [],
        )
        .unwrap();

    let policy = scheduling_policy(900, "initial");
    let binding = scheduling_binding(
        1,
        None,
        policy.clone(),
        SemanticPolicyDiffKindV1::Strengthening,
    );
    let schema = RepositoryStoreSchemaV1::WorkRecord.schema_id().unwrap();
    let request_object = StoreObjectV1::new(
        schema,
        CborValue::text("authority-scheduling-root-request").unwrap(),
        vec![],
    )
    .unwrap();
    let binding_object = StoreObjectV1::new(schema, binding.canonical_value(), vec![]).unwrap();
    let request_id = ActionRequestIdV1::derive("authority-scheduling-root-request").unwrap();
    let probe = StoreIdempotencyProbeV1::new(
        "maestro.test.authority-scheduling-root.v1",
        digest("authority-scheduling-root-key"),
        digest("authority-scheduling-root-meaning"),
    )
    .unwrap();
    let planning = PlanningSchedulingPolicyInputV1::from_stage7_planning(
        &policy,
        &scheduling_safety_floor(1_000),
    )
    .unwrap();

    let unchanged = store.active_head().unwrap().unwrap();
    let mismatched_object = StoreObjectV1::new(
        schema,
        binding.canonical_value(),
        vec![governance_floor.id()],
    )
    .unwrap();
    let mismatched_authority = PlanningRepositoryActionAuthorityV1::new(
        fixture.selection,
        action,
        subject,
        owner_basis,
        *mismatched_object.id().as_bytes(),
    )
    .unwrap();
    assert!(
        AuthorityFacadeV1::new(&mut store)
            .publish_scheduling_policy(
                &probe,
                mismatched_authority,
                SchedulingPolicyPublicationInputV1::new(
                    request_id,
                    request_object.clone(),
                    mismatched_object,
                    planning.clone(),
                ),
            )
            .is_err()
    );
    assert_eq!(store.active_head().unwrap().unwrap(), unchanged);

    let authority = PlanningRepositoryActionAuthorityV1::new(
        fixture.selection,
        action,
        subject,
        owner_basis,
        *binding_object.id().as_bytes(),
    )
    .unwrap();
    let committed = AuthorityFacadeV1::new(&mut store)
        .publish_scheduling_policy(
            &probe,
            authority,
            SchedulingPolicyPublicationInputV1::new(
                request_id,
                request_object,
                binding_object.clone(),
                planning,
            ),
        )
        .unwrap();
    assert!(matches!(
        committed,
        StorePublicationOutcomeV1::Committed { .. }
    ));
    assert!(
        store
            .generation(committed.head().generation_id())
            .unwrap()
            .roots()
            .contains(&binding_object.id())
    );

    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn actual_scheduling_weakening_requires_a_live_mandate_and_writes_nothing_without_it() {
    use super::test_support::{AuthorityFixtureModeV1, repository_owner_family_authority_fixture};

    let action = RepositoryDownstreamActionLeafV1::PUBLISH_SCHEDULING_POLICY_BINDING;
    let subject = digest("authority-scheduling-weakening-subject");
    let owner_basis = digest("authority-scheduling-weakening-basis");
    let current_policy = scheduling_policy(800, "current-strong");
    let current_binding = scheduling_binding(
        1,
        None,
        current_policy,
        SemanticPolicyDiffKindV1::Strengthening,
    );
    let schema = RepositoryStoreSchemaV1::WorkRecord.schema_id().unwrap();
    let current_binding_object =
        StoreObjectV1::new(schema, current_binding.canonical_value(), vec![]).unwrap();
    let candidate_policy = scheduling_policy(900, "candidate-weaker");
    let candidate_binding = scheduling_binding(
        2,
        Some(&current_binding),
        candidate_policy.clone(),
        SemanticPolicyDiffKindV1::Weakening,
    );
    let candidate_binding_object = StoreObjectV1::new(
        schema,
        candidate_binding.canonical_value(),
        vec![current_binding_object.id()],
    )
    .unwrap();
    let request_object = StoreObjectV1::new(
        schema,
        CborValue::text("authority-scheduling-weakening-request").unwrap(),
        vec![],
    )
    .unwrap();
    let mut fixture = repository_owner_family_authority_fixture(
        vec![(action.literal(), subject)],
        AuthorityFixtureModeV1::Valid,
    );
    let root = test_root();
    let domain =
        StoreDomainV1::derive(StoreRoleV1::Repository, b"authority-scheduling-weakening").unwrap();
    let governance_floor = repository_governance_floor_for_fixture(&domain, &fixture);
    fixture
        .objects
        .extend([governance_floor.clone(), current_binding_object.clone()]);
    let mut store = StoreV1::create(&root, domain.clone()).unwrap();
    put_objects_in_reference_order(&mut store, fixture.objects);
    let mut roots = vec![
        fixture.authority_root_id,
        governance_floor.id(),
        current_binding_object.id(),
    ];
    roots.sort_unstable();
    let generation = StoreGenerationV1::new(
        domain,
        1,
        None,
        ContractRootIdV1::from_digest(digest("authority-scheduling-weakening-contract")),
        StoreCompatibilityV1::stage0_successor().unwrap(),
        roots,
    )
    .unwrap();
    store.publish_generation(&generation, None).unwrap();
    Connection::open(root.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE store_state SET state = 'active', state_revision = state_revision + 1
             WHERE singleton = 1",
            [],
        )
        .unwrap();

    let request_id = ActionRequestIdV1::derive("authority-scheduling-weakening-request").unwrap();
    let probe = StoreIdempotencyProbeV1::new(
        "maestro.test.authority-scheduling-weakening.v1",
        digest("authority-scheduling-weakening-key"),
        digest("authority-scheduling-weakening-meaning"),
    )
    .unwrap();
    let authority = PlanningRepositoryActionAuthorityV1::new(
        fixture.selection,
        action,
        subject,
        owner_basis,
        *candidate_binding_object.id().as_bytes(),
    )
    .unwrap();
    let planning = PlanningSchedulingPolicyInputV1::from_stage7_planning(
        &candidate_policy,
        &scheduling_safety_floor(1_000),
    )
    .unwrap();
    let unchanged = store.active_head().unwrap().unwrap();
    let wrong_root_object = StoreObjectV1::new(
        schema,
        candidate_binding.canonical_value(),
        vec![governance_floor.id()],
    )
    .unwrap();
    let wrong_root_authority = PlanningRepositoryActionAuthorityV1::new(
        fixture.selection,
        action,
        subject,
        owner_basis,
        *wrong_root_object.id().as_bytes(),
    )
    .unwrap();
    assert!(
        AuthorityFacadeV1::new(&mut store)
            .publish_scheduling_policy(
                &probe,
                wrong_root_authority,
                SchedulingPolicyPublicationInputV1::new(
                    request_id,
                    request_object.clone(),
                    wrong_root_object,
                    planning.clone(),
                ),
            )
            .is_err()
    );
    assert_eq!(store.active_head().unwrap().unwrap(), unchanged);

    assert!(
        AuthorityFacadeV1::new(&mut store)
            .publish_scheduling_policy(
                &probe,
                authority,
                SchedulingPolicyPublicationInputV1::new(
                    request_id,
                    request_object,
                    candidate_binding_object,
                    planning,
                ),
            )
            .is_err()
    );
    assert_eq!(store.active_head().unwrap().unwrap(), unchanged);

    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[expect(
    clippy::too_many_arguments,
    reason = "the test clone binds the exact source lineage and mutant Generation carriers"
)]
fn clone_store_through_mutant_generation_three(
    source_root: &std::path::Path,
    source_store: &StoreV1,
    domain: &StoreDomainV1,
    contract_root: ContractRootIdV1,
    head_two: &crate::domain::persistence::StoreHeadV1,
    source_generation_three: &StoreGenerationV1,
    mutant_root_object: &StoreObjectV1,
    mutant_result_object_id: StoreObjectIdV1,
    mut objects: Vec<StoreObjectV1>,
) -> (
    std::path::PathBuf,
    StoreV1,
    crate::domain::persistence::StoreHeadV1,
) {
    let original_idempotency = Connection::open(source_root.join("store.sqlite3"))
        .unwrap()
        .query_row(
            "SELECT namespace, key_digest, meaning_digest
             FROM store_idempotency WHERE generation_id = ?1",
            [source_generation_three.id().as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .unwrap();
    let head_one_id = head_two.previous_head_id().unwrap();
    let generation_one = source_store.publication_generation(head_one_id).unwrap();
    let generation_two = source_store.publication_generation(head_two.id()).unwrap();
    objects.extend(objects_from_roots(source_store, generation_one.roots()));
    objects.push(mutant_root_object.clone());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());

    let mutant_root = test_root();
    let mut mutant_store = StoreV1::create(&mutant_root, domain.clone()).unwrap();
    put_objects_in_reference_order(&mut mutant_store, objects);
    let cloned_head_one = mutant_store
        .publish_generation(&generation_one, None)
        .unwrap();
    assert_eq!(cloned_head_one.id(), head_one_id);
    let cloned_head_two = mutant_store
        .publish_generation(&generation_two, Some(cloned_head_one.id()))
        .unwrap();
    assert_eq!(cloned_head_two.id(), head_two.id());
    let mutant_generation_three = StoreGenerationV1::new(
        domain.clone(),
        3,
        Some(generation_two.id()),
        contract_root,
        StoreCompatibilityV1::stage0_successor().unwrap(),
        vec![mutant_root_object.id()],
    )
    .unwrap();
    let mutant_head_three = mutant_store
        .publish_generation(&mutant_generation_three, Some(cloned_head_two.id()))
        .unwrap();
    let connection = Connection::open(mutant_root.join("store.sqlite3")).unwrap();
    connection
        .execute(
            "INSERT INTO store_idempotency
             (namespace, key_digest, meaning_digest, result_object_id, generation_id, head_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                original_idempotency.0,
                original_idempotency.1,
                original_idempotency.2,
                mutant_result_object_id.as_bytes(),
                mutant_generation_three.id().as_bytes(),
                mutant_head_three.id().as_bytes(),
            ],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE store_state SET state = 'active', state_revision = state_revision + 1
             WHERE singleton = 1",
            [],
        )
        .unwrap();
    drop(connection);
    (mutant_root, mutant_store, mutant_head_three)
}

#[test]
fn store_loaded_authority_and_complete_post_cut_publish_atomically_and_replay_without_writes() {
    let (root, _domain, mut store, contract_root, head, authority_root, request, _, _) =
        seeded_store();
    let publication = plan(request, contract_root, &head, authority_root);
    let first = AuthorityFacadeV1::new(&mut store)
        .issue_bootstrap_mandate(publication.clone())
        .unwrap();
    assert_eq!(first.kind(), AuthorityPublicationKindV1::Committed);
    assert_eq!(first.head().generation_ordinal(), 3);
    let replay = AuthorityFacadeV1::new(&mut store)
        .issue_bootstrap_mandate(publication)
        .unwrap();
    assert_eq!(replay.kind(), AuthorityPublicationKindV1::Replayed);
    assert_eq!(replay.logical_result_id(), first.logical_result_id());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn g0_issues_root_attached_bounded_grant_atomically_without_capacity_debit() {
    let (root, _domain, mut store, contract_root, head, authority_root, _request, _, _) =
        seeded_store();
    let context_id = AuthorityContextIdV1::derive("repository-context").unwrap();
    let capacity_root_id = CapacityRootIdV1::derive("repository-admin-capacity").unwrap();
    let parent_grant_id = GrantIdV1::derive("genesis-grant").unwrap();
    let delegation_id = DelegationIdV1::derive("admin-delegation").unwrap();
    let child_grant_id = GrantIdV1::derive("admin-grant").unwrap();
    let child = GrantDefinitionV1 {
        id: child_grant_id,
        context_id,
        grantee_principal_id: PrincipalIdV1::derive("delegate-admin").unwrap(),
        parent_grant_id: Some(parent_grant_id),
        delegation_id: Some(delegation_id),
        terminal_scope: GrantScopeV1::new(vec![
            ScopeAtomV1::new(
                "ReissueRootAttachedGrantOneToOne",
                &capacity_root_id.render(),
                9,
            )
            .unwrap(),
            ScopeAtomV1::new("RevokeGrant", &capacity_root_id.render(), 9).unwrap(),
        ])
        .unwrap(),
        delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
        validity: HalfOpenValidityV1::new(110, 190).unwrap(),
        delegation_depth_remaining: 7,
        authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(capacity_root_id),
    }
    .validate()
    .unwrap();
    let child = OrdinaryBoundedGrantV1::new(child).unwrap();
    let delegation = OrdinaryGrantDelegationV1::new(
        context_id,
        capacity_root_id,
        DelegationV1::new(delegation_id, parent_grant_id, child_grant_id),
        &child,
    )
    .unwrap();
    let publication = IssueRootAttachedBoundedGrantPublicationV1::new(
        GrantActionIdentityV1::new(
            ActionRequestIdV1::derive("issue-admin-grant").unwrap(),
            IdempotencyKeyIdV1::derive("issue-admin-grant-key").unwrap(),
        ),
        super::super::AuthorityPublicationLineageV1::successor(
            contract_root,
            head.generation_id(),
            head.id(),
            authority_root,
        ),
        GenesisGrantIdV1::derive(&parent_grant_id.render()).unwrap(),
        child,
        delegation,
    )
    .unwrap();

    let mut wrong_root_publication = publication.clone();
    let wrong_root = CapacityRootIdV1::derive("not-established-capacity").unwrap();
    let mut wrong_definition = wrong_root_publication.grant.grant().definition();
    wrong_definition.authority_use_constraint = AuthorityUseConstraintV1::BoundedBy(wrong_root);
    wrong_root_publication.grant =
        OrdinaryBoundedGrantV1::new(wrong_definition.validate().unwrap()).unwrap();
    wrong_root_publication.delegation = OrdinaryGrantDelegationV1::new(
        context_id,
        wrong_root,
        DelegationV1::new(delegation_id, parent_grant_id, child_grant_id),
        &wrong_root_publication.grant,
    )
    .unwrap();
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store)
            .issue_root_attached_bounded_grant(wrong_root_publication),
        Err(AuthorityPublicationError::InvalidBootstrapGrantAuthority)
    ));
    assert_eq!(store.active_head().unwrap().unwrap().id(), head.id());

    let first = AuthorityFacadeV1::new(&mut store)
        .issue_root_attached_bounded_grant(publication.clone())
        .unwrap();
    assert_eq!(first.kind(), AuthorityPublicationKindV1::Committed);
    assert_eq!(first.head().generation_ordinal(), 3);
    let objects = active_objects(&store);
    assert_eq!(
        schema_objects(&objects, AuthoritySchemaV1::OrdinaryBoundedGrant)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        schema_objects(&objects, AuthoritySchemaV1::OrdinaryGrantDelegation)
            .unwrap()
            .len(),
        1
    );
    assert!(
        schema_objects(&objects, AuthoritySchemaV1::GovernedCapacityDebit)
            .unwrap()
            .is_empty()
    );
    let mut meaning_conflict = publication.clone();
    meaning_conflict.identity = GrantActionIdentityV1::new(
        ActionRequestIdV1::derive("changed-meaning-request").unwrap(),
        meaning_conflict.identity.idempotency_key(),
    );
    assert!(
        AuthorityFacadeV1::new(&mut store)
            .issue_root_attached_bounded_grant(meaning_conflict)
            .is_err()
    );
    assert_eq!(
        store.active_head().unwrap().unwrap().id(),
        first.head().id()
    );
    let replay = AuthorityFacadeV1::new(&mut store)
        .issue_root_attached_bounded_grant(publication)
        .unwrap();
    assert_eq!(replay.kind(), AuthorityPublicationKindV1::Replayed);
    assert_eq!(replay.head().id(), first.head().id());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordinary_admin_refuses_a_non_administration_capacity_kind() {
    let context_id = AuthorityContextIdV1::derive("wrong-kind-context").unwrap();
    let root_id = CapacityRootIdV1::derive("wrong-kind-root").unwrap();
    let root = GovernedCapacityRootV1::new(
        root_id,
        AuthorityContextKindV1::RepositoryAuthorityContext,
        context_id,
        GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation,
        ),
        4,
    )
    .unwrap();
    let object = authority_object(
        AuthoritySchemaV1::GovernedCapacityRoot,
        root.schema_value().unwrap(),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        current_repository_admin_capacity_root(&[object], context_id, root_id),
        Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority)
    ));
}

#[test]
fn ordinary_admin_reissues_and_revokes_with_one_debit_each_and_retirement_closure() {
    let (root, _domain, mut store, contract_root, head_two, authority_root, _request, _, _) =
        seeded_store();
    let admin_a = admin_grant_issue_plan("admin-a", contract_root, &head_two, authority_root);
    AuthorityFacadeV1::new(&mut store)
        .issue_root_attached_bounded_grant(admin_a)
        .unwrap();
    let head_three = store.active_head().unwrap().unwrap();
    let generation_three = store.publication_generation(head_three.id()).unwrap();
    let [root_three] = generation_three.roots() else {
        panic!("Grant issue successor must have one Authority root");
    };
    let admin_b = admin_grant_issue_plan("admin-b", contract_root, &head_three, *root_three);
    AuthorityFacadeV1::new(&mut store)
        .issue_root_attached_bounded_grant(admin_b)
        .unwrap();
    let head_four = store.active_head().unwrap().unwrap();
    let generation_four = store.publication_generation(head_four.id()).unwrap();
    let [root_four] = generation_four.roots() else {
        panic!("Grant issue successor must have one Authority root");
    };

    let candidate = admin_grant_issue_plan("admin-c", contract_root, &head_four, *root_four);
    let admin_a_id = GrantIdV1::derive("admin-a-grant").unwrap();
    let admin_b_id = GrantIdV1::derive("admin-b-grant").unwrap();
    let admin_c_id = GrantIdV1::derive("admin-c-grant").unwrap();
    let administrator = GrantAdministrationAuthorityV1::new(
        PrincipalBindingIdV1::derive("responder-binding").unwrap(),
        SessionIdV1::derive("responder-session").unwrap(),
        admin_a_id,
    );
    assert_eq!(
        RevokeGrantPublicationV1::new(
            GrantActionIdentityV1::new(
                ActionRequestIdV1::derive("self-revoke-admin-a").unwrap(),
                IdempotencyKeyIdV1::derive("self-revoke-admin-a-key").unwrap(),
            ),
            super::super::AuthorityPublicationLineageV1::successor(
                contract_root,
                head_four.generation_id(),
                head_four.id(),
                *root_four,
            ),
            administrator,
            admin_a_id,
        ),
        Err(super::super::AuthorityPublicationPlanError::SelfAuthorizingGrantMutation)
    );

    let mut widened_definition = candidate.grant.grant().definition();
    let mut widened_atoms = widened_definition
        .terminal_scope
        .atoms()
        .cloned()
        .collect::<Vec<_>>();
    widened_atoms.push(
        ScopeAtomV1::new(
            "UnexpectedAdministrativeAuthority",
            &CapacityRootIdV1::derive("repository-admin-capacity")
                .unwrap()
                .render(),
            9,
        )
        .unwrap(),
    );
    widened_definition.terminal_scope = GrantScopeV1::new(widened_atoms).unwrap();
    let widened_grant =
        OrdinaryBoundedGrantV1::new(widened_definition.validate().unwrap()).unwrap();
    let widened_delegation = OrdinaryGrantDelegationV1::new(
        widened_grant.grant().context_id(),
        widened_grant.capacity_root_id(),
        candidate.delegation.delegation(),
        &widened_grant,
    )
    .unwrap();
    let widening = ReissueRootAttachedGrantOneToOnePublicationV1::new(
        GrantActionIdentityV1::new(
            ActionRequestIdV1::derive("widen-admin-b").unwrap(),
            IdempotencyKeyIdV1::derive("widen-admin-b-key").unwrap(),
        ),
        super::super::AuthorityPublicationLineageV1::successor(
            contract_root,
            head_four.generation_id(),
            head_four.id(),
            *root_four,
        ),
        administrator,
        admin_b_id,
        widened_grant,
        widened_delegation,
    )
    .unwrap();
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).reissue_root_attached_grant_one_to_one(widening),
        Err(AuthorityPublicationError::GrantReissueWidening)
    ));
    assert_eq!(store.active_head().unwrap().unwrap().id(), head_four.id());

    let reissue = ReissueRootAttachedGrantOneToOnePublicationV1::new(
        GrantActionIdentityV1::new(
            ActionRequestIdV1::derive("reissue-admin-b").unwrap(),
            IdempotencyKeyIdV1::derive("reissue-admin-b-key").unwrap(),
        ),
        super::super::AuthorityPublicationLineageV1::successor(
            contract_root,
            head_four.generation_id(),
            head_four.id(),
            *root_four,
        ),
        administrator,
        admin_b_id,
        candidate.grant,
        candidate.delegation,
    )
    .unwrap();
    AuthorityFacadeV1::new(&mut store)
        .reissue_root_attached_grant_one_to_one(reissue)
        .unwrap();
    let objects_after_reissue = active_objects(&store);
    assert_eq!(
        schema_objects(
            &objects_after_reissue,
            AuthoritySchemaV1::GovernedCapacityDebit
        )
        .unwrap()
        .len(),
        1
    );
    let revocations_after_reissue =
        one_schema_object(&objects_after_reissue, AuthoritySchemaV1::RevocationSet).unwrap();
    let revocations_after_reissue = AuthorityRevocationSetV1::from_canonical_bytes(
        &object_value_bytes(&revocations_after_reissue).unwrap(),
    )
    .unwrap();
    assert!(
        revocations_after_reissue
            .revocations()
            .contains(super::super::RevocationTargetV1::Grant(admin_b_id))
    );
    let successor_snapshot = one_schema_object(
        &objects_after_reissue,
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
    )
    .unwrap();
    let successor_facts = BootstrapAuthoritySnapshotV1::from_canonical_bytes(
        &object_value_bytes(&successor_snapshot).unwrap(),
    )
    .unwrap();
    let admitted_candidate = schema_objects(
        &objects_after_reissue,
        AuthoritySchemaV1::OrdinaryBoundedGrant,
    )
    .unwrap()
    .into_iter()
    .map(|object| {
        OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(&object).unwrap()).unwrap()
    })
    .find(|grant| grant.grant().id() == admin_c_id)
    .unwrap();
    let admitted_delegation = schema_objects(
        &objects_after_reissue,
        AuthoritySchemaV1::OrdinaryGrantDelegation,
    )
    .unwrap()
    .into_iter()
    .find_map(|object| {
        OrdinaryGrantDelegationV1::from_canonical_bytes(
            &object_value_bytes(&object).unwrap(),
            &admitted_candidate,
        )
        .ok()
    })
    .unwrap();
    super::super::admit_repository_authority_candidate(
        &successor_facts,
        admitted_candidate.capacity_root_id(),
        &admitted_candidate,
        &admitted_delegation,
    )
    .unwrap();

    let head_five = store.active_head().unwrap().unwrap();
    let generation_five = store.publication_generation(head_five.id()).unwrap();
    let [root_five] = generation_five.roots() else {
        panic!("Grant reissue successor must have one Authority root");
    };
    let revoke = RevokeGrantPublicationV1::new(
        GrantActionIdentityV1::new(
            ActionRequestIdV1::derive("revoke-admin-c").unwrap(),
            IdempotencyKeyIdV1::derive("revoke-admin-c-key").unwrap(),
        ),
        super::super::AuthorityPublicationLineageV1::successor(
            contract_root,
            head_five.generation_id(),
            head_five.id(),
            *root_five,
        ),
        administrator,
        admin_c_id,
    )
    .unwrap();
    let revoked = AuthorityFacadeV1::new(&mut store)
        .revoke_grant(revoke.clone())
        .unwrap();
    assert_eq!(revoked.kind(), AuthorityPublicationKindV1::Committed);
    let objects_after_revoke = active_objects(&store);
    assert_eq!(
        schema_objects(
            &objects_after_revoke,
            AuthoritySchemaV1::GovernedCapacityDebit
        )
        .unwrap()
        .len(),
        1
    );
    let capacity_after_revoke = schema_objects(
        &objects_after_revoke,
        AuthoritySchemaV1::GovernedCapacityRoot,
    )
    .unwrap()
    .into_iter()
    .find(|object| {
        matches!(object.value(), CborValue::Array(fields) if fields[6] == CborValue::Unsigned(2))
    })
    .unwrap();
    let CborValue::Array(capacity_fields) = capacity_after_revoke.value() else {
        panic!("capacity root must use the canonical array carrier");
    };
    assert_eq!(capacity_fields[6], CborValue::Unsigned(2));
    let revocations_after_revoke =
        one_schema_object(&objects_after_revoke, AuthoritySchemaV1::RevocationSet).unwrap();
    let revocations_after_revoke = AuthorityRevocationSetV1::from_canonical_bytes(
        &object_value_bytes(&revocations_after_revoke).unwrap(),
    )
    .unwrap();
    assert!(
        revocations_after_revoke
            .revocations()
            .contains(super::super::RevocationTargetV1::Grant(admin_b_id))
    );
    assert!(
        revocations_after_revoke
            .revocations()
            .contains(super::super::RevocationTargetV1::Grant(admin_c_id))
    );
    let replay = AuthorityFacadeV1::new(&mut store)
        .revoke_grant(revoke)
        .unwrap();
    assert_eq!(replay.kind(), AuthorityPublicationKindV1::Replayed);
    assert_eq!(replay.head().id(), revoked.head().id());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn one_to_one_reissue_refuses_an_ordinary_parent_even_on_the_same_capacity_root() {
    let (root, _domain, mut store, contract_root, head_two, authority_root, _request, _, _) =
        seeded_store();
    let mut parent =
        admin_grant_issue_plan("ordinary-parent", contract_root, &head_two, authority_root);
    let mut parent_definition = parent.grant.grant().definition();
    parent_definition.delegable_scope = parent_definition.terminal_scope.clone();
    parent.grant = OrdinaryBoundedGrantV1::new(parent_definition.validate().unwrap()).unwrap();
    parent.delegation = OrdinaryGrantDelegationV1::new(
        parent.grant.grant().context_id(),
        parent.grant.capacity_root_id(),
        parent.delegation.delegation(),
        &parent.grant,
    )
    .unwrap();
    AuthorityFacadeV1::new(&mut store)
        .issue_root_attached_bounded_grant(parent)
        .unwrap();

    let head_three = store.active_head().unwrap().unwrap();
    let generation_three = store.publication_generation(head_three.id()).unwrap();
    let [root_three] = generation_three.roots() else {
        panic!("Grant issue successor must have one Authority root");
    };
    let retired = admin_grant_issue_plan("retired-admin", contract_root, &head_three, *root_three);
    AuthorityFacadeV1::new(&mut store)
        .issue_root_attached_bounded_grant(retired)
        .unwrap();

    let head_four = store.active_head().unwrap().unwrap();
    let generation_four = store.publication_generation(head_four.id()).unwrap();
    let [root_four] = generation_four.roots() else {
        panic!("Grant issue successor must have one Authority root");
    };
    let mut candidate =
        admin_grant_issue_plan("ordinary-child", contract_root, &head_four, *root_four);
    let parent_grant_id = GrantIdV1::derive("ordinary-parent-grant").unwrap();
    let mut candidate_definition = candidate.grant.grant().definition();
    candidate_definition.parent_grant_id = Some(parent_grant_id);
    candidate_definition.delegation_depth_remaining = 6;
    candidate.grant =
        OrdinaryBoundedGrantV1::new(candidate_definition.validate().unwrap()).unwrap();
    candidate.delegation = OrdinaryGrantDelegationV1::new(
        candidate.grant.grant().context_id(),
        candidate.grant.capacity_root_id(),
        DelegationV1::new(
            candidate.delegation.delegation().id,
            parent_grant_id,
            candidate.grant.grant().id(),
        ),
        &candidate.grant,
    )
    .unwrap();
    let reissue = ReissueRootAttachedGrantOneToOnePublicationV1::new(
        candidate.identity,
        candidate.lineage,
        GrantAdministrationAuthorityV1::new(
            PrincipalBindingIdV1::derive("responder-binding").unwrap(),
            SessionIdV1::derive("responder-session").unwrap(),
            parent_grant_id,
        ),
        GrantIdV1::derive("retired-admin-grant").unwrap(),
        candidate.grant,
        candidate.delegation,
    )
    .unwrap();

    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).reissue_root_attached_grant_one_to_one(reissue),
        Err(AuthorityPublicationError::GrantReissueWidening)
    ));
    assert_eq!(store.active_head().unwrap().unwrap().id(), head_four.id());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn g0_to_a_to_b_cannot_use_b_to_revoke_or_reissue_ancestor_a() {
    let root = CapacityRootIdV1::derive("ancestor-admin-root").unwrap();
    let g0 = GrantIdV1::derive("ancestor-g0").unwrap();
    let admin_scope = || {
        GrantScopeV1::new(vec![
            ScopeAtomV1::new("ReissueRootAttachedGrantOneToOne", &root.render(), 9).unwrap(),
            ScopeAtomV1::new("RevokeGrant", &root.render(), 9).unwrap(),
        ])
        .unwrap()
    };
    let admin_a = ordinary_test_grant("ancestor-a", g0, root, admin_scope(), admin_scope(), 7);
    let admin_b = ordinary_test_grant(
        "ancestor-b",
        admin_a.grant().id(),
        root,
        admin_scope(),
        GrantScopeV1::new(vec![]).unwrap(),
        6,
    );
    let handoff = ordinary_test_grant(
        "ancestor-handoff",
        g0,
        root,
        admin_scope(),
        GrantScopeV1::new(vec![]).unwrap(),
        7,
    );
    let grants = vec![admin_a.clone(), admin_b.clone()];

    for action in ["RevokeGrant", "ReissueRootAttachedGrantOneToOne"] {
        assert!(matches!(
            reject_administrator_ancestor_mutation(action, &admin_b, admin_a.grant().id(), &grants,),
            Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority)
        ));
    }

    assert!(
        !has_independently_live_repository_administrator(
            IndependentRepositoryAdministratorCheckV1 {
                grants: &grants,
                candidate: None,
                target_grant_id: admin_a.grant().id(),
                current_revocations: &RevocationSetV1::empty(),
                capacity_root_id: root,
                action: "RevokeGrant",
                protocol_revision: 9,
                trusted_time: TrustedTimeV1::verified(120, 130).unwrap(),
            },
        )
        .unwrap()
    );
    assert!(
        has_independently_live_repository_administrator(
            IndependentRepositoryAdministratorCheckV1 {
                grants: &grants,
                candidate: Some(&handoff),
                target_grant_id: admin_a.grant().id(),
                current_revocations: &RevocationSetV1::empty(),
                capacity_root_id: root,
                action: "ReissueRootAttachedGrantOneToOne",
                protocol_revision: 9,
                trusted_time: TrustedTimeV1::verified(120, 130).unwrap(),
            },
        )
        .unwrap()
    );
}

#[test]
fn same_root_non_admin_does_not_satisfy_the_post_mutation_admin_invariant() {
    let root = CapacityRootIdV1::derive("non-admin-survivor-root").unwrap();
    let g0 = GrantIdV1::derive("non-admin-survivor-g0").unwrap();
    let admin_scope = GrantScopeV1::new(vec![
        ScopeAtomV1::new("ReissueRootAttachedGrantOneToOne", &root.render(), 9).unwrap(),
        ScopeAtomV1::new("RevokeGrant", &root.render(), 9).unwrap(),
    ])
    .unwrap();
    let admin = ordinary_test_grant(
        "only-admin",
        g0,
        root,
        admin_scope,
        GrantScopeV1::new(vec![]).unwrap(),
        7,
    );
    let non_admin = ordinary_test_grant(
        "same-root-reader",
        g0,
        root,
        GrantScopeV1::new(vec![
            ScopeAtomV1::new("CreateDraftWork", "repository", 9).unwrap(),
        ])
        .unwrap(),
        GrantScopeV1::new(vec![]).unwrap(),
        7,
    );

    assert!(
        !has_independently_live_repository_administrator(
            IndependentRepositoryAdministratorCheckV1 {
                grants: &[admin.clone(), non_admin],
                candidate: None,
                target_grant_id: admin.grant().id(),
                current_revocations: &RevocationSetV1::empty(),
                capacity_root_id: root,
                action: "RevokeGrant",
                protocol_revision: 9,
                trusted_time: TrustedTimeV1::verified(120, 130).unwrap(),
            },
        )
        .unwrap()
    );
}

#[test]
fn inactive_store_cannot_publish_authority() {
    let (root, _domain, mut store, contract_root, head, authority_root, request, _, _) =
        seeded_store_with_activation(false);
    let publication = plan(request, contract_root, &head, authority_root);
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).issue_bootstrap_mandate(publication),
        Err(AuthorityPublicationError::InactiveStore)
    ));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn inactive_store_refusal_mints_no_diagnostic_invocation() {
    let (root, _domain, mut store, _contract_root, _head, _authority_root, _request, diagnostic, _) =
        seeded_store_with_activation(false);
    let (
        active_root,
        _active_domain,
        mut active_store,
        _active_contract_root,
        active_head,
        _active_authority_root,
        _active_request,
        active_diagnostic,
        _,
    ) = seeded_store_with_activation(true);
    let (mut provider, mut connection) = active_diagnostic.test_ports(
        &active_root,
        active_store.domain(),
        &active_store,
        &active_head,
        b"inactive-diagnostic",
    );
    let control = connection.control();
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert!(!control.invocation_is_pending());
    assert_eq!(
        AuthorityFacadeV1::new(&mut active_store)
            .protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                active_diagnostic.protected_subject,
            )
            .unwrap()
            .disposition(),
        ProtectedContinuityDiagnosticDispositionV1::CurrentProtectedSnapshot
    );
    drop(active_store);
    drop(store);
    fs::remove_dir_all(active_root).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_guard_is_subject_bound_and_zero_write() {
    let (root, _domain, mut store, _contract_root, head, _authority_root, _request, diagnostic, _) =
        seeded_store();
    let before_head = store.active_head().unwrap().unwrap();
    let (mut provider, mut connection) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"successful-diagnostic",
    );
    let envelope = AuthorityFacadeV1::new(&mut store)
        .protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut provider,
            diagnostic.protected_subject,
        )
        .unwrap();
    assert_eq!(
        envelope.disposition(),
        ProtectedContinuityDiagnosticDispositionV1::CurrentProtectedSnapshot
    );
    assert!(provider.was_consumed());

    let first_issuer = ProtectedDiagnosticInvocationIssuerV1::fresh().unwrap();
    let second_issuer = ProtectedDiagnosticInvocationIssuerV1::fresh().unwrap();
    assert_eq!(
        first_issuer.process_incarnation,
        second_issuer.process_incarnation
    );
    assert_ne!(
        first_issuer.invocation_entropy,
        second_issuer.invocation_entropy
    );
    assert_ne!(first_issuer.sequence, second_issuer.sequence);

    let base_nonce = protected_diagnostic_nonce_derivation(
        [0x11; 32], [0x22; 32], 1, [0x33; 32], [0x44; 32], [0x55; 32],
    )
    .unwrap();
    assert_ne!(base_nonce, [0; 32]);
    assert_ne!(
        base_nonce,
        protected_diagnostic_nonce_derivation(
            [0x12; 32], [0x22; 32], 1, [0x33; 32], [0x44; 32], [0x55; 32],
        )
        .unwrap()
    );
    assert_ne!(
        base_nonce,
        protected_diagnostic_nonce_derivation(
            [0x11; 32], [0x23; 32], 1, [0x33; 32], [0x44; 32], [0x55; 32],
        )
        .unwrap()
    );
    assert_ne!(
        base_nonce,
        protected_diagnostic_nonce_derivation(
            [0x11; 32], [0x22; 32], 2, [0x33; 32], [0x44; 32], [0x55; 32],
        )
        .unwrap()
    );

    let (mut replay_provider, _) =
        diagnostic.test_ports(&root, store.domain(), &store, &head, b"replay-diagnostic");
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut replay_provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert!(replay_provider.was_consumed());

    assert_eq!(store.active_head().unwrap().unwrap(), before_head);
    assert_eq!(before_head, head);
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_guard_is_non_oracular_across_subjects() {
    let (root, _domain, mut store, _contract_root, _head, _authority_root, _request, diagnostic, _) =
        seeded_store();
    let unknown_subject = reference("protected-nonexistent-subject");
    let (mut unknown_provider, mut unknown_connection) =
        diagnostic.test_ports(&root, store.domain(), &store, &_head, b"unknown-subject");
    let unknown_error = AuthorityFacadeV1::new(&mut store)
        .protected_continuity_diagnostic_reference_envelope(
            &mut unknown_connection,
            &mut unknown_provider,
            unknown_subject,
        )
        .unwrap_err();

    let (mut wrong_provider, mut wrong_connection) =
        diagnostic.test_ports(&root, store.domain(), &store, &_head, b"wrong-operator");
    wrong_connection
        .control()
        .substitute_operator_identity_dimension(0);
    let wrong_error = AuthorityFacadeV1::new(&mut store)
        .protected_continuity_diagnostic_reference_envelope(
            &mut wrong_connection,
            &mut wrong_provider,
            diagnostic.protected_subject,
        )
        .unwrap_err();
    assert_eq!(unknown_error.to_string(), wrong_error.to_string());
    assert!(!unknown_error.to_string().contains("protected-nonexistent"));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_joins_every_independent_host_identity_dimension() {
    for dimension in 0..15 {
        let (
            root,
            _domain,
            mut store,
            _contract_root,
            head,
            _authority_root,
            _request,
            diagnostic,
            _,
        ) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            format!("wrong-host-dimension-{dimension}").as_bytes(),
        );
        connection
            .control()
            .substitute_operator_identity_dimension(dimension);
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    let (root, _domain, mut store, _contract_root, head, _, _, diagnostic, _) = seeded_store();
    let before_head = store.active_head().unwrap().unwrap();
    let (mut provider, mut connection) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"presentation-witness-commitment-mismatch",
    );
    connection
        .control()
        .substitute_presented_attestation_commitment();
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert_eq!(store.active_head().unwrap().unwrap(), before_head);
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_final_recheck_rejects_host_fence_turnover() {
    for schedule in [
        crate::domain::integration::TrustedHostDiagnosticTestControlV1::disconnect_on_final_recheck,
        crate::domain::integration::TrustedHostDiagnosticTestControlV1::revoke_on_final_recheck,
        crate::domain::integration::TrustedHostDiagnosticTestControlV1::advance_currentness_on_final_recheck,
        crate::domain::integration::TrustedHostDiagnosticTestControlV1::replace_incarnation_on_final_recheck,
    ] {
        let (
            root,
            _domain,
            mut store,
            _contract_root,
            head,
            _authority_root,
            _request,
            diagnostic,
            _,
        ) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            b"host-fence-turnover",
        );
        schedule(&connection.control());
        reset_protected_diagnostic_envelope_test_observation();
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        assert_eq!(protected_diagnostic_envelope_test_observation(), (1, 1, 0));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    for mutation in [
        ProtectedContinuityDiagnosticAssemblerModeV1::SubstituteAdmission,
        ProtectedContinuityDiagnosticAssemblerModeV1::IgnoreInput,
    ] {
        let (root, _domain, mut store, _contract_root, head, _, _, diagnostic, _) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            b"prepared-envelope-substitution",
        );
        reset_protected_diagnostic_envelope_test_observation();
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store)
                .protected_continuity_diagnostic_reference_envelope_with_assembler_mutation(
                    &mut connection,
                    &mut provider,
                    diagnostic.protected_subject,
                    mutation,
                ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        assert_eq!(protected_diagnostic_envelope_test_observation(), (1, 0, 0));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn protected_continuity_diagnostic_final_recheck_rejects_host_claim_turnover() {
    for dimension in 0..12 {
        let (
            root,
            _domain,
            mut store,
            _contract_root,
            head,
            _authority_root,
            _request,
            diagnostic,
            _,
        ) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            format!("host-claim-turnover-{dimension}").as_bytes(),
        );
        connection
            .control()
            .substitute_claim_dimension_on_final_recheck(dimension);
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    for dimension in 0..15 {
        let (
            root,
            _domain,
            mut store,
            _contract_root,
            head,
            _authority_root,
            _request,
            diagnostic,
            _,
        ) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            format!("operator-identity-turnover-{dimension}").as_bytes(),
        );
        connection
            .control()
            .substitute_operator_identity_dimension_on_final_recheck(dimension);
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn protected_continuity_diagnostic_failed_subject_consumes_host_authentication_event() {
    let (root, _domain, mut store, _contract_root, head, _authority_root, _request, diagnostic, _) =
        seeded_store();
    let (mut first_provider, mut connection) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"failed-subject-consumes-auth-event",
    );
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut first_provider,
            reference("unknown-protected-subject"),
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert!(first_provider.was_consumed());
    let (mut replay_provider, _) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"failed-subject-replay",
    );
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut replay_provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_zero_subject_consumes_host_authentication_event() {
    let (root, _domain, mut store, _contract_root, head, _authority_root, _request, diagnostic, _) =
        seeded_store();
    let (mut first_provider, mut connection) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"zero-subject-consumes-auth-event",
    );
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut first_provider,
            ContinuityReferenceV1::from_digest([0; 32]),
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    let (mut replay_provider, _) =
        diagnostic.test_ports(&root, store.domain(), &store, &head, b"zero-subject-replay");
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut replay_provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_refuses_ambiguous_operator_mapping() {
    let (root, _domain, mut store, _contract_root, head, _authority_root, _request, diagnostic, _) =
        seeded_store_with_diagnostic_mode(
            true,
            ProtectedDiagnosticFixtureModeV1::DuplicateOperatorMapping,
        );
    let (mut provider, mut connection) =
        diagnostic.test_ports(&root, store.domain(), &store, &head, b"ambiguous-operator");
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn session_request_commitment_is_snapshot_identity_not_host_authority() {
    let (root, _domain, mut store, _contract_root, head, _authority_root, _request, diagnostic, _) =
        seeded_store_with_diagnostic_mode(
            true,
            ProtectedDiagnosticFixtureModeV1::ChangedRequestCommitment,
        );
    let (mut provider, mut connection) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"request-commitment-is-not-host-auth",
    );
    assert_eq!(
        AuthorityFacadeV1::new(&mut store)
            .protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            )
            .unwrap()
            .disposition(),
        ProtectedContinuityDiagnosticDispositionV1::CurrentProtectedSnapshot
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_guard_refuses_noncurrent_human_facts() {
    for mode in [
        ProtectedDiagnosticFixtureModeV1::NonHuman,
        ProtectedDiagnosticFixtureModeV1::RevokedBinding,
        ProtectedDiagnosticFixtureModeV1::RevokedSession,
        ProtectedDiagnosticFixtureModeV1::RevokedTrustRoot,
        ProtectedDiagnosticFixtureModeV1::WrongBindingContext,
        ProtectedDiagnosticFixtureModeV1::WrongSessionContext,
        ProtectedDiagnosticFixtureModeV1::WrongSessionBinding,
        ProtectedDiagnosticFixtureModeV1::WrongTrustRoot,
        ProtectedDiagnosticFixtureModeV1::StaleSession,
        ProtectedDiagnosticFixtureModeV1::WrongSessionEpoch,
        ProtectedDiagnosticFixtureModeV1::UnavailableTime,
        ProtectedDiagnosticFixtureModeV1::InvertedTime,
        ProtectedDiagnosticFixtureModeV1::ExpiredBinding,
        ProtectedDiagnosticFixtureModeV1::PrematureBinding,
    ] {
        let (
            root,
            _domain,
            mut store,
            _contract_root,
            before_head,
            _authority_root,
            _request,
            diagnostic,
            _,
        ) = seeded_store_with_diagnostic_mode(true, mode);
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &before_head,
            b"noncurrent-diagnostic",
        );
        let outcome = AuthorityFacadeV1::new(&mut store)
            .protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            );
        assert!(
            matches!(
                outcome,
                Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
            ),
            "mode {mode:?} unexpectedly produced {outcome:?}"
        );
        assert_eq!(store.active_head().unwrap().unwrap(), before_head);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn protected_continuity_diagnostic_selects_one_authority_root_in_a_heterogeneous_generation() {
    let (root, _domain, mut store, _, head, _, _, diagnostic, _) =
        seeded_store_with_diagnostic_mode(
            true,
            ProtectedDiagnosticFixtureModeV1::HeterogeneousRoot,
        );
    let (mut provider, mut connection) =
        diagnostic.test_ports(&root, store.domain(), &store, &head, b"heterogeneous-root");
    assert!(
        AuthorityFacadeV1::new(&mut store)
            .protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            )
            .is_ok()
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_continuity_diagnostic_refuses_missing_duplicate_and_stale_authority_roots() {
    for mode in [
        ProtectedDiagnosticFixtureModeV1::MissingAuthorityRoot,
        ProtectedDiagnosticFixtureModeV1::DuplicateAuthorityRoot,
        ProtectedDiagnosticFixtureModeV1::StaleAuthorityRoot,
        ProtectedDiagnosticFixtureModeV1::ForeignAuthorityRoot,
    ] {
        let (root, _domain, mut store, _, head, _, _, diagnostic, _) =
            seeded_store_with_diagnostic_mode(true, mode);
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            b"invalid-authority-root",
        );
        let outcome = AuthorityFacadeV1::new(&mut store)
            .protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            );
        assert!(
            matches!(
                outcome,
                Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
            ),
            "mode {mode:?} unexpectedly produced {outcome:?}"
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn protected_continuity_diagnostic_refuses_every_substituted_store_anchor_dimension() {
    for mutation in ProtectedDiagnosticTestAnchorMutationV1::ALL {
        let (root, _domain, mut store, _, head, _, _, diagnostic, _) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            format!("anchor-{mutation:?}").as_bytes(),
        );
        provider.substitute_anchor_dimension(mutation);
        assert!(
            matches!(
                AuthorityFacadeV1::new(&mut store)
                    .protected_continuity_diagnostic_reference_envelope(
                        &mut connection,
                        &mut provider,
                        diagnostic.protected_subject,
                    ),
                Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
            ),
            "mutation {mutation:?} unexpectedly passed"
        );
        assert!(provider.was_consumed());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    let provider_dimensions = [
        ProtectedDiagnosticTestAnchorMutationV1::StoreInstance,
        ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierIdentity,
        ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierToken,
        ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierRevision,
        ProtectedDiagnosticTestAnchorMutationV1::ActivationAttempt,
        ProtectedDiagnosticTestAnchorMutationV1::ActivationDestinationSeal,
        ProtectedDiagnosticTestAnchorMutationV1::ActivationRestoreIncarnation,
        ProtectedDiagnosticTestAnchorMutationV1::ProviderCurrentnessRevision,
    ];
    for mutation in provider_dimensions {
        let (root, _domain, mut store, _, head, _, _, diagnostic, _) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            format!("post-bind-{mutation:?}").as_bytes(),
        );
        provider.substitute_on_final_recheck(mutation);
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        assert!(provider.was_consumed());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    for mutation in provider_dimensions {
        let (root, _domain, mut store, _, head, _, _, diagnostic, _) = seeded_store();
        let (mut provider, mut connection) = diagnostic.test_ports(
            &root,
            store.domain(),
            &store,
            &head,
            format!("post-bind-aba-{mutation:?}").as_bytes(),
        );
        provider.aba_on_final_recheck(mutation);
        assert!(matches!(
            AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
                &mut connection,
                &mut provider,
                diagnostic.protected_subject,
            ),
            Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
        ));
        assert!(provider.was_consumed());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    let (root, _domain, mut store, _, head, _, _, diagnostic, _) = seeded_store();
    let (mut provider, mut connection) = diagnostic.test_ports(
        &root,
        store.domain(),
        &store,
        &head,
        b"post-bind-provider-unavailable",
    );
    provider.unavailable_on_final_recheck();
    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).protected_continuity_diagnostic_reference_envelope(
            &mut connection,
            &mut provider,
            diagnostic.protected_subject,
        ),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert!(provider.was_consumed());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_same_key_has_one_commit_and_one_zero_write_replay() {
    let (root, domain, store, contract_root, head, authority_root, request, _, _) = seeded_store();
    let publication = plan(request, contract_root, &head, authority_root);
    drop(store);
    let barrier = Arc::new(Barrier::new(3));
    let workers = (0..2)
        .map(|_| {
            let root = root.clone();
            let domain = domain.clone();
            let barrier = Arc::clone(&barrier);
            let publication = publication.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(root, domain).unwrap();
                barrier.wait();
                AuthorityFacadeV1::new(&mut store)
                    .issue_bootstrap_mandate(publication)
                    .unwrap()
                    .kind()
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let kinds = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == AuthorityPublicationKindV1::Committed)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == AuthorityPublicationKindV1::Replayed)
            .count(),
        1
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reopened_store_reauthorizes_different_key_and_converges_without_second_binding() {
    let (root, domain, mut store, contract_root, head, authority_root, request, _, _) =
        seeded_store();
    let first = AuthorityFacadeV1::new(&mut store)
        .issue_bootstrap_mandate(plan(request.clone(), contract_root, &head, authority_root))
        .unwrap();
    let first_produced = produced_ids(first.result());
    assert_eq!(first_produced.len(), 2);
    assert_eq!(
        schema_objects(
            &active_objects(&store),
            AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
        )
        .unwrap()
        .len(),
        1
    );
    drop(store);

    let mut reopened = StoreV1::open(&root, domain).unwrap();
    let current_head = reopened.active_head().unwrap().unwrap();
    let current_generation = reopened.publication_generation(current_head.id()).unwrap();
    let [current_root] = current_generation.roots() else {
        panic!("authority generation must have one canonical root");
    };
    let second = AuthorityFacadeV1::new(&mut reopened)
        .issue_bootstrap_mandate(plan(
            duplicate_request(&request),
            contract_root,
            &current_head,
            *current_root,
        ))
        .unwrap();
    let second_produced = produced_ids(second.result());
    assert_eq!(second.kind(), AuthorityPublicationKindV1::Committed);
    assert_eq!(second.head().generation_ordinal(), 4);
    assert_ne!(second.logical_result_id(), first.logical_result_id());
    assert_eq!(second_produced, vec![first_produced[0]]);
    assert_eq!(
        schema_objects(
            &active_objects(&reopened),
            AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
        )
        .unwrap()
        .len(),
        1
    );
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mandate_convergence_refuses_missing_or_multiple_issuance_bindings() {
    let (root, _domain, mut store, contract_root, head, authority_root, request, _, _) =
        seeded_store();
    AuthorityFacadeV1::new(&mut store)
        .issue_bootstrap_mandate(plan(request, contract_root, &head, authority_root))
        .unwrap();
    let objects = active_objects(&store);
    let mandate = one_schema_object(&objects, AuthoritySchemaV1::AuthorityMandate).unwrap();
    let binding =
        one_schema_object(&objects, AuthoritySchemaV1::BootstrapMandateIssuanceBinding).unwrap();
    let binding_value = object_value_bytes(&binding).unwrap();
    assert!(!validate_mandate_issuance_cardinality(&objects, &mandate, &binding_value).unwrap());

    let without_binding = objects
        .iter()
        .filter(|object| object.id() != binding.id())
        .cloned()
        .collect::<Vec<_>>();
    assert!(matches!(
        validate_mandate_issuance_cardinality(&without_binding, &mandate, &binding_value),
        Err(AuthorityPublicationError::StoredMandateBindingMismatch)
    ));

    let duplicate = StoreObjectV1::new(
        binding.schema_id(),
        binding.value().clone(),
        vec![mandate.id()],
    )
    .unwrap();
    let mut with_duplicate = objects;
    with_duplicate.push(duplicate);
    assert!(matches!(
        validate_mandate_issuance_cardinality(&with_duplicate, &mandate, &binding_value),
        Err(AuthorityPublicationError::StoredMandateBindingMismatch)
    ));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn store_loaded_post_cut_refuses_a_mismatched_coupled_commitment() {
    let (root, domain, mut store, contract_root, head_two, authority_root, request, _, _) =
        seeded_store();
    let first = AuthorityFacadeV1::new(&mut store)
        .issue_bootstrap_mandate(plan(
            request.clone(),
            contract_root,
            &head_two,
            authority_root,
        ))
        .unwrap();
    let generation_three = store.publication_generation(first.head().id()).unwrap();
    let [post_cut_id] = generation_three.roots() else {
        panic!("authority generation must have one post-cut root");
    };
    let post_cut = store.read_object(*post_cut_id).unwrap();
    let mut mutant_value = post_cut.value().clone();
    let CborValue::Array(fields) = &mut mutant_value else {
        panic!("post-cut carrier must be an array");
    };
    fields[7] = bytes(&digest("mismatched-phase-owned-mutation"));
    let mutant = StoreObjectV1::new(
        post_cut.schema_id(),
        mutant_value,
        post_cut.references().to_vec(),
    )
    .unwrap();

    let objects = active_objects(&store);
    let result_object = one_schema_object(&objects, AuthoritySchemaV1::ActionResult).unwrap();
    let (mutant_root, mut mutant_store, mutant_head_three) =
        clone_store_through_mutant_generation_three(
            &root,
            &store,
            &domain,
            contract_root,
            &head_two,
            &generation_three,
            &mutant,
            result_object.id(),
            objects,
        );

    assert!(matches!(
        AuthorityFacadeV1::new(&mut mutant_store).issue_bootstrap_mandate(plan(
            duplicate_request(&request),
            contract_root,
            &mutant_head_three,
            mutant.id(),
        )),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert_eq!(
        mutant_store.active_head().unwrap().unwrap().id(),
        mutant_head_three.id()
    );
    drop(mutant_store);
    drop(store);
    fs::remove_dir_all(mutant_root).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn store_loaded_post_cut_refuses_a_nonexact_action_result_receipt_count() {
    let (root, domain, mut store, contract_root, head_two, authority_root, request, _, _) =
        seeded_store();
    let first = AuthorityFacadeV1::new(&mut store)
        .issue_bootstrap_mandate(plan(
            request.clone(),
            contract_root,
            &head_two,
            authority_root,
        ))
        .unwrap();
    let generation_three = store.publication_generation(first.head().id()).unwrap();
    let [post_cut_id] = generation_three.roots() else {
        panic!("authority generation must have one post-cut root");
    };
    let post_cut = store.read_object(*post_cut_id).unwrap();
    let mut objects = active_objects(&store);
    let result = one_schema_object(&objects, AuthoritySchemaV1::ActionResult).unwrap();
    let mut mutant_result_value = result.value().clone();
    let CborValue::Array(result_fields) = &mut mutant_result_value else {
        panic!("Action Result carrier must be an array");
    };
    result_fields[3] = CborValue::Unsigned(0);
    let mutant_result = StoreObjectV1::new(
        result.schema_id(),
        mutant_result_value,
        result.references().to_vec(),
    )
    .unwrap();
    let mut mutant_post_cut_references = post_cut
        .references()
        .iter()
        .map(|reference| {
            if *reference == result.id() {
                mutant_result.id()
            } else {
                *reference
            }
        })
        .collect::<Vec<_>>();
    mutant_post_cut_references.sort_unstable();
    let mutant_post_cut = StoreObjectV1::new(
        post_cut.schema_id(),
        post_cut.value().clone(),
        mutant_post_cut_references,
    )
    .unwrap();
    objects.push(mutant_result.clone());
    let (mutant_root, mut mutant_store, mutant_head_three) =
        clone_store_through_mutant_generation_three(
            &root,
            &store,
            &domain,
            contract_root,
            &head_two,
            &generation_three,
            &mutant_post_cut,
            mutant_result.id(),
            objects,
        );

    assert!(matches!(
        AuthorityFacadeV1::new(&mut mutant_store).issue_bootstrap_mandate(plan(
            duplicate_request(&request),
            contract_root,
            &mutant_head_three,
            mutant_post_cut.id(),
        )),
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    ));
    assert_eq!(
        mutant_store.active_head().unwrap().unwrap().id(),
        mutant_head_three.id()
    );
    drop(mutant_store);
    drop(store);
    fs::remove_dir_all(mutant_root).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_removal_guard_v3_rechecks_every_live_authority_binding() {
    let dimensions = [
        "realm",
        "store_generation",
        "store_head",
        "authority_snapshot",
        "authority_epoch",
        "trust_root",
        "authority_fence",
        "state_token",
        "authority_currentness",
        "revocation_revision",
        "revocation_state",
    ];
    for (index, dimension) in dimensions.into_iter().enumerate() {
        let (root, _, mut store, _, _, _, _, currentness) = seeded_installation_removal_store();
        let consumer = removal_consumer_binding(40);
        let mut stale = currentness;
        match index {
            0 => stale.realm[0] ^= 1,
            1 => stale.store_generation[0] ^= 1,
            2 => stale.store_head[0] ^= 1,
            3 => stale.authority_snapshot[0] ^= 1,
            4 => stale.authority_epoch += 1,
            5 => stale.trust_root[0] ^= 1,
            6 => stale.authority_fence[0] ^= 1,
            7 => stale.state_token[0] ^= 1,
            8 => stale.authority_currentness[0] ^= 1,
            9 => stale.revocation_revision += 1,
            10 => stale.revocation_state[0] ^= 1,
            _ => unreachable!(),
        }
        let binding = LegacyRemovalGuardBindingV3::test_from_currentness(stale, consumer);
        let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
        let callback_invocations = Cell::new(0_u8);
        assert_eq!(
            guard.consume_for_test(consumer, || {
                callback_invocations.set(callback_invocations.get() + 1);
                Ok::<_, ()>(())
            }),
            Err(LegacyRemovalGuardAdmissionErrorV3::AuthorityCurrentnessDrift),
            "live Authority drift must refuse {dimension}"
        );
        assert_eq!(callback_invocations.get(), 0, "{dimension}");
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn legacy_removal_guard_v3_recomputes_revocation_state_before_linearization() {
    let (root, _, mut store, _, _, _, _, currentness) = seeded_installation_removal_store();
    let consumer = removal_consumer_binding(60);
    let mut stale = currentness;
    stale.revocation_state[31] ^= 1;
    let binding = LegacyRemovalGuardBindingV3::test_from_currentness(stale, consumer);
    let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
    let callback_invocations = Cell::new(0_u8);

    assert_eq!(
        guard.consume_for_test(consumer, || {
            callback_invocations.set(callback_invocations.get() + 1);
            Ok::<_, ()>(())
        }),
        Err(LegacyRemovalGuardAdmissionErrorV3::AuthorityCurrentnessDrift)
    );
    assert_eq!(callback_invocations.get(), 0);
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_removal_guard_v3_rederives_the_unique_invocation_before_linearization() {
    let (root, _, mut store, _, _, _, _, currentness) = seeded_installation_removal_store();
    let consumer = removal_consumer_binding(70);
    let mut binding = LegacyRemovalGuardBindingV3::test_from_currentness(currentness, consumer);
    binding.corrupt_invocation_for_test();
    let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
    let callback_invocations = Cell::new(0_u8);

    assert_eq!(
        guard.consume_for_test(consumer, || {
            callback_invocations.set(callback_invocations.get() + 1);
            Ok::<_, ()>(())
        }),
        Err(LegacyRemovalGuardAdmissionErrorV3::InvocationUnavailable)
    );
    assert_eq!(callback_invocations.get(), 0);
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_removal_guard_v3_rechecks_every_consumer_closure_binding() {
    let dimensions = [
        "consumer_closure",
        "release",
        "source_case_manifest",
        "sighting_manifest",
        "classification_manifest",
        "overlap_manifest",
        "loss_manifest",
        "quarantine_manifest",
        "foundation_closure",
        "rollback_assessment",
        "quarantine_epoch",
        "replacement_activation",
        "reader_closure",
        "hold_closure",
        "rollback_rehearsal",
        "deletion_plan",
        "expected_old_state",
    ];
    for (index, dimension) in dimensions.into_iter().enumerate() {
        let (root, _, mut store, _, _, _, _, currentness) = seeded_installation_removal_store();
        let consumer = removal_consumer_binding(80);
        let mut substituted = consumer;
        match index {
            0 => substituted.consumer_closure[0] ^= 1,
            1 => substituted.release[0] ^= 1,
            2 => substituted.source_case_manifest[0] ^= 1,
            3 => substituted.sighting_manifest[0] ^= 1,
            4 => substituted.classification_manifest[0] ^= 1,
            5 => substituted.overlap_manifest[0] ^= 1,
            6 => substituted.loss_manifest[0] ^= 1,
            7 => substituted.quarantine_manifest[0] ^= 1,
            8 => substituted.foundation_closure[0] ^= 1,
            9 => substituted.rollback_assessment[0] ^= 1,
            10 => substituted.quarantine_epoch[0] ^= 1,
            11 => substituted.replacement_activation[0] ^= 1,
            12 => substituted.reader_closure[0] ^= 1,
            13 => substituted.hold_closure[0] ^= 1,
            14 => substituted.rollback_rehearsal[0] ^= 1,
            15 => substituted.deletion_plan[0] ^= 1,
            16 => substituted.expected_old_state[0] ^= 1,
            _ => unreachable!(),
        }
        let binding = LegacyRemovalGuardBindingV3::test_from_currentness(currentness, consumer);
        let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
        let callback_invocations = Cell::new(0_u8);
        assert_eq!(
            guard.consume_for_test(substituted, || {
                callback_invocations.set(callback_invocations.get() + 1);
                Ok::<_, ()>(())
            }),
            Err(LegacyRemovalGuardAdmissionErrorV3::OwnerFactsMismatch),
            "consumer substitution must refuse {dimension}"
        );
        assert_eq!(callback_invocations.get(), 0, "{dimension}");
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn legacy_removal_guard_v3_invokes_linearization_once_and_preserves_callback_error() {
    let (root, _, mut store, _, _, _, _, currentness) = seeded_installation_removal_store();
    let consumer = removal_consumer_binding(100);
    let binding = LegacyRemovalGuardBindingV3::test_from_currentness(currentness, consumer);
    let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
    let callback_invocations = Cell::new(0_u8);
    assert_eq!(
        guard.consume_for_test(consumer, || {
            callback_invocations.set(callback_invocations.get() + 1);
            Ok::<_, &'static str>(91_u8)
        }),
        Ok(Ok(91))
    );
    assert_eq!(callback_invocations.get(), 1);

    let currentness = store
        .with_serialized_active_view(observe_legacy_removal_guard_currentness_v3)
        .unwrap();
    let binding = LegacyRemovalGuardBindingV3::test_from_currentness(currentness, consumer);
    let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
    assert_eq!(
        guard.consume_for_test(consumer, || Err::<(), _>("expected-old refused")),
        Ok(Err("expected-old refused"))
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_removal_guard_v3_refuses_a_reopened_store_publication_after_mint() {
    let (root, domain, mut store, generation, head, authority_root, contract_root, currentness) =
        seeded_installation_removal_store();
    let consumer = removal_consumer_binding(120);
    let binding = LegacyRemovalGuardBindingV3::test_from_currentness(currentness, consumer);
    let guard = LegacyRemovalGuardV3::mint(binding, &mut store);

    let mut racing_store = StoreV1::open(&root, domain.clone()).unwrap();
    let successor = StoreGenerationV1::new(
        domain,
        2,
        Some(generation.id()),
        contract_root,
        StoreCompatibilityV1::stage0_successor().unwrap(),
        vec![authority_root],
    )
    .unwrap();
    racing_store
        .publish_generation(&successor, Some(head.id()))
        .unwrap();
    let callback_invocations = Cell::new(0_u8);
    assert_eq!(
        guard.consume_for_test(consumer, || {
            callback_invocations.set(callback_invocations.get() + 1);
            Ok::<_, ()>(())
        }),
        Err(LegacyRemovalGuardAdmissionErrorV3::AuthorityCurrentnessDrift)
    );
    assert_eq!(callback_invocations.get(), 0);
    drop(racing_store);
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_removal_guard_v3_holds_the_store_write_fence_through_linearization() {
    let (root, _, mut store, _, _, _, _, currentness) = seeded_installation_removal_store();
    let consumer = removal_consumer_binding(140);
    let binding = LegacyRemovalGuardBindingV3::test_from_currentness(currentness, consumer);
    let guard = LegacyRemovalGuardV3::mint(binding, &mut store);
    let database_path = root.join("store.sqlite3");

    assert_eq!(
        guard.consume_for_test(consumer, || {
            let racing_connection = Connection::open(&database_path).unwrap();
            racing_connection
                .busy_timeout(std::time::Duration::ZERO)
                .unwrap();
            let error = racing_connection
                .execute_batch("BEGIN IMMEDIATE")
                .expect_err("a second writer must not cross the retained fence");
            assert!(matches!(
                error,
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if matches!(
                        failure.code,
                        rusqlite::ErrorCode::DatabaseBusy
                            | rusqlite::ErrorCode::DatabaseLocked
                    )
            ));
            Ok::<_, ()>(77_u8)
        }),
        Ok(Ok(77))
    );

    let released_connection = Connection::open(database_path).unwrap();
    released_connection
        .execute_batch("BEGIN IMMEDIATE")
        .unwrap();
    released_connection.execute_batch("ROLLBACK").unwrap();
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_removal_guard_v3_has_only_the_private_process_local_capability_shape() {
    let source = include_str!("legacy_removal_guard.rs");
    let guard = source
        .split("pub(in crate::domain) struct LegacyRemovalGuardV3")
        .nth(1)
        .expect("guard definition must remain present");

    assert!(guard.contains("_store: &'cut mut StoreV1"));
    assert!(guard.contains("_not_send_or_sync: PhantomData<Rc<()>>"));
    assert!(guard.contains("pub(super) const fn mint("));
    assert!(guard.contains("store: &'cut mut StoreV1"));
    assert!(guard.contains("pub(in crate::domain) fn consume_for("));
    assert!(guard.contains("pub(in crate::domain) fn consume_with_linearization<"));
    assert!(guard.contains("linearize_expected_old: impl FnOnce() -> Result<T, E>"));
    assert!(guard.contains("with_serialized_active_view"));
    assert!(guard.contains("LinearizationRequired"));
    assert!(guard.contains("Result<(), LegacyRemovalGuardAdmissionErrorV3>"));
    for closure_binding in [
        "closure.identity()",
        "closure.release_id()",
        "closure.source_case_manifest_id()",
        "closure.sighting_manifest_id()",
        "closure.classification_manifest_id()",
        "closure.overlap_manifest_id()",
        "closure.loss_manifest_id()",
        "closure.quarantine_manifest_id()",
        "closure.foundation_closure_id()",
        "closure.rollback_assessment_id()",
        "closure.legacy_quarantine_epoch_id()",
        "closure.replacement_activation_id()",
        "closure.physical_pruning_reader_zero_id()",
        "closure.physical_pruning_hold_zero_id()",
        "closure.rollback_rehearsal_id()",
        "closure.deletion_plan_id()",
    ] {
        assert!(
            source.contains(closure_binding),
            "missing consumption recheck {closure_binding}"
        );
    }
    assert!(!guard.contains("pub fn new("));
    assert!(!guard.contains("pub(crate) fn"));
    assert_eq!(guard.matches("pub(in crate::domain) fn").count(), 2);
    assert!(!guard.contains("impl Clone"));
    assert!(!guard.contains("impl Debug"));
    assert!(!guard.contains("Serialize"));
    assert!(!guard.contains("Deserialize"));
    assert!(!guard.contains("canonical_bytes"));
    assert!(!guard.contains("schema_value"));
    assert!(!guard.contains("<'static>"));
    assert!(!guard.contains("into_"));
    assert!(!guard.contains("-> [u8; 32]"));
    assert!(!guard.contains("-> MigrationDigestV1"));
}

#[test]
fn legacy_removal_guard_v3_admission_requires_exact_owner_issued_facts() {
    let source = include_str!("facade.rs");
    let admission = source
        .split("pub(in crate::domain) fn admit_legacy_physical_pruning<")
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "reason = \"the V2 pruning admission remains historical while V3 is the sole current path\"",
            )
                .next()
        })
        .expect("named pruning admission must remain present");

    for owner_fact in [
        "&LegacySourceCaseManifestV3",
        "&Stage12SightingManifestV2",
        "&MigrationClassificationManifestV3",
        "&DeclaredOverlapManifestV2",
        "&UnavailablePreexistingLossManifestV4",
        "&SealedQuarantineManifestV3",
        "&FoundationLegacyQuarantineClosureV2",
        "&LegacyRollbackAssessmentV4",
        "&LegacyQuarantineEpochV4",
        "&Stage12ReplacementActivationV3",
        "&Stage12ConsumerReaderHoldClosureV3",
        "&Stage12RollbackRehearsalV3",
        "&InstallationLegacyDeletionPlanV3",
    ] {
        assert!(
            admission.contains(owner_fact),
            "missing typed owner fact {owner_fact}"
        );
    }
    assert!(!admission.contains("usize"));
    assert!(!admission.contains("historical"));
    assert!(!admission.contains("count:"));
    assert!(!admission.contains("digest:"));
    assert!(!admission.contains("Vec<"));
    assert!(!admission.contains("String"));
}

#[test]
fn legacy_removal_guard_v3_validation_is_atomic_and_effect_inert() {
    let source = include_str!("facade.rs");
    let validation = source
        .split("fn validate_legacy_physical_pruning_admission_v3")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn validate_legacy_physical_pruning_admission_v2")
                .next()
        })
        .expect("guard validation must remain present");

    let admission = source
        .split("pub(in crate::domain) fn admit_legacy_physical_pruning<")
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "reason = \"the V2 pruning admission remains historical while V3 is the sole current path\"",
            )
                .next()
        })
        .expect("named pruning admission must remain present");
    assert!(admission.contains("with_serialized_active_view"));
    for required_recheck in [
        "select_current_authority_root",
        "load_current_authority",
        "StoreRoleV1::Installation",
        "current_generation.id()",
        "current_head.id()",
        "context.authority_epoch()",
        "context.trust_root_revision()",
        "current.state.carrier_fence()",
        "current.state.state_token()",
        "current.state.carrier_currentness()",
        "epoch.revocation_revision()",
        ".revocations()",
        "let expected_old_state_id = deletion_plan.expected_old_state_id()",
        "consumer_reader_hold.expected_old_state_id() != expected_old_state_id",
        "expected_old_state_id.as_bytes()",
    ] {
        assert!(
            validation.contains(required_recheck),
            "missing live Authority recheck {required_recheck}"
        );
    }
    for forbidden_effect in [
        "remove_file",
        "remove_dir",
        "delete(",
        "rename",
        "write(",
        "publish_generation",
        "commit(",
    ] {
        assert!(
            !validation.contains(forbidden_effect),
            "guard admission must remain effect-inert: {forbidden_effect}"
        );
    }
}

#[test]
fn legacy_removal_guard_v3_is_not_a_wire_resource_action_or_scope_atom() {
    let forbidden_surfaces = [
        include_str!("action_basis.rs"),
        include_str!("context.rs"),
        include_str!("grant.rs"),
        include_str!("../capability/literals.rs"),
    ];
    assert!(
        forbidden_surfaces
            .iter()
            .all(|source| !source.contains("LegacyRemovalGuardV3"))
    );
}
