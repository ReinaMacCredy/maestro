use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::domain::card::schema::Card;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{
    ensure_dir, read_to_string_if_exists, write_string_if_unchanged,
};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{CARD_SCHEMA_VERSION, Compat, classify};

/// A card and the exact bytes it was read from, captured for the
/// compare-and-set write (SPEC D1). `card` is `None` when the file is absent,
/// so a brand-new card is created by loading the absent snapshot (`raw = None`)
/// and writing against it.
#[derive(Clone, Debug, PartialEq)]
pub struct CardSnapshot {
    pub card: Option<Card>,
    raw: Option<String>,
}

/// Path to a card's record file: `.maestro/cards/<id>/card.yaml`.
pub fn card_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.cards_dir().join(id).join("card.yaml")
}

/// Load a card, or `None` when its file does not exist.
pub fn load(path: &Path) -> Result<Option<Card>> {
    Ok(load_with_snapshot(path)?.card)
}

/// Load a card together with the raw bytes backing the next CAS write.
pub fn load_with_snapshot(path: &Path) -> Result<CardSnapshot> {
    let Some(contents) = read_to_string_if_exists(path)? else {
        return Ok(CardSnapshot {
            card: None,
            raw: None,
        });
    };
    let card: Card = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&card.schema_version, CARD_SCHEMA_VERSION) != Compat::Exact {
        return Err(MaestroError::SchemaMismatch {
            artifact: path.display().to_string(),
            expected: CARD_SCHEMA_VERSION,
            found: card.schema_version,
        }
        .into());
    }
    Ok(CardSnapshot {
        card: Some(card),
        raw: Some(contents),
    })
}

/// Write a card, but only when its file still matches the snapshot it was read
/// from (SPEC D1, the single save-CAS seam). Creates the card directory first
/// so the write-lock marker lands inside it.
pub fn save_with_snapshot(path: &Path, card: &Card, snapshot: &CardSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let contents = serde_yaml::to_string(card).context("failed to serialize card")?;
    write_string_if_unchanged(path, snapshot.raw.as_deref(), &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::card::schema::{Card, CardType, Dep, DepKind};

    fn temp_card_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("maestro-{name}-{}-{nanos}", process::id()))
            .join("card.yaml")
    }

    /// A fixture with every field set to a non-default value, so the round-trip
    /// would catch a wrong `#[serde(rename)]`, a dropping `skip_serializing_if`,
    /// or a mis-spelled enum tag (advisor: a hollow round-trip proves nothing).
    fn full_card() -> Card {
        Card {
            schema_version: CARD_SCHEMA_VERSION.to_string(),
            id: "card-a1b2".to_string(),
            card_type: CardType::Task,
            title: "Add CSV export".to_string(),
            status: "in_progress".to_string(),
            parent: Some("agent-cli-ux".to_string()),
            deps: vec![
                Dep {
                    kind: DepKind::Blocks,
                    target: "card-9f7d".to_string(),
                },
                Dep {
                    kind: DepKind::Related,
                    target: "card-3c4d".to_string(),
                },
                Dep {
                    kind: DepKind::Supersedes,
                    target: "card-5e6f".to_string(),
                },
            ],
            lane: Some("build".to_string()),
            claimed_by: Some("claude#session-1".to_string()),
            claimed_at: Some("2026-06-08T00:00:00Z".to_string()),
            created_at: "2026-06-08T00:00:00Z".to_string(),
            updated_at: "2026-06-08T01:00:00Z".to_string(),
            description: Some("Stream rows to stdout.".to_string()),
            extra: serde_yaml::from_str(
                "legacy_field: kept\nstate_history:\n  - draft\n  - ready\n",
            )
            .expect("invariant: fixture extra parses"),
        }
    }

    #[test]
    fn card_round_trips_through_save_and_load() {
        let path = temp_card_path("card-round-trip");
        let card = full_card();

        let snapshot = load_with_snapshot(&path).expect("invariant: absent card loads as None");
        assert!(
            snapshot.card.is_none(),
            "a fresh card path has no record yet"
        );
        save_with_snapshot(&path, &card, &snapshot).expect("invariant: new card should save");

        let loaded = load(&path)
            .expect("invariant: saved card should load")
            .expect("invariant: saved card should be present");
        assert_eq!(loaded, card, "every field must survive the round-trip");

        let _ = std::fs::remove_dir_all(path.parent().expect("card path has a parent"));
    }

    #[test]
    fn save_with_snapshot_rejects_stale_card_writer() {
        let path = temp_card_path("card-stale-writer");

        let first = load_with_snapshot(&path).expect("invariant: first card load should succeed");
        let second = load_with_snapshot(&path).expect("invariant: second card load should succeed");

        let mut winner = full_card();
        winner.title = "second writer".to_string();
        save_with_snapshot(&path, &winner, &second).expect("invariant: second writer saves first");

        let mut loser = full_card();
        loser.title = "stale writer".to_string();
        let error = save_with_snapshot(&path, &loser, &first)
            .expect_err("stale card writer must be rejected");
        assert!(
            error.to_string().contains("failed to write")
                && format!("{error:#}").contains("changed since it was read; re-run"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(path.parent().expect("card path has a parent"));
    }
}
