use maestro::domain::vnext::authority::{
    ActionRequestIdV1, AuthorityContextIdV1, AuthorityContextV1, AuthorityContinuityManifestV1,
    AuthorityEvaluationErrorV1, AuthorityEvaluatorV1, AuthorityRevocationSetV1,
    AuthoritySnapshotV1, AuthorityUseConstraintV1, BootstrapAuthoritySnapshotErrorV1,
    BootstrapAuthoritySnapshotV1, BootstrapContinuityTransitionProofV1, BootstrapG0PathV1,
    BootstrapInteractionSubjectV1, BootstrapMandateInteractionObservationJoinV1,
    BootstrapMandatePresentationObservationV1, BootstrapMandateResponseObservationV1,
    BootstrapMandateTargetV1, BootstrapResponseDispositionV1, CapacityRootIdV1,
    ConsentSlotEvaluationFactsV1, GenesisGrantIdV1, GrantDefinitionV1, GrantIdV1, GrantScopeV1,
    GuardAdmissionKindV1, HalfOpenValidityV1, IdempotencyKeyIdV1, IssueBootstrapMandateInputV1,
    IssueBootstrapMandateRequestV1, PrincipalBindingIdV1, PrincipalBindingV1, PrincipalIdV1,
    RevocationSetV1, ScopeAtomV1, SessionIdV1, SessionV1, StateTokenIdV1, TargetActionEffectKindV1,
    TargetActionOwnerV1, TargetActionProjectionV1, TargetActionProtocolV1, TargetExpectedHeadsV1,
    TransitionGuardKindV1, TrustedTimeV1, issue_bootstrap_mandate,
};
use maestro::foundation::core::deterministic_cbor::{self, CborValue};

fn id<T>(seed: &str) -> T
where
    T: TryFromSeed,
{
    T::from_seed(seed)
}

trait TryFromSeed {
    fn from_seed(seed: &str) -> Self;
}

macro_rules! seed_id {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl TryFromSeed for $ty {
                fn from_seed(seed: &str) -> Self {
                    <$ty>::derive(seed).unwrap()
                }
            }
        )+
    };
}

seed_id!(
    ActionRequestIdV1,
    AuthorityContextIdV1,
    CapacityRootIdV1,
    GenesisGrantIdV1,
    GrantIdV1,
    IdempotencyKeyIdV1,
    PrincipalBindingIdV1,
    PrincipalIdV1,
    SessionIdV1,
    StateTokenIdV1,
);

struct Fixture {
    request: IssueBootstrapMandateRequestV1,
    facts: BootstrapAuthoritySnapshotV1,
}

fn array_mut(value: &mut CborValue) -> &mut Vec<CborValue> {
    match value {
        CborValue::Array(values) => values,
        _ => panic!("expected canonical array"),
    }
}

fn assert_unavailable(fixture: Fixture, mutate: impl FnOnce(&mut CborValue)) {
    let mut value = deterministic_cbor::decode(&fixture.facts.canonical_bytes().unwrap()).unwrap();
    mutate(&mut value);
    let Ok(facts) = BootstrapAuthoritySnapshotV1::from_canonical_bytes(
        &deterministic_cbor::encode(&value).unwrap(),
    ) else {
        return;
    };
    assert_eq!(
        AuthorityEvaluatorV1::evaluate_bootstrap_mandate(fixture.request, &facts),
        Err(AuthorityEvaluationErrorV1::Unavailable)
    );
}

fn fixture() -> Fixture {
    let context_id = id("repository-context");
    let actor_principal_id = id("actor-principal");
    let actor_binding_id = id("actor-binding");
    let responder_principal_id = id("responder-principal");
    let responder_binding_id = id("responder-binding");
    let target_head = id("target-head");
    let validity = HalfOpenValidityV1::new(10, 1_000).unwrap();
    let context = AuthorityContextV1::repository(context_id, "installation", 7, 11, 3).unwrap();
    let snapshot = AuthoritySnapshotV1::new(
        context_id,
        7,
        11,
        3,
        5,
        TrustedTimeV1::verified(100, 102).unwrap(),
    );
    let expected_heads = TargetExpectedHeadsV1::new(context_id, 7, 11, 3, 5, target_head).unwrap();
    let target = TargetActionProjectionV1::new(
        BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
        "recovery-selection",
        5,
        TargetActionOwnerV1::Authority,
        TargetActionProtocolV1::RecoveryCommitmentSelection,
        TargetActionEffectKindV1::Rotate,
        "sha256:effect-closure",
        expected_heads,
        validity,
    )
    .unwrap();
    let target_commitment = target.target_action_commitment().unwrap();
    let consent_slot_facts =
        ConsentSlotEvaluationFactsV1::derive_for_target(&target, validity).unwrap();
    let consent_slot = consent_slot_facts.binding().clone();
    let request_commitment = target_commitment.render();
    let actor_binding = PrincipalBindingV1::new(
        actor_binding_id,
        actor_principal_id,
        context_id,
        3,
        4,
        validity,
        false,
    )
    .unwrap();
    let actor_session = SessionV1::new(
        id("actor-session"),
        actor_binding_id,
        context_id,
        7,
        11,
        &request_commitment,
        validity,
    )
    .unwrap();
    let responder_binding = PrincipalBindingV1::new(
        responder_binding_id,
        responder_principal_id,
        context_id,
        3,
        9,
        validity,
        true,
    )
    .unwrap();
    let responder_session = SessionV1::new(
        id("responder-session"),
        responder_binding_id,
        context_id,
        7,
        11,
        &request_commitment,
        validity,
    )
    .unwrap();
    let interaction_subject = BootstrapInteractionSubjectV1::new(
        context_id,
        id("interaction-plan"),
        id("interaction-attempt"),
        responder_binding_id,
        9,
        target_commitment,
        consent_slot.clone(),
        id("option-map"),
        id("affirmative-option"),
    );
    let presentation = BootstrapMandatePresentationObservationV1::new(
        interaction_subject.clone(),
        id("carrier"),
        id("procedure"),
    )
    .unwrap();
    let response = BootstrapMandateResponseObservationV1::new(
        interaction_subject,
        presentation.id(),
        BootstrapResponseDispositionV1::Affirmative,
        id("affirmative-option"),
    )
    .unwrap();
    let carrier_procedure_ref = id("procedure");
    let interaction_join = BootstrapMandateInteractionObservationJoinV1::new(
        &presentation,
        &response,
        responder_session.id(),
        carrier_procedure_ref,
    )
    .unwrap();
    let request = IssueBootstrapMandateRequestV1::try_from(IssueBootstrapMandateInputV1 {
        request_id: id("request"),
        idempotency_key: id("key"),
        context_id,
        actor_binding_id,
        actor_session_id: actor_session.id(),
        responder_binding_id,
        presentation_observation_id: presentation.id(),
        response_observation_id: response.id(),
        target: BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
        target_subject: "recovery-selection".to_owned(),
        target_revision: 5,
        consent_slot: consent_slot.clone(),
        supplied_mandates: Vec::new(),
    })
    .unwrap();
    let issue_atom = ScopeAtomV1::new(
        "IssueBootstrapMandate",
        &request_commitment,
        AuthorityEvaluatorV1::ISSUE_BOOTSTRAP_MANDATE_PROTOCOL_REVISION,
    )
    .unwrap();
    let grant = GrantDefinitionV1 {
        id: id("genesis-grant-value"),
        context_id,
        grantee_principal_id: actor_principal_id,
        parent_grant_id: None,
        delegation_id: None,
        terminal_scope: GrantScopeV1::new(vec![issue_atom]).unwrap(),
        delegable_scope: GrantScopeV1::new(Vec::new()).unwrap(),
        validity,
        delegation_depth_remaining: 0,
        authority_use_constraint: AuthorityUseConstraintV1::NoLocalBoundedRoot,
    }
    .validate()
    .unwrap();
    let genesis_grant_id = GenesisGrantIdV1::derive(&grant.id().render()).unwrap();
    let g0_path = BootstrapG0PathV1::new(
        genesis_grant_id,
        grant,
        7,
        11,
        3,
        true,
        Vec::<CapacityRootIdV1>::new(),
    )
    .unwrap();
    let continuity = BootstrapContinuityTransitionProofV1::new(
        context_id,
        7,
        11,
        3,
        AuthorityContinuityManifestV1::repository().unwrap().id(),
        GuardAdmissionKindV1::Established(
            TransitionGuardKindV1::RepositoryFloorOrTrustRootRotation,
        ),
        id("continuity-state"),
        validity,
    );
    let facts = BootstrapAuthoritySnapshotV1::new(
        context,
        snapshot,
        actor_binding,
        actor_session,
        responder_binding,
        responder_session,
        vec![g0_path],
        AuthorityRevocationSetV1::new(context_id, RevocationSetV1::empty()),
        Some(interaction_join),
        carrier_procedure_ref,
        target,
        target_head,
        consent_slot_facts,
        continuity,
    )
    .unwrap();
    Fixture { request, facts }
}

#[test]
fn exact_current_g0_and_affirmative_join_produce_one_sealed_mandate_evaluation() {
    let fixture = fixture();
    let evaluation =
        AuthorityEvaluatorV1::evaluate_bootstrap_mandate(fixture.request, &fixture.facts).unwrap();
    let issuance = issue_bootstrap_mandate(evaluation).unwrap();

    assert_eq!(issuance.mandate.maximum_uses(), 1);
    assert_eq!(issuance.mandate.delegation_depth(), 0);
    assert_eq!(issuance.mandate.responder_assurance_revision(), 9);
    assert_eq!(
        issuance.mandate.validity(),
        HalfOpenValidityV1::new(100, 402).unwrap()
    );
}

#[test]
fn absent_or_fabricated_interaction_observation_is_unavailable() {
    assert_unavailable(fixture(), |value| {
        array_mut(value)[9] = CborValue::Array(Vec::new());
    });

    assert_unavailable(fixture(), |value| {
        let join = &mut array_mut(&mut array_mut(value)[9])[1];
        array_mut(join)[5] = CborValue::Bytes(
            maestro::domain::vnext::authority::ObservationIdV1::derive("fabricated-presentation")
                .unwrap()
                .as_bytes()
                .to_vec(),
        );
    });
}

#[test]
fn stale_generation_epoch_and_changed_authentication_are_unavailable() {
    assert_unavailable(fixture(), |value| {
        let snapshot = &mut array_mut(value)[2];
        array_mut(snapshot)[2] = CborValue::Unsigned(8);
    });
    assert_unavailable(fixture(), |value| {
        let snapshot = &mut array_mut(value)[2];
        array_mut(snapshot)[3] = CborValue::Unsigned(12);
    });
    assert_unavailable(fixture(), |value| {
        let join = &mut array_mut(&mut array_mut(value)[9])[1];
        array_mut(join)[4] = CborValue::Bytes(
            SessionIdV1::derive("changed-responder-authentication")
                .unwrap()
                .as_bytes()
                .to_vec(),
        );
    });
    assert_unavailable(fixture(), |value| {
        array_mut(value)[10] = CborValue::Bytes(
            StateTokenIdV1::derive("changed-carrier-procedure")
                .unwrap()
                .as_bytes()
                .to_vec(),
        );
    });
}

#[test]
fn revoked_binding_untrusted_time_and_more_than_one_g0_path_are_unavailable() {
    let revoked_binding_id = fixture().facts.actor_binding().id();
    assert_unavailable(fixture(), |value| {
        let revocations = &mut array_mut(value)[8];
        array_mut(revocations)[2] = CborValue::Array(vec![CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(revoked_binding_id.as_bytes().to_vec()),
        ])]);
    });
    assert_unavailable(fixture(), |value| {
        let snapshot = &mut array_mut(value)[2];
        array_mut(snapshot)[6] = CborValue::Array(vec![CborValue::Unsigned(2)]);
    });
    assert_unavailable(fixture(), |value| {
        let paths = &mut array_mut(value)[7];
        let second = array_mut(paths)[0].clone();
        array_mut(paths).push(second);
    });
}

#[test]
fn every_target_commitment_dimension_and_current_head_are_bound() {
    assert_unavailable(fixture(), |value| {
        let target = &mut array_mut(value)[11];
        array_mut(target)[1] =
            CborValue::Unsigned(BootstrapMandateTargetV1::EnrollRecoveryCommitmentSelection as u64);
    });
    assert_unavailable(fixture(), |value| {
        let target = &mut array_mut(value)[11];
        array_mut(target)[2] = CborValue::Text("different-natural-subject".to_owned());
    });
    assert_unavailable(fixture(), |value| {
        let target = &mut array_mut(value)[11];
        array_mut(target)[3] = CborValue::Unsigned(6);
    });
    assert_unavailable(fixture(), |value| {
        array_mut(value)[12] = CborValue::Bytes(
            StateTokenIdV1::derive("different-current-target-head")
                .unwrap()
                .as_bytes()
                .to_vec(),
        );
    });
}

#[test]
fn consent_slot_is_derived_from_target_protocol_and_one_natural_member() {
    let base = fixture();
    let original = base.facts.consent_slot().binding().clone();
    let same = ConsentSlotEvaluationFactsV1::derive_for_target(
        base.facts.target(),
        base.facts.consent_slot().validity(),
    )
    .unwrap();
    assert_eq!(same.binding(), &original);

    assert_unavailable(fixture(), |value| {
        let consent = &mut array_mut(value)[13];
        let binding = &mut array_mut(consent)[1];
        array_mut(binding)[3] = CborValue::Bytes(
            maestro::domain::vnext::authority::ConsentSlotCommitmentIdV1::derive(
                "caller-minted-alternative-slot",
            )
            .unwrap()
            .as_bytes()
            .to_vec(),
        );
    });
    assert_unavailable(fixture(), |value| {
        let consent = &mut array_mut(value)[13];
        let binding = &mut array_mut(consent)[1];
        array_mut(binding)[3] = CborValue::Bytes(
            maestro::domain::vnext::authority::ConsentSlotCommitmentIdV1::derive(
                "prospective-mandate-output-dependent-slot",
            )
            .unwrap()
            .as_bytes()
            .to_vec(),
        );
    });
    assert_unavailable(fixture(), |value| {
        let consent = &mut array_mut(value)[13];
        let binding = &mut array_mut(consent)[1];
        array_mut(binding)[1] = CborValue::Bytes(
            maestro::domain::vnext::authority::ConsentProtocolCommitmentIdV1::derive(
                "wrong-slot-identity-protocol",
            )
            .unwrap()
            .as_bytes()
            .to_vec(),
        );
    });
    assert_unavailable(fixture(), |value| {
        let consent = &mut array_mut(value)[13];
        let members = &mut array_mut(consent)[2];
        let duplicate = array_mut(members)[0].clone();
        array_mut(members).push(duplicate);
    });
}

#[test]
fn distinct_request_keys_converge_to_one_semantic_mandate() {
    let fixture = fixture();
    let alternative_request =
        IssueBootstrapMandateRequestV1::try_from(IssueBootstrapMandateInputV1 {
            request_id: id("alternative-request"),
            idempotency_key: id("alternative-idempotency-key"),
            context_id: fixture.request.context_id(),
            actor_binding_id: fixture.request.actor_binding_id(),
            actor_session_id: fixture.request.actor_session_id(),
            responder_binding_id: fixture.request.responder_binding_id(),
            presentation_observation_id: fixture.request.presentation_observation_id(),
            response_observation_id: fixture.request.response_observation_id(),
            target: fixture.request.target(),
            target_subject: fixture.request.target_subject().to_owned(),
            target_revision: fixture.request.target_revision(),
            consent_slot: fixture.request.consent_slot().clone(),
            supplied_mandates: Vec::new(),
        })
        .unwrap();
    let first = issue_bootstrap_mandate(
        AuthorityEvaluatorV1::evaluate_bootstrap_mandate(fixture.request, &fixture.facts).unwrap(),
    )
    .unwrap();
    let second = issue_bootstrap_mandate(
        AuthorityEvaluatorV1::evaluate_bootstrap_mandate(alternative_request, &fixture.facts)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first.mandate.id(), second.mandate.id());
    assert_eq!(
        first.mandate.canonical_bytes().unwrap(),
        second.mandate.canonical_bytes().unwrap()
    );
}

#[test]
fn authoritative_fact_codecs_round_trip_and_aggregate_decode_is_strict() {
    let fixture = fixture();
    let binding = fixture.facts.actor_binding();
    assert_eq!(
        PrincipalBindingV1::from_canonical_bytes(&binding.canonical_bytes().unwrap()).unwrap(),
        *binding
    );
    let session = fixture.facts.actor_session();
    assert_eq!(
        SessionV1::from_canonical_bytes(&session.canonical_bytes().unwrap()).unwrap(),
        *session
    );
    let genesis = fixture.facts.g0_candidate_paths()[0].genesis_grant();
    assert_eq!(
        maestro::domain::vnext::authority::BootstrapGenesisGrantV1::from_canonical_bytes(
            &genesis.canonical_bytes().unwrap(),
        )
        .unwrap(),
        genesis.clone()
    );
    let join = fixture.facts.interaction_join().unwrap();
    assert_eq!(
        BootstrapMandateInteractionObservationJoinV1::from_canonical_bytes(
            &join.canonical_bytes().unwrap(),
        )
        .unwrap(),
        join.clone()
    );
    let revocations = fixture.facts.revocations();
    assert_eq!(
        AuthorityRevocationSetV1::from_canonical_bytes(&revocations.canonical_bytes().unwrap())
            .unwrap(),
        revocations.clone()
    );
    let snapshot = fixture.facts.snapshot();
    assert_eq!(
        AuthoritySnapshotV1::from_canonical_bytes(&snapshot.canonical_bytes().unwrap()).unwrap(),
        *snapshot
    );

    let value = deterministic_cbor::decode(&fixture.facts.canonical_bytes().unwrap()).unwrap();
    for mutation in 0..3 {
        let mut malformed = value.clone();
        match mutation {
            0 => {
                array_mut(&mut malformed).pop();
            }
            1 => array_mut(&mut malformed).push(CborValue::Unsigned(0)),
            2 => {
                array_mut(&mut malformed)[0] =
                    CborValue::Text("maestro.vnext.unknown-bootstrap-snapshot.v1".to_owned());
            }
            _ => unreachable!(),
        }
        assert_eq!(
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(
                &deterministic_cbor::encode(&malformed).unwrap(),
            ),
            Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot)
        );
    }
}
