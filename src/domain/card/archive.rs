//! Query-driven card archival (SPEC-beads-model P4, E4/D5). `archive <feature>`
//! moves the feature card and every `parent=<feature>` card from the live
//! `cards/` tree to `archive/cards/`. The flat layout has no directory-move
//! cascade, so the cascade is a query (this is the D5 archive seam that owns it):
//! each matched card's whole directory -- `card.yaml` plus any `spec.md`/`notes.md`
//! sidecars -- moves as a unit.

use std::fs;

use anyhow::{Context, Result, bail};

use crate::domain::card::query::{Coarse, coarse_of, scan};
use crate::domain::card::schema::CardType;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;

/// What an [`archive_feature`] run moved: the feature id and its archived
/// children (the `parent=<feature>` cards), in scan order.
#[derive(Debug)]
pub struct ArchiveReport {
    pub feature: String,
    pub children: Vec<String>,
}

/// Archive a feature card and its children (SPEC E4): the feature card plus
/// every card whose `parent` is the feature (one flat level).
///
/// Gated like the legacy feature archive: the feature AND every child must be
/// coarse-CLOSED (a shipped/cancelled feature, a closed/verified task), else it
/// refuses and lists the open ids -- live work is never archived out from under
/// a dependent. The whole set moves together, so no ref inside it can dangle.
///
/// No-clobber and pre-flight: every target directory is checked before any move,
/// so a name collision aborts the run rather than leaving it half-moved. A move
/// that fails mid-run reports what already moved; recovery is manual (re-running
/// hits the no-clobber guard on the already-archived feature).
pub fn archive_feature(paths: &MaestroPaths, feature: &str) -> Result<ArchiveReport> {
    let cards = scan(paths)?;
    let Some(feature_card) = cards.iter().find(|c| c.id == feature) else {
        bail!("no card {feature} to archive");
    };
    if feature_card.card_type != CardType::Feature {
        bail!(
            "{feature} is a {}, not a feature; archive takes a feature id",
            feature_card.card_type.as_str()
        );
    }

    // The feature + its one-level children (E4 query-driven cascade), in scan
    // order (sorted by id).
    let members: Vec<&str> = cards
        .iter()
        .filter(|c| c.id == feature || c.parent.as_deref() == Some(feature))
        .map(|c| c.id.as_str())
        .collect();

    // Gate: every card in the set must be settled (coarse-CLOSED).
    let open: Vec<&str> = cards
        .iter()
        .filter(|c| members.contains(&c.id.as_str()))
        .filter(|c| coarse_of(&c.status) != Some(Coarse::Closed))
        .map(|c| c.id.as_str())
        .collect();
    if !open.is_empty() {
        bail!(
            "cannot archive {feature} -- {} card(s) not closed: {}; close or cancel them first",
            open.len(),
            open.join(", ")
        );
    }

    // Pre-flight no-clobber: refuse before moving anything if any target exists.
    let archive_root = paths.archive_cards_dir();
    for id in &members {
        let target = archive_root.join(id);
        if target.exists() {
            bail!(
                "cannot archive {feature} -- an archived copy of {id} already exists at {}",
                target.display()
            );
        }
    }

    ensure_dir(&archive_root)?;
    let mut moved: Vec<String> = Vec::new();
    for id in &members {
        let src = paths.cards_dir().join(id);
        let dst = archive_root.join(id);
        fs::rename(&src, &dst).with_context(|| {
            format!(
                "failed to archive {id} ({} -> {}); already moved: [{}]",
                src.display(),
                dst.display(),
                moved.join(", ")
            )
        })?;
        moved.push((*id).to_string());
    }

    Ok(ArchiveReport {
        feature: feature.to_string(),
        children: moved.into_iter().filter(|id| id != feature).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::card::schema::Card;
    use crate::domain::card::store::{card_path, load_with_snapshot, save_with_snapshot};
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

    fn seed(paths: &MaestroPaths, id: &str, ty: CardType, status: &str, parent: Option<&str>) {
        let mut card = Card::new(id, ty, id, status, NOW);
        card.parent = parent.map(str::to_string);
        let path = card_path(paths, id);
        let snap = load_with_snapshot(&path).expect("absent loads None");
        save_with_snapshot(&path, &card, &snap).expect("seed card");
    }

    #[test]
    fn archives_a_shipped_feature_and_its_closed_children() {
        let paths = repo("archive-ok");
        seed(&paths, "csv-export", CardType::Feature, "shipped", None);
        seed(
            &paths,
            "task-001",
            CardType::Task,
            "verified",
            Some("csv-export"),
        );
        seed(
            &paths,
            "task-002",
            CardType::Task,
            "closed",
            Some("csv-export"),
        );
        // a sibling under another feature must stay put
        seed(&paths, "other", CardType::Feature, "shipped", None);
        seed(&paths, "task-003", CardType::Task, "closed", Some("other"));

        let report = archive_feature(&paths, "csv-export").expect("archive succeeds");
        assert_eq!(report.feature, "csv-export");
        assert_eq!(report.children, vec!["task-001", "task-002"]);

        for id in ["csv-export", "task-001", "task-002"] {
            assert!(
                paths
                    .archive_cards_dir()
                    .join(id)
                    .join("card.yaml")
                    .is_file(),
                "{id} moved to the archive"
            );
            assert!(
                !paths.cards_dir().join(id).exists(),
                "{id} left the live store"
            );
        }
        // the unrelated feature and its child are untouched
        assert!(paths.cards_dir().join("other").is_dir());
        assert!(paths.cards_dir().join("task-003").is_dir());
    }

    #[test]
    fn refuses_when_a_member_is_not_closed() {
        let paths = repo("archive-open");
        seed(&paths, "csv-export", CardType::Feature, "shipped", None);
        seed(
            &paths,
            "task-001",
            CardType::Task,
            "in_progress",
            Some("csv-export"),
        );

        let err = archive_feature(&paths, "csv-export").expect_err("an open child blocks archive");
        let msg = err.to_string();
        assert!(msg.contains("task-001"), "the open id is named: {msg}");
        assert!(
            paths.cards_dir().join("csv-export").is_dir(),
            "nothing moved when the gate fails"
        );
    }

    #[test]
    fn refuses_a_non_feature_id_and_a_missing_id() {
        let paths = repo("archive-bad-id");
        seed(&paths, "task-001", CardType::Task, "closed", None);

        assert!(
            archive_feature(&paths, "task-001").is_err(),
            "archive takes a feature, not a task"
        );
        assert!(
            archive_feature(&paths, "ghost").is_err(),
            "a missing id is rejected"
        );
    }

    #[test]
    fn refuses_when_an_archived_copy_already_exists() {
        let paths = repo("archive-clobber");
        seed(&paths, "csv-export", CardType::Feature, "shipped", None);
        seed(
            &paths,
            "task-001",
            CardType::Task,
            "closed",
            Some("csv-export"),
        );
        // a stale archived copy of one child already occupies the target
        ensure_dir(paths.archive_cards_dir().join("task-001")).expect("plant a clobber");

        let err = archive_feature(&paths, "csv-export").expect_err("a clobber aborts the run");
        assert!(err.to_string().contains("already exists"));
        // pre-flight: the feature dir must NOT have moved
        assert!(
            paths.cards_dir().join("csv-export").is_dir(),
            "no-clobber is checked before any move"
        );
    }
}
