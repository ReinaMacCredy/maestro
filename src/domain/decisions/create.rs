use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::domain::decisions::query::{
    DecisionSource, decision_entries, decision_exists, load_store_at, load_store_with_snapshot,
    normalize_decision_id, parse_decision_number, save_store_at, save_store_with_snapshot,
    store_paths,
};
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus, DecisionStore};
use crate::domain::feature;
use crate::foundation::core::fs::{
    ALLOC_MARKER_PREFIX, DirReservation, child_dirs, try_reserve_marker_dir,
};
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
    let mut snapshot = load_store_with_snapshot(&path)?;
    let reserved = reserve_next_decision_id(paths)?;
    let id = reserved.id.clone();
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
    snapshot.store.decisions.push(record.clone());
    save_store_with_snapshot(&path, &snapshot.store, &snapshot)?;
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
        let snapshot = load_store_with_snapshot(&path.path)?;
        if let Some(index) = snapshot
            .store
            .decisions
            .iter()
            .position(|record| record.id == id)
        {
            found = Some((path, snapshot, index));
            break;
        }
    }
    let Some((store_path, mut snapshot, index)) = found else {
        if decision_exists(paths, &id)? {
            bail!("{id} is a frozen legacy decision; create a new decision that supersedes it");
        }
        bail!("decision not found: {id}");
    };
    if snapshot.store.decisions[index].status != DecisionStatus::Open {
        bail!(
            "{} is already {}; create a new decision to supersede it",
            snapshot.store.decisions[index].id,
            snapshot.store.decisions[index].status.as_str()
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
        let record = &mut snapshot.store.decisions[index];
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
    let record = snapshot.store.decisions[index].clone();
    save_store_with_snapshot(&store_path.path, &snapshot.store, &snapshot)?;

    for target in &supersedes {
        mark_superseded(paths, target, &record.id)?;
    }

    let note_line = if let Some(feature_id) = record.feature.as_deref() {
        Some(
            feature::note(
                paths,
                feature_id,
                &format!("{} locked -- {}", record.id, record.title),
            )?
            .line,
        )
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
        feature::ensure_exists(paths, feature_id)?;
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

#[derive(Debug)]
struct ReservedDecisionId {
    id: String,
    _marker: DirReservation,
}

fn reserve_next_decision_id(paths: &MaestroPaths) -> Result<ReservedDecisionId> {
    let mut candidate = max_decision_number(paths)? + 1;
    loop {
        let marker_name = format!("{ALLOC_MARKER_PREFIX}decision-{candidate:03}");
        let Some(marker) = try_reserve_marker_dir(paths.decisions_dir(), &marker_name)? else {
            candidate += 1;
            continue;
        };
        let id = format!("decision-{candidate:03}");
        if decision_exists(paths, &id)? {
            drop(marker);
            candidate += 1;
            continue;
        }
        return Ok(ReservedDecisionId {
            id,
            _marker: marker,
        });
    }
}

fn max_decision_number(paths: &MaestroPaths) -> Result<u32> {
    let mut max_number = 0_u32;
    for store_path in store_paths(paths)? {
        for record in load_store_at(&store_path.path)?.decisions {
            if let Some(number) = parse_decision_number(&record.id) {
                max_number = max_number.max(number);
            }
        }
    }
    for entry in decision_entries(&paths.decisions_dir())? {
        if let Some(number) = parse_decision_number(&entry.file_name) {
            max_number = max_number.max(number);
        }
    }
    for (path, _) in child_dirs(&paths.decisions_dir())? {
        if let Some(number) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(ALLOC_MARKER_PREFIX))
            .and_then(|rest| rest.strip_prefix("decision-"))
            .and_then(|value| value.parse::<u32>().ok())
        {
            max_number = max_number.max(number);
        }
    }
    Ok(max_number)
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
        let mut snapshot = load_store_with_snapshot(&path.path)?;
        if let Some(record) = snapshot
            .store
            .decisions
            .iter_mut()
            .find(|record| record.id == id)
        {
            record.status = DecisionStatus::Superseded;
            record.superseded_by = Some(by.to_string());
            save_store_with_snapshot(&path.path, &snapshot.store, &snapshot)?;
            return Ok(());
        }
    }
    Ok(())
}
