mod common;
mod support;

use std::path::Path;

use common::cli_harness::maestro as cli_maestro;
use support::TestTempDir;

fn maestro(cwd: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    cli_maestro(cwd)
        .args(args)
        .env("HOME", home.as_os_str())
        .output()
        .into_raw()
}

fn assert_success(output: &std::process::Output, args: &[&str]) {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        stdout(output),
        stderr(output)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
}

#[test]
fn universal_commands_run_without_repo_root() {
    let cwd = TestTempDir::new("maestro-universal-cwd");
    let home = TestTempDir::new("maestro-universal-home");

    let cases: &[(&[&str], &str)] = &[
        (&["shell-init"], "maestro"),
        (&["mcp", "tools"], "status"),
        (&["mcp", "list"], "status"),
        (
            &["sync", "--global-skills", "--dry-run"],
            "global Maestro skills would sync for all supported agents",
        ),
    ];

    for (args, expected_stdout) in cases {
        let output = maestro(cwd.path(), home.path(), args);

        assert_success(&output, args);
        assert!(
            stdout(&output).contains(expected_stdout),
            "maestro {args:?} should print {expected_stdout:?}\nstdout:\n{}",
            stdout(&output)
        );
        assert!(
            !stderr(&output).contains("failed to discover repository root"),
            "universal command leaked repo discovery error:\n{}",
            stderr(&output)
        );
        assert!(
            !cwd.path().join(".maestro").exists(),
            "universal command {:?} must not scaffold .maestro",
            args
        );
    }
}
