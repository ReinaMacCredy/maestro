use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::decisions::query::{
    DecisionSource, decision_exists, load_store_at, normalize_decision_id, save_store_at,
    store_paths,
};
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus, DecisionStore};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionWriteReport {
    pub record: DecisionRecord,
    pub path: PathBuf,
    pub source: DecisionSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionLockReport {
    pub record: DecisionRecord,
    pub path: PathBuf,
    pub source: DecisionSource,
    pub note_line: Option<String>,
}

pub fn empty_store_yaml() -> Result<String> {
    serde_yaml::to_string(&DecisionStore::empty()).context("failed to serialize decisions store")
}

pub fn create_open(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
) -> Result<DecisionWriteReport> {
    if slugify_ascii(title).is_empty() {
        bail!("decision title must contain at least one ASCII letter or digit");
    }
    let feature = feature.map(str::trim).filter(|value| !value.is_empty());
    let (path, source) = decision_store_target(paths, feature)?;
    let mut store = load_store_at(&path)?;
    let id = format!("decision-{:03}", next_decision_number(paths)?);
    let record = DecisionRecord {
        id,
        title: title.trim().to_string(),
        status: DecisionStatus::Open,
        feature: feature.map(str::to_string),
        context: context
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        decision: None,
        rejected: Vec::new(),
        preview: None,
        supersedes: Vec::new(),
        superseded_by: None,
        created_at: utc_now_timestamp(),
        locked_at: None,
    };
    store.decisions.push(record.clone());
    save_store_at(&path, &store)?;
    Ok(DecisionWriteReport {
        record,
        path,
        source,
    })
}

pub fn lock(
    paths: &MaestroPaths,
    id: &str,
    decision: &str,
    rejected: &[String],
    preview: Option<&str>,
    supersedes: &[String],
) -> Result<DecisionLockReport> {
    if decision.trim().is_empty() {
        bail!("--decision must not be empty");
    }
    if rejected.iter().any(|value| value.trim().is_empty()) {
        bail!("--rejected values must not be empty");
    }
    let id = normalize_decision_id(id)?;
    let mut found = None;
    for path in store_paths(paths)? {
        let store = load_store_at(&path.path)?;
        if let Some(index) = store.decisions.iter().position(|record| record.id == id) {
            found = Some((path, store, index));
            break;
        }
    }
    let Some((store_path, mut store, index)) = found else {
        if decision_exists(paths, &id)? {
            bail!("{id} is a frozen legacy decision; create a new decision that supersedes it");
        }
        bail!("decision not found: {id}");
    };
    if store.decisions[index].status != DecisionStatus::Open {
        bail!(
            "{} is already {}; create a new decision to supersede it",
            store.decisions[index].id,
            store.decisions[index].status.as_str()
        );
    }

    let supersedes = supersedes
        .iter()
        .map(|value| normalize_decision_id(value))
        .collect::<Result<Vec<_>>>()?;
    if supersedes.iter().any(|target| target == &id) {
        bail!("{id} cannot supersede itself");
    }
    for target in &supersedes {
        ensure_decision_exists(paths, target)?;
    }

    let now = utc_now_timestamp();
    {
        let record = &mut store.decisions[index];
        record.status = DecisionStatus::Locked;
        record.decision = Some(decision.trim().to_string());
        record.rejected = rejected
            .iter()
            .map(|value| value.trim().to_string())
            .collect();
        record.preview = preview
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        record.supersedes = supersedes.clone();
        record.locked_at = Some(now);
    }
    let record = store.decisions[index].clone();
    save_store_at(&store_path.path, &store)?;

    for target in &supersedes {
        mark_superseded(paths, target, &record.id)?;
    }

    let note_line = if let Some(feature_id) = record.feature.as_deref() {
        Some(append_feature_note(
            paths,
            feature_id,
            &format!("{} locked -- {}", record.id, record.title),
        )?)
    } else {
        None
    };

    Ok(DecisionLockReport {
        record,
        path: store_path.path,
        source: store_path.source,
        note_line,
    })
}

pub(crate) fn ensure_feature_store(paths: &MaestroPaths, feature_id: &str) -> Result<()> {
    let path = paths.features_dir().join(feature_id).join("decisions.yaml");
    if path.exists() {
        return Ok(());
    }
    save_store_at(&path, &DecisionStore::empty())
}

fn decision_store_target(
    paths: &MaestroPaths,
    feature: Option<&str>,
) -> Result<(PathBuf, DecisionSource)> {
    if let Some(feature_id) = feature {
        validate_component("feature id", feature_id)?;
        let feature_yaml = paths.features_dir().join(feature_id).join("feature.yaml");
        if !feature_yaml.is_file() {
            bail!("feature not found: {feature_id}");
        }
        let path = paths.features_dir().join(feature_id).join("decisions.yaml");
        if !path.exists() {
            ensure_feature_store(paths, feature_id)?;
        }
        Ok((
            path,
            DecisionSource::Feature {
                feature_id: feature_id.to_string(),
            },
        ))
    } else {
        let path = paths.decisions_file();
        if !path.exists() {
            save_store_at(&path, &DecisionStore::empty())?;
        }
        Ok((path, DecisionSource::Global))
    }
}

fn next_decision_number(paths: &MaestroPaths) -> Result<u32> {
    let mut max_number = 0_u32;
    for entry in crate::domain::decisions::query::list(paths)? {
        if let Some(number) = crate::domain::decisions::query::parse_decision_number(&entry.id) {
            max_number = max_number.max(number);
        }
    }
    Ok(max_number + 1)
}

fn ensure_decision_exists(paths: &MaestroPaths, id: &str) -> Result<()> {
    if decision_exists(paths, id)? {
        Ok(())
    } else {
        bail!("decision not found: {id}")
    }
}

fn mark_superseded(paths: &MaestroPaths, id: &str, by: &str) -> Result<()> {
    for path in store_paths(paths)? {
        let mut store = load_store_at(&path.path)?;
        if let Some(record) = store.decisions.iter_mut().find(|record| record.id == id) {
            record.status = DecisionStatus::Superseded;
            record.superseded_by = Some(by.to_string());
            save_store_at(&path.path, &store)?;
            return Ok(());
        }
    }
    Ok(())
}

fn append_feature_note(paths: &MaestroPaths, feature_id: &str, text: &str) -> Result<String> {
    let feature_yaml = paths.features_dir().join(feature_id).join("feature.yaml");
    let raw = std::fs::read_to_string(&feature_yaml)
        .with_context(|| format!("failed to read {}", feature_yaml.display()))?;
    let feature: FeatureTitle = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", feature_yaml.display()))?;
    let notes = paths.features_dir().join(feature_id).join("notes.md");
    append_note_file(&notes, &feature.title, text)
}

fn append_note_file(path: &Path, title: &str, text: &str) -> Result<String> {
    let existing = crate::foundation::core::fs::read_to_string_if_exists(path)?;
    let mut contents = existing.unwrap_or_else(|| format!("# {title}\n\n"));
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    let date = utc_now_timestamp()
        .split_once('T')
        .map(|(date, _)| date.to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());
    let line = format!("{date}  {}", text.trim());
    contents.push_str(&line);
    contents.push('\n');
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    crate::foundation::core::safe_write::write_string_atomic(path, &contents)
        .with_context(|| format!("failed to append feature note {}", path.display()))?;
    Ok(line)
}

fn validate_component(label: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    let mut components = path.components();
    if value.is_empty()
        || !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid {label}: {value}");
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct FeatureTitle {
    title: String,
}
