//! CI bump guard for shipped skills.
//!
//! A committed `(name, version, tree-hash)` table for every shipped skill. The
//! test recomputes a hash over the skill's whole directory tree (every file's
//! relative path and bytes, in a canonical sorted order) and asserts it matches
//! the recorded one. Editing any file in a skill tree turns this red, forcing
//! you to *notice* the edit and re-record the table (and, when the change is
//! user-visible, bump the `version:` per `AGENTS.md`). It enforces
//! acknowledgement, not a mechanical bump.

use maestro::core::hash::sha256_hex;
use maestro::skills::catalog::{skills, Skill};

/// `(skill name, shipped version, sha256 tree-hash of the skill directory)`.
const SKILL_VERSION_GUARD: [(&str, &str, &str); 4] = [
    (
        "maestro-task",
        "1.0.0",
        "b55f520c234c90ec4c24d52c190f8741e58f071f2d3b41c05eb2696fd849bc38",
    ),
    (
        "maestro-setup",
        "1.0.0",
        "faa0ac058f347ddc0f4913aa6751573adcc2fa5e2fdb4e0dd41ef9883807a4c7",
    ),
    (
        "maestro-verify",
        "1.0.0",
        "6fa64229274d7204d868a86816437cfd123bc05aaf217947c156089b3e0036c4",
    ),
    (
        "maestro-design",
        "1.0.0",
        "76aec1db895d719f6d44ab73bc9fffbcda55b80841ed1d4ed53de2fabdea7a3b",
    ),
];

/// Hash a skill's whole tree: every file's relative path and bytes, in a
/// canonical order imposed here (sorted by relative path) so the hash is a
/// property of the `(path, bytes)` set and independent of catalog iteration
/// order. Each file is length-prefixed so no separator can be forged by a path
/// or byte payload (matters once a skill ships a binary asset).
fn tree_hash(skill: &Skill) -> String {
    let mut files: Vec<_> = skill.files.iter().collect();
    files.sort_by_key(|file| file.relative_path);

    let mut buf = Vec::new();
    for file in files {
        let path = file.relative_path.as_bytes();
        buf.extend_from_slice(&(path.len() as u32).to_le_bytes());
        buf.extend_from_slice(path);
        buf.extend_from_slice(&(file.contents.len() as u64).to_le_bytes());
        buf.extend_from_slice(file.contents);
    }
    sha256_hex(&buf)
}

#[test]
fn shipped_skill_trees_and_versions_match_the_recorded_guard() {
    for skill in skills() {
        let (_, version, hash) = SKILL_VERSION_GUARD
            .iter()
            .find(|(name, _, _)| *name == skill.name)
            .copied()
            .expect("invariant: every shipped skill must appear in SKILL_VERSION_GUARD");

        assert_eq!(
            tree_hash(skill),
            hash,
            "skill {} tree changed; bump its `version:` if user-visible, then \
             re-record (version, tree-hash) in tests/skills_version_guard.rs",
            skill.name
        );
        assert!(
            skill.skill_md().contains(&format!("version: {version}")),
            "skill {} frontmatter version must match the recorded {version}",
            skill.name
        );
    }
}

#[test]
fn every_recorded_guard_entry_maps_to_a_shipped_skill() {
    for (name, _, _) in SKILL_VERSION_GUARD {
        assert!(
            skills().iter().any(|skill| skill.name == name),
            "SKILL_VERSION_GUARD lists {name}, which is no longer shipped"
        );
    }
}
