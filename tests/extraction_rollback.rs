mod support;

use std::fs;

use maestro::domain::extraction::{
    ExtractMode, FolderDecision, extract_all, preview_all, validate_all,
};
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

#[test]
fn extract_all_rolls_back_earlier_resources_when_a_later_one_fails() {
    let temp_dir = TestTempDir::new("maestro-extract-rollback-test");
    let paths = MaestroPaths::new(temp_dir.path());

    // Pre-create only the harness anchor. In Create mode the harness extractor
    // (which runs last) bails because HARNESS.md already exists, after the hook
    // script has already written. The whole extraction must unwind so it leaves
    // no partial state.
    fs::create_dir_all(paths.harness_dir()).expect("invariant: harness dir should be creatable");
    fs::write(
        paths.harness_dir().join("HARNESS.md"),
        "# pre-existing harness\n",
    )
    .expect("invariant: harness anchor should be writable");

    let error = extract_all(&paths, ExtractMode::Create)
        .expect_err("invariant: extract_all must fail when the harness anchor already exists");

    assert!(
        error.to_string().contains("already exists"),
        "unexpected error: {error}"
    );
    assert!(
        !paths.hooks_dir().join("record.sh").exists(),
        "the hook script written before the harness failure must be rolled back"
    );
    assert_eq!(
        fs::read_to_string(paths.harness_dir().join("HARNESS.md"))
            .expect("invariant: pre-existing harness anchor should remain"),
        "# pre-existing harness\n",
        "the pre-existing harness anchor must be left untouched"
    );
}

#[test]
fn update_backs_up_and_removes_obsolete_playbook_folder() {
    let temp_dir = TestTempDir::new("maestro-extract-playbook-update");
    let paths = MaestroPaths::new(temp_dir.path());
    let playbook_dir = temp_dir.path().join(".maestro/playbook");
    fs::create_dir_all(&playbook_dir).expect("invariant: playbook dir should be creatable");
    fs::write(playbook_dir.join("local.md"), "local guide\n")
        .expect("invariant: local playbook file should be writable");

    let report = extract_all(
        &paths,
        ExtractMode::Update {
            backup_timestamp: "20260617T041500Z",
        },
    )
    .expect("update should move obsolete playbook into backups");

    assert!(
        !playbook_dir.exists(),
        "obsolete per-repo playbook should be removed after backup"
    );
    let backup = report
        .backups
        .iter()
        .find(|backup| backup.name == "playbook")
        .expect("playbook cleanup should record its backup");
    assert_eq!(
        fs::read_to_string(backup.path.join("local.md"))
            .expect("backed-up playbook file should remain readable"),
        "local guide\n"
    );
}

#[test]
fn obsolete_playbook_cleanup_is_visible_before_it_mutates() {
    let temp_dir = TestTempDir::new("maestro-extract-playbook-preview");
    let paths = MaestroPaths::new(temp_dir.path());
    let playbook_dir = temp_dir.path().join(".maestro/playbook");
    fs::create_dir_all(&playbook_dir).expect("invariant: playbook dir should be creatable");
    fs::write(playbook_dir.join("local.md"), "local guide\n")
        .expect("invariant: local playbook file should be writable");

    let previews = preview_all(
        &paths,
        ExtractMode::Update {
            backup_timestamp: "20260617T041501Z",
        },
    )
    .expect("preview should include obsolete playbook cleanup");
    let playbook = previews
        .iter()
        .find(|preview| preview.name == "playbook")
        .expect("playbook preview should be present");
    assert_eq!(playbook.decision, FolderDecision::Refresh);
    assert_eq!(playbook.installed_version.as_deref(), Some("obsolete"));

    validate_all(&paths, ExtractMode::Merge).expect("merge keeps obsolete playbook untouched");
    assert!(
        playbook_dir.exists(),
        "read-only preview/validate paths must not remove the playbook"
    );

    let error = validate_all(&paths, ExtractMode::Create)
        .expect_err("create mode should conflict on an existing obsolete playbook");
    assert!(
        error.to_string().contains(".maestro/playbook"),
        "unexpected create-mode error: {error}"
    );
}
