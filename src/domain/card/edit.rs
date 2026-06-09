//! Write-side card edits (SPEC-beads-model P4): mutations that load a card,
//! change it, and persist through the CAS seam (D1). The read-side counterpart
//! is `query`; both sit above the `store` persistence seam.

use anyhow::{Result, bail};

use crate::domain::card::schema::{Dep, DepKind};
use crate::domain::card::store::{card_path, load_with_snapshot, save_with_snapshot};
use crate::foundation::core::paths::MaestroPaths;

/// Add a `blocks` edge so `child` waits on `parent` (SPEC E1/DN6: the edge is
/// stored on the dependent and gates only its `ready`). Mirrors
/// `bd dep add child parent` -- `child` is the dependent, `parent` the blocker.
///
/// Validates at the user-input boundary: a card cannot block itself, and both
/// cards must exist (a dep to a missing card would dangle, which the card-mode
/// doctor flags under E5 -- failing here keeps the bad ref from being written
/// at all). Idempotent: a second identical edge is a no-op. Returns whether a
/// new edge was written.
pub fn add_blocks_dep(paths: &MaestroPaths, child: &str, parent: &str, now: &str) -> Result<bool> {
    if child == parent {
        bail!("a card cannot block itself: {child}");
    }
    if load_with_snapshot(&card_path(paths, parent))?
        .card
        .is_none()
    {
        bail!("no card {parent} to depend on");
    }

    let child_path = card_path(paths, child);
    let snapshot = load_with_snapshot(&child_path)?;
    let Some(mut card) = snapshot.card.clone() else {
        bail!("no card {child} to add a dependency to");
    };
    if card
        .deps
        .iter()
        .any(|dep| dep.kind == DepKind::Blocks && dep.target == parent)
    {
        return Ok(false);
    }

    card.deps.push(Dep {
        kind: DepKind::Blocks,
        target: parent.to_string(),
    });
    card.updated_at = now.to_string();
    save_with_snapshot(&child_path, &card, &snapshot)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::card::schema::{Card, CardType};
    use crate::domain::card::store::load;
    use crate::foundation::core::fs::ensure_dir;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    const NOW: &str = "2026-06-09T00:00:00Z";

    fn repo(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-{label}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    fn seed(paths: &MaestroPaths, id: &str) {
        let card = Card::new(id, CardType::Task, id, "ready", NOW);
        let path = card_path(paths, id);
        let snap = load_with_snapshot(&path).expect("absent loads None");
        save_with_snapshot(&path, &card, &snap).expect("seed card");
    }

    #[test]
    fn add_writes_a_blocks_edge_onto_the_child() {
        let paths = repo("edit-add");
        seed(&paths, "task-001");
        seed(&paths, "task-002");

        let added = add_blocks_dep(&paths, "task-002", "task-001", "2026-06-09T01:00:00Z")
            .expect("add succeeds");
        assert!(added, "a fresh edge is written");

        let child = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("child exists");
        assert_eq!(child.deps.len(), 1);
        assert_eq!(child.deps[0].kind, DepKind::Blocks);
        assert_eq!(child.deps[0].target, "task-001");
        assert_eq!(
            child.updated_at, "2026-06-09T01:00:00Z",
            "mutation bumps updated_at"
        );

        // the blocker (parent) is never touched
        let parent = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("parent exists");
        assert!(
            parent.deps.is_empty(),
            "the edge lives only on the dependent"
        );
    }

    #[test]
    fn add_is_idempotent() {
        let paths = repo("edit-idem");
        seed(&paths, "task-001");
        seed(&paths, "task-002");

        assert!(add_blocks_dep(&paths, "task-002", "task-001", NOW).expect("first add"));
        assert!(
            !add_blocks_dep(&paths, "task-002", "task-001", NOW).expect("second add"),
            "a duplicate edge is a no-op"
        );
        let child = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("child exists");
        assert_eq!(child.deps.len(), 1, "no duplicate edge appended");
    }

    #[test]
    fn add_rejects_self_block_and_missing_cards() {
        let paths = repo("edit-reject");
        seed(&paths, "task-001");

        assert!(
            add_blocks_dep(&paths, "task-001", "task-001", NOW).is_err(),
            "a card cannot block itself"
        );
        assert!(
            add_blocks_dep(&paths, "task-001", "task-404", NOW).is_err(),
            "the blocker must exist (no dangling ref)"
        );
        assert!(
            add_blocks_dep(&paths, "task-404", "task-001", NOW).is_err(),
            "the dependent must exist"
        );
    }
}
