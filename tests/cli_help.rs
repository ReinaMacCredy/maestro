use std::process::Command;

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
            "update",
            "sync",
            "uninstall",
            "doctor",
            "shell-init",
            "task",
            "event",
            "feature",
            "decision",
            "harness",
            "query",
            "mcp",
            "hook",
            "watch",
            "verify",
            "version",
        ],
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
        ],
    );

    // The refresh trio documents real invocations under an Examples block.
    for command in ["init", "sync", "update"] {
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
            "Create, show, and list decision records in .maestro/decisions/",
            "List, show, apply, and measure harness improvement suggestions",
            "Query computed read models (matrix, friction, decisions, proof, backlog)",
            "Run or inspect the MCP server (serve, stdin, tools, list)",
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
        &maestro(&["harness", "--help"]),
        &["list", "show", "apply", "measure"],
    );
    assert_contains_all(
        &maestro(&["query", "--help"]),
        &["matrix", "friction", "decisions", "backlog", "proof"],
    );
    assert_contains_all(&maestro(&["query", "proof", "--help"]), &["--task-id"]);
    assert_contains_all(
        &maestro(&["mcp", "--help"]),
        &["serve", "stdin", "tools", "list"],
    );
    assert_contains_all(&maestro(&["hook", "--help"]), &["record"]);
    assert_contains_all(&maestro(&["watch", "--help"]), &["snapshot"]);
}
