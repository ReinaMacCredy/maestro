use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use maestro::foundation::core::deterministic_cbor;

const SNAPSHOT_PATHS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src",
    "embedded",
    "tests",
    "tools/vnext_contracts",
    "contracts/vnext/catalogs",
    "contracts/vnext/stage0",
    "contracts/vnext/stage2",
    "contracts/vnext/stage3",
    "contracts/vnext/stage4/execution",
];
const STAGE4_SOURCE_ARCHIVE_LENGTH: usize = 16_486_231;
const STAGE4_SOURCE_ARCHIVE_SHA256: &str =
    "347eaf928f81d9ce6e07e3767f0cdaf2cde23cd98d13bad41b745d5fbc359910";
const STAGE4_SOURCE_COMMIT: &str = "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2";
const STAGE4_SOURCE_TREE: &str = "2f832a04c7109e17b4b298e40b4827c1ced2d527";

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn published_stage5_three_engine_receipts_bind_one_inactive_artifact() {
    let pointer_path = workspace().join("contracts/vnext/stage5/evidence-gates/current-proof.json");
    assert!(
        pointer_path.is_file(),
        "the published Stage 5 contract requires its exact proof pointer"
    );
    let pointer: Value = serde_json::from_slice(&fs::read(&pointer_path).unwrap()).unwrap();
    assert_eq!(
        pointer["schema_version"],
        "maestro.vnext.proof-publication-pointer.v1"
    );
    let release_identity = pointer["release_identity"].as_str().unwrap();
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
    assert_frozen_tree(&release_object);
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
    assert_eq!(
        artifact["behavior_manifest_identity"],
        "sha256:a45a1774976a2ad7d3e9cf9702ea78bb5bbae33a9deca7a06d5127c451477f12"
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
            "sha256:a45a1774976a2ad7d3e9cf9702ea78bb5bbae33a9deca7a06d5127c451477f12"
        );
        assert_eq!(receipt["artifact_id"], identity);
        assert_eq!(receipt["behavior_passed"], 55);
        assert_eq!(receipt["behavior_runs"].as_array().unwrap().len(), 8);
        assert_eq!(
            receipt["behavior_runs"][7]["label"],
            "same-count-substitution-mutant"
        );
        assert_eq!(receipt["behavior_runs"][7]["rejected"], true);
        assert_eq!(receipt["publication_state"], "inactive_candidate");
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
    let behavior_rows = builder["behavior_runs"]
        .as_array()
        .unwrap()
        .iter()
        .take(7)
        .flat_map(|run| run["tests"].as_array().unwrap())
        .map(|test| Value::Array(vec![test["command"][0].clone(), test["name"].clone()]))
        .collect::<Vec<_>>();
    assert_eq!(behavior_rows.len(), 55);
    assert_eq!(
        behavior_rows
            .iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        55
    );
    assert_eq!(
        format!(
            "sha256:{}",
            sha256(&canonical_json(&Value::Array(behavior_rows)))
        ),
        "sha256:a45a1774976a2ad7d3e9cf9702ea78bb5bbae33a9deca7a06d5127c451477f12"
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
    assert_eq!(consensus["behavior_passed"], 55);
    assert_eq!(
        consensus["behavior_manifest_identity"],
        "sha256:a45a1774976a2ad7d3e9cf9702ea78bb5bbae33a9deca7a06d5127c451477f12"
    );
    assert_eq!(consensus["proof_harness_passed"], 66);
    assert_eq!(harness["passed"], 66);
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
    let current_source_rows = live_snapshot_source_rows(workspace());
    assert_eq!(snapshot_manifest["source_rows"], current_source_rows);
    assert_eq!(
        snapshot_manifest["source_identity"],
        format!("sha256:{}", sha256(&canonical_json(&current_source_rows)))
    );
    assert_eq!(
        snapshot_manifest["schema_version"],
        "maestro.vnext.stage5.immutable-workspace-snapshot.v1"
    );
    let canonical = &release["canonical_value"];
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

fn read_json(path: PathBuf) -> Value {
    assert!(
        !fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn semantic_behavior_runs(runs: &Value) -> Value {
    let mut projected = runs.as_array().unwrap().clone();
    let mut binary_by_target = BTreeMap::new();
    for run in &mut projected {
        let binary_sha256 = run["binary_sha256"].as_str().unwrap();
        assert_eq!(binary_sha256.len(), 64);
        assert!(
            binary_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        let target = if let Some(tests) = run.get("tests").and_then(Value::as_array) {
            let targets = tests
                .iter()
                .map(|test| test["command"][0].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(targets.len(), 1);
            (*targets.first().unwrap()).to_owned()
        } else {
            run["command"][0].as_str().unwrap().to_owned()
        };
        if let Some(previous) = binary_by_target.insert(target, binary_sha256.to_owned()) {
            assert_eq!(previous, binary_sha256);
        }
        run.as_object_mut()
            .unwrap()
            .remove("binary_sha256")
            .unwrap();
    }
    Value::Array(projected)
}

fn canonical_json(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn live_snapshot_source_rows(root: &Path) -> Value {
    let mut rows = Vec::new();
    for relative in SNAPSHOT_PATHS {
        collect_source_rows(root, &root.join(relative), &mut rows);
    }
    rows.push(serde_json::json!([
        "predecessors/stage4-source.tar.gz",
        STAGE4_SOURCE_ARCHIVE_LENGTH,
        STAGE4_SOURCE_ARCHIVE_SHA256,
        false,
    ]));
    rows.sort_by(|left, right| left[0].as_str().unwrap().cmp(right[0].as_str().unwrap()));
    Value::Array(rows)
}

fn collect_source_rows(root: &Path, path: &Path, rows: &mut Vec<Value>) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        if path.file_name().is_some_and(|name| name == "__pycache__") {
            return;
        }
        let mut children = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            collect_source_rows(root, &child, rows);
        }
        return;
    }
    assert!(metadata.is_file());
    if path.extension().is_some_and(|extension| extension == "pyc") {
        return;
    }
    let bytes = fs::read(path).unwrap();
    rows.push(serde_json::json!([
        path.strip_prefix(root).unwrap().to_string_lossy(),
        bytes.len(),
        sha256(&bytes),
        metadata.permissions().mode() & 0o111 != 0,
    ]));
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

fn assert_frozen_tree(root: &Path) {
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.permissions().mode() & 0o222, 0);
    if metadata.is_dir() {
        for entry in fs::read_dir(root).unwrap() {
            assert_frozen_tree(&entry.unwrap().path());
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
