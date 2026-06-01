mod support;

use std::fs;
use std::path::Path;

use maestro::domain::extraction::{ExtractMode, extract_all};
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

/// Count regular files anywhere under `dir`, returning 0 when it is absent.
///
/// Rollback removes written files but leaves the directories extraction
/// created, so a file count (not a directory check) is what proves the writes
/// were unwound.
fn file_count(dir: &Path) -> usize {
    let mut count = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("invariant: directory should be readable: {error}"),
        };
        for entry in entries {
            let path = entry
                .expect("invariant: directory entry should be readable")
                .path();
            if path.is_dir() {
                stack.push(path);
            } else {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn extract_all_rolls_back_earlier_resources_when_a_later_one_fails() {
    let temp_dir = TestTempDir::new("maestro-extract-rollback-test");
    let paths = MaestroPaths::new(temp_dir.path());

    // Pre-create only the harness anchor. In Create mode the harness extractor
    // (which runs last) bails because HARNESS.md already exists, after skills
    // and the hook script have already written. The whole extraction must
    // unwind so it leaves no partial state.
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
        file_count(&paths.skills_dir()),
        0,
        "skills written before the harness failure must be rolled back"
    );
    assert_eq!(
        fs::read_to_string(paths.harness_dir().join("HARNESS.md"))
            .expect("invariant: pre-existing harness anchor should remain"),
        "# pre-existing harness\n",
        "the pre-existing harness anchor must be left untouched"
    );
}
