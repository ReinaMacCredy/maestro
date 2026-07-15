use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryRoot(PathBuf);

impl TemporaryRoot {
    fn new(name: &str) -> Self {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "maestro-stage0-stage2-proof-{}-{sequence}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary proof root");
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
    fs::create_dir_all(destination).expect("create mutant root");
    for entry in fs::read_dir(source).expect("read Authority artifact root") {
        let entry = entry.expect("read Authority artifact entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy Authority artifact");
        }
    }
}

fn write_json(path: &Path, value: &Value) {
    let mut encoded = serde_json::to_vec_pretty(value).expect("encode mutant JSON");
    encoded.push(b'\n');
    fs::write(path, encoded).expect("write mutant JSON");
}

fn assert_rejected_by_both(repo: &Path, name: &str, mutate: impl FnOnce(&mut Value)) {
    let temporary = TemporaryRoot::new(name);
    let root = temporary.0.join("authority");
    copy_tree(&repo.join("contracts/vnext/stage2/authority"), &root);
    let manifest_path = root.join("stage2-authority-manifest.v1.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read Stage 2 manifest"))
            .expect("parse Stage 2 manifest");
    mutate(&mut manifest);
    write_json(&manifest_path, &manifest);

    let root_arg = root.to_str().expect("UTF-8 mutant root");
    let python = run(
        repo,
        "python3",
        &[
            "tools/vnext_contracts/stage2/authority/validate.py",
            "--root",
            root_arg,
        ],
    );
    assert!(
        !python.status.success(),
        "Python semantic validator accepted {name}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&python.stdout),
        String::from_utf8_lossy(&python.stderr),
    );

    let ruby = run(
        repo,
        "ruby",
        &[
            "tools/vnext_contracts/stage2/authority/verify.rb",
            "--root",
            root_arg,
        ],
    );
    assert!(
        !ruby.status.success(),
        "Ruby verifier accepted {name}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&ruby.stdout),
        String::from_utf8_lossy(&ruby.stderr),
    );
}

#[test]
fn stage2_semantic_consumer_delta_is_identity_bound_to_the_stage0_predecessor() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let delta_path =
        repo.join("contracts/vnext/stage0/effect-home/stage2-semantic-consumer-delta-v1.json");
    let delta: Value =
        serde_json::from_slice(&fs::read(&delta_path).expect("read Stage 2 consumer delta"))
            .expect("parse Stage 2 consumer delta");
    assert_eq!(
        delta["schema_version"],
        "maestro.vnext.stage2.semantic-consumer-delta.v1"
    );
    assert_eq!(
        delta["publication_state"],
        "candidate_only_runtime_inactive"
    );
    assert_eq!(
        delta["predecessor"]["consumer_census_id"]
            .as_str()
            .expect("predecessor census identity")
            .len(),
        71
    );
    assert_eq!(
        delta["predecessor"]["candidate_contract_root_id"]
            .as_str()
            .expect("predecessor root identity")
            .len(),
        71
    );

    let rows = delta["consumer_rows"]
        .as_array()
        .expect("Stage 2 semantic consumer rows");
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!(
            row.as_object()
                .expect("consumer row object")
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            [
                "consumer_disposition",
                "matched_literals",
                "owner",
                "path",
                "proof",
                "resource_identity",
                "worktree_sha256",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
        let path = row["path"].as_str().expect("consumer path");
        let bytes = fs::read(repo.join(path)).expect("read declared semantic consumer");
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(row["worktree_sha256"], digest);
        assert_eq!(row["resource_identity"], format!("sha256:{digest}"));
        assert!(
            !row["matched_literals"]
                .as_array()
                .expect("matched literals")
                .is_empty()
        );
    }

    let manifest: Value = serde_json::from_slice(
        &fs::read(repo.join("contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json"))
            .expect("read Stage 2 manifest"),
    )
    .expect("parse Stage 2 manifest");
    assert_eq!(
        manifest["component_ids"]["stage2_semantic_consumer_delta"],
        delta["identity"]
    );
}

#[test]
fn independent_stage2_proof_rejects_root_projection_mutants_and_field_drift() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_rejected_by_both(repo, "component_identity_substitution", |manifest| {
        manifest["component_ids"]["authority_literals"] = json!("0".repeat(64));
    });
    assert_rejected_by_both(repo, "root_canonical_projection_substitution", |manifest| {
        manifest["canonical_value"] = json!(["invented"]);
    });
    assert_rejected_by_both(repo, "artifact_omission", |manifest| {
        manifest["artifacts"]
            .as_array_mut()
            .expect("artifact rows")
            .pop();
    });
    assert_rejected_by_both(repo, "unknown_field", |manifest| {
        manifest["unknown_projection"] = json!(true);
    });
}
