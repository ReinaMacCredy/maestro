mod common;
mod support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
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

fn assert_failure(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout_json(output: &std::process::Output) -> JsonValue {
    serde_json::from_slice(&output.stdout).expect("invariant: stdout should be JSON")
}

#[test]
fn freeform_prompt_routes_to_design_required_with_provenance() {
    let repo = init_repo("maestro-intake-freeform");
    let intake = repo.path().join("external-plan.md");
    fs::write(&intake, "# Imported plan\n\nBuild the thing.\n")
        .expect("invariant: intake fixture should write");

    let output = maestro(
        &[
            "intake",
            "--from",
            intake.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["schema"], "maestro.intake.v1");
    assert_eq!(json["route"], "design_required");
    assert_eq!(json["source_provenance"]["kind"], "file");
    assert_eq!(
        json["source_provenance"]["path"],
        intake.display().to_string()
    );
    assert_eq!(json["source_provenance"]["bytes"], 34);
    assert!(
        json["missing"]
            .as_array()
            .unwrap()
            .contains(&JsonValue::String(
                "structured route_hint frontmatter".to_string()
            ))
    );
}

#[test]
fn structured_card_ready_intake_routes_to_card_ready() {
    let repo = init_repo("maestro-intake-card-ready");
    let intake = repo.path().join("external-spec.md");
    fs::write(
        &intake,
        "\
---
route_hint: card_ready
owner: import-api
evidence:
  acceptance: true
  affected_areas: true
---
# Imported spec
",
    )
    .expect("invariant: intake fixture should write");

    let output = maestro(
        &[
            "intake",
            "--from",
            intake.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["route"], "card_ready");
    assert_eq!(json["route_hint"], "card_ready");
    assert_eq!(json["owner"], "import-api");
    assert!(json["missing"].as_array().unwrap().is_empty());
    assert_eq!(json["writes_allowed"], false);
}

#[test]
fn work_ready_hint_downgrades_when_owner_is_not_ready() {
    let repo = init_repo("maestro-intake-work-downgrade");
    let feature = maestro(
        &[
            "feature",
            "new",
            "Import API",
            "--description",
            "Existing proposed owner",
            "--id-only",
        ],
        repo.path(),
    );
    assert_success(&feature);
    let owner = String::from_utf8(feature.stdout)
        .expect("feature id should be UTF-8")
        .trim()
        .to_string();
    let intake = repo.path().join("external-work.md");
    fs::write(
        &intake,
        format!(
            "\
---
route_hint: work_ready
owner: {owner}
evidence:
  acceptance: true
  affected_areas: true
  proof_path: true
  handoff_fresh: true
  blockers_clear: true
---
# Imported implementation plan
"
        ),
    )
    .expect("invariant: intake fixture should write");

    let output = maestro(
        &[
            "intake",
            "--from",
            intake.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["route"], "card_ready");
    assert_eq!(json["route_hint"], "work_ready");
    assert!(
        json["blocked_by"]
            .as_array()
            .unwrap()
            .contains(&JsonValue::String(format!(
                "owner feature {owner} status is proposed"
            )))
    );
}

#[test]
fn work_ready_hint_routes_to_work_ready_for_ready_owner() {
    let repo = init_repo("maestro-intake-work-ready");
    let feature = maestro(
        &[
            "feature",
            "new",
            "Import API",
            "--description",
            "Existing ready owner",
            "--id-only",
        ],
        repo.path(),
    );
    assert_success(&feature);
    let owner = String::from_utf8(feature.stdout)
        .expect("feature id should be UTF-8")
        .trim()
        .to_string();
    assert_success(&maestro(
        &[
            "feature",
            "set",
            &owner,
            "--acceptance",
            "Imported work has a proof path",
            "--area",
            "src/domain/intake.rs",
        ],
        repo.path(),
    ));
    assert_success(&maestro(&["feature", "reconcile", &owner], repo.path()));
    assert_success(&maestro(&["feature", "finalize", &owner], repo.path()));
    assert_success(&maestro(
        &[
            "feature",
            "accept",
            &owner,
            "--qa",
            "none",
            "--reason",
            "intake integration test",
        ],
        repo.path(),
    ));

    let intake = repo.path().join("external-work.md");
    fs::write(
        &intake,
        format!(
            "\
---
route_hint: work_ready
owner: {owner}
evidence:
  acceptance: true
  affected_areas: true
  proof_path: true
  handoff_fresh: true
  blockers_clear: true
---
# Imported implementation plan
"
        ),
    )
    .expect("invariant: intake fixture should write");

    let output = maestro(
        &[
            "intake",
            "--from",
            intake.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["route"], "work_ready");
    assert_eq!(json["owner"], owner);
    assert!(json["blocked_by"].as_array().unwrap().is_empty());
    assert!(json["next"].as_str().unwrap().contains("maestro ready"));
}

#[cfg(unix)]
#[test]
fn intake_refuses_symlinked_sources_without_leaking_source_contents() {
    let repo = init_repo("maestro-intake-symlink");
    let outside = repo.path().join("outside-secret-plan.md");
    fs::write(
        &outside,
        "---\nroute_hint: work_ready\n---\napi_key=top-secret-token\nMIT License\n",
    )
    .expect("invariant: outside fixture should write");
    let intake = repo.path().join("linked-plan.md");
    unix_fs::symlink(&outside, &intake).expect("invariant: symlink fixture should write");

    let output = maestro(
        &[
            "intake",
            "--from",
            intake.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );

    assert_failure(&output);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("symlink"), "{combined}");
    assert!(!combined.contains("top-secret-token"), "{combined}");
    assert!(!combined.contains("MIT License"), "{combined}");
}

#[test]
fn intake_refuses_binary_and_oversized_sources() {
    let repo = init_repo("maestro-intake-unsafe-bytes");
    let binary = repo.path().join("binary-plan.md");
    fs::write(&binary, b"---\nroute_hint: card_ready\n---\n\0secret")
        .expect("invariant: binary fixture should write");
    let output = maestro(
        &[
            "intake",
            "--from",
            binary.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("binary"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let huge = repo.path().join("huge-plan.md");
    fs::write(&huge, "x".repeat(1_048_577)).expect("invariant: huge fixture should write");
    let output = maestro(
        &[
            "intake",
            "--from",
            huge.to_str().expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("too large"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
