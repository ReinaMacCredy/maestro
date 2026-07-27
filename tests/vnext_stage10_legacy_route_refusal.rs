mod support;

use std::process::Command;

use support::TestTempDir;

const REFUSAL: &str = "unsupported_legacy_successor_surface";
const REPLACEMENT: &str = "maestro packet read";
const LEGACY_RECIPES: [&str; 15] = [
    "adversarial-review",
    "audit",
    "conflict-handoff",
    "design-relay",
    "design",
    "feature-fanout",
    "generate-filter",
    "intake-triage",
    "learning",
    "loop-until-done",
    "progress",
    "ship",
    "synthesize",
    "unattended",
    "work",
];

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

#[test]
fn every_legacy_recipe_route_refuses_before_repository_or_recipe_reads() {
    let outside_repository = TestTempDir::new("vnext-stage10-legacy-recipe-refusal");

    for recipe in LEGACY_RECIPES {
        let cases = [
            vec!["loop", "show", recipe],
            vec![
                "loop",
                "show",
                recipe,
                "--compact",
                "--phase",
                "act",
                "--json",
            ],
            vec!["loop", "validate", recipe],
            vec![
                "loop",
                "outcome",
                "--recipe",
                recipe,
                "--phase",
                "act",
                "--selected-unit",
                "task-1",
                "--json",
            ],
        ];
        for arguments in cases {
            let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
                .args(arguments)
                .current_dir(outside_repository.path())
                .env_remove("MAESTRO_ROOT")
                .output()
                .expect("run public legacy recipe route");

            assert!(
                !output.status.success(),
                "{recipe:?} unexpectedly succeeded"
            );
            assert!(
                output.stdout.is_empty(),
                "{recipe:?} emitted legacy recipe output: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(REFUSAL),
                "{recipe:?} did not emit the typed refusal: {stderr}"
            );
            assert!(
                stderr.contains(REPLACEMENT),
                "{recipe:?} did not name the canonical replacement: {stderr}"
            );
            assert!(
                !stderr.contains("repo root not found") && !stderr.contains("unknown loop recipe"),
                "{recipe:?} read legacy repository or recipe state before refusing: {stderr}"
            );
        }
    }
}

#[test]
fn exact_vnext_only_recipe_identifiers_continue_past_the_legacy_gate() {
    let outside_repository = TestTempDir::new("vnext-stage10-vnext-recipe-continuation");

    for recipe in ["bounded-continuation", "fanout", "setup", "wayfinding"] {
        for arguments in [
            vec!["loop", "show", recipe, "--compact", "--json"],
            vec!["loop", "validate", recipe],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
                .args(arguments)
                .current_dir(outside_repository.path())
                .env_remove("MAESTRO_ROOT")
                .output()
                .expect("run exact vNext recipe route");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.contains(REFUSAL),
                "{recipe:?} was mistaken for a legacy recipe: {stderr}"
            );
        }
    }

    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args([
            "loop",
            "outcome",
            "--recipe",
            "bounded-continuation",
            "--phase",
            "act",
            "--selected-unit",
            "task-1",
            "--json",
        ])
        .current_dir(outside_repository.path())
        .env_remove("MAESTRO_ROOT")
        .output()
        .expect("run exact vNext outcome route");
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains(REFUSAL),
        "vNext outcome recipe was mistaken for a legacy recipe"
    );
}
