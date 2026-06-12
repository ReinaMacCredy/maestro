use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use crate::domain::feature::qa;
use crate::domain::feature::registry;
use crate::domain::feature::schema::{
    AcceptanceEvidenceEntry, AcceptanceEvidenceKind, AcceptanceSweepRun, FeatureRecord,
    normalize_acceptance_id,
};
use crate::domain::task::{self, TaskEntry, TaskRecord, TaskState, VerificationStatus};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::{timestamp_nanos, utc_now_timestamp};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceCoverage {
    pub ac_id: String,
    pub text: String,
    pub tasks: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureVerifyReport {
    pub feature_id: String,
    pub recorded: Option<String>,
    pub sweep: Option<AcceptanceSweepReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceSweepReport {
    pub at: String,
    pub invalidated_by: Vec<String>,
    pub items: Vec<AcceptanceSweepItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceSweepItem {
    pub ac_id: String,
    pub text: String,
    pub proof: AcceptanceProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceProof {
    Task(Vec<String>),
    Qa(Vec<String>),
    Explicit(String),
    Waived(String),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeatureProofUpdate {
    Explicit { ac_id: String, evidence: String },
    Waive { ac_id: String, reason: String },
}

pub fn acceptance_id(index: usize) -> String {
    format!("ac-{}", index + 1)
}

pub fn acceptance_coverage(
    paths: &MaestroPaths,
    feature_id: &str,
) -> Result<Vec<AcceptanceCoverage>> {
    let record = registry::load_record(paths, feature_id)?;
    acceptance_coverage_for_record(paths, &record)
}

pub fn acceptance_coverage_archived(
    paths: &MaestroPaths,
    feature_id: &str,
) -> Result<Vec<AcceptanceCoverage>> {
    let record = registry::load_archived_record(paths, feature_id)?;
    let task_entries = task::load_archived_task_entries(paths)?;
    Ok(acceptance_coverage_for_record_in_entries(
        &record,
        &task_entries,
    ))
}

pub fn uncovered_acceptance(paths: &MaestroPaths, feature_id: &str) -> Result<Vec<String>> {
    Ok(acceptance_coverage(paths, feature_id)?
        .into_iter()
        .filter(|item| item.tasks.is_empty())
        .map(|item| item.ac_id)
        .collect())
}

pub fn verify_feature(
    paths: &MaestroPaths,
    feature_id: &str,
    updates: Vec<FeatureProofUpdate>,
) -> Result<FeatureVerifyReport> {
    let (mut record, write) = registry::load_record_for_update(paths, feature_id)?;
    if !updates.is_empty() {
        let mut entries = Vec::new();
        let mut recorded = Vec::new();
        let at = utc_now_timestamp();
        for update in updates {
            let (kind, ac_id, text) = match update {
                FeatureProofUpdate::Explicit { ac_id, evidence } => (
                    AcceptanceEvidenceKind::Explicit,
                    ac_id,
                    evidence.trim().to_string(),
                ),
                FeatureProofUpdate::Waive { ac_id, reason } => (
                    AcceptanceEvidenceKind::Waived,
                    ac_id,
                    reason.trim().to_string(),
                ),
            };
            let ac_id = normalize_existing_acceptance_id(&record, &ac_id)?;
            if text.is_empty() {
                bail!("feature acceptance evidence must not be empty");
            }
            let kind_label = kind.as_str();
            recorded.push(format!("{kind_label} {ac_id} ({} bytes)", text.len()));
            entries.push(AcceptanceEvidenceEntry {
                ac_id,
                kind,
                text,
                at: at.clone(),
            });
        }
        record.acceptance_evidence.extend(entries);
        registry::save_record(paths, &record, &write)?;
        return Ok(FeatureVerifyReport {
            feature_id: record.id,
            recorded: Some(recorded.join("; ")),
            sweep: None,
        });
    }

    let previous = record.acceptance_sweeps.last().map(|run| run.at.clone());
    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    let report = sweep_acceptance(paths, &record, previous.as_deref(), &task_entries)?;
    let resolved = report
        .items
        .iter()
        .filter(|item| !matches!(item.proof, AcceptanceProof::Missing))
        .map(|item| item.ac_id.clone())
        .collect::<Vec<_>>();
    let unresolved = report
        .items
        .iter()
        .filter(|item| matches!(item.proof, AcceptanceProof::Missing))
        .map(|item| item.ac_id.clone())
        .collect::<Vec<_>>();
    record.acceptance_sweeps.push(AcceptanceSweepRun {
        at: report.at.clone(),
        resolved,
        unresolved,
        invalidated_by: report.invalidated_by.clone(),
    });
    registry::save_record(paths, &record, &write)?;
    Ok(FeatureVerifyReport {
        feature_id: record.id,
        recorded: None,
        sweep: Some(report),
    })
}

pub(crate) fn acceptance_ship_gap(
    record: &FeatureRecord,
    task_entries: &[TaskEntry],
) -> Result<Option<String>> {
    if record.acceptance.is_empty() {
        return Ok(None);
    }
    let Some(latest) = record.acceptance_sweeps.last() else {
        return Ok(Some(format!(
            "contract sweep missing — {} acceptance item(s) need feature-level evidence\n    fix: maestro feature verify {}\n    retry: maestro feature ship {} --outcome \"<outcome>\"",
            record.acceptance.len(),
            record.id,
            record.id
        )));
    };
    let invalidated_by = invalidations_since(record, &latest.at, task_entries);
    if !invalidated_by.is_empty() {
        return Ok(Some(format!(
            "contract sweep stale — {}\n    fix: maestro feature verify {}\n    retry: maestro feature ship {} --outcome \"<outcome>\"",
            invalidated_by.join("; "),
            record.id,
            record.id
        )));
    }
    if !latest.unresolved.is_empty() {
        return Ok(Some(format!(
            "contract sweep incomplete — {} acceptance item(s) unresolved: {}\n    fix: maestro feature verify {} then add proof with `maestro feature verify {} --prove <ac-id> --evidence \"<observed>\"` or waive with `--waive <ac-id> --reason \"<why>\"`\n    retry: maestro feature ship {} --outcome \"<outcome>\"",
            latest.unresolved.len(),
            latest.unresolved.join(", "),
            record.id,
            record.id,
            record.id
        )));
    }
    Ok(None)
}

fn sweep_acceptance(
    paths: &MaestroPaths,
    record: &FeatureRecord,
    previous_sweep_at: Option<&str>,
    task_entries: &[TaskEntry],
) -> Result<AcceptanceSweepReport> {
    let explicit = latest_explicit_evidence(record);
    let task_proofs = task_proofs_by_acceptance_in_entries(task_entries, &record.id);
    let qa_proofs = qa::acceptance_ids_covered_by_counting_slices(&registry::feature_sidecar_dir(
        paths, &record.id,
    ))?;
    let mut items = Vec::new();
    for (index, text) in record.acceptance.iter().enumerate() {
        let ac_id = acceptance_id(index);
        let proof = if let Some(entry) = explicit.get(&ac_id) {
            match entry.kind {
                AcceptanceEvidenceKind::Explicit => AcceptanceProof::Explicit(entry.text.clone()),
                AcceptanceEvidenceKind::Waived => AcceptanceProof::Waived(entry.text.clone()),
            }
        } else if let Some(tasks) = task_proofs.get(&ac_id) {
            AcceptanceProof::Task(tasks.clone())
        } else if qa_proofs.contains(&ac_id) {
            AcceptanceProof::Qa(vec!["qa.md counting slice".to_string()])
        } else {
            AcceptanceProof::Missing
        };
        items.push(AcceptanceSweepItem {
            ac_id,
            text: text.clone(),
            proof,
        });
    }
    Ok(AcceptanceSweepReport {
        at: utc_now_timestamp(),
        invalidated_by: previous_sweep_at
            .map(|at| invalidations_since(record, at, task_entries))
            .unwrap_or_default(),
        items,
    })
}

fn acceptance_coverage_for_record(
    paths: &MaestroPaths,
    record: &FeatureRecord,
) -> Result<Vec<AcceptanceCoverage>> {
    acceptance_coverage_for_record_in_task_root(record, &paths.tasks_dir())
}

fn acceptance_coverage_for_record_in_task_root(
    record: &FeatureRecord,
    tasks_dir: &Path,
) -> Result<Vec<AcceptanceCoverage>> {
    let task_entries = task::load_task_entries(tasks_dir)?;
    Ok(acceptance_coverage_for_record_in_entries(
        record,
        &task_entries,
    ))
}

pub(crate) fn acceptance_coverage_for_record_in_entries(
    record: &FeatureRecord,
    task_entries: &[TaskEntry],
) -> Vec<AcceptanceCoverage> {
    let tasks = task_proofs_and_links_by_acceptance_in_entries(task_entries, &record.id, false);
    record
        .acceptance
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let ac_id = acceptance_id(index);
            AcceptanceCoverage {
                tasks: tasks.get(&ac_id).cloned().unwrap_or_default(),
                ac_id,
                text: text.clone(),
            }
        })
        .collect()
}

fn normalize_existing_acceptance_id(record: &FeatureRecord, value: &str) -> Result<String> {
    let Some(ac_id) = normalize_acceptance_id(value) else {
        bail!("invalid acceptance id `{value}`; expected ac-1, ac-2, ...");
    };
    let known = record
        .acceptance
        .iter()
        .enumerate()
        .map(|(index, _)| acceptance_id(index))
        .collect::<BTreeSet<_>>();
    if !known.contains(&ac_id) {
        bail!(
            "unknown acceptance id `{ac_id}` for feature {}; known ids: {}",
            record.id,
            known.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(ac_id)
}

fn latest_explicit_evidence(record: &FeatureRecord) -> BTreeMap<String, &AcceptanceEvidenceEntry> {
    let mut entries = BTreeMap::new();
    for entry in record.acceptance_evidence.iter().rev() {
        entries.entry(entry.ac_id.clone()).or_insert(entry);
    }
    entries
}

fn task_proofs_by_acceptance_in_entries(
    task_entries: &[TaskEntry],
    feature_id: &str,
) -> BTreeMap<String, Vec<String>> {
    task_proofs_and_links_by_acceptance_in_entries(task_entries, feature_id, true)
}

fn task_proofs_and_links_by_acceptance_in_entries(
    task_entries: &[TaskEntry],
    feature_id: &str,
    require_verified: bool,
) -> BTreeMap<String, Vec<String>> {
    let mut by_acceptance = BTreeMap::<String, Vec<String>>::new();
    for entry in task_entries {
        let task = &entry.task;
        if task.feature_id.as_deref() != Some(feature_id) {
            continue;
        }
        if require_verified && !is_verified_task(task) {
            continue;
        }
        for cover in &task.covers {
            if let Some(ac_id) = normalize_acceptance_id(cover) {
                by_acceptance
                    .entry(ac_id)
                    .or_default()
                    .push(task.id.clone());
            }
        }
    }
    for tasks in by_acceptance.values_mut() {
        tasks.sort();
        tasks.dedup();
    }
    by_acceptance
}

fn is_verified_task(task: &TaskRecord) -> bool {
    task.state == TaskState::Verified
        && task.verification.status == Some(VerificationStatus::Passed)
}

fn invalidations_since(
    record: &FeatureRecord,
    since: &str,
    task_entries: &[TaskEntry],
) -> Vec<String> {
    let Some(since) = timestamp_nanos(since) else {
        return vec!["prior sweep timestamp could not be parsed".to_string()];
    };
    let mut invalidations = Vec::new();
    for amend in record.amends.iter().filter(|entry| entry.is_behavioral()) {
        if timestamp_nanos(&amend.at).is_some_and(|at| at > since) {
            invalidations.push(format!("behavioral amend at {}", amend.at));
        }
    }
    for entry in task_entries {
        let task = &entry.task;
        if task.feature_id.as_deref() != Some(&record.id) {
            continue;
        }
        if task.state.is_live() {
            continue;
        }
        if timestamp_nanos(&task.updated_at).is_some_and(|at| at > since) {
            invalidations.push(format!("{} settled at {}", task.id, task.updated_at));
        }
    }
    invalidations
}
