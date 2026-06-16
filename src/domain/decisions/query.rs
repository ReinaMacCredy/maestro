use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::schema::Card;
use crate::domain::card::suggest;
use crate::domain::decisions::cards;
use crate::domain::decisions::schema::DecisionRecord;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::paths::MaestroPaths;

/// One frozen legacy decision markdown file found under `.maestro/decisions`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionEntry {
    pub file_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionSource {
    Global,
    Feature { feature_id: String },
    Legacy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionListEntry {
    pub id: String,
    pub title: String,
    pub status: String,
    pub source: DecisionSource,
    pub path: PathBuf,
    pub created_at: String,
    pub locked_at: Option<String>,
}

impl DecisionListEntry {
    /// The timestamp the recent-N list windows on: when the fork was locked,
    /// else when it was opened. ISO-8601 strings sort chronologically; a legacy
    /// markdown decision carries neither and sorts oldest (empty string).
    pub fn activity(&self) -> &str {
        self.locked_at.as_deref().unwrap_or(&self.created_at)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionContent {
    Structured {
        record: Box<DecisionRecord>,
        source: DecisionSource,
        path: PathBuf,
    },
    Legacy {
        id: String,
        title: String,
        contents: String,
        path: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionDiagnostic {
    pub structured_count: usize,
    pub legacy_count: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// List frozen legacy decision markdown files.
pub fn decision_entries(decisions_dir: &Path) -> Result<Vec<DecisionEntry>> {
    if !decisions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(decisions_dir)
        .with_context(|| format!("failed to read {}", decisions_dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", decisions_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if is_decision_file_name(&file_name) {
            entries.push(DecisionEntry {
                file_name,
                path: entry.path(),
            });
        }
    }
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(entries)
}

pub fn list(paths: &MaestroPaths) -> Result<Vec<DecisionListEntry>> {
    let mut entries = Vec::new();
    for (record, source, path) in cards::scan(paths, false)? {
        entries.push(decision_list_entry(record, source, path));
    }
    for legacy in decision_entries(&paths.decisions_dir())? {
        entries.push(legacy_decision_list_entry(legacy)?);
    }
    sort_decision_entries(&mut entries);
    Ok(entries)
}

pub fn list_tolerant(paths: &MaestroPaths) -> Vec<DecisionListEntry> {
    let mut entries = Vec::new();
    // A corrupt decision card is skipped (the tolerant scan swallows it), not
    // surfaced as an `unreadable` row. This now DIVERGES from the feature roster,
    // which marks a schema-incompatible feature card Unreadable rather than
    // dropping it; surfacing unreadable decision rows the same way is a net-new
    // behavior left to the user as a follow-up card, not decided here.
    for (record, source, path) in cards::scan(paths, true).unwrap_or_default() {
        entries.push(decision_list_entry(record, source, path));
    }
    for legacy in decision_entries(&paths.decisions_dir()).unwrap_or_default() {
        entries.push(legacy_decision_list_entry_tolerant(legacy));
    }
    sort_decision_entries(&mut entries);
    entries
}

pub fn known_decision_ids(paths: &MaestroPaths) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for (record, _, _) in cards::scan(paths, false)? {
        ids.insert(record.id);
    }
    for legacy in decision_entries(&paths.decisions_dir())? {
        ids.insert(decision_display_id(&legacy.file_name));
    }
    Ok(ids)
}

fn decision_list_entry(
    record: DecisionRecord,
    source: DecisionSource,
    path: PathBuf,
) -> DecisionListEntry {
    DecisionListEntry {
        id: record.id,
        title: record.title,
        status: record.status.as_str().to_string(),
        source,
        path,
        created_at: record.created_at,
        locked_at: record.locked_at,
    }
}

fn legacy_decision_list_entry(legacy: DecisionEntry) -> Result<DecisionListEntry> {
    let title = decision_title(&legacy.path)?;
    Ok(legacy_decision_entry(legacy, title))
}

fn legacy_decision_list_entry_tolerant(legacy: DecisionEntry) -> DecisionListEntry {
    let title = decision_title(&legacy.path).unwrap_or_else(|error| format!("{error:#}"));
    legacy_decision_entry(legacy, title)
}

/// Build a legacy decision list entry once the title is resolved -- the strict
/// and tolerant variants differ only in how they obtain that title.
fn legacy_decision_entry(legacy: DecisionEntry, title: String) -> DecisionListEntry {
    DecisionListEntry {
        id: decision_display_id(&legacy.file_name),
        title,
        status: "legacy".to_string(),
        source: DecisionSource::Legacy,
        path: legacy.path,
        created_at: String::new(),
        locked_at: None,
    }
}

fn sort_decision_entries(entries: &mut [DecisionListEntry]) {
    entries.sort_by(|left, right| {
        decision_sort_key(&left.id)
            .cmp(&decision_sort_key(&right.id))
            .then_with(|| left.title.cmp(&right.title))
    });
}

pub fn decisions_for_feature(
    paths: &MaestroPaths,
    feature_id: &str,
) -> Result<Vec<DecisionRecord>> {
    let mut records: Vec<DecisionRecord> = cards::scan(paths, false)?
        .into_iter()
        .filter(|(_, source, _)| {
            matches!(source, DecisionSource::Feature { feature_id: id } if id == feature_id)
        })
        .map(|(record, _, _)| record)
        .collect();
    records.sort_by_key(|record| decision_sort_key(&record.id));
    Ok(records)
}

pub fn show(paths: &MaestroPaths, id: &str) -> Result<DecisionContent> {
    let id = normalize_decision_id(id)?;
    let Some(content) = find_decision_content(paths, &id)? else {
        return Err(not_found(paths, &id));
    };
    Ok(content)
}

/// The decision id-not-found error: carries the nearest known decision id so
/// the main.rs funnel prints a did-you-mean hint (a hint only -- the lookup is
/// never fuzzy-resolved).
pub(crate) fn not_found(paths: &MaestroPaths, id: &str) -> anyhow::Error {
    let nearest = known_decision_ids(paths)
        .ok()
        .and_then(|ids| suggest::did_you_mean(id, ids.iter().map(String::as_str)));
    MaestroError::IdNotFound {
        kind: "decision",
        id: id.to_string(),
        nearest,
    }
    .into()
}

fn find_decision_content(paths: &MaestroPaths, id: &str) -> Result<Option<DecisionContent>> {
    // The structured lookup and the frozen-legacy markdown lookup are a UNION: a
    // card-mode repo still reads `.maestro/decisions/*.md` (the migration never
    // folds it), and `lock`'s frozen-legacy guard and the supersedes validation
    // both depend on this resolving a markdown decision.
    if let Some((record, source, resolved)) = cards::load_one(paths, id)? {
        return Ok(Some(DecisionContent::Structured {
            record: Box::new(record),
            source,
            path: resolved.path().to_path_buf(),
        }));
    }

    let Some(path) = find_legacy_decision_path(&paths.decisions_dir(), id)? else {
        return Ok(None);
    };
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read decision file {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(Some(DecisionContent::Legacy {
        id: decision_display_id(&file_name),
        title: decision_title(&path)?,
        contents,
        path,
    }))
}

pub fn decision_exists(paths: &MaestroPaths, id: &str) -> Result<bool> {
    let id = normalize_decision_id(id)?;
    Ok(find_decision_content(paths, &id)?.is_some())
}

pub fn decision_bodies(paths: &MaestroPaths) -> Result<Vec<String>> {
    let mut bodies = Vec::new();
    for (record, _, _) in cards::scan(paths, false)? {
        bodies.push(render_record(&record));
    }
    for entry in decision_entries(&paths.decisions_dir())? {
        bodies.push(
            fs::read_to_string(&entry.path).with_context(|| {
                format!("failed to read decision file {}", entry.path.display())
            })?,
        );
    }
    Ok(bodies)
}

pub fn diagnose(paths: &MaestroPaths, cards: &[(Card, PathBuf)]) -> DecisionDiagnostic {
    let mut structured_count = 0_usize;
    let mut legacy_count = 0_usize;
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Decision cards come from the doctor's one shared store walk; envelope
    // failures are reported centrally there, so only a Decision-typed card
    // whose folded record fails to convert lands in this bucket.
    match cards::records_in_cards(cards, false) {
        Ok(decisions) => structured_count += decisions.len(),
        Err(error) => errors.push(format!("{error:#}")),
    }

    match decision_entries(&paths.decisions_dir()) {
        Ok(entries) => {
            legacy_count = entries.len();
            for entry in entries {
                match fs::read_to_string(&entry.path) {
                    Ok(contents)
                        if contents.contains("Why this decision exists.")
                            || contents.contains("What we decided.") =>
                    {
                        warnings.push(format!(
                            "{} still contains decision template placeholder text",
                            entry.file_name
                        ));
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(format!(
                        "failed to read decision file {}: {error}",
                        entry.path.display()
                    )),
                }
            }
        }
        Err(error) => errors.push(format!("{error:#}")),
    }

    DecisionDiagnostic {
        structured_count,
        legacy_count,
        warnings,
        errors,
    }
}

pub fn dangling_reference_warnings(paths: &MaestroPaths, cards: &[(Card, PathBuf)]) -> Vec<String> {
    let mut warnings = Vec::new();
    // Both the resolvable-id set and the supersedes scan derive from the
    // doctor's one shared store walk rather than re-reading the store.
    let records = decision_records_with_path(cards);
    let existing = resolvable_decision_ids(paths, &records);
    warn_dangling_note_pointers(paths, cards, &existing, &mut warnings);
    warn_dangling_supersedes(&records, &existing, &mut warnings);
    warnings.sort();
    warnings.dedup();
    warnings
}

/// Every structured decision paired with the card path it lives in, read
/// tolerantly (a corrupt decision record just drops from the scan). The legacy
/// markdown is folded into the resolvable set separately (it has no supersedes
/// to validate).
fn decision_records_with_path(cards: &[(Card, PathBuf)]) -> Vec<(PathBuf, DecisionRecord)> {
    cards::records_in_cards(cards, true)
        .unwrap_or_default()
        .into_iter()
        .map(|(record, _source, path)| (path, record))
        .collect()
}

pub fn render_record(record: &DecisionRecord) -> String {
    let mut out = String::new();
    out.push_str(&format!("id: {}\n", record.id));
    out.push_str(&format!("title: {}\n", record.title));
    out.push_str(&format!("status: {}\n", record.status.as_str()));
    if let Some(feature) = record.feature.as_deref() {
        out.push_str(&format!("feature: {feature}\n"));
    }
    out.push_str(&format!("created_at: {}\n", record.created_at));
    if let Some(locked_at) = record.locked_at.as_deref() {
        out.push_str(&format!("locked_at: {locked_at}\n"));
    }
    if let Some(context) = record.context.as_deref() {
        out.push_str("context:\n");
        out.push_str(&indent(context));
    }
    if let Some(decision) = record.decision.as_deref() {
        out.push_str("decision:\n");
        out.push_str(&indent(decision));
    }
    if !record.rejected.is_empty() {
        out.push_str("rejected:\n");
        for rejected in &record.rejected {
            out.push_str(&format!("- {rejected}\n"));
        }
    }
    if let Some(preview) = record.preview.as_deref() {
        out.push_str("preview:\n");
        out.push_str(&indent(preview));
    }
    if !record.supersedes.is_empty() {
        out.push_str("supersedes:\n");
        for id in &record.supersedes {
            out.push_str(&format!("- {id}\n"));
        }
    }
    if let Some(id) = record.superseded_by.as_deref() {
        out.push_str(&format!("superseded_by: {id}\n"));
    }
    out
}

/// Resolve a frozen legacy decision id or file name to a markdown path.
pub fn resolve_decision_path(decisions_dir: &Path, id: &str) -> Result<PathBuf> {
    find_legacy_decision_path(decisions_dir, id)?
        .with_context(|| format!("decision not found: {id}"))
}

fn find_legacy_decision_path(decisions_dir: &Path, id: &str) -> Result<Option<PathBuf>> {
    validate_decision_lookup_id(id)?;
    if id.ends_with(".md") {
        let path = decisions_dir.join(id);
        if valid_decision_file(&path)? {
            return Ok(Some(path));
        }
    }

    let direct = decisions_dir.join(format!("{id}.md"));
    if valid_decision_file(&direct)? {
        return Ok(Some(direct));
    }

    let prefix = format!("{id}-");
    let matches = decision_entries(decisions_dir)?
        .into_iter()
        .filter(|entry| entry.file_name.starts_with(&prefix))
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0].path.clone())),
        _ => bail!("decision id {id} is ambiguous"),
    }
}

fn valid_decision_file(path: &Path) -> Result<bool> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    Ok(metadata.is_file() && !metadata.file_type().is_symlink())
}

fn validate_decision_lookup_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid decision id: {id}");
    }
    Ok(())
}

pub fn normalize_decision_id(id: &str) -> Result<String> {
    validate_decision_lookup_id(id)?;
    let trimmed = id.trim_end_matches(".md");
    if let Some(number) = parse_decision_number(trimmed) {
        return Ok(format!("decision-{number:03}"));
    }
    if let Ok(number) = trimmed.parse::<u32>() {
        return Ok(format!("decision-{number:03}"));
    }
    Ok(trimmed.to_string())
}

/// Parse the sequence number from a decision id or file name.
pub fn parse_decision_number(value: &str) -> Option<u32> {
    let number = value.strip_prefix("decision-")?.split('-').next()?;
    number.parse::<u32>().ok()
}

/// Return the id portion of a decision file name.
pub fn decision_id(file_name: &str) -> &str {
    file_name.trim_end_matches(".md")
}

/// The canonical display id for a decision file: `decision-NNN` when the
/// sequence number parses, else the raw slug for a malformed name.
pub fn decision_display_id(file_name: &str) -> String {
    match parse_decision_number(file_name) {
        Some(number) => format!("decision-{number:03}"),
        None => decision_id(file_name).to_string(),
    }
}

fn resolvable_decision_ids(
    paths: &MaestroPaths,
    records: &[(PathBuf, DecisionRecord)],
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (_, record) in records {
        // Normalize before inserting (falling back to the raw id) so the
        // resolvable set matches the normalized ids the dangling-ref checks
        // look up -- a non-canonical stored id like `decision-7` must still
        // satisfy a `decision-007` reference.
        ids.insert(normalize_decision_id(&record.id).unwrap_or_else(|_| record.id.clone()));
    }
    for entry in decision_entries(&paths.decisions_dir()).unwrap_or_default() {
        if let Ok(id) = normalize_decision_id(&decision_display_id(&entry.file_name)) {
            ids.insert(id);
        }
    }
    ids
}

fn warn_dangling_note_pointers(
    paths: &MaestroPaths,
    cards: &[(Card, PathBuf)],
    existing: &BTreeSet<String>,
    warnings: &mut Vec<String>,
) {
    for path in feature_note_paths(paths, cards) {
        let Ok(Some(contents)) = read_to_string_if_exists(&path) else {
            continue;
        };
        for id in structured_note_decision_refs(&contents) {
            if !existing.contains(&id) {
                warnings.push(format!(
                    "{} references missing decision {id}; fix: restore that decision record or add a superseding note",
                    path.display()
                ));
            }
        }
    }
}

/// Every feature card's `notes.md` sidecar that may carry structured decision
/// pointers, derived from the shared store walk.
fn feature_note_paths(paths: &MaestroPaths, cards: &[(Card, PathBuf)]) -> Vec<PathBuf> {
    cards
        .iter()
        .filter(|(card, _)| card.card_type == crate::domain::card::schema::CardType::Feature)
        .map(|(card, _)| paths.cards_dir().join(&card.id).join("notes.md"))
        .collect()
}

fn warn_dangling_supersedes(
    records: &[(PathBuf, DecisionRecord)],
    existing: &BTreeSet<String>,
    warnings: &mut Vec<String>,
) {
    for (path, record) in records {
        for id in &record.supersedes {
            let normalized = normalize_decision_id(id).unwrap_or_else(|_| id.clone());
            if !existing.contains(&normalized) {
                warnings.push(format!(
                    "{} has {} superseding missing decision {normalized}; fix: restore the target decision or remove the supersedes entry",
                    path.display(),
                    record.id
                ));
            }
        }
    }
}

fn structured_note_decision_refs(contents: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in contents.lines() {
        if !(line.contains(" locked --") || line.contains(" superseded --")) {
            continue;
        }
        for token in line.split_whitespace() {
            let candidate = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
            if let Some(id) = note_decision_pointer(candidate) {
                ids.push(id);
                break;
            }
        }
    }
    ids
}

/// The decision id a structured note token points at: a canonical `decision-NNN`
/// (normalized), a content-addressed `card-<hash>` (the post-remint form, frozen
/// forever per SPEC-card-slug-ids D3), or a typed slug `dec-<slug>-<hex4>` (the
/// minted form). Any other token is not a structured pointer.
fn note_decision_pointer(candidate: &str) -> Option<String> {
    if candidate.starts_with("decision-") {
        normalize_decision_id(candidate).ok()
    } else if candidate.starts_with("card-") || candidate.starts_with("dec-") {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// The title from a decision file's `# decision-NNN: Title` heading, or
/// `<untitled>` when the heading is missing or malformed.
pub fn decision_title(path: &Path) -> Result<String> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let title = raw
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .and_then(|heading| heading.split_once(": ").map(|(_, title)| title.to_string()))
        .unwrap_or_else(|| "<untitled>".to_string());
    Ok(title)
}

fn is_decision_file_name(file_name: &str) -> bool {
    file_name.starts_with("decision-") && file_name.ends_with(".md")
}

fn decision_sort_key(id: &str) -> (u32, String) {
    (
        parse_decision_number(id).unwrap_or(u32::MAX),
        id.to_string(),
    )
}

fn indent(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::dangling_reference_warnings;
    use crate::domain::card::schema::{Card, CardType};
    use crate::domain::card::store::{card_path, load_with_snapshot, save_with_snapshot};
    use crate::domain::decisions::create::create_open;
    use crate::foundation::core::fs::ensure_dir;
    use crate::foundation::core::paths::MaestroPaths;

    const NOW: &str = "1970-01-01T00:00:00Z";

    fn card_mode_repo(name: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-{name}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    fn save_feature_card(paths: &MaestroPaths, id: &str) {
        let card = Card::new(id, CardType::Feature, "Feature", "in_progress", NOW);
        let path = card_path(paths, id);
        let snapshot = load_with_snapshot(&path).expect("snapshot");
        save_with_snapshot(&path, &card, &snapshot).expect("save feature card");
    }

    /// Part E liveness: in card mode the dangling-note gate must read the feature
    /// cards' sidecars (`cards/<feat>/notes.md`) and recognize a `card-<hash>`
    /// pointer. The pre-remint code read `features/<feat>/notes.md`, which is
    /// empty in card mode, so it would pass vacuously here. The valid-pointer leg
    /// guards over-warning; the dangling leg is the liveness proof -- an inert
    /// branch finds no notes and stays silent, failing this assertion.
    #[test]
    fn dangling_note_pointer_in_card_mode_warns_only_when_unresolved() {
        let paths = card_mode_repo("dangling-card-note");

        // A real decision card minted in card mode carries a `dec-<slug>-<hex4>` id.
        let decision =
            create_open(&paths, "Writer choice", None, None, None).expect("create decision");
        let decision_id = decision.record.id.clone();
        assert!(
            decision_id.starts_with("dec-"),
            "card-mode decision id: {decision_id}"
        );

        // A feature card whose notes.md points at that decision (structured
        // pointer on a `locked --` line).
        save_feature_card(&paths, "csv-export");
        let notes = paths.cards_dir().join("csv-export").join("notes.md");
        fs::write(
            &notes,
            format!("- {decision_id} locked -- chose the streaming writer\n"),
        )
        .expect("write notes");

        let cards = crate::domain::card::query::scan_with_failures(&paths)
            .expect("walk store")
            .cards;
        assert!(
            dangling_reference_warnings(&paths, &cards).is_empty(),
            "a resolvable card-<hash> pointer must not warn"
        );

        // Repoint at a decision that does not exist: the gate must now fire.
        // Only notes.md changed, so the loaded card set is still current.
        fs::write(&notes, "- card-deadbe locked -- points at nothing\n").expect("rewrite notes");
        let warnings = dangling_reference_warnings(&paths, &cards);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("card-deadbe")),
            "a dangling card-<hash> pointer must warn: {warnings:?}"
        );

        let _ = fs::remove_dir_all(paths.maestro_dir());
    }

    /// Parent-filtering separation: a global decision card (parent `None`) and a
    /// feature-scoped one (parent the feature) co-exist in the one flat store.
    /// `decisions_for_feature` returns ONLY the feature-scoped card while `list`
    /// spans both. (Ported from the deleted decision_card_cutover: no other test
    /// exercises the global-vs-feature split now that the two stores are one.)
    #[test]
    fn decisions_for_feature_returns_only_the_feature_scoped_cards() {
        let paths = card_mode_repo("decisions-by-feature");
        // The real verb writes a feature card whose `extra` carries a parseable
        // FeatureRecord; the bare `save_feature_card` shortcut leaves `extra`
        // empty, which `create_open`'s `feature::ensure_exists` cannot read.
        let feature_id = crate::domain::feature::create(&paths, "Csv export", None)
            .expect("create feature card");

        let global = create_open(&paths, "Use fire-and-forget hooks", None, None, None)
            .expect("create global decision");
        let feature = create_open(
            &paths,
            "Use a replay queue for hooks",
            None,
            Some(&feature_id),
            None,
        )
        .expect("create feature decision");

        let scoped =
            super::decisions_for_feature(&paths, &feature_id).expect("decisions for feature");
        let scoped_ids: Vec<String> = scoped.iter().map(|record| record.id.clone()).collect();
        assert_eq!(
            scoped_ids,
            vec![feature.record.id.clone()],
            "only the feature-scoped card lists under the feature"
        );

        let all = super::list(&paths).expect("list all decisions");
        let mut listed: Vec<String> = all.iter().map(|entry| entry.id.clone()).collect();
        listed.sort();
        let mut expected = vec![global.record.id.clone(), feature.record.id.clone()];
        expected.sort();
        assert_eq!(
            listed, expected,
            "the global list spans both the global and feature-scoped decisions"
        );

        let _ = fs::remove_dir_all(paths.maestro_dir());
    }
}
