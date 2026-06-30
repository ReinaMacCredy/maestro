//! Task <-> card glue for card-backed task records.
//!
//! Mirrors the feature cutover (`domain/feature/registry.rs`): a migrated repo
//! routes task reads and writes through the flat `.maestro/cards/<id>/card.yaml`
//! store; an unmigrated repo keeps the legacy `task.yaml` tree. The task->feature
//! link -- which the legacy store derives from the directory path -- is carried by
//! `card.parent` here, recovered on load and written back on save, because
//! `TaskRecord.feature_id` is `#[serde(skip)]` and never appears in the record
//! mapping. The card stores shared fields in the envelope and type-specific
//! payload under slim `extra` (`record_from_card` / `record_to_mapping`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::domain::card::fold;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardHome, ResolvedCard};
use crate::domain::task::lookup;
use crate::domain::task::template::{TaskRecord, TaskState};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// Reconstruct a [`TaskRecord`] from a task card's slim `extra` payload plus the
/// envelope fields it omits, gating the payload's schema version against the
/// task schema pack, then recovering the path-derived `feature_id` from `card.parent`
/// (the field is `#[serde(skip)]`, so it is never in the mapping).
pub(crate) fn record_from_card(card: Card, artifact: String) -> Result<TaskRecord> {
    // A card minted natively by the card model (DN9 `maestro create`) carries no
    // `extra`, so the slim-payload read below has nothing to parse. Synthesize
    // the record the task subsystem needs from the card's own fields instead.
    // `status` and `doctor` must read a canonically-created task card without
    // crashing while task behavior still consumes TaskRecord.
    if card.extra.is_empty() {
        return Ok(record_from_native_card(card));
    }
    let Card {
        id,
        title,
        status,
        parent,
        project,
        claimed_by,
        claimed_at,
        created_at,
        updated_at,
        extra,
        ..
    } = card;
    let mut extra = extra;
    fold::seed_string_if_absent(&mut extra, "id", &id);
    fold::seed_string_if_absent(&mut extra, "title", &title);
    let record_state = task_state_from_status(&status).unwrap_or(TaskState::Draft);
    fold::seed_string_if_absent(&mut extra, "state", record_state.as_str());
    fold::seed_optional_string_if_absent(&mut extra, "claimed_by", claimed_by.as_deref());
    fold::seed_optional_string_if_absent(&mut extra, "claimed_at", claimed_at.as_deref());
    fold::seed_string_if_absent(&mut extra, "created_at", &created_at);
    fold::seed_string_if_absent(&mut extra, "updated_at", &updated_at);
    fold::ensure_supported_schema(&extra, &artifact, "task")?;
    let mut record: TaskRecord = fold::record_from_extra(extra, &artifact)?;
    // Identity is the envelope's, never the payload's: a divergent `extra.id`
    // would route later saves and lookups at a different logical record.
    record.id = id;
    record.feature_id = parent;
    record.project = project;
    // The card verbs (`update`, `close`, `claim`) write only the top-level copy
    // fields, so they are the freshest source for what they own (SPEC DN3: the
    // card status is the single source of truth). The overlay is conservative:
    // an unrecognized status word and an absent claim keep the record's own.
    record.title = title;
    if let Some(state) = task_state_from_status(&status) {
        record.state = state;
    }
    if claimed_by.is_some() {
        record.claimed_by = claimed_by;
        record.claimed_at = claimed_at;
    }
    Ok(record)
}

/// Map a card status word to the task state it denotes (SPEC DN3 vocabulary).
/// `closed` is the DN3b uniform terminal word (`card close`), folded onto
/// `verified`; an unknown word maps to `None` so callers keep a better source.
fn task_state_from_status(status: &str) -> Option<TaskState> {
    Some(match status {
        "draft" => TaskState::Draft,
        "exploring" => TaskState::Exploring,
        "ready" => TaskState::Ready,
        "in_progress" => TaskState::InProgress,
        "needs_verification" => TaskState::NeedsVerification,
        "verified" | "closed" => TaskState::Verified,
        "rejected" => TaskState::Rejected,
        "abandoned" => TaskState::Abandoned,
        "superseded" => TaskState::Superseded,
        _ => return None,
    })
}

/// Build a [`TaskRecord`] from a native card's own fields (no `extra` carrier).
/// The acceptance contract, verification binding, and fine state history a
/// migrated task carries have no native card fields yet, so the record keeps
/// the draft defaults for those; the fine `state` is mapped from the card's
/// coarse status word.
fn record_from_native_card(card: Card) -> TaskRecord {
    let mut record = TaskRecord::draft(&card.id, &card.title, &card.created_at);
    record.feature_id = card.parent;
    record.project = card.project;
    record.updated_at = card.updated_at;
    record.claimed_by = card.claimed_by;
    record.claimed_at = card.claimed_at;
    record.state = task_state_from_status(&card.status).unwrap_or(TaskState::Draft);
    record
}

/// Fold a task record into its card against the current clock. `updated_at` is
/// read from the record's own mapping, so the clock is only a fallback.
fn card_for(record: &TaskRecord) -> Result<Card> {
    Ok(fold::task_card(
        record.id.clone(),
        fold::record_to_mapping(record, "task record")?,
        record.feature_id.clone(),
        &utc_now_timestamp(),
    ))
}

/// Load one task for a read-modify-write: `Some((record, resolved))` when a
/// `Task`-typed card exists for `id` -- in any home the resolver covers (a
/// `tasks/` pool dir or a pre-migration flat dir) -- else `None`. No archive
/// fallback -- the card archive tree is its own scan -- so a missing card is
/// simply "task not found". The resolved card is the CAS basis the matching
/// save checks (SPEC D1).
pub(crate) fn load_one(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, ResolvedCard)>> {
    let Some(resolved) = card_store::resolve(paths, id)? else {
        return Ok(None);
    };
    if resolved.card.card_type != CardType::Task {
        return Ok(None);
    }
    let record = record_from_card(resolved.card.clone(), resolved.path().display().to_string())?;
    Ok(Some((record, resolved)))
}

/// [`load_one`] over the archived card tree (`archive/cards/`), read-only:
/// archived tasks stay immutable, so the resolve basis is dropped and only
/// the record and its artifact directory return. The L6b proof read crosses
/// the live/archive boundary through this seam.
pub(crate) fn load_one_archived(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, PathBuf)>> {
    lookup::validate_task_lookup_id(id)?;
    let Some(resolved) = crate::domain::card::archive_db::resolve(paths, id)? else {
        return Ok(None);
    };
    if resolved.card.card_type != CardType::Task {
        return Ok(None);
    }
    let task_dir = resolved
        .path
        .parent()
        .map(Path::to_path_buf)
        .context("card path is missing parent directory")?;
    let record = record_from_card(resolved.card.clone(), resolved.path.display().to_string())?;
    Ok(Some((record, task_dir)))
}

/// Persist a task record back to the home it was resolved from (the
/// card-store CAS rejects a racing writer, SPEC D1). The fold derives
/// `blocks` edges from the record's open blockers; a just-resolved blocker
/// releases its edge here, so readiness tracks the blocker list (an id also
/// held by a still-open blocker survives via the fold's own deps).
pub(crate) fn save(record: &TaskRecord, resolved: &ResolvedCard) -> Result<()> {
    let card = card_for(record)?;
    let released: BTreeSet<String> = record
        .blockers
        .iter()
        .filter(|blocker| blocker.resolved_at.is_some())
        .filter_map(|blocker| blocker.blocked_ref.as_ref())
        .map(|reference| reference.id.clone())
        .collect();
    card_store::save_folded_resolved_releasing(card, resolved, &released)
}

/// Reconstruct every live `Task`-typed card with its artifact directory (the
/// dir holding its record -- a `tasks/<id>/` pool dir or a pre-migration flat
/// dir), sorted by id. `feature_id` is recovered from `card.parent`, so the
/// scan consumers (counts, projections) group correctly.
pub(crate) fn scan(paths: &MaestroPaths) -> Result<Vec<(TaskRecord, PathBuf)>> {
    scan_dir(&paths.cards_dir())
}

/// [`scan`] over an explicit card tree root, so archived task reads
/// (`archive/cards/`) ride the same seam as the live store.
pub(crate) fn scan_dir(cards_dir: &Path) -> Result<Vec<(TaskRecord, PathBuf)>> {
    let mut records = Vec::new();
    for (card, path) in crate::domain::card::query::scan_dir_with_paths(cards_dir)? {
        if card.card_type != CardType::Task {
            continue;
        }
        let task_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cards_dir.to_path_buf());
        records.push((
            record_from_card(card, path.display().to_string())?,
            task_dir,
        ));
    }
    Ok(records)
}

/// [`scan`] from an already-loaded card set (the card-aware doctor's one store
/// walk). Strict like `scan`: the first task record that fails to convert
/// surfaces its error.
pub(crate) fn records_in_cards(cards: &[(Card, PathBuf)]) -> Result<Vec<TaskRecord>> {
    let mut records = Vec::new();
    for (card, path) in cards {
        if card.card_type != CardType::Task {
            continue;
        }
        records.push(record_from_card(card.clone(), path.display().to_string())?);
    }
    Ok(records)
}

/// Create a new task card from a draft record, landing in the `tasks/` pool
/// its parent dictates (a feature container's or the root one). The write is
/// a CAS create, so a concurrent create of the same id is rejected (matching
/// the legacy `.alloc-` atomic-create guard). Returns the home so callers can
/// report the landing path.
pub(crate) fn create(
    paths: &MaestroPaths,
    record: &TaskRecord,
    project: Option<String>,
) -> Result<CardHome> {
    let mut card = card_for(record)?;
    card.project = project;
    card_store::create_card(paths, &card)
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::card::schema::{Dep, DepKind};
    use crate::domain::task::blockers;
    use crate::domain::task::template::{BlockerKind, BlockerRef, TaskState};
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

    /// Fidelity: a record folded into a slim card and read back is identical,
    /// including the path-derived `feature_id` recovered from `card.parent`.
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
        for key in [
            "id",
            "title",
            "state",
            "created_at",
            "updated_at",
            "claimed_by",
            "claimed_at",
        ] {
            assert!(
                !card
                    .extra
                    .contains_key(serde_yaml::Value::String(key.to_string())),
                "extra omits envelope-owned {key}"
            );
        }

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the record");
        assert_eq!(
            reconstructed, record,
            "every field, including the recovered feature_id, survives the slim-card round-trip"
        );
    }

    /// The card verbs (`update --status`, `claim`, `update --title`) write only
    /// the top-level copy; the typed read treats that copy as the freshest
    /// source (SPEC DN3) across the FULL fine-state vocabulary -- a
    /// `needs_verification` card must not collapse to draft.
    #[test]
    fn typed_read_overlays_top_level_status_claim_and_title() {
        let record = parented_draft();
        let mut card = card_for(&record).expect("fold record into card");
        card.status = "needs_verification".to_string();
        card.claimed_by = Some("claude#s1".to_string());
        card.claimed_at = Some("2026-06-08T02:00:00Z".to_string());
        card.title = "Add CSV export (retitled)".to_string();

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the record");
        assert_eq!(reconstructed.state, TaskState::NeedsVerification);
        assert_eq!(reconstructed.claimed_by.as_deref(), Some("claude#s1"));
        assert_eq!(
            reconstructed.claimed_at.as_deref(),
            Some("2026-06-08T02:00:00Z")
        );
        assert_eq!(reconstructed.title, "Add CSV export (retitled)");
    }

    /// Conservative overlay: an unrecognized top-level word keeps the
    /// extra-carried state, and the DN3b uniform terminal word (`card close`)
    /// folds onto verified.
    #[test]
    fn status_overlay_is_conservative_and_maps_closed_to_verified() {
        let record = parented_draft();

        let mut typo = card_for(&record).expect("fold record into card");
        typo.extra.insert(
            serde_yaml::Value::String("state".to_string()),
            serde_yaml::Value::String("in_progress".to_string()),
        );
        typo.status = "in-progress".to_string();
        let reconstructed = record_from_card(typo, "test".to_string()).expect("reconstruct");
        assert_eq!(
            reconstructed.state,
            TaskState::InProgress,
            "an unknown word keeps the record's own state"
        );

        let mut closed = card_for(&record).expect("fold record into card");
        closed.status = "closed".to_string();
        let reconstructed = record_from_card(closed, "test".to_string()).expect("reconstruct");
        assert_eq!(reconstructed.state, TaskState::Verified);
    }

    /// A below-floor payload (`maestro.task.v1`) must refuse with the pack's
    /// migrate route BEFORE the typed parse: the v1 shape misses required v2
    /// fields, so a post-parse gate would die as a serde error instead.
    #[test]
    fn below_floor_payload_refuses_with_the_pack_migrate_route() {
        let mut card = Card::new("task-001", CardType::Task, "Legacy task", "draft", "1");
        card.extra.insert(
            serde_yaml::Value::String("schema_version".to_string()),
            serde_yaml::Value::String("maestro.task.v1".to_string()),
        );

        let error = record_from_card(card, "card task-001".to_string())
            .expect_err("a v1 payload must refuse, not parse");
        assert!(
            format!("{error:#}").contains("schema mismatch"),
            "{error:#}"
        );
        let hint = error
            .downcast_ref::<crate::foundation::core::error::MaestroError>()
            .and_then(|typed| typed.hint());
        assert_eq!(hint.as_deref(), Some("run maestro migrate-v2"));
    }

    /// An undeclared payload version (neither current nor a named legacy one)
    /// must refuse naming the read set, with the generic doctor hint.
    #[test]
    fn unknown_payload_version_refuses_naming_the_read_set() {
        let mut card = Card::new("task-002", CardType::Task, "Foreign task", "draft", "1");
        card.extra.insert(
            serde_yaml::Value::String("schema_version".to_string()),
            serde_yaml::Value::String("maestro.galaxy.v9".to_string()),
        );

        let error = record_from_card(card, "card task-002".to_string())
            .expect_err("an unknown payload version must refuse");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("schema mismatch"), "{rendered}");
        assert!(
            rendered.contains("this binary reads maestro.task.v2"),
            "{rendered}"
        );
        let hint = error
            .downcast_ref::<crate::foundation::core::error::MaestroError>()
            .and_then(|typed| typed.hint());
        assert_eq!(hint.as_deref(), Some("run maestro doctor"));
    }

    /// A typed save must not destroy the card-only fields -- dep edges
    /// (`dep add`) and a card-set description carry no record home -- and the
    /// typed claim must lift into the top-level copy so `list`/`ready`/`show`
    /// see it. The draft's parent names no feature container, so the create
    /// falls back to the root `tasks/` pool.
    #[test]
    fn typed_save_preserves_card_only_fields_and_lifts_the_claim() {
        let paths = card_mode_repo("preserve-fields");
        let home = create(&paths, &parented_draft(), None).expect("create the task card");
        assert!(
            home.path().ends_with("tasks/task-001/task.yaml"),
            "an unparented-in-practice task pools at the root: {}",
            home.path().display()
        );

        let resolved = card_store::resolve(&paths, "task-001")
            .expect("resolve the card")
            .expect("card exists");
        let mut card = resolved.card.clone();
        card.deps.push(Dep {
            kind: DepKind::Blocks,
            target: "card-9f7d".to_string(),
        });
        card.description = Some("Stream rows to stdout.".to_string());
        card_store::save_resolved(&card, &resolved).expect("card-verb save");

        let (mut record, resolved) = load_one(&paths, "task-001")
            .expect("typed read")
            .expect("card exists");
        record.claimed_by = Some("claude#s1".to_string());
        record.claimed_at = Some("2026-06-08T02:00:00Z".to_string());
        save(&record, &resolved).expect("typed save");

        let saved = card_store::resolve(&paths, "task-001")
            .expect("reload the card")
            .expect("card present")
            .card;
        assert_eq!(saved.deps.len(), 1, "the dep edge survives the typed save");
        assert_eq!(saved.deps[0].target, "card-9f7d");
        assert_eq!(saved.description.as_deref(), Some("Stream rows to stdout."));
        assert_eq!(
            saved.claimed_by.as_deref(),
            Some("claude#s1"),
            "the typed claim is lifted to the top-level copy"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// D6.6 passthrough: the typed save rebuilds `extra` from the record, so a
    /// foreign key (a newer writer's field) and the top-level unknown bag must
    /// be carried -- while a pack-known field the fold intentionally dropped
    /// (the cleared `claims` list) must NOT resurrect from the old copy.
    #[test]
    fn typed_save_carries_foreign_payload_without_resurrecting_cleared_fields() {
        let paths = card_mode_repo("carry-foreign");
        let mut draft = parented_draft();
        draft.claims = vec!["touched src/export.rs".to_string()];
        create(&paths, &draft, None).expect("create the task card");

        let resolved = card_store::resolve(&paths, "task-001")
            .expect("resolve the card")
            .expect("card exists");
        let mut card = resolved.card.clone();
        card.extra.insert(
            serde_yaml::Value::String("future_extra".to_string()),
            serde_yaml::Value::String("from-a-newer-maestro".to_string()),
        );
        card.unknown.insert(
            serde_yaml::Value::String("future_top".to_string()),
            serde_yaml::Value::String("kept".to_string()),
        );
        card_store::save_resolved(&card, &resolved).expect("seed the foreign fields");

        let (mut record, resolved) = load_one(&paths, "task-001")
            .expect("typed read")
            .expect("card exists");
        record.claims.clear();
        save(&record, &resolved).expect("typed save");

        let saved = card_store::resolve(&paths, "task-001")
            .expect("reload the card")
            .expect("card present")
            .card;
        let key = |name: &str| serde_yaml::Value::String(name.to_string());
        assert_eq!(
            saved.extra.get(key("future_extra")),
            Some(&key("from-a-newer-maestro")),
            "a foreign extra key survives the typed save"
        );
        assert_eq!(
            saved.unknown.get(key("future_top")),
            Some(&key("kept")),
            "the top-level unknown bag survives the typed save"
        );
        assert!(
            !saved.extra.contains_key(key("claims")),
            "the cleared pack-known claims list must not resurrect: {:?}",
            saved.extra
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// The fold derives a `blocks` dep from every open blocker that names an
    /// in-store ref; resolved blockers and ref-less External/Human blockers
    /// derive nothing. Without this, `ready` (which consults only `card.deps`)
    /// lists a blocked task as workable.
    #[test]
    fn fold_derives_blocking_deps_from_open_ref_blockers() {
        let mut record = parented_draft();
        blockers::add_blocker(
            &mut record,
            "blk-001".to_string(),
            BlockerKind::Task,
            Some(BlockerRef {
                kind: BlockerKind::Task,
                id: "card-bbb222".to_string(),
            }),
            "waits on the parser".to_string(),
            "parser lands first".to_string(),
            "2026-06-08T01:00:00Z".to_string(),
        );
        blockers::add_blocker(
            &mut record,
            "blk-002".to_string(),
            BlockerKind::Decision,
            Some(BlockerRef {
                kind: BlockerKind::Decision,
                id: "card-ccc333".to_string(),
            }),
            "ruling pending".to_string(),
            "format undecided".to_string(),
            "2026-06-08T01:00:00Z".to_string(),
        );
        blockers::resolve_blocker(&mut record, "blk-002", "2026-06-08T02:00:00Z".to_string())
            .expect("resolve the decision blocker");
        blockers::add_blocker(
            &mut record,
            "blk-003".to_string(),
            BlockerKind::External,
            None,
            "vendor outage".to_string(),
            "upstream API down".to_string(),
            "2026-06-08T03:00:00Z".to_string(),
        );

        let card = card_for(&record).expect("fold record into card");
        assert_eq!(
            card.deps,
            vec![Dep {
                kind: DepKind::Blocks,
                target: "card-bbb222".to_string(),
            }],
            "only the open ref blocker becomes a blocking dep"
        );
    }

    /// Through the store: a typed save unions the derived blocker edge with a
    /// manually-added dep, and resolving the blocker releases only the derived
    /// edge -- the manual one survives the union's release set.
    #[test]
    fn typed_save_unions_blocker_deps_and_releases_them_on_resolve() {
        let paths = card_mode_repo("blocker-deps");
        create(&paths, &parented_draft(), None).expect("create the task card");

        let resolved = card_store::resolve(&paths, "task-001")
            .expect("resolve the card")
            .expect("card exists");
        let mut card = resolved.card.clone();
        card.deps.push(Dep {
            kind: DepKind::Blocks,
            target: "card-9f7d".to_string(),
        });
        card_store::save_resolved(&card, &resolved).expect("manual dep add");

        let (mut record, resolved) = load_one(&paths, "task-001")
            .expect("typed read")
            .expect("card exists");
        blockers::add_blocker(
            &mut record,
            "blk-001".to_string(),
            BlockerKind::Task,
            Some(BlockerRef {
                kind: BlockerKind::Task,
                id: "card-bbb222".to_string(),
            }),
            "waits on the parser".to_string(),
            "parser lands first".to_string(),
            "2026-06-08T01:00:00Z".to_string(),
        );
        save(&record, &resolved).expect("typed save with the open blocker");

        let saved = card_store::resolve(&paths, "task-001")
            .expect("reload")
            .expect("card present")
            .card;
        let targets: Vec<&str> = saved.deps.iter().map(|dep| dep.target.as_str()).collect();
        assert!(
            targets.contains(&"card-bbb222"),
            "the open blocker derives a blocking dep: {targets:?}"
        );
        assert!(
            targets.contains(&"card-9f7d"),
            "the manual edge survives the typed save: {targets:?}"
        );

        let (mut record, resolved) = load_one(&paths, "task-001")
            .expect("typed reread")
            .expect("card exists");
        blockers::resolve_blocker(&mut record, "blk-001", "2026-06-08T04:00:00Z".to_string())
            .expect("resolve the blocker");
        save(&record, &resolved).expect("typed save after resolve");

        let saved = card_store::resolve(&paths, "task-001")
            .expect("reload after resolve")
            .expect("card present")
            .card;
        let targets: Vec<&str> = saved.deps.iter().map(|dep| dep.target.as_str()).collect();
        assert!(
            !targets.contains(&"card-bbb222"),
            "the resolved blocker's derived edge is released: {targets:?}"
        );
        assert!(
            targets.contains(&"card-9f7d"),
            "the manual edge is kept: {targets:?}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// SPEC D1 in card mode: two readers each take a load-time snapshot; the first
    /// save wins, the second is rejected because the card store's raw-string CAS
    /// checks the snapshot read at load time, not a fresh one.
    #[test]
    fn card_mode_save_rejects_a_stale_task_writer() {
        let paths = card_mode_repo("stale-writer");
        create(&paths, &parented_draft(), None).expect("create the task card");

        let (mut winner, winner_resolved) = load_one(&paths, "task-001")
            .expect("first read")
            .expect("card exists");
        let (mut loser, loser_resolved) = load_one(&paths, "task-001")
            .expect("second read")
            .expect("card exists");

        winner.title = "winner".to_string();
        save(&winner, &winner_resolved).expect("first writer commits");

        loser.title = "stale writer".to_string();
        let error = save(&loser, &loser_resolved).expect_err("the stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }
}
