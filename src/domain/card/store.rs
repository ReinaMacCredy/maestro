use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};

use crate::domain::card::schema::Card;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{
    child_dirs, ensure_dir, read_to_string_if_exists, write_string_if_unchanged,
};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{CARD_SCHEMA_VERSION, Compat, classify};
use crate::foundation::core::time::utc_now_millis_timestamp;

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

/// Reject an id that is not a single normal path component, so a verb id like
/// `../../x` or `/etc/x` cannot escape the card store when it is joined into a
/// path. Minted ids are safe by construction, but the read/verb surface
/// accepts arbitrary ids; this mirrors the task and feature id guards.
pub fn validate_card_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid card id: {id}");
    }
    Ok(())
}

/// Sorted ids of the card-bearing child directories of `cards_dir` -- the one
/// walk every per-type scan shares. Symlink-safe via `child_dirs`; skips dirs
/// without a `card.yaml` (the `.alloc-` id-reservation markers are
/// `card.yaml`-less by design). A missing store yields no ids.
pub(crate) fn card_dir_ids(cards_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for (dir, _modified) in child_dirs(cards_dir)? {
        if !dir.join("card.yaml").is_file() {
            continue;
        }
        if let Some(name) = dir.file_name().and_then(|name| name.to_str()) {
            ids.push(name.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

/// Load a card, or `None` when its file does not exist.
pub fn load(path: &Path) -> Result<Option<Card>> {
    Ok(load_with_snapshot(path)?.card)
}

/// Load a card together with the raw bytes backing the next CAS write.
pub fn load_with_snapshot(path: &Path) -> Result<CardSnapshot> {
    // Refuse to follow a symlinked card directory. A `.maestro/cards/<id>` that is
    // a symlink could redirect this load to a card.yaml outside the store; this is
    // the single-load mirror of `cards::scan`'s symlink skip, placed on the shared
    // store seam so feature/decision/harness single-loads are covered too. An
    // absent directory is not a symlink, so card creation (which loads the absent
    // path to obtain a None snapshot) is unaffected.
    if let Some(parent) = path.parent()
        && fs::symlink_metadata(parent)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Ok(CardSnapshot {
            card: None,
            raw: None,
        });
    }
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

/// O3: `card-<hash>` = `card-` + the first 6 lowercase hex of `sha256(input)`.
pub(crate) fn hash_id(input: &str) -> String {
    format!("card-{}", &sha256_hex(input.as_bytes())[..6])
}

/// Mint a stable opaque `card-<hash>` id from `input`, salt-bumping past any id
/// the `is_taken` predicate rejects (SPEC O3, the single hash-id seam migration
/// and creation share). The base is `hash_id(input)`; on a rejection it retries
/// `hash_id("{input}-{N}")` for N=1,2,... until one is accepted. Deterministic
/// for a fixed `input` and predicate (O6), so a migration re-run reproduces the
/// same ids. The two callers differ only in the predicate: the migration tests
/// an in-memory taken set, creation tests on-disk card existence.
pub(crate) fn mint_hash_id(input: &str, mut is_taken: impl FnMut(&str) -> bool) -> String {
    let base = hash_id(input);
    if !is_taken(&base) {
        return base;
    }
    let mut salt = 1u32;
    loop {
        let candidate = hash_id(&format!("{input}-{salt}"));
        if !is_taken(&candidate) {
            return candidate;
        }
        salt += 1;
    }
}

/// Process-local sequence appended to the creation nonce so two cards minted in
/// the same millisecond by the same process still hash apart (SPEC O3').
static CREATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Mint a fresh `card-<hash>` id for a newly created non-feature card (SPEC
/// E2/O3'). The hash input is the title plus a process-unique nonce
/// (millisecond timestamp + pid + a process-local counter), so two cards
/// created back-to-back differ even with identical titles. The disk-existence
/// predicate leaves the create-time CAS (D1) as the real collision guard; the
/// nonce only keeps the first attempt from colliding.
pub(crate) fn mint_card_id(paths: &MaestroPaths, title: &str) -> String {
    let ts = utc_now_millis_timestamp();
    let seq = CREATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let input = format!("{title}|{ts}|{}|{seq}", process::id());
    mint_hash_id(&input, |id| card_path(paths, id).exists())
}

/// [`save_with_snapshot`] for a card rebuilt by a typed-record fold. The fold
/// derives the copy fields from the record mapping, but the card-level fields a
/// record does not carry -- `deps` edges (`dep add`), `lane`, and a card-set
/// `description` -- live only on the existing card, so carry them over before
/// the CAS write would wipe them.
pub(crate) fn save_folded_with_snapshot(
    path: &Path,
    mut card: Card,
    snapshot: &CardSnapshot,
) -> Result<()> {
    if let Some(existing) = &snapshot.card {
        card.deps = existing.deps.clone();
        if card.lane.is_none() {
            card.lane = existing.lane.clone();
        }
        if card.description.is_none() {
            card.description = existing.description.clone();
        }
    }
    save_with_snapshot(path, &card, snapshot)
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

    fn temp_cards_repo(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-{label}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
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

    /// An id joined into `cards/<id>/card.yaml` must be a single normal path
    /// component, or a verb id could address a file outside the store.
    #[test]
    fn validate_card_id_rejects_path_escapes() {
        for bad in ["", ".", "..", "../x", "/etc/passwd", "a/b", "a/../b"] {
            assert!(validate_card_id(bad).is_err(), "{bad:?} must be rejected");
        }
        for good in ["card-a1b2", "csv-export", "task-001"] {
            assert!(validate_card_id(good).is_ok(), "{good:?} must be accepted");
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

    /// O3/O6: the hash id is `card-` + 6 lowercase hex of sha256(input),
    /// deterministic across runs, and salt-bumps to a distinct id when the
    /// predicate rejects the base.
    #[test]
    fn mint_hash_id_is_deterministic_and_salt_bumps() {
        let first = mint_hash_id("task-001", |_| false);
        assert!(
            first.starts_with("card-"),
            "hash id is card-prefixed: {first}"
        );
        assert_eq!(first.len(), 11, "card- (5) + 6 hex");
        assert!(
            first[5..]
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "lowercase hex suffix: {first}"
        );

        // Deterministic: same input + same predicate yields the same id (O6).
        assert_eq!(mint_hash_id("task-001", |_| false), first);

        // Collision: reject the base, so the mint must salt-bump to a new id.
        let bumped = mint_hash_id("task-001", |id| id == first);
        assert_ne!(bumped, first, "salt-bump yields a distinct id");
        assert!(bumped.starts_with("card-"));
    }

    /// O3': two creation mints with the same title yield distinct ids, because
    /// the per-process counter (and timestamp) vary the hash input.
    #[test]
    fn mint_card_id_varies_per_call() {
        let paths = temp_cards_repo("mint-card");

        let first = mint_card_id(&paths, "Add CSV export");
        let second = mint_card_id(&paths, "Add CSV export");
        assert_ne!(first, second, "same title must still mint distinct ids");
        assert!(first.starts_with("card-") && second.starts_with("card-"));

        let _ = std::fs::remove_dir_all(paths.cards_dir());
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
