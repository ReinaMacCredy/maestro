//! Fold pre-migration flat leaf dirs (`cards/<id>/card.yaml`) into the
//! container layout (SPEC-card-sprawl S2-S5'): decisions become entries in
//! their container's `decisions.yaml`, ideas entries in the root `ideas.yaml`,
//! and workable cards per-task dirs under their container's `tasks/` pool.
//! Feature dirs are not touched -- they ARE the container layout.
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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{
    CARD_FILE, CardHome, EntriesSnapshot, card_dir_ids, home_for_new, load, load_entries,
    load_with_snapshot, save_entries, save_with_snapshot,
};
use crate::foundation::core::fs::ensure_dir;
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
    flat_dir: PathBuf,
    target_yaml: PathBuf,
    card: Card,
}

/// Fold every flat leaf card dir into the container layout.
pub fn run(paths: &MaestroPaths) -> Result<ContainerMigrateReport> {
    let cards_dir = paths.cards_dir();
    let mut report = ContainerMigrateReport::default();

    // Pass 1: collect and verify, writing nothing.
    let mut entry_folds: BTreeMap<PathBuf, (EntriesSnapshot, Vec<Card>)> = BTreeMap::new();
    let mut task_moves: Vec<TaskMove> = Vec::new();
    let mut sidecar_syncs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut leftover_dirs: Vec<PathBuf> = Vec::new();

    for id in card_dir_ids(&cards_dir)? {
        let flat_dir = cards_dir.join(&id);
        let Some(card) = load(&flat_dir.join(CARD_FILE))? else {
            continue;
        };
        if card.card_type == CardType::Feature {
            continue;
        }
        match home_for_new(paths, &card)? {
            CardHome::Entry(file) => {
                // An entry carries only the record: any other file in the flat
                // dir (a note, an evidence dir, a stale lock) would be lost.
                ensure_only_record_file(&flat_dir)?;
                if !entry_folds.contains_key(&file) {
                    entry_folds.insert(file.clone(), (load_entries(&file)?, Vec::new()));
                }
                let (snapshot, append) = entry_folds
                    .get_mut(&file)
                    .expect("invariant: entry group inserted above");
                match snapshot.cards.iter().find(|entry| entry.id == card.id) {
                    Some(entry) if *entry == card => {
                        report.finished += 1;
                        leftover_dirs.push(flat_dir);
                    }
                    Some(_) => bail!(divergent_copies(&card.id, &flat_dir, &file)),
                    None => {
                        tally(&mut report, card.card_type);
                        append.push(card);
                        leftover_dirs.push(flat_dir);
                    }
                }
            }
            CardHome::Dir(target_yaml) => match load(&target_yaml)? {
                Some(existing) if existing == card => {
                    // The record landed but the crash may have stranded
                    // sidecars flat; carry them before the dir is removed.
                    report.finished += 1;
                    sidecar_syncs.push((flat_dir.clone(), pooled_dir(&target_yaml)?));
                    leftover_dirs.push(flat_dir);
                }
                Some(_) => bail!(divergent_copies(&card.id, &flat_dir, &target_yaml)),
                None => {
                    tally(&mut report, card.card_type);
                    task_moves.push(TaskMove {
                        flat_dir,
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
    for fold in &task_moves {
        copy_task_dir(fold)?;
        leftover_dirs.push(fold.flat_dir.clone());
    }
    for (flat_dir, target_dir) in &sidecar_syncs {
        merge_sidecars(flat_dir, target_dir)?;
    }

    // Pass 3: the flat dirs go only after every write landed.
    for dir in leftover_dirs {
        fs::remove_dir_all(&dir).with_context(|| format!("failed to remove {}", dir.display()))?;
    }

    Ok(report)
}

fn tally(report: &mut ContainerMigrateReport, card_type: CardType) {
    match card_type {
        CardType::Decision => report.decisions += 1,
        CardType::Idea => report.ideas += 1,
        CardType::Task | CardType::Bug | CardType::Chore => report.tasks += 1,
        CardType::Feature => {}
    }
}

/// Write the pooled record, then copy every sidecar beside it. The record
/// write is a CAS against the absent target, so a racing create of the same
/// pooled id loses cleanly.
fn copy_task_dir(fold: &TaskMove) -> Result<()> {
    let snapshot = load_with_snapshot(&fold.target_yaml)?;
    save_with_snapshot(&fold.target_yaml, &fold.card, &snapshot)
        .with_context(|| format!("failed to write pooled card {}", fold.card.id))?;
    merge_sidecars(&fold.flat_dir, &pooled_dir(&fold.target_yaml)?)
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

/// Refuse to fold a flat dir into an entry while it holds anything besides its
/// record -- the entry has nowhere to carry a sidecar.
fn ensure_only_record_file(flat_dir: &Path) -> Result<()> {
    for entry in
        fs::read_dir(flat_dir).with_context(|| format!("failed to read {}", flat_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", flat_dir.display()))?;
        if entry.file_name().to_str() != Some(CARD_FILE) {
            bail!(
                "cannot fold {} into a container entry: {} holds {} besides its record; move or remove it first",
                flat_dir.file_name().unwrap_or_default().to_string_lossy(),
                flat_dir.display(),
                entry.file_name().to_string_lossy()
            );
        }
    }
    Ok(())
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
            paths.cards_dir().join("card-d2").join("notes.md"),
            "stranded\n",
        )
        .expect("decision sidecar");

        let error = run(&paths).expect_err("a sidecar on an entry fold must abort");
        let message = format!("{error:#}");
        assert!(message.contains("card-d2"), "{message}");
        assert!(message.contains("notes.md"), "{message}");

        cleanup(&root);
    }
}
