use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID: &str =
    "bf2075863bfa3ec7e5269560464182264e78fbeec6dff8197d5dae7bf278a0b4";
const PREDECESSOR_ACTION_SPEC_CATALOG_ID: &str =
    "7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970";

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"))
}

fn assert_success(label: &str, output: Output) {
    assert!(
        output.status.success(),
        "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn stage0_tree_digest(repo: &Path) -> String {
    let output = run(
        repo,
        "git",
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "contracts/vnext/stage0",
            "tools/vnext_contracts/stage0",
        ],
    );
    assert_success("enumerate tracked Stage 0 files", output.clone());
    let mut paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8(path.to_vec()).expect("Stage 0 path must be UTF-8"))
        .filter(|path| !path.ends_with(".pyc") && !path.contains("/__pycache__/"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut digest = Sha256::new();
    for path in paths {
        let path_bytes = path.as_bytes();
        let bytes = fs::read(repo.join(&path)).expect("read tracked Stage 0 file");
        digest.update((path_bytes.len() as u64).to_be_bytes());
        digest.update(path_bytes);
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn strings(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("{field} must be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{field} items must be strings"))
                .to_owned()
        })
        .collect()
}

#[test]
fn stage_two_authority_literals_are_reproducible_closed_and_additive() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let before = stage0_tree_digest(repo);

    assert_success(
        "Stage 2 Authority reproducibility check",
        run(
            repo,
            "python3",
            &["tools/vnext_contracts/stage2/authority/build.py", "--check"],
        ),
    );
    assert_success(
        "Stage 2 independent Ruby verification",
        run(
            repo,
            "ruby",
            &["tools/vnext_contracts/stage2/authority/verify.rb"],
        ),
    );
    assert_success(
        "Stage 2 semantic validation and mutants",
        run(
            repo,
            "python3",
            &[
                "tools/vnext_contracts/stage2/authority/validate.py",
                "--mutants",
            ],
        ),
    );

    let after = stage0_tree_digest(repo);
    assert_eq!(after, before, "Stage 2 verification changed Stage 0 bytes");

    let root = repo.join("contracts/vnext/stage2/authority");
    let literals: Value = serde_json::from_slice(
        &fs::read(root.join("authority-literals.v1.json")).expect("read Authority literals"),
    )
    .expect("parse Authority literals");
    assert_eq!(literals["publication_state"], "inactive_candidate");
    assert_eq!(
        strings(&literals, "authority_contexts"),
        ["RepositoryAuthorityContext", "InstallationAuthorityContext"]
    );
    assert_eq!(strings(&literals, "action_authority_bases").len(), 3);
    assert_eq!(strings(&literals, "action_result_outcomes").len(), 7);
    assert_eq!(strings(&literals, "response_origins"), ["fresh", "replay"]);
    assert_eq!(strings(&literals, "repository_capacity_kinds").len(), 6);
    assert_eq!(strings(&literals, "installation_capacity_kinds").len(), 6);
    assert_eq!(
        strings(&literals, "cma_observation_publication_purposes"),
        [
            "TrustedTimeAcquisition",
            "RecoveryExternalRegistration",
            "RecoveryExternalStatus",
            "MaintenanceExecutorCurrentness",
            "ProspectiveContinuityCarrier",
        ]
    );
    assert_eq!(
        strings(&literals, "cma_effect_withdrawal_slot_families"),
        [
            "MaintenanceExecutorCurrentness",
            "ProspectiveContinuityCarrier",
            "PlannedTurnoverHighWater",
            "RepositoryRecoveryAdmission",
            "InstallationRecoveryAdmission",
        ]
    );
    assert_eq!(
        strings(&literals, "transition_guard_kinds"),
        [
            "RepositoryWorkAuthorityPolicyTransition",
            "RepositoryFirstWorkPublication",
            "RepositoryFloorOrTrustRootRotation",
            "InstallationPolicyBindingReplacement",
            "InstallationStructuralRootFloorReplacement",
            "TrustedTimePolicyStackRotation",
            "ExternalLogicalCarrierProfileRotation",
            "PlannedEpochTurnoverPreparation",
        ]
    );
    assert_eq!(
        strings(&literals, "repository_continuity_classes").len(),
        35
    );
    assert_eq!(
        strings(&literals, "installation_continuity_classes").len(),
        30
    );

    let targets = literals["bootstrap_target_rows"]
        .as_array()
        .expect("Bootstrap target rows");
    assert_eq!(targets.len(), 11);
    assert_eq!(
        targets
            .iter()
            .filter(|row| row["disposition"] == "admitted")
            .count(),
        3
    );
    assert_eq!(
        targets
            .iter()
            .filter(|row| row["disposition"] == "excluded")
            .count(),
        8
    );
    assert_eq!(
        targets
            .iter()
            .map(|row| row["leaf"].as_str().expect("target leaf"))
            .collect::<Vec<_>>(),
        [
            "EnrollRecoveryCommitmentSelection",
            "RotateRecoveryCommitmentSelection",
            "RevokeRecoveryCommitmentSelection",
            "FirstHumanBindingEnrollment",
            "ReserveBootstrapMandateInteractionEffect",
            "PublishBootstrapMandateInteractionOutcome",
            "PublishBootstrapMandatePresentationObservation",
            "PublishBootstrapMandateResponseObservation",
            "ReconcileBootstrapMandateInteractionEffect",
            "IssueBootstrapMandate",
            "WithdrawBootstrapMandateInteractionEffect",
        ]
    );

    let schemas: Value = serde_json::from_slice(
        &fs::read(root.join("schema-descriptors.v1.json")).expect("read schema descriptors"),
    )
    .expect("parse schema descriptors");
    let descriptors = schemas["descriptors"]
        .as_array()
        .expect("schema descriptor rows");
    assert_eq!(descriptors.len(), 22);
    let expected_schemas = [
        "AuthorityMandateV1",
        "BootstrapMandateIssuanceBindingV1",
        "AuthorizationReceiptV1",
        "ActionResultV1",
        "IssueBootstrapMandateRequestV1",
        "ConsentSlotBindingParameterV1",
        "ActionAuthorityBasisV1",
        "AuthorityContextV1",
        "GovernedCapacityDebitV1",
        "AuthorityContinuityManifestV1",
        "PrincipalBindingV1",
        "SessionV1",
        "BootstrapGenesisGrantV1",
        "BootstrapMandateInteractionObservationJoinV1",
        "RevocationSetV1",
        "BootstrapAuthoritySnapshotV1",
        "GovernedCapacityRootV1",
        "SuccessVisibleAuthorityContinuityStateV1",
        "AdmittedTransitionGuardV1",
        "LinearizationCoverageWitnessV1",
        "AuthorityContinuityPostCutConsequenceSetV1",
        "AuthorityContinuityClosureV1",
    ];
    assert_eq!(
        descriptors
            .iter()
            .map(|row| row["schema_name"].as_str().expect("schema name"))
            .collect::<Vec<_>>(),
        expected_schemas
    );
    for descriptor in descriptors {
        let identity = descriptor["descriptor_id"]
            .as_str()
            .expect("schema descriptor identity");
        assert_eq!(identity.len(), 64);
        assert!(
            root.join(descriptor["cbor_path"].as_str().expect("CBOR path"))
                .is_file()
        );
    }

    let action_spec: Value = serde_json::from_slice(
        &fs::read(root.join("action-spec-v2.v1.json")).expect("read ActionSpecV2"),
    )
    .expect("parse ActionSpecV2");
    assert_eq!(action_spec["successor_scope"], "IssueBootstrapMandate_only");
    assert_eq!(action_spec["leaf"], "IssueBootstrapMandate");
    assert_eq!(
        action_spec["issuance_binding_cardinality"]["newly_minted_mandate"],
        1
    );
    assert_eq!(
        action_spec["issuance_binding_cardinality"]["converged_existing_mandate"],
        0
    );
    assert_eq!(
        action_spec["predecessor"]["descriptor_id"],
        PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID
    );
    assert_eq!(
        action_spec["predecessor"]["catalog_09_manifest_id"],
        PREDECESSOR_ACTION_SPEC_CATALOG_ID
    );
    assert_eq!(
        strings(&action_spec, "produced_record_closure"),
        [
            "AuthorityMandateOrConvergenceRefV1",
            "BootstrapMandateIssuanceBindingV1:exactly_one_if_newly_minted",
            "AuthorizationReceiptV1:primary",
            "ActionResultV1",
            "IdempotencyRecordV1",
            "AuthorityContinuityClosureV1:pre_cut_exact",
            "SuccessVisibleAuthorityContinuityStateV1:exactly_one",
            "AdmittedTransitionGuardV1:persisted_from_serialization_current_owner_facts",
            "BootstrapAuthoritySnapshotV1:successor_current_authority_carrier",
            "LinearizationCoverageWitnessV1:recoverable",
            "AuthorityContinuityPostCutConsequenceSetV1:complete_exact",
        ]
    );
    assert_eq!(
        action_spec["produced_schema_bindings"]
            .as_array()
            .expect("produced schema bindings")
            .iter()
            .map(|row| row["schema_name"].as_str().expect("produced schema name"))
            .collect::<Vec<_>>(),
        [
            "AuthorityMandateV1",
            "BootstrapMandateIssuanceBindingV1",
            "AuthorizationReceiptV1",
            "ActionResultV1",
            "BootstrapAuthoritySnapshotV1",
            "AuthorityContinuityClosureV1",
            "SuccessVisibleAuthorityContinuityStateV1",
            "AdmittedTransitionGuardV1",
            "LinearizationCoverageWitnessV1",
            "AuthorityContinuityPostCutConsequenceSetV1",
        ]
    );
    assert!(
        action_spec["produced_schema_bindings"]
            .as_array()
            .expect("produced schema bindings")
            .iter()
            .all(|row| row["descriptor_id"]
                .as_str()
                .is_some_and(|id| id.len() == 64))
    );

    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join("stage2-authority-manifest.v1.json"))
            .expect("read Stage 2 Authority manifest"),
    )
    .expect("parse Stage 2 Authority manifest");
    assert_eq!(manifest["stage0_tree_sha256"], before);
    assert_eq!(manifest["publication_state"], "inactive_candidate");
    assert_eq!(
        manifest["root_id"]
            .as_str()
            .expect("Stage 2 Authority root identity")
            .len(),
        64
    );
}
