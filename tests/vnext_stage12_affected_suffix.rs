use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "maestro-stage12-snapshot-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test directory");
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("remove test directory");
    }
}

fn run_validator(ancestry: &Path, snapshot: &Path) -> std::process::Output {
    Command::new("python3")
        .arg(snapshot.join("tools/vnext_contracts/stage10/validate.py"))
        .args(["--ancestry-repository"])
        .arg(ancestry)
        .args(["--snapshot-root"])
        .arg(snapshot)
        .args(["--final-ref", "HEAD"])
        .output()
        .expect("run Stage12 validator")
}

#[test]
fn affected_suffix_validator_accepts_gitless_snapshot_and_rejects_byte_drift() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = TestDir::new();
    let archive = temp.path.join("snapshot.tar");
    let snapshot = temp.path.join("snapshot");
    fs::create_dir(&snapshot).expect("create snapshot directory");

    let archived = Command::new("git")
        .args(["archive", "--format=tar", "--output"])
        .arg(&archive)
        .arg("HEAD")
        .current_dir(repository)
        .output()
        .expect("archive final ref");
    assert!(
        archived.status.success(),
        "git archive failed: {}",
        String::from_utf8_lossy(&archived.stderr)
    );
    let extracted = Command::new("tar")
        .args(["-xf"])
        .arg(&archive)
        .args(["-C"])
        .arg(&snapshot)
        .output()
        .expect("extract final snapshot");
    assert!(
        extracted.status.success(),
        "snapshot extraction failed: {}",
        String::from_utf8_lossy(&extracted.stderr)
    );
    assert!(!snapshot.join(".git").exists());

    let accepted = run_validator(repository, &snapshot);
    assert!(
        accepted.status.success(),
        "Git-less snapshot validation failed: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let receipt: Value = serde_json::from_slice(&accepted.stdout).expect("validator receipt");
    assert_eq!(
        receipt["affected_suffix_parent"],
        "acd2a469d058f5a17162d3f0a5a44fe394cf6676"
    );
    assert_eq!(
        receipt["affected_suffix_checkpoint"],
        "e03d21b64995a20cfda3e90d706048ca79038f30"
    );
    assert_eq!(
        receipt["affected_suffix_tree"],
        "600171763b9e782d494fa0c04ba5de9a5d7fa5a4"
    );

    fs::write(
        snapshot.join("embedded/vnext/adapter/mcp-tools.v1.json"),
        b"{}\n",
    )
    .expect("mutate snapshot");
    let rejected = run_validator(repository, &snapshot);
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("final snapshot Stage12 bytes differ")
    );
}
