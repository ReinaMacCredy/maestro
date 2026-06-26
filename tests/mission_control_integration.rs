pub mod card_support;
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use card_support::{cards_repo, id_by_title};
use serde_json::Value;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION", "mission-control-test")
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn run(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn artifact_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(base: &Path, path: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        if !path.exists() {
            return;
        }
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap_or_else(|error| panic!("snapshot read_dir {}: {error}", path.display()))
            .map(|entry| entry.expect("invariant: readable dir entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                walk(base, &entry, out);
            } else {
                let relative = entry
                    .strip_prefix(base)
                    .expect("invariant: walked path remains under base")
                    .to_path_buf();
                let bytes = fs::read(&entry)
                    .unwrap_or_else(|error| panic!("snapshot read {}: {error}", entry.display()));
                out.push((relative, bytes));
            }
        }
    }

    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn mission_control_preview_json_render_check_are_read_only() {
    let temp = cards_repo("mission-control-read-only");
    let repo = temp.path();

    run(repo, &["create", "-t", "feature", "Import receipts"]);
    let feature_id = id_by_title(repo, "Import receipts");
    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Parse receipt PDFs",
            "--parent",
            &feature_id,
        ],
    );

    let before = artifact_snapshot(&repo.join(".maestro"));

    let preview = run(
        repo,
        &[
            "mission-control",
            "--preview",
            "--size",
            "120x40",
            "--format",
            "plain",
        ],
    );
    assert!(
        preview.contains("Mission Control")
            && preview.contains("Cards / Features")
            && preview.contains("Activity / Events")
            && preview.contains("Proof / Verify")
            && preview.contains("Import receipts")
            && preview.contains("Read-only restore slice"),
        "preview should restore the Mission Control shell over current data:\n{preview}"
    );

    let json: Value = serde_json::from_str(&run(repo, &["mission-control", "--json"]))
        .expect("mission-control --json should emit JSON");
    assert_eq!(json["schema"], "maestro.mission_control.snapshot.v1");
    assert_eq!(json["config"]["read_only"], true);
    assert_eq!(
        json["config"]["source"],
        "current Maestro card/task/run/proof read models"
    );
    assert!(
        json["features"]
            .as_array()
            .expect("features should be an array")
            .iter()
            .any(|feature| feature["title"] == "Import receipts"),
        "snapshot should include current feature cards: {json:#}"
    );

    let render_check: Value = serde_json::from_str(&run(
        repo,
        &["mission-control", "--render-check", "--size", "120x40"],
    ))
    .expect("mission-control --render-check should emit JSON");
    assert_eq!(render_check["ok"], true);
    let screens: Vec<_> = render_check["screens"]
        .as_array()
        .expect("screens should be an array")
        .iter()
        .map(|screen| screen["screen"].as_str().unwrap_or_default())
        .collect();
    for expected in [
        "dashboard",
        "cards",
        "tasks",
        "activity",
        "proof",
        "config",
        "help",
    ] {
        assert!(
            screens.contains(&expected),
            "render-check should cover {expected}: {render_check:#}"
        );
    }

    let after = artifact_snapshot(&repo.join(".maestro"));
    assert_eq!(
        before, after,
        "mission-control preview/json/render-check must be read-only"
    );

    let watch = run(repo, &["watch", "snapshot"]);
    assert!(
        watch.contains("Import receipts") && watch.contains("Parse receipt PDFs"),
        "watch snapshot should still render after mission-control addition:\n{watch}"
    );
}
