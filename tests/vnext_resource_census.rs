use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Output};

use maestro::domain::vnext::distribution::{
    BundleKindV1, BundleManifestInputV1, BundleManifestV1, CommitmentV1, ContentEncodingV1,
    DirectConsumerInputV1, DirectConsumerKindV1, DirectConsumerV1, OwnerRefV1,
    ReleaseResourceCensusInputV1, ReleaseResourceCensusV1, ResourceDescriptorInputV1,
    ResourceDescriptorV1, ResourceDispositionV1, ResourceKindV1, ResourceProvenanceKindV1,
    ResourceReleaseError, TargetClassV1, release_census_entry_descriptor_id,
};
use maestro::foundation::core::deterministic_cbor::CborValue;

fn run_python(repo: &Path, args: &[&str], label: &str) -> Output {
    let output = Command::new("python3")
        .args(args)
        .current_dir(repo)
        .env("PYTHONPATH", ".")
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert!(
        output.status.success(),
        "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

#[test]
fn historical_census_evidence_is_attested_and_stays_non_promoting() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_output = run_python(
        repo,
        &[
            "tools/vnext_contracts/public/build_census_ledgers.py",
            "--check",
        ],
        "historical census ledger deterministic build check",
    );
    let build_receipt: serde_json::Value =
        serde_json::from_slice(&build_output.stdout).expect("parse census ledger build receipt");
    assert_eq!(build_receipt["status"], "pass");
    assert_eq!(build_receipt["mode"], "check");
    assert_eq!(build_receipt["physical_receipt_emitted"], false);
    assert_eq!(build_receipt["e204_count"], 204);
    assert_eq!(
        build_receipt["e204_digest"],
        "c8fc4c6cd53d81272d19c3b402e99a0ca3f69ebd18cf9464539db1d1ecf85388"
    );
    assert_eq!(build_receipt["c325_count"], 325);
    assert_eq!(
        build_receipt["c325_digest"],
        "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a"
    );
    assert_eq!(build_receipt["mismatches"].as_array().unwrap().len(), 0);

    let output = run_python(
        repo,
        &[
            "tools/vnext_contracts/public/validate_census.py",
            "--repo",
            ".",
        ],
        "vNext census validation",
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse census validation receipt");
    assert_eq!(receipt["stage0_historical_evidence_admission"], "pass");
    assert_eq!(
        receipt["stage11_live_migration_admission"],
        "blocked_pending_recensus"
    );
    assert_eq!(
        receipt["embedded"]["status"],
        "non_promoting_historical_coverage_attested"
    );
    assert_eq!(receipt["embedded"]["count"], 204);
    assert_eq!(
        receipt["embedded"]["current_source_equality_claimed"],
        false
    );
    assert_eq!(
        receipt["direct_consumers"]["status"],
        "non_promoting_historical_coverage_attested"
    );
    assert_eq!(receipt["direct_consumers"]["count"], 325);
    assert_eq!(
        receipt["direct_consumers"]["current_source_equality_claimed"],
        false
    );
    assert_eq!(
        receipt["physical"]["status"],
        "historical_tool_output_attestation_pass"
    );
    assert_eq!(receipt["physical"]["historical_locator_count"], 28_102);
    assert_eq!(receipt["physical"]["current_live_locator_count"], 28_075);
    assert_eq!(receipt["physical"]["current_live_equality"], false);
    assert_eq!(receipt["physical"]["stage11_recensus_required"], true);
    assert_eq!(receipt["physical"]["output_bytes"], 459);
    assert_eq!(
        receipt["physical"]["output_sha256"],
        "5b27eb5d880e9b8ab313672676fc25b5b39d95b1bd476b10b626ed13b5155341"
    );
    assert_eq!(receipt["physical"]["parsed"]["node_count"], 28_102);
    assert_eq!(receipt["physical"]["parsed"]["stable"], true);
    assert_eq!(receipt["physical"]["parsed"]["changed_rows"], 0);

    let mutant_output = run_python(
        repo,
        &[
            "tools/vnext_contracts/public/validate_census.py",
            "--repo",
            ".",
            "--mutant-suite",
        ],
        "historical census mutation suite",
    );
    let mutant_receipt: serde_json::Value =
        serde_json::from_slice(&mutant_output.stdout).expect("parse census mutant receipt");
    assert_eq!(mutant_receipt["status"], "pass");
    assert_eq!(mutant_receipt["total_mutants"], 4);
    assert_eq!(mutant_receipt["rejected_mutants"], 4);
    assert_eq!(mutant_receipt["escaped"].as_array().unwrap().len(), 0);

    let live_migration = Command::new("python3")
        .args([
            "tools/vnext_contracts/public/validate_census.py",
            "--repo",
            ".",
            "--require-live-migration",
        ])
        .current_dir(repo)
        .env("PYTHONPATH", ".")
        .output()
        .expect("run live-migration census gate");
    assert_eq!(live_migration.status.code(), Some(2));
}

#[test]
fn current_surface_and_consumer_census_are_exact_and_content_bound() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    run_python(
        repo,
        &[
            "-m",
            "unittest",
            "-q",
            "tools.vnext_contracts.stage0.resource_release.c868_contract",
        ],
        "frozen C868 contract suite",
    );

    let surface: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            repo.join("contracts/vnext/stage0/resource-release/current-surface-manifest.v1.json"),
        )
        .expect("read current surface manifest"),
    )
    .expect("parse current surface manifest");
    let consumers: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            repo.join("contracts/vnext/stage0/resource-release/current-consumer-census.v1.json"),
        )
        .expect("read current consumer census"),
    )
    .expect("parse current consumer census");

    assert_stage0_commitment(&surface, "maestro.vnext.current-surface-manifest.v1");
    assert_stage0_commitment(&consumers, "maestro.vnext.current-consumer-census.v1");

    let resources = surface["resources"]
        .as_array()
        .expect("surface Resource rows");
    let direct_readers = surface["direct_readers"]
        .as_array()
        .expect("surface direct-reader rows");
    let consumer_readers = consumers["readers"]
        .as_array()
        .expect("consumer direct-reader rows");
    assert_eq!(surface["resource_count"], 377);
    assert_eq!(resources.len(), 377);
    assert_eq!(surface["direct_reader_edge_count"], 377);
    assert_eq!(direct_readers.len(), 377);
    assert_eq!(consumers["resource_count"], 377);
    assert_eq!(consumers["direct_reader_edge_count"], 377);
    assert_eq!(consumer_readers.len(), 377);
    assert_eq!(consumer_readers, direct_readers);
    assert_eq!(consumers["exact_one_reader_evidence_per_resource"], true);
    assert_eq!(consumers["historical_c325_promoted"], false);

    let resource_keys = resources
        .iter()
        .map(|row| {
            row["stable_resource_key"]
                .as_str()
                .expect("Resource stable key")
        })
        .collect::<BTreeSet<_>>();
    let reader_resource_keys = direct_readers
        .iter()
        .map(|row| {
            row["resource_stable_key"]
                .as_str()
                .expect("reader Resource stable key")
        })
        .collect::<BTreeSet<_>>();
    let resource_ids = resources
        .iter()
        .map(|row| row["resource_id"].as_str().expect("Resource id"))
        .collect::<BTreeSet<_>>();
    let reader_resource_ids = direct_readers
        .iter()
        .map(|row| row["resource_id"].as_str().expect("reader Resource id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(resource_keys.len(), 377);
    assert_eq!(resource_keys, reader_resource_keys);
    assert_eq!(resource_ids.len(), 377);
    assert_eq!(resource_ids, reader_resource_ids);
    assert_eq!(
        resources
            .iter()
            .filter(|row| row["required_bundle_kind"] == "Migration")
            .count(),
        184
    );

    let surface_value = surface["canonical_value"]
        .as_array()
        .expect("surface canonical value");
    assert_eq!(surface_value.len(), 7);
    assert_eq!(surface_value[2].as_array().unwrap().len(), 377);
    assert_eq!(surface_value[3].as_array().unwrap().len(), 377);
    assert!(surface_value[6].as_array().unwrap().is_empty());
    let consumer_value = consumers["canonical_value"]
        .as_array()
        .expect("consumer canonical value");
    assert_eq!(consumer_value.len(), 3);
    assert_eq!(consumer_value[2].as_array().unwrap().len(), 377);

    assert!(surface["unclassified_paths"].as_array().unwrap().is_empty());
    assert_eq!(
        surface["generated_output_policy"]["classification"],
        "post_release_noncanonical"
    );
    assert_eq!(
        surface["generated_output_policy"]["path_byte_and_presence_identity_participation"],
        false
    );
    assert_eq!(
        surface["generated_output_policy"]["root_worker_post_root_delta_owner"],
        true
    );

    assert_fields_absent(
        &surface,
        &[
            "status",
            "rows",
            "unclassified_count",
            "identity_domain",
            "manifest_identity_envelope",
            "current_source_equality_claimed",
            "duplicate_classification_count",
            "declared_roots",
            "excluded_or_aggregated",
        ],
    );
    assert_fields_absent(
        &consumers,
        &[
            "status",
            "rows",
            "edge_count",
            "fabricated_default_edge_count",
            "source_reference_edge_count",
            "generated_contract_binding_edge_count",
            "identity_domain",
            "manifest_identity_envelope",
            "catalog_literal_lineage",
        ],
    );
}

fn assert_stage0_commitment(document: &serde_json::Value, schema: &str) {
    assert_eq!(document["schema"], schema);
    assert_eq!(document["identity_protocol"], "Stage0CanonicalCommitmentV1");
    assert_eq!(
        document["identity_scope"],
        "canonical_commitment_envelope_only"
    );
    assert_eq!(document["candidate_only"], true);
    assert_eq!(document["runtime_activation"], false);

    let envelope = document["canonical_commitment_envelope"]
        .as_array()
        .expect("canonical commitment envelope");
    assert_eq!(envelope.len(), 2);
    assert_eq!(envelope[0], schema);
    assert_eq!(envelope[1], document["canonical_value"]);
    assert_eq!(
        document["identity"].as_str().expect("commitment identity"),
        format!(
            "sha256:{}",
            document["canonical_cbor_sha256"]
                .as_str()
                .expect("canonical CBOR digest")
        )
    );
    assert!(document["canonical_cbor_byte_length"].as_u64().unwrap() > 0);
    assert_eq!(
        document["canonical_cbor_hex"]
            .as_str()
            .expect("canonical CBOR hex")
            .len(),
        document["canonical_cbor_byte_length"].as_u64().unwrap() as usize * 2
    );
}

fn assert_fields_absent(document: &serde_json::Value, fields: &[&str]) {
    for field in fields {
        assert!(
            document.get(field).is_none(),
            "legacy field {field:?} must not survive the canonical commitment shape"
        );
    }
}

fn id(value: u8) -> CommitmentV1 {
    CommitmentV1::from_bytes([value; 32])
}

fn descriptor(
    tag: u64,
    kind: BundleKindV1,
    disposition: ResourceDispositionV1,
) -> ResourceDescriptorV1 {
    ResourceDescriptorV1::new(ResourceDescriptorInputV1 {
        resource_tag: tag,
        stable_resource_key: format!("stable-{tag}"),
        content: format!("content-{tag}").into_bytes(),
        content_encoding: ContentEncodingV1::Utf8Text,
        media_type: "text/plain".to_owned(),
        resource_kind: ResourceKindV1::PublicContract,
        semantic_owner: OwnerRefV1::new(1, id(1)).expect("owner"),
        required_bundle_kind: kind,
        provenance_kind: ResourceProvenanceKindV1::FirstParty,
        provenance_commitment_id: id(2),
        license_commitment_id: None,
        backward_dependencies: vec![],
        compatibility_profile_id: id(3),
        generator_commitment_id: None,
        target_policy_profile_id: id(4),
        custody_policy_profile_id: id(5),
        migration_profile_id: id(6),
        rollback_profile_id: id(7),
        uninstall_profile_id: id(8),
        retention_profile_id: id(9),
        removal_profile_id: id(10),
        disposition,
        proof_profile_id: id(11),
    })
    .expect("descriptor")
}

fn census_fixture() -> (
    Vec<ResourceDescriptorV1>,
    Vec<BundleManifestV1>,
    ReleaseResourceCensusV1,
) {
    let resources = BundleKindV1::NON_RELEASE_TOPOLOGY
        .into_iter()
        .enumerate()
        .map(|(index, kind)| descriptor(index as u64 + 1, kind, ResourceDispositionV1::Retain))
        .collect::<Vec<_>>();
    let mut bundles = Vec::new();
    for (index, resource) in resources.iter().cloned().enumerate() {
        bundles.push(
            BundleManifestV1::new(BundleManifestInputV1 {
                bundle_tag: index as u64 + 1,
                bundle_kind: resource.required_bundle_kind(),
                stable_bundle_key: format!("bundle-{}", index + 1),
                semantic_version: "1".to_owned(),
                compatibility_profile_id: id(20),
                resources: vec![resource],
                dependency_bundles: bundles.last().cloned().into_iter().collect(),
                provenance_commitment_id: id(21),
                license_commitment_id: None,
                package_policy_profile_id: id(22),
                supported_target_classes: vec![TargetClassV1::WholeTarget],
                rollback_profile_id: id(23),
                uninstall_profile_id: id(24),
                retention_profile_id: id(25),
            })
            .expect("bundle"),
        );
    }
    let consumer = DirectConsumerV1::new(DirectConsumerInputV1 {
        locator: "consumer".to_owned(),
        semantic_owner: OwnerRefV1::new(1, id(30)).expect("owner"),
        consumer_kind: DirectConsumerKindV1::Runtime,
        resources: vec![resources[0].resource_ref()],
        provenance_commitment_id: id(31),
        disposition: ResourceDispositionV1::Retain,
        migration_profile_id: id(32),
        proof_profile_id: id(33),
        removal_profile_id: id(34),
    })
    .expect("consumer");
    let resource_locators = resources
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
        source_inventory_digest: id(40),
        consumer_inventory_digest: id(41),
        build_graph_digest: id(42),
        resource_locators: Some(resource_locators),
    })
    .expect("census");
    (resources, bundles, census)
}

fn array_mut(value: &mut CborValue) -> &mut Vec<CborValue> {
    match value {
        CborValue::Array(values) => values,
        _ => panic!("expected array"),
    }
}

fn rehash_census_row(row: &mut CborValue) {
    let entry = array_mut(row)[2].clone();
    let descriptor_id = release_census_entry_descriptor_id(&entry).expect("descriptor id");
    array_mut(row)[1] = CborValue::Bytes(descriptor_id.as_bytes().to_vec());
}

#[test]
fn census_locator_is_separate_from_stable_resource_key() {
    let (resources, _, census) = census_fixture();
    assert_eq!(
        census.resource_locator(resources[0].id()),
        Some("locator/1")
    );
    assert_ne!(
        census.resource_locator(resources[0].id()),
        Some(resources[0].stable_resource_key())
    );
}

#[test]
fn wrong_owning_bundle_and_cross_resource_consumer_pairs_are_rejected() {
    let (resources, bundles, census) = census_fixture();

    let mut wrong_bundle = census.envelope().clone();
    let envelope = array_mut(&mut wrong_bundle);
    let rows = array_mut(&mut envelope[4]);
    let first_row = array_mut(&mut rows[0]);
    let entry = array_mut(&mut first_row[2]);
    let resource_branch = array_mut(&mut entry[1]);
    let resource_value = array_mut(&mut resource_branch[1]);
    resource_value[3] = CborValue::Bytes(bundles[1].id().as_bytes().to_vec());
    rehash_census_row(&mut rows[0]);
    assert_eq!(
        ReleaseResourceCensusV1::from_envelope(wrong_bundle, &resources, &bundles).unwrap_err(),
        ResourceReleaseError::InvalidCensusCoordinates
    );

    let mut cross_resource = census.envelope().clone();
    let envelope = array_mut(&mut cross_resource);
    let rows = array_mut(&mut envelope[4]);
    let consumer_row = array_mut(&mut rows[resources.len()]);
    let entry = array_mut(&mut consumer_row[2]);
    let consumer_branch = array_mut(&mut entry[2]);
    let consumer_value = array_mut(&mut consumer_branch[1]);
    let resource_pairs = array_mut(&mut consumer_value[3]);
    let first_pair = array_mut(&mut resource_pairs[0]);
    first_pair[0] = CborValue::Unsigned(resources[1].resource_tag());
    rehash_census_row(&mut rows[resources.len()]);
    assert_eq!(
        ReleaseResourceCensusV1::from_envelope(cross_resource, &resources, &bundles).unwrap_err(),
        ResourceReleaseError::InvalidConsumerEdge
    );
}

#[test]
fn manifest_core_profile_commitments_are_pinned() {
    let (resources, bundles, census) = census_fixture();
    let mut mutant = census.envelope().clone();
    let envelope = array_mut(&mut mutant);
    let header = array_mut(&mut envelope[3]);
    let core = array_mut(&mut header[0]);
    core[9] = CborValue::Bytes(id(99).as_bytes().to_vec());
    assert!(ReleaseResourceCensusV1::from_envelope(mutant, &resources, &bundles).is_err());
}

#[test]
fn remove_requires_zero_direct_consumers() {
    let (mut resources, _, _) = census_fixture();
    resources[0] = descriptor(1, BundleKindV1::Migration, ResourceDispositionV1::Remove);
    let mut rebuilt_bundles = Vec::new();
    for (index, resource) in resources.iter().cloned().enumerate() {
        rebuilt_bundles.push(
            BundleManifestV1::new(BundleManifestInputV1 {
                bundle_tag: index as u64 + 1,
                bundle_kind: resource.required_bundle_kind(),
                stable_bundle_key: format!("bundle-{}", index + 1),
                semantic_version: "1".to_owned(),
                compatibility_profile_id: id(20),
                resources: vec![resource],
                dependency_bundles: rebuilt_bundles.last().cloned().into_iter().collect(),
                provenance_commitment_id: id(21),
                license_commitment_id: None,
                package_policy_profile_id: id(22),
                supported_target_classes: vec![TargetClassV1::WholeTarget],
                rollback_profile_id: id(23),
                uninstall_profile_id: id(24),
                retention_profile_id: id(25),
            })
            .expect("rebuilt bundle"),
        );
    }
    let consumer = DirectConsumerV1::new(DirectConsumerInputV1 {
        locator: "consumer".to_owned(),
        semantic_owner: OwnerRefV1::new(1, id(30)).expect("owner"),
        consumer_kind: DirectConsumerKindV1::Runtime,
        resources: vec![resources[0].resource_ref()],
        provenance_commitment_id: id(31),
        disposition: ResourceDispositionV1::Retain,
        migration_profile_id: id(32),
        proof_profile_id: id(33),
        removal_profile_id: id(34),
    })
    .expect("consumer");
    let result = ReleaseResourceCensusV1::new(ReleaseResourceCensusInputV1 {
        release_key: "release".to_owned(),
        release_version: "1".to_owned(),
        platform_qualifier: "test".to_owned(),
        resources,
        bundles: rebuilt_bundles,
        direct_consumers: vec![consumer],
        source_inventory_digest: id(40),
        consumer_inventory_digest: id(41),
        build_graph_digest: id(42),
        resource_locators: None,
    });
    assert_eq!(
        result.unwrap_err(),
        ResourceReleaseError::RemoveHasDirectConsumers
    );
}
