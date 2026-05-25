mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use support::TestTempDir;

fn maestro_shell_init(shell: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("shell-init")
        .env("MAESTRO_SHELL", shell)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests");

    assert!(
        output.status.success(),
        "maestro shell-init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invariant: shell snippet should be valid UTF-8")
}

#[test]
fn shell_init_emits_bash_wrapper_that_updates_current_task_after_success() {
    let snippet = maestro_shell_init("bash");

    assert!(snippet.contains("maestro() {"));
    assert!(snippet.contains("command maestro \"$@\""));
    assert!(snippet.contains("export MAESTRO_CURRENT_TASK=\"$3\""));
    assert!(snippet.contains("unset MAESTRO_CURRENT_TASK"));
    assert!(snippet.contains("return \"$__maestro_status\""));
}

#[test]
fn shell_init_emits_zsh_wrapper_using_posix_exports() {
    let snippet = maestro_shell_init("/bin/zsh");

    assert!(snippet.contains("Maestro shell integration for bash/zsh"));
    assert!(snippet.contains("local __maestro_status"));
    assert!(snippet.contains("[ \"$1\" = \"task\" ] && [ \"$2\" = \"claim\" ]"));
    assert!(snippet.contains("[ \"$1\" = \"task\" ] && [ \"$2\" = \"complete\" ]"));
}

#[test]
fn shell_init_emits_fish_wrapper_that_uses_fish_environment_syntax() {
    let snippet = maestro_shell_init("fish");

    assert!(snippet.contains("function maestro"));
    assert!(snippet.contains("command maestro $argv"));
    assert!(snippet.contains("set -gx MAESTRO_CURRENT_TASK \"$argv[3]\""));
    assert!(snippet.contains("set -e MAESTRO_CURRENT_TASK"));
    assert!(snippet.contains("return $__maestro_status"));
}

#[test]
fn shell_init_prefers_maestro_shell_over_shell_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("shell-init")
        .env("MAESTRO_SHELL", "fish")
        .env("SHELL", "/bin/bash")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests");

    assert!(output.status.success());
    let snippet = String::from_utf8(output.stdout).expect("invariant: snippet should be UTF-8");
    assert!(snippet.contains("function maestro"));
    assert!(!snippet.contains("maestro() {"));
}

#[test]
fn bash_wrapper_preserves_output_exit_code_and_env_independence() {
    if !shell_exists("bash") {
        return;
    }

    let temp = TestTempDir::new("maestro-shell-init");
    let bin_dir = temp.path().join("bin");
    fs::create_dir(&bin_dir).expect("invariant: fake bin directory should be creatable");
    write_fake_maestro(&bin_dir.join("maestro"));

    let snippet = maestro_shell_init("bash");
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").expect("invariant: PATH should be set for integration tests")
    );

    let output = Command::new("bash")
        .arg("-c")
        .arg(
            r#"
set -u
eval "$MAESTRO_SNIPPET"
maestro task claim task-123
claim_status=$?
claim_current=${MAESTRO_CURRENT_TASK-}
maestro task complete task-123 --summary done --claim proof
complete_status=$?
complete_current=${MAESTRO_CURRENT_TASK-unset}
MAESTRO_FAKE_STATUS=7 maestro task claim task-999
failure_status=$?
failure_current=${MAESTRO_CURRENT_TASK-unset}
printf 'claim_status=%s\n' "$claim_status"
printf 'claim_current=%s\n' "$claim_current"
printf 'complete_status=%s\n' "$complete_status"
printf 'complete_current=%s\n' "$complete_current"
printf 'failure_status=%s\n' "$failure_status"
printf 'failure_current=%s\n' "$failure_current"
"#,
        )
        .env("MAESTRO_SNIPPET", snippet)
        .env("PATH", path)
        .output()
        .expect("invariant: bash should execute shell wrapper test");

    assert!(
        output.status.success(),
        "bash wrapper test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(stdout.contains("real:task claim task-123"));
    assert!(stdout.contains("real:task complete task-123 --summary done --claim proof"));
    assert!(stdout.contains("claim_status=0"));
    assert!(stdout.contains("claim_current=task-123"));
    assert!(stdout.contains("complete_status=0"));
    assert!(stdout.contains("complete_current=unset"));
    assert!(stdout.contains("failure_status=7"));
    assert!(stdout.contains("failure_current=unset"));

    let inherited = Command::new("bash")
        .arg("-c")
        .arg("printf '%s' \"${MAESTRO_CURRENT_TASK-unset}\"")
        .env(
            "PATH",
            std::env::var("PATH").expect("invariant: PATH should be set"),
        )
        .output()
        .expect("invariant: bash should execute independence check");
    assert_eq!(String::from_utf8_lossy(&inherited.stdout), "unset");
}

fn shell_exists(shell: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {shell} >/dev/null 2>&1"))
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn write_fake_maestro(path: &Path) {
    fs::write(
        path,
        "#!/bin/sh\nprintf 'real:%s\\n' \"$*\"\nexit \"${MAESTRO_FAKE_STATUS:-0}\"\n",
    )
    .expect("invariant: fake maestro should be writable");
    let mut permissions = fs::metadata(path)
        .expect("invariant: fake maestro metadata should be readable")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .expect("invariant: fake maestro permissions should be writable");
}
