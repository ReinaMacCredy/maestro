//! `maestro card <verb>` forwards the 10 flat card-store verbs (ready, list,
//! dep, archive, claim, note, create, show, update, close) to the exact flat
//! handlers, so the namespaced spelling an agent guesses is never a dead end
//! and its output is byte-identical to the flat spelling.

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn stdout(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn init_repo(prefix: &str) -> TestTempDir {
    let temp_dir = TestTempDir::new(prefix);
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: .git marker creatable");
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    temp_dir
}

#[test]
fn card_show_output_is_identical_to_flat_show() {
    let repo = init_repo("maestro-card-ns-show");
    let create = ["create", "-t", "task", "Wire the adapter", "--id-only"];
    let id_line = stdout(maestro(&create, repo.path()), &create);
    let id = id_line.trim();

    let flat = stdout(maestro(&["show", id], repo.path()), &["show", id]);
    let namespaced = stdout(
        maestro(&["card", "show", id], repo.path()),
        &["card", "show", id],
    );
    assert_eq!(namespaced, flat, "card show must mirror flat show exactly");
}

#[test]
fn all_ten_card_verbs_forward_to_the_flat_handlers() {
    let repo = init_repo("maestro-card-ns-verbs");
    let root = repo.path();

    // create (via the namespace) -- a feature container plus two child tasks.
    let out = stdout(
        maestro(&["card", "create", "-t", "feature", "CSV export"], root),
        &["card", "create", "-t", "feature", "CSV export"],
    );
    assert!(out.contains("created csv-export (feature)"), "{out}");
    let blocker = stdout(
        maestro(
            &[
                "card",
                "create",
                "-t",
                "task",
                "Parse rows",
                "--parent",
                "csv-export",
                "--id-only",
            ],
            root,
        ),
        &["card", "create", "(blocker)"],
    );
    let blocker = blocker.trim().to_string();
    let child = stdout(
        maestro(
            &[
                "card",
                "create",
                "-t",
                "task",
                "Render rows",
                "--parent",
                "csv-export",
                "--id-only",
            ],
            root,
        ),
        &["card", "create", "(child)"],
    );
    let child = child.trim().to_string();

    // dep -- child waits on blocker.
    let out = stdout(
        maestro(&["card", "dep", "add", &child, &blocker], root),
        &["card", "dep", "add"],
    );
    assert!(out.contains(&child), "{out}");

    // ready -- the blocker is workable, the dep'd child is not.
    let ready = stdout(maestro(&["card", "ready"], root), &["card", "ready"]);
    assert!(ready.contains(&blocker), "{ready}");
    assert!(!ready.contains(&child), "{ready}");

    // list -- both tasks ride under the feature.
    let list = stdout(
        maestro(&["card", "list", "--parent", "csv-export"], root),
        &["card", "list"],
    );
    assert!(list.contains(&blocker) && list.contains(&child), "{list}");

    // claim + note + update on the blocker.
    let out = stdout(
        maestro(&["card", "claim", &blocker], root),
        &["card", "claim"],
    );
    assert!(out.contains("claimed"), "{out}");
    let out = stdout(
        maestro(&["card", "note", &blocker, "namespaced note"], root),
        &["card", "note"],
    );
    assert!(out.contains("noted"), "{out}");
    let out = stdout(
        maestro(
            &["card", "update", &blocker, "--title", "Parse rows fast"],
            root,
        ),
        &["card", "update"],
    );
    assert!(out.contains("updated"), "{out}");

    // close -- both tasks reach the uniform terminal status.
    for id in [&blocker, &child] {
        let out = stdout(maestro(&["card", "close", id], root), &["card", "close"]);
        assert!(out.contains(&format!("closed {id}")), "{out}");
    }

    // archive -- the loose sweep boxes a closed parentless task.
    let loose = stdout(
        maestro(
            &["card", "create", "-t", "task", "Loose chore", "--id-only"],
            root,
        ),
        &["card", "create", "(loose)"],
    );
    let loose = loose.trim().to_string();
    stdout(
        maestro(&["card", "close", &loose], root),
        &["card", "close", "(loose)"],
    );
    let out = stdout(
        maestro(&["card", "archive", "--loose"], root),
        &["card", "archive", "--loose"],
    );
    assert!(out.contains(&format!("boxed: {loose}")), "{out}");
}
