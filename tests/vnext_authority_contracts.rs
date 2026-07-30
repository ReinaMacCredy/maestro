use maestro::domain::authority::{
    ActionAuthorityBasisKindV1, ActionAuthorityBasisV1, ActionOutcomeV1, ActionRequestIdV1,
    ActionResultError, ActionResultV1, AuthorityContextIdV1, AuthorityContextKindV1,
    AuthorityContextV1, AuthorityContinuityManifestV1, AuthoritySnapshotV1, AuthorityTagError,
    AuthorityUseConstraintV1, AuthorityValidationError, AuthorizationReceiptV1,
    BootstrapControlG0AuthorityBasisV1, BootstrapMandateTargetV1, BootstrapTargetDispositionV1,
    CmaBranchIdV1, CmaWithdrawalCapacityIdV1, CmaWithdrawalCapacityV1, CmaWithdrawalPurposeV1,
    ContinuityMaintenanceAuthorityBasisV1, DelegationAncestryV1, DelegationIdV1, DelegationV1,
    ExecutorAssertionIdV1, GenesisGrantIdV1, GrantDefinitionV1, GrantIdV1, GrantScopeV1,
    HalfOpenValidityV1, InstallationAuthorityContinuityClassV1,
    InstallationGovernedCapacitySlotKindV1, OrdinaryAuthorityBasisV1, PrincipalBindingIdV1,
    PrincipalBindingV1, PrincipalIdV1, RepositoryAuthorityContinuityClassV1,
    RepositoryGovernedCapacitySlotKindV1, ResponseOriginV1, RevocationSetV1, RevocationTargetV1,
    ScopeAtomV1, SessionIdV1, SessionV1, SlotIdV1, StateTokenIdV1, TransitionGuardKindV1,
    TrustedTimeV1, bootstrap_mandate_target_catalog, validate_delegation,
    validate_ordinary_authority,
};

#[test]
fn closed_authority_tags_fail_closed_on_unknown_values() {
    assert_eq!(
        AuthorityContextKindV1::try_from(3),
        Err(AuthorityTagError::UnknownAuthorityContextKind(3))
    );
    assert_eq!(
        ActionAuthorityBasisKindV1::try_from(4),
        Err(AuthorityTagError::UnknownActionAuthorityBasisKind(4))
    );
    assert_eq!(
        TransitionGuardKindV1::try_from(9),
        Err(AuthorityTagError::UnknownTransitionGuardKind(9))
    );
}

#[test]
fn capacity_and_bootstrap_catalogs_are_closed_and_withdrawal_never_refills() {
    assert_eq!(RepositoryGovernedCapacitySlotKindV1::ALL.len(), 6);
    assert_eq!(InstallationGovernedCapacitySlotKindV1::ALL.len(), 6);
    assert_eq!(CmaWithdrawalPurposeV1::ALL.len(), 5);
    assert!(RepositoryGovernedCapacitySlotKindV1::try_from(7).is_err());
    assert!(InstallationGovernedCapacitySlotKindV1::try_from(7).is_err());
    assert!(CmaWithdrawalPurposeV1::try_from(6).is_err());

    let catalog = bootstrap_mandate_target_catalog();
    assert_eq!(catalog.len(), 11);
    assert_eq!(
        catalog
            .iter()
            .filter(|row| row.disposition() == BootstrapTargetDispositionV1::Admitted)
            .count(),
        3
    );
    assert_eq!(
        catalog
            .iter()
            .filter(|row| row.disposition() == BootstrapTargetDispositionV1::Excluded)
            .count(),
        8
    );
    assert!(catalog.contains(&BootstrapMandateTargetV1::WithdrawBootstrapMandateInteractionEffect));

    let capacity = CmaWithdrawalCapacityV1::new(
        CmaWithdrawalCapacityIdV1::derive("withdrawal-capacity").unwrap(),
        CmaWithdrawalPurposeV1::TrustedTimeAcquisition,
        2,
    )
    .unwrap();
    let spent = capacity.spend(0).unwrap();
    assert_eq!(spent.remaining(), 1);
    assert_eq!(
        spent.advance_spent(1, 0),
        Err(maestro::domain::authority::CapacityError::NonMonotonicSpend)
    );
}

#[test]
fn authorization_receipts_are_non_bearer_and_replay_is_response_origin() {
    assert_eq!(ActionOutcomeV1::ALL.len(), 7);
    assert!(ActionOutcomeV1::try_from(8).is_err());
    let request_id = ActionRequestIdV1::derive("request").unwrap();
    assert_eq!(
        ActionResultV1::new(request_id, ActionOutcomeV1::Committed, None, None),
        Err(ActionResultError::CommittedRequiresAuthorizationReceipt)
    );
    let receipt = AuthorizationReceiptV1::new(
        request_id,
        AuthorityContextIdV1::derive("context").unwrap(),
        ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
        StateTokenIdV1::derive("prior-state").unwrap(),
        StateTokenIdV1::derive("resulting-state").unwrap(),
    )
    .unwrap();
    assert!(!receipt.is_bearer_authority());
    let mismatched = AuthorizationReceiptV1::new(
        ActionRequestIdV1::derive("different-request").unwrap(),
        AuthorityContextIdV1::derive("context").unwrap(),
        ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
        StateTokenIdV1::derive("prior-state").unwrap(),
        StateTokenIdV1::derive("resulting-state").unwrap(),
    )
    .unwrap();
    assert_eq!(
        ActionResultV1::new(
            request_id,
            ActionOutcomeV1::Committed,
            Some(mismatched),
            None,
        ),
        Err(ActionResultError::AuthorizationReceiptRequestMismatch)
    );
    let result =
        ActionResultV1::new(request_id, ActionOutcomeV1::Committed, Some(receipt), None).unwrap();
    let replay = result.replay();
    assert_eq!(replay.id(), result.id());
    assert_eq!(replay.outcome(), ActionOutcomeV1::Committed);
    assert_eq!(
        replay.response_origin(),
        ResponseOriginV1::Replay {
            original_result_id: result.id()
        }
    );
}

#[test]
fn continuity_manifests_have_exact_nominal_totals() {
    assert_eq!(RepositoryAuthorityContinuityClassV1::ALL.len(), 35);
    assert_eq!(InstallationAuthorityContinuityClassV1::ALL.len(), 30);
    assert!(RepositoryAuthorityContinuityClassV1::try_from(36).is_err());
    assert!(InstallationAuthorityContinuityClassV1::try_from(31).is_err());
    let manifest = AuthorityContinuityManifestV1::repository().unwrap();
    assert_eq!(manifest.class_count(), 35);
}

#[test]
fn ordinary_authority_fails_closed_on_time_revocation_and_stale_subject() {
    let context_id = AuthorityContextIdV1::derive("repository-context").unwrap();
    let principal_id = PrincipalIdV1::derive("principal").unwrap();
    let binding_id = PrincipalBindingIdV1::derive("binding").unwrap();
    let session_id = SessionIdV1::derive("session").unwrap();
    let atom = ScopeAtomV1::new("AmendContract", "contract-a", 3).unwrap();
    let grant = GrantDefinitionV1 {
        id: GrantIdV1::derive("grant").unwrap(),
        context_id,
        grantee_principal_id: principal_id,
        parent_grant_id: None,
        delegation_id: None,
        terminal_scope: GrantScopeV1::new(vec![atom.clone()]).unwrap(),
        delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
        validity: HalfOpenValidityV1::new(10, 100).unwrap(),
        delegation_depth_remaining: 0,
        authority_use_constraint: AuthorityUseConstraintV1::NoLocalBoundedRoot,
    }
    .validate()
    .unwrap();
    let binding = PrincipalBindingV1::new(
        binding_id,
        principal_id,
        context_id,
        2,
        4,
        HalfOpenValidityV1::new(10, 100).unwrap(),
        true,
    )
    .unwrap();
    let session = SessionV1::new(
        session_id,
        binding_id,
        context_id,
        8,
        11,
        "request-a",
        HalfOpenValidityV1::new(20, 80).unwrap(),
    )
    .unwrap();
    let snapshot = AuthoritySnapshotV1::new(
        context_id,
        8,
        11,
        2,
        3,
        TrustedTimeV1::verified(40, 42).unwrap(),
    );
    assert!(
        validate_ordinary_authority(
            &snapshot,
            &binding,
            &session,
            &grant,
            &atom,
            &RevocationSetV1::empty(),
        )
        .is_ok()
    );

    let expired = AuthoritySnapshotV1::new(
        context_id,
        8,
        11,
        2,
        3,
        TrustedTimeV1::verified(100, 101).unwrap(),
    );
    assert_eq!(
        validate_ordinary_authority(
            &expired,
            &binding,
            &session,
            &grant,
            &atom,
            &RevocationSetV1::empty(),
        ),
        Err(AuthorityValidationError::ExpiredOrNotYetValid)
    );

    let revoked =
        RevocationSetV1::new(vec![RevocationTargetV1::PrincipalBinding(binding_id)]).unwrap();
    assert_eq!(
        validate_ordinary_authority(&snapshot, &binding, &session, &grant, &atom, &revoked),
        Err(AuthorityValidationError::Revoked)
    );

    let stale_subject = AuthoritySnapshotV1::new(
        context_id,
        8,
        11,
        2,
        4,
        TrustedTimeV1::verified(40, 42).unwrap(),
    );
    assert_eq!(
        validate_ordinary_authority(
            &stale_subject,
            &binding,
            &session,
            &grant,
            &atom,
            &RevocationSetV1::empty(),
        ),
        Err(AuthorityValidationError::StaleSubjectRevision)
    );
}

#[test]
fn delegation_rejects_scope_widening_and_ancestry_self_targeting() {
    let context_id = AuthorityContextIdV1::derive("repository-context").unwrap();
    let action_a = ScopeAtomV1::new("AmendContract", "subject-a", 3).unwrap();
    let action_b = ScopeAtomV1::new("PublishAssessment", "subject-b", 1).unwrap();
    let parent = GrantDefinitionV1 {
        id: GrantIdV1::derive("parent").unwrap(),
        context_id,
        grantee_principal_id: PrincipalIdV1::derive("parent-principal").unwrap(),
        parent_grant_id: None,
        delegation_id: None,
        terminal_scope: GrantScopeV1::new(vec![action_a.clone()]).unwrap(),
        delegable_scope: GrantScopeV1::new(vec![action_a.clone()]).unwrap(),
        validity: HalfOpenValidityV1::new(10, 100).unwrap(),
        delegation_depth_remaining: 3,
        authority_use_constraint: AuthorityUseConstraintV1::NoLocalBoundedRoot,
    }
    .validate()
    .unwrap();
    let delegation_id = DelegationIdV1::derive("delegation").unwrap();
    let child_id = GrantIdV1::derive("child").unwrap();
    let child_principal = PrincipalIdV1::derive("child-principal").unwrap();
    let widened = GrantDefinitionV1 {
        id: child_id,
        context_id,
        grantee_principal_id: child_principal,
        parent_grant_id: Some(parent.id()),
        delegation_id: Some(delegation_id),
        terminal_scope: GrantScopeV1::new(vec![action_a.clone(), action_b]).unwrap(),
        delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
        validity: HalfOpenValidityV1::new(20, 90).unwrap(),
        delegation_depth_remaining: 2,
        authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(
            maestro::domain::authority::CapacityRootIdV1::derive("root").unwrap(),
        ),
    }
    .validate()
    .unwrap();
    let delegation = DelegationV1::new(delegation_id, parent.id(), child_id);
    let ancestry = DelegationAncestryV1::new(vec![parent.id()], vec![], false).unwrap();
    assert_eq!(
        validate_delegation(&parent, &widened, &delegation, &ancestry),
        Err(AuthorityValidationError::ScopeWidening)
    );

    let self_targeting = GrantDefinitionV1 {
        terminal_scope: GrantScopeV1::new(vec![action_a]).unwrap(),
        grantee_principal_id: child_principal,
        ..widened.definition()
    }
    .validate()
    .unwrap();
    let ancestry =
        DelegationAncestryV1::new(vec![parent.id()], vec![child_principal], false).unwrap();
    assert_eq!(
        validate_delegation(&parent, &self_targeting, &delegation, &ancestry),
        Err(AuthorityValidationError::PrincipalAlreadyInAncestry)
    );
}

#[test]
fn contexts_and_action_bases_are_closed_nominal_unions() {
    let repository = AuthorityContextV1::repository(
        AuthorityContextIdV1::derive("repository-context").unwrap(),
        "repository-installation",
        4,
        7,
        2,
    )
    .unwrap();
    let installation = AuthorityContextV1::installation(
        AuthorityContextIdV1::derive("installation-context").unwrap(),
        "installation",
        "protected-realm",
        8,
        11,
        3,
        5,
    )
    .unwrap();
    assert_eq!(
        repository.kind(),
        AuthorityContextKindV1::RepositoryAuthorityContext
    );
    assert_eq!(
        installation.kind(),
        AuthorityContextKindV1::InstallationAuthorityContext
    );

    let binding = PrincipalBindingIdV1::derive("binding").unwrap();
    let session = SessionIdV1::derive("session").unwrap();
    let bases = [
        ActionAuthorityBasisV1::OrdinaryLiveRuntime(
            OrdinaryAuthorityBasisV1::new(
                binding,
                session,
                GrantIdV1::derive("grant").unwrap(),
                vec![],
            )
            .unwrap(),
        ),
        ActionAuthorityBasisV1::BootstrapControlG0(BootstrapControlG0AuthorityBasisV1::new(
            binding,
            session,
            GenesisGrantIdV1::derive("g0").unwrap(),
        )),
        ActionAuthorityBasisV1::ContinuityMaintenance(ContinuityMaintenanceAuthorityBasisV1::new(
            CmaBranchIdV1::derive("cma").unwrap(),
            SlotIdV1::derive("slot").unwrap(),
            ExecutorAssertionIdV1::derive("executor-assertion").unwrap(),
        )),
    ];
    assert_eq!(
        bases.map(|basis| basis.kind()),
        [
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            ActionAuthorityBasisKindV1::BootstrapControlG0,
            ActionAuthorityBasisKindV1::ContinuityMaintenance,
        ]
    );
}
