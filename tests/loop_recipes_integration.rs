mod support;

use std::path::Path;
use std::process::{Command, Output};

use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn stdout(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

#[test]
fn loop_index_lists_structured_lifecycle_contracts_and_legacy_recipes() {
    let temp = TestTempDir::new("maestro-loop-index");
    let out = stdout(temp.path(), &["loop"]);

    assert!(out.contains("## Lifecycle Recipes"), "{out}");
    assert!(out.contains("design  --"), "{out}");
    assert!(out.contains("work  --"), "{out}");
    assert!(out.contains("unattended  --"), "{out}");
    assert!(out.contains("## Custom Recipe Policy"), "{out}");
    assert!(out.contains("## Orchestration Recipes"), "{out}");
    assert!(out.contains("conflict-handoff"), "{out}");
}

#[test]
fn loop_show_renders_structured_contracts_from_yaml() {
    let temp = TestTempDir::new("maestro-loop-show-contract");
    let out = stdout(temp.path(), &["loop", "show", "unattended"]);

    assert!(out.contains("# Unattended loop"), "{out}");
    assert!(
        out.contains("schema_version: maestro.loop_recipe.v1"),
        "{out}"
    );
    assert!(
        out.contains("perceive -> choose -> act -> observe -> learn -> continue"),
        "{out}"
    );
    assert!(out.contains("## Custom Recipe Policy"), "{out}");
    assert!(out.contains("Work Lease helper contract"), "{out}");
    assert!(out.contains("selected_unit"), "{out}");
    assert!(out.contains("maestro status --json"), "{out}");
    assert!(
        out.contains("returned inspect or status handle cannot be read"),
        "{out}"
    );
    assert!(out.contains("reconcile_handles"), "{out}");
    assert!(out.contains("run report command"), "{out}");
    assert!(out.contains("Forbidden verbs"), "{out}");
    assert!(out.contains("worker launcher"), "{out}");
}

#[test]
fn loop_show_keeps_legacy_orchestration_recipe_names_available() {
    let temp = TestTempDir::new("maestro-loop-show-legacy");
    let out = stdout(temp.path(), &["loop", "show", "conflict-handoff"]);

    assert!(out.contains("# Conflict handoff"), "{out}");
    assert!(out.contains("git worktree add"), "{out}");
}
