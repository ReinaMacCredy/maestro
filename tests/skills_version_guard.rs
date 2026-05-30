//! CI bump guard for shipped skills.
//!
//! A committed `(name, version, sha256(contents))` table for every shipped
//! skill. The test recomputes the hash of the embedded `SKILL.md` contents and
//! asserts it matches the recorded one. Editing a skill body turns this red,
//! forcing you to *notice* the edit and re-record the table (and, when the
//! change is user-visible, bump the `version:` per `AGENTS.md`). It enforces
//! acknowledgement, not a mechanical bump.

use maestro::core::hash::sha256_hex;
use maestro::skills::catalog::skills;

/// `(skill name, shipped version, sha256 of the embedded `SKILL.md` contents)`.
const SKILL_VERSION_GUARD: [(&str, &str, &str); 4] = [
    (
        "maestro-task",
        "1.0.0",
        "7ac2444322903cea06214f013650c70c3de62608fa08485c235b2b60b8d15f57",
    ),
    (
        "maestro-setup",
        "1.0.0",
        "6fed9a0d57624a6fa519cef4d21f7bf814bdd654019667c996cdf5a9b95428d2",
    ),
    (
        "maestro-verify",
        "1.0.0",
        "b9e49b7e32f69ce90de6a06a84a887555bc84f20125a5857808030c6fc5cbb2c",
    ),
    (
        "maestro-design",
        "1.0.0",
        "b93e020aa7c10d69f70131cbb6fccd4674d9ec51cd1e831f9ba1e4602329e927",
    ),
];

#[test]
fn shipped_skill_bodies_and_versions_match_the_recorded_guard() {
    for skill in skills() {
        let (_, version, hash) = SKILL_VERSION_GUARD
            .iter()
            .find(|(name, _, _)| *name == skill.name)
            .copied()
            .expect("invariant: every shipped skill must appear in SKILL_VERSION_GUARD");

        assert_eq!(
            sha256_hex(skill.skill_md().as_bytes()),
            hash,
            "skill {} body changed; bump its `version:` if user-visible, then \
             re-record (version, sha256) in tests/skills_version_guard.rs",
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
