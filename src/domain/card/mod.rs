//! The card model: one typed entity (SPEC-beads-model.md) that folds features,
//! tasks, harness-backlog items, and decisions into a single flat
//! `.maestro/cards/<id>/` store.
//!
//! Slice 1 (P1) is the additive data container plus its CAS-backed store; the
//! four existing entities are untouched until the migration and cutover slices.

pub mod fold;
pub mod schema;
pub mod store;

use crate::foundation::core::paths::MaestroPaths;

/// Which persistence backend a repo's entity verbs read and write
/// (SPEC-beads-model P1 dual-read cutover).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreMode {
    /// `.maestro/cards/` exists: the unified card store is authoritative.
    Cards,
    /// No `cards/` yet: the legacy per-entity trees are authoritative.
    Legacy,
}

/// Pick the store for this repo by whether the card store directory exists.
/// Migration is all-or-nothing, so the repo is never split: an unmigrated repo
/// keeps the untouched legacy path and the live dogfooding repo is never
/// auto-migrated (pen-split). Once a repo has migrated, every entity's
/// persistence routes through the single card-store CAS seam (SPEC D1).
pub fn store_mode(paths: &MaestroPaths) -> StoreMode {
    if paths.cards_dir().is_dir() {
        StoreMode::Cards
    } else {
        StoreMode::Legacy
    }
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn store_mode_follows_cards_dir_existence() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("maestro-storemode-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);

        assert_eq!(
            store_mode(&paths),
            StoreMode::Legacy,
            "no cards/ -> the untouched legacy trees stay authoritative"
        );
        std::fs::create_dir_all(paths.cards_dir()).expect("create cards dir");
        assert_eq!(
            store_mode(&paths),
            StoreMode::Cards,
            "cards/ present -> the unified card store is authoritative"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
