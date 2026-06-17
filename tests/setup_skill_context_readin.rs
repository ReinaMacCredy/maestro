//! T6: the maestro-setup skill must instruct a bounded, per-project doc/agent-
//! spec read-in that synthesizes into the single root harness guidance and is
//! read-in only. This is a markdown skill, so its falsifier is a content guard
//! over the shipped SKILL.md (the install/sync root-only half is structurally
//! held by tests/install_mirrors.rs, which only ever plans root-relative
//! CLAUDE.md/AGENTS.md writes). The resources_version_guard forces a version
//! bump + re-record on any edit; this guard asserts the read-in step is
//! actually present, not merely that the file changed.

use maestro::domain::skills::catalog::skills;

fn setup_skill_md() -> &'static str {
    skills()
        .iter()
        .find(|skill| skill.name == "maestro-setup")
        .expect("invariant: maestro-setup skill must be shipped")
        .skill_md()
        // catalog SkillFile contents are 'static, so the borrow outlives the
        // temporary Vec from skills().
        .to_string()
        .leak()
}

#[test]
fn setup_skill_names_the_bounded_doc_set() {
    let md = setup_skill_md();
    assert!(md.contains("BOUNDED"), "must call the read-in set bounded");
    for doc in ["AGENTS.md", "CLAUDE.md", "README.md", "docs/"] {
        assert!(md.contains(doc), "bounded doc set must name {doc}");
    }
}

#[test]
fn setup_skill_enumerates_per_declared_project() {
    let md = setup_skill_md();
    // Enumeration is keyed to the projects: declaration, and the single-repo
    // (no declaration) case must collapse to the repo root alone.
    assert!(
        md.contains("projects:"),
        "per-project enumeration must key off the projects: declaration"
    );
    assert!(
        md.contains("per project"),
        "synthesis must produce one section per project"
    );
    assert!(
        md.contains("No `projects:` declared") || md.contains("no `projects:` declared"),
        "must spell out the single-repo (no declaration) collapse to root only"
    );
}

#[test]
fn setup_skill_is_read_in_only() {
    let md = setup_skill_md();
    assert!(
        md.contains("read-in only"),
        "must declare the read-in is read-in only"
    );
    assert!(
        md.contains("never write maestro-managed guidance into a sub-project"),
        "must forbid writing managed guidance into sub-project specs"
    );
}
