use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use sha2::{Digest, Sha256};

use maestro::foundation::core::deterministic_cbor;

const STAGE4_SOURCE_ARCHIVE_LENGTH: usize = 16_486_231;
const STAGE4_SOURCE_ARCHIVE_SHA256: &str =
    "347eaf928f81d9ce6e07e3767f0cdaf2cde23cd98d13bad41b745d5fbc359910";
const STAGE4_SOURCE_COMMIT: &str = "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2";
const STAGE4_SOURCE_TREE: &str = "2f832a04c7109e17b4b298e40b4827c1ced2d527";
const CERTIFIED_STAGE5_COMMIT: &str = "527f7b2687a7d51737dc3e6e0c02dfdb6d6f611a";
const CERTIFIED_STAGE5_TREE: &str = "ebc01e90cd4f4bd9452662251f5252513358b86c";
const CERTIFIED_STAGE5_POINTER_PATH: &str =
    "contracts/vnext/stage5/evidence-gates/current-proof.json";
const CERTIFIED_STAGE5_RELEASE_IDENTITY: &str =
    "sha256:7c0a4aab9f2fdc8989c1affc9818ce6235ef9338008f8d00d53ad2d4022940c6";
const CERTIFIED_STAGE5_RELEASE_OBJECT: &str = "contracts/vnext/stage5/evidence-gates/releases/objects/7c0a4aab9f2fdc8989c1affc9818ce6235ef9338008f8d00d53ad2d4022940c6";
const CERTIFIED_STAGE5_PLAN_IDENTITY: &str =
    "sha256:4e1cc2633a93645c457f78f326b155be44e8f1b3f267098e43c43b0c44f8296c";
const CERTIFIED_STAGE5_SNAPSHOT_IDENTITY: &str =
    "sha256:e76ae1421bb871b9f35edb23eec7a7b510d07f12d25df1ffa36d62abae8f7ece";

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn stage5_behavior_manifest_rejects_missing_or_reordered_normal_runs() {
    let mut runs = [
        "assessment-kernel",
        "submission-evidence-join",
        "authorized-evidence-store",
        "work-completion-boundary",
        "claim-contracts",
        "submission-claim-carrier",
        "evidence-gate-contracts",
        "diagnostic-architecture",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, label)| {
        let name = format!("fixture-{index}");
        serde_json::json!({
            "label": label,
            "tests": [{
                "command": ["maestro", name, "--exact", "--nocapture"],
                "name": name,
                "result": "pass"
            }]
        })
    })
    .collect::<Vec<_>>();
    runs.push(serde_json::json!({"label": "same-count-substitution-mutant"}));
    assert_eq!(exact_behavior_manifest_rows(&runs).unwrap().len(), 8);

    let mut missing_run = runs.clone();
    missing_run.remove(0);
    assert!(exact_behavior_manifest_rows(&missing_run).is_none());
    let mut reordered_runs = runs.clone();
    reordered_runs.swap(0, 1);
    assert!(exact_behavior_manifest_rows(&reordered_runs).is_none());
}

#[test]
fn published_stage5_three_engine_receipts_bind_one_inactive_artifact() {
    let commit_spec = format!("{CERTIFIED_STAGE5_COMMIT}^{{commit}}");
    let tree_spec = format!("{CERTIFIED_STAGE5_COMMIT}^{{tree}}");
    assert_eq!(
        git_text(&["rev-parse", &commit_spec]),
        CERTIFIED_STAGE5_COMMIT
    );
    assert_eq!(git_text(&["rev-parse", &tree_spec]), CERTIFIED_STAGE5_TREE);

    let pointer_path = workspace().join(CERTIFIED_STAGE5_POINTER_PATH);
    assert!(
        pointer_path.is_file(),
        "the published Stage 5 contract requires its exact proof pointer"
    );
    let pointer_bytes = fs::read(&pointer_path).unwrap();
    assert_eq!(
        pointer_bytes,
        certified_blob(CERTIFIED_STAGE5_POINTER_PATH),
        "the live pointer must remain byte-identical to the certified commit"
    );
    let pointer: Value = serde_json::from_slice(&pointer_bytes).unwrap();
    assert_eq!(
        pointer,
        serde_json::json!({
            "object": format!(
                "objects/{}",
                CERTIFIED_STAGE5_RELEASE_IDENTITY
                    .strip_prefix("sha256:")
                    .unwrap()
            ),
            "release_identity": CERTIFIED_STAGE5_RELEASE_IDENTITY,
            "schema_version": "maestro.vnext.proof-publication-pointer.v1",
        })
    );
    let release_identity = pointer["release_identity"].as_str().unwrap();
    assert_eq!(release_identity, CERTIFIED_STAGE5_RELEASE_IDENTITY);
    let releases = workspace().join("contracts/vnext/stage5/evidence-gates/releases");
    let release_object = releases
        .join("objects")
        .join(release_identity.strip_prefix("sha256:").unwrap());
    for component in [&releases, &releases.join("objects"), &release_object] {
        assert!(
            !fs::symlink_metadata(component)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    assert_content_addressed_tree(&release_object);
    assert_certified_release_tree(&release_object, CERTIFIED_STAGE5_RELEASE_OBJECT);
    let release_root = release_object.join("payload");
    assert!(release_root.is_dir());

    let artifact = read_json(release_root.join("evidence-gates.v1.json"));
    let artifact_cbor = fs::read(release_root.join("evidence-gates.v1.cbor")).unwrap();
    let builder_bytes = fs::read(release_root.join("python-builder-receipt.v1.json")).unwrap();
    let validator_bytes =
        fs::read(release_root.join("semantic-validation-receipt.v1.json")).unwrap();
    let ruby_bytes = fs::read(release_root.join("ruby-verification-receipt.v1.json")).unwrap();
    let builder: Value = serde_json::from_slice(&builder_bytes).unwrap();
    let validator: Value = serde_json::from_slice(&validator_bytes).unwrap();
    let ruby: Value = serde_json::from_slice(&ruby_bytes).unwrap();
    let consensus = read_json(release_root.join("three-engine-consensus-receipt.v1.json"));
    let harness = read_json(release_root.join("proof-harness-receipt.v1.json"));
    let predecessor = read_json(release_root.join("predecessor-closure.v1.json"));
    let predecessor_source = fs::read(release_root.join("stage4-source.tar.gz")).unwrap();
    let toolchain_path = release_root.join("rust-toolchain-closure.v1.json");
    let toolchain_bytes = fs::read(&toolchain_path).unwrap();
    let toolchain = read_json(toolchain_path.clone());
    let snapshot_manifest = read_json(release_root.join("workspace-snapshot-manifest.v1.json"));
    let release = read_json(
        release_root
            .parent()
            .expect("release payload has a release root")
            .join("release.json"),
    );
    assert_eq!(release["identity"], release_identity);
    assert_eq!(
        format!(
            "sha256:{}",
            sha256(&canonical_json(&release["canonical_value"]))
        ),
        release_identity
    );
    assert_eq!(
        pointer["object"],
        format!(
            "objects/{}",
            release_identity.strip_prefix("sha256:").unwrap()
        )
    );
    assert_eq!(
        release["canonical_value"]["payload_manifest"],
        tree_manifest(&release_root)
    );
    let identity = artifact["artifact_id"].as_str().unwrap();

    assert_eq!(
        artifact_cbor.len(),
        artifact["byte_length"].as_u64().unwrap() as usize
    );
    assert_eq!(sha256(&artifact_cbor), identity);
    assert_eq!(hex(&artifact_cbor), artifact["cbor_hex"].as_str().unwrap());
    let decoded_artifact = deterministic_cbor::decode(&artifact_cbor).unwrap();
    assert_eq!(
        deterministic_cbor::encode(&decoded_artifact).unwrap(),
        artifact_cbor
    );

    assert_eq!(artifact["publication_state"], "inactive_candidate");
    assert_eq!(artifact["diagnostic_proof_claim"], "test_adapter_only");
    let source_paths = artifact["source_closure"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row[0].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
        "src/domain/vnext/authority/downstream_action_basis.rs",
        "src/domain/vnext/authority/facade_tests.rs",
        "src/domain/vnext/integration/mod.rs",
        "src/domain/vnext/integration/trusted_host_diagnostic.rs",
        "src/domain/vnext/persistence/protected_diagnostic.rs",
        "tests/architecture_imports.rs",
    ] {
        assert!(
            source_paths.contains(required),
            "missing Stage 5 source {required}"
        );
    }
    assert_eq!(
        artifact["behavior_manifest_identity"],
        "sha256:7647ace03d25f7d57fecc4cfcb93e5c2eaa5982a91fdb94778a3cb752e8e711e"
    );
    assert_eq!(artifact["observation_kinds"].as_array().unwrap().len(), 43);
    assert_eq!(
        artifact["protocol"]["gate_results"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        artifact["protocol"]["gate_input_classes"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        artifact["protocol"]["gate_operators"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        artifact["invalidation_reasons"].as_array().unwrap().len(),
        9
    );
    let source_closure_sha256 = sha256(&canonical_json(&artifact["source_closure"]));
    let engine_receipts = [
        (
            &builder,
            "maestro.vnext.stage5.python-builder-receipt.v1",
            "builder_sha256",
        ),
        (
            &validator,
            "maestro.vnext.stage5.semantic-validation-receipt.v1",
            "validator_sha256",
        ),
        (
            &ruby,
            "maestro.vnext.stage5.ruby-verification-receipt.v1",
            "verifier_sha256",
        ),
    ];
    for (receipt, schema, engine_hash_key) in engine_receipts {
        let expected_keys = [
            "artifact_id",
            "artifact_sha256",
            "behavior_manifest_identity",
            "behavior_passed",
            "behavior_runs",
            "diagnostic_proof_claim",
            engine_hash_key,
            "publication_state",
            "receipt_identity",
            "schema_version",
            "source_closure_sha256",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(
            receipt
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected_keys
        );
        assert_eq!(receipt["schema_version"], schema);
        assert_eq!(receipt["source_closure_sha256"], source_closure_sha256);
        assert_eq!(
            receipt["behavior_manifest_identity"],
            "sha256:7647ace03d25f7d57fecc4cfcb93e5c2eaa5982a91fdb94778a3cb752e8e711e"
        );
        assert_eq!(receipt["artifact_id"], identity);
        assert_eq!(receipt["behavior_passed"], 73);
        assert_eq!(receipt["behavior_runs"].as_array().unwrap().len(), 9);
        assert_eq!(
            receipt["behavior_runs"][8]["label"],
            "same-count-substitution-mutant"
        );
        assert_eq!(receipt["behavior_runs"][8]["rejected"], true);
        assert_eq!(receipt["publication_state"], "inactive_candidate");
        assert_eq!(receipt["diagnostic_proof_claim"], "test_adapter_only");
        let mut identity_value = receipt.clone();
        let receipt_identity = identity_value
            .as_object_mut()
            .unwrap()
            .remove("receipt_identity")
            .unwrap();
        assert_eq!(
            receipt_identity,
            format!("sha256:{}", sha256(&canonical_json(&identity_value)))
        );
        assert!(
            receipt["behavior_runs"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|run| {
                    run.get("tests")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .all(|test| test.get("stdout_sha256").is_none()
                    && test.get("stderr_sha256").is_none())
        );
    }
    let builder_runs = builder["behavior_runs"].as_array().unwrap();
    let behavior_rows = exact_behavior_manifest_rows(builder_runs)
        .expect("the eight normal runs and terminal mutant must be exact");
    assert_eq!(behavior_rows.len(), 73);
    assert_eq!(
        behavior_rows
            .iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        73
    );
    assert_eq!(
        format!(
            "sha256:{}",
            sha256(&canonical_json(&Value::Array(behavior_rows)))
        ),
        "sha256:7647ace03d25f7d57fecc4cfcb93e5c2eaa5982a91fdb94778a3cb752e8e711e"
    );
    let builder_semantics = semantic_behavior_runs(&builder["behavior_runs"]);
    let validator_semantics = semantic_behavior_runs(&validator["behavior_runs"]);
    let ruby_semantics = semantic_behavior_runs(&ruby["behavior_runs"]);
    assert_eq!(builder_semantics, validator_semantics);
    assert_eq!(builder_semantics, ruby_semantics);
    for (name, bytes) in [
        ("builder", &builder_bytes),
        ("validator", &validator_bytes),
        ("ruby", &ruby_bytes),
    ] {
        assert!(
            consensus["inputs"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!([name, bytes.len(), sha256(bytes)]))
        );
    }
    assert_eq!(consensus["artifact_id"], identity);
    let expected_consensus_keys = [
        "artifact_id",
        "behavior_manifest_identity",
        "behavior_passed",
        "consensus_identity",
        "diagnostic_proof_claim",
        "exact_behavior_receipt_sha256",
        "inputs",
        "predecessor_identity",
        "proof_harness_passed",
        "publication_state",
        "schema_version",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(
        consensus
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected_consensus_keys
    );
    let mut consensus_value = consensus.clone();
    let consensus_identity = consensus_value
        .as_object_mut()
        .unwrap()
        .remove("consensus_identity")
        .unwrap();
    assert_eq!(
        consensus_identity,
        format!("sha256:{}", sha256(&canonical_json(&consensus_value)))
    );
    assert_eq!(
        consensus["exact_behavior_receipt_sha256"],
        sha256(&canonical_json(&builder_semantics))
    );
    assert_eq!(consensus["behavior_passed"], 73);
    assert_eq!(
        consensus["behavior_manifest_identity"],
        "sha256:7647ace03d25f7d57fecc4cfcb93e5c2eaa5982a91fdb94778a3cb752e8e711e"
    );
    assert_eq!(consensus["proof_harness_passed"], 66);
    assert_eq!(consensus["diagnostic_proof_claim"], "test_adapter_only");
    assert_eq!(harness["passed"], 66);
    assert_eq!(harness["diagnostic_proof_claim"], "test_adapter_only");
    let harness_tests = harness["tests"].as_array().unwrap();
    assert_eq!(harness_tests.len(), 66);
    assert_eq!(
        harness_tests
            .iter()
            .map(|test| test.as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        66
    );
    assert_eq!(
        harness["manifest_identity"],
        "sha256:c5d8562805f5b655447d32f1262d4fc06e91c7a80ce9ccdeab4eb0c77e1188a1"
    );
    assert_eq!(
        harness["manifest_identity"],
        format!("sha256:{}", sha256(&canonical_json(&harness["tests"])))
    );
    assert_eq!(consensus["predecessor_identity"], predecessor["identity"]);
    assert_eq!(
        predecessor["identity"],
        "sha256:462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077"
    );
    assert_eq!(predecessor["files"].as_array().unwrap().len(), 6);
    assert_eq!(predecessor["source_commit"], STAGE4_SOURCE_COMMIT);
    assert_eq!(predecessor["source_tree"], STAGE4_SOURCE_TREE);
    assert_eq!(
        predecessor["source_archive_byte_length"],
        STAGE4_SOURCE_ARCHIVE_LENGTH
    );
    assert_eq!(
        predecessor["source_archive_sha256"],
        STAGE4_SOURCE_ARCHIVE_SHA256
    );
    assert_eq!(predecessor_source.len(), STAGE4_SOURCE_ARCHIVE_LENGTH);
    assert_eq!(sha256(&predecessor_source), STAGE4_SOURCE_ARCHIVE_SHA256);
    assert!(
        consensus["inputs"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!([
                "predecessor-source",
                STAGE4_SOURCE_ARCHIVE_LENGTH,
                STAGE4_SOURCE_ARCHIVE_SHA256,
            ]))
    );
    assert_eq!(
        toolchain["schema_version"],
        "maestro.vnext.stage5.rust-toolchain-closure.v1"
    );
    let toolchain_rows = toolchain["files"].as_array().unwrap();
    assert!(toolchain_rows.len() >= 3);
    assert_eq!(
        toolchain_rows
            .iter()
            .map(|row| row[0].as_str().unwrap())
            .collect::<Vec<_>>(),
        toolchain_rows
            .iter()
            .map(|row| row[0].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    for row in toolchain_rows {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 4);
        assert!(row[0].as_str().unwrap().starts_with("toolchain/"));
        assert!(row[1].as_u64().is_some());
        assert_eq!(row[2].as_str().unwrap().len(), 64);
        assert!(row[3].as_bool().is_some());
    }
    assert_eq!(
        toolchain["identity"],
        format!("sha256:{}", sha256(&canonical_json(&toolchain["files"])))
    );
    assert!(
        consensus["inputs"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!([
                "toolchain",
                toolchain_bytes.len(),
                sha256(&toolchain_bytes),
            ]))
    );
    assert_eq!(toolchain_bytes, canonical_json(&toolchain));
    let historical = &predecessor["historical_receipt_validation"];
    assert_eq!(
        historical["mode"],
        "read_only_commit_tree_content_and_receipt_equality"
    );
    assert_eq!(historical["source_commit"], STAGE4_SOURCE_COMMIT);
    assert_eq!(historical["source_tree"], STAGE4_SOURCE_TREE);
    assert_eq!(historical["receipt_count"], 4);
    assert_eq!(historical["receipts_report_pass"], true);
    assert_eq!(historical["archive_matches_source_commit"], true);
    assert_eq!(historical["canonical_files_match_archive"], true);
    assert!(predecessor.get("full_chain_reexecution").is_none());
    let frozen_source_rows = &snapshot_manifest["source_rows"];
    assert_eq!(frozen_source_rows.as_array().unwrap().len(), 1024);
    assert_eq!(
        snapshot_manifest["source_identity"],
        format!("sha256:{}", sha256(&canonical_json(frozen_source_rows)))
    );
    assert_eq!(
        snapshot_manifest["snapshot_identity"],
        CERTIFIED_STAGE5_SNAPSHOT_IDENTITY
    );
    assert_eq!(
        snapshot_manifest["schema_version"],
        "maestro.vnext.stage5.immutable-workspace-snapshot.v1"
    );
    let canonical = &release["canonical_value"];
    assert_eq!(canonical["plan_identity"], CERTIFIED_STAGE5_PLAN_IDENTITY);
    assert!(
        canonical["run_token"]
            .as_str()
            .unwrap()
            .starts_with("stage5-")
    );
    assert_eq!(canonical["phase_receipts"].as_array().unwrap().len(), 7);
    assert!(
        canonical["phase_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|phase| phase.get("cache_status").is_none())
    );
    assert_eq!(
        canonical["plan"]["schema_version"],
        "maestro.vnext.proof-engine.v1"
    );
    assert!(
        canonical["plan"]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|input| {
                input["value"]["name"] == "workspace-snapshot"
                    && input["value"]["path_identity"] == "content"
            })
    );
    assert!(
        canonical["plan"]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|input| {
                input["value"]["name"] == "sdk-root"
                    && input["value"]["kind"] == "symlink_tree"
                    && input["value"]["path_identity"] == "content"
            })
    );
}

#[test]
fn stage5_seal_parallelizes_independent_engines_and_validates_predecessors_read_only() {
    let seal =
        fs::read_to_string(workspace().join("tools/vnext_contracts/stage5/evidence_gates/seal.py"))
            .unwrap();
    let proof_engine =
        fs::read_to_string(workspace().join("tools/vnext_contracts/proof_engine/engine.py"))
            .unwrap();
    let behavior = fs::read_to_string(
        workspace().join("tools/vnext_contracts/stage5/evidence_gates/behavior.py"),
    )
    .unwrap();
    assert!(seal.contains("MAX_LOGICAL_WORKERS = 6"));
    assert!(seal.contains("MAX_COMPILE_WORKERS = 2"));
    assert!(seal.contains("default=MAX_LOGICAL_WORKERS"));
    assert!(seal.contains("max_workers=args.max_workers"));
    assert!(seal.contains("resource_limits={\"compile\": compile_workers}"));
    let builder_index = seal.find("name=\"builder\"").unwrap();
    let validator_index = seal.find("name=\"validator\"").unwrap();
    let ruby_index = seal.find("name=\"ruby\"").unwrap();
    let consensus_index = seal.find("name=\"consensus\"").unwrap();
    assert!(builder_index < validator_index);
    assert!(validator_index < ruby_index);
    assert!(ruby_index < consensus_index);
    let validator_phase = &seal[validator_index..ruby_index];
    let ruby_phase = &seal[ruby_index..consensus_index];
    for phase in [validator_phase, ruby_phase] {
        assert!(phase.contains("dependencies=(\"builder\", \"toolchain\")"));
        assert!(phase.contains("resource_class=\"compile\""));
        assert!(phase.contains("\"{phase_root}/out\""));
    }
    assert_eq!(
        seal.matches("dependencies=(\"builder\", \"toolchain\")")
            .count(),
        2
    );
    assert_eq!(seal.matches("resource_class=\"compile\"").count(), 3);
    assert!(!seal.contains("max_workers=1"));
    assert!(proof_engine.contains("phase_root = run_root / \"phases\" / phase.name / \"output\""));
    assert!(proof_engine.contains("phase_temp = phase_parent / \"tmp\""));
    assert!(seal.contains("(\"CARGO_TARGET_DIR\", \"{phase_temp}/cargo-target\")"));
    assert!(seal.contains("name=\"builder\""));
    assert!(seal.contains("name=\"validator\""));
    assert!(seal.contains("name=\"ruby\""));
    assert!(seal.contains("name=\"predecessor\""));
    assert!(seal.contains("name=\"consensus\""));
    assert!(seal.contains("name=\"harness\""));
    assert!(seal.contains("name=\"toolchain\""));
    assert!(seal.contains("three-engine-consensus-before-publication"));
    assert_eq!(seal.matches("cache_mode=\"content\"").count(), 2);
    assert_eq!(seal.matches("cache_mode=\"run\"").count(), 5);
    assert!(seal.contains("{dependency:toolchain}/out/toolchain/bin:/usr/bin:/bin"));
    assert!(seal.contains("InputBinding.file(\"git-bin\", git)"));
    assert!(seal.contains("\"{input:git-bin}\""));
    assert!(seal.contains("InputBinding.literal(\"rustc-driver-name\", rustc_driver.name)"));
    assert!(seal.contains("\"--driver-name\""));
    assert!(seal.contains("rustc_driver.name,"));
    assert!(!seal.contains("\"{input:rustc-driver-name}\""));
    assert!(seal.contains("predecessors/stage4-source.tar.gz"));
    assert!(seal.contains("out/stage4-source.tar.gz"));
    assert!(!seal.contains("fresh_full_chain_ancestor_behavior_and_compiled_mutant_reexecution"));
    assert!(behavior.contains("--exact"));
    assert!(behavior.contains("same-count-substitution-mutant"));
    assert!(!behavior.contains("stdout_sha256"));
    assert!(!behavior.contains("stderr_sha256"));
    assert!(!seal.contains("MAESTRO_VNEXT_STAGE5_PREPUBLICATION"));
    assert!(seal.contains("--resume-token"));
    assert!(seal.contains("secrets.token_hex(16)"));
    assert!(seal.contains("build_snapshot"));
    assert!(seal.contains("os.execve"));
    assert!(seal.contains("--immutable-snapshot"));
    assert!(seal.contains("--sdk-root"));
    assert!(seal.contains("(\"SDKROOT\", \"{input:sdk-root}\")"));
    assert!(seal.contains("workspace-snapshot-manifest.v1.json"));
    assert!(seal.contains("path_identity=\"content\""));
}

fn git_output(arguments: &[&str]) -> Vec<u8> {
    let inherited_git_variables = std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| key.to_string_lossy().starts_with("GIT_"))
        .collect::<Vec<_>>();
    let mut command = Command::new("git");
    command
        .arg("--no-replace-objects")
        .arg("--no-lazy-fetch")
        .arg("--no-optional-locks")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("core.commitGraph=false")
        .arg("-c")
        .arg("core.multiPackIndex=false")
        .arg("-c")
        .arg("gc.auto=0")
        .arg("-c")
        .arg("maintenance.auto=false")
        .arg("-C")
        .arg(workspace())
        .args(arguments);
    for key in inherited_git_variables {
        command.env_remove(key);
    }
    let output = command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        arguments,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn git_text(arguments: &[&str]) -> String {
    String::from_utf8(git_output(arguments))
        .unwrap()
        .trim_end()
        .to_owned()
}

fn certified_blob(path: &str) -> Vec<u8> {
    let object = format!("{CERTIFIED_STAGE5_COMMIT}:{path}");
    git_output(&["show", &object])
}

fn assert_certified_release_tree(root: &Path, repository_root: &str) {
    let listing = git_output(&[
        "ls-tree",
        "-r",
        "--full-tree",
        "-z",
        CERTIFIED_STAGE5_COMMIT,
        "--",
        repository_root,
    ]);
    let prefix = format!("{repository_root}/");
    let mut certified_entries = listing
        .split(|byte| *byte == 0)
        .filter(|row| !row.is_empty())
        .map(|row| {
            let tab = row.iter().position(|byte| *byte == b'\t').unwrap();
            let metadata = std::str::from_utf8(&row[..tab]).unwrap();
            let fields = metadata.split(' ').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3);
            assert!(matches!(fields[0], "100644" | "100755"));
            assert_eq!(fields[1], "blob");
            assert_eq!(fields[2].len(), 40);
            assert!(fields[2].bytes().all(|byte| byte.is_ascii_hexdigit()));
            let path = std::str::from_utf8(&row[tab + 1..]).unwrap();
            (
                path.strip_prefix(&prefix).unwrap().to_owned(),
                fields[0].to_owned(),
            )
        })
        .collect::<Vec<_>>();
    certified_entries.sort();

    let mut live_paths = Vec::new();
    collect_regular_file_paths(root, root, &mut live_paths);
    live_paths.sort();
    let certified_paths = certified_entries
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        live_paths, certified_paths,
        "the live release object path set must equal the certified commit"
    );
    for (relative, certified_mode) in certified_entries {
        let repository_path = format!("{repository_root}/{relative}");
        assert_eq!(
            fs::read(root.join(&relative)).unwrap(),
            certified_blob(&repository_path),
            "the live release object substituted certified bytes at {relative}"
        );
        let live_mode = fs::symlink_metadata(root.join(&relative))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            live_mode & 0o111 != 0,
            certified_mode == "100755",
            "the live release object substituted certified Git mode at {relative}"
        );
    }
}

fn collect_regular_file_paths(root: &Path, directory: &Path, paths: &mut Vec<String>) {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        if metadata.is_dir() {
            collect_regular_file_paths(root, &path, paths);
        } else {
            assert!(metadata.is_file());
            paths.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
        }
    }
}

fn read_json(path: PathBuf) -> Value {
    assert!(
        !fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn exact_behavior_manifest_rows(runs: &[Value]) -> Option<Vec<Value>> {
    const NORMAL_LABELS: [&str; 8] = [
        "assessment-kernel",
        "submission-evidence-join",
        "authorized-evidence-store",
        "work-completion-boundary",
        "claim-contracts",
        "submission-claim-carrier",
        "evidence-gate-contracts",
        "diagnostic-architecture",
    ];
    let (mutant, normal_runs) = runs.split_last()?;
    if normal_runs.len() != NORMAL_LABELS.len()
        || mutant.get("label")?.as_str()? != "same-count-substitution-mutant"
    {
        return None;
    }
    let mut rows = Vec::new();
    for (run, expected_label) in normal_runs.iter().zip(NORMAL_LABELS) {
        if run.get("label")?.as_str()? != expected_label {
            return None;
        }
        for test in run.get("tests")?.as_array()? {
            let command = test.get("command")?.as_array()?;
            if command.len() != 4
                || command[1] != *test.get("name")?
                || command[2] != "--exact"
                || command[3] != "--nocapture"
                || test.get("result")? != "pass"
            {
                return None;
            }
            rows.push(Value::Array(vec![
                command[0].clone(),
                test.get("name")?.clone(),
            ]));
        }
    }
    Some(rows)
}

fn semantic_behavior_runs(runs: &Value) -> Value {
    let mut projected = runs.as_array().unwrap().clone();
    assert!(projected.len() >= 2);
    let normal_count = projected.len() - 1;
    let mut binary_by_target = BTreeMap::<String, String>::new();
    let mut labels = BTreeSet::new();
    let mut total_passed = 0_u64;
    let mut first_exact = None;
    for run in &mut projected[..normal_count] {
        let label = run["label"].as_str().unwrap();
        assert!(valid_behavior_label(label));
        assert_ne!(label, "same-count-substitution-mutant");
        assert!(labels.insert(label.to_owned()));
        let tests = run["tests"].as_array().unwrap();
        assert!(!tests.is_empty());
        let passed = run["passed"].as_u64().unwrap();
        assert_eq!(passed, tests.len() as u64);
        total_passed += passed;
        let mut targets = BTreeSet::new();
        for test in tests {
            let name = test["name"].as_str().unwrap();
            let command = test["command"].as_array().unwrap();
            assert_eq!(command.len(), 4);
            let target = command[0].as_str().unwrap();
            assert_eq!(command[1], name);
            assert_eq!(command[2], "--exact");
            assert_eq!(command[3], "--nocapture");
            assert_eq!(test["result"], "pass");
            targets.insert(target);
            first_exact.get_or_insert_with(|| (target.to_owned(), name.to_owned()));
        }
        assert_eq!(targets.len(), 1);
        bind_engine_binary(run, targets.first().unwrap(), &mut binary_by_target);
        run.as_object_mut()
            .unwrap()
            .remove("binary_sha256")
            .unwrap();
    }
    assert_eq!(total_passed, 73);

    let (first_target, first_exact_name) = first_exact.unwrap();
    let mutant = &mut projected[normal_count];
    assert_eq!(mutant["label"], "same-count-substitution-mutant");
    assert_eq!(mutant["passed"].as_u64(), Some(0));
    assert_eq!(mutant["rejected"], true);
    assert_eq!(mutant["result"], "rejected");
    assert_eq!(mutant["substituted_for"], first_exact_name);
    assert_eq!(
        mutant["command"],
        serde_json::json!([
            first_target,
            format!("{first_exact_name}_same_count_substitution_mutant"),
            "--exact",
            "--nocapture",
        ])
    );
    bind_engine_binary(mutant, &first_target, &mut binary_by_target);
    mutant
        .as_object_mut()
        .unwrap()
        .remove("binary_sha256")
        .unwrap();
    Value::Array(projected)
}

fn valid_behavior_label(label: &str) -> bool {
    !label.is_empty()
        && label.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn bind_engine_binary(run: &Value, target: &str, binary_by_target: &mut BTreeMap<String, String>) {
    let binary_sha256 = run["binary_sha256"].as_str().unwrap();
    assert_eq!(binary_sha256.len(), 64);
    assert!(
        binary_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    if let Some(previous) = binary_by_target.insert(target.to_owned(), binary_sha256.to_owned()) {
        assert_eq!(previous, binary_sha256);
    }
}

fn canonical_json(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn assert_content_addressed_tree(root: &Path) {
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        for entry in fs::read_dir(root).unwrap() {
            assert_content_addressed_tree(&entry.unwrap().path());
        }
    }
}

fn tree_manifest(root: &Path) -> Value {
    let mut rows = Vec::new();
    collect_manifest_rows(root, root, &mut rows);
    rows.sort_by(|left, right| {
        left["path"]
            .as_str()
            .unwrap()
            .cmp(right["path"].as_str().unwrap())
    });
    let rows = Value::Array(rows);
    serde_json::json!({
        "identity": format!("sha256:{}", sha256(&canonical_json(&rows))),
        "rows": rows,
    })
}

fn collect_manifest_rows(root: &Path, directory: &Path, rows: &mut Vec<Value>) {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        let relative = path.strip_prefix(root).unwrap().to_string_lossy();
        if metadata.is_dir() {
            rows.push(serde_json::json!({"path": relative, "type": "directory"}));
            collect_manifest_rows(root, &path, rows);
        } else {
            let bytes = fs::read(&path).unwrap();
            rows.push(serde_json::json!({
                "byte_length": bytes.len(),
                "executable": metadata.permissions().mode() & 0o111 != 0,
                "path": relative,
                "sha256": format!("sha256:{}", sha256(&bytes)),
                "type": "file",
            }));
        }
    }
}
