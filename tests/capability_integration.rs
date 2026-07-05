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
fn missing_registry_reports_empty_clean_state() {
    let repo = init_repo("maestro-capability-missing");

    let output = maestro(&["capability", "--json"], repo.path());

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["schema"], "maestro.capability.v1");
    assert_eq!(json["registry"]["present"], false);
    assert!(json["capabilities"].as_array().unwrap().is_empty());
}

#[test]
fn capability_report_distinguishes_provider_states() {
    let repo = init_repo("maestro-capability-report");
    let maestro_dir = repo.path().join(".maestro");
    let present_tool = repo.path().join("tools/present-tool");
    fs::create_dir_all(present_tool.parent().unwrap())
        .expect("invariant: tool fixture dir should write");
    fs::write(&present_tool, "#!/bin/sh\nexit 0\n").expect("invariant: tool fixture should write");
    fs::create_dir_all(maestro_dir.join("receipts"))
        .expect("invariant: receipt fixture dir should write");
    fs::write(
        maestro_dir.join("receipts/github-write.yml"),
        "schema: maestro.capability-receipt.v1\nstatus: denied\ndetail: host policy denied write access\n",
    )
    .expect("invariant: denied receipt should write");
    fs::write(
        maestro_dir.join("receipts/docs-lookup.yml"),
        "schema: maestro.capability-receipt.v1\nstatus: unverified\ndetail: connector was not exercised in this session\n",
    )
    .expect("invariant: unverified receipt should write");
    fs::write(
        maestro_dir.join("capabilities.yml"),
        "\
schema: maestro.capabilities.v1
capabilities:
  - id: impact-analysis
    active: true
    providers:
      - name: present-tool
        kind: file
        path: tools/present-tool
      - name: missing-tool
        kind: cli
        command: definitely-not-a-real-maestro-test-command
      - name: partial-tool
        kind: cli
  - id: github-write
    active: true
    providers:
      - name: github
        kind: host_receipt
        receipt: receipts/github-write.yml
  - id: docs-lookup
    active: true
    providers:
      - name: browser
        kind: host_receipt
        receipt: receipts/docs-lookup.yml
  - id: deploy-verification
    active: false
    providers:
      - name: deploy-cli
        kind: cli
        command: missing-deploy-cli
",
    )
    .expect("invariant: capability manifest should write");

    let output = maestro(&["capability", "--json"], repo.path());

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["registry"]["present"], true);
    let capabilities = json["capabilities"].as_array().unwrap();
    assert_eq!(capabilities.len(), 4);

    let impact = capability(capabilities, "impact-analysis");
    assert_eq!(impact["status"], "present");
    assert_eq!(provider(impact, "present-tool")["status"], "present");
    assert_eq!(provider(impact, "missing-tool")["status"], "missing");
    assert_eq!(provider(impact, "partial-tool")["status"], "unverified");

    let github_write = capability(capabilities, "github-write");
    assert_eq!(github_write["status"], "denied");
    assert_eq!(provider(github_write, "github")["status"], "denied");

    let docs_lookup = capability(capabilities, "docs-lookup");
    assert_eq!(docs_lookup["status"], "unverified");
    assert_eq!(provider(docs_lookup, "browser")["status"], "unverified");

    let deploy = capability(capabilities, "deploy-verification");
    assert_eq!(deploy["status"], "inactive");
    assert_eq!(deploy["active"], false);
}

fn capability<'a>(capabilities: &'a [JsonValue], id: &str) -> &'a JsonValue {
    capabilities
        .iter()
        .find(|capability| capability["id"] == id)
        .expect("capability should be present")
}

fn provider<'a>(capability: &'a JsonValue, name: &str) -> &'a JsonValue {
    capability["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["name"] == name)
        .expect("provider should be present")
}
