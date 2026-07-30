// Candidate-private assertions are expanded by the owned library module.
#[allow(unused_macros)]
macro_rules! stage9_distribution_candidate_tests {
    () => {
        use std::collections::BTreeSet;

        use $crate::domain::authority::ActionRequestIdV1;
        use $crate::domain::distribution::CommitmentV1;
        use $crate::domain::distribution::runtime::{
            CanonicalTargetIdentityV1, CatalogRotationV1, CustodyAssessmentV1, CustodyBasisV1,
            DistributionActionV1, DistributionDomainKindV1, DistributionDomainRefV1,
            DistributionMutationKindV1, DistributionPlanTargetV1, DistributionPlanV1,
            DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
            OrdinarySnapshotCatalogStateV1, TargetEffectKindV1, TargetIdentityPartsV1,
            UnmanagedReasonV1,
        };
        use $crate::domain::identity::StoreObjectIdV1;

        fn commitment(byte: u8) -> CommitmentV1 {
            CommitmentV1::from_bytes([byte; 32])
        }

        fn object_id(byte: u8) -> StoreObjectIdV1 {
            StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
        }

        fn domain(kind: DistributionDomainKindV1) -> DistributionDomainRefV1 {
            DistributionDomainRefV1::new(kind, commitment(1), commitment(2), commitment(3)).unwrap()
        }

        fn scoped(
            domain: &DistributionDomainRefV1,
            kind: DistributionRuntimeObjectKindV1,
            byte: u8,
        ) -> DistributionScopedObjectRefV1 {
            DistributionScopedObjectRefV1::new(domain.clone(), kind, object_id(byte)).unwrap()
        }

        fn managed_custody(domain: &DistributionDomainRefV1) -> CustodyAssessmentV1 {
            CustodyAssessmentV1::assess(&CustodyBasisV1 {
                domain: domain.clone(),
                target_identity: commitment(10),
                alias_closure_id: commitment(11),
                receipt_ref: Some(scoped(
                    domain,
                    DistributionRuntimeObjectKindV1::DistributionReceipt,
                    12,
                )),
                claim_ref: Some(scoped(
                    domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    13,
                )),
                claimed_target_identity: Some(commitment(10)),
                resource_id: Some(commitment(14)),
                bundle_id: Some(commitment(15)),
                release_id: Some(commitment(16)),
                claimed_content_sha256: Some(commitment(17)),
                observed_content_sha256: Some(commitment(18)),
                managed_block: None,
                foreign_owner_observed: false,
                external_manager_observed: false,
                alias_ambiguous: false,
                unsafe_path_state: false,
            })
            .unwrap()
        }

        #[test]
        fn action_catalog_tags_are_the_frozen_distribution_range() {
            assert_eq!(DistributionActionV1::ALL.len(), 13);
            for (index, action) in DistributionActionV1::ALL.into_iter().enumerate() {
                assert_eq!(action.global_tag(), 117 + index as u64);
                assert_eq!(action.local_tag(), 1 + index as u64);
                assert_eq!(action.owner_tag(), 20);
                assert!(!action.literal().is_empty());
            }
        }

        #[test]
        fn canonical_target_identity_binds_alias_mount_manager_and_vacancy() {
            let domain = domain(DistributionDomainKindV1::InstallationDomain);
            let target = CanonicalTargetIdentityV1::new(
                domain.clone(),
                TargetIdentityPartsV1 {
                    display_locator: "~/.maestro/bin/maestro".to_owned(),
                    resolved_locator: "/private/tmp/stage9/home/.maestro/bin/maestro".to_owned(),
                    declared_root_id: commitment(20),
                    parent_identity_id: commitment(21),
                    mount_identity_id: commitment(22),
                    manager_realm_id: commitment(23),
                    security_realm_id: commitment(24),
                    observed_object_identity_id: None,
                    vacant_slot: true,
                    aliases: BTreeSet::from(["/private/tmp/stage9/alias/maestro".to_owned()]),
                },
            )
            .unwrap();
            assert_eq!(target.domain(), &domain);
            assert_ne!(target.identity().as_bytes(), &[0; 32]);

            let mut invalid = target.parts().clone();
            invalid.observed_object_identity_id = Some(commitment(25));
            assert!(CanonicalTargetIdentityV1::new(domain, invalid).is_err());
        }

        #[test]
        fn receipt_bound_claim_retains_custody_across_preexisting_drift() {
            let domain = domain(DistributionDomainKindV1::InstallationDomain);
            let assessment = managed_custody(&domain);
            assert!(assessment.permits_mutation());
            assert!(assessment.has_preexisting_drift());
            assert_eq!(assessment.unmanaged_reason(), None);

            let unmanaged = CustodyAssessmentV1::assess(&CustodyBasisV1 {
                domain,
                target_identity: commitment(10),
                alias_closure_id: commitment(11),
                receipt_ref: None,
                claim_ref: None,
                claimed_target_identity: None,
                resource_id: None,
                bundle_id: None,
                release_id: None,
                claimed_content_sha256: None,
                observed_content_sha256: Some(commitment(18)),
                managed_block: None,
                foreign_owner_observed: false,
                external_manager_observed: false,
                alias_ambiguous: false,
                unsafe_path_state: false,
            })
            .unwrap();
            assert!(!unmanaged.permits_mutation());
            assert_eq!(
                unmanaged.unmanaged_reason(),
                Some(UnmanagedReasonV1::Unclaimed)
            );
        }

        #[test]
        fn plan_rejects_unmanaged_targets_and_wrong_domain_release_binding() {
            let repository = domain(DistributionDomainKindV1::RepositoryDomain);
            let unmanaged = CustodyAssessmentV1::assess(&CustodyBasisV1 {
                domain: repository.clone(),
                target_identity: commitment(10),
                alias_closure_id: commitment(11),
                receipt_ref: None,
                claim_ref: None,
                claimed_target_identity: None,
                resource_id: None,
                bundle_id: None,
                release_id: None,
                claimed_content_sha256: None,
                observed_content_sha256: None,
                managed_block: None,
                foreign_owner_observed: true,
                external_manager_observed: false,
                alias_ambiguous: false,
                unsafe_path_state: false,
            })
            .unwrap();
            let target = DistributionPlanTargetV1 {
                target_tag: 1,
                target_identity_ref: scoped(
                    &repository,
                    DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                    20,
                ),
                target_identity: commitment(10),
                custody: unmanaged,
                expected_preimage_commitment: commitment(21),
                candidate_commitment: Some(commitment(22)),
                effect_kind: TargetEffectKindV1::RewriteOwnedTarget,
                outside_prefix_commitment: None,
                outside_suffix_commitment: None,
            };
            let result = DistributionPlanV1::new(
                repository.clone(),
                DistributionMutationKindV1::Update,
                ActionRequestIdV1::derive("stage9-unmanaged").unwrap(),
                scoped(
                    &repository,
                    DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                    23,
                ),
                scoped(
                    &repository,
                    DistributionRuntimeObjectKindV1::DistributionPlan,
                    24,
                ),
                scoped(
                    &repository,
                    DistributionRuntimeObjectKindV1::IdempotencyKey,
                    25,
                ),
                None,
                None,
                None,
                None,
                vec![target],
            );
            assert!(result.is_err());

            let installation = domain(DistributionDomainKindV1::InstallationDomain);
            assert!(
                DistributionPlanV1::new(
                    installation.clone(),
                    DistributionMutationKindV1::Install,
                    ActionRequestIdV1::derive("stage9-missing-release").unwrap(),
                    scoped(
                        &installation,
                        DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                        26,
                    ),
                    scoped(
                        &installation,
                        DistributionRuntimeObjectKindV1::DistributionPlan,
                        27,
                    ),
                    scoped(
                        &installation,
                        DistributionRuntimeObjectKindV1::IdempotencyKey,
                        28,
                    ),
                    None,
                    None,
                    None,
                    None,
                    vec![],
                )
                .is_err()
            );
        }

        #[test]
        fn rollback_of_rollback_keeps_only_two_prior_ordinary_snapshots() {
            let domain = domain(DistributionDomainKindV1::InstallationDomain);
            let commit = |byte| {
                scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionCommitRecord,
                    byte,
                )
            };
            let snapshot = |byte| {
                scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionSnapshot,
                    byte,
                )
            };
            let mut catalog =
                OrdinarySnapshotCatalogStateV1::empty(domain.clone(), commit(30)).unwrap();
            catalog
                .rotate_after_commit(CatalogRotationV1 {
                    captured_prior_ref: snapshot(40),
                    source_commit_ref: commit(30),
                    captured_sequence: 1,
                    selected_rollback_ref: None,
                    committed_current_ref: commit(31),
                })
                .unwrap();
            catalog
                .rotate_after_commit(CatalogRotationV1 {
                    captured_prior_ref: snapshot(41),
                    source_commit_ref: commit(31),
                    captured_sequence: 2,
                    selected_rollback_ref: None,
                    committed_current_ref: commit(32),
                })
                .unwrap();
            catalog
                .rotate_after_commit(CatalogRotationV1 {
                    captured_prior_ref: snapshot(42),
                    source_commit_ref: commit(32),
                    captured_sequence: 3,
                    selected_rollback_ref: Some(snapshot(41)),
                    committed_current_ref: commit(33),
                })
                .unwrap();
            assert_eq!(catalog.eligible().len(), 2);
            assert_eq!(catalog.eligible()[0].0, snapshot(42));
            assert_eq!(catalog.eligible()[1].0, snapshot(40));
            assert!(!catalog.selectable(&snapshot(41)).unwrap());

            catalog
                .rotate_after_commit(CatalogRotationV1 {
                    captured_prior_ref: snapshot(43),
                    source_commit_ref: commit(33),
                    captured_sequence: 4,
                    selected_rollback_ref: Some(snapshot(40)),
                    committed_current_ref: commit(34),
                })
                .unwrap();
            assert_eq!(catalog.eligible().len(), 2);
            assert_eq!(catalog.eligible()[0].0, snapshot(43));
            assert_eq!(catalog.eligible()[1].0, snapshot(42));
        }
    };
}
