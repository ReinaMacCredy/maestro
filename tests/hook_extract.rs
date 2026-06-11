mod support;

use std::fs;
use std::path::PathBuf;

use maestro::domain::extraction::{ExtractMode, extract_hook_script, extract_hook_script_from};
use maestro::foundation::core::backup::backup_operation_timestamp;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

const V1: &str = "# maestro:hook-version: 1.0.0\nexec maestro hook record\n";
const V1_EDITED: &str = "# maestro:hook-version: 1.0.0\n# local tweak\nexec maestro hook record\n";
const V2: &str = "# maestro:hook-version: 2.0.0\nexec maestro hook record\n";

fn record_path(paths: &MaestroPaths) -> PathBuf {
    paths.hooks_dir().join("record.sh")
}

#[test]
fn extract_hook_script_writes_the_bundled_record_script() {
    let temp_dir = TestTempDir::new("maestro-hook-test");
    let paths = MaestroPaths::new(temp_dir.path());

    extract_hook_script(&paths, ExtractMode::Create)
        .expect("invariant: the bundled hook script should extract into an empty repo");

    let contents = fs::read_to_string(record_path(&paths))
        .expect("invariant: extracted record.sh should be readable");
    assert!(contents.contains("# maestro:hook-version:"));
    assert!(
        !contents.contains("@MAESTRO_BIN@"),
        "the installed script must pin a concrete binary path:\n{contents}"
    );
    assert!(
        contents.contains("exec \"$MAESTRO_BIN\" hook record"),
        "the installed script must execute the pinned binary:\n{contents}"
    );
    let binary_line = contents
        .lines()
        .find(|line| line.starts_with("MAESTRO_BIN='"))
        .expect("the installed script declares MAESTRO_BIN");
    let binary_path = binary_line
        .strip_prefix("MAESTRO_BIN='")
        .and_then(|value| value.strip_suffix('\''))
        .expect("MAESTRO_BIN is single-quoted");
    assert!(
        PathBuf::from(binary_path).is_absolute(),
        "MAESTRO_BIN should be absolute: {binary_path}"
    );
}

#[test]
fn extract_hook_script_create_rejects_an_existing_script() {
    let temp_dir = TestTempDir::new("maestro-hook-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = record_path(&paths);
    fs::create_dir_all(
        path.parent()
            .expect("invariant: record.sh path has a parent"),
    )
    .expect("invariant: hooks dir should be creatable");
    fs::write(&path, "custom\n").expect("invariant: record.sh should be writable");

    let error = extract_hook_script(&paths, ExtractMode::Create)
        .expect_err("invariant: an existing record.sh should be rejected in Create mode");

    assert!(error.to_string().contains("already exists"));
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: existing record.sh should be readable"),
        "custom\n"
    );
}

#[test]
fn extract_hook_script_update_preserves_local_edit_when_version_matches() {
    let temp_dir = TestTempDir::new("maestro-hook-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = record_path(&paths);

    extract_hook_script_from(&paths, V1, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");
    fs::write(&path, V1_EDITED).expect("invariant: record.sh should be editable");

    let timestamp =
        backup_operation_timestamp().expect("invariant: a backup timestamp should be available");
    let report = extract_hook_script_from(
        &paths,
        V1,
        ExtractMode::Update {
            backup_timestamp: &timestamp,
        },
    )
    .expect("invariant: a same-version update should succeed");

    assert!(
        report.backups.is_empty(),
        "a same-version update must not back up the edited script"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: record.sh should be readable"),
        V1_EDITED,
        "a local edit must survive a same-version update"
    );
}

#[test]
fn extract_hook_script_update_refreshes_and_backs_up_when_version_differs() {
    let temp_dir = TestTempDir::new("maestro-hook-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = record_path(&paths);

    extract_hook_script_from(&paths, V1, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");
    fs::write(&path, V1_EDITED).expect("invariant: record.sh should be editable");

    let timestamp =
        backup_operation_timestamp().expect("invariant: a backup timestamp should be available");
    let report = extract_hook_script_from(
        &paths,
        V2,
        ExtractMode::Update {
            backup_timestamp: &timestamp,
        },
    )
    .expect("invariant: a differing-version update should succeed");

    assert_eq!(
        report.backups.len(),
        1,
        "a version bump must back up the edited script"
    );
    assert_eq!(report.backups[0].name, "record.sh");
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: record.sh should be readable"),
        V2,
        "a version bump must restore the bundled script"
    );
}

#[test]
fn extract_hook_script_update_migrates_a_pre_version_install() {
    let temp_dir = TestTempDir::new("maestro-hook-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = record_path(&paths);
    fs::create_dir_all(
        path.parent()
            .expect("invariant: record.sh path has a parent"),
    )
    .expect("invariant: hooks dir should be creatable");
    // A pre-version install carries no version marker at all.
    fs::write(&path, "exec maestro hook record\n")
        .expect("invariant: record.sh should be writable");

    let timestamp =
        backup_operation_timestamp().expect("invariant: a backup timestamp should be available");
    let report = extract_hook_script_from(
        &paths,
        V1,
        ExtractMode::Update {
            backup_timestamp: &timestamp,
        },
    )
    .expect("invariant: migrating a pre-version install should succeed");

    assert_eq!(
        report.backups.len(),
        1,
        "a pre-version install must be backed up before migration"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: record.sh should be readable"),
        V1,
        "a pre-version install must be migrated to the bundled script"
    );
}
