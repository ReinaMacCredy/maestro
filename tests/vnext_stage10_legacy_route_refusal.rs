mod support;

use std::process::Command;

use support::TestTempDir;

const REFUSAL: &str = "unsupported_legacy_successor_surface";
const REPLACEMENT: &str = "maestro packet read";

#[test]
fn public_legacy_successor_routes_refuse_before_repository_state_reads() {
    let outside_repository = TestTempDir::new("vnext-stage10-legacy-refusal");

    let cases = [
        vec!["next"],
        vec!["next", "--json"],
        vec!["next", "--brief", "--json"],
        vec!["task", "next"],
        vec!["task", "next", "--json"],
        vec!["loop", "next"],
        vec!["loop", "next", "--json", "--compact", "--phase", "execute"],
        vec!["card", "ready"],
        vec!["card", "ready", "--json", "--project", "example"],
        vec!["card", "ready", "--json", "feature-001"],
    ];

    for arguments in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
            .args(&arguments)
            .current_dir(outside_repository.path())
            .env_remove("MAESTRO_ROOT")
            .output()
            .expect("run public legacy successor route");

        assert!(
            !output.status.success(),
            "{arguments:?} unexpectedly succeeded"
        );
        assert!(
            output.stdout.is_empty(),
            "{arguments:?} emitted legacy output: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(REFUSAL),
            "{arguments:?} did not emit the typed refusal: {stderr}"
        );
        assert!(
            stderr.contains(REPLACEMENT),
            "{arguments:?} did not name the canonical replacement: {stderr}"
        );
        assert!(
            !stderr.contains("repo root not found"),
            "{arguments:?} consulted repository state before refusing: {stderr}"
        );
    }
}
