mod support;

use std::fs;
use std::path::PathBuf;

use maestro::domain::harness::extract::{extract_harness, extract_harness_from, ExtractMode};
use maestro::foundation::core::backup::backup_operation_timestamp;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

const V1: &str = "---\nversion: 1.0.0\n---\n# Maestro Harness Protocol\n";
const V1_EDITED: &str = "---\nversion: 1.0.0\n---\n# Maestro Harness Protocol\n\nlocal tweak\n";
const V2: &str = "---\nversion: 2.0.0\n---\n# Maestro Harness Protocol\n";

fn harness_path(paths: &MaestroPaths) -> PathBuf {
    paths.harness_dir().join("HARNESS.md")
}

#[test]
fn extract_harness_writes_the_bundled_protocol() {
    let temp_dir = TestTempDir::new("maestro-harness-extract-test");
    let paths = MaestroPaths::new(temp_dir.path());

    extract_harness(&paths, ExtractMode::Create)
        .expect("invariant: the bundled harness protocol should extract into an empty repo");

    let contents = fs::read_to_string(harness_path(&paths))
        .expect("invariant: extracted HARNESS.md should be readable");
    assert!(contents.contains("version:"));
    assert!(contents.contains("# Maestro Harness Protocol"));
}

#[test]
fn extract_harness_create_rejects_an_existing_protocol() {
    let temp_dir = TestTempDir::new("maestro-harness-extract-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = harness_path(&paths);
    fs::create_dir_all(
        path.parent()
            .expect("invariant: HARNESS.md path has a parent"),
    )
    .expect("invariant: harness dir should be creatable");
    fs::write(&path, "custom\n").expect("invariant: HARNESS.md should be writable");

    let error = extract_harness(&paths, ExtractMode::Create)
        .expect_err("invariant: an existing HARNESS.md should be rejected in Create mode");

    assert!(error.to_string().contains("already exists"));
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: existing HARNESS.md should be readable"),
        "custom\n"
    );
}

#[test]
fn extract_harness_update_preserves_local_edit_when_version_matches() {
    let temp_dir = TestTempDir::new("maestro-harness-extract-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = harness_path(&paths);

    extract_harness_from(&paths, V1, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");
    fs::write(&path, V1_EDITED).expect("invariant: HARNESS.md should be editable");

    let timestamp =
        backup_operation_timestamp().expect("invariant: a backup timestamp should be available");
    let report = extract_harness_from(
        &paths,
        V1,
        ExtractMode::Update {
            backup_timestamp: &timestamp,
        },
    )
    .expect("invariant: a same-version update should succeed");

    assert!(
        report.backups.is_empty(),
        "a same-version update must not back up the edited protocol"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: HARNESS.md should be readable"),
        V1_EDITED,
        "a local edit must survive a same-version update"
    );
}

#[test]
fn extract_harness_update_refreshes_and_backs_up_when_version_differs() {
    let temp_dir = TestTempDir::new("maestro-harness-extract-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = harness_path(&paths);

    extract_harness_from(&paths, V1, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");
    fs::write(&path, V1_EDITED).expect("invariant: HARNESS.md should be editable");

    let timestamp =
        backup_operation_timestamp().expect("invariant: a backup timestamp should be available");
    let report = extract_harness_from(
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
        "a version bump must back up the edited protocol"
    );
    assert_eq!(report.backups[0].name, "HARNESS.md");
    assert_eq!(
        fs::read_to_string(&path).expect("invariant: HARNESS.md should be readable"),
        V2,
        "a version bump must restore the bundled protocol"
    );
}

#[test]
fn extract_harness_update_migrates_a_pre_version_install() {
    let temp_dir = TestTempDir::new("maestro-harness-extract-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let path = harness_path(&paths);
    fs::create_dir_all(
        path.parent()
            .expect("invariant: HARNESS.md path has a parent"),
    )
    .expect("invariant: harness dir should be creatable");
    // A pre-version install carries no frontmatter version at all.
    fs::write(&path, "# Maestro Harness Protocol\n")
        .expect("invariant: HARNESS.md should be writable");

    let timestamp =
        backup_operation_timestamp().expect("invariant: a backup timestamp should be available");
    let report = extract_harness_from(
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
        fs::read_to_string(&path).expect("invariant: HARNESS.md should be readable"),
        V1,
        "a pre-version install must be migrated to the bundled protocol"
    );
}
