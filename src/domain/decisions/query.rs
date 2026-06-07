use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::decisions::schema::{DecisionRecord, DecisionStore};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{ensure_dir, read_to_string_if_exists};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{Compat, DECISIONS_SCHEMA_VERSION, classify};

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
pub(crate) struct DecisionStorePath {
    pub path: PathBuf,
    pub source: DecisionSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionListEntry {
    pub id: String,
    pub title: String,
    pub status: String,
    pub source: DecisionSource,
    pub path: PathBuf,
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
    for store_path in store_paths(paths)? {
        let store = load_store_at(&store_path.path)?;
        for record in store.decisions {
            entries.push(DecisionListEntry {
                id: record.id,
                title: record.title,
                status: record.status.as_str().to_string(),
                source: store_path.source.clone(),
                path: store_path.path.clone(),
            });
        }
    }
    for legacy in decision_entries(&paths.decisions_dir())? {
        entries.push(DecisionListEntry {
            id: decision_display_id(&legacy.file_name),
            title: decision_title(&legacy.path)?,
            status: "legacy".to_string(),
            source: DecisionSource::Legacy,
            path: legacy.path,
        });
    }
    entries.sort_by(|left, right| {
        decision_sort_key(&left.id)
            .cmp(&decision_sort_key(&right.id))
            .then_with(|| left.title.cmp(&right.title))
    });
    Ok(entries)
}

pub fn list_tolerant(paths: &MaestroPaths) -> Vec<DecisionListEntry> {
    let mut entries = Vec::new();
    for store_path in store_paths(paths).unwrap_or_default() {
        match load_store_at(&store_path.path) {
            Ok(store) => {
                for record in store.decisions {
                    entries.push(DecisionListEntry {
                        id: record.id,
                        title: record.title,
                        status: record.status.as_str().to_string(),
                        source: store_path.source.clone(),
                        path: store_path.path.clone(),
                    });
                }
            }
            Err(error) => entries.push(DecisionListEntry {
                id: store_path
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("decisions.yaml")
                    .to_string(),
                title: format!("unreadable decision store: {error:#}"),
                status: "unreadable".to_string(),
                source: store_path.source,
                path: store_path.path,
            }),
        }
    }
    for legacy in decision_entries(&paths.decisions_dir()).unwrap_or_default() {
        let title = decision_title(&legacy.path).unwrap_or_else(|error| format!("{error:#}"));
        entries.push(DecisionListEntry {
            id: decision_display_id(&legacy.file_name),
            title,
            status: "legacy".to_string(),
            source: DecisionSource::Legacy,
            path: legacy.path,
        });
    }
    entries.sort_by(|left, right| {
        decision_sort_key(&left.id)
            .cmp(&decision_sort_key(&right.id))
            .then_with(|| left.title.cmp(&right.title))
    });
    entries
}

pub fn decisions_for_feature(
    paths: &MaestroPaths,
    feature_id: &str,
) -> Result<Vec<DecisionRecord>> {
    let path = paths.features_dir().join(feature_id).join("decisions.yaml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut records = load_store_at(&path)?.decisions;
    records.sort_by_key(|record| decision_sort_key(&record.id));
    Ok(records)
}

pub fn show(paths: &MaestroPaths, id: &str) -> Result<DecisionContent> {
    let id = normalize_decision_id(id)?;
    let Some(content) = find_decision_content(paths, &id)? else {
        bail!("decision not found: {id}");
    };
    Ok(content)
}

fn find_decision_content(paths: &MaestroPaths, id: &str) -> Result<Option<DecisionContent>> {
    for store_path in store_paths(paths)? {
        let store = load_store_at(&store_path.path)?;
        if let Some(record) = store.decisions.into_iter().find(|record| record.id == id) {
            return Ok(Some(DecisionContent::Structured {
                record: Box::new(record),
                source: store_path.source,
                path: store_path.path,
            }));
        }
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
    for store_path in store_paths(paths)? {
        for record in load_store_at(&store_path.path)?.decisions {
            bodies.push(render_record(&record));
        }
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

pub fn diagnose(paths: &MaestroPaths) -> DecisionDiagnostic {
    let mut structured_count = 0_usize;
    let mut legacy_count = 0_usize;
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for path in store_paths(paths).unwrap_or_else(|error| {
        errors.push(format!("{error:#}"));
        Vec::new()
    }) {
        match load_store_at(&path.path) {
            Ok(store) => structured_count += store.decisions.len(),
            Err(error) => errors.push(format!("{error:#}")),
        }
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

pub fn dangling_reference_warnings(paths: &MaestroPaths) -> Vec<String> {
    let mut warnings = Vec::new();
    let existing = resolvable_decision_ids(paths);
    warn_dangling_note_pointers(paths, &existing, &mut warnings);
    warn_dangling_supersedes(paths, &existing, &mut warnings);
    warnings.sort();
    warnings.dedup();
    warnings
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

pub(crate) fn store_paths(paths: &MaestroPaths) -> Result<Vec<DecisionStorePath>> {
    let mut stores = Vec::new();
    if paths.decisions_file().exists() {
        stores.push(DecisionStorePath {
            path: paths.decisions_file(),
            source: DecisionSource::Global,
        });
    }
    let features_dir = paths.features_dir();
    if features_dir.is_dir() {
        for entry in fs::read_dir(&features_dir)
            .with_context(|| format!("failed to read {}", features_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("failed to read entry in {}", features_dir.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            let Some(feature_id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let path = entry.path().join("decisions.yaml");
            if path.exists() {
                stores.push(DecisionStorePath {
                    path,
                    source: DecisionSource::Feature { feature_id },
                });
            }
        }
    }
    stores.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(stores)
}

pub(crate) fn load_store_at(path: &Path) -> Result<DecisionStore> {
    let Some(contents) = read_to_string_if_exists(path)? else {
        return Ok(DecisionStore::empty());
    };
    let store: DecisionStore = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&store.schema_version, DECISIONS_SCHEMA_VERSION) != Compat::Exact {
        return Err(MaestroError::SchemaMismatch {
            artifact: path.display().to_string(),
            expected: DECISIONS_SCHEMA_VERSION,
            found: store.schema_version,
        }
        .into());
    }
    Ok(store)
}

pub(crate) fn save_store_at(path: &Path, store: &DecisionStore) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let contents = serde_yaml::to_string(store).context("failed to serialize decisions store")?;
    write_string_atomic(path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
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

fn resolvable_decision_ids(paths: &MaestroPaths) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for store_path in store_paths(paths).unwrap_or_default() {
        if let Ok(store) = load_store_at(&store_path.path) {
            for record in store.decisions {
                ids.insert(record.id);
            }
        }
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
    existing: &BTreeSet<String>,
    warnings: &mut Vec<String>,
) {
    let features_dir = paths.features_dir();
    let Ok(entries) = fs::read_dir(&features_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let path = entry.path().join("notes.md");
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

fn warn_dangling_supersedes(
    paths: &MaestroPaths,
    existing: &BTreeSet<String>,
    warnings: &mut Vec<String>,
) {
    for store_path in store_paths(paths).unwrap_or_default() {
        let Ok(store) = load_store_at(&store_path.path) else {
            continue;
        };
        for record in store.decisions {
            for id in record.supersedes {
                let normalized = normalize_decision_id(&id).unwrap_or(id);
                if !existing.contains(&normalized) {
                    warnings.push(format!(
                        "{} has {} superseding missing decision {normalized}; fix: restore the target decision or remove the supersedes entry",
                        store_path.path.display(),
                        record.id
                    ));
                }
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
            if candidate.starts_with("decision-")
                && let Ok(id) = normalize_decision_id(candidate)
            {
                ids.push(id);
                break;
            }
        }
    }
    ids
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
