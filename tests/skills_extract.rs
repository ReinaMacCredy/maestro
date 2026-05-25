mod support;

use std::fs;

use maestro::core::backup::backup_operation_timestamp;
use maestro::core::paths::MaestroPaths;
use maestro::skills::bundled::bundled_skills;
use maestro::skills::extract::{extract_bundled_skills, ExtractMode};
use support::TestTempDir;

const BUNDLED_SKILL_NAMES: [&str; 4] = [
    "maestro-task",
    "maestro-setup",
    "maestro-verify",
    "maestro-design",
];

#[test]
fn bundled_skill_list_is_exactly_the_four_v1_skills() {
    let names = bundled_skills()
        .iter()
        .map(|skill| skill.name)
        .collect::<Vec<_>>();

    assert_eq!(names, BUNDLED_SKILL_NAMES);
}

#[test]
fn extract_bundled_skills_writes_each_skill_without_index() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());

    extract_bundled_skills(&paths, ExtractMode::Create)
        .expect("invariant: bundled skills should extract into an empty temp repo");

    for skill in bundled_skills() {
        let path = paths.skills_dir().join(skill.name).join("SKILL.md");
        let contents =
            fs::read_to_string(&path).expect("invariant: extracted skill should be readable");

        assert_eq!(contents, skill.contents);
        assert!(contents.starts_with("---\n"));
        assert!(contents.contains(&format!("name: {}", skill.name)));
        assert!(contents.contains("description: "));
    }
    assert!(!paths.maestro_dir().join("skill-index.yaml").exists());
    assert!(!paths.skills_dir().join("skill-index.yaml").exists());
}

#[test]
fn extract_bundled_skills_preserves_user_added_skills() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let custom_skill = paths.skills_dir().join("custom").join("SKILL.md");
    fs::create_dir_all(
        custom_skill
            .parent()
            .expect("invariant: custom skill path should have a parent"),
    )
    .expect("invariant: custom skill directory should be creatable");
    fs::write(&custom_skill, "custom skill\n").expect("invariant: custom skill should be writable");

    extract_bundled_skills(&paths, ExtractMode::Create)
        .expect("invariant: bundled extraction should not touch custom skills");

    assert_eq!(
        fs::read_to_string(custom_skill).expect("invariant: custom skill should be readable"),
        "custom skill\n"
    );
}

#[cfg(unix)]
#[test]
fn extract_bundled_skills_rejects_symlinked_skill_tree() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let external = TestTempDir::new("maestro-skills-external");
    let paths = MaestroPaths::new(temp_dir.path());
    fs::create_dir_all(paths.maestro_dir()).expect("invariant: maestro dir should be creatable");
    std::os::unix::fs::symlink(external.path(), paths.skills_dir())
        .expect("invariant: symlinked skills dir should be creatable");

    let error = extract_bundled_skills(&paths, ExtractMode::Create)
        .expect_err("invariant: symlinked skills dir should be rejected");

    assert!(error.to_string().contains("symlink"));
    assert!(!external.path().join("maestro-task/SKILL.md").exists());
}

#[cfg(unix)]
#[test]
fn extract_bundled_skills_rejects_symlinked_skill_parent() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let external = TestTempDir::new("maestro-skills-external");
    let paths = MaestroPaths::new(temp_dir.path());
    fs::create_dir_all(paths.skills_dir()).expect("invariant: skills dir should be creatable");
    std::os::unix::fs::symlink(external.path(), paths.skills_dir().join("maestro-task"))
        .expect("invariant: symlinked skill dir should be creatable");

    let error = extract_bundled_skills(&paths, ExtractMode::Create)
        .expect_err("invariant: symlinked skill parent should be rejected");

    assert!(error.to_string().contains("symlink"));
    assert!(!external.path().join("SKILL.md").exists());
}

#[test]
fn extract_bundled_skills_refuses_existing_bundled_file_by_default() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let existing = paths.skills_dir().join("maestro-task").join("SKILL.md");
    fs::create_dir_all(
        existing
            .parent()
            .expect("invariant: bundled skill path should have a parent"),
    )
    .expect("invariant: bundled skill directory should be creatable");
    fs::write(&existing, "custom task\n").expect("invariant: skill should be writable");

    let error = extract_bundled_skills(&paths, ExtractMode::Create)
        .expect_err("invariant: existing bundled skill should be rejected");

    assert!(error.to_string().contains("already exists"));
    assert_eq!(
        fs::read_to_string(existing).expect("invariant: existing skill should be readable"),
        "custom task\n"
    );
}

#[test]
fn extract_bundled_skills_create_preflights_before_writing() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let existing = paths.skills_dir().join("maestro-setup").join("SKILL.md");
    fs::create_dir_all(
        existing
            .parent()
            .expect("invariant: bundled skill path should have a parent"),
    )
    .expect("invariant: bundled skill directory should be creatable");
    fs::write(&existing, "custom setup\n").expect("invariant: skill should be writable");

    let error = extract_bundled_skills(&paths, ExtractMode::Create)
        .expect_err("invariant: existing bundled skill should be rejected");

    assert!(error.to_string().contains("already exists"));
    assert!(!paths.skills_dir().join("maestro-task/SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(existing).expect("invariant: existing skill should be readable"),
        "custom setup\n"
    );
}

#[test]
fn extract_bundled_skills_merge_preserves_existing_bundled_file() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let existing = paths.skills_dir().join("maestro-task").join("SKILL.md");
    fs::create_dir_all(
        existing
            .parent()
            .expect("invariant: bundled skill path should have a parent"),
    )
    .expect("invariant: bundled skill directory should be creatable");
    fs::write(&existing, "custom task\n").expect("invariant: skill should be writable");

    extract_bundled_skills(&paths, ExtractMode::Merge)
        .expect("invariant: merge extraction should preserve existing bundled skills");

    assert_eq!(
        fs::read_to_string(existing).expect("invariant: existing skill should be readable"),
        "custom task\n"
    );
}

#[test]
fn extract_bundled_skills_force_backs_up_existing_bundled_file() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let existing = paths.skills_dir().join("maestro-task").join("SKILL.md");
    fs::create_dir_all(
        existing
            .parent()
            .expect("invariant: bundled skill path should have a parent"),
    )
    .expect("invariant: bundled skill directory should be creatable");
    fs::write(&existing, "custom task\n").expect("invariant: skill should be writable");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    extract_bundled_skills(
        &paths,
        ExtractMode::Force {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: force extraction should overwrite with backup");

    assert_eq!(
        fs::read_to_string(&existing).expect("invariant: rewritten skill should be readable"),
        bundled_skills()[0].contents
    );
    let backup = paths
        .backups_dir()
        .join(format!("{backup_timestamp}-init"))
        .join(".maestro/skills/maestro-task/SKILL.md");
    assert_eq!(
        fs::read_to_string(backup).expect("invariant: backed up skill should be readable"),
        "custom task\n"
    );
}

#[test]
fn extract_bundled_skills_update_skips_unchanged_bundled_files() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    extract_bundled_skills(&paths, ExtractMode::Create)
        .expect("invariant: initial bundled extraction should succeed");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    let report = extract_bundled_skills(
        &paths,
        ExtractMode::Update {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: update extraction should succeed");

    assert!(report.backups.is_empty());
    assert!(!paths
        .backups_dir()
        .join(format!("{backup_timestamp}-update"))
        .exists());
}
