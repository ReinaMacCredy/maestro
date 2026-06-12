//! Freshness gate for the generated `reference/cli.md` shipped in every
//! embedded skill: the committed copies must be byte-equal to what this
//! binary's clap model renders, and their header self-check stamp must match
//! the body. Run the ignored `regenerate_cli_md` test to rewrite them after a
//! CLI change -- no skill version bump or guard re-record is required (the
//! version guard excludes cli.md from the tree hash).

use std::fs;
use std::path::PathBuf;

use maestro::domain::skills::catalog::skills;
use maestro::interfaces::cli::reference::{
    REGENERATE_COMMAND, render_cli_reference, verify_self_check,
};

fn cli_md_path(skill: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("embedded/skills")
        .join(skill)
        .join("reference/cli.md")
}

#[test]
fn every_embedded_skill_ships_a_cli_md_matching_the_clap_model() {
    let expected = render_cli_reference();
    for skill in skills() {
        let path = cli_md_path(skill.name);
        let committed = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "missing or unreadable {} ({error}); regenerate: {REGENERATE_COMMAND}",
                path.display()
            )
        });
        assert_eq!(
            committed,
            expected,
            "{} is stale against the clap model; regenerate: {REGENERATE_COMMAND}",
            path.display()
        );
    }
}

#[test]
fn every_committed_cli_md_passes_its_header_self_check() {
    for skill in skills() {
        let path = cli_md_path(skill.name);
        let committed = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "missing or unreadable {} ({error}); regenerate: {REGENERATE_COMMAND}",
                path.display()
            )
        });
        verify_self_check(&committed).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}

#[test]
#[ignore = "writes embedded/skills/*/reference/cli.md; run after a CLI change"]
fn regenerate_cli_md() {
    let content = render_cli_reference();
    for skill in skills() {
        let path = cli_md_path(skill.name);
        let parent = path.parent().expect("cli.md path always has a parent");
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
        fs::write(&path, &content)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
        println!("regenerated {}", path.display());
    }
}
