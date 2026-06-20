mod support;

use std::process::Command;

use support::TestTempDir;

fn maestro(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in CLI tests");

    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invariant: clap help should be valid UTF-8")
}

fn assert_contains_all(output: &str, expected: &[&str]) {
    for item in expected {
        assert!(
            output.contains(item),
            "expected help output to contain {item:?}\n{output}"
        );
    }
}

#[test]
fn root_help_lists_top_level_commands() {
    let output = maestro(&["--help"]);

    assert_contains_all(
        &output,
        &[
            "Usage: maestro",
            "init",
            "install",
            "upgrade",
            "sync",
            "uninstall",
            "doctor",
            "shell-init",
            "next",
            "resume",
            "task",
            "event",
            "feature",
            "qa",
            "decision",
            "card",
            "active",
            "link",
            "harness",
            "query",
            "mcp",
            "hook",
            "watch",
            "version",
        ],
    );

    // The 11 flat card verbs, top-level verify, and both migrations are hidden:
    // the card namespace is canonical, but each flat spelling still dispatches.
    let command_section = output
        .split("Options:")
        .next()
        .expect("root --help always has a Commands section before Options");
    for hidden in [
        "  ready ",
        "  list ",
        "  dep ",
        "  archive ",
        "  claim ",
        "  assign ",
        "  note ",
        "  create ",
        "  show ",
        "  update ",
        "  close ",
        "  verify ",
        "  migrate ",
        "  migrate-v2 ",
    ] {
        assert!(
            !command_section.contains(hidden),
            "hidden verb `{}` must not appear in root --help:\n{command_section}",
            hidden.trim()
        );
    }
}

#[test]
fn task_help_hides_retired_archive_verbs() {
    let output = maestro(&["task", "--help"]);
    let command_section = output
        .split("Options:")
        .next()
        .expect("task --help always has a Commands section before Options");

    assert!(
        !command_section.contains("  archive "),
        "retired task archive must not appear in task --help:\n{command_section}"
    );
    assert!(
        !command_section.contains("  unarchive "),
        "retired task unarchive must not appear in task --help:\n{command_section}"
    );
}

#[test]
fn top_level_help_fills_descriptions_and_examples() {
    // Every top-level command carries a non-blank `about`; spot-check ones that
    // were previously blank (init-ux D5), including the new `sync` verb.
    assert_contains_all(
        &maestro(&["--help"]),
        &[
            "Resync bundled resources to this binary's versions (offline)",
            "Scaffold .maestro/ and extract bundled resources into this repo",
            "Diagnose the maestro installation and report problems",
            "Print a clean-session resume packet from current repo artifacts",
        ],
    );

    // The refresh trio documents real invocations under an Examples block.
    for command in ["init", "sync", "upgrade"] {
        let help = maestro(&[command, "--help"]);
        assert!(
            help.contains("Examples:") && help.contains(&format!("maestro {command}")),
            "expected `{command} --help` to carry an Examples block with real invocations\n{help}"
        );
    }
}

#[test]
fn root_about_strings_name_every_subcommand() {
    // T7 regression: the group `about` lines must not omit subcommands.
    assert_contains_all(
        &maestro(&["--help"]),
        &[
            "Create, show, and list decision cards in the card store",
            "Author non-blocking related links between cards",
            "List, show, apply, unapply, dismiss, and measure harness improvement suggestions",
            "Query computed read models (matrix, friction, backlog)",
            "Run or inspect the MCP server (serve, tools)",
        ],
    );
}

#[test]
fn version_flag_matches_the_version_subcommand() {
    // S7: `--version`/`-V` now exists and prints the same version string as the
    // `version` subcommand (which adds the release date and binary path).
    let flag = maestro(&["--version"]);
    assert!(
        flag.starts_with("maestro "),
        "unexpected --version output: {flag}"
    );
    let subcommand = maestro(&["version"]);
    let flag_ver = flag.split_whitespace().nth(1).unwrap_or_default();
    let sub_ver = subcommand.split_whitespace().nth(1).unwrap_or_default();
    assert_eq!(
        flag_ver, sub_ver,
        "--version and the `version` subcommand disagree on the version string"
    );
    assert_eq!(maestro(&["-V"]), flag, "`-V` should match `--version`");
}

#[test]
fn version_subcommand_runs_without_repo_root() {
    let temp_dir = TestTempDir::new("maestro-version-rootless-test");
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("version")
        .current_dir(temp_dir.path())
        .output()
        .expect("invariant: compiled maestro binary should be runnable in CLI tests");

    assert!(
        output.status.success(),
        "maestro version failed outside a repo\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.starts_with("maestro "),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stderr.contains("failed to discover repository root"),
        "version should not discover repo roots:\n{stderr}"
    );
    assert!(
        !temp_dir.path().join(".maestro").exists(),
        "version must not scaffold .maestro"
    );
}

#[test]
fn nested_help_lists_section_38_command_tree() {
    assert_contains_all(
        &maestro(&["init", "--help"]),
        &["--dry-run", "--merge", "--force", "--yes"],
    );
    assert_contains_all(
        &maestro(&["install", "--help"]),
        &["--agent", "claude", "codex"],
    );
    assert_contains_all(
        &maestro(&["uninstall", "--help"]),
        &["--agent", "claude", "codex"],
    );
    assert_contains_all(
        &maestro(&["resume", "--help"]),
        &[
            "--task",
            "--feature",
            "--full",
            "--handoff",
            "--write",
            "--json",
        ],
    );
    assert_contains_all(
        &maestro(&["task", "--help"]),
        &[
            "create",
            "explore",
            "accept",
            "claim",
            "complete",
            "verify",
            "update",
            "block",
            "unblock",
            "reject",
            "abandon",
            "supersede",
            "show",
            "list",
            "watch",
            "doctor",
        ],
    );
    assert_contains_all(&maestro(&["event", "--help"]), &["create"]);
    assert_contains_all(
        &maestro(&["event", "create", "--help"]),
        &["--task-id", "--message", "--payload", "--claim", "--run"],
    );
    assert_contains_all(
        &maestro(&["feature", "--help"]),
        &[
            "new", "set", "accept", "amend", "start", "ship", "cancel", "show", "list",
        ],
    );
    assert_contains_all(&maestro(&["decision", "--help"]), &["new", "show", "list"]);
    assert_contains_all(
        &maestro(&["ready", "--help"]),
        &["Examples:", "maestro ready"],
    );
    assert_contains_all(
        &maestro(&["list", "--help"]),
        &[
            "--parent",
            "--type",
            "--assignee",
            "--status",
            "--grep",
            "--archived",
        ],
    );
    // `create` takes a variadic title (batch-mint) and the display-only
    // `--active-form` label; the per-card text fields are documented as
    // batch-refused.
    assert_contains_all(
        &maestro(&["create", "--help"]),
        &["<TITLE>...", "--active-form", "--description", "--id-only"],
    );
    assert_contains_all(&maestro(&["dep", "--help"]), &["add", "remove"]);
    assert_contains_all(
        &maestro(&["active", "--help"]),
        &["--all", "Examples:", "maestro active"],
    );
    let link_help = maestro(&["link", "--help"]);
    assert_contains_all(&link_help, &["add", "remove"]);
    assert!(
        !link_help.contains("list"),
        "link v1 exposes add/remove only, not list:\n{link_help}"
    );
    assert_contains_all(
        &maestro(&["link", "add", "--help"]),
        &["CARD-A", "CARD-B", "Examples:", "maestro link add"],
    );
    assert_contains_all(
        &maestro(&["link", "remove", "--help"]),
        &["FROM", "TO", "Examples:", "maestro link remove"],
    );
    assert_contains_all(
        &maestro(&["archive", "--help"]),
        &["FEATURE", "Examples:", "maestro archive"],
    );
    assert_contains_all(
        &maestro(&["claim", "--help"]),
        &["ID", "Examples:", "maestro claim"],
    );
    assert_contains_all(
        &maestro(&["note", "--help"]),
        &["ID", "TEXT", "Examples:", "maestro note"],
    );
    assert_contains_all(
        &maestro(&["dep", "add", "--help"]),
        &["CHILD", "PARENT", "Examples:", "maestro dep add"],
    );
    assert_contains_all(
        &maestro(&["dep", "remove", "--help"]),
        &["CHILD", "PARENT", "Examples:", "maestro dep remove"],
    );
    assert_contains_all(
        &maestro(&["harness", "--help"]),
        &["list", "show", "apply", "unapply", "measure"],
    );
    assert_contains_all(
        &maestro(&["query", "--help"]),
        &["matrix", "friction", "backlog"],
    );
    // query proof/graph/decisions are hidden but still dispatch (back-compat);
    // task proof and card graph are the canonical homes.
    assert_contains_all(&maestro(&["query", "proof", "--help"]), &["--task-id"]);
    assert_contains_all(&maestro(&["task", "proof", "--help"]), &["--task-id"]);
    assert_contains_all(&maestro(&["card", "graph", "--help"]), &["--dot"]);
    assert_contains_all(&maestro(&["mcp", "--help"]), &["serve", "tools"]);
    assert_contains_all(&maestro(&["hook", "--help"]), &["record"]);
    assert_contains_all(&maestro(&["watch", "--help"]), &["snapshot"]);
}

#[test]
fn nested_help_hides_internal_names_and_sibling_examples() {
    // R27: the `--task-id` flag must show a clean value placeholder, not the
    // raw field identifier (`TASK_ID_FLAG`) clap derives by default.
    let proof = maestro(&["query", "proof", "--help"]);
    assert!(
        !proof.contains("TASK_ID_FLAG"),
        "query proof --help leaked the internal field name TASK_ID_FLAG\n{proof}"
    );
    // R28: `uninstall` shares AgentArgs with `install`; its help must not point
    // the reader at `maestro install` invocations.
    let uninstall = maestro(&["uninstall", "--help"]);
    assert!(
        !uninstall.contains("maestro install"),
        "uninstall --help showed a `maestro install` example\n{uninstall}"
    );
}
