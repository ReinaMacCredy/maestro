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
        "schema: maestro.capability-receipt.v1\nissued_by: codex-host\nissued_at: 2026-07-08T00:00:00.000Z\nstatus: denied\ndetail: host policy denied write access\n",
    )
    .expect("invariant: denied receipt should write");
    fs::write(
        maestro_dir.join("receipts/docs-lookup.yml"),
        "schema: maestro.capability-receipt.v1\nissued_by: codex-host\nissued_at: 2026-07-08T00:00:00.000Z\nstatus: unverified\ndetail: connector was not exercised in this session\n",
    )
    .expect("invariant: unverified receipt should write");
    fs::write(
        maestro_dir.join("receipts/self-declared.yml"),
        "schema: maestro.capability-receipt.v1\nstatus: present\ndetail: no host issuer metadata\n",
    )
    .expect("invariant: self-declared receipt should write");
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
  - id: self-declared
    active: true
    providers:
      - name: local-note
        kind: host_receipt
        receipt: receipts/self-declared.yml
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
    assert_eq!(capabilities.len(), 5);

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

    let self_declared = capability(capabilities, "self-declared");
    assert_eq!(self_declared["status"], "unverified");
    assert_eq!(
        provider(self_declared, "local-note")["status"],
        "unverified"
    );
    assert_eq!(
        provider(self_declared, "local-note")["evidence"]["detail"],
        "receipt issuer metadata missing"
    );

    let deploy = capability(capabilities, "deploy-verification");
    assert_eq!(deploy["status"], "inactive");
    assert_eq!(deploy["active"], false);
}

#[test]
fn capability_report_preserves_permission_and_scope_boundaries() {
    let repo = init_repo("maestro-capability-boundaries");
    let outside = repo.path().join("../outside-capability-token.txt");
    fs::write(&outside, "secret provider fixture")
        .expect("invariant: outside fixture should write");
    let receipts = repo.path().join(".maestro/receipts");
    fs::create_dir_all(&receipts).expect("invariant: receipt dir should write");
    fs::write(
        receipts.join("host.yml"),
        "schema: maestro.capability-receipt.v1\nissued_by: codex-host\nissued_at: 2026-07-08T00:00:00.000Z\nstatus: present\ndetail: api_key=top-secret-token Authorization Bearer sk-testsecret123 ghp_testsecret123 host allowed local read\n",
    )
    .expect("invariant: receipt fixture should write");
    fs::write(
        repo.path().join(".maestro/capabilities.yml"),
        format!(
            "\
schema: maestro.capabilities.v1
capabilities:
  - id: out-of-scope-file
    providers:
      - name: outside
        kind: file
        path: {}
  - id: host-receipt
    providers:
      - name: host
        kind: host_receipt
        receipt: receipts/host.yml
",
            outside.display()
        ),
    )
    .expect("invariant: capability manifest should write");

    let output = maestro(&["capability", "--json"], repo.path());

    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("top-secret-token"), "{stdout}");
    assert!(!stdout.contains("sk-testsecret123"), "{stdout}");
    assert!(!stdout.contains("ghp_testsecret123"), "{stdout}");
    let json: JsonValue = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(json["schema"], "maestro.capability.v1");
    let capabilities = json["capabilities"].as_array().unwrap();

    let scoped = capability(capabilities, "out-of-scope-file");
    assert_eq!(scoped["grants_permission"], false);
    assert_eq!(scoped["status"], "denied");
    let scoped_provider = provider(scoped, "outside");
    assert_eq!(scoped_provider["status"], "denied");
    assert!(
        scoped_provider["evidence"]["detail"]
            .as_str()
            .unwrap()
            .contains("outside repository scope")
    );

    let receipt = capability(capabilities, "host-receipt");
    assert_eq!(receipt["grants_permission"], false);
    assert_eq!(receipt["status"], "present");
    assert!(
        provider(receipt, "host")["evidence"]["detail"]
            .as_str()
            .unwrap()
            .contains("[redacted]")
    );
}

#[test]
fn capability_manifest_from_nested_scope_resolves_relative_file_providers_there() {
    let repo = init_repo("maestro-capability-nested-scope");
    let nested = repo.path().join("packages/app/.maestro");
    fs::create_dir_all(nested.join("tools")).expect("invariant: nested tools dir should write");
    fs::write(nested.join("tools/local-tool"), "available")
        .expect("invariant: nested tool should write");
    fs::write(
        nested.join("capabilities.yml"),
        "\
schema: maestro.capabilities.v1
capabilities:
  - id: nested-tooling
    providers:
      - name: local-tool
        kind: file
        path: tools/local-tool
",
    )
    .expect("invariant: nested manifest should write");

    let output = maestro(
        &[
            "capability",
            "--from",
            nested
                .join("capabilities.yml")
                .to_str()
                .expect("fixture path should be UTF-8"),
            "--json",
        ],
        repo.path(),
    );

    assert_success(&output);
    let json = stdout_json(&output);
    let capabilities = json["capabilities"].as_array().unwrap();
    let nested_tooling = capability(capabilities, "nested-tooling");
    assert_eq!(nested_tooling["status"], "present");
    assert_eq!(provider(nested_tooling, "local-tool")["status"], "present");
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
