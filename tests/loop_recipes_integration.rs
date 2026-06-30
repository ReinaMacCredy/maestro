mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
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

fn stderr(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        !output.status.success(),
        "maestro {args:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn write_custom_recipe(repo: &Path, name: &str, body: &str) {
    let dir = repo.join(".maestro/loop-recipes");
    fs::create_dir_all(&dir).expect("custom recipe dir should be creatable");
    fs::write(dir.join(format!("{name}.yml")), body).expect("custom recipe should be writable");
}

#[test]
fn loop_index_lists_unified_structured_recipe_catalog() {
    let temp = TestTempDir::new("maestro-loop-index");
    let out = stdout(temp.path(), &["loop"]);

    assert!(out.contains("## Shipped Recipe Catalog"), "{out}");
    assert!(out.contains("design  [lifecycle]"), "{out}");
    assert!(out.contains("work  [lifecycle]"), "{out}");
    assert!(out.contains("unattended  [lifecycle]"), "{out}");
    assert!(out.contains("conflict-handoff  [orchestration]"), "{out}");
    assert!(out.contains("feature-fanout"), "{out}");
    assert!(out.contains("adversarial-review"), "{out}");
    assert!(out.contains("generate-filter"), "{out}");
    assert!(out.contains("## Custom Recipe Policy"), "{out}");
    assert!(out.contains("conflict-handoff"), "{out}");
    assert!(!out.contains("feature-fan-out"), "{out}");
    assert!(!out.contains("adversarial-fan-out"), "{out}");
    assert!(!out.contains("generate-and-filter"), "{out}");
}

#[test]
fn loop_show_renders_structured_contracts_from_yaml() {
    let temp = TestTempDir::new("maestro-loop-show-contract");
    let out = stdout(temp.path(), &["loop", "show", "unattended"]);

    assert!(out.contains("# Unattended loop"), "{out}");
    assert!(out.contains("schema_version: maestro.recipe.v2"), "{out}");
    assert!(out.contains("## Router Metadata"), "{out}");
    assert!(out.contains("## Authority Scope"), "{out}");
    assert!(out.contains("## Autonomy"), "{out}");
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
fn loop_show_renders_migrated_orchestration_recipe_from_yaml() {
    let temp = TestTempDir::new("maestro-loop-show-migrated");
    let out = stdout(temp.path(), &["loop", "show", "conflict-handoff"]);

    assert!(out.contains("# Conflict handoff"), "{out}");
    assert!(out.contains("git worktree add"), "{out}");
    assert!(out.contains("schema_version: maestro.recipe.v2"), "{out}");
}

#[test]
fn loop_rejects_old_renamed_recipe_ids() {
    let temp = TestTempDir::new("maestro-loop-old-aliases");
    for legacy in [
        "feature-fan-out",
        "adversarial-fan-out",
        "generate-and-filter",
        "unattended-loop",
    ] {
        let error = stderr(temp.path(), &["loop", "show", legacy]);
        assert!(error.contains("unknown loop recipe"), "{legacy}: {error}");
        assert!(error.contains("feature-fanout"), "{legacy}: {error}");
        assert!(!error.contains("feature-fan-out,"), "{legacy}: {error}");
    }
}

#[test]
fn loop_lists_shows_and_validates_project_custom_recipes() {
    let temp = TestTempDir::new("maestro-loop-custom");
    write_custom_recipe(temp.path(), "brief", CUSTOM_RECIPE);

    let index = stdout(temp.path(), &["loop"]);
    assert!(index.contains("## Project Custom Recipes"), "{index}");
    assert!(
        index
            .contains("brief  --  Handle one bounded support brief through current Maestro cards."),
        "{index}"
    );

    let shown = stdout(temp.path(), &["loop", "show", "brief"]);
    assert!(shown.contains("# Support brief loop"), "{shown}");
    assert!(
        shown.contains("schema_version: maestro.recipe.v2"),
        "{shown}"
    );
    assert!(
        shown.contains("perceive -> choose -> act -> observe -> learn -> continue"),
        "{shown}"
    );
    assert!(shown.contains("## Custom Recipe Policy"), "{shown}");

    let validated = stdout(temp.path(), &["loop", "validate", "brief"]);
    assert!(
        validated.contains("valid project custom loop recipe: brief"),
        "{validated}"
    );
}

#[test]
fn loop_rejects_invalid_project_custom_recipes() {
    let temp = TestTempDir::new("maestro-loop-custom-invalid");
    write_custom_recipe(
        temp.path(),
        "brief",
        "schema_version: maestro.recipe.v2\nid: brief\n",
    );

    let error = stderr(temp.path(), &["loop", "show", "brief"]);
    assert!(
        error.contains("invalid custom loop recipe brief.yml"),
        "{error}"
    );
}

#[test]
fn loop_rejects_symlinked_project_custom_recipe_file() {
    let temp = TestTempDir::new("maestro-loop-custom-file-symlink");
    let external = temp.path().join("external-brief.yml");
    fs::write(&external, CUSTOM_RECIPE).expect("external recipe should be writable");
    let dir = temp.path().join(".maestro/loop-recipes");
    fs::create_dir_all(&dir).expect("custom recipe dir should be creatable");
    unix_fs::symlink(&external, dir.join("brief.yml")).expect("recipe symlink should be creatable");

    let error = stderr(temp.path(), &["loop", "show", "brief"]);
    assert!(error.contains("symlink"), "{error}");
}

#[test]
fn loop_rejects_symlinked_project_custom_recipe_dir() {
    let temp = TestTempDir::new("maestro-loop-custom-dir-symlink");
    let external = temp.path().join("external-loop-recipes");
    fs::create_dir_all(&external).expect("external recipe dir should be creatable");
    fs::write(external.join("brief.yml"), CUSTOM_RECIPE)
        .expect("external recipe should be writable");
    fs::create_dir_all(temp.path().join(".maestro")).expect("maestro dir should be creatable");
    unix_fs::symlink(&external, temp.path().join(".maestro/loop-recipes"))
        .expect("recipe dir symlink should be creatable");

    let error = stderr(temp.path(), &["loop"]);
    assert!(error.contains("symlink"), "{error}");
}

const CUSTOM_RECIPE: &str = r#"schema_version: maestro.recipe.v2
id: brief
kind:
  category: custom
  tags: ["support", "brief"]
title: Support brief loop
summary: Handle one bounded support brief through current Maestro cards.
authority_scope:
  - current support brief and selected Maestro card
autonomy:
  - local autonomous work only inside the selected brief
router:
  status: custom_brief
  priority: 3
  confidence: medium
transitions:
  - trigger: brief needs ordinary implementation
    to: work
    authority_scope:
      - selected card
    allowed_verbs:
      - maestro card show <id>
      - maestro task complete <id>
    forbidden_verbs:
      - external ship action
    hard_stops:
      - brief requires external approval
    return_condition: selected card is verified or blocked
invocations: []
outputs:
  - selected card
  - verified card
  - hard stop
applies_when:
  - a user request is already scoped to one support brief
hard_stops:
  - the brief requires external ship authority
phases:
  perceive:
    goal: Read the current support brief and current Maestro state.
    bricks: ["status", "card show"]
    reads: ["maestro status", "maestro card show <id>"]
    allowed_verbs: ["maestro status", "maestro card show <id>"]
    forbidden_verbs: ["external ship action"]
    checks: ["brief and current card are visible"]
    durable_learning: []
    outputs: ["brief context"]
  choose:
    goal: Choose one existing card or create one scoped card for the brief.
    bricks: ["card ready", "task create"]
    reads: ["maestro card ready"]
    allowed_verbs: ["maestro card ready", "maestro task create"]
    forbidden_verbs: ["worker launcher"]
    checks: ["one card is selected"]
    durable_learning: []
    outputs: ["selected card"]
  act:
    goal: Work the selected card through current Maestro verbs.
    bricks: ["task", "proof"]
    reads: ["maestro task show <id>"]
    allowed_verbs: ["maestro task complete <id>", "maestro task verify <id>"]
    forbidden_verbs: ["hidden store"]
    checks: ["proof backs the brief result"]
    durable_learning: []
    outputs: ["verified card"]
  observe:
    goal: Confirm the result is inspectable.
    bricks: ["proof", "query"]
    reads: ["maestro query run --json"]
    allowed_verbs: ["maestro query run --json"]
    forbidden_verbs: ["claim success without proof"]
    checks: ["result appears in proof or run events"]
    durable_learning: []
    outputs: ["observed result"]
  learn:
    goal: Record only reusable corrections.
    bricks: ["memory", "decision"]
    reads: ["maestro memory list"]
    allowed_verbs: ["maestro memory create", "maestro decision new"]
    forbidden_verbs: ["chat-only learning"]
    checks: ["learning is durable when needed"]
    durable_learning: ["approved memory", "locked decision"]
    outputs: ["optional durable learning"]
  continue:
    goal: Return the next local Maestro action or a hard stop.
    bricks: ["status", "task next"]
    reads: ["maestro status", "maestro task next"]
    allowed_verbs: ["maestro status", "maestro task next"]
    forbidden_verbs: ["scheduler"]
    checks: ["next step is explicit"]
    durable_learning: []
    outputs: ["next step", "hard stop"]
"#;
