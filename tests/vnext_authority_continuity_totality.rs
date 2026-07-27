use maestro::domain::authority::{
    AcceptedAuthorityTimeFloorV1, AuthorityContextKindV1, AuthorityContinuityCoverageDispositionV1,
    AuthorityContinuityCoverageObligationV1, AuthorityContinuityError,
    AuthorityContinuityManifestV1, ContinuityClassIdV1, ContinuityReferenceV1,
    ContinuitySemanticOwnerV1, CoverageDispositionKindV1, CoverageObligationIdV1,
    HTimeAcceptanceErrorV1, HTimeAcceptanceRelationV1, HTimeCarryBasisV1,
    HTimeContinuationContributionV1, InstallationAuthorityContinuityClassV1, OwnerContributionIdV1,
    RepositoryAuthorityContinuityClassV1, TransitionGuardKindV1,
    repository_authority_continuity_totality_input,
};
use sha2::{Digest, Sha256};

fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn reference(seed: &str) -> ContinuityReferenceV1 {
    ContinuityReferenceV1::derive(seed).unwrap()
}

#[test]
fn independent_totality_census_freezes_the_exact_repository_golden() {
    let manifest = AuthorityContinuityManifestV1::prove(
        repository_authority_continuity_totality_input().unwrap(),
    )
    .unwrap();

    let expected_classes = RepositoryAuthorityContinuityClassV1::ALL
        .map(ContinuityClassIdV1::Repository)
        .to_vec();
    assert_eq!(manifest.class_ids(), expected_classes);
    assert_eq!(manifest.class_count(), 35);
    assert_eq!(manifest.obligation_count(), 36);
    assert_eq!(
        hexadecimal(&Sha256::digest(manifest.canonical_bytes().unwrap())),
        "dfaa99f7cdc41cc0d67fde4788026d7b696a9d3241aedf3897e35144381b9c3e"
    );

    let installation = AuthorityContinuityManifestV1::installation().unwrap();
    assert_eq!(installation.class_count(), 30);
    assert_eq!(InstallationAuthorityContinuityClassV1::ALL.len(), 30);
}

#[test]
fn totality_refuses_omission_extra_duplicate_wrong_owner_orphan_cycle_and_version_mutants() {
    let base = repository_authority_continuity_totality_input().unwrap();

    let mut omitted = base.clone();
    omitted.obligations.pop();
    assert_eq!(
        AuthorityContinuityManifestV1::prove(omitted).unwrap_err(),
        AuthorityContinuityError::FrozenOwnerCensusMismatch
    );

    let mut duplicated_obligation = base.clone();
    duplicated_obligation
        .obligations
        .push(duplicated_obligation.obligations[0].clone());
    assert_eq!(
        AuthorityContinuityManifestV1::prove(duplicated_obligation).unwrap_err(),
        AuthorityContinuityError::DuplicateObligation
    );

    let mut swapped_dispositions = base.clone();
    let first_kind = swapped_dispositions.dispositions[0].kind.clone();
    swapped_dispositions.dispositions[0].kind = swapped_dispositions.dispositions[1].kind.clone();
    swapped_dispositions.dispositions[1].kind = first_kind;
    assert_eq!(
        AuthorityContinuityManifestV1::prove(swapped_dispositions).unwrap_err(),
        AuthorityContinuityError::FrozenOwnerCensusMismatch
    );

    let mut extra = base.clone();
    let extra_id = CoverageObligationIdV1::new(2_000).unwrap();
    extra
        .obligations
        .push(AuthorityContinuityCoverageObligationV1 {
            id: extra_id,
            context_kind: AuthorityContextKindV1::InstallationAuthorityContext,
            owner: ContinuitySemanticOwnerV1::Authority,
            source_protocol: reference("extra-installation-source"),
            subject_projection: reference("extra-installation-projection"),
            protocol_version: 1,
        });
    extra
        .dispositions
        .push(AuthorityContinuityCoverageDispositionV1 {
            obligation_id: extra_id,
            owner: ContinuitySemanticOwnerV1::Authority,
            kind: CoverageDispositionKindV1::ExplicitlyNonContinuity {
                owner_invariant: reference("extra-owner-invariant"),
                proof: reference("extra-proof"),
            },
        });
    assert_eq!(
        AuthorityContinuityManifestV1::prove(extra).unwrap_err(),
        AuthorityContinuityError::WrongContext
    );

    let mut duplicate = base.clone();
    duplicate.descriptors.push(duplicate.descriptors[0].clone());
    assert_eq!(
        AuthorityContinuityManifestV1::prove(duplicate).unwrap_err(),
        AuthorityContinuityError::DuplicateClass
    );

    let mut omitted_descriptor = base.clone();
    omitted_descriptor.descriptors.pop();
    assert_eq!(
        AuthorityContinuityManifestV1::prove(omitted_descriptor).unwrap_err(),
        AuthorityContinuityError::DescriptorSetMismatch
    );

    let mut wrong_owner = base.clone();
    wrong_owner.descriptors[0].owner = ContinuitySemanticOwnerV1::Research;
    assert_eq!(
        AuthorityContinuityManifestV1::prove(wrong_owner).unwrap_err(),
        AuthorityContinuityError::WrongOwner
    );

    let mut orphan = base.clone();
    orphan.descriptors[0].owner_contribution_id = OwnerContributionIdV1::new(999).unwrap();
    assert_eq!(
        AuthorityContinuityManifestV1::prove(orphan).unwrap_err(),
        AuthorityContinuityError::OrphanReference
    );

    let mut descriptor_cycle = base.clone();
    let first_class = descriptor_cycle.descriptors[0].class_id;
    descriptor_cycle.descriptors[0].depends_on.push(first_class);
    assert_eq!(
        AuthorityContinuityManifestV1::prove(descriptor_cycle).unwrap_err(),
        AuthorityContinuityError::CyclicReference
    );

    let mut contribution_cycle = base.clone();
    let first_contribution = contribution_cycle.owner_contributions[0].id;
    contribution_cycle.owner_contributions[0]
        .depends_on
        .push(first_contribution);
    assert_eq!(
        AuthorityContinuityManifestV1::prove(contribution_cycle).unwrap_err(),
        AuthorityContinuityError::CyclicReference
    );

    let mut unsupported = base;
    unsupported.protocol_version = 2;
    assert_eq!(
        AuthorityContinuityManifestV1::prove(unsupported).unwrap_err(),
        AuthorityContinuityError::UnsupportedVersion
    );
}

#[test]
fn included_by_is_nonempty_finite_exact_and_explicit_non_continuity_is_not_a_class() {
    let mut input = repository_authority_continuity_totality_input().unwrap();
    let meta = input
        .dispositions
        .iter()
        .find(|row| {
            matches!(
                row.kind,
                CoverageDispositionKindV1::ExplicitlyNonContinuity { .. }
            )
        })
        .unwrap();
    assert!(
        !input
            .closed_class_sum
            .iter()
            .any(|class_id| class_id.tag() as u16 == meta.obligation_id.get())
    );

    let included = input
        .dispositions
        .iter_mut()
        .find(|row| matches!(row.kind, CoverageDispositionKindV1::IncludedBy { .. }))
        .unwrap();
    included.kind = CoverageDispositionKindV1::IncludedBy {
        class_ids: Vec::new(),
    };
    assert_eq!(
        AuthorityContinuityManifestV1::prove(input).unwrap_err(),
        AuthorityContinuityError::InvalidIncludedClassSet
    );
}

#[test]
fn authority_owns_authority_semantics_while_persistence_owns_only_physical_retention() {
    let repository = AuthorityContinuityManifestV1::repository().unwrap();
    for class in RepositoryAuthorityContinuityClassV1::ALL
        .into_iter()
        .take(22)
    {
        assert_eq!(
            repository
                .descriptor(ContinuityClassIdV1::Repository(class))
                .unwrap()
                .owner,
            ContinuitySemanticOwnerV1::Authority
        );
    }
    assert_eq!(
        repository
            .descriptor(ContinuityClassIdV1::Repository(
                RepositoryAuthorityContinuityClassV1::RepositoryPersistenceRetentionState,
            ))
            .unwrap()
            .owner,
        ContinuitySemanticOwnerV1::Persistence
    );

    let installation = AuthorityContinuityManifestV1::installation().unwrap();
    for class in InstallationAuthorityContinuityClassV1::ALL
        .into_iter()
        .take(20)
    {
        assert_eq!(
            installation
                .descriptor(ContinuityClassIdV1::Installation(class))
                .unwrap()
                .owner,
            ContinuitySemanticOwnerV1::Authority
        );
    }
}

#[test]
fn trusted_time_acceptance_has_exact_genesis_same_advance_and_upper_bound_rules() {
    assert_eq!(
        TransitionGuardKindV1::RepositoryFloorOrTrustRootRotation
            .term_bundle()
            .terms()
            .len(),
        3
    );
    let initial = AcceptedAuthorityTimeFloorV1::context_genesis(
        reference("stable-lineage"),
        reference("coordinate"),
        reference("stack"),
        reference("externally-rooted-time-origin"),
        170,
        180,
    )
    .unwrap();
    assert_eq!(initial.lower_bound(), 170);
    assert_eq!(
        initial.relation(),
        HTimeAcceptanceRelationV1::ContextGenesis
    );

    let carry_only = AcceptedAuthorityTimeFloorV1::continue_from(
        &initial,
        initial.stable_lineage(),
        initial.coordinate(),
        initial.policy_stack(),
        HTimeCarryBasisV1::ExactNoLineageChange,
        HTimeContinuationContributionV1::CarryOnly,
    )
    .unwrap();
    assert_eq!(carry_only.lower_bound(), 170);
    assert_eq!(carry_only.relation(), HTimeAcceptanceRelationV1::Same);

    let upper_does_not_advance = AcceptedAuthorityTimeFloorV1::continue_from(
        &initial,
        initial.stable_lineage(),
        initial.coordinate(),
        initial.policy_stack(),
        HTimeCarryBasisV1::ExactNoLineageChange,
        HTimeContinuationContributionV1::CarryPlusFreshLower {
            lower_bound: 160,
            upper_bound: 9_999,
        },
    )
    .unwrap();
    assert_eq!(upper_does_not_advance.lower_bound(), 170);
    assert_eq!(
        upper_does_not_advance.relation(),
        HTimeAcceptanceRelationV1::Same
    );

    let advance = AcceptedAuthorityTimeFloorV1::continue_from(
        &initial,
        initial.stable_lineage(),
        initial.coordinate(),
        initial.policy_stack(),
        HTimeCarryBasisV1::ExactNoLineageChange,
        HTimeContinuationContributionV1::CarryPlusFreshLower {
            lower_bound: 175,
            upper_bound: 180,
        },
    )
    .unwrap();
    assert_eq!(advance.lower_bound(), 175);
    assert_eq!(advance.relation(), HTimeAcceptanceRelationV1::Advance);

    assert_eq!(
        AcceptedAuthorityTimeFloorV1::continue_from(
            &initial,
            initial.stable_lineage(),
            initial.coordinate(),
            initial.policy_stack(),
            HTimeCarryBasisV1::ExactNoLineageChange,
            HTimeContinuationContributionV1::CarryPlusFreshLower {
                lower_bound: 160,
                upper_bound: 169,
            },
        )
        .unwrap_err(),
        HTimeAcceptanceErrorV1::InvalidFreshBounds
    );
    assert_eq!(
        AcceptedAuthorityTimeFloorV1::continue_from(
            &initial,
            initial.stable_lineage(),
            initial.coordinate(),
            reference("different-stack"),
            HTimeCarryBasisV1::ExactNoLineageChange,
            HTimeContinuationContributionV1::CarryOnly,
        )
        .unwrap_err(),
        HTimeAcceptanceErrorV1::FalseNoLineageChange
    );
}
