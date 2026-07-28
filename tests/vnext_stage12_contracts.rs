use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new("python3")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run Stage 12 validator: {error}"))
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "Stage 12 validator emitted invalid JSON: {error}; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn canonical_promotion_checkpoint_is_deterministic_and_non_authoritative() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let first = run(
        repo,
        &["tools/vnext_contracts/stage12/census.py", "--summary"],
    );
    let second = run(
        repo,
        &["tools/vnext_contracts/stage12/census.py", "--summary"],
    );
    assert!(
        first.status.success(),
        "first census failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "second census failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let census = parse_stdout(&first);
    assert_eq!(census["closed_world"], false);
    assert_eq!(census["release_claim"], false);
    assert_eq!(census["row_count"], 384);
    assert_eq!(census["rule_counts"]["temporary_vnext_source_path"], 0);
    assert_eq!(
        census["rule_counts"]["temporary_domain_namespace_reference"],
        0
    );
    assert_eq!(census["rule_counts"]["temporary_domain_module_export"], 0);
    assert_eq!(census["rule_counts"]["legacy_skill_surface"], 274);
    assert_eq!(census["rule_counts"]["legacy_next_surface"], 108);
    assert_eq!(census["rule_counts"]["legacy_harness_resource"], 2);

    let validation = run(repo, &["tools/vnext_contracts/stage12/validate.py"]);
    assert!(
        validation.status.success(),
        "candidate validation failed: {}",
        String::from_utf8_lossy(&validation.stderr)
    );
    let receipt = parse_stdout(&validation);
    assert_eq!(receipt["status"], "pass");
    assert_eq!(receipt["authority_state"], "none");
    assert_eq!(receipt["candidate_ready_claim"], false);
    assert_eq!(
        receipt["candidate_state"],
        "canonical_namespace_promoted_legacy_pruning_blocked_unverified"
    );
    assert_eq!(receipt["namespace_promotion"]["entry_count"], 210);
    assert_eq!(receipt["namespace_promotion"]["collision_count"], 10);
    assert_eq!(
        receipt["namespace_promotion"]["temporary_namespace_count"],
        0
    );
    assert_eq!(receipt["legacy_pruning_authorized"], false);
    assert_eq!(receipt["compile_lane_needed"], true);
    assert_eq!(receipt["release_evaluated"], false);
    assert_eq!(receipt["negative_case_count"], 16);
}

#[test]
fn release_preflight_fails_closed_on_legacy_consumers_and_missing_seal_inputs() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = run(
        repo,
        &[
            "tools/vnext_contracts/stage12/validate.py",
            "--mode",
            "release-preflight",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let receipt = parse_stdout(&output);
    assert_eq!(receipt["status"], "blocked");
    assert_eq!(receipt["authority_state"], "none");
    assert_eq!(
        receipt["candidate_state"],
        "canonical_namespace_promoted_legacy_pruning_blocked_unverified"
    );
    assert_eq!(receipt["certification_claim"], false);
    assert_eq!(receipt["compile_lane_needed"], true);
    assert_eq!(receipt["release_evaluated"], false);
    let blockers = receipt["blockers"].as_array().unwrap();
    assert!(blockers.iter().any(|blocker| {
        blocker["id"] == "consumer_rows_nonzero"
            && blocker["rule_id"] == "legacy_skill_surface"
            && blocker["count"] == 274
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker["id"] == "consumer_rows_nonzero"
            && blocker["rule_id"] == "legacy_next_surface"
            && blocker["count"] == 108
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker["id"] == "consumer_rows_nonzero"
            && blocker["rule_id"] == "legacy_harness_resource"
            && blocker["count"] == 2
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker["id"] == "missing_external_input"
            && blocker["slot"] == "stage6_integrated_unsealed_checkpoint"
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker["id"] == "missing_external_input"
            && blocker["slot"] == "stage11_integrated_unsealed_checkpoint"
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker["id"] == "missing_external_input" && blocker["slot"] == "final_full_chain_seal"
    }));
}

#[test]
fn negative_compatibility_and_positive_claim_mutants_are_rejected() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = run(
        repo,
        &[
            "tools/vnext_contracts/stage12/validate.py",
            "--mutant-suite",
        ],
    );
    assert!(
        output.status.success(),
        "mutant suite failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = parse_stdout(&output);
    assert_eq!(receipt["status"], "pass");
    assert_eq!(receipt["accepted_mutants"], 0);
    assert_eq!(receipt["rejected_mutants"], 3);
    assert_eq!(receipt["authority_state"], "none");
}
