mod support;

use std::fs;

use maestro::domain::extraction::{ExtractMode, extract_all};
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
