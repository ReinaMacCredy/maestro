//! An unrecognized subcommand must land in the main.rs no-dead-end funnel:
//! clap's error stays verbatim (exit 2 preserved) plus exactly one `fix:`
//! recovery line, so a guessed verb never strands an agent without a next step.

use std::process::Command;

fn maestro(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .output()
        .expect("invariant: compiled maestro binary should be runnable")
}

#[test]
fn unknown_subcommand_exits_2_with_clap_error_plus_one_recovery_hint() {
    let output = maestro(&["tasks"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized subcommand 'tasks'"),
        "{stderr}"
    );
    assert!(stderr.contains("Usage: maestro"), "{stderr}");
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("fix: "))
            .count(),
        1,
        "{stderr}"
    );
    assert!(
        stderr.contains("fix: pick a verb from the skill's reference/cli.md or maestro --help"),
        "{stderr}"
    );
    assert!(output.stdout.is_empty(), "errors must not leak onto stdout");
}

#[test]
fn unknown_nested_subcommand_gets_the_same_recovery_hint() {
    let output = maestro(&["task", "frobnicate"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized subcommand 'frobnicate'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("fix: pick a verb from the skill's reference/cli.md or maestro --help"),
        "{stderr}"
    );
}

#[test]
fn other_parse_errors_keep_plain_clap_output() {
    let output = maestro(&["--bogus"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unexpected argument"), "{stderr}");
    assert!(!stderr.contains("fix:"), "{stderr}");
}

#[test]
fn help_and_version_keep_exit_0() {
    for flag in ["--help", "--version"] {
        let output = maestro(&[flag]);
        assert_eq!(output.status.code(), Some(0), "{flag} should exit 0");
        assert!(!output.stdout.is_empty(), "{flag} should print to stdout");
    }
}
