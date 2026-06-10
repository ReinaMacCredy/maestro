//! Task <-> card glue for the SPEC-beads-model P1 dual-read cutover.
//!
//! Mirrors the feature cutover (`domain/feature/registry.rs`): a migrated repo
//! routes task reads and writes through the flat `.maestro/cards/<id>/card.yaml`
//! store; an unmigrated repo keeps the legacy `task.yaml` tree. The task->feature
//! link -- which the legacy store derives from the directory path -- is carried by
//! `card.parent` here, recovered on load and written back on save, because
//! `TaskRecord.feature_id` is `#[serde(skip)]` and never appears in the record
//! mapping (`record_from_card` / `record_to_mapping`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::fold;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardSnapshot};
use crate::domain::task::lookup;
use crate::domain::task::template::{TaskRecord, TaskState};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{Compat, TASK_SCHEMA_VERSION, classify};
use crate::foundation::core::time::utc_now_timestamp;

/// Reconstruct a [`TaskRecord`] from a task card's verbatim source mapping
/// (`extra`, the COPY-design payload), re-checking the task schema the same way
/// the legacy read does, then recovering the path-derived `feature_id` from
/// `card.parent` (the field is `#[serde(skip)]`, so it is never in the mapping).
pub(crate) fn record_from_card(card: Card, artifact: String) -> Result<TaskRecord> {
    // A card minted natively by the card model (DN9 `maestro create`) carries no
    // `extra`, so the verbatim-mapping read below has nothing to parse. Synthesize
    // the record the task subsystem needs from the card's own fields instead. This
    // bridge retires in S4 (E7), when the task lifecycle moves onto the native
    // fields and the carrier disappears; until then `status`/`doctor` must read a
    // canonically-created task card without crashing.
    if card.extra.is_empty() {
        return Ok(record_from_native_card(card));
    }
    let parent = card.parent.clone();
    let mut record: TaskRecord = serde_yaml::from_value(Value::Mapping(card.extra))
        .with_context(|| format!("failed to parse {artifact}"))?;
    if classify(&record.schema_version, TASK_SCHEMA_VERSION) != Compat::Exact {
        return Err(MaestroError::SchemaMismatch {
            artifact,
            expected: TASK_SCHEMA_VERSION,
            found: record.schema_version,
        }
        .into());
    }
    record.feature_id = parent;
    Ok(record)
}

/// Build a [`TaskRecord`] from a native card's own fields (no `extra` carrier).
/// The acceptance contract, verification binding, and fine state history a
/// migrated task carries have no native home yet (the S4 gap), so the record
/// keeps the draft defaults for those; the fine `state` is mapped from the
/// card's coarse status word.
fn record_from_native_card(card: Card) -> TaskRecord {
    let mut record = TaskRecord::draft(&card.id, &card.title, &card.created_at);
    record.feature_id = card.parent;
    record.updated_at = card.updated_at;
    record.claimed_by = card.claimed_by;
    record.claimed_at = card.claimed_at;
    record.state = match card.status.as_str() {
        "in_progress" => TaskState::InProgress,
        "closed" => TaskState::Verified,
        _ => TaskState::Draft,
    };
    record
}

/// Serialize a task record to the mapping the card builder folds into `extra`.
/// Round-trips with [`record_from_card`]; feeding the same mapping the migration
/// reads off `task.yaml` keeps a saved card byte-identical to a migrated one.
/// `feature_id` is `#[serde(skip)]`, so it is absent here and the fold takes the
/// parent explicitly.
fn record_to_mapping(record: &TaskRecord) -> Result<Mapping> {
    match serde_yaml::to_value(record).context("failed to serialize task record")? {
        Value::Mapping(map) => Ok(map),
        _ => bail!("task record did not serialize to a mapping"),
    }
}

/// Fold a task record into its card against the current clock. `updated_at` is
/// read from the record's own mapping, so the clock is only a fallback.
fn card_for(record: &TaskRecord) -> Result<Card> {
    Ok(fold::task_card(
        record.id.clone(),
        record_to_mapping(record)?,
        record.feature_id.clone(),
        &utc_now_timestamp(),
    ))
}

/// Load one task for a read-modify-write: `Some((record, snapshot, card path))`
/// when a `Task`-typed card exists for `id`, else `None`. No archive fallback --
/// the card archive tree is P4 -- so a missing card is simply "task not found".
/// The snapshot is the CAS basis the matching save checks (SPEC D1).
pub(crate) fn load_one(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, CardSnapshot, PathBuf)>> {
    let path = card_store::card_path(paths, id);
    let snapshot = card_store::load_with_snapshot(&path)?;
    let Some(card) = snapshot.card.clone() else {
        return Ok(None);
    };
    if card.card_type != CardType::Task {
        return Ok(None);
    }
    let record = record_from_card(card, path.display().to_string())?;
    Ok(Some((record, snapshot, path)))
}

/// [`load_one`] over the archived card tree (`archive/cards/<id>/card.yaml`),
/// read-only: archived tasks stay immutable, so no save snapshot is taken. The
/// L6b proof read crosses the live/archive boundary through this seam.
pub(crate) fn load_one_archived(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, PathBuf)>> {
    lookup::validate_task_lookup_id(id)?;
    let path = paths.archive_cards_dir().join(id).join("card.yaml");
    let Some(card) = card_store::load(&path)? else {
        return Ok(None);
    };
    if card.card_type != CardType::Task {
        return Ok(None);
    }
    let task_dir = path
        .parent()
        .map(Path::to_path_buf)
        .context("card path is missing parent directory")?;
    let record = record_from_card(card, path.display().to_string())?;
    Ok(Some((record, task_dir)))
}

/// Persist a task record to an explicit card path against its load-time snapshot
/// (the card-store CAS rejects a racing writer, SPEC D1). The save seam takes a
/// path, not `paths`, because the snapshot already carries it.
pub(crate) fn save_at(path: &Path, record: &TaskRecord, snapshot: &CardSnapshot) -> Result<()> {
    let card = card_for(record)?;
    card_store::save_with_snapshot(path, &card, snapshot)
}

/// Reconstruct every live `Task`-typed card with its card directory, sorted by
/// id. `feature_id` is recovered from `card.parent`, so the scan consumers
/// (counts, projections) group correctly with no directory to read.
pub(crate) fn scan(paths: &MaestroPaths) -> Result<Vec<(TaskRecord, PathBuf)>> {
    scan_dir(&paths.cards_dir())
}

/// [`scan`] over an explicit card tree root, so archived task reads
/// (`archive/cards/`) ride the same seam as the live store.
pub(crate) fn scan_dir(cards_dir: &Path) -> Result<Vec<(TaskRecord, PathBuf)>> {
    let mut records = Vec::new();
    for id in card_store::card_dir_ids(cards_dir)? {
        let path = cards_dir.join(&id).join("card.yaml");
        if let Some(card) = card_store::load(&path)?
            && card.card_type == CardType::Task
        {
            let task_dir = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| cards_dir.to_path_buf());
            records.push((
                record_from_card(card, path.display().to_string())?,
                task_dir,
            ));
        }
    }
    Ok(records)
}

/// Create a new task card from a draft record. The write is a CAS against the
/// absent snapshot, so a concurrent create of the same id is rejected (matching
/// the legacy `.alloc-` atomic-create guard). The id is reserved by the caller.
pub(crate) fn create(paths: &MaestroPaths, record: &TaskRecord) -> Result<()> {
    let path = card_store::card_path(paths, &record.id);
    let snapshot = card_store::load_with_snapshot(&path)?;
    if snapshot.card.is_some() {
        bail!("task {} already exists", record.id);
    }
    let card = card_for(record)?;
    card_store::save_with_snapshot(&path, &card, &snapshot)
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::task::template::TaskState;
    use crate::foundation::core::fs::ensure_dir;

    fn card_mode_repo(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-task-cards-{label}-{}-{nanos}",
            process::id()
        ));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    fn parented_draft() -> TaskRecord {
        let mut record = TaskRecord::draft("task-001", "Add CSV export", "2026-06-08T00:00:00Z");
        // Exercise the parent recovery and a non-default lifecycle state: the link
        // is `#[serde(skip)]`, so the round-trip only survives if it folds into
        // `card.parent` and is read back from there.
        record.feature_id = Some("csv-export".to_string());
        record.covers = vec!["ac-1".to_string()];
        record.state = TaskState::InProgress;
        record.acceptance_locked = true;
        record.acceptance.checks = vec!["exports a header row".to_string()];
        record.updated_at = "2026-06-08T01:00:00Z".to_string();
        record
    }

    /// Fidelity: a record folded into a card and read back is byte-identical,
    /// including the path-derived `feature_id` recovered from `card.parent`. This
    /// is why a migrated card and a live-saved card reconstruct the same record.
    #[test]
    fn record_round_trips_through_the_card() {
        let record = parented_draft();
        let card = card_for(&record).expect("fold record into card");

        assert_eq!(card.card_type, CardType::Task);
        assert_eq!(card.id, "task-001");
        assert_eq!(card.parent.as_deref(), Some("csv-export"));
        assert_eq!(
            card.status, "in_progress",
            "status derives from the record state"
        );

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the record");
        assert_eq!(
            reconstructed, record,
            "every field, including the recovered feature_id, survives the round-trip"
        );
    }

    /// SPEC D1 in card mode: two readers each take a load-time snapshot; the first
    /// save wins, the second is rejected because the card store's raw-string CAS
    /// checks the snapshot read at load time, not a fresh one.
    #[test]
    fn card_mode_save_rejects_a_stale_task_writer() {
        let paths = card_mode_repo("stale-writer");
        create(&paths, &parented_draft()).expect("create the task card");

        let (mut winner, winner_snapshot, winner_path) = load_one(&paths, "task-001")
            .expect("first read")
            .expect("card exists");
        let (mut loser, loser_snapshot, loser_path) = load_one(&paths, "task-001")
            .expect("second read")
            .expect("card exists");

        winner.title = "winner".to_string();
        save_at(&winner_path, &winner, &winner_snapshot).expect("first writer commits");

        loser.title = "stale writer".to_string();
        let error = save_at(&loser_path, &loser, &loser_snapshot)
            .expect_err("the stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }
}
