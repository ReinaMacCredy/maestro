mod common;
mod support;

use std::fs;
use std::path::Path;

use common::cli_harness::maestro as cli_maestro;
use serde_json::Value as JsonValue;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    cli_maestro(cwd)
        .args(args)
        .env("HOME", cwd.join("home").as_os_str())
        .output()
        .into_raw()
}

fn init_repo(prefix: &str) -> TestTempDir {
    let temp = TestTempDir::new(prefix);
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");
    let output = maestro(&["init", "--yes"], temp.path());
    assert!(
        output.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    temp
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout_json(output: &std::process::Output) -> JsonValue {
    serde_json::from_slice(&output.stdout).expect("invariant: stdout should be JSON")
}

#[test]
fn maturity_json_exposes_context_proof_friction_level_and_owner() {
    let repo = init_repo("maestro-maturity-json");
    fs::write(
        repo.path().join("UX_GAPS.md"),
        "# UX gaps\n\n- Surface: feature set\n  Observed friction: fixture\n",
    )
    .expect("invariant: UX gap fixture should write");
    let feature = maestro(
        &[
            "feature",
            "new",
            "Maturity Demo",
            "--description",
            "Fixture feature",
            "--id-only",
        ],
        repo.path(),
    );
    assert_success(&feature);
    let feature_id = String::from_utf8(feature.stdout)
        .expect("feature id should be UTF-8")
        .trim()
        .to_string();
    assert_success(&maestro(
        &[
            "feature",
            "set",
            &feature_id,
            "--acceptance",
            "First proof gap",
            "--acceptance",
            "Second proof gap",
            "--area",
            "src/domain/maturity.rs",
        ],
        repo.path(),
    ));

    let output = maestro(&["maturity", &feature_id, "--json"], repo.path());

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["schema"], "maestro.maturity.v1");
    assert_eq!(json["target"], feature_id);
    assert_eq!(json["proof"]["total"], 2);
    assert_eq!(json["proof"]["incomplete"], 2);
    assert_eq!(json["proof"]["gaps"].as_array().unwrap().len(), 2);
    assert_eq!(json["friction"]["ux_gap_entries"], 1);
    assert_eq!(json["maturity"]["level"], "L1 report");
    assert_eq!(json["next_owner"]["surface"], "feature_proof");
    assert_eq!(
        json["next_owner"]["command"],
        format!("maestro feature prepare {feature_id} --draft")
    );
    let context = json["context"].as_array().unwrap();
    assert!(context_item(context, "harness")["status"] == "present");
    assert!(context_item(context, "feature")["status"] == "present");
    assert!(context_item(context, "acceptance")["status"] == "present");
    assert!(context_item(context, "proof")["status"] == "missing");
}

#[test]
fn maturity_human_output_names_the_routing_sections() {
    let repo = init_repo("maestro-maturity-human");

    let output = maestro(&["maturity"], repo.path());

    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("maturity:"), "{stdout}");
    assert!(stdout.contains("context:"), "{stdout}");
    assert!(stdout.contains("proof:"), "{stdout}");
    assert!(stdout.contains("friction:"), "{stdout}");
    assert!(stdout.contains("next_owner:"), "{stdout}");
}

#[test]
fn maturity_does_not_count_unverified_task_covers_as_feature_proof() {
    let repo = init_repo("maestro-maturity-unverified-covers");
    let feature = maestro(
        &[
            "feature",
            "new",
            "Covered Feature",
            "--description",
            "Fixture feature",
            "--id-only",
        ],
        repo.path(),
    );
    assert_success(&feature);
    let feature_id = String::from_utf8(feature.stdout)
        .expect("feature id should be UTF-8")
        .trim()
        .to_string();
    assert_success(&maestro(
        &[
            "feature",
            "set",
            &feature_id,
            "--acceptance",
            "Covered acceptance",
        ],
        repo.path(),
    ));
    assert_success(&maestro(
        &[
            "task",
            "create",
            "Unverified cover task",
            "--feature",
            &feature_id,
            "--covers",
            "ac-1",
            "--check",
            "Covered acceptance",
        ],
        repo.path(),
    ));

    let output = maestro(&["maturity", &feature_id, "--json"], repo.path());

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["proof"]["total"], 1);
    assert_eq!(json["proof"]["incomplete"], 1);
    assert_eq!(json["maturity"]["level"], "L1 report");
    assert_eq!(json["next_owner"]["surface"], "feature_proof");
}

fn context_item<'a>(context: &'a [JsonValue], name: &str) -> &'a JsonValue {
    context
        .iter()
        .find(|item| item["name"] == name)
        .expect("context item should be present")
}
