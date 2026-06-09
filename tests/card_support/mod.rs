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
    fs::write(temp.path().join(".maestro/harness/harness.yml"), harness_yml)
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

/// The raw `card.yaml` for a card id, parsed as YAML. Top-level carries the card
/// header (`id`/`type`/`title`/`status`/timestamps); a card minted by a legacy
/// entity verb (e.g. `task create`) also carries the verbatim source record under
/// `extra`. Use [`task_record`] to read the task fields directly.
pub fn card_doc(repo: &Path, id: &str) -> Value {
    let path = repo.join(".maestro/cards").join(id).join("card.yaml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("card.yaml for {id} should be readable: {e}"));
    serde_yaml::from_str(&raw).expect("invariant: card.yaml should parse as YAML")
}

/// The folded task record carried under `card.extra` for a card minted by the
/// legacy `task` verbs, so an assertion written against the old `task.yaml`
/// shape (`doc["state"]`, `doc["acceptance"]`, ...) reads unchanged.
pub fn task_record(repo: &Path, id: &str) -> Value {
    card_doc(repo, id)["extra"].clone()
}
