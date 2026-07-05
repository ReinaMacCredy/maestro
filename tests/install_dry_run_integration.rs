mod common;
mod support;

use std::fs;
use std::path::Path;

use common::cli_harness::maestro as cli_maestro;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    cli_maestro(cwd)
        .args(args)
        .env("HOME", cwd.join("home").as_os_str())
        .output()
        .into_raw()
}

fn init_repo(prefix: &str) -> TestTempDir {
    let temp = TestTempDir::new(prefix);
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");
    let output = maestro(&["init", "--yes"], temp.path());
    assert!(
        output.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    temp
}

#[test]
fn install_dry_run_exposes_safety_and_shim_plan_without_writing() {
    let repo = init_repo("maestro-install-dry-run");

    let output = maestro(&["install", "--agent", "codex", "--dry-run"], repo.path());

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("install dry-run: codex"), "{stdout}");
    assert!(stdout.contains("writes=false"), "{stdout}");
    assert!(stdout.contains("backup_if_changed=true"), "{stdout}");
    assert!(stdout.contains("managed-block refresh"), "{stdout}");
    assert!(stdout.contains("shim refresh"), "{stdout}");
    assert!(stdout.contains("stale-resource detection"), "{stdout}");
    assert!(stdout.contains("resource guards"), "{stdout}");
    assert!(stdout.contains("AGENTS.md"), "{stdout}");
    assert!(stdout.contains(".codex/config.toml"), "{stdout}");
    assert!(!repo.path().join("AGENTS.md").exists());
    assert!(!repo.path().join(".codex/config.toml").exists());
    assert!(!repo.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn init_sync_install_help_exposes_safe_mutation_paths() {
    let repo = init_repo("maestro-install-help");

    let init = maestro(&["init", "--help"], repo.path());
    assert!(init.status.success());
    let init = String::from_utf8(init.stdout).expect("init help should be UTF-8");
    assert!(init.contains("--dry-run"), "{init}");
    assert!(init.contains("--merge"), "{init}");
    assert!(init.contains("--force"), "{init}");
    assert!(init.contains("backing them up"), "{init}");

    let sync = maestro(&["sync", "--help"], repo.path());
    assert!(sync.status.success());
    let sync = String::from_utf8(sync.stdout).expect("sync help should be UTF-8");
    assert!(sync.contains("--dry-run"), "{sync}");
    assert!(sync.contains("--global-skills"), "{sync}");
    assert!(sync.contains("--adopt-unmanaged"), "{sync}");
    assert!(sync.contains("Back up unmanaged"), "{sync}");

    let install = maestro(&["install", "--help"], repo.path());
    assert!(install.status.success());
    let install = String::from_utf8(install.stdout).expect("install help should be UTF-8");
    assert!(install.contains("--dry-run"), "{install}");
    assert!(install.contains("Preview mirror writes"), "{install}");
}
