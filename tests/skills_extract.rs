mod support;

use std::fs;

use maestro::domain::skills::catalog::{skills, Skill, SkillFile};
use maestro::domain::skills::extract::{extract_skills, extract_skills_from, ExtractMode};
use maestro::foundation::core::backup::backup_operation_timestamp;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

const BUNDLED_SKILL_NAMES: [&str; 4] = [
    "maestro-task",
    "maestro-setup",
    "maestro-verify",
    "maestro-design",
];

const BUNDLED_SKILL_RESOURCES: [(&str, &str); 4] = [
    (
        "maestro-task",
        include_str!("../embedded/skills/maestro-task/SKILL.md"),
    ),
    (
        "maestro-setup",
        include_str!("../embedded/skills/maestro-setup/SKILL.md"),
    ),
    (
        "maestro-verify",
        include_str!("../embedded/skills/maestro-verify/SKILL.md"),
    ),
    (
        "maestro-design",
        include_str!("../embedded/skills/maestro-design/SKILL.md"),
    ),
];

#[test]
fn bundled_skill_list_is_exactly_the_four_v1_skills() {
    let names = skills().iter().map(|skill| skill.name).collect::<Vec<_>>();

    assert_eq!(names, BUNDLED_SKILL_NAMES);
}

#[test]
fn bundled_skill_contents_match_embedded_resources() {
    for skill in skills() {
        let resource = BUNDLED_SKILL_RESOURCES
            .iter()
            .find_map(|(name, contents)| (*name == skill.name).then_some(*contents))
            .expect("invariant: every bundled skill has a resource fixture");
        assert_eq!(skill.skill_md(), resource);
    }
}

#[test]
fn extract_bundled_skills_writes_each_skill_without_index() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());

    extract_skills(&paths, ExtractMode::Create)
        .expect("invariant: bundled skills should extract into an empty temp repo");

    for skill in skills() {
        let path = paths.skills_dir().join(skill.name).join("SKILL.md");
        let contents =
            fs::read_to_string(&path).expect("invariant: extracted skill should be readable");

        assert_eq!(contents, skill.skill_md());
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

    extract_skills(&paths, ExtractMode::Create)
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

    let error = extract_skills(&paths, ExtractMode::Create)
        .expect_err("invariant: symlinked skills dir should be rejected");

    assert!(error.to_string().contains("symlink"));
    assert!(!external.path().join("maestro-task/SKILL.md").exists());
}

#[cfg(unix)]
#[test]
fn extract_bundled_skills_rejects_symlinked_maestro_root() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let external = TestTempDir::new("maestro-skills-external");
    let paths = MaestroPaths::new(temp_dir.path());
    std::os::unix::fs::symlink(external.path(), paths.maestro_dir())
        .expect("invariant: symlinked maestro dir should be creatable");

    let error = extract_skills(&paths, ExtractMode::Create)
        .expect_err("invariant: symlinked maestro root should be rejected");

    assert!(error.to_string().contains("symlink"));
    assert!(!external
        .path()
        .join("skills/maestro-task/SKILL.md")
        .exists());
}

#[test]
fn bundled_skill_contents_include_activation_logging_instruction() {
    for skill in skills() {
        let skill_md = skill.skill_md();
        assert!(skill_md.contains("skill_activation"));
        assert!(skill_md.contains(skill.name));
        assert!(skill_md.contains("maestro hook record"));
    }
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

    let error = extract_skills(&paths, ExtractMode::Create)
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

    let error = extract_skills(&paths, ExtractMode::Create)
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

    let error = extract_skills(&paths, ExtractMode::Create)
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

    extract_skills(&paths, ExtractMode::Merge)
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

    extract_skills(
        &paths,
        ExtractMode::Force {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: force extraction should overwrite with backup");

    assert_eq!(
        fs::read_to_string(&existing).expect("invariant: rewritten skill should be readable"),
        skills()[0].skill_md()
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
    extract_skills(&paths, ExtractMode::Create)
        .expect("invariant: initial bundled extraction should succeed");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    let report = extract_skills(
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

#[test]
fn extract_skills_update_preserves_local_edit_when_version_matches() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    extract_skills(&paths, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");

    // Locally edit an installed skill while keeping the shipped version.
    let installed = paths.skills_dir().join("maestro-task").join("SKILL.md");
    let edited = "---\nname: maestro-task\nversion: 1.1.0\n---\n\nlocal edit\n";
    fs::write(&installed, edited).expect("invariant: installed skill should be writable");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    let report = extract_skills(
        &paths,
        ExtractMode::Update {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: update extraction should succeed");

    assert!(report.backups.is_empty());
    assert_eq!(
        fs::read_to_string(&installed).expect("invariant: installed skill should be readable"),
        edited,
        "a matching version must preserve the local edit"
    );
}

#[test]
fn extract_skills_update_refreshes_and_backs_up_when_version_differs() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    extract_skills(&paths, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");

    let installed = paths.skills_dir().join("maestro-task").join("SKILL.md");
    let stale = "---\nname: maestro-task\nversion: 0.9.0\n---\n\nstale\n";
    fs::write(&installed, stale).expect("invariant: installed skill should be writable");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    let report = extract_skills(
        &paths,
        ExtractMode::Update {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: update extraction should succeed");

    let shipped = skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should ship");
    assert_eq!(
        fs::read_to_string(&installed).expect("invariant: installed skill should be readable"),
        shipped.skill_md(),
        "a differing version must refresh to the shipped contents"
    );
    assert_eq!(report.backups.len(), 1);
    let backup = paths
        .backups_dir()
        .join(format!("{backup_timestamp}-update"))
        .join(".maestro/skills/maestro-task/SKILL.md");
    assert_eq!(
        fs::read_to_string(backup).expect("invariant: backup should be readable"),
        stale,
        "the stale local copy must be backed up before refresh"
    );
}

#[test]
fn extract_skills_update_refreshes_a_pre_version_install() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    extract_skills(&paths, ExtractMode::Create)
        .expect("invariant: initial extraction should succeed");

    // A pre-versioning install whose frontmatter carries no `version:` reads as
    // None, which differs from the shipped Some(..) and must refresh once.
    let installed = paths.skills_dir().join("maestro-task").join("SKILL.md");
    let pre_version = "---\nname: maestro-task\n---\n\npre-version\n";
    fs::write(&installed, pre_version).expect("invariant: installed skill should be writable");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    let report = extract_skills(
        &paths,
        ExtractMode::Update {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: update extraction should succeed");

    let shipped = skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should ship");
    assert_eq!(
        fs::read_to_string(&installed).expect("invariant: installed skill should be readable"),
        shipped.skill_md(),
        "a missing installed version must refresh to the shipped contents"
    );
    assert_eq!(report.backups.len(), 1);
}

const SYNTHETIC_SKILL_MD: &[u8] =
    b"---\nname: synthetic\nversion: 1.0.0\n---\n\nsynthetic skill body\n";

/// Build a synthetic multi-file skill: a versioned `SKILL.md` plus a nested
/// `reference/guide.md` and a `scripts/run.sh`. Multi-file skills are not yet
/// shipped, so this drives the tree writer through the `extract_skills_from`
/// seam to prove the directory-tree write path.
fn synthetic_multi_file_skill() -> Skill {
    Skill {
        name: "synthetic",
        files: vec![
            SkillFile {
                relative_path: "SKILL.md",
                contents: SYNTHETIC_SKILL_MD,
            },
            SkillFile {
                relative_path: "reference/guide.md",
                contents: b"# guide\n",
            },
            SkillFile {
                relative_path: "scripts/run.sh",
                contents: b"#!/bin/sh\necho run\n",
            },
        ],
    }
}

#[test]
fn extract_skills_from_writes_a_multi_file_tree() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let skill = synthetic_multi_file_skill();

    let report = extract_skills_from(&paths, std::slice::from_ref(&skill), ExtractMode::Create)
        .expect("invariant: a synthetic multi-file skill should extract into an empty repo");

    let skill_dir = paths.skills_dir().join("synthetic");
    assert_eq!(
        fs::read(skill_dir.join("SKILL.md")).expect("invariant: SKILL.md should be readable"),
        SYNTHETIC_SKILL_MD
    );
    assert_eq!(
        fs::read_to_string(skill_dir.join("reference/guide.md"))
            .expect("invariant: nested reference file should be readable"),
        "# guide\n"
    );
    assert_eq!(
        fs::read_to_string(skill_dir.join("scripts/run.sh"))
            .expect("invariant: nested script file should be readable"),
        "#!/bin/sh\necho run\n"
    );
    assert_eq!(
        report.writes.len(),
        3,
        "every file in the tree should be recorded as a write"
    );
}

#[test]
fn extract_skills_from_backs_up_and_refreshes_an_edited_multi_file_tree() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let skill = synthetic_multi_file_skill();
    let skills = std::slice::from_ref(&skill);
    extract_skills_from(&paths, skills, ExtractMode::Create)
        .expect("invariant: initial synthetic extraction should succeed");

    // Install a stale, lower-versioned SKILL.md and locally edit a sibling file.
    let skill_dir = paths.skills_dir().join("synthetic");
    let stale_skill_md = "---\nname: synthetic\nversion: 0.9.0\n---\n\nstale\n";
    fs::write(skill_dir.join("SKILL.md"), stale_skill_md)
        .expect("invariant: installed SKILL.md should be writable");
    fs::write(skill_dir.join("reference/guide.md"), "edited guide\n")
        .expect("invariant: installed reference file should be writable");
    let backup_timestamp =
        backup_operation_timestamp().expect("invariant: backup timestamp should be available");

    let report = extract_skills_from(
        &paths,
        skills,
        ExtractMode::Update {
            backup_timestamp: &backup_timestamp,
        },
    )
    .expect("invariant: synthetic update extraction should succeed");

    // The whole folder refreshes: every installed file is backed up and rewritten.
    assert_eq!(
        fs::read(skill_dir.join("SKILL.md")).expect("invariant: SKILL.md should be readable"),
        SYNTHETIC_SKILL_MD
    );
    assert_eq!(
        fs::read_to_string(skill_dir.join("reference/guide.md"))
            .expect("invariant: refreshed reference file should be readable"),
        "# guide\n"
    );
    assert_eq!(
        report.backups.len(),
        3,
        "every installed tree file is backed up"
    );
    let backup_root = paths
        .backups_dir()
        .join(format!("{backup_timestamp}-update"))
        .join(".maestro/skills/synthetic");
    assert_eq!(
        fs::read_to_string(backup_root.join("SKILL.md"))
            .expect("invariant: backed up SKILL.md should be readable"),
        stale_skill_md
    );
    assert_eq!(
        fs::read_to_string(backup_root.join("reference/guide.md"))
            .expect("invariant: backed up reference file should be readable"),
        "edited guide\n"
    );
}

#[test]
fn extract_skills_from_rolls_back_a_partial_multi_file_write() {
    let temp_dir = TestTempDir::new("maestro-skills-test");
    let paths = MaestroPaths::new(temp_dir.path());
    // Order matters: `reference` is written as a regular file first, so writing
    // `reference/guide.md` next fails (its parent is a file), forcing a rollback
    // of the already-written files within this extraction.
    let skill = Skill {
        name: "synthetic",
        files: vec![
            SkillFile {
                relative_path: "SKILL.md",
                contents: SYNTHETIC_SKILL_MD,
            },
            SkillFile {
                relative_path: "reference",
                contents: b"collides with the directory\n",
            },
            SkillFile {
                relative_path: "reference/guide.md",
                contents: b"# guide\n",
            },
        ],
    };

    let error = extract_skills_from(&paths, std::slice::from_ref(&skill), ExtractMode::Create)
        .expect_err("invariant: writing a file under a path already taken by a file should fail");
    assert!(error
        .to_string()
        .contains("failed to write bundled resource"));

    // Rollback removed the files this extraction had already written.
    let skill_dir = paths.skills_dir().join("synthetic");
    assert!(
        !skill_dir.join("SKILL.md").exists(),
        "a failed tree write must roll back files it already wrote"
    );
    assert!(!skill_dir.join("reference").exists());
}
