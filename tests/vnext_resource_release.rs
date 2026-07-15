use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use maestro::domain::vnext::distribution::{
    BUNDLE_KIND_COUNT, BundleKindV1, BundleManifestInputV1, BundleManifestV1,
    C868_RUNTIME_EDGE_COUNT, C868_SCHEMA_COUNT, C868_SUITE_COMPONENT_COUNT, CommitmentV1,
    ContentEncodingV1, DESCRIPTOR_ENVELOPE_SLOT_COUNT, DeltaDispositionV1, DeltaIdentityKindV1,
    DirectConsumerInputV1, DirectConsumerKindV1, DirectConsumerV1, DownstreamIdentityObligationV1,
    DownstreamObligationKindV1, EmbeddedReleaseBundleV1, EmbeddedReleaseInputV1,
    ExpectedDeltaClosureV1, IdentityDeltaEntryV1, MANIFEST_ENVELOPE_SLOT_COUNT,
    NON_RELEASE_BUNDLE_KIND_COUNT, OwnerRefV1, RESOURCE_DESCRIPTOR_FIELD_COUNT,
    ReleaseResourceCensusInputV1, ReleaseResourceCensusV1, ResourceDescriptorInputV1,
    ResourceDescriptorV1, ResourceDispositionV1, ResourceKindV1, ResourceProvenanceKindV1,
    ResourceRefV1, ResourceReleaseError, TargetClassV1, validate_manifest_identity_cbor,
    validate_release_closure,
};
use maestro::foundation::core::deterministic_cbor::CborValue;

fn commitment(value: u8) -> CommitmentV1 {
    CommitmentV1::from_bytes([value; 32])
}

fn resource(
    tag: u64,
    kind: BundleKindV1,
    dependencies: Vec<ResourceRefV1>,
    disposition: ResourceDispositionV1,
) -> ResourceDescriptorV1 {
    resource_with_compatibility(tag, kind, dependencies, disposition, commitment(13))
}

fn resource_with_compatibility(
    tag: u64,
    kind: BundleKindV1,
    dependencies: Vec<ResourceRefV1>,
    disposition: ResourceDispositionV1,
    compatibility_profile_id: CommitmentV1,
) -> ResourceDescriptorV1 {
    ResourceDescriptorV1::new(ResourceDescriptorInputV1 {
        resource_tag: tag,
        stable_resource_key: format!("resource-{tag}"),
        content: format!("resource-{tag}").into_bytes(),
        content_encoding: ContentEncodingV1::Utf8Text,
        media_type: "text/plain".to_owned(),
        resource_kind: ResourceKindV1::PublicContract,
        semantic_owner: OwnerRefV1::new(1, commitment(11)).expect("owner"),
        required_bundle_kind: kind,
        provenance_kind: ResourceProvenanceKindV1::FirstParty,
        provenance_commitment_id: commitment(12),
        license_commitment_id: None,
        backward_dependencies: dependencies,
        compatibility_profile_id,
        generator_commitment_id: None,
        target_policy_profile_id: commitment(14),
        custody_policy_profile_id: commitment(15),
        migration_profile_id: commitment(16),
        rollback_profile_id: commitment(17),
        uninstall_profile_id: commitment(18),
        retention_profile_id: commitment(19),
        removal_profile_id: commitment(20),
        disposition,
        proof_profile_id: commitment(21),
    })
    .expect("ResourceDescriptorV1")
}

fn build_bundles(resources: &[ResourceDescriptorV1]) -> Vec<BundleManifestV1> {
    let mut bundles = Vec::new();
    for (index, resource) in resources.iter().cloned().enumerate() {
        let dependency_bundles = bundles.last().cloned().into_iter().collect();
        bundles.push(
            BundleManifestV1::new(BundleManifestInputV1 {
                bundle_tag: index as u64 + 1,
                bundle_kind: resource.required_bundle_kind(),
                stable_bundle_key: format!("bundle-{}", index + 1),
                semantic_version: "1".to_owned(),
                compatibility_profile_id: commitment(30),
                resources: vec![resource],
                dependency_bundles,
                provenance_commitment_id: commitment(31),
                license_commitment_id: None,
                package_policy_profile_id: commitment(32),
                supported_target_classes: vec![TargetClassV1::WholeTarget],
                rollback_profile_id: commitment(33),
                uninstall_profile_id: commitment(34),
                retention_profile_id: commitment(35),
            })
            .expect("BundleManifestV1"),
        );
    }
    bundles
}

fn closure(
    kinds: &[BundleKindV1],
) -> (
    Vec<ResourceDescriptorV1>,
    Vec<BundleManifestV1>,
    ReleaseResourceCensusV1,
    EmbeddedReleaseBundleV1,
) {
    let mut resources = Vec::new();
    for (index, kind) in kinds.iter().copied().enumerate() {
        let dependencies = resources
            .last()
            .map(ResourceDescriptorV1::resource_ref)
            .into_iter()
            .collect();
        resources.push(resource(
            index as u64 + 1,
            kind,
            dependencies,
            ResourceDispositionV1::Retain,
        ));
    }
    let bundles = build_bundles(&resources);
    let deserialized_resource =
        ResourceDescriptorV1::from_envelope(resources[0].envelope().clone())
            .expect("deserialized-style Resource");
    let consumer = DirectConsumerV1::from_resources(
        DirectConsumerInputV1 {
            locator: "consumer".to_owned(),
            semantic_owner: OwnerRefV1::new(1, commitment(41)).expect("owner"),
            consumer_kind: DirectConsumerKindV1::Runtime,
            resources: vec![],
            provenance_commitment_id: commitment(42),
            disposition: ResourceDispositionV1::Retain,
            migration_profile_id: commitment(43),
            proof_profile_id: commitment(44),
            removal_profile_id: commitment(45),
        },
        &[deserialized_resource],
    )
    .expect("consumer");
    let locators = resources
        .iter()
        .map(|resource| {
            (
                resource.id(),
                format!("locator/{}", resource.resource_tag()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let census = ReleaseResourceCensusV1::new(ReleaseResourceCensusInputV1 {
        release_key: "release".to_owned(),
        release_version: "1".to_owned(),
        platform_qualifier: "test".to_owned(),
        resources: resources.clone(),
        bundles: bundles.clone(),
        direct_consumers: vec![consumer],
        source_inventory_digest: commitment(51),
        consumer_inventory_digest: commitment(52),
        build_graph_digest: commitment(53),
        resource_locators: Some(locators),
    })
    .expect("ReleaseResourceCensusV1");
    let release = EmbeddedReleaseBundleV1::new(
        EmbeddedReleaseInputV1 {
            release_key: "release".to_owned(),
            release_version: "1".to_owned(),
            platform_qualifier: "test".to_owned(),
            core_contract_root_id: commitment(61),
            binary_compatibility_id: commitment(62),
            public_catalog_id: commitment(63),
            compatibility_profile_id: commitment(64),
            rollback_profile_id: commitment(65),
            uninstall_profile_id: commitment(66),
            retention_profile_id: commitment(67),
        },
        &resources,
        &bundles,
        &census,
    )
    .expect("EmbeddedReleaseBundleV1");
    (resources, bundles, census, release)
}

fn default_kinds() -> [BundleKindV1; NON_RELEASE_BUNDLE_KIND_COUNT] {
    BundleKindV1::NON_RELEASE_TOPOLOGY
}

fn as_array(value: &CborValue) -> &[CborValue] {
    match value {
        CborValue::Array(values) => values,
        _ => panic!("expected array"),
    }
}

#[test]
fn frozen_field_envelope_domain_and_kind_counts_are_literal() {
    let (resources, bundles, census, release) = closure(&default_kinds());
    assert_eq!(C868_SCHEMA_COUNT, 38);
    assert_eq!(C868_SUITE_COMPONENT_COUNT, 62);
    assert_eq!(C868_RUNTIME_EDGE_COUNT, 61);
    assert_eq!(BUNDLE_KIND_COUNT, 8);
    assert_eq!(
        as_array(resources[0].value()).len(),
        RESOURCE_DESCRIPTOR_FIELD_COUNT
    );
    assert_eq!(
        as_array(resources[0].envelope()).len(),
        DESCRIPTOR_ENVELOPE_SLOT_COUNT
    );
    assert_eq!(
        as_array(bundles[0].envelope()).len(),
        MANIFEST_ENVELOPE_SLOT_COUNT
    );
    assert_eq!(
        as_array(census.envelope()).len(),
        MANIFEST_ENVELOPE_SLOT_COUNT
    );
    assert_eq!(
        as_array(release.envelope()).len(),
        MANIFEST_ENVELOPE_SLOT_COUNT
    );
    assert_eq!(
        as_array(resources[0].envelope())[0],
        CborValue::Text("maestro.vnext.resource.descriptor.v1".to_owned())
    );
    assert_eq!(
        as_array(release.envelope())[0],
        CborValue::Text("maestro.vnext.embedded-release-bundle.manifest.v1".to_owned())
    );
}

#[test]
fn repeated_bundle_kinds_and_strict_backward_dependency_subsets_are_valid() {
    let kinds = [
        BundleKindV1::Migration,
        BundleKindV1::ExternalPattern,
        BundleKindV1::SharedContract,
        BundleKindV1::SharedContract,
        BundleKindV1::Orchestration,
        BundleKindV1::Capability,
        BundleKindV1::Adapter,
        BundleKindV1::AgentBootstrap,
    ];
    let (resources, bundles, census, release) = closure(&kinds);
    assert_eq!(bundles.len(), 8);
    assert_eq!(bundles[2].kind(), bundles[3].kind());
    assert!(bundles[0].dependencies().is_empty());
    assert!(
        bundles
            .iter()
            .skip(1)
            .all(|bundle| bundle.dependencies().len() == 1)
    );
    validate_release_closure(&resources, &bundles, &census, &release)
        .expect("exact repeated-kind closure");
}

#[test]
fn release_is_the_sole_release_kind_root_without_synthetic_state() {
    let (resources, bundles, census, release) = closure(&default_kinds());
    assert!(
        bundles
            .iter()
            .all(|bundle| bundle.kind() != BundleKindV1::Release)
    );
    assert_eq!(as_array(release.value()).len(), 2);
    assert_eq!(as_array(&as_array(release.value())[0]).len(), 13);
    assert_eq!(release.census_id(), census.id());
    assert_eq!(release.bundle_ids().len(), NON_RELEASE_BUNDLE_KIND_COUNT);
    validate_release_closure(&resources, &bundles, &census, &release)
        .expect("exact Release closure");

    let release_bundle = BundleManifestV1::new(BundleManifestInputV1 {
        bundle_tag: 1,
        bundle_kind: BundleKindV1::Release,
        stable_bundle_key: "release".to_owned(),
        semantic_version: "1".to_owned(),
        compatibility_profile_id: commitment(1),
        resources: vec![resource(
            1,
            BundleKindV1::Release,
            vec![],
            ResourceDispositionV1::Retain,
        )],
        dependency_bundles: vec![],
        provenance_commitment_id: commitment(2),
        license_commitment_id: None,
        package_policy_profile_id: commitment(3),
        supported_target_classes: vec![TargetClassV1::WholeTarget],
        rollback_profile_id: commitment(4),
        uninstall_profile_id: commitment(5),
        retention_profile_id: commitment(6),
    });
    assert_eq!(
        release_bundle.unwrap_err(),
        ResourceReleaseError::InvalidBundleKind
    );
}

#[test]
fn strict_cbor_rejects_null_even_when_nested() {
    assert!(validate_manifest_identity_cbor(&[0x81, 0xf6]).is_err());
    let (resources, bundles, census, release) = closure(&default_kinds());
    for bytes in [
        resources[0].canonical_cbor(),
        bundles[0].canonical_cbor(),
        census.canonical_cbor(),
        release.canonical_cbor(),
    ] {
        validate_manifest_identity_cbor(bytes).expect("strict canonical identity bytes");
    }
}

#[test]
fn resource_closed_tag_99_mutant_is_rejected() {
    let original = resource(
        1,
        BundleKindV1::Migration,
        vec![],
        ResourceDispositionV1::Retain,
    );
    let mut envelope = original.envelope().clone();
    let envelope = match &mut envelope {
        CborValue::Array(envelope) => envelope,
        _ => unreachable!(),
    };
    let value = match &mut envelope[2] {
        CborValue::Array(value) => value,
        _ => unreachable!(),
    };
    value[4] = CborValue::Unsigned(99);
    assert_eq!(
        ResourceDescriptorV1::from_envelope(CborValue::Array(envelope.clone())).unwrap_err(),
        ResourceReleaseError::InvalidClosedTag
    );
}

#[test]
fn hidden_future_resource_identity_outside_dependency_field_is_rejected() {
    let future = resource(
        2,
        BundleKindV1::ExternalPattern,
        vec![],
        ResourceDispositionV1::Retain,
    );
    let hidden_future = resource_with_compatibility(
        1,
        BundleKindV1::Migration,
        vec![],
        ResourceDispositionV1::Retain,
        future.id(),
    );
    let mut resources = vec![hidden_future, future];
    for (index, kind) in BundleKindV1::NON_RELEASE_TOPOLOGY
        .into_iter()
        .skip(2)
        .enumerate()
    {
        resources.push(resource(
            index as u64 + 3,
            kind,
            vec![],
            ResourceDispositionV1::Retain,
        ));
    }
    let bundles = build_bundles(&resources);
    let census = ReleaseResourceCensusV1::new(ReleaseResourceCensusInputV1 {
        release_key: "release".to_owned(),
        release_version: "1".to_owned(),
        platform_qualifier: "test".to_owned(),
        resources: resources.clone(),
        bundles: bundles.clone(),
        direct_consumers: vec![],
        source_inventory_digest: commitment(51),
        consumer_inventory_digest: commitment(52),
        build_graph_digest: commitment(53),
        resource_locators: None,
    })
    .expect("semantically shaped census before identity-placement validation");
    let release = EmbeddedReleaseBundleV1::new(
        EmbeddedReleaseInputV1 {
            release_key: "release".to_owned(),
            release_version: "1".to_owned(),
            platform_qualifier: "test".to_owned(),
            core_contract_root_id: commitment(61),
            binary_compatibility_id: commitment(62),
            public_catalog_id: commitment(63),
            compatibility_profile_id: commitment(64),
            rollback_profile_id: commitment(65),
            uninstall_profile_id: commitment(66),
            retention_profile_id: commitment(67),
        },
        &resources,
        &bundles,
        &census,
    )
    .expect("full Census semantics before hidden identity placement gate");
    assert_eq!(
        validate_release_closure(&resources, &bundles, &census, &release).unwrap_err(),
        ResourceReleaseError::IdentityBackreference
    );
}

#[test]
fn release_resource_tags_are_globally_strict_across_bundles() {
    let resources = BundleKindV1::NON_RELEASE_TOPOLOGY
        .into_iter()
        .map(|kind| resource(1, kind, vec![], ResourceDispositionV1::Retain))
        .collect::<Vec<_>>();
    let bundles = build_bundles(&resources);
    let result = ReleaseResourceCensusV1::new(ReleaseResourceCensusInputV1 {
        release_key: "release".to_owned(),
        release_version: "1".to_owned(),
        platform_qualifier: "test".to_owned(),
        resources,
        bundles,
        direct_consumers: vec![],
        source_inventory_digest: commitment(51),
        consumer_inventory_digest: commitment(52),
        build_graph_digest: commitment(53),
        resource_locators: None,
    });
    assert_eq!(result.unwrap_err(), ResourceReleaseError::InvalidStrictTags);
}

#[test]
fn exact_delta_has_only_release_then_root_finalization_and_handoff_obligations() {
    let (_, _, _, release) = closure(&default_kinds());
    let entries = DeltaIdentityKindV1::ALL
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            IdentityDeltaEntryV1::new(
                kind,
                format!("identity-{index}"),
                None,
                if kind == DeltaIdentityKindV1::Release {
                    release.release_id()
                } else {
                    commitment(index as u8 + 80)
                },
                DeltaDispositionV1::Introduce,
                "generated:test",
                commitment(index as u8 + 90),
            )
            .expect("delta entry")
        })
        .collect::<Vec<_>>();
    let obligations = DownstreamObligationKindV1::ALL
        .into_iter()
        .map(|kind| DownstreamIdentityObligationV1::new(kind, release.release_id()))
        .collect::<Vec<_>>();
    let delta = ExpectedDeltaClosureV1::new(entries.clone(), obligations.clone())
        .expect("exact downstream obligations");
    assert_eq!(delta.entries().len(), 6);
    assert_eq!(delta.downstream_obligations().len(), 3);
    assert_eq!(
        ExpectedDeltaClosureV1::new(entries, obligations[..2].to_vec()).unwrap_err(),
        ResourceReleaseError::InvalidDownstreamIdentityObligations
    );
}

#[test]
fn owner_digest_sha256_prefix_is_normalized() {
    let digest = "11".repeat(32);
    assert_eq!(
        CommitmentV1::from_hex(&digest).expect("raw digest"),
        CommitmentV1::from_hex(&format!("sha256:{digest}")).expect("rendered digest")
    );
}

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .env("PYTHONPATH", ".")
        .output()
        .unwrap_or_else(|error| panic!("{program}: {error}"))
}

#[test]
fn frozen_c868_oracle_and_mutations_pass() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let oracle = run(
        repo,
        "python3",
        &[
            "-m",
            "unittest",
            "-q",
            "tools.vnext_contracts.stage0.resource_release.c868_contract",
        ],
    );
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
}
