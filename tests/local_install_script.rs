mod support;

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use support::TestTempDir;

#[test]
fn install_local_replaces_by_rename_not_in_place_overwrite() {
    let temp = TestTempDir::new("maestro-local-install-script");
    let source = temp.path().join("new-maestro");
    let destination = temp.path().join("bin/maestro");
    fs::create_dir_all(destination.parent().expect("destination has a parent"))
        .expect("invariant: destination parent should be creatable");
    fs::write(&source, "new binary\n").expect("invariant: source should be writable");
    fs::write(&destination, "old binary\n").expect("invariant: destination should be writable");
    let mut old_handle = File::open(&destination).expect("invariant: old destination opens");

    let output = Command::new("sh")
        .arg("scripts/install-local.sh")
        .arg(&source)
        .arg(&destination)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("invariant: install-local script should run");

    assert!(
        output.status.success(),
        "install-local failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&destination).expect("installed destination should be readable"),
        "new binary\n"
    );
    old_handle
        .seek(SeekFrom::Start(0))
        .expect("old handle should seek");
    let mut old_contents = String::new();
    old_handle
        .read_to_string(&mut old_contents)
        .expect("old handle should remain readable");
    assert_eq!(
        old_contents, "old binary\n",
        "a live reader of the previous executable should keep the old inode"
    );
    assert_eq!(
        fs::metadata(&destination)
            .expect("installed destination should have metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[test]
fn release_instructions_use_the_atomic_local_install_script() {
    let agents = fs::read_to_string("AGENTS.md").expect("AGENTS.md should be readable");

    assert!(
        agents.contains("scripts/install-local.sh"),
        "AGENTS.md should route local release installs through the atomic script"
    );
    assert!(
        !agents.contains("cp target/release/maestro ~/.local/bin/maestro"),
        "AGENTS.md must not recommend in-place overwrite of a running executable"
    );
}
