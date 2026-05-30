//! CI bump guard for shipped, version-gated resources.
//!
//! A committed `(group, name, version, tree-hash)` table for every resource that
//! extracts under the shared version gate: skills, the hook recorder script, and
//! the harness protocol. The test recomputes a hash over each resource's files
//! (every relative path and bytes, in canonical sorted order) and asserts it
//! matches the recorded one. Editing any shipped resource turns this red, forcing
//! you to *notice* the edit and re-record the table (and, when the change is
//! user-visible, bump its version per `AGENTS.md`). It enforces acknowledgement,
//! not a mechanical bump.

use maestro::core::hash::sha256_hex;
use maestro::skills::catalog::skills;

/// The shipped hook recorder script (its `# maestro:hook-version:` comment is
/// the version marker the recorder and installer gate on).
const RECORD_SH: &str = include_str!("../resources/hooks/record.sh");

/// The shipped harness protocol (its frontmatter `version:` is the gate marker).
const HARNESS_MD: &str = include_str!("../resources/harness/HARNESS.md");

/// `(group, name, shipped version, sha256 tree-hash of the resource files)`.
const RESOURCE_VERSION_GUARD: [(&str, &str, &str, &str); 6] = [
    (
        "skill",
        "maestro-task",
        "1.0.0",
        "b55f520c234c90ec4c24d52c190f8741e58f071f2d3b41c05eb2696fd849bc38",
    ),
    (
        "skill",
        "maestro-setup",
        "1.0.0",
        "faa0ac058f347ddc0f4913aa6751573adcc2fa5e2fdb4e0dd41ef9883807a4c7",
    ),
    (
        "skill",
        "maestro-verify",
        "1.0.0",
        "6fa64229274d7204d868a86816437cfd123bc05aaf217947c156089b3e0036c4",
    ),
    (
        "skill",
        "maestro-design",
        "1.0.0",
        "76aec1db895d719f6d44ab73bc9fffbcda55b80841ed1d4ed53de2fabdea7a3b",
    ),
    (
        "hook",
        "record.sh",
        "1.0.0",
        "9f002dc8744763598966c99f9af7f5713535341aeca0935f63157a69986422b7",
    ),
    (
        "harness",
        "HARNESS.md",
        "1.0.0",
        "dfd7874db3011cffd7dd2c1e62a6942af83fcb9f3879e2126ff3f5890031a431",
    ),
];

/// Hash a resource's files: each `(relative path, bytes)`, sorted by path, each
/// length-prefixed so no separator can be forged by a path or byte payload (it
/// matters once a resource ships a binary asset). For a single-file resource the
/// list has one entry.
fn tree_hash(files: &[(&str, &[u8])]) -> String {
    let mut files: Vec<_> = files.iter().collect();
    files.sort_by_key(|(path, _)| *path);

    let mut buf = Vec::new();
    for (path, contents) in files {
        let path = path.as_bytes();
        buf.extend_from_slice(&(path.len() as u32).to_le_bytes());
        buf.extend_from_slice(path);
        buf.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        buf.extend_from_slice(contents);
    }
    sha256_hex(&buf)
}

#[test]
fn shipped_resource_trees_and_versions_match_the_recorded_guard() {
    for (group, name, version, hash) in RESOURCE_VERSION_GUARD {
        let (actual_hash, version_marker_present) = match group {
            "skill" => {
                let skill = skills()
                    .iter()
                    .find(|skill| skill.name == name)
                    .unwrap_or_else(|| panic!("recorded skill {name} is no longer shipped"));
                let files: Vec<(&str, &[u8])> = skill
                    .files
                    .iter()
                    .map(|file| (file.relative_path, file.contents))
                    .collect();
                (
                    tree_hash(&files),
                    skill.skill_md().contains(&format!("version: {version}")),
                )
            }
            "hook" => (
                tree_hash(&[(name, RECORD_SH.as_bytes())]),
                RECORD_SH.contains(&format!("# maestro:hook-version: {version}")),
            ),
            "harness" => (
                tree_hash(&[(name, HARNESS_MD.as_bytes())]),
                HARNESS_MD.contains(&format!("version: {version}")),
            ),
            other => panic!("unknown resource group {other} in RESOURCE_VERSION_GUARD"),
        };

        assert_eq!(
            actual_hash, hash,
            "{group} {name} changed; bump its version if user-visible, then \
             re-record (version, tree-hash) in tests/resources_version_guard.rs",
        );
        assert!(
            version_marker_present,
            "{group} {name} must declare the recorded version {version}",
        );
    }
}

#[test]
fn every_recorded_guard_entry_maps_to_a_shipped_resource() {
    for (group, name, _, _) in RESOURCE_VERSION_GUARD {
        match group {
            "skill" => assert!(
                skills().iter().any(|skill| skill.name == name),
                "RESOURCE_VERSION_GUARD lists skill {name}, which is no longer shipped"
            ),
            // The hook script and harness protocol are fixed single-file
            // resources Maestro always ships.
            "hook" | "harness" => {}
            other => panic!("unknown resource group {other} in RESOURCE_VERSION_GUARD"),
        }
    }
}

#[test]
fn every_shipped_skill_is_recorded_in_the_guard() {
    for skill in skills() {
        assert!(
            RESOURCE_VERSION_GUARD
                .iter()
                .any(|(group, name, _, _)| *group == "skill" && *name == skill.name),
            "shipped skill {} is missing from RESOURCE_VERSION_GUARD",
            skill.name
        );
    }
}
