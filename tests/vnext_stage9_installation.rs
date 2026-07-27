// Candidate-private assertions are expanded by the owned library module.
#[allow(unused_macros)]
macro_rules! stage9_installation_candidate_tests {
    () => {
        use $crate::domain::distribution::runtime::{
            DistributionDomainKindV1, DistributionDomainRefV1, DistributionRuntimeObjectKindV1,
            DistributionScopedObjectRefV1,
        };
        use $crate::domain::distribution::{CommitmentV1, ResourceDispositionV1};
        use $crate::domain::identity::StoreObjectIdV1;
        use $crate::domain::installation::{
            CutoverDomainBindingV1, DomainCurrentnessV1, HostActivationEntryV1,
            HostAdmissionStateV1, InstallationCensusClassV1, InstallationCensusEntryV1,
            InstallationCensusHeaderV1, InstallationCensusV1, InstallationCutoverErrorV1,
            ObservedHostActivationV1, ObservedInstallationClosureV1,
            UserAgentInstallationClosureV1, assess_user_agent_currentness,
        };
        use $crate::domain::migration::{CutoverCommitmentV1, CutoverDomainRefV1, CutoverDomainV1};

        fn commitment(byte: u8) -> CommitmentV1 {
            CommitmentV1::from_bytes([byte; 32])
        }

        fn object_id(byte: u8) -> StoreObjectIdV1 {
            StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
        }

        fn domain() -> DistributionDomainRefV1 {
            DistributionDomainRefV1::new(
                DistributionDomainKindV1::InstallationDomain,
                commitment(1),
                commitment(2),
                commitment(3),
            )
            .unwrap()
        }

        fn scoped(
            domain: &DistributionDomainRefV1,
            kind: DistributionRuntimeObjectKindV1,
            byte: u8,
        ) -> DistributionScopedObjectRefV1 {
            DistributionScopedObjectRefV1::new(domain.clone(), kind, object_id(byte)).unwrap()
        }

        fn closure() -> UserAgentInstallationClosureV1 {
            let domain = domain();
            let skill = scoped(
                &domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                10,
            );
            UserAgentInstallationClosureV1 {
                domain: domain.clone(),
                release_id: commitment(11),
                binary_claim_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    12,
                ),
                tui_closure_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::TuiClosure,
                    13,
                )),
                capability_catalog_id: commitment(14),
                maestro_skill_claim_ref: skill.clone(),
                agents_activation_claim_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    15,
                ),
                claude_activation_claim_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    16,
                ),
                host_entries: vec![HostActivationEntryV1 {
                    host_tag: 1,
                    domain: domain.clone(),
                    host_adapter_id: commitment(17),
                    admission_state: HostAdmissionStateV1::Admitted,
                    skill_activation_claim_ref: Some(skill),
                    mcp_packet_descriptor_claim_ref: Some(scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                        18,
                    )),
                    mcp_cli_search_descriptor_claim_ref: Some(scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                        19,
                    )),
                    running_catalog_observation_ref: Some(scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::RunningCatalogObservation,
                        20,
                    )),
                }],
                claim_set_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
                    21,
                ),
                receipt_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionReceipt,
                    22,
                ),
                snapshot_catalog_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::OrdinarySnapshotCatalog,
                    23,
                ),
                recovery_root_set_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::RecoveryRootSet,
                    24,
                ),
                verification_result_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::VerificationResult,
                    25,
                ),
            }
        }

        fn observed(closure: &UserAgentInstallationClosureV1) -> ObservedInstallationClosureV1 {
            let host = &closure.host_entries[0];
            ObservedInstallationClosureV1 {
                release_id: closure.release_id,
                binary_claim_ref: closure.binary_claim_ref.clone(),
                claim_set_ref: closure.claim_set_ref.clone(),
                receipt_ref: closure.receipt_ref.clone(),
                snapshot_catalog_ref: closure.snapshot_catalog_ref.clone(),
                recovery_root_set_ref: closure.recovery_root_set_ref.clone(),
                verification_result_ref: closure.verification_result_ref.clone(),
                hosts: vec![ObservedHostActivationV1 {
                    host_tag: host.host_tag,
                    host_adapter_id: host.host_adapter_id,
                    skill_activation_claim_ref: host.skill_activation_claim_ref.clone(),
                    mcp_packet_descriptor_claim_ref: host.mcp_packet_descriptor_claim_ref.clone(),
                    mcp_cli_search_descriptor_claim_ref: host
                        .mcp_cli_search_descriptor_claim_ref
                        .clone(),
                    running_catalog_observation_ref: host.running_catalog_observation_ref.clone(),
                }],
            }
        }

        #[test]
        fn admitted_host_requires_one_skill_and_two_distinct_mcp_descriptors() {
            let closure = closure();
            closure.validate().unwrap();
            let object = closure.to_store_object().unwrap();
            assert_eq!(
                object.schema_id(),
                DistributionRuntimeObjectKindV1::UserAgentInstallationClosure
                    .schema_id()
                    .unwrap()
            );
            assert!(
                object
                    .references()
                    .contains(&closure.receipt_ref.object_id())
            );
            assert!(
                object.references().contains(
                    &closure.host_entries[0]
                        .mcp_packet_descriptor_claim_ref
                        .as_ref()
                        .unwrap()
                        .object_id()
                )
            );
            assert!(
                object.references().contains(
                    &closure.host_entries[0]
                        .mcp_cli_search_descriptor_claim_ref
                        .as_ref()
                        .unwrap()
                        .object_id()
                )
            );
        }

        #[test]
        fn stale_host_is_domain_local_and_never_bearer_authority() {
            let closure = closure();
            let mut observed = observed(&closure);
            assert_eq!(
                assess_user_agent_currentness(&closure, &observed),
                DomainCurrentnessV1::Coherent
            );
            observed.hosts[0].running_catalog_observation_ref = None;
            let currentness = assess_user_agent_currentness(&closure, &observed);
            assert_eq!(
                currentness,
                DomainCurrentnessV1::HostLocalStale { host_tags: vec![1] }
            );
            assert!(currentness.domain_head_remains_current());
            assert!(!currentness.grants_mutation_authority());
        }

        #[test]
        fn store_closure_drift_is_distinct_from_host_local_staleness() {
            let closure = closure();
            let mut observed = observed(&closure);
            observed.release_id = commitment(99);
            let currentness = assess_user_agent_currentness(&closure, &observed);
            assert_eq!(currentness, DomainCurrentnessV1::StoreClosureDrift);
            assert!(!currentness.domain_head_remains_current());
            assert!(!currentness.grants_mutation_authority());
        }

        #[test]
        fn read_only_census_materializes_the_exact_declared_root_closure() {
            let domain = domain();
            let alias = scoped(&domain, DistributionRuntimeObjectKindV1::AliasClosure, 31);
            let entry = InstallationCensusEntryV1 {
                entry_tag: 1,
                display_locator: "~/.maestro/bin/maestro".to_owned(),
                resolved_locator: "/private/tmp/stage9/home/.maestro/bin/maestro".to_owned(),
                classification: InstallationCensusClassV1::Active,
                custody_class:
                    $crate::domain::distribution::runtime::TargetCustodyClassV1::MaestroOwnedTarget,
                unmanaged_reason: None,
                resource_id: Some(commitment(32)),
                bundle_id: Some(commitment(33)),
                release_id: Some(commitment(34)),
                claim_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    35,
                )),
                receipt_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionReceipt,
                    36,
                )),
                content_sha256: Some(commitment(37)),
                alias_closure_ref: alias.clone(),
                consumer_refs: vec![(1, commitment(38)), (2, commitment(39))],
                disposition: ResourceDispositionV1::Retain,
            };
            let census = InstallationCensusV1 {
                header: InstallationCensusHeaderV1 {
                    domain: domain.clone(),
                    inspection_request_ref: scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                        40,
                    ),
                    declared_root_set_ref: scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::DeclaredRootSet,
                        41,
                    ),
                    host_adapter_set_ref: scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::HostAdapterSet,
                        42,
                    ),
                    legacy_locator_set_ref: scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::LegacyLocatorSet,
                        43,
                    ),
                    observed_state_ref: scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::ObservedDistributionState,
                        44,
                    ),
                    proof_profile_id: commitment(45),
                },
                rows: vec![(1, commitment(46), entry)],
            };
            let object = census.to_store_object().unwrap();
            assert_eq!(
                object.schema_id(),
                DistributionRuntimeObjectKindV1::InstallationCensus
                    .schema_id()
                    .unwrap()
            );
            assert!(object.references().contains(&alias.object_id()));
            assert!(
                object
                    .references()
                    .contains(&census.header.declared_root_set_ref.object_id())
            );
        }

        #[test]
        fn unmanaged_census_entry_cannot_carry_a_claim_or_receipt() {
            let domain = domain();
            let entry = InstallationCensusEntryV1 {
                entry_tag: 1,
                display_locator: "~/.foreign".to_owned(),
                resolved_locator: "/private/tmp/stage9/home/.foreign".to_owned(),
                classification: InstallationCensusClassV1::Foreign,
                custody_class:
                    $crate::domain::distribution::runtime::TargetCustodyClassV1::Unmanaged,
                unmanaged_reason: Some(
                    $crate::domain::distribution::runtime::UnmanagedReasonV1::Foreign,
                ),
                resource_id: None,
                bundle_id: None,
                release_id: None,
                claim_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    47,
                )),
                receipt_ref: None,
                content_sha256: None,
                alias_closure_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::AliasClosure,
                    48,
                ),
                consumer_refs: vec![],
                disposition: ResourceDispositionV1::Retain,
            };
            assert!(entry.validate(&domain).is_err());
        }

        #[test]
        fn cutover_domain_binding_derives_one_typed_destination_and_rejects_substitution() {
            let domain = domain();
            let binding = CutoverDomainBindingV1::new(domain.clone(), 7, 11).unwrap();
            assert_eq!(
                binding.destination_domain_id().as_bytes(),
                domain.domain_id().as_bytes()
            );
            assert_eq!(binding.distribution_domain(), &domain);
            binding
                .require_same_cutover_domain_ref(binding.cutover_domain_ref())
                .unwrap();

            let substituted = CutoverDomainRefV1::new(
                CutoverDomainV1::Installation,
                CutoverCommitmentV1::new([99; 32]).unwrap(),
                7,
                11,
            )
            .unwrap();
            assert!(matches!(
                binding.require_same_cutover_domain_ref(&substituted),
                Err(InstallationCutoverErrorV1::CutoverDomainIdentityMismatch)
            ));
        }
    };
}
