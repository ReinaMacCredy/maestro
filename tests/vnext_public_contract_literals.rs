use std::path::Path;
use std::process::{Command, Output};

fn run_python(repo: &Path, args: &[&str], label: &str) -> Output {
    let output = Command::new("python3")
        .args(args)
        .current_dir(repo)
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
fn public_candidate_literals_are_exact_and_inactive() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_output = run_python(
        repo,
        &[
            "tools/vnext_contracts/public/build_public_literals.py",
            "--check",
        ],
        "public literal deterministic build check",
    );
    let build_receipt: serde_json::Value =
        serde_json::from_slice(&build_output.stdout).expect("parse public literal build receipt");
    assert_eq!(build_receipt["status"], "pass");
    assert_eq!(build_receipt["mode"], "check");
    assert_eq!(build_receipt["mismatches"].as_array().unwrap().len(), 0);

    let output = run_python(
        repo,
        &[
            "tools/vnext_contracts/public/validate_public_contracts.py",
            "--repo",
            ".",
        ],
        "public candidate literal validation",
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse public validation receipt");
    assert_eq!(receipt["status"], "pass");
    assert_eq!(receipt["runtime_activated"], false);
    assert_eq!(receipt["inactive_source_roots"], 3);
    assert_eq!(receipt["recipes"], 10);
    assert_eq!(receipt["recipe_manifests"], 10);
    assert_eq!(receipt["bounded_continuation_profiles"], 2);
    assert_eq!(receipt["selection_application_vectors"], 30);
    assert_eq!(receipt["recipe_return_reasons"], 30);
    assert_eq!(receipt["recipe_return_vectors"], 196);
    assert_eq!(receipt["job_recipe_edges"], 22);
    assert_eq!(receipt["job_recipe_non_edges"], 48);
    assert_eq!(receipt["job_recipe_admitted"], 66);
    assert_eq!(receipt["job_recipe_refused"], 144);
    assert_eq!(receipt["setup_action_rows"], 145);
    assert_eq!(receipt["setup_ceremony_rows"], 11);
    assert_eq!(receipt["instruction_resources"], 31);
    assert_eq!(receipt["mcp_tools"], 2);
    assert_eq!(receipt["project_mcp_tools"], 0);
    assert_eq!(receipt["schema_descriptors"], 79);
    assert_eq!(receipt["context_budget_profiles"], 2);
    assert_eq!(receipt["context_budget_closures_per_profile"], 750);
    assert_eq!(
        receipt["skill_activation_catalog_counts"],
        serde_json::json!([43, 145, 23])
    );
    assert_eq!(receipt["setup_catalog_bound"], true);

    let mutant_output = run_python(
        repo,
        &[
            "tools/vnext_contracts/public/validate_public_contracts.py",
            "--repo",
            ".",
            "--mutant-suite",
        ],
        "public candidate mutation suite",
    );
    let mutant_receipt: serde_json::Value =
        serde_json::from_slice(&mutant_output.stdout).expect("parse public mutant receipt");
    assert_eq!(mutant_receipt["status"], "pass");
    assert_eq!(mutant_receipt["semantic_mutant_categories"], 12);
    assert_eq!(mutant_receipt["total_mutants"], 84);
    assert_eq!(mutant_receipt["rejected_mutants"], 84);
    assert_eq!(mutant_receipt["escaped"].as_array().unwrap().len(), 0);
}
