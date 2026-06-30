use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};

use crate::domain::card::fold;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::{archive_db, live_db};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{
    child_dirs, ensure_dir, read_to_string_if_exists, remove_dir_if_file_unchanged,
    sorted_child_dirs, write_string_if_unchanged,
};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{CARD_SCHEMA_VERSION, Compat, classify};
use crate::foundation::core::slug::slugify_ascii;
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
/// `card.yaml`-less by design) and dot-prefixed dirs (a crash-leaked
/// `.<id>.removing` tombstone still holds the record it was deleting). A
/// missing store yields no ids.
pub(crate) fn card_dir_ids(cards_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for (dir, _modified) in child_dirs(cards_dir)? {
        if !dir.join("card.yaml").is_file() {
            continue;
        }
        if let Some(name) = dir.file_name().and_then(|name| name.to_str())
            && !name.starts_with('.')
        {
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

/// Whether the path itself is a symlink. The store refuses symlinked card
/// dirs, `tasks/` pools, and container files wholesale; see
/// [`load_with_snapshot`].
pub(crate) fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// Load a card together with the raw bytes backing the next CAS write.
pub fn load_with_snapshot(path: &Path) -> Result<CardSnapshot> {
    // Refuse to follow a symlinked card directory. A `.maestro/cards/<id>` that is
    // a symlink could redirect this load to a card.yaml outside the store; this is
    // the single-load mirror of `cards::scan`'s symlink skip, placed on the shared
    // store seam so feature/decision/harness single-loads are covered too. Bail
    // rather than report absence: an "absent" snapshot would let a create path
    // save through the link (CAS sees no prior bytes), landing card.yaml and its
    // lock files outside the store. Probes that want quiet absence use
    // `dir_card_exists`, which never reaches this load.
    if path.parent().is_some_and(is_symlink) {
        bail!(
            "card dir {} is a symlink; the card store refuses symlinked dirs",
            path.parent().unwrap_or(path).display()
        );
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

/// Generated-id prefixes, reserved against feature ids (SPEC-card-slug-ids
/// D1b): a feature named like a generated card would re-merge the namespaces
/// the prefix exists to separate. Legacy `card-<hex>` ids are not minted
/// anymore but stay valid forever (D3).
pub(crate) const GENERATED_ID_PREFIXES: &[&str] = &[
    "task-",
    "bug-",
    "chore-",
    "custom-",
    "progress-",
    "mem-",
    "dec-",
    "idea-",
];

/// The id prefix a generated card carries for its type (D1b): the type word,
/// with `decision` shortened to `dec`. Features mint no generated id (their id
/// is the user's creation slug), so they have none.
fn type_prefix(card_type: CardType) -> Option<&'static str> {
    match card_type {
        CardType::Feature => None,
        CardType::Custom => Some("custom"),
        CardType::Progress => Some("progress"),
        CardType::Memory => Some("mem"),
        CardType::Task => Some("task"),
        CardType::Bug => Some("bug"),
        CardType::Chore => Some("chore"),
        CardType::Decision => Some("dec"),
        CardType::Idea => Some("idea"),
    }
}

/// Cap the title slug so generated ids stay readable in `ls`: at most 40
/// chars, cut at a word boundary. `slugify_ascii` output is pure ASCII, so
/// byte slicing is char-safe.
fn capped_slug(title: &str) -> String {
    let slug = slugify_ascii(title);
    if slug.len() <= 40 {
        return slug;
    }
    match slug[..40].rfind('-') {
        Some(cut) if cut > 0 => slug[..cut].to_string(),
        _ => slug[..40].to_string(),
    }
}

/// One `<prefix>-<slug>-<hex4>` candidate; a slug-less title (no ASCII
/// alphanumerics) collapses to `<prefix>-<hex4>`.
fn typed_id(prefix: &str, slug: &str, input: &str) -> String {
    let tail = &sha256_hex(input.as_bytes())[..4];
    if slug.is_empty() {
        format!("{prefix}-{tail}")
    } else {
        format!("{prefix}-{slug}-{tail}")
    }
}

/// Mint a fresh typed slug id `<type>-<slug>-<hex4>` for a newly created
/// non-feature card (SPEC-card-slug-ids D1/D1b). The slug is frozen at
/// creation -- a later retitle never renames the card -- and the 4-hex tail
/// hashes the title plus a process-unique nonce (millisecond timestamp, pid,
/// and a process-local counter), so two cards created back-to-back differ
/// even with identical titles. The disk-existence predicate leaves the
/// create-time CAS (D1) as the real collision guard; the nonce only keeps the
/// first attempt from colliding.
pub(crate) fn mint_card_id(paths: &MaestroPaths, card_type: CardType, title: &str) -> String {
    let ts = utc_now_millis_timestamp();
    let seq = CREATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let input = format!("{title}|{ts}|{}|{seq}", process::id());
    // An unreadable store reads as free rather than salt-bumping forever; the
    // create-time CAS stays the real collision guard either way.
    let is_taken = |id: &str| {
        locate(paths, id)
            .map(|home| home.is_some())
            .unwrap_or(false)
    };
    let Some(prefix) = type_prefix(card_type) else {
        // Features never mint (their id is the creation slug); fall back to
        // the legacy hash mint rather than panicking on a misuse.
        return mint_hash_id(&input, is_taken);
    };
    let slug = capped_slug(title);
    let base = typed_id(prefix, &slug, &input);
    if !is_taken(&base) {
        return base;
    }
    let mut salt = 1u32;
    loop {
        let candidate = typed_id(prefix, &slug, &format!("{input}-{salt}"));
        if !is_taken(&candidate) {
            return candidate;
        }
        salt += 1;
    }
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
        merge_folded_deps(&mut card, existing, &BTreeSet::new());
        carry_card_only_fields(&mut card, existing);
    }
    save_with_snapshot(path, &card, snapshot)
}

/// Union the existing card's dep edges into a folded card. The fold derives
/// its own blocking edges (a task's unresolved blockers); every other edge
/// (`dep add`) lives only on the existing card. A `released` target names a
/// `blocks` edge a just-resolved blocker no longer owns -- it is dropped
/// unless the fold re-derived it from a still-open blocker.
fn merge_folded_deps(card: &mut Card, existing: &Card, released: &BTreeSet<String>) {
    for dep in &existing.deps {
        if dep.kind.is_blocking() && released.contains(&dep.target) {
            continue;
        }
        if !card.deps.contains(dep) {
            card.deps.push(dep.clone());
        }
    }
}

/// Carry the card-only fields a typed-record fold cannot derive (`deps`
/// edges, `lane`, a card-set `description`, the create-time `project`) from
/// the existing card, so the rebuilt copy does not wipe them.
fn carry_card_only_fields(card: &mut Card, existing: &Card) {
    if card.lane.is_none() {
        card.lane = existing.lane.clone();
    }
    if card.description.is_none() {
        card.description = existing.description.clone();
    }
    if card.project.is_none() {
        card.project = existing.project.clone();
    }
    carry_unknown_payload(card, existing);
}

/// D6.6 passthrough at the typed-fold seam: a fold rebuilds the card from its
/// record, so a top-level key or extra key this binary does not declare would
/// be destroyed on save. The top-level `unknown` bag is carried whole (a fold
/// never produces one); an extra key is carried only when it is OUTSIDE the
/// family's pack field list -- a pack-known field absent from the fold output
/// is an intentional clear (a released claim, an emptied list) and must not
/// resurrect.
fn carry_unknown_payload(card: &mut Card, existing: &Card) {
    if card.unknown.is_empty() {
        card.unknown = existing.unknown.clone();
    }
    if existing.extra.is_empty() {
        return;
    }
    let Some(known) = fold::payload_pack_fields(card.card_type) else {
        return;
    };
    for (key, value) in &existing.extra {
        let foreign = key.as_str().is_none_or(|name| !known.contains(name));
        if foreign && !card.extra.contains_key(key) {
            card.extra.insert(key.clone(), value.clone());
        }
    }
}

/// Write a card, but only when its file still matches the snapshot it was read
/// from (SPEC D1, the single save-CAS seam). Creates the card directory only
/// on first save: for an existing card a missing dir means a concurrent
/// removal, and the CAS seam rejects the stale write without resurrecting it.
pub fn save_with_snapshot(path: &Path, card: &Card, snapshot: &CardSnapshot) -> Result<()> {
    if snapshot.raw.is_none()
        && let Some(parent) = path.parent()
    {
        ensure_dir(parent)?;
    }
    let contents = serde_yaml::to_string(card).context("failed to serialize card")?;
    write_string_if_unchanged(path, snapshot.raw.as_deref(), &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

// ---------------------------------------------------------------------------
// Container layout (SPEC-card-sprawl): the id -> (container, entry) resolver
// that replaced `cards/<id>/card.yaml` as the universal addressing layer
// (advisor F1). Features own a container dir; workable cards keep per-task
// dirs grouped under a `tasks/` pool (S5'); decisions and ideas are entries
// in per-container list files (S2-S4). Pre-migration flat leaf dirs keep
// reading as dir-backed homes until `maestro migrate` folds them.
// ---------------------------------------------------------------------------

/// In-dir record file of a feature container (and of a pre-migration flat
/// leaf card dir).
pub(crate) const CARD_FILE: &str = "card.yaml";
/// In-dir record file of a workable card under a `tasks/` pool (S5').
pub(crate) const TASK_FILE: &str = "task.yaml";
/// Per-container pool dir for workable cards (S5').
pub(crate) const TASKS_DIR: &str = "tasks";
/// Per-container decision entries file (S2/S3).
pub(crate) const DECISIONS_FILE: &str = "decisions.yaml";
/// Store-root idea entries file (S4).
pub(crate) const IDEAS_FILE: &str = "ideas.yaml";

/// Names the container layout claims inside every container dir: a feature id
/// equal to one of these would shadow the work pool, an entry file, or the
/// container's own record/prose files.
pub(crate) const RESERVED_CONTAINER_NAMES: &[&str] = &[
    TASKS_DIR,
    DECISIONS_FILE,
    IDEAS_FILE,
    CARD_FILE,
    "notes.md",
    "spec.md",
];

/// Where a card lives in the container layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CardHome {
    /// Dir-backed: the yaml record inside the card's own directory. Feature
    /// containers (`cards/<slug>/card.yaml`), pooled task dirs
    /// (`[<feature>/]tasks/<id>/task.yaml`), and pre-migration flat leaf dirs
    /// all load and save through this arm.
    Dir(PathBuf),
    /// Entry-backed: one entry in a container list file
    /// (`[<feature>/]decisions.yaml` or `ideas.yaml`), written back under
    /// whole-file CAS (the S2/S4 single-file contention rider).
    Entry(PathBuf),
    /// DB-backed: the record row in `.maestro/store.sqlite`.
    Db(PathBuf),
}

impl CardHome {
    /// The file backing this home: the card's own yaml record, or the
    /// container list file holding its entry.
    pub fn path(&self) -> &Path {
        match self {
            Self::Dir(yaml) => yaml,
            Self::Entry(file) => file,
            Self::Db(path) => path,
        }
    }
}

/// The home a NEW card of this type/parent is created at. Features mint their
/// own container dir; workable cards land in the `tasks/` pool of their
/// parent feature's container, decisions in its `decisions.yaml`; ideas are
/// entries in the root `ideas.yaml`.
pub fn home_for_new(paths: &MaestroPaths, card: &Card) -> Result<CardHome> {
    validate_card_id(&card.id)?;
    Ok(match card.card_type {
        CardType::Feature | CardType::Custom | CardType::Progress | CardType::Memory => {
            if RESERVED_CONTAINER_NAMES.contains(&card.id.as_str()) {
                bail!(
                    "{} id {} is reserved by the card store layout",
                    card.card_type.as_str(),
                    card.id
                );
            }
            // D1b: the generated-id prefixes are reserved, so a feature can
            // never look like a minted task/bug/chore/dec/idea id.
            if card.card_type == CardType::Feature
                && let Some(prefix) = GENERATED_ID_PREFIXES
                    .iter()
                    .find(|prefix| card.id.starts_with(*prefix))
            {
                bail!(
                    "feature id {} starts with the reserved generated-id prefix {prefix}; pick another name",
                    card.id
                );
            }
            CardHome::Dir(paths.cards_dir().join(&card.id).join(CARD_FILE))
        }
        CardType::Task | CardType::Bug | CardType::Chore
            if live_db::parent_is_db_container(paths, card.parent.as_deref())? =>
        {
            CardHome::Db(live_db::synthetic_card_path(paths, &card.id, TASK_FILE))
        }
        CardType::Task | CardType::Bug | CardType::Chore => {
            let pool = container_dir(paths, card.parent.as_deref())?.join(TASKS_DIR);
            // A symlinked pool would land the CAS write outside the store;
            // creation fails loud where reads silently refuse.
            if is_symlink(&pool) {
                bail!(
                    "task pool {} is a symlink; the card store refuses symlinked dirs",
                    pool.display()
                );
            }
            CardHome::Dir(pool.join(&card.id).join(TASK_FILE))
        }
        CardType::Decision => {
            if live_db::parent_is_db_container(paths, card.parent.as_deref())? {
                return Ok(CardHome::Db(live_db::synthetic_card_path(
                    paths, &card.id, CARD_FILE,
                )));
            }
            CardHome::Entry(container_dir(paths, card.parent.as_deref())?.join(DECISIONS_FILE))
        }
        CardType::Idea => CardHome::Entry(paths.cards_dir().join(IDEAS_FILE)),
    })
}

/// The container a parented card belongs to: the parent's feature dir when
/// `parent` names an existing feature container, else the store root. The
/// parent's TYPE is read (not just dir existence) so a pre-migration flat
/// leaf dir -- e.g. a decision card's -- never becomes a nesting point; a
/// task parent, an archived feature, or a dangling id all fall back to the
/// root container.
fn container_dir(paths: &MaestroPaths, parent: Option<&str>) -> Result<PathBuf> {
    let root = paths.cards_dir();
    let Some(parent) = parent else {
        return Ok(root);
    };
    validate_card_id(parent)?;
    let yaml = root.join(parent).join(CARD_FILE);
    match load(&yaml)? {
        Some(card) if card.card_type.owns_task_container() => Ok(root.join(parent)),
        _ => Ok(root),
    }
}

/// Find the home of an existing card id anywhere in the layout. Probe order
/// is deterministic: dir-backed homes first (the root dir -- features and
/// pre-migration flat leaf cards -- then the root `tasks/` pool, then each
/// container's `tasks/`), then the entry files (root `decisions.yaml`, root
/// `ideas.yaml`, then each container's `decisions.yaml`). `None` when no
/// home holds the id.
pub fn locate(paths: &MaestroPaths, id: &str) -> Result<Option<CardHome>> {
    if live_db::contains_card_id(paths, id)? {
        let record_file = match live_db::resolve(paths, id)? {
            Some(db_card) => db_card
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(CARD_FILE)
                .to_string(),
            None => CARD_FILE.to_string(),
        };
        return Ok(Some(CardHome::Db(live_db::synthetic_card_path(
            paths,
            id,
            &record_file,
        ))));
    }
    locate_in(&paths.cards_dir(), id)
}

/// [`locate`] over an explicit store root, so the archive tree
/// (`archive/cards/`) probes through the same layout walk as the live store.
pub(crate) fn locate_in(root: &Path, id: &str) -> Result<Option<CardHome>> {
    Ok(locate_basis_in(root, id)?.map(|basis| match basis {
        LocatedBasis::Dir(yaml) => CardHome::Dir(yaml),
        LocatedBasis::Entry(file, _) => CardHome::Entry(file),
    }))
}

/// A located home carrying the entry snapshot the probe already parsed, so
/// [`resolve_in`] does not re-read the container file the walk just loaded.
enum LocatedBasis {
    Dir(PathBuf),
    Entry(PathBuf, EntriesSnapshot),
}

fn locate_basis_in(root: &Path, id: &str) -> Result<Option<LocatedBasis>> {
    validate_card_id(id)?;

    let flat = root.join(id).join(CARD_FILE);
    if dir_card_exists(&flat) {
        return Ok(Some(LocatedBasis::Dir(flat)));
    }
    // The pool level needs its own symlink check: `dir_card_exists` covers
    // only the task dir itself, and a symlinked `tasks/` would route every
    // pooled probe (and the CAS write a hit backs) outside the store.
    let pool = root.join(TASKS_DIR);
    if !is_symlink(&pool) {
        let pooled = pool.join(id).join(TASK_FILE);
        if dir_card_exists(&pooled) {
            return Ok(Some(LocatedBasis::Dir(pooled)));
        }
    }
    let containers: Vec<PathBuf> = sorted_child_dirs(root)?
        .into_iter()
        .filter(|dir| dir.file_name().is_none_or(|name| name != TASKS_DIR))
        .collect();
    for dir in &containers {
        let pool = dir.join(TASKS_DIR);
        if is_symlink(&pool) {
            continue;
        }
        let nested = pool.join(id).join(TASK_FILE);
        if dir_card_exists(&nested) {
            return Ok(Some(LocatedBasis::Dir(nested)));
        }
    }

    let mut entry_files = vec![root.join(DECISIONS_FILE), root.join(IDEAS_FILE)];
    entry_files.extend(containers.iter().map(|dir| dir.join(DECISIONS_FILE)));
    for file in entry_files {
        let entries = load_entries(&file)?;
        if entries.cards.iter().any(|card| card.id == id) {
            return Ok(Some(LocatedBasis::Entry(file, entries)));
        }
    }
    Ok(None)
}

/// Whether a dir-backed record file exists, honoring the symlinked-dir
/// rejection [`load_with_snapshot`] applies -- a probe must never say
/// "present" for a record the load refuses to read.
fn dir_card_exists(yaml: &Path) -> bool {
    !yaml.parent().is_some_and(is_symlink) && yaml.is_file()
}

/// All entries of a container list file plus the exact bytes backing the next
/// whole-file CAS write. `raw` is `None` when the file is absent.
#[derive(Clone, Debug, PartialEq)]
pub struct EntriesSnapshot {
    pub cards: Vec<Card>,
    raw: Option<String>,
}

impl EntriesSnapshot {
    /// Whether the container file existed when this snapshot was read, so an
    /// aggregate save can skip creating an empty file for an empty store.
    pub fn exists(&self) -> bool {
        self.raw.is_some()
    }
}

/// Load every entry of a container list file (`decisions.yaml`/`ideas.yaml`).
/// An absent or empty file is an empty list; a symlinked file or container
/// dir is refused like a symlinked card dir. One malformed or
/// schema-mismatched entry fails the whole file -- the container file is the
/// failure grain.
pub fn load_entries(file: &Path) -> Result<EntriesSnapshot> {
    if is_symlink(file) || file.parent().is_some_and(is_symlink) {
        return Ok(EntriesSnapshot {
            cards: Vec::new(),
            raw: None,
        });
    }
    let Some(contents) = read_to_string_if_exists(file)? else {
        return Ok(EntriesSnapshot {
            cards: Vec::new(),
            raw: None,
        });
    };
    let cards: Vec<Card> = if contents.trim().is_empty() {
        Vec::new()
    } else {
        serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", file.display()))?
    };
    for card in &cards {
        if classify(&card.schema_version, CARD_SCHEMA_VERSION) != Compat::Exact {
            return Err(MaestroError::SchemaMismatch {
                artifact: format!("{}#{}", file.display(), card.id),
                expected: CARD_SCHEMA_VERSION,
                found: card.schema_version.clone(),
            }
            .into());
        }
    }
    Ok(EntriesSnapshot {
        cards,
        raw: Some(contents),
    })
}

/// Write the full entry list of a container file, but only when the file
/// still matches the snapshot it was read from -- the whole file is the CAS
/// unit (the S2/S4 contention rider).
pub fn save_entries(file: &Path, cards: &[Card], snapshot: &EntriesSnapshot) -> Result<()> {
    if snapshot.raw.is_none()
        && let Some(parent) = file.parent()
    {
        ensure_dir(parent)?;
    }
    let contents = serde_yaml::to_string(cards).context("failed to serialize card entries")?;
    write_string_if_unchanged(file, snapshot.raw.as_deref(), &contents)
        .with_context(|| format!("failed to write {}", file.display()))
}

/// A card found by [`resolve`] plus the exact backing-store state for writing
/// it back: a one-card CAS basis for a dir-backed home, the whole entry list
/// for an entry-backed one.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCard {
    pub card: Card,
    basis: ResolvedBasis,
}

#[derive(Clone, Debug, PartialEq)]
enum ResolvedBasis {
    Dir {
        yaml: PathBuf,
        // Boxed for variant-size parity with `Entry` (clippy).
        snapshot: Box<CardSnapshot>,
    },
    Entry {
        file: PathBuf,
        snapshot: EntriesSnapshot,
    },
    Db {
        paths: MaestroPaths,
        path: PathBuf,
        raw: String,
    },
}

impl ResolvedCard {
    /// The file backing this card: its own yaml for a dir-backed home, the
    /// container list file for an entry-backed one.
    pub fn path(&self) -> &Path {
        match &self.basis {
            ResolvedBasis::Dir { yaml, .. } => yaml,
            ResolvedBasis::Entry { file, .. } => file,
            ResolvedBasis::Db { path, .. } => path,
        }
    }

    /// Whether this card resolved to a record file of its own rather than an
    /// entry in a container list file.
    pub fn is_dir_backed(&self) -> bool {
        matches!(self.basis, ResolvedBasis::Dir { .. })
    }
}

/// Whether a record path is a dir-backed card file (`card.yaml`/`task.yaml`)
/// rather than an entry in a container list file. The seam owns this cut;
/// callers holding a bare path must ask it instead of matching filenames.
pub(crate) fn is_dir_backed(record: &Path) -> bool {
    matches!(
        record.file_name().and_then(|name| name.to_str()),
        Some(CARD_FILE | TASK_FILE)
    )
}

/// Load a card by id from wherever it lives (locate + read in one step).
/// `None` when no home holds the id. The returned basis backs the matching
/// [`save_resolved`]/[`remove_resolved`] CAS.
pub fn resolve(paths: &MaestroPaths, id: &str) -> Result<Option<ResolvedCard>> {
    if let Some(db_card) = live_db::resolve(paths, id)? {
        return Ok(Some(ResolvedCard {
            card: db_card.card,
            basis: ResolvedBasis::Db {
                paths: paths.clone(),
                path: db_card.path,
                raw: db_card.raw,
            },
        }));
    }
    resolve_in(&paths.cards_dir(), id)
}

/// [`resolve`] over an explicit store root (the archive tree), read-only in
/// spirit: the basis still backs a CAS save, but archive callers only take
/// the card and its path.
pub(crate) fn resolve_in(root: &Path, id: &str) -> Result<Option<ResolvedCard>> {
    let Some(basis) = locate_basis_in(root, id)? else {
        return Ok(None);
    };
    match basis {
        LocatedBasis::Dir(yaml) => {
            let snapshot = load_with_snapshot(&yaml)?;
            let Some(card) = snapshot.card.clone() else {
                return Ok(None);
            };
            Ok(Some(ResolvedCard {
                card,
                basis: ResolvedBasis::Dir {
                    yaml,
                    snapshot: Box::new(snapshot),
                },
            }))
        }
        LocatedBasis::Entry(file, snapshot) => {
            let Some(card) = snapshot.cards.iter().find(|card| card.id == id).cloned() else {
                return Ok(None);
            };
            Ok(Some(ResolvedCard {
                card,
                basis: ResolvedBasis::Entry { file, snapshot },
            }))
        }
    }
}

/// Write a mutated card back to the home it was resolved from, under that
/// home's CAS basis. This writes in place and never moves a card between
/// homes -- re-homing on a parent change is a migration-grade move, not a
/// field save (beads E2 amendment).
pub fn save_resolved(card: &Card, basis: &ResolvedCard) -> Result<()> {
    if card.id != basis.card.id {
        bail!(
            "resolved save id mismatch: {} resolved, {} saved",
            basis.card.id,
            card.id
        );
    }
    match &basis.basis {
        ResolvedBasis::Dir { yaml, snapshot } => save_with_snapshot(yaml, card, snapshot),
        ResolvedBasis::Entry { file, snapshot } => {
            let mut cards = snapshot.cards.clone();
            let slot = cards
                .iter_mut()
                .find(|entry| entry.id == card.id)
                .expect("invariant: resolved entry present in its own snapshot");
            *slot = card.clone();
            save_entries(file, &cards, snapshot)
        }
        ResolvedBasis::Db { paths, raw, .. } => live_db::save_card_if_unchanged(paths, card, raw),
    }
}

/// Create a brand-new card at the home its type/parent dictates, returning
/// that home. Fails when the id already exists anywhere; the write itself is
/// a CAS create (absent record / unchanged entry file), so a racing create of
/// the same id loses cleanly (SPEC D1).
pub fn create_card(paths: &MaestroPaths, card: &Card) -> Result<CardHome> {
    if locate(paths, &card.id)?.is_some() {
        bail!("card {} already exists", card.id);
    }
    // Archived ids stay reserved: archive fallbacks (`show`, `update`'s
    // not-found hint) resolve by id, so a live re-mint would shadow the
    // archived card and make those reads ambiguous.
    if locate_in(&paths.archive_cards_dir(), &card.id)?.is_some()
        || archive_db::contains_card_id(paths, &card.id)?
    {
        bail!(
            "card {} already exists in the archive; pick a new id",
            card.id
        );
    }
    let home = home_for_new(paths, card)?;
    match &home {
        CardHome::Dir(yaml) => {
            let snapshot = load_with_snapshot(yaml)?;
            if snapshot.card.is_some() {
                bail!("card {} already exists", card.id);
            }
            save_with_snapshot(yaml, card, &snapshot)?;
        }
        CardHome::Entry(file) => {
            let snapshot = load_entries(file)?;
            if snapshot.cards.iter().any(|entry| entry.id == card.id) {
                bail!("card {} already exists", card.id);
            }
            let mut cards = snapshot.cards.clone();
            cards.push(card.clone());
            save_entries(file, &cards, &snapshot)?;
        }
        CardHome::Db(path) => {
            let record_file = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(TASK_FILE);
            live_db::insert_card(paths, card, record_file)?;
        }
    }
    Ok(home)
}

/// [`save_resolved`] for a card rebuilt by a typed-record fold: carry the
/// card-only fields (`deps`, `lane`, a card-set `description`) from the
/// resolved card before the write would wipe them -- the resolved twin of
/// [`save_folded_with_snapshot`].
pub(crate) fn save_folded_resolved(card: Card, basis: &ResolvedCard) -> Result<()> {
    save_folded_resolved_releasing(card, basis, &BTreeSet::new())
}

/// [`save_folded_resolved`], dropping the existing `blocks` edges whose
/// targets the record's just-resolved blockers released. Only the task fold
/// derives blocking edges from its record, so only the task save names
/// releases; everyone else goes through the plain wrapper.
pub(crate) fn save_folded_resolved_releasing(
    mut card: Card,
    basis: &ResolvedCard,
    released: &BTreeSet<String>,
) -> Result<()> {
    merge_folded_deps(&mut card, &basis.card, released);
    carry_card_only_fields(&mut card, &basis.card);
    save_resolved(&card, basis)
}

/// Remove a resolved card from its home: a dir-backed card's whole directory
/// (record and sidecars -- notes, evidence, proof -- travel with the dir), or
/// its entry rewritten out of the container file under whole-file CAS.
pub fn remove_resolved(basis: &ResolvedCard) -> Result<()> {
    match &basis.basis {
        ResolvedBasis::Dir { yaml, snapshot } => remove_dir_with_snapshot(yaml, snapshot),
        ResolvedBasis::Entry { file, snapshot } => {
            let cards: Vec<Card> = snapshot
                .cards
                .iter()
                .filter(|entry| entry.id != basis.card.id)
                .cloned()
                .collect();
            save_entries(file, &cards, snapshot)
        }
        ResolvedBasis::Db { paths, raw, .. } => {
            live_db::remove_card_if_unchanged(paths, &basis.card, raw)
        }
    }
}

/// Remove a dir-backed card at the exact record path a caller scanned, only
/// when the record still matches that scan-time snapshot.
pub(crate) fn remove_dir_with_snapshot(path: &Path, snapshot: &CardSnapshot) -> Result<()> {
    let dir = path
        .parent()
        .with_context(|| format!("card path missing parent: {}", path.display()))?;
    remove_dir_if_file_unchanged(path, snapshot.raw.as_deref(), dir)
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
            suggested_for: Some("codex#session-2".to_string()),
            created_at: "2026-06-08T00:00:00Z".to_string(),
            updated_at: "2026-06-08T01:00:00Z".to_string(),
            description: Some("Stream rows to stdout.".to_string()),
            active_form: Some("Streaming CSV rows".to_string()),
            project: Some("svc-pay".to_string()),
            extra: serde_yaml::from_str(
                "legacy_field: kept\nstate_history:\n  - draft\n  - ready\n",
            )
            .expect("invariant: fixture extra parses"),
            unknown: serde_yaml::from_str("future_field: preserved\n")
                .expect("invariant: fixture unknown bag parses"),
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

        let first = mint_card_id(&paths, CardType::Task, "Add CSV export");
        let second = mint_card_id(&paths, CardType::Task, "Add CSV export");
        assert_ne!(first, second, "same title must still mint distinct ids");
        assert!(
            first.starts_with("task-add-csv-export-") && second.starts_with("task-add-csv-export-"),
            "{first} / {second}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    #[test]
    fn mint_card_id_carries_type_prefix_slug_and_hex_tail() {
        let paths = temp_cards_repo("mint-typed");

        let dec = mint_card_id(&paths, CardType::Decision, "Fly, not train!");
        let tail = dec
            .strip_prefix("dec-fly-not-train-")
            .unwrap_or_else(|| panic!("dec id carries prefix + frozen slug: {dec}"));
        assert_eq!(tail.len(), 4, "4-hex tail: {dec}");
        assert!(tail.chars().all(|c| c.is_ascii_hexdigit()), "{dec}");

        assert!(
            mint_card_id(&paths, CardType::Bug, "Fix ordering race")
                .starts_with("bug-fix-ordering-race-")
        );
        assert!(
            mint_card_id(&paths, CardType::Progress, "Docs cleanup")
                .starts_with("progress-docs-cleanup-")
        );
        assert!(mint_card_id(&paths, CardType::Idea, "grep").starts_with("idea-grep-"));

        // The slug caps at a word boundary so ls stays readable.
        let long = mint_card_id(
            &paths,
            CardType::Task,
            "migrate-v2 should leave the repository doctor-clean afterwards every time",
        );
        let slug = long
            .strip_prefix("task-")
            .and_then(|rest| rest.rsplit_once('-'))
            .map(|(slug, _tail)| slug)
            .unwrap_or_default();
        assert!(slug.len() <= 40, "capped slug: {long}");
        assert!(!slug.ends_with('-'), "{long}");

        // A title with no ASCII alphanumerics collapses to `<prefix>-<hex4>`.
        let bare = mint_card_id(&paths, CardType::Chore, "###");
        let tail = bare
            .strip_prefix("chore-")
            .unwrap_or_else(|| panic!("{bare}"));
        assert_eq!(tail.len(), 4, "slug-less id is prefix + tail: {bare}");

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    #[test]
    fn card_progress_type_is_dir_backed_and_not_a_work_pool_entry() {
        let paths = temp_cards_repo("card-progress-home");
        let root = paths.cards_dir();
        let progress = typed_card("progress-doc-cleanup-019f", CardType::Progress, None);

        assert_eq!(
            home_for_new(&paths, &progress).expect("progress home resolves"),
            CardHome::Dir(root.join("progress-doc-cleanup-019f").join("card.yaml"))
        );
        assert!(
            !CardType::Progress.workable(),
            "progress is a workflow card, not a legacy claimable work card"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    #[test]
    fn home_for_new_rejects_feature_ids_with_generated_prefixes() {
        let paths = temp_cards_repo("feature-prefix-guard");
        for id in ["task-cleanup", "bug-tracker", "dec-log", "idea-board"] {
            let err = home_for_new(&paths, &typed_card(id, CardType::Feature, None))
                .expect_err("generated-id prefixes are reserved against feature ids");
            assert!(
                err.to_string().contains("reserved generated-id prefix"),
                "{err}"
            );
        }
        // Words that merely share letters with a prefix stay legal.
        assert!(home_for_new(&paths, &typed_card("decision-log", CardType::Feature, None)).is_ok());
        assert!(home_for_new(&paths, &typed_card("tasks-board", CardType::Feature, None)).is_ok());
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

    fn typed_card(id: &str, card_type: CardType, parent: Option<&str>) -> Card {
        let mut card = Card::new(id, card_type, id, "open", "2026-06-10T00:00:00Z");
        card.parent = parent.map(str::to_string);
        card
    }

    #[test]
    fn db_backed_card_resolves_saves_and_scans_without_live_dir() {
        let paths = temp_cards_repo("db-backed-card");
        let mut feature = typed_card("db-feature", CardType::Feature, None);
        feature.status = "proposed".to_string();
        create_card(&paths, &feature).expect("create file-backed feature");
        let feature_dir = paths.cards_dir().join("db-feature");
        std::fs::write(feature_dir.join("notes.md"), "design note\n").expect("write sidecar");

        live_db::import_card_dir(&paths, "db-feature", &feature_dir, true)
            .expect("import feature into DB store");

        assert!(
            !feature_dir.exists(),
            "finalized DB authority no longer needs the live card folder"
        );
        let resolved = resolve(&paths, "db-feature")
            .expect("resolve DB-backed card")
            .expect("DB-backed card exists");
        assert_eq!(resolved.card.id, "db-feature");
        assert!(
            resolved.path().starts_with(paths.store_db_file()),
            "synthetic DB path: {}",
            resolved.path().display()
        );
        assert_eq!(
            live_db::read_text_file(&paths, "db-feature", "notes.md")
                .expect("read DB sidecar")
                .as_deref(),
            Some("design note\n")
        );

        let mut updated = resolved.card.clone();
        updated.status = "ready".to_string();
        save_resolved(&updated, &resolved).expect("save DB-backed card");

        let reloaded = resolve(&paths, "db-feature")
            .expect("resolve updated DB-backed card")
            .expect("DB-backed card remains");
        assert_eq!(reloaded.card.status, "ready");
        let scanned = crate::domain::card::query::scan(&paths).expect("scan DB-backed cards");
        assert!(
            scanned
                .iter()
                .any(|card| card.id == "db-feature" && card.status == "ready"),
            "DB-backed card appears in shared scans"
        );

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }

    #[test]
    fn child_card_of_db_backed_container_is_created_in_db_store() {
        let paths = temp_cards_repo("db-backed-child");
        let mut feature = typed_card("db-feature", CardType::Feature, None);
        feature.status = "ready".to_string();
        create_card(&paths, &feature).expect("create file-backed feature");
        let feature_dir = paths.cards_dir().join("db-feature");
        live_db::import_card_dir(&paths, "db-feature", &feature_dir, true)
            .expect("import feature into DB store");

        let task = typed_card("task-child-0001", CardType::Task, Some("db-feature"));
        let home = create_card(&paths, &task).expect("create child task under DB parent");
        assert!(
            matches!(home, CardHome::Db(_)),
            "child of DB-backed feature should be DB-backed"
        );
        assert!(
            !paths
                .cards_dir()
                .join("db-feature")
                .join("tasks")
                .join("task-child-0001")
                .exists(),
            "creating a child must not recreate the finalized feature folder"
        );
        let resolved = resolve(&paths, "task-child-0001")
            .expect("resolve DB child")
            .expect("DB child exists");
        assert_eq!(resolved.card.parent.as_deref(), Some("db-feature"));
        assert!(resolved.path().starts_with(paths.store_db_file()));

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }

    /// Seed a card the pre-migration way: a flat `cards/<id>/card.yaml` dir.
    fn seed_flat(paths: &MaestroPaths, card: &Card) {
        let path = card_path(paths, &card.id);
        let snapshot = load_with_snapshot(&path).expect("absent loads None");
        save_with_snapshot(&path, card, &snapshot).expect("seed flat card");
    }

    /// SPEC-card-sprawl final layout: each type's creation home.
    #[test]
    fn home_for_new_places_each_type_per_the_container_layout() {
        let paths = temp_cards_repo("home-for-new");
        let root = paths.cards_dir();
        create_card(&paths, &typed_card("csv-export", CardType::Feature, None))
            .expect("create the feature container");
        // a pre-migration flat NON-feature dir: never a nesting point
        seed_flat(&paths, &typed_card("card-dec001", CardType::Decision, None));

        let home = |card: &Card| home_for_new(&paths, card).expect("home resolves");
        assert_eq!(
            home(&typed_card("new-feat", CardType::Feature, None)),
            CardHome::Dir(root.join("new-feat").join("card.yaml"))
        );
        assert_eq!(
            home(&typed_card(
                "mem-refund-policy-1234",
                CardType::Memory,
                None
            )),
            CardHome::Dir(root.join("mem-refund-policy-1234").join("card.yaml")),
            "memory cards own top-level sidecar dirs"
        );
        assert_eq!(
            home(&typed_card(
                "card-t00001",
                CardType::Task,
                Some("csv-export")
            )),
            CardHome::Dir(
                root.join("csv-export")
                    .join("tasks")
                    .join("card-t00001")
                    .join("task.yaml")
            ),
            "feature work pools under the container"
        );
        assert_eq!(
            home(&typed_card("card-t00002", CardType::Bug, None)),
            CardHome::Dir(root.join("tasks").join("card-t00002").join("task.yaml")),
            "standalone work pools at the root"
        );
        assert_eq!(
            home(&typed_card(
                "card-t00003",
                CardType::Chore,
                Some("card-dec001")
            )),
            CardHome::Dir(root.join("tasks").join("card-t00003").join("task.yaml")),
            "a non-feature parent falls back to the root pool"
        );
        assert_eq!(
            home(&typed_card(
                "card-d00001",
                CardType::Decision,
                Some("csv-export")
            )),
            CardHome::Entry(root.join("csv-export").join("decisions.yaml"))
        );
        assert_eq!(
            home(&typed_card("card-d00002", CardType::Decision, None)),
            CardHome::Entry(root.join("decisions.yaml")),
            "a global decision is an entry in the root container file"
        );
        assert_eq!(
            home(&typed_card("card-i00001", CardType::Idea, None)),
            CardHome::Entry(root.join("ideas.yaml"))
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn home_for_new_rejects_reserved_feature_ids() {
        let paths = temp_cards_repo("home-reserved");
        for reserved in ["tasks", "decisions.yaml", "ideas.yaml", "card.yaml"] {
            assert!(
                home_for_new(&paths, &typed_card(reserved, CardType::Feature, None)).is_err(),
                "{reserved:?} must be rejected as a feature id"
            );
        }
        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// S1/S2: a feature's decisions are entries in ONE container file; the
    /// resolver round-trips them (create -> resolve -> save) without minting
    /// any top-level dir beyond the feature slug.
    #[test]
    fn entry_cards_round_trip_inside_the_container_file() {
        let paths = temp_cards_repo("entry-round-trip");
        let root = paths.cards_dir();
        create_card(&paths, &typed_card("csv-export", CardType::Feature, None))
            .expect("create the feature");
        create_card(
            &paths,
            &typed_card("card-d00001", CardType::Decision, Some("csv-export")),
        )
        .expect("create the first decision");
        create_card(
            &paths,
            &typed_card("card-d00002", CardType::Decision, Some("csv-export")),
        )
        .expect("create the second decision");

        let file = root.join("csv-export").join("decisions.yaml");
        let entries = load_entries(&file).expect("load the container file");
        let ids: Vec<&str> = entries.cards.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, vec!["card-d00001", "card-d00002"]);
        let top_level: Vec<String> = sorted_child_dirs(&root)
            .expect("list store root")
            .iter()
            .filter_map(|dir| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert_eq!(
            top_level,
            vec!["csv-export".to_string()],
            "no top-level dir beyond the feature container"
        );

        let resolved = resolve(&paths, "card-d00001")
            .expect("resolve")
            .expect("the entry resolves");
        assert_eq!(resolved.path(), file.as_path());
        let mut card = resolved.card.clone();
        card.status = "locked".to_string();
        save_resolved(&card, &resolved).expect("save the mutated entry");

        let again = resolve(&paths, "card-d00001")
            .expect("re-resolve")
            .expect("still present");
        assert_eq!(again.card.status, "locked");
        let sibling = resolve(&paths, "card-d00002")
            .expect("resolve sibling")
            .expect("sibling present");
        assert_eq!(
            sibling.card.status, "open",
            "the sibling entry is untouched"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// SPEC D1 on the entry arm: the whole container file is the CAS unit
    /// (the S2/S4 rider), so a stale writer to ANY entry is rejected.
    #[test]
    fn entry_save_rejects_a_stale_writer_via_whole_file_cas() {
        let paths = temp_cards_repo("entry-stale-writer");
        create_card(&paths, &typed_card("card-d00001", CardType::Decision, None))
            .expect("create a global decision");

        let first = resolve(&paths, "card-d00001")
            .expect("first read")
            .expect("present");
        let second = resolve(&paths, "card-d00001")
            .expect("second read")
            .expect("present");

        let mut winner = second.card.clone();
        winner.title = "winner".to_string();
        save_resolved(&winner, &second).expect("first writer commits");

        let mut loser = first.card.clone();
        loser.title = "stale writer".to_string();
        let error = save_resolved(&loser, &first).expect_err("the stale writer must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    #[test]
    fn create_card_rejects_an_id_that_exists_anywhere() {
        let paths = temp_cards_repo("create-unique");
        create_card(&paths, &typed_card("csv-export", CardType::Feature, None))
            .expect("create the feature");
        create_card(&paths, &typed_card("card-d00001", CardType::Decision, None))
            .expect("create a global decision entry");
        seed_flat(&paths, &typed_card("card-old001", CardType::Task, None));

        for taken in ["csv-export", "card-d00001", "card-old001"] {
            let error = create_card(&paths, &typed_card(taken, CardType::Idea, None))
                .expect_err("a taken id must be rejected whatever home it lives in");
            assert!(format!("{error:#}").contains("already exists"), "{error:#}");
        }

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// Archived ids stay reserved: a live re-mint would shadow the archived
    /// card in every by-id archive fallback (`show`, the `update` hint).
    #[test]
    fn create_card_rejects_an_id_that_lives_in_the_archive() {
        let paths = temp_cards_repo("create-archived");
        let archived = paths
            .archive_cards_dir()
            .join("card-arch01")
            .join(CARD_FILE);
        let snapshot = load_with_snapshot(&archived).expect("absent loads None");
        save_with_snapshot(
            &archived,
            &typed_card("card-arch01", CardType::Feature, None),
            &snapshot,
        )
        .expect("plant the archived card");

        let error = create_card(&paths, &typed_card("card-arch01", CardType::Idea, None))
            .expect_err("an archived id must stay reserved");
        assert!(format!("{error:#}").contains("archive"), "{error:#}");

        let _ = std::fs::remove_dir_all(paths.cards_dir());
        let _ = std::fs::remove_dir_all(paths.archive_dir());
    }

    /// S5': workable cards keep per-task dirs, grouped under `tasks/` pools.
    #[test]
    fn task_cards_live_in_per_task_dirs_under_the_pool() {
        let paths = temp_cards_repo("task-pool");
        let root = paths.cards_dir();
        create_card(&paths, &typed_card("csv-export", CardType::Feature, None))
            .expect("create the feature");
        create_card(
            &paths,
            &typed_card("card-t00001", CardType::Task, Some("csv-export")),
        )
        .expect("create the feature task");
        create_card(&paths, &typed_card("card-t00002", CardType::Task, None))
            .expect("create the standalone task");

        assert!(
            root.join("csv-export")
                .join("tasks")
                .join("card-t00001")
                .join("task.yaml")
                .is_file()
        );
        assert!(
            root.join("tasks")
                .join("card-t00002")
                .join("task.yaml")
                .is_file()
        );

        let resolved = resolve(&paths, "card-t00001")
            .expect("resolve the pooled task")
            .expect("present");
        let mut card = resolved.card.clone();
        card.status = "in_progress".to_string();
        save_resolved(&card, &resolved).expect("save in place");
        assert_eq!(
            resolve(&paths, "card-t00001")
                .expect("re-resolve")
                .expect("present")
                .card
                .status,
            "in_progress"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn remove_resolved_drops_the_entry_and_keeps_siblings() {
        let paths = temp_cards_repo("remove-entry");
        create_card(&paths, &typed_card("card-i00001", CardType::Idea, None))
            .expect("create first idea");
        create_card(&paths, &typed_card("card-i00002", CardType::Idea, None))
            .expect("create second idea");

        let resolved = resolve(&paths, "card-i00001")
            .expect("resolve")
            .expect("present");
        remove_resolved(&resolved).expect("remove the entry");

        assert!(
            resolve(&paths, "card-i00001")
                .expect("re-resolve")
                .is_none(),
            "the removed entry is gone"
        );
        assert!(
            resolve(&paths, "card-i00002")
                .expect("resolve sibling")
                .is_some(),
            "the sibling entry survives"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    #[test]
    fn remove_resolved_removes_a_dir_backed_card_dir() {
        let paths = temp_cards_repo("remove-dir");
        create_card(&paths, &typed_card("card-t00001", CardType::Task, None))
            .expect("create a pooled task");
        let dir = paths.cards_dir().join("tasks").join("card-t00001");
        assert!(dir.is_dir());

        let resolved = resolve(&paths, "card-t00001")
            .expect("resolve")
            .expect("present");
        remove_resolved(&resolved).expect("remove the dir");
        assert!(!dir.exists(), "the whole task dir is removed");

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    #[test]
    fn remove_resolved_rejects_a_stale_dir_backed_card() {
        let paths = temp_cards_repo("remove-dir-stale");
        create_card(&paths, &typed_card("card-t00001", CardType::Task, None))
            .expect("create a pooled task");
        let resolved = resolve(&paths, "card-t00001")
            .expect("resolve")
            .expect("present");
        let path = resolved.path().to_path_buf();
        let fresh = load_with_snapshot(&path).expect("load fresh snapshot");
        let mut edited = fresh.card.clone().expect("card exists");
        edited.status = "in_progress".to_string();
        save_with_snapshot(&path, &edited, &fresh).expect("concurrent edit wins");

        let error = remove_resolved(&resolved).expect_err("stale remove must fail");
        let message = format!("{error:#}");
        assert!(
            message.contains("changed since it was read"),
            "stale removal gets the retryable CAS error:\n{message}"
        );
        assert!(
            resolve(&paths, "card-t00001")
                .expect("resolve after stale remove")
                .is_some(),
            "stale removal must not delete the edited card"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// The CAS seam must reject a stale writer after a removal WITHOUT
    /// recreating the deleted card dir as an empty husk (the lock reservation
    /// used to `ensure_dir` the vanished parent on every retry).
    #[test]
    fn stale_save_after_removal_leaves_no_husk() {
        let paths = temp_cards_repo("remove-then-stale-save");
        create_card(&paths, &typed_card("card-t00001", CardType::Task, None))
            .expect("create a pooled task");
        let resolved = resolve(&paths, "card-t00001")
            .expect("resolve")
            .expect("present");
        let path = resolved.path().to_path_buf();
        let stale = load_with_snapshot(&path).expect("snapshot before removal");
        remove_resolved(&resolved).expect("remove the card");
        let dir = path.parent().expect("card dir").to_path_buf();
        assert!(!dir.exists(), "removal deletes the dir");

        let card = stale.card.clone().expect("card existed before removal");
        let error = save_with_snapshot(&path, &card, &stale).expect_err("stale save must fail");
        let message = format!("{error:#}");
        assert!(
            message.contains("changed since it was read"),
            "stale save gets the retryable CAS error:\n{message}"
        );
        assert!(
            !dir.exists(),
            "a stale retry must not resurrect the card dir as an empty husk"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// A crash between the tombstone rename and the delete leaves the
    /// tombstone behind; the next removal of the same id clears it instead of
    /// failing the rename.
    #[test]
    fn remove_resolved_clears_a_stale_tombstone_from_a_crashed_removal() {
        let paths = temp_cards_repo("tombstone-clear");
        create_card(&paths, &typed_card("card-t00001", CardType::Task, None))
            .expect("create a pooled task");
        let pool = paths.cards_dir().join(TASKS_DIR);
        std::fs::rename(pool.join("card-t00001"), pool.join(".card-t00001.removing"))
            .expect("simulate a crash mid-removal");
        create_card(&paths, &typed_card("card-t00001", CardType::Task, None))
            .expect("recreate the card");

        let resolved = resolve(&paths, "card-t00001")
            .expect("resolve")
            .expect("present");
        remove_resolved(&resolved).expect("removal clears the stale tombstone and succeeds");
        assert!(!pool.join("card-t00001").exists(), "the card dir is gone");
        assert!(
            !pool.join(".card-t00001.removing").exists(),
            "the stale tombstone is gone too"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// Probe order is deterministic: a dir-backed home shadows an entry with
    /// the same id (a store corruption the doctor flags; reads stay stable).
    #[test]
    fn locate_prefers_a_dir_home_over_an_entry() {
        let paths = temp_cards_repo("locate-precedence");
        create_card(&paths, &typed_card("card-x00001", CardType::Decision, None))
            .expect("create the entry");
        seed_flat(&paths, &typed_card("card-x00001", CardType::Task, None));

        let home = locate(&paths, "card-x00001")
            .expect("locate")
            .expect("present");
        assert_eq!(
            home,
            CardHome::Dir(card_path(&paths, "card-x00001")),
            "the flat dir wins the probe order"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }

    /// A symlinked `tasks/` pool routes pooled probes -- and the CAS write a
    /// probe hit backs -- outside the store. The pool level has its own guard:
    /// `dir_card_exists` only checks the task dir, which reads as a real dir
    /// behind a symlinked pool.
    #[test]
    fn symlinked_task_pools_are_refused() {
        let paths = temp_cards_repo("symlinked-pool");
        let external = std::env::temp_dir().join(format!(
            "maestro-symlinked-pool-ext-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: test clock after Unix epoch")
                .as_nanos()
        ));
        let outside = external.join("card-t00001");
        ensure_dir(&outside).expect("external task dir");
        std::fs::write(
            outside.join(TASK_FILE),
            serde_yaml::to_string(&typed_card("card-t00001", CardType::Task, None))
                .expect("invariant: fixture serializes"),
        )
        .expect("external record");

        // Root pool symlinked at cards/tasks.
        crate::foundation::core::fs::create_directory_symlink(
            &external,
            &paths.cards_dir().join(TASKS_DIR),
        )
        .expect("symlink the root pool");
        // Nested pool symlinked inside a real feature container.
        create_card(&paths, &typed_card("csv-export", CardType::Feature, None))
            .expect("create the feature container");
        crate::foundation::core::fs::create_directory_symlink(
            &external,
            &paths.cards_dir().join("csv-export").join(TASKS_DIR),
        )
        .expect("symlink the nested pool");

        assert_eq!(
            locate(&paths, "card-t00001").expect("locate"),
            None,
            "a pooled record behind a symlinked pool is invisible"
        );
        for parent in [None, Some("csv-export")] {
            let error = create_card(&paths, &typed_card("card-t00002", CardType::Task, parent))
                .expect_err("creating into a symlinked pool must fail");
            assert!(format!("{error:#}").contains("symlink"), "{error:#}");
        }
        assert!(
            !external.join("card-t00002").exists(),
            "nothing was written through the symlink"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
        let _ = std::fs::remove_dir_all(&external);
    }

    #[test]
    fn a_symlinked_card_dir_fails_the_single_load_loud() {
        let paths = temp_cards_repo("symlinked-card-dir");
        let external = std::env::temp_dir().join(format!(
            "maestro-symlinked-card-ext-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: test clock after Unix epoch")
                .as_nanos()
        ));
        ensure_dir(&external).expect("external dir");
        // A pre-planted symlink at the deterministic feature slug: treating it
        // as absence would let `create` save card.yaml through the link.
        crate::foundation::core::fs::create_directory_symlink(
            &external,
            &paths.cards_dir().join("csv-export"),
        )
        .expect("symlink the card dir");

        let error = load_with_snapshot(&paths.cards_dir().join("csv-export").join(CARD_FILE))
            .expect_err("a symlinked card dir must not read as absent");
        assert!(format!("{error:#}").contains("symlink"), "{error:#}");

        let error = create_card(&paths, &typed_card("csv-export", CardType::Feature, None))
            .expect_err("creating through a symlinked card dir must fail");
        assert!(format!("{error:#}").contains("symlink"), "{error:#}");
        assert!(
            !external.join(CARD_FILE).exists(),
            "nothing was written through the symlink"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
        let _ = std::fs::remove_dir_all(&external);
    }

    #[test]
    fn load_entries_tolerates_an_empty_file_and_flags_schema_mismatch() {
        let paths = temp_cards_repo("entries-edges");
        let file = paths.cards_dir().join("decisions.yaml");

        std::fs::write(&file, "").expect("write empty file");
        let empty = load_entries(&file).expect("an empty file loads");
        assert!(empty.cards.is_empty());

        let mut bad = typed_card("card-bad001", CardType::Decision, None);
        bad.schema_version = "not-a-version".to_string();
        let contents = serde_yaml::to_string(&vec![bad]).expect("invariant: fixture serializes");
        std::fs::write(&file, contents).expect("write mismatched entry");
        let error = load_entries(&file).expect_err("schema mismatch fails the file");
        assert!(
            format!("{error:#}").contains("card-bad001"),
            "the failing entry is named: {error:#}"
        );

        let _ = std::fs::remove_dir_all(paths.cards_dir());
    }
}
