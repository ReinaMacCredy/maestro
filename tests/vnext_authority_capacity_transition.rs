use maestro::domain::vnext::authority::{
    AuthorityContextIdV1, AuthorityContextKindV1, AuthorityTagError, CapacityError,
    CapacityRootIdV1, CapacityUseDispositionV1, CmaEffectWithdrawalSlotFamilyV1,
    CmaObservationPublicationPurposeV1, GovernedCapacityKindV1, GovernedCapacityRootV1,
    RepositoryGovernedCapacitySlotKindV1, TransitionGuardKindV1, TransitionGuardTermV1,
};
use maestro::foundation::core::deterministic_cbor::{self, CborValue};

#[test]
fn transition_guard_closure_matches_the_effective_locked_owner_bundles() {
    use TransitionGuardKindV1 as Kind;
    use TransitionGuardTermV1 as Term;

    assert_eq!(
        Kind::ALL,
        [
            Kind::RepositoryWorkAuthorityPolicyTransition,
            Kind::RepositoryFirstWorkPublication,
            Kind::RepositoryFloorOrTrustRootRotation,
            Kind::InstallationPolicyBindingReplacement,
            Kind::InstallationStructuralRootFloorReplacement,
            Kind::TrustedTimePolicyStackRotation,
            Kind::ExternalLogicalCarrierProfileRotation,
            Kind::PlannedEpochTurnoverPreparation,
        ]
    );
    assert_eq!(
        Kind::RepositoryWorkAuthorityPolicyTransition
            .term_bundle()
            .terms(),
        &[
            Term::RepositoryGovernanceFloor,
            Term::CurrentWorkRootOutgoingRequirement,
            Term::CandidateWorkRootIncomingRequirement,
        ]
    );
    assert_eq!(
        Kind::RepositoryFirstWorkPublication.term_bundle().terms(),
        &[
            Term::RepositoryGovernanceFloor,
            Term::CurrentWorkRootAbsence,
            Term::CandidateWorkRootIncomingRequirement,
        ]
    );
    assert_eq!(
        Kind::RepositoryFloorOrTrustRootRotation
            .term_bundle()
            .terms(),
        &[
            Term::CurrentRepositoryGovernanceFloor,
            Term::ProposedRepositoryGovernanceFloor,
            Term::CurrentPredeclaredContinuityOrRootRotationRequirement,
        ]
    );
    assert_eq!(
        Kind::InstallationPolicyBindingReplacement
            .term_bundle()
            .terms(),
        &[
            Term::CurrentCorePolicyTransitionProtocol,
            Term::CurrentFloorPolicyTransitionRequirement,
            Term::CurrentPolicyOutgoingPolicyTransitionRequirement,
            Term::CandidatePolicyIncomingActivationRequirement,
            Term::CurrentOwnedPolicyTransitionRequirement,
        ]
    );
    assert_eq!(
        Kind::InstallationStructuralRootFloorReplacement
            .term_bundle()
            .terms(),
        &[
            Term::CurrentCoreStructuralTransitionProtocol,
            Term::CurrentFloorOutgoingStructuralTransitionRequirement,
            Term::CandidateFloorIncomingStructuralActivationRequirement,
            Term::CurrentPolicyOutgoingStructuralTransitionRequirement,
            Term::CandidatePolicyIncomingStructuralActivationRequirement,
            Term::CurrentPredeclaredStructuralContinuityRequirement,
            Term::CurrentOwnedStructuralTransitionRequirement,
        ]
    );
    assert_eq!(
        Kind::TrustedTimePolicyStackRotation.term_bundle().terms(),
        &[
            Term::CoreTrustedTimeTransitionFloor,
            Term::CurrentStackOutgoingRequirement,
            Term::CandidateStackIncomingRequirement,
            Term::CurrentContinuityOwnerRotationRequirement,
            Term::CurrentPredeclaredSourceContinuityRequirement,
        ]
    );
    assert_eq!(
        Kind::ExternalLogicalCarrierProfileRotation
            .term_bundle()
            .terms(),
        &[
            Term::CoreExternalHighWaterTransitionFloor,
            Term::CurrentLogicalCarrierProfileOutgoingRequirement,
            Term::CandidateLogicalCarrierProfileIncomingRequirement,
            Term::CurrentContinuityOwnerCarrierRotationRequirement,
            Term::CurrentPredeclaredCarrierContinuityRequirement,
        ]
    );
    assert_eq!(
        Kind::PlannedEpochTurnoverPreparation.term_bundle().terms(),
        &[
            Term::CorePlannedTurnoverPreparationFloor,
            Term::CurrentEpochOutgoingRequirement,
            Term::CandidateEpochIncomingRequirement,
            Term::CurrentContinuityOwnerPreparationRequirement,
            Term::CurrentPredeclaredExternalContinuityRequirement,
        ]
    );

    for (index, expected) in Kind::ALL.into_iter().enumerate() {
        assert_eq!(Kind::try_from(index as u8 + 1), Ok(expected));
        assert_eq!(expected.term_bundle().kind(), expected);
    }
    assert_eq!(
        Kind::try_from(9),
        Err(AuthorityTagError::UnknownTransitionGuardKind(9))
    );
}

#[test]
fn cma_observation_purposes_and_h3_withdrawal_families_are_distinct_closed_sets() {
    use CmaEffectWithdrawalSlotFamilyV1 as Withdrawal;
    use CmaObservationPublicationPurposeV1 as Observation;

    assert_eq!(
        Observation::ALL,
        [
            Observation::TrustedTimeAcquisition,
            Observation::RecoveryExternalRegistration,
            Observation::RecoveryExternalStatus,
            Observation::MaintenanceExecutorCurrentness,
            Observation::ProspectiveContinuityCarrier,
        ]
    );
    assert_eq!(
        Withdrawal::ALL,
        [
            Withdrawal::MaintenanceExecutorCurrentness,
            Withdrawal::ProspectiveContinuityCarrier,
            Withdrawal::PlannedTurnoverHighWater,
            Withdrawal::RepositoryRecoveryAdmission,
            Withdrawal::InstallationRecoveryAdmission,
        ]
    );
    assert_eq!(
        Observation::try_from(1),
        Ok(Observation::TrustedTimeAcquisition)
    );
    assert_eq!(
        Withdrawal::try_from(1),
        Ok(Withdrawal::MaintenanceExecutorCurrentness)
    );
    assert_eq!(
        Observation::try_from(6),
        Err(AuthorityTagError::UnknownCmaObservationPublicationPurpose(
            6
        ))
    );
    assert_eq!(
        Withdrawal::try_from(6),
        Err(AuthorityTagError::UnknownCmaEffectWithdrawalSlotFamily(6))
    );
}

#[test]
fn governed_capacity_debits_only_a_fresh_commit() {
    let context_id = AuthorityContextIdV1::derive("repository-capacity-context").unwrap();
    let kind = GovernedCapacityKindV1::Repository(
        RepositoryGovernedCapacitySlotKindV1::RepositoryAuthorityAdministration,
    );
    let root = GovernedCapacityRootV1::new(
        CapacityRootIdV1::derive("repository-capacity-root").unwrap(),
        AuthorityContextKindV1::RepositoryAuthorityContext,
        context_id,
        kind,
        2,
    )
    .unwrap();

    let committed = root
        .transition(context_id, kind, 0, CapacityUseDispositionV1::FreshCommit)
        .unwrap();
    let debit = committed.debit().expect("fresh commit debit");
    assert_eq!(debit.root_id(), root.id());
    assert_eq!(debit.context_kind(), root.context_kind());
    assert_eq!(debit.context_id(), context_id);
    assert_eq!(debit.kind(), kind);
    assert_eq!(debit.quantity(), 1);
    assert_eq!(debit.ordinal(), 0);
    assert_eq!(debit.prior_spent(), 0);
    assert_eq!(debit.resulting_spent(), 1);
    assert_eq!(committed.root().spent(), 1);
    assert_eq!(committed.root().remaining(), 1);
    let CborValue::Array(root_fields) =
        deterministic_cbor::decode(&root.canonical_bytes().unwrap()).unwrap()
    else {
        panic!("capacity root must be an exact array carrier");
    };
    assert_eq!(root_fields.len(), 7);
    assert_eq!(
        root_fields[0],
        CborValue::Text(GovernedCapacityRootV1::SCHEMA_DOMAIN.to_owned())
    );
    let CborValue::Array(debit_fields) =
        deterministic_cbor::decode(&debit.canonical_bytes().unwrap()).unwrap()
    else {
        panic!("capacity debit must be an exact array carrier");
    };
    assert_eq!(debit_fields.len(), 8);

    let committed_root = *committed.root();
    for disposition in [
        CapacityUseDispositionV1::Replay,
        CapacityUseDispositionV1::NoOp,
        CapacityUseDispositionV1::Failure,
    ] {
        let unchanged = committed_root
            .transition(context_id, kind, 1, disposition)
            .unwrap();
        assert_eq!(unchanged.debit(), None);
        assert_eq!(*unchanged.root(), committed_root);
    }
}

#[test]
fn governed_capacity_rejects_zero_stale_exhausted_and_donated_state() {
    let context_id = AuthorityContextIdV1::derive("repository-capacity-context").unwrap();
    let other_context_id = AuthorityContextIdV1::derive("other-capacity-context").unwrap();
    let root_id = CapacityRootIdV1::derive("repository-capacity-root").unwrap();
    let kind = GovernedCapacityKindV1::Repository(
        RepositoryGovernedCapacitySlotKindV1::RepositoryAuthorityAdministration,
    );
    let other_kind = GovernedCapacityKindV1::Repository(
        RepositoryGovernedCapacitySlotKindV1::RepositoryEvidenceAcquisition,
    );

    assert_eq!(
        GovernedCapacityRootV1::new(
            root_id,
            AuthorityContextKindV1::RepositoryAuthorityContext,
            context_id,
            kind,
            0,
        ),
        Err(CapacityError::InvalidInitialMaximum)
    );
    assert_eq!(
        GovernedCapacityRootV1::new(
            root_id,
            AuthorityContextKindV1::InstallationAuthorityContext,
            context_id,
            kind,
            1,
        ),
        Err(CapacityError::ContextKindMismatch)
    );

    let root = GovernedCapacityRootV1::new(
        root_id,
        AuthorityContextKindV1::RepositoryAuthorityContext,
        context_id,
        kind,
        1,
    )
    .unwrap();
    assert_eq!(
        root.transition(
            other_context_id,
            kind,
            0,
            CapacityUseDispositionV1::FreshCommit,
        ),
        Err(CapacityError::ContextMismatch)
    );
    assert_eq!(
        root.transition(
            context_id,
            other_kind,
            0,
            CapacityUseDispositionV1::FreshCommit,
        ),
        Err(CapacityError::CapacityKindMismatch)
    );
    assert_eq!(
        root.transition(context_id, kind, 1, CapacityUseDispositionV1::FreshCommit),
        Err(CapacityError::ExpectedSpentMismatch)
    );

    let spent = root
        .transition(context_id, kind, 0, CapacityUseDispositionV1::FreshCommit)
        .unwrap()
        .into_root();
    assert_eq!(spent.spent(), 1);
    assert_eq!(spent.remaining(), 0);
    assert_eq!(
        spent.transition(context_id, kind, 1, CapacityUseDispositionV1::FreshCommit),
        Err(CapacityError::Exhausted)
    );
}
