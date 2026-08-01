use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const EXPECTED_STAGE3_SOURCES: &[&str] = &[
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
    "contracts/vnext/public/setup_operation_compatibility.v1.json",
    "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json",
    "src/lib.rs",
    "src/domain/mod.rs",
    "src/domain/authority/action_basis.rs",
    "src/domain/authority/bootstrap_catalog.rs",
    "src/domain/authority/capacity.rs",
    "src/domain/authority/closed.rs",
    "src/domain/authority/context.rs",
    "src/domain/authority/continuity.rs",
    "src/domain/authority/continuity/allocation.rs",
    "src/domain/authority/continuity/catalog.rs",
    "src/domain/authority/continuity/closure.rs",
    "src/domain/authority/continuity/state.rs",
    "src/domain/authority/continuity/totality.rs",
    "src/domain/authority/continuity/trusted_time.rs",
    "src/domain/authority/downstream_action_basis.rs",
    "src/domain/authority/evaluator.rs",
    "src/domain/authority/facade.rs",
    "src/domain/authority/facade/repository_admission.rs",
    "src/domain/authority/facade/repository_leaf_authority.rs",
    "src/domain/authority/facade_tests.rs",
    "src/domain/authority/grant.rs",
    "src/domain/authority/governance_attestation.rs",
    "src/domain/authority/governance_attestation_stage7_seed.rs",
    "src/domain/authority/governance_floor.rs",
    "src/domain/authority/identity.rs",
    "src/domain/authority/legacy_removal_guard.rs",
    "src/domain/authority/mandate.rs",
    "src/domain/authority/materialization.rs",
    "src/domain/authority/mod.rs",
    "src/domain/authority/post_cut.rs",
    "src/domain/authority/principal.rs",
    "src/domain/authority/protected_diagnostic_envelope.rs",
    "src/domain/authority/protected_diagnostic_envelope_stage8_seed.rs",
    "src/domain/authority/publication.rs",
    "src/domain/authority/result.rs",
    "src/domain/authority/transition.rs",
    "src/domain/contract/assembly.rs",
    "src/domain/contract/component.rs",
    "src/domain/contract/component_kind.rs",
    "src/domain/contract/decision_closure.rs",
    "src/domain/contract/finalization.rs",
    "src/domain/contract/handoff.rs",
    "src/domain/contract/materialization.rs",
    "src/domain/contract/mod.rs",
    "src/domain/contract/proof.rs",
    "src/domain/contract/provenance.rs",
    "src/domain/contract/root.rs",
    "src/domain/contract/runtime.rs",
    "src/domain/evidence/submission_claim.rs",
    "src/domain/design/batch.rs",
    "src/domain/design/closure.rs",
    "src/domain/design/common.rs",
    "src/domain/design/decision.rs",
    "src/domain/design/legacy.rs",
    "src/domain/design/materialization.rs",
    "src/domain/design/mod.rs",
    "src/domain/design/revision.rs",
    "src/domain/evidence/assessment.rs",
    "src/domain/evidence/claim.rs",
    "src/domain/evidence/diagnostics/mod.rs",
    "src/domain/evidence/erasure.rs",
    "src/domain/evidence/identity.rs",
    "src/domain/evidence/mod.rs",
    "src/domain/evidence/observation.rs",
    "src/domain/evidence/store.rs",
    "src/domain/identity/digest.rs",
    "src/domain/identity/manifest.rs",
    "src/domain/identity/mod.rs",
    "src/domain/identity/schema.rs",
    "src/domain/mod.rs",
    "src/domain/persistence/consumer_snapshot.rs",
    "src/domain/persistence/export.rs",
    "src/domain/persistence/generation.rs",
    "src/domain/persistence/idempotency.rs",
    "src/domain/persistence/legacy_quarantine.rs",
    "src/domain/persistence/legacy_source_history.rs",
    "src/domain/persistence/metadata.rs",
    "src/domain/persistence/mod.rs",
    "src/domain/persistence/object.rs",
    "src/domain/persistence/protected_diagnostic.rs",
    "src/domain/persistence/protected_diagnostic_stage9_seed.rs",
    "src/domain/persistence/protected_locator_lease.rs",
    "src/domain/persistence/protected_locator_stage9_seed.rs",
    "src/domain/persistence/retention.rs",
    "src/domain/persistence/root_universe.rs",
    "src/domain/persistence/snapshot.rs",
    "src/domain/persistence/snapshot_blocks.rs",
    "src/domain/persistence/snapshot_export.rs",
    "src/domain/persistence/snapshot_restore.rs",
    "src/domain/persistence/snapshot_rows.rs",
    "src/domain/persistence/store.rs",
    "src/domain/persistence/tests/atomic_publication.rs",
    "src/domain/persistence/tests/canonical_store.rs",
    "src/domain/persistence/tests/mod.rs",
    "src/domain/persistence/tests/store_full_export.rs",
    "src/domain/persistence/tests/store_safety.rs",
    "src/domain/persistence/types.rs",
    "src/domain/repository/bootstrap.rs",
    "src/domain/repository/legacy_quarantine_admission.rs",
    "src/domain/repository/legacy_source_history.rs",
    "src/domain/repository/mod.rs",
    "src/domain/repository/root_universe.rs",
    "src/domain/repository/tests.rs",
    "src/domain/step/amendment.rs",
    "src/domain/step/graph.rs",
    "src/domain/step/identity.rs",
    "src/domain/step/lifecycle.rs",
    "src/domain/step/mod.rs",
    "src/domain/step/revision.rs",
    "src/domain/step/submission.rs",
    "src/domain/work/identity.rs",
    "src/domain/work/lifecycle.rs",
    "src/domain/work/mod.rs",
    "src/domain/work/relation.rs",
    "src/domain/work/submission.rs",
    "src/foundation/mod.rs",
    "src/foundation/core/mod.rs",
    "src/foundation/core/deterministic_cbor.rs",
    "src/foundation/core/secure_fs.rs",
    "tests/vnext_work_identity.rs",
    "tests/vnext_work_lifecycle.rs",
    "tests/vnext_work_relations.rs",
    "tests/vnext_step_graph.rs",
    "tests/vnext_step_amendment.rs",
    "tests/vnext_step_amendment_application.rs",
    "tests/vnext_contract_step_publication.rs",
    "tests/vnext_design_revisions.rs",
    "tests/vnext_decision_kernel.rs",
    "tests/vnext_decision_closure.rs",
    "tests/vnext_decision_materialization_plan.rs",
    "tests/vnext_evidence_claims.rs",
    "tests/vnext_submission_claim_set.rs",
    "tests/vnext_stage3_contracts.rs",
    "tools/vnext_contracts/public/build_public_literals.py",
    "tools/vnext_contracts/catalogs/cbor_py.py",
    "tools/vnext_contracts/stage2/authority/build.py",
    "tools/vnext_contracts/stage2/authority/validate.py",
    "tools/vnext_contracts/stage2/authority/verify.rb",
    "tools/vnext_contracts/stage3/domain/build.py",
    "tools/vnext_contracts/stage3/domain/validate.py",
    "tools/vnext_contracts/stage3/domain/verify.rb",
];

const REQUIRED_STAGE3_INVARIANTS: &[&str] = &[
    "claim_binds_exactly_one_submission",
    "claim_subject_matches_full_submission_subject",
    "work_claim_subject_matches_exact_step_submission_closure",
    "submission_claim_cardinality_1_to_n_without_second_count_cap",
    "nonauthoritative_claim_carrier_refused",
    "step_binding_commits_contract_generation",
    "contract_generation_identity_excludes_runtime_authority_and_is_predictable",
    "candidate_root_derived_only_from_typed_consequence_plan",
    "equal_root_detected_before_authority_and_requires_none",
    "exactly_equivalent_distinct_root_validated_before_authority_and_writes_nothing",
    "same_store_atomic_owner_joined_publication",
    "initial_contract_publication_roots_complete_step_dag_and_open_fresh_states",
    "contract_amendment_consumes_total_step_plan_and_publishes_all_dispositions",
    "stage3_satisfaction_carry_unavailable_until_canonical_evidence_gate_material",
    "closed_owner_handlers_have_no_generic_lifecycle_bypass",
    "repository_actions_use_exact_nominal_authority_leaves",
    "deferred_execution_evidence_and_gate_publication_surfaces_absent",
    "materialization_candidate_only_and_joined_only_by_contract_publication",
    "no_standalone_materialize_decision_action",
    "ordinary_grant_has_canonical_parent_delegation_reachability",
    "authority_is_store_loaded_and_action_is_admitted_before_commit",
    "replay_returns_original_committed_result",
    "stale_store_basis_refused_before_commit",
    "failed_replayed_or_stale_publication_leaves_no_orphan_objects",
];

struct TemporaryRoot(PathBuf);

impl TemporaryRoot {
    fn new(name: &str) -> Self {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "maestro-stage3-proof-{}-{sequence}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary Stage 3 proof root");
        Self(path)
    }
}

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"))
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create mutant Stage 3 root");
    for entry in fs::read_dir(source).expect("read Stage 3 artifact root") {
        let entry = entry.expect("read Stage 3 artifact entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy Stage 3 artifact");
        }
    }
}

fn write_json(path: &Path, value: &Value) {
    let mut encoded = serde_json::to_vec_pretty(value).expect("encode mutant Stage 3 JSON");
    encoded.push(b'\n');
    fs::write(path, encoded).expect("write mutant Stage 3 JSON");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON artifact"))
        .expect("parse JSON artifact")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn proof_receipt_rows(repo: &Path, paths: &[&str]) -> Value {
    Value::Array(
        paths
            .iter()
            .map(|relative| {
                let bytes = fs::read(repo.join(relative)).expect("read predecessor proof receipt");
                json!({
                    "byte_length": bytes.len(),
                    "path": relative,
                    "sha256": sha256_hex(&bytes),
                })
            })
            .collect(),
    )
}

fn copy_workspace_file(repo: &Path, workspace: &Path, relative: &str) {
    let destination = workspace.join(relative);
    fs::create_dir_all(destination.parent().expect("workspace file parent"))
        .expect("create mutant workspace parent");
    fs::copy(repo.join(relative), destination).expect("copy mutant workspace file");
}

fn assert_rejected_by_both(repo: &Path, name: &str, mutate: impl FnOnce(&mut Value)) {
    let temporary = TemporaryRoot::new(name);
    let root = temporary.0.join("domain");
    copy_tree(&repo.join("contracts/vnext/stage3/domain"), &root);
    let manifest_path = root.join("domain-kernel.v1.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read Stage 3 manifest"))
            .expect("parse Stage 3 manifest");
    mutate(&mut manifest);
    write_json(&manifest_path, &manifest);

    let root_arg = root.to_str().expect("UTF-8 mutant Stage 3 root");
    for (program, script) in [
        ("python3", "tools/vnext_contracts/stage3/domain/validate.py"),
        ("ruby", "tools/vnext_contracts/stage3/domain/verify.rb"),
    ] {
        let output = run(
            repo,
            program,
            &[script, "--root", root_arg, "--artifact-only"],
        );
        assert!(
            !output.status.success(),
            "{program} accepted Stage 3 mutant {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn stage3_certification_receipts_bind_full_predecessor_chain() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stage0 =
        read_json(&repo.join("contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"));
    let stage2 =
        read_json(&repo.join("contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json"));
    let expected_chain = json!({
        "mode": "full_chain",
        "stage0": {
            "proof_receipts": proof_receipt_rows(repo, &[
                "contracts/vnext/stage0/effect-home/encoder-receipt.json",
                "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
            ]),
            "semantic_root": stage0["identity"],
            "source_tree_root": format!("sha256:{}", stage2["stage0_tree_sha256"].as_str().expect("Stage 0 tree root")),
        },
        "stage2": {
            "proof_receipts": proof_receipt_rows(repo, &[
                "contracts/vnext/stage2/authority/python-encoder-receipt.v1.json",
                "contracts/vnext/stage2/authority/semantic-validation-receipt.v1.json",
                "contracts/vnext/stage2/authority/ruby-verification-receipt.v1.json",
            ]),
            "semantic_root": format!("sha256:{}", stage2["root_id"].as_str().expect("Stage 2 semantic root")),
        },
    });
    for relative in [
        "contracts/vnext/stage3/domain/python-encoder-receipt.v1.json",
        "contracts/vnext/stage3/domain/semantic-validation-receipt.v1.json",
        "contracts/vnext/stage3/domain/ruby-verification-receipt.v1.json",
    ] {
        let receipt = read_json(&repo.join(relative));
        assert_eq!(receipt["validation_mode"], "full_chain", "{relative}");
        assert_eq!(receipt["predecessor_chain"], expected_chain, "{relative}");
    }
}

#[test]
fn stage3_artifact_only_validation_mints_no_certification_receipt() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (program, script, receipt_name) in [
        (
            "python3",
            "tools/vnext_contracts/stage3/domain/validate.py",
            "semantic-validation-receipt.v1.json",
        ),
        (
            "ruby",
            "tools/vnext_contracts/stage3/domain/verify.rb",
            "ruby-verification-receipt.v1.json",
        ),
    ] {
        let temporary = TemporaryRoot::new(receipt_name);
        let root = temporary.0.join("domain");
        copy_tree(&repo.join("contracts/vnext/stage3/domain"), &root);
        fs::remove_file(root.join(receipt_name)).expect("remove copied certification receipt");
        let root_arg = root.to_str().expect("UTF-8 artifact-only root");
        let output = run(
            repo,
            program,
            &[script, "--root", root_arg, "--artifact-only"],
        );
        assert!(
            output.status.success(),
            "{program} artifact-only validation failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !root.join(receipt_name).exists(),
            "{program} minted a normal certification receipt after skipping the predecessor chain"
        );
    }
}

#[test]
fn stage3_build_check_rejects_skipped_certification_mode() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("skipped-certification-mode");
    let root = temporary.0.join("domain");
    copy_tree(&repo.join("contracts/vnext/stage3/domain"), &root);
    let receipt_path = root.join("semantic-validation-receipt.v1.json");
    let mut receipt = read_json(&receipt_path);
    receipt["validation_mode"] = json!("artifact_only");
    receipt["predecessor_chain"]["mode"] = json!("skipped");
    write_json(&receipt_path, &receipt);

    let output = run(
        repo,
        "python3",
        &[
            "tools/vnext_contracts/stage3/domain/build.py",
            "--check",
            "--root",
            root.to_str().expect("UTF-8 skipped-mode root"),
        ],
    );
    assert!(
        !output.status.success(),
        "Stage 3 build check accepted a skipped-mode certification receipt"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("semantic-validation-receipt.v1.json"),
        "Stage 3 build check rejected skipped mode for the wrong reason\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stage3_source_closure_rejects_transitive_semantic_dependency_mutations() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "src/domain/contract/root.rs",
        "src/foundation/core/deterministic_cbor.rs",
    ] {
        let temporary = TemporaryRoot::new("transitive-source-mutation");
        let workspace = temporary.0.join("workspace");
        for source in EXPECTED_STAGE3_SOURCES {
            copy_workspace_file(repo, &workspace, source);
        }
        copy_tree(
            &repo.join("contracts/vnext/stage3/domain"),
            &workspace.join("contracts/vnext/stage3/domain"),
        );
        let source_path = workspace.join(relative);
        let mut bytes = fs::read(&source_path).expect("read transitive semantic dependency");
        bytes.extend_from_slice(b"\n// Stage 3 omitted-source semantic mutation\n");
        fs::write(&source_path, bytes).expect("mutate transitive semantic dependency");

        for (program, script) in [
            ("python3", "tools/vnext_contracts/stage3/domain/validate.py"),
            ("ruby", "tools/vnext_contracts/stage3/domain/verify.rb"),
        ] {
            let output = run(
                &workspace,
                program,
                &[
                    script,
                    "--root",
                    "contracts/vnext/stage3/domain",
                    "--artifact-only",
                ],
            );
            assert!(
                !output.status.success(),
                "{program} accepted a mutation to transitive Stage 3 source {relative}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("semantic"),
                "{program} rejected transitive Stage 3 source {relative} for the wrong reason\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn stage3_proof_check_rejects_cfg_disabled_canonical_compilation_ancestry() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("cfg-disabled-vnext-export");
    let workspace = temporary.0.join("workspace");
    for source in EXPECTED_STAGE3_SOURCES {
        copy_workspace_file(repo, &workspace, source);
    }
    copy_tree(
        &repo.join("contracts/vnext/stage3/domain"),
        &workspace.join("contracts/vnext/stage3/domain"),
    );
    let domain_root = workspace.join("src/domain/mod.rs");
    let source = fs::read_to_string(&domain_root).expect("read copied domain compilation root");
    assert!(!source.contains("pub mod vnext;"));
    assert!(source.contains("pub mod contract;"));
    fs::write(
        &domain_root,
        source.replace("pub mod contract;", "#[cfg(any())]\npub mod contract;"),
    )
    .expect("cfg-disable copied canonical contract export");

    for (program, script) in [
        ("python3", "tools/vnext_contracts/stage3/domain/validate.py"),
        ("ruby", "tools/vnext_contracts/stage3/domain/verify.rb"),
    ] {
        let output = run(
            &workspace,
            program,
            &[
                script,
                "--root",
                "contracts/vnext/stage3/domain",
                "--artifact-only",
            ],
        );
        assert!(
            !output.status.success(),
            "{program} accepted a cfg-disabled src/domain/mod.rs canonical contract export"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("semantic"),
            "{program} rejected the cfg-disabled canonical contract export for the wrong reason\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn stage3_artifact_is_identity_bound_to_exact_semantics_and_sources() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = repo.join("contracts/vnext/stage3/domain");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join("domain-kernel.v1.json")).expect("read Stage 3 manifest"),
    )
    .expect("parse Stage 3 manifest");
    assert_eq!(
        manifest["schema_version"],
        "maestro.vnext.stage3.domain-kernel.v1"
    );
    assert_eq!(manifest["publication_state"], "inactive_candidate");
    let canonical = manifest["canonical_value"]
        .as_array()
        .expect("canonical Stage 3 array");
    assert_eq!(
        canonical[3],
        json!([
            [
                "Work",
                "work",
                [
                    "identity",
                    "revision",
                    "lifecycle",
                    "submission",
                    "requirement",
                    "relation"
                ]
            ],
            [
                "Contract",
                "contract",
                [
                    "revision",
                    "generation",
                    "semantic-publication-request",
                    "current-root"
                ]
            ],
            [
                "Step",
                "step",
                [
                    "identity",
                    "revision",
                    "binding",
                    "dag",
                    "lifecycle",
                    "submission",
                    "amendment"
                ]
            ],
            [
                "Design",
                "design",
                [
                    "source-binding",
                    "revision",
                    "slot-manifest",
                    "reconciliation"
                ]
            ],
            [
                "Decision",
                "design",
                [
                    "revision",
                    "alternative",
                    "resolution",
                    "materialization",
                    "lineage",
                    "batch"
                ]
            ],
            [
                "Evidence",
                "evidence",
                [
                    "claim",
                    "observation-reference",
                    "submission-reference",
                    "claim-subject",
                    "no-lifecycle"
                ]
            ],
            [
                "Repository Store",
                "repository",
                [
                    "publication-boundary",
                    "authority-admission",
                    "atomic-owner-joined-commit",
                    "idempotent-replay",
                    "stale-basis-refusal"
                ]
            ]
        ])
    );
    let invariants = canonical[8].as_array().expect("Stage 3 invariants");
    for invariant in REQUIRED_STAGE3_INVARIANTS {
        assert!(
            invariants.iter().any(|value| value == invariant),
            "missing Stage 3 invariant {invariant}"
        );
    }
    let action_catalog = canonical[9]
        .as_array()
        .expect("Stage 3 Action catalog closure");
    assert_eq!(
        action_catalog[0],
        "maestro.vnext.stage3.repository-action-catalog-closure.v1"
    );
    assert_eq!(action_catalog[3], 1, "Stage 3 Action protocol revision");
    assert_eq!(
        action_catalog[4],
        json!([
            [
                "implemented",
                1,
                "CreateDraftWork",
                "56ded201d62fbb94486581d13cc6a086b3e114ad889aa1a954841f7f646afc40",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                1
            ],
            [
                "implemented",
                2,
                "CancelWork",
                "b58d2fecb0f1b27146884f85847cb1b22575b32d8d6e92efe6608cf582420615",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                2
            ],
            [
                "deferred_stage5",
                3,
                "CompleteWork",
                "163de9814514910c9ca1d5b1f76ac982e0788bd8a81025eff5c551a0a923b5d2",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                3
            ],
            [
                "catalogued_deferred_stage4_fail_closed",
                4,
                "AbsorbWork",
                "4fb2d35f4bd7c2169bec6bc51af840f325c419c28de0041d60b69bc4691125ea",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                4
            ],
            [
                "deferred_stage5",
                5,
                "SubmitWorkCompletion",
                "7d8083d10f75348f805e89e8fcd5f27d81face12d2d8ae1047a01c53fbfb2803",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                5
            ],
            [
                "deferred_stage5",
                6,
                "RejectWorkCompletion",
                "d03d0a753eb0821b43f002de3eab1afb32a7ede75ff0a3588b0718292c0d7b3f",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                6
            ],
            [
                "deferred_stage5",
                7,
                "ReturnWorkForRepair",
                "2fbc3d51f0b750cb9a1292d404ada520c17fac6c9fd960350188d97f3b6acc0b",
                1,
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee",
                7
            ],
            [
                "deferred_stage4",
                8,
                "SubmitStep",
                "c5a7079af2dafa9acc956477b3004b5fb21dd688b4022677416164c4485f96d6",
                2,
                "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e",
                1
            ],
            [
                "deferred_stage5",
                9,
                "SatisfyStep",
                "a5887f9b87af5f6a5f466df222b3a76b2eca5762b33a5694b4ebcb61b3db127e",
                2,
                "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e",
                2
            ],
            [
                "deferred_stage5",
                10,
                "RejectStepSubmission",
                "b75460c72c4907e2893bb48559b2c19d99ecb4d27ab43a10adda4ee95dfbc62a",
                2,
                "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e",
                3
            ],
            [
                "deferred_stage5",
                11,
                "RecoverStepSubmission",
                "130cb3ecfd8146ba869de9a1198b3c3b6b67b2c06101cecf62a955db5b587e13",
                2,
                "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e",
                4
            ],
            [
                "implemented",
                12,
                "PublishInitialContract",
                "5c3bf7e45cc2e8348bb5a6ce403cf6d14f718c7ef4514ca01e60370174387234",
                3,
                "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab",
                1
            ],
            [
                "implemented",
                13,
                "AmendContract",
                "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4",
                3,
                "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab",
                2
            ],
            [
                "implemented",
                15,
                "AppendDesignRevision",
                "4235a07743d0fa3557f612b0d4dd499afcd02adadcc92facfcbc806645a99e83",
                4,
                "85aad446bae62f47851f719f74296bd2576f30894b95ccb4b3b0c59790a80dc5",
                2
            ],
            [
                "implemented",
                20,
                "ResolveDecision",
                "4e05d1c7d9314a843d43538c399ece7df7da52793062c7ca43805e8f763f75ac",
                5,
                "a3d6c9c0dcd9b5e3447cf4dc45edf5d1b338c99dfc27a61df23966b7514ae9dc",
                3
            ],
            [
                "deferred_stage4",
                23,
                "AcquireStepExecution",
                "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2",
                6,
                "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292",
                1
            ]
        ])
    );
    let action_rows = action_catalog[4].as_array().expect("Stage 3 Action rows");
    assert_eq!(action_rows.len(), 16);
    assert!(action_rows.iter().all(|row| {
        matches!(
            row.as_array().and_then(|fields| fields[0].as_str()),
            Some(
                "implemented"
                    | "catalogued_deferred_stage4_fail_closed"
                    | "deferred_stage4"
                    | "deferred_stage5"
            )
        )
    }));
    assert_eq!(
        action_rows
            .iter()
            .filter(|row| {
                row.as_array().and_then(|fields| fields[0].as_str()) == Some("implemented")
            })
            .count(),
        6,
        "Stage 3 exposes exactly six executable Repository leaves"
    );
    assert_eq!(
        action_rows
            .iter()
            .filter_map(|row| {
                let fields = row.as_array()?;
                matches!(
                    fields[0].as_str(),
                    Some("implemented" | "catalogued_deferred_stage4_fail_closed")
                )
                .then(|| fields[2].as_str())
                .flatten()
            })
            .collect::<Vec<_>>(),
        [
            "CreateDraftWork",
            "CancelWork",
            "AbsorbWork",
            "PublishInitialContract",
            "AmendContract",
            "AppendDesignRevision",
            "ResolveDecision",
        ],
        "Stage 3 preserves the exact seven-member catalog while AbsorbWork remains fail closed"
    );
    assert!(action_rows.iter().all(|row| {
        let fields = row.as_array().expect("Stage 3 Action row");
        !matches!(
            fields[2].as_str(),
            Some("MaterializeDecision" | "CancelStep" | "SupersedeStep" | "DesignAppend")
        )
    }));
    let store_schemas = canonical[10]
        .as_array()
        .expect("Stage 3 Repository Store schema closure");
    assert_eq!(
        store_schemas[0],
        "maestro.vnext.stage3.repository-store-schema-closure.v1"
    );
    let schema_rows = store_schemas[1]
        .as_array()
        .expect("Stage 3 Repository Store schema rows");
    assert_eq!(schema_rows.len(), 14);
    assert_eq!(
        store_schemas[1],
        json!([
            [
                1,
                "ActionRequest",
                "maestro.vnext.repository-action-request.v1",
                "c512faedbf87869f531be18baad9674652c5d83ab4ab76bc555330604e5791cd"
            ],
            [
                2,
                "WorkRecord",
                "maestro.vnext.repository-work-record.v1",
                "03e178d89683e4c93528e1d6ed7c900d0da8957024e729b3661350b6c397cf37"
            ],
            [
                3,
                "DesignStream",
                "maestro.vnext.repository-design-stream.v1",
                "8686ba5cc854ff7cb408e322817f173d6db93e99d492bea9bac3aa344b9f5537"
            ],
            [
                4,
                "ContractRevision",
                "maestro.vnext.repository-contract-revision.v1",
                "221f327f2b29497599f943dd4c6fdea0d328916b7f2e7269c1cda6f81d4a12c0"
            ],
            [
                5,
                "ContractGeneration",
                "maestro.vnext.repository-contract-generation.v1",
                "d5208061bd3aa95917b36ab2e55439365987286bc594e09ba3a327b3601a6cc7"
            ],
            [
                6,
                "DesignFinalizationManifest",
                "maestro.vnext.repository-design-finalization-manifest.v1",
                "5ac8298b026b63f4548121ffcf054fa68c30314cb491e67f3150a3f929550226"
            ],
            [
                7,
                "ContractRoot",
                "maestro.vnext.repository-contract-root.v1",
                "b6950c9de50712ae010468d1a1375ad2635967036972d07cdee0130286c42337"
            ],
            [
                8,
                "Decision",
                "maestro.vnext.repository-decision.v1",
                "604471128e0c03afa1ac53f72f34c2287f1f975b98effbd2a78569e3490c7fb3"
            ],
            [
                9,
                "StepGraph",
                "maestro.vnext.repository-step-graph.v1",
                "57bc1aca32b395c1fb43b6c960b38d5828aa71d3d63eac0f7cecf1887a1ed005"
            ],
            [
                10,
                "StepState",
                "maestro.vnext.repository-step-state.v1",
                "90158ef1e261ea84260ad5fd98deb8461b0655d3216c4993d0d949df1cfc6596"
            ],
            [
                11,
                "StepAmendmentAudit",
                "maestro.vnext.repository-step-amendment-audit.v1",
                "cf16f6af5205d093c58c63cc27fcd8c7314109ae2604e2baafe7473e74770bcf"
            ],
            [
                12,
                "DecisionMaterializationAudit",
                "maestro.vnext.repository-decision-materialization-audit.v1",
                "19f3f998c8e408e621bc46aafb0716b4fb0d7b61b721b1c9d177addc4c9a88c4"
            ],
            [
                13,
                "ExactEquivalenceReceipt",
                "maestro.vnext.repository-exact-equivalence-receipt.v1",
                "8108071f0749a887669d2537f9f7a95d7570e7588410a488e22c95dd73754fa8"
            ],
            [
                14,
                "ComponentInvalidationReceipt",
                "maestro.vnext.repository-component-invalidation-receipt.v1",
                "ebc6630651d0379159b7369f70b4570440254418dae527043e64cf252128ad23"
            ]
        ])
    );
    assert!(schema_rows.iter().all(|row| {
        let fields = row.as_array().expect("Stage 3 Repository Store schema row");
        !fields[2].as_str().is_some_and(|domain| {
            domain.contains("execution")
                || domain.contains("observation")
                || domain.contains("gate")
        })
    }));
    let sources = canonical[11].as_array().expect("Stage 3 source rows");
    let declared_sources = sources
        .iter()
        .map(|row| {
            row.as_array().expect("Stage 3 source row")[0]
                .as_str()
                .expect("source path")
        })
        .collect::<Vec<_>>();
    assert_eq!(declared_sources, EXPECTED_STAGE3_SOURCES);
    assert!(
        declared_sources
            .iter()
            .all(|relative| !relative.starts_with("src/domain/execution/")),
        "Stage 4 Execution sources remain explicitly deferred from Stage 3"
    );
    for row in sources {
        let row = row.as_array().expect("Stage 3 source row");
        let relative = row[0].as_str().expect("source path");
        let bytes = fs::read(repo.join(relative)).expect("read declared Stage 3 source");
        assert_eq!(row[1].as_u64(), Some(bytes.len() as u64));
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(row[2], digest);
    }
    assert_eq!(
        canonical[12],
        json!([
            "maestro.vnext.stage3.authority-successor.v1",
            "additive_over_frozen_stage2_and_public_predecessors",
            [
                [
                    23,
                    "OrdinaryBoundedGrantV1",
                    [
                        "complete_grant_definition",
                        "exact_parent_and_delegation",
                        "bounded_capacity_root",
                        "immutable"
                    ]
                ],
                [
                    24,
                    "OrdinaryGrantDelegationV1",
                    [
                        "exact_parent_child",
                        "same_context",
                        "same_capacity_root",
                        "immutable"
                    ]
                ]
            ],
            [
                ["AllocateGovernedCapacitySlot", "BootstrapControlG0"],
                ["EstablishConsumptionCellRoot", "BootstrapControlG0"],
                ["IssueRootAttachedBoundedGrant", "BootstrapControlG0"],
                ["ReissueRootAttachedGrantOneToOne", "OrdinaryLiveRuntime"],
                ["RevokeGrant", "OrdinaryLiveRuntime"]
            ],
            [
                "parentless_bounded_grant_refused",
                "unknown_or_cma_grant_action_basis_refused",
                "candidate_or_target_self_authorization_refused",
                "g0_issue_has_no_ordinary_capacity_debit",
                "reissue_and_revoke_require_separate_live_admin_grant",
                "reissue_and_revoke_spend_exactly_one_admin_capacity_unit"
            ]
        ])
    );

    let validator =
        fs::read_to_string(repo.join("tools/vnext_contracts/stage3/domain/validate.py"))
            .expect("read independent Python validator");
    assert!(
        !validator.contains("import build"),
        "Stage 3 Python validator must reconstruct semantics independently"
    );
    for relative in [
        "tools/vnext_contracts/stage3/domain/build.py",
        "tools/vnext_contracts/stage3/domain/validate.py",
        "tools/vnext_contracts/stage3/domain/verify.rb",
    ] {
        let proof_source =
            fs::read_to_string(repo.join(relative)).expect("read Stage 3 proof tool");
        assert!(
            proof_source.contains("tools/vnext_contracts/stage0/effect_home/build.py")
                && proof_source.contains("tools/vnext_contracts/stage2/authority/build.py")
                && proof_source.contains("tools/vnext_contracts/stage2/authority/validate.py"),
            "{relative} must reject a stale predecessor chain"
        );
    }

    for (program, script) in [
        ("python3", "tools/vnext_contracts/stage3/domain/validate.py"),
        ("ruby", "tools/vnext_contracts/stage3/domain/verify.rb"),
    ] {
        let output = run(repo, program, &[script]);
        assert!(
            output.status.success(),
            "{program} rejected Stage 3 contract\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn independent_stage3_proof_rejects_semantic_and_shape_mutants() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_rejected_by_both(repo, "owner-substitution", |manifest| {
        manifest["canonical_value"][3][0][0] = json!("HiddenWorkOwner");
    });
    assert_rejected_by_both(repo, "transition-substitution", |manifest| {
        manifest["canonical_value"][5][0][0][2] = json!("completed");
    });
    assert_rejected_by_both(repo, "grant-action-basis-substitution", |manifest| {
        manifest["canonical_value"][12][3][0][1] = json!("OrdinaryLiveRuntime");
    });
    assert_rejected_by_both(repo, "evidence-lifecycle-substitution", |manifest| {
        manifest["canonical_value"][3][5][2][4] = json!("lifecycle");
    });
    assert_rejected_by_both(repo, "repository-boundary-substitution", |manifest| {
        manifest["canonical_value"][3][6][2][0] = json!("adapter-publication-boundary");
    });
    assert_rejected_by_both(repo, "claim-invariant-omission", |manifest| {
        manifest["canonical_value"][8]
            .as_array_mut()
            .expect("Stage 3 invariants")
            .retain(|value| value != "claim_binds_exactly_one_submission");
    });
    assert_rejected_by_both(repo, "premature-satisfaction-carry", |manifest| {
        manifest["canonical_value"][8]
            .as_array_mut()
            .expect("Stage 3 invariants")
            .retain(|value| {
                value
                    != "stage3_satisfaction_carry_unavailable_until_canonical_evidence_gate_material"
            });
    });
    assert_rejected_by_both(repo, "source-closure-omission", |manifest| {
        manifest["canonical_value"][11]
            .as_array_mut()
            .expect("Stage 3 source rows")
            .pop();
    });
    assert_rejected_by_both(repo, "invented-repository-action", |manifest| {
        manifest["canonical_value"][9][4][13][2] = json!("DesignAppend");
    });
    assert_rejected_by_both(repo, "action-row-reordering", |manifest| {
        manifest["canonical_value"][9][4]
            .as_array_mut()
            .expect("Action rows")
            .swap(0, 1);
    });
    assert_rejected_by_both(repo, "action-status-substitution", |manifest| {
        manifest["canonical_value"][9][4][0][0] = json!("implemented_later");
    });
    assert_rejected_by_both(repo, "absorb-work-premature-implementation", |manifest| {
        manifest["canonical_value"][9][4][3][0] = json!("implemented");
    });
    assert_rejected_by_both(repo, "action-descriptor-duplication", |manifest| {
        manifest["canonical_value"][9][4][1][3] = manifest["canonical_value"][9][4][0][3].clone();
    });
    assert_rejected_by_both(repo, "premature-execution-surface", |manifest| {
        manifest["canonical_value"][9][4][15][0] = json!("implemented");
    });
    assert_rejected_by_both(repo, "action-protocol-substitution", |manifest| {
        manifest["canonical_value"][9][3] = json!(2);
    });
    assert_rejected_by_both(repo, "surrogate-execution-schema", |manifest| {
        manifest["canonical_value"][10][1][8][2] =
            json!("maestro.vnext.repository-execution-run.v1");
    });
    assert_rejected_by_both(repo, "store-schema-row-reordering", |manifest| {
        manifest["canonical_value"][10][1]
            .as_array_mut()
            .expect("Store schema rows")
            .swap(0, 1);
    });
    assert_rejected_by_both(repo, "store-schema-ordinal-duplication", |manifest| {
        manifest["canonical_value"][10][1][1][0] = json!(1);
    });
    assert_rejected_by_both(repo, "unknown-field", |manifest| {
        manifest["shadow_scheduler"] = json!(true);
    });
}

#[test]
fn stage3_proof_rejects_a_stale_action_descriptor_envelope() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("stale-action-descriptor-envelope");
    let workspace = temporary.0.join("workspace");
    for relative in [
        "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
        "contracts/vnext/public/setup_operation_compatibility.v1.json",
        "tools/vnext_contracts/catalogs/cbor_py.py",
        "tools/vnext_contracts/stage3/domain/build.py",
        "tools/vnext_contracts/stage3/domain/validate.py",
        "tools/vnext_contracts/stage3/domain/verify.rb",
    ] {
        copy_workspace_file(repo, &workspace, relative);
    }
    copy_tree(
        &repo.join("contracts/vnext/stage3/domain"),
        &workspace.join("contracts/vnext/stage3/domain"),
    );
    let catalog_path =
        workspace.join("contracts/vnext/catalogs/generated/catalog-09-action-spec.json");
    let mut catalog: Value =
        serde_json::from_slice(&fs::read(&catalog_path).expect("read mutant ActionSpec catalog"))
            .expect("parse mutant ActionSpec catalog");
    catalog["descriptors"][0]["value"][3] = json!(2);
    write_json(&catalog_path, &catalog);
    let mutated_catalog_bytes = fs::read(&catalog_path).expect("read encoded mutant catalog");
    let mutated_catalog_digest = Sha256::digest(&mutated_catalog_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let setup_path = workspace.join("contracts/vnext/public/setup_operation_compatibility.v1.json");
    let mut setup: Value =
        serde_json::from_slice(&fs::read(&setup_path).expect("read mutant setup binding"))
            .expect("parse mutant setup binding");
    setup["catalog_bindings"]["action_spec_file_sha256"] = json!(mutated_catalog_digest);
    write_json(&setup_path, &setup);

    for (program, script) in [
        ("python3", "tools/vnext_contracts/stage3/domain/build.py"),
        ("python3", "tools/vnext_contracts/stage3/domain/validate.py"),
        ("ruby", "tools/vnext_contracts/stage3/domain/verify.rb"),
    ] {
        let output = run(&workspace, program, &[script, "--catalog-only"]);
        assert!(
            !output.status.success(),
            "{program} accepted an ActionSpec value whose frozen envelope and identity were stale\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("frozen ActionSpec row drifted for 1:CreateDraftWork"),
            "{program} failed for the wrong reason while checking the stale ActionSpec envelope\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
