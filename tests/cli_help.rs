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
            "uninstall",
            "doctor",
            "shell-init",
            "task",
            "event",
            "feature",
            "decision",
            "improve",
            "query",
            "metrics",
            "mcp",
            "hook",
            "watch",
            "verify",
            "identity",
        ],
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
    assert_contains_all(&maestro(&["improve", "--help"]), &["list", "show", "apply"]);
    assert_contains_all(
        &maestro(&["query", "--help"]),
        &["matrix", "friction", "decisions", "backlog", "proof"],
    );
    assert_contains_all(&maestro(&["query", "proof", "--help"]), &["--task-id"]);
    assert_contains_all(&maestro(&["metrics", "--help"]), &["summary"]);
    assert_contains_all(
        &maestro(&["mcp", "--help"]),
        &["serve", "stdin", "tools", "list"],
    );
    assert_contains_all(&maestro(&["hook", "--help"]), &["record"]);
    assert_contains_all(&maestro(&["watch", "--help"]), &["snapshot"]);
}
