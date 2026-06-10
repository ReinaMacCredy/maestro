//! Shared card-mode test helpers for the SPEC-beads-model legacy-removal cutover.
//!
//! After the card model became the sole store, creation verbs mint opaque
//! content-hash ids (`card-XXXXXX`) instead of the old sequential `task-001`, so
//! a test can no longer hardcode the id it just created. Recover it by its unique
//! title via the same `card::query::scan` the production board reads
//! (`id_by_title`), the pattern proven in `card_commands_integration`.

#![allow(dead_code)]

use std::fs;
use std::path::Path;

use maestro::domain::card::query;
use maestro::domain::card::schema::CardType;
use maestro::domain::card::store::{CardHome, locate};
use maestro::foundation::core::paths::MaestroPaths;
use serde_yaml::Value;

use crate::support::TestTempDir;

/// A repo already in card mode: `.maestro/cards/` exists, so `store_mode`
/// resolves to Cards and `discover_repo_root` finds `.maestro/`.
pub fn cards_repo(name: &str) -> TestTempDir {
    let temp = TestTempDir::new(name);
    fs::create_dir_all(temp.path().join(".maestro/cards"))
        .expect("invariant: cards dir should be creatable");
    temp
}

/// A card-mode repo that also carries a harness config, for verbs whose
/// behavior reads `.maestro/harness/harness.yml` (verification gating, claims).
pub fn cards_repo_with_harness(name: &str, harness_yml: &str) -> TestTempDir {
    let temp = cards_repo(name);
    fs::create_dir_all(temp.path().join(".maestro/harness"))
        .expect("invariant: harness dir should be creatable");
    fs::write(
        temp.path().join(".maestro/harness/harness.yml"),
        harness_yml,
    )
    .expect("invariant: harness.yml should be writable");
    temp
}

/// Recover a freshly created card's content-hash id by its unique title --
/// content-hash ids do not sort by creation order, so a test that creates a card
/// then drives later verbs against it looks the id up by the title it chose.
pub fn id_by_title(repo: &Path, title: &str) -> String {
    let paths = MaestroPaths::new(repo);
    query::scan(&paths)
        .expect("invariant: card scan should succeed")
        .into_iter()
        .find(|card| card.title == title)
        .unwrap_or_else(|| panic!("no card titled {title:?}"))
        .id
}

/// The minted id of the only `idea` card in the repo. Friction detection mints
/// opaque `card-<hash>` ids (D7 retired the sequential `hb-NNN` mint), so a test
/// that triggers one detection recovers the card by its type.
pub fn sole_idea_id(repo: &Path) -> String {
    let paths = MaestroPaths::new(repo);
    let mut ideas: Vec<String> = query::scan(&paths)
        .expect("invariant: card scan should succeed")
        .into_iter()
        .filter(|card| card.card_type == CardType::Idea)
        .map(|card| card.id)
        .collect();
    assert_eq!(ideas.len(), 1, "expected exactly one idea card: {ideas:?}");
    ideas.remove(0)
}

/// The raw persisted card record for an id, parsed as YAML and resolved through
/// the same store probe production uses -- a dir-backed card's whole file, or
/// the card's own entry from its container file (`decisions.yaml`/`ideas.yaml`).
/// Top-level carries the card header (`id`/`type`/`title`/`status`/timestamps);
/// a card minted by a legacy entity verb (e.g. `task create`) also carries the
/// verbatim source record under `extra`. Use [`task_record`] to read the task
/// fields directly.
pub fn card_doc(repo: &Path, id: &str) -> Value {
    let paths = MaestroPaths::new(repo);
    let home = locate(&paths, id)
        .expect("invariant: card lookup should succeed")
        .unwrap_or_else(|| panic!("no card home found for {id}"));
    let raw = fs::read_to_string(home.path()).unwrap_or_else(|e| {
        panic!(
            "card record for {id} should be readable at {}: {e}",
            home.path().display()
        )
    });
    match home {
        CardHome::Dir(_) => {
            serde_yaml::from_str(&raw).expect("invariant: card.yaml should parse as YAML")
        }
        CardHome::Entry(_) => {
            let entries: Vec<Value> = serde_yaml::from_str(&raw)
                .expect("invariant: container file should parse as a YAML sequence");
            entries
                .into_iter()
                .find(|entry| entry["id"] == id)
                .unwrap_or_else(|| panic!("no entry for {id} in its container file"))
        }
    }
}

/// The folded task record carried under `card.extra` for a card minted by the
/// legacy `task` verbs, so an assertion written against the old `task.yaml`
/// shape (`doc["state"]`, `doc["acceptance"]`, ...) reads unchanged.
pub fn task_record(repo: &Path, id: &str) -> Value {
    card_doc(repo, id)["extra"].clone()
}
