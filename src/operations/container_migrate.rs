//! Fold pre-migration flat leaf dirs (`cards/<id>/card.yaml`) into the
//! container layout (SPEC-card-sprawl S2-S5'): decisions become entries in
//! their container's `decisions.yaml`, ideas entries in the root `ideas.yaml`,
//! and workable cards per-task dirs under their container's `tasks/` pool.
//! A flat decision/idea's `notes.md` (written by the pre-container note verb)
//! folds into the container's shared log with `[<id>]` attribution. Feature
//! dirs are not touched -- they ARE the container layout.
//!
//! Idempotent and crash-safe in three passes: collect every flat card and
//! verify its target slot (writing nothing), then write -- ONE `save_entries`
//! per container file, one pooled dir per task -- and only then remove the
//! flat dirs. A crash between write and removal leaves both copies; the next
//! run sees the byte-equal pair, carries any sidecar the crash stranded flat,
//! removes the leftover dir, and counts it as a finished move. A target slot
//! holding DIFFERENT content aborts loud before anything is written, because
//! either copy could carry the newer edit.
//!
//! The live store only: archived flat dirs keep reading through the
//! resolver's flat fallback, and an unarchive lands the dir back here where
//! the next run folds it.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{
    CARD_FILE, CardHome, CardSnapshot, EntriesSnapshot, card_dir_ids, home_for_new, load,
    load_entries, load_with_snapshot, remove_dir_with_snapshot, save_entries, save_with_snapshot,
};
use crate::foundation::core::fs::{append_text_file, ensure_dir};
use crate::foundation::core::paths::MaestroPaths;

/// Per-type tally of a container-fold run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerMigrateReport {
    pub decisions: usize,
    pub ideas: usize,
    pub tasks: usize,
    /// Flat dirs whose card already lived byte-equal at its container home
    /// (a move an earlier run wrote but did not finish removing).
    pub finished: usize,
}

/// One pooled-task move collected in pass 1.
struct TaskMove {
    flat: FlatRemoval,
    target_yaml: PathBuf,
    card: Card,
}

/// A flat card dir queued for removal after the container write lands.
#[derive(Clone)]
struct FlatRemoval {
    flat_dir: PathBuf,
    record_path: PathBuf,
    snapshot: CardSnapshot,
}

/// One flat decision/idea `notes.md` queued to fold into its container's
/// shared log.
struct NotesFold {
    card_id: String,
    flat_notes: PathBuf,
    container_dir: PathBuf,
}

/// Fold every flat leaf card dir into the container layout.
pub fn run(paths: &MaestroPaths) -> Result<ContainerMigrateReport> {
    let cards_dir = paths.cards_dir();
    let mut report = ContainerMigrateReport::default();

    // Pass 1: collect and verify, writing nothing.
    let mut entry_folds: BTreeMap<PathBuf, (EntriesSnapshot, Vec<Card>)> = BTreeMap::new();
    let mut task_moves: Vec<TaskMove> = Vec::new();
    let mut notes_folds: Vec<NotesFold> = Vec::new();
    let mut sidecar_syncs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut leftover_dirs: Vec<FlatRemoval> = Vec::new();

    for id in card_dir_ids(&cards_dir)? {
        let flat_dir = cards_dir.join(&id);
        let record_path = flat_dir.join(CARD_FILE);
        let snapshot = load_with_snapshot(&record_path)?;
        let Some(card) = snapshot.card.clone() else {
            continue;
        };
        if card.card_type == CardType::Feature {
            continue;
        }
        let flat = FlatRemoval {
            flat_dir,
            record_path,
            snapshot,
        };
        match home_for_new(paths, &card)? {
            CardHome::Entry(file) => {
                // An entry carries only the record. A `notes.md` (the one
                // sidecar the legacy note verb wrote on every card type)
                // folds into the container's shared log; anything else (an
                // evidence dir, a stale lock) has nowhere to go and aborts.
                if let Some(flat_notes) = foldable_notes(&flat.flat_dir)? {
                    notes_folds.push(NotesFold {
                        card_id: card.id.clone(),
                        flat_notes,
                        container_dir: file
                            .parent()
                            .with_context(|| {
                                format!("entry file missing parent: {}", file.display())
                            })?
                            .to_path_buf(),
                    });
                }
                if !entry_folds.contains_key(&file) {
                    entry_folds.insert(file.clone(), (load_entries(&file)?, Vec::new()));
                }
                let (snapshot, append) = entry_folds
                    .get_mut(&file)
                    .expect("invariant: entry group inserted above");
                match snapshot.cards.iter().find(|entry| entry.id == card.id) {
                    Some(entry) if *entry == card => {
                        report.finished += 1;
                        leftover_dirs.push(flat);
                    }
                    Some(_) => bail!(divergent_copies(&card.id, &flat.flat_dir, &file)),
                    None => {
                        tally(&mut report, card.card_type);
                        append.push(card);
                        leftover_dirs.push(flat);
                    }
                }
            }
            CardHome::Dir(target_yaml) => match load(&target_yaml)? {
                Some(existing) if existing == card => {
                    // The record landed but the crash may have stranded
                    // sidecars flat; carry them before the dir is removed.
                    report.finished += 1;
                    sidecar_syncs.push((flat.flat_dir.clone(), pooled_dir(&target_yaml)?));
                    leftover_dirs.push(flat);
                }
                Some(_) => bail!(divergent_copies(&card.id, &flat.flat_dir, &target_yaml)),
                None => {
                    tally(&mut report, card.card_type);
                    task_moves.push(TaskMove {
                        flat,
                        target_yaml,
                        card,
                    });
                }
            },
        }
    }

    // Pass 2: writes. One whole-file CAS per container file; per-task dirs are
    // copied whole (record renamed card.yaml -> task.yaml, sidecars ride).
    for (file, (snapshot, append)) in &entry_folds {
        if append.is_empty() {
            continue;
        }
        let mut cards = snapshot.cards.clone();
        cards.extend(append.iter().cloned());
        save_entries(file, &cards, snapshot)?;
    }
    for fold in &notes_folds {
        fold_notes(fold)?;
    }
    for fold in &task_moves {
        copy_task_dir(fold)?;
        leftover_dirs.push(fold.flat.clone());
    }
    for (flat_dir, target_dir) in &sidecar_syncs {
        merge_sidecars(flat_dir, target_dir)?;
    }

    // Pass 3: the flat dirs go only after every write landed.
    for flat in &leftover_dirs {
        remove_flat_dir(flat)?;
    }

    Ok(report)
}

fn tally(report: &mut ContainerMigrateReport, card_type: CardType) {
    match card_type {
        CardType::Decision => report.decisions += 1,
        CardType::Idea => report.ideas += 1,
        CardType::Task | CardType::Bug | CardType::Chore => report.tasks += 1,
        CardType::Feature | CardType::Custom | CardType::Progress => {}
    }
}

/// Write the pooled record, then copy every sidecar beside it. The record
/// write is a CAS against the absent target, so a racing create of the same
/// pooled id loses cleanly.
fn copy_task_dir(fold: &TaskMove) -> Result<()> {
    let snapshot = load_with_snapshot(&fold.target_yaml)?;
    save_with_snapshot(&fold.target_yaml, &fold.card, &snapshot)
        .with_context(|| format!("failed to write pooled card {}", fold.card.id))?;
    merge_sidecars(&fold.flat.flat_dir, &pooled_dir(&fold.target_yaml)?)
}

fn remove_flat_dir(flat: &FlatRemoval) -> Result<()> {
    remove_dir_with_snapshot(&flat.record_path, &flat.snapshot)
        .with_context(|| format!("failed to remove {}", flat.flat_dir.display()))
}

/// The pooled dir a `tasks/<id>/task.yaml` record lives in.
fn pooled_dir(target_yaml: &Path) -> Result<PathBuf> {
    Ok(target_yaml
        .parent()
        .with_context(|| format!("pooled path missing parent: {}", target_yaml.display()))?
        .to_path_buf())
}

/// Copy every sidecar (everything beside the record) into the pooled dir,
/// taking only what is missing there: a crash-interrupted run may have copied
/// some already, and a landed copy may carry a newer edit.
fn merge_sidecars(flat_dir: &Path, target_dir: &Path) -> Result<()> {
    for entry in
        fs::read_dir(flat_dir).with_context(|| format!("failed to read {}", flat_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", flat_dir.display()))?;
        if entry.file_name().to_str() == Some(CARD_FILE) {
            continue;
        }
        copy_missing(&entry.path(), &target_dir.join(entry.file_name()))?;
    }
    Ok(())
}

/// Recursive copy that never overwrites: existing target files win, missing
/// ones are filled in. Symlinks are skipped, matching the legacy fold's copy.
fn copy_missing(src: &Path, dst: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(src)
        .with_context(|| format!("failed to inspect {}", src.display()))?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_dir() {
        ensure_dir(dst)?;
        for entry in
            fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))?
        {
            let entry = entry.with_context(|| format!("failed to list {}", src.display()))?;
            copy_missing(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else if !dst.exists() {
        if let Some(parent) = dst.parent() {
            ensure_dir(parent)?;
        }
        fs::copy(src, dst)
            .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))?;
    }
    Ok(())
}

/// The one sidecar an entry fold can carry: a `notes.md` beside the record,
/// which folds into the container's shared log. Anything else in the flat dir
/// has nowhere to go and aborts loud.
fn foldable_notes(flat_dir: &Path) -> Result<Option<PathBuf>> {
    let mut notes = None;
    for entry in
        fs::read_dir(flat_dir).with_context(|| format!("failed to read {}", flat_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", flat_dir.display()))?;
        match entry.file_name().to_str() {
            Some(CARD_FILE) => {}
            Some("notes.md") => notes = Some(entry.path()),
            _ => bail!(
                "cannot fold {} into a container entry: {} holds {} besides its record; move or remove it first",
                flat_dir.file_name().unwrap_or_default().to_string_lossy(),
                flat_dir.display(),
                entry.file_name().to_string_lossy()
            ),
        }
    }
    Ok(notes)
}

/// Fold a flat decision/idea dir's `notes.md` into its container's shared
/// log -- the same file (and the same `<date>  [<id>] <text>` line shape) the
/// note verb writes entry-backed notes to. The legacy per-card title header
/// is dropped, dated lines keep their date under the new attribution, and
/// lines the log already holds are skipped so a crash-interrupted fold
/// re-runs clean.
fn fold_notes(fold: &NotesFold) -> Result<()> {
    let source = fs::read_to_string(&fold.flat_notes)
        .with_context(|| format!("failed to read {}", fold.flat_notes.display()))?;
    let log_path = fold.container_dir.join("notes.md");
    let existing = if log_path.exists() {
        fs::read_to_string(&log_path)
            .with_context(|| format!("failed to read {}", log_path.display()))?
    } else {
        String::new()
    };
    let logged: HashSet<&str> = existing.lines().collect();

    let mut pending: Vec<String> = Vec::new();
    let mut past_header = false;
    for line in source.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !past_header {
            past_header = true;
            if line.starts_with("# ") {
                continue;
            }
        }
        let attributed = match dated(line) {
            Some((date, text)) => format!("{date}  [{}] {text}", fold.card_id),
            None => format!("[{}] {line}", fold.card_id),
        };
        if logged.contains(attributed.as_str()) || pending.contains(&attributed) {
            continue;
        }
        pending.push(attributed);
    }
    if pending.is_empty() {
        return Ok(());
    }

    // Header seeding mirrors edit::append_note: the container card's title,
    // or the root-log fallback.
    let header = load(&fold.container_dir.join(CARD_FILE))?
        .map(|container| container.title)
        .unwrap_or_else(|| "Notes".to_string());
    let mut block = pending.join("\n");
    block.push('\n');
    append_text_file(&log_path, &format!("# {header}\n\n"), &block)
        .with_context(|| format!("failed to fold notes into {}", log_path.display()))?;
    Ok(())
}

/// Split a legacy `YYYY-MM-DD  <text>` note line into its date and text.
fn dated(line: &str) -> Option<(&str, &str)> {
    let date = line.get(..10)?;
    let rest = line.get(10..)?;
    let date_shaped = date.chars().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 {
            c == '-'
        } else {
            c.is_ascii_digit()
        }
    });
    (date_shaped && rest.starts_with(char::is_whitespace)).then(|| (date, rest.trim_start()))
}

fn divergent_copies(id: &str, flat_dir: &Path, target: &Path) -> String {
    format!(
        "card {id} exists both flat ({}) and at its container home ({}) with different content; reconcile the two copies, remove the stale one, then re-run the migration",
        flat_dir.display(),
        target.display()
    )
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::card::store::{TASK_FILE, locate, resolve};

    const NOW: &str = "2026-06-10T12:00:00Z";

    fn temp_repo(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("maestro-contmig-{name}-{}-{nanos}", process::id()))
    }

    fn typed_card(id: &str, card_type: CardType, parent: Option<&str>) -> Card {
        let mut card = Card::new(id, card_type, &format!("Card {id}"), "open", NOW);
        card.parent = parent.map(str::to_string);
        card
    }

    /// Plant a card as a pre-migration flat leaf dir, bypassing the seam.
    fn seed_flat(paths: &MaestroPaths, card: &Card) {
        let yaml = paths.cards_dir().join(&card.id).join(CARD_FILE);
        fs::create_dir_all(yaml.parent().expect("flat dir")).expect("create flat dir");
        fs::write(
            &yaml,
            serde_yaml::to_string(card).expect("serialize fixture"),
        )
        .expect("write fixture");
    }

    /// A store with one feature container plus one flat dir of every
    /// non-feature kind, parented and rootless.
    fn flat_store(root: &Path) -> MaestroPaths {
        let paths = MaestroPaths::new(root);
        seed_flat(&paths, &typed_card("csv-export", CardType::Feature, None));
        seed_flat(
            &paths,
            &typed_card("card-d1", CardType::Decision, Some("csv-export")),
        );
        seed_flat(&paths, &typed_card("card-d2", CardType::Decision, None));
        seed_flat(&paths, &typed_card("card-i1", CardType::Idea, None));
        seed_flat(
            &paths,
            &typed_card("card-t1", CardType::Task, Some("csv-export")),
        );
        seed_flat(&paths, &typed_card("card-t2", CardType::Bug, None));
        paths
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folds_every_flat_kind_into_its_container_home() {
        let root = temp_repo("fold");
        let paths = flat_store(&root);
        fs::write(paths.cards_dir().join("card-t1").join("notes.md"), "kept\n")
            .expect("task sidecar");

        let report = run(&paths).expect("fold succeeds");
        assert_eq!(
            report,
            ContainerMigrateReport {
                decisions: 2,
                ideas: 1,
                tasks: 2,
                finished: 0
            }
        );

        let cards = paths.cards_dir();
        // The feature container is untouched; every flat leaf dir is gone.
        assert!(cards.join("csv-export").join(CARD_FILE).is_file());
        for id in ["card-d1", "card-d2", "card-i1", "card-t1", "card-t2"] {
            assert!(!cards.join(id).exists(), "flat dir {id} removed");
        }
        // Each card resolves at its container home.
        let home = |id: &str| {
            locate(&paths, id)
                .expect("locate")
                .unwrap_or_else(|| panic!("{id} resolves"))
                .path()
                .to_path_buf()
        };
        assert_eq!(
            home("card-d1"),
            cards.join("csv-export").join("decisions.yaml")
        );
        assert_eq!(home("card-d2"), cards.join("decisions.yaml"));
        assert_eq!(home("card-i1"), cards.join("ideas.yaml"));
        assert_eq!(
            home("card-t1"),
            cards
                .join("csv-export")
                .join("tasks")
                .join("card-t1")
                .join(TASK_FILE)
        );
        assert_eq!(
            home("card-t2"),
            cards.join("tasks").join("card-t2").join(TASK_FILE)
        );
        // The task's sidecar rode the move; the record kept its content.
        assert_eq!(
            fs::read_to_string(
                cards
                    .join("csv-export")
                    .join("tasks")
                    .join("card-t1")
                    .join("notes.md")
            )
            .expect("sidecar"),
            "kept\n"
        );
        assert_eq!(
            resolve(&paths, "card-t1")
                .expect("resolve")
                .expect("present")
                .card
                .title,
            "Card card-t1"
        );

        cleanup(&root);
    }

    #[test]
    fn rerun_is_a_no_op() {
        let root = temp_repo("rerun");
        let paths = flat_store(&root);

        run(&paths).expect("first fold");
        let second = run(&paths).expect("second fold");
        assert_eq!(second, ContainerMigrateReport::default());

        cleanup(&root);
    }

    #[test]
    fn an_interrupted_move_is_finished_on_rerun() {
        let root = temp_repo("finish");
        let paths = flat_store(&root);

        // Simulate a crash between pass 2 and pass 3: the entry landed, the
        // flat dir survived byte-equal.
        let entries_file = paths.cards_dir().join("ideas.yaml");
        let idea = typed_card("card-i1", CardType::Idea, None);
        save_entries(
            &entries_file,
            std::slice::from_ref(&idea),
            &load_entries(&entries_file).expect("absent snapshot"),
        )
        .expect("seed the written half of the move");

        let report = run(&paths).expect("fold finishes the move");
        assert_eq!(report.finished, 1, "the leftover dir counts as finished");
        assert_eq!(report.ideas, 0, "the idea is not folded twice");
        assert!(!paths.cards_dir().join("card-i1").exists());
        let snapshot = load_entries(&entries_file).expect("entries");
        assert_eq!(
            snapshot
                .cards
                .iter()
                .filter(|card| card.id == "card-i1")
                .count(),
            1,
            "no duplicate entry"
        );

        cleanup(&root);
    }

    /// A crash after the pooled record landed but before the sidecars copied:
    /// the re-run must carry the stranded sidecars (without clobbering any
    /// that already landed) before it removes the flat dir.
    #[test]
    fn an_interrupted_task_move_carries_stranded_sidecars() {
        let root = temp_repo("stranded");
        let paths = flat_store(&root);
        let flat = paths.cards_dir().join("card-t1");
        fs::write(flat.join("notes.md"), "stale flat copy\n").expect("flat notes");
        fs::create_dir_all(flat.join("proof")).expect("flat proof dir");
        fs::write(flat.join("proof").join("evidence.txt"), "stranded\n").expect("flat proof");

        // Simulate the crash: the record landed byte-equal, notes.md landed
        // and was since edited at its new home, proof/ never copied.
        let pooled = paths
            .cards_dir()
            .join("csv-export")
            .join("tasks")
            .join("card-t1");
        fs::create_dir_all(&pooled).expect("pooled dir");
        fs::write(
            pooled.join(TASK_FILE),
            serde_yaml::to_string(&typed_card("card-t1", CardType::Task, Some("csv-export")))
                .expect("serialize fixture"),
        )
        .expect("pooled record");
        fs::write(pooled.join("notes.md"), "edited at home\n").expect("pooled notes");

        let report = run(&paths).expect("fold finishes the move");
        assert_eq!(report.finished, 1, "the leftover dir counts as finished");
        assert!(!flat.exists(), "flat dir removed");
        assert_eq!(
            fs::read_to_string(pooled.join("proof").join("evidence.txt")).expect("carried"),
            "stranded\n",
            "the never-copied sidecar rode the re-run"
        );
        assert_eq!(
            fs::read_to_string(pooled.join("notes.md")).expect("kept"),
            "edited at home\n",
            "the landed copy is never overwritten"
        );

        cleanup(&root);
    }

    #[test]
    fn flat_dir_removal_rejects_a_stale_record_snapshot() {
        let root = temp_repo("stale-remove");
        let paths = flat_store(&root);
        let flat_dir = paths.cards_dir().join("card-i1");
        let record_path = flat_dir.join(CARD_FILE);
        let snapshot = load_with_snapshot(&record_path).expect("load flat snapshot");
        let flat = FlatRemoval {
            flat_dir: flat_dir.clone(),
            record_path: record_path.clone(),
            snapshot,
        };

        let mut edited = typed_card("card-i1", CardType::Idea, None);
        edited.title = "Edited while migration was running".to_string();
        fs::write(
            &record_path,
            serde_yaml::to_string(&edited).expect("serialize edited card"),
        )
        .expect("write racing edit");

        let error = remove_flat_dir(&flat).expect_err("stale removal must be rejected");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );
        assert!(
            flat_dir.join(CARD_FILE).is_file(),
            "the edited flat card remains for the retry"
        );
        assert_eq!(
            resolve(&paths, "card-i1")
                .expect("resolve")
                .expect("flat card present")
                .card
                .title,
            "Edited while migration was running"
        );

        cleanup(&root);
    }

    #[test]
    fn divergent_copies_abort_loud_before_any_write() {
        let root = temp_repo("divergent");
        let paths = flat_store(&root);

        let mut edited = typed_card("card-d2", CardType::Decision, None);
        edited.title = "Edited after the entry was written".to_string();
        let entries_file = paths.cards_dir().join("decisions.yaml");
        save_entries(
            &entries_file,
            std::slice::from_ref(&edited),
            &load_entries(&entries_file).expect("absent snapshot"),
        )
        .expect("seed a diverged entry");

        let error = run(&paths).expect_err("divergent copies must abort");
        assert!(
            format!("{error:#}").contains("card card-d2 exists both flat"),
            "{error:#}"
        );
        // Pass-1 abort: no other flat dir was folded.
        for id in ["card-d1", "card-i1", "card-t1", "card-t2"] {
            assert!(
                paths.cards_dir().join(id).join(CARD_FILE).is_file(),
                "{id} untouched after the abort"
            );
        }
        assert!(
            !paths.cards_dir().join("ideas.yaml").exists(),
            "no entry file written after the abort"
        );

        cleanup(&root);
    }

    /// The composed `maestro migrate` pipeline: legacy fold, container fold,
    /// then an edit to a folded card. The re-run must skip every folded card
    /// via the resolver (not the flat path) -- re-minting one flat would
    /// resurrect a stale copy and the container fold would abort on it.
    #[test]
    fn rerun_after_the_container_fold_skips_edited_cards() {
        let root = temp_repo("composed");
        let paths = flat_store(&root);

        // Stage a legacy decision store so the legacy fold has work to do.
        fs::create_dir_all(paths.decisions_file().parent().expect("dir")).expect("decisions dir");
        fs::write(
            paths.decisions_file(),
            "schema_version: maestro.decisions.v1\ndecisions:\n  - id: decision-001\n    title: Pick the writer\n    status: locked\n    created_at: 2026-06-01T04:00:00Z\n",
        )
        .expect("legacy decisions store");

        crate::operations::card_migrate::run(&paths, NOW).expect("legacy fold");
        run(&paths).expect("container fold");

        // Edit the folded legacy decision through the resolver, as a verb would.
        let folded = resolve(&paths, &crate::domain::card::store::hash_id("decision-001"))
            .expect("resolve")
            .expect("folded decision present");
        let mut edited = folded.card.clone();
        edited.status = "superseded".to_string();
        crate::domain::card::store::save_resolved(&edited, &folded).expect("edit the entry");

        // Re-run both stages with the legacy tree still in place.
        let refold =
            crate::operations::card_migrate::run(&paths, "2026-06-10T13:00:00Z").expect("refold");
        assert_eq!(refold.decisions, 0, "the edited decision is not re-minted");
        assert!(refold.skipped > 0, "the legacy artifact counts as skipped");
        let second = run(&paths).expect("container refold");
        assert_eq!(second, ContainerMigrateReport::default());
        assert_eq!(
            resolve(&paths, &edited.id)
                .expect("resolve")
                .expect("still present")
                .card
                .status,
            "superseded",
            "the edit survives the re-run"
        );

        cleanup(&root);
    }

    #[test]
    fn an_entry_fold_with_a_sidecar_aborts_loud() {
        let root = temp_repo("sidecar");
        let paths = flat_store(&root);
        fs::write(
            paths.cards_dir().join("card-d2").join("evidence.txt"),
            "stranded\n",
        )
        .expect("decision sidecar");

        let error = run(&paths).expect_err("a sidecar on an entry fold must abort");
        let message = format!("{error:#}");
        assert!(message.contains("card-d2"), "{message}");
        assert!(message.contains("evidence.txt"), "{message}");

        cleanup(&root);
    }

    /// A flat decision/idea dir holding the one sidecar the legacy note verb
    /// wrote -- `notes.md` -- folds it into the container's shared log with
    /// `[<id>]` attribution instead of aborting.
    #[test]
    fn entry_notes_fold_into_the_container_log() {
        let root = temp_repo("notes-fold");
        let paths = flat_store(&root);
        fs::write(
            paths.cards_dir().join("card-d1").join("notes.md"),
            "# Card card-d1\n\n2026-06-08  picked the writer\nfree-form remark\n",
        )
        .expect("decision notes");
        fs::write(
            paths.cards_dir().join("card-i1").join("notes.md"),
            "2026-06-09  raw idea\n",
        )
        .expect("idea notes");

        run(&paths).expect("fold succeeds");

        let container_log =
            fs::read_to_string(paths.cards_dir().join("csv-export").join("notes.md"))
                .expect("container log");
        assert!(
            container_log.starts_with("# Card csv-export\n\n"),
            "container log seeds the feature title: {container_log:?}"
        );
        assert!(
            container_log.contains("2026-06-08  [card-d1] picked the writer\n"),
            "dated line keeps its date under the attribution: {container_log:?}"
        );
        assert!(
            container_log.contains("[card-d1] free-form remark\n"),
            "undated line is carried attributed: {container_log:?}"
        );
        assert!(
            !container_log.contains("# Card card-d1"),
            "the per-card title header is dropped: {container_log:?}"
        );
        let root_log =
            fs::read_to_string(paths.cards_dir().join("notes.md")).expect("root shared log");
        assert!(
            root_log.starts_with("# Notes\n\n"),
            "root log seeds the fallback header: {root_log:?}"
        );
        assert!(
            root_log.contains("2026-06-09  [card-i1] raw idea\n"),
            "{root_log:?}"
        );

        cleanup(&root);
    }

    /// A crash after the entry and its notes landed but before the flat dir
    /// was removed: the re-run's finished arm refolds the notes without
    /// duplicating lines the log already holds.
    #[test]
    fn a_refolded_notes_file_does_not_duplicate_log_lines() {
        let root = temp_repo("notes-refold");
        let paths = flat_store(&root);
        let flat_notes = paths.cards_dir().join("card-d2").join("notes.md");
        fs::write(&flat_notes, "2026-06-08  first ruling\n").expect("decision notes");

        run(&paths).expect("first fold");

        // Resurrect the flat dir byte-equal, as a crash before pass 3 would
        // leave it.
        seed_flat(&paths, &typed_card("card-d2", CardType::Decision, None));
        fs::write(&flat_notes, "2026-06-08  first ruling\n").expect("reseed notes");

        let report = run(&paths).expect("refold finishes the move");
        assert_eq!(report.finished, 1);
        let log = fs::read_to_string(paths.cards_dir().join("notes.md")).expect("shared log");
        assert_eq!(
            log.matches("2026-06-08  [card-d2] first ruling").count(),
            1,
            "no duplicate line: {log:?}"
        );
        assert!(!paths.cards_dir().join("card-d2").exists());

        cleanup(&root);
    }
}
