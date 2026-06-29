mod support;

use std::fs;
use std::process::Command;

use maestro::domain::design;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd.join("home"))
        .output()
        .expect("invariant: compiled maestro binary should be runnable")
}

fn init_repo() -> TestTempDir {
    let temp = TestTempDir::new("maestro-design-cli");
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");
    temp
}

#[test]
fn init_dry_run_reports_target_style_catalog_and_writes_nothing() {
    let repo = init_repo();

    let output = maestro(&["design", "init", "--dry-run"], repo.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let canonical_repo = repo
        .path()
        .canonicalize()
        .expect("invariant: repo path should canonicalize");
    assert!(stdout.contains("action: dry-run"), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "target: {}",
            canonical_repo.join("DESIGN.md").display()
        )),
        "{stdout}"
    );
    assert!(stdout.contains("style: neutral"), "{stdout}");
    assert!(stdout.contains("source: maestro:neutral"), "{stdout}");
    assert!(stdout.contains("available_styles: 75"), "{stdout}");
    assert!(stdout.contains("design_md_exists: false"), "{stdout}");
    assert!(!repo.path().join("DESIGN.md").exists());
    assert!(!repo.path().join(".maestro").exists());
}

#[test]
fn init_writes_neutral_design_md_only_by_explicit_command() {
    let repo = init_repo();

    let output = maestro(&["design", "init"], repo.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let design_md = fs::read_to_string(repo.path().join("DESIGN.md"))
        .expect("invariant: DESIGN.md should be written");
    assert!(design_md.contains("style: neutral"), "{design_md}");
    assert!(design_md.contains("# DESIGN.md"), "{design_md}");
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let repo = init_repo();
    fs::write(repo.path().join("DESIGN.md"), "user-owned\n")
        .expect("invariant: DESIGN.md should be writable");

    let output = maestro(&["design", "init"], repo.path());

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"), "{stderr}");
    assert!(stderr.contains("--force"), "{stderr}");
    let design_md = fs::read_to_string(repo.path().join("DESIGN.md"))
        .expect("invariant: DESIGN.md should remain readable");
    assert_eq!(design_md, "user-owned\n");
}

#[test]
fn force_overwrites_existing_design_md_with_backup() {
    let repo = init_repo();
    fs::write(repo.path().join("DESIGN.md"), "user-owned\n")
        .expect("invariant: DESIGN.md should be writable");

    let output = maestro(&["design", "init", "--force"], repo.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("backup: "), "{stdout}");
    let backup = fs::read_dir(repo.path().join(".maestro/backups"))
        .expect("invariant: backups dir should be written")
        .flat_map(|entry| entry.map(|entry| entry.path()))
        .find_map(|dir| {
            let candidate = dir.join("DESIGN.md");
            candidate.is_file().then_some(candidate)
        })
        .expect("invariant: DESIGN.md backup should exist");
    assert_eq!(
        fs::read_to_string(backup).expect("invariant: backup should be readable"),
        "user-owned\n"
    );
    let design_md = fs::read_to_string(repo.path().join("DESIGN.md"))
        .expect("invariant: DESIGN.md should remain readable");
    assert!(design_md.contains("style: neutral"), "{design_md}");
}

#[test]
fn awesome_style_dry_run_reports_pin_metadata_and_init_writes_verbatim() {
    let repo = init_repo();

    let dry_run = maestro(
        &[
            "design",
            "init",
            "--style",
            "awesome:voltagent",
            "--dry-run",
        ],
        repo.path(),
    );

    assert!(
        dry_run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&dry_run.stderr)
    );
    let stdout = String::from_utf8_lossy(&dry_run.stdout);
    assert!(stdout.contains("style: awesome:voltagent"), "{stdout}");
    assert!(
        stdout.contains("upstream_repository: https://github.com/VoltAgent/awesome-design-md"),
        "{stdout}"
    );
    assert!(
        stdout.contains("upstream_commit: 664b3e78fd1a298ba11973822da988483256d4b4"),
        "{stdout}"
    );
    assert!(stdout.contains("upstream_license: MIT"), "{stdout}");
    assert!(stdout.contains("upstream_copied_files: 74"), "{stdout}");
    assert!(!repo.path().join("DESIGN.md").exists());

    let write = maestro(
        &["design", "init", "--style", "awesome:voltagent"],
        repo.path(),
    );

    assert!(
        write.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&write.stderr)
    );
    let design_md = fs::read_to_string(repo.path().join("DESIGN.md"))
        .expect("invariant: DESIGN.md should be written");
    assert_eq!(
        design_md,
        design::serve(Some("awesome:voltagent"))
            .expect("invariant: vendored style should serve")
            .contents
    );
}

#[test]
fn unknown_style_fails_loud_with_available_tokens() {
    let repo = init_repo();

    let output = maestro(
        &["design", "init", "--style", "awesome:not-real"],
        repo.path(),
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown design style"), "{stderr}");
    assert!(stderr.contains("awesome:not-real"), "{stderr}");
    assert!(stderr.contains("neutral"), "{stderr}");
    assert!(stderr.contains("awesome:voltagent"), "{stderr}");
    assert!(!repo.path().join("DESIGN.md").exists());
}
