//! T5: a user-authored `projects:` declaration in harness.yml survives every
//! lifecycle path that could rewrite the config -- `init` (merge), `init
//! --force`, and `sync` -- and a config with no `projects:` key keeps loading
//! unchanged across those same paths.

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use maestro::foundation::core::paths::MaestroPaths;
use maestro::operations::harness::load_config;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd.join("home"))
        .output()
        .expect("invariant: compiled maestro binary should be runnable in projects-lifecycle tests")
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn init(repo: &Path) {
    let output = maestro(&["init", "--yes"], repo);
    assert!(
        output.status.success(),
        "init --yes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn harness_yml_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".maestro/harness/harness.yml")
}

/// Insert a user-authored `projects:` block at the top of the existing
/// harness.yml, the way a maintainer would hand-edit it. The exact globs and
/// their order are what the lifecycle paths must preserve verbatim.
fn author_projects(repo: &Path, projects: &[&str]) {
    let path = harness_yml_path(repo);
    let existing = fs::read_to_string(&path).expect("invariant: harness.yml should be readable");
    let mut block = String::from("projects:\n");
    for project in projects {
        block.push_str(&format!("  - \"{project}\"\n"));
    }
    fs::write(&path, format!("{block}{existing}"))
        .expect("invariant: harness.yml should be writable");
}

/// Read the persisted `projects:` declaration back through the real loader,
/// so the assertion checks parsed content and order, immune to YAML quoting.
fn loaded_projects(repo: &Path) -> Vec<String> {
    let paths = MaestroPaths::new(repo.to_path_buf());
    load_config(&paths)
        .expect("invariant: harness.yml should load")
        .expect("invariant: harness.yml should exist after init")
        .projects
}

#[test]
fn default_init_merge_preserves_authored_projects() {
    let temp = TestTempDir::new("maestro-projects-lifecycle");
    init_git_marker(temp.path());
    init(temp.path());
    author_projects(temp.path(), &["services/*", "apps/*"]);

    let output = maestro(&["init", "--yes"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        loaded_projects(temp.path()),
        vec!["services/*".to_string(), "apps/*".to_string()],
        "default (merge) init must not touch a user-authored harness.yml"
    );
}

#[test]
fn force_init_preserves_authored_projects() {
    let temp = TestTempDir::new("maestro-projects-lifecycle");
    init_git_marker(temp.path());
    init(temp.path());
    author_projects(temp.path(), &["services/*", "apps/*"]);

    let output = maestro(&["init", "--force"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        loaded_projects(temp.path()),
        vec!["services/*".to_string(), "apps/*".to_string()],
        "force init regenerates harness.yml but must carry the authored projects forward verbatim"
    );
}

#[test]
fn sync_preserves_authored_projects() {
    let temp = TestTempDir::new("maestro-projects-lifecycle");
    init_git_marker(temp.path());
    init(temp.path());
    author_projects(temp.path(), &["services/*", "apps/*"]);

    let output = maestro(&["sync"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        loaded_projects(temp.path()),
        vec!["services/*".to_string(), "apps/*".to_string()],
        "sync resyncs bundled resources and must leave harness.yml's projects untouched"
    );
}

#[test]
fn no_projects_key_survives_init_and_sync_round_trip() {
    let temp = TestTempDir::new("maestro-projects-lifecycle");
    init_git_marker(temp.path());
    init(temp.path());

    // The freshly detected config emits no `projects:` key (empty Vec is skipped).
    let before =
        fs::read_to_string(harness_yml_path(temp.path())).expect("invariant: harness.yml readable");
    assert!(
        !before.contains("projects:"),
        "a fresh init must not emit a projects: key: {before}"
    );

    for args in [vec!["init", "--yes"], vec!["sync"], vec!["init", "--force"]] {
        let output = maestro(&args, temp.path());
        assert!(
            output.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let after =
        fs::read_to_string(harness_yml_path(temp.path())).expect("invariant: harness.yml readable");
    assert!(
        !after.contains("projects:"),
        "a config with no projects: key must not gain one across init/sync: {after}"
    );
    assert!(
        loaded_projects(temp.path()).is_empty(),
        "the loader must still see an empty projects declaration"
    );
}
