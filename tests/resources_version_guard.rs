//! CI bump guard for shipped, version-gated resources.
//!
//! A committed `(group, name, version, tree-hash)` table for every resource that
//! extracts under the shared version gate (skills, the hook recorder script, the
//! harness protocol) plus the embedded schema contract packs, whose recorded
//! version is the family's current schema stamp. The test recomputes a hash over
//! each resource's files
//! (every relative path and bytes, in canonical sorted order) and asserts it
//! matches the recorded one. Editing any shipped resource turns this red, forcing
//! you to *notice* the edit and re-record the table (and, when the change is
//! user-visible, bump its version per `AGENTS.md`). It enforces acknowledgement,
//! not a mechanical bump.

use include_dir::{Dir, include_dir};
use maestro::domain::skills::catalog::skills;
use maestro::foundation::core::hash::sha256_hex;

/// The shipped schema contract packs (WS5 / D6.2-B), one directory per artifact
/// family. Included here directly, independent of the runtime catalog, so the
/// guard never couples to kernel internals.
static SCHEMAS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/schemas");

/// The shipped hook recorder script (its `# maestro:hook-version:` comment is
/// the version marker the recorder and installer gate on).
const RECORD_SH: &str = include_str!("../embedded/hooks/record.sh");

/// The shipped harness protocol (its frontmatter `version:` is the gate marker).
const HARNESS_MD: &str = include_str!("../embedded/harness/HARNESS.md");
const RECOVERY_MD: &str = include_str!("../embedded/harness/RECOVERY.md");

/// `(group, name, shipped version, sha256 tree-hash of the resource files)`.
const RESOURCE_VERSION_GUARD: [(&str, &str, &str, &str); 16] = [
    (
        "skill",
        "maestro-card",
        "1.9.0",
        "7c1eaa95a9dc8764e940b76e4dadbb781dbf2859f6e38eabce4ce50f26659a04",
    ),
    (
        "skill",
        "maestro-setup",
        "1.4.2",
        "6b22e0f6ab18f647498aa62437dd3709a7757844ce336f04f76e9cf744fcba6b",
    ),
    (
        "skill",
        "maestro-design",
        "1.12.0",
        "8e80469bb8adc9b680901f634231d13a56f260bed60192f0810d360d7deaf151",
    ),
    (
        "skill",
        "maestro-audit",
        "1.3.0",
        "5072f747b73c73d5deba6b754a216224f823fb59857e5351e4e7dbc46242fe17",
    ),
    (
        "hook",
        "record.sh",
        "1.0.2",
        "c1a75218747b8f58ffcd216aa8177d68fffd83376ff82dcf2eb32e40ea2d2fe7",
    ),
    (
        "harness",
        "HARNESS.md",
        "1.13.0",
        "8c5c03f3e99212efc4fec2bda3876ef8c31b072fa506be6112f9c4f0b452f679",
    ),
    (
        "schema",
        "backlog",
        "maestro.card.v1",
        "fda5556f0f296a95d3e9b6213fa2f2dc72f79d59efb5326d6ff4c325abebd663",
    ),
    (
        "schema",
        "card",
        "maestro.card.v1",
        "0d4e24fb3a74575dfa585be641c076ee6de50c3d3b12b7c24f655acd91e7aa14",
    ),
    (
        "schema",
        "decision",
        "maestro.card.v1",
        "15bae87c3fd9b7200454078480e1f14cd06a79d3064e48c15eb12ce42de916f7",
    ),
    (
        "schema",
        "feature",
        "maestro.feature.v2",
        "aa696177f2727c94b339b8fcbb45ba3b10beec5b2552622570278149efea159d",
    ),
    (
        "schema",
        "harness",
        "maestro.harness.v1",
        "a570dbd3acad8e22ec644f17c0e5602e2440f0b0d247be73bd6125cd25415cf4",
    ),
    (
        "schema",
        "install",
        "maestro.install_lock.v1",
        "2f0f9979f49f39583250f5b37be1f6035394f24d02ac35a34489df0fab9954ef",
    ),
    (
        "schema",
        "proof",
        "maestro.verification.v1",
        "5122fac7ed7f4e40fcd122eb8e47da895d58a851fa62e47443414da64f799a6a",
    ),
    (
        "schema",
        "run-event",
        "maestro.event.v1",
        "3ff6c384434913f27635d4ed1419f13adca6d38253a88bb553548b7dd32ffddc",
    ),
    (
        "schema",
        "run-evidence",
        "maestro.run_evidence.v1",
        "66bae6cc9fe317881dc0dfa27793108b9ea0f37609462fe60b039e0328119d98",
    ),
    (
        "schema",
        "task",
        "maestro.task.v2",
        "4cd0e80ef00d83f69df3a266211cf316f1215e57f37f5a3a36057151f13df9a0",
    ),
];

/// Collect every file under an embedded dir, with paths relative to `root`.
fn collect_embedded_files(
    dir: &'static Dir<'static>,
    root: &'static Dir<'static>,
) -> Vec<(&'static str, &'static [u8])> {
    let mut files: Vec<(&'static str, &'static [u8])> = dir
        .files()
        .map(|file| {
            let relative = file
                .path()
                .strip_prefix(root.path())
                .ok()
                .and_then(|path| path.to_str())
                .expect("invariant: an embedded file lives under its root with a UTF-8 path");
            (relative, file.contents())
        })
        .collect();
    for subdir in dir.dirs() {
        files.extend(collect_embedded_files(subdir, root));
    }
    files
}

/// The embedded schema pack directory for one artifact family.
fn schema_pack_dir(family: &str) -> Option<&'static Dir<'static>> {
    SCHEMAS_DIR
        .dirs()
        .find(|dir| dir.path().file_name().and_then(|name| name.to_str()) == Some(family))
}

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
                    // The generated reference/cli.md regenerates on any CLI
                    // change and has its own freshness gate; hashing it here
                    // would force a version bump for every flag edit.
                    .filter(|file| file.relative_path != "reference/cli.md")
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
                tree_hash(&[
                    (name, HARNESS_MD.as_bytes()),
                    ("RECOVERY.md", RECOVERY_MD.as_bytes()),
                ]),
                HARNESS_MD.contains(&format!("version: {version}")),
            ),
            "schema" => {
                let pack = schema_pack_dir(name)
                    .unwrap_or_else(|| panic!("recorded schema pack {name} is no longer shipped"));
                let files = collect_embedded_files(pack, pack);
                let current = files
                    .iter()
                    .find(|(path, _)| *path == "current.yaml")
                    .map(|(_, contents)| String::from_utf8_lossy(contents))
                    .unwrap_or_else(|| panic!("schema pack {name} is missing current.yaml"));
                (
                    tree_hash(&files),
                    current.contains(&format!("schema_version: {version}")),
                )
            }
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
            "schema" => assert!(
                schema_pack_dir(name).is_some(),
                "RESOURCE_VERSION_GUARD lists schema pack {name}, which is no longer shipped"
            ),
            other => panic!("unknown resource group {other} in RESOURCE_VERSION_GUARD"),
        }
    }
}

#[test]
fn every_shipped_schema_pack_is_recorded_in_the_guard() {
    for dir in SCHEMAS_DIR.dirs() {
        let family = dir
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("invariant: an embedded schema pack directory has a UTF-8 name");
        assert!(
            RESOURCE_VERSION_GUARD
                .iter()
                .any(|(group, name, _, _)| *group == "schema" && *name == family),
            "shipped schema pack {family} is missing from RESOURCE_VERSION_GUARD",
        );
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
