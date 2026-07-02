use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::{archive_db, store as card_store};
use crate::domain::decisions::cards;
use crate::domain::decisions::query::{normalize_decision_id, not_found};
use crate::domain::decisions::schema::{
    DecisionRecord, DecisionRecordKind, DecisionSetChildSummary, DecisionSetText, DecisionStatus,
};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::slug::slugify_ascii;

const DECISION_SET_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionSetInput {
    #[serde(default)]
    schema_version: Option<u32>,
    title: String,
    #[serde(default)]
    feature: Option<String>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    source_approval: Option<DecisionSetTextInput>,
    #[serde(default)]
    advisor_review: Option<DecisionSetTextInput>,
    children: Vec<DecisionSetChildInput>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionSetChildInput {
    #[serde(default)]
    key: Option<String>,
    title: String,
    #[serde(default)]
    order: Option<u32>,
    #[serde(default)]
    context: Option<String>,
    decision: String,
    #[serde(default)]
    rejected: Vec<String>,
    #[serde(default)]
    preview: Option<String>,
    #[serde(default)]
    supersedes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionSetTextInput {
    summary: String,
    #[serde(default)]
    raw: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecisionSetPlan {
    pub schema_version: u32,
    pub set_id: String,
    pub title: String,
    pub feature: Option<String>,
    pub project: Option<String>,
    pub source_approval: Option<DecisionSetText>,
    pub advisor_review: Option<DecisionSetText>,
    pub input_hash: String,
    pub warnings: Vec<DecisionSetWarning>,
    pub children: Vec<DecisionSetPlannedChild>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecisionSetPlannedChild {
    pub key: String,
    pub title: String,
    pub order: u32,
    pub context: Option<String>,
    pub decision: String,
    pub rejected: Vec<String>,
    pub preview: Option<String>,
    pub supersedes: Vec<String>,
    pub summary: DecisionSetChildSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecisionSetWarning {
    pub code: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompressedSummaryDetection {
    pub signals: Vec<String>,
    pub blocking: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecisionSetWriteReport {
    pub set_record: DecisionRecord,
    pub child_records: Vec<DecisionRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompressedDecisionCandidate {
    pub id: String,
    pub title: String,
    pub status: String,
    pub signals: Vec<String>,
    pub audited_override: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionSetArchiveScope {
    SetOnly,
    IncludeChildren,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecisionSetArchiveReport {
    pub set_id: String,
    pub archived: Vec<String>,
}

pub fn plan_from_yaml(raw: &str) -> Result<DecisionSetPlan> {
    let input: DecisionSetInput =
        serde_yaml::from_str(raw).context("failed to parse DecisionSet YAML")?;
    plan_from_input(input)
}

pub fn draft_from_text(raw: &str) -> Result<DecisionSetPlan> {
    let children: Vec<DecisionSetChildInput> = raw
        .lines()
        .filter_map(|line| clean_optional(Some(line)))
        .map(|line| strip_list_prefix(&line))
        .filter(|line| !line.is_empty())
        .map(|line| DecisionSetChildInput {
            key: None,
            title: line.clone(),
            order: None,
            context: None,
            decision: line,
            rejected: Vec::new(),
            preview: None,
            supersedes: Vec::new(),
        })
        .collect();
    if children.is_empty() {
        bail!("--from-text needs at least one non-empty decision line");
    }

    let mut plan = plan_from_input(DecisionSetInput {
        schema_version: Some(DECISION_SET_SCHEMA_VERSION),
        title: "Inferred decision set".to_string(),
        feature: None,
        project: None,
        source_approval: None,
        advisor_review: None,
        children,
    })?;
    plan.warnings.push(DecisionSetWarning {
        code: "inferred_from_text".to_string(),
        message:
            "Draft was inferred from plain text; review and lock from YAML for durable storage."
                .to_string(),
        blocking: false,
    });
    Ok(plan)
}

pub fn detect_compressed_summary(text: &str) -> Option<CompressedSummaryDetection> {
    let lower = text.to_ascii_lowercase();
    let decision_id_count = text.matches("dec-").count();
    let bullet_count = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed
                    .chars()
                    .next()
                    .is_some_and(|char| char.is_ascii_digit())
                    && trimmed.contains(". ")
        })
        .count();
    let mut signals = Vec::new();

    if lower.contains("locked all")
        || lower.contains("lock all")
        || lower.contains("all remaining recommendation")
    {
        signals.push("lock_all_wording".to_string());
    }
    if decision_id_count >= 2 {
        signals.push("multiple_decision_ids".to_string());
    }
    if bullet_count >= 3 {
        signals.push("multi_item_list".to_string());
    }
    if lower.contains("recommendations") || lower.contains("decisions as design decisions") {
        signals.push("batch_recommendation_wording".to_string());
    }

    let strong_signal = signals
        .iter()
        .any(|signal| signal == "multiple_decision_ids" || signal == "lock_all_wording");
    if signals.len() >= 2 && strong_signal {
        Some(CompressedSummaryDetection {
            signals,
            blocking: true,
        })
    } else {
        None
    }
}

pub fn compressed_summary_candidates(
    paths: &MaestroPaths,
) -> Result<Vec<CompressedDecisionCandidate>> {
    let mut candidates = Vec::new();
    for (record, _, _) in cards::scan(paths, false)? {
        if let Some(candidate) = compressed_candidate_from_record(&record) {
            candidates.push(candidate);
        }
    }
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(candidates)
}

pub fn compressed_summary_candidate(
    paths: &MaestroPaths,
    id: &str,
) -> Result<CompressedDecisionCandidate> {
    let id = normalize_decision_id(id)?;
    let Some((record, _, _)) = cards::load_one(paths, &id)? else {
        return Err(not_found(paths, &id));
    };
    compressed_candidate_from_record(&record).ok_or_else(|| {
        anyhow!(
            "{} is not a compressed summary candidate; run `maestro decision audit --compressed` first",
            record.id
        )
    })
}

pub fn records_from_plan(plan: &DecisionSetPlan, now: &str) -> Vec<DecisionRecord> {
    let children: Vec<DecisionRecord> = plan
        .children
        .iter()
        .map(|child| {
            let child_id = child_id(plan, child);
            DecisionRecord {
                id: child_id,
                title: child.title.clone(),
                status: DecisionStatus::Locked,
                kind: DecisionRecordKind::Individual,
                feature: plan.feature.clone(),
                project: plan.project.clone(),
                context: child.context.clone(),
                decision: Some(child.decision.clone()),
                rejected: child.rejected.clone(),
                preview: child.preview.clone(),
                supersedes: child.supersedes.clone(),
                superseded_by: None,
                decision_set_id: Some(plan.set_id.clone()),
                decision_set_children: Vec::new(),
                source_approval: None,
                advisor_review: None,
                input_hash: None,
                decision_set_schema_version: None,
                summary_override: None,
                created_at: now.to_string(),
                locked_at: Some(now.to_string()),
            }
        })
        .collect();

    let summaries = plan
        .children
        .iter()
        .zip(children.iter())
        .map(|(child, record)| DecisionSetChildSummary {
            key: child.key.clone(),
            title: child.title.clone(),
            order: child.order,
            preview: child.preview.clone(),
            child_decision_id: Some(record.id.clone()),
            locked_at: Some(now.to_string()),
        })
        .collect();

    let set = DecisionRecord {
        id: plan.set_id.clone(),
        title: plan.title.clone(),
        status: DecisionStatus::Locked,
        kind: DecisionRecordKind::DecisionSet,
        feature: plan.feature.clone(),
        project: plan.project.clone(),
        context: None,
        decision: Some(format!(
            "DecisionSet grouping record for {} child decision(s).",
            children.len()
        )),
        rejected: Vec::new(),
        preview: None,
        supersedes: Vec::new(),
        superseded_by: None,
        decision_set_id: None,
        decision_set_children: summaries,
        source_approval: plan.source_approval.clone(),
        advisor_review: plan.advisor_review.clone(),
        input_hash: Some(plan.input_hash.clone()),
        decision_set_schema_version: Some(plan.schema_version),
        summary_override: None,
        created_at: now.to_string(),
        locked_at: Some(now.to_string()),
    };

    let mut records = Vec::with_capacity(children.len() + 1);
    records.push(set);
    records.extend(children);
    records
}

pub fn write_plan_records(
    paths: &MaestroPaths,
    plan: &DecisionSetPlan,
    now: &str,
) -> Result<DecisionSetWriteReport> {
    let records = records_from_plan(plan, now);
    let set_record = records
        .first()
        .expect("invariant: records_from_plan always includes set")
        .clone();
    let child_records = records[1..].to_vec();

    let mut created = Vec::new();
    for record in &records {
        if let Err(error) = cards::create(paths, record, record.project.clone()) {
            rollback_created(paths, &created)?;
            return Err(error);
        }
        created.push(record.id.clone());
    }

    Ok(DecisionSetWriteReport {
        set_record,
        child_records,
    })
}

pub fn repair_compressed_summary(
    paths: &MaestroPaths,
    compressed_id: &str,
    plan: &DecisionSetPlan,
    now: &str,
) -> Result<DecisionSetWriteReport> {
    let compressed_id = normalize_decision_id(compressed_id)?;
    let Some((old_record, _, _)) = cards::load_one(paths, &compressed_id)? else {
        return Err(not_found(paths, &compressed_id));
    };
    if old_record.status != DecisionStatus::Locked {
        bail!(
            "{} is {}; only locked compressed summaries can be repaired",
            old_record.id,
            old_record.status.as_str()
        );
    }
    if compressed_candidate_from_record(&old_record).is_none() {
        bail!(
            "{} is not a compressed summary candidate; run `maestro decision audit --compressed` first",
            old_record.id
        );
    }

    let mut records = records_from_plan(plan, now);
    if records.is_empty() {
        bail!("DecisionSet repair produced no records");
    }
    inherit_repair_scope(&old_record, &mut records);
    records
        .first_mut()
        .expect("invariant: records is non-empty")
        .supersedes
        .push(old_record.id.clone());

    let mut created = Vec::new();
    for record in &records {
        if let Err(error) = cards::create(paths, record, record.project.clone()) {
            rollback_created(paths, &created)?;
            return Err(error);
        }
        created.push(record.id.clone());
    }

    let (mut current_old, _, current_old_resolved) = match cards::load_one(paths, &old_record.id) {
        Ok(Some(loaded)) => loaded,
        Ok(None) => {
            rollback_created(paths, &created)?;
            bail!(
                "decision {} disappeared before repair metadata could be written; replacement records were rolled back",
                old_record.id
            );
        }
        Err(error) => {
            rollback_created(paths, &created)?;
            bail!(
                "decision {} could not be reloaded before repair metadata was written: {error:#}; replacement records were rolled back",
                old_record.id
            );
        }
    };
    if current_old.status != DecisionStatus::Locked {
        rollback_created(paths, &created)?;
        bail!(
            "decision {} changed to {} while repairing it; replacement records were rolled back",
            current_old.id,
            current_old.status.as_str()
        );
    }

    current_old.status = DecisionStatus::Superseded;
    current_old.superseded_by = Some(
        records
            .first()
            .expect("invariant: records still include set")
            .id
            .clone(),
    );
    if let Err(error) = cards::save(&current_old, &current_old_resolved) {
        rollback_created(paths, &created)?;
        bail!(
            "decision {} repair records were created but superseded metadata could not be written: {error:#}; replacement records were rolled back",
            current_old.id
        );
    }

    let set_record = records
        .first()
        .expect("invariant: records still include set")
        .clone();
    Ok(DecisionSetWriteReport {
        set_record,
        child_records: records[1..].to_vec(),
    })
}

pub fn archive_set(
    paths: &MaestroPaths,
    id: &str,
    scope: DecisionSetArchiveScope,
) -> Result<DecisionSetArchiveReport> {
    let id = normalize_decision_id(id)?;
    let Some((record, _, _)) = cards::load_one(paths, &id)? else {
        return Err(not_found(paths, &id));
    };
    if record.kind != DecisionRecordKind::DecisionSet {
        bail!("{id} is a {}, not a DecisionSet", record.kind.as_str());
    }

    let mut ids = vec![record.id.clone()];
    if scope == DecisionSetArchiveScope::IncludeChildren {
        ids.extend(
            record
                .decision_set_children
                .iter()
                .filter_map(|child| child.child_decision_id.clone()),
        );
    }
    ids.sort();
    ids.dedup();

    let targets = load_archive_targets(paths, &ids)?;
    let mut archived = Vec::new();
    for target in &targets {
        if let Err(error) = archive_decision_card(paths, target) {
            if let Err(rollback_error) = rollback_archived_decisions(paths, &archived) {
                return Err(anyhow!(
                    "failed to archive {}: {error:#}; rollback of previously archived DecisionSet records failed: {rollback_error:#}",
                    target.record.id
                ));
            }
            return Err(error).with_context(|| {
                format!(
                    "failed to archive {}; previously archived DecisionSet records were restored",
                    target.record.id
                )
            });
        }
        archived.push(target.record.id.clone());
    }
    Ok(DecisionSetArchiveReport {
        set_id: record.id,
        archived,
    })
}

struct DecisionArchiveTarget {
    record: DecisionRecord,
    resolved: card_store::ResolvedCard,
}

fn load_archive_targets(
    paths: &MaestroPaths,
    ids: &[String],
) -> Result<Vec<DecisionArchiveTarget>> {
    let mut targets = Vec::with_capacity(ids.len());
    for id in ids {
        let Some((record, _, resolved)) = cards::load_one(paths, id)? else {
            return Err(not_found(paths, id));
        };
        targets.push(DecisionArchiveTarget { record, resolved });
    }
    Ok(targets)
}

fn rollback_archived_decisions(paths: &MaestroPaths, archived: &[String]) -> Result<()> {
    archive_db::restore_snapshots(paths, archived)?;
    Ok(())
}

fn rollback_created(paths: &MaestroPaths, ids: &[String]) -> Result<()> {
    for id in ids.iter().rev() {
        if let Some((_, _, resolved)) = cards::load_one(paths, id)? {
            card_store::remove_resolved(&resolved)?;
        }
    }
    Ok(())
}

fn compressed_candidate_from_record(
    record: &DecisionRecord,
) -> Option<CompressedDecisionCandidate> {
    let detection = compressed_detection_for_record(record)?;
    Some(CompressedDecisionCandidate {
        id: record.id.clone(),
        title: record.title.clone(),
        status: record.status.as_str().to_string(),
        signals: detection.signals,
        audited_override: record.summary_override.is_some(),
    })
}

fn compressed_detection_for_record(record: &DecisionRecord) -> Option<CompressedSummaryDetection> {
    let text = record_detection_text(record);
    detect_compressed_summary(&text).or_else(|| {
        record
            .summary_override
            .as_ref()
            .map(|override_record| CompressedSummaryDetection {
                signals: override_record.signals.clone(),
                blocking: false,
            })
    })
}

fn record_detection_text(record: &DecisionRecord) -> String {
    let mut text = String::new();
    if let Some(decision) = record.decision.as_deref() {
        text.push_str(decision);
    }
    for value in &record.rejected {
        text.push('\n');
        text.push_str(value);
    }
    if let Some(preview) = record.preview.as_deref() {
        text.push('\n');
        text.push_str(preview);
    }
    text
}

fn inherit_repair_scope(old_record: &DecisionRecord, records: &mut [DecisionRecord]) {
    for record in records {
        if record.feature.is_none() {
            record.feature = old_record.feature.clone();
        }
        if record.project.is_none() {
            record.project = old_record.project.clone();
        }
    }
}

fn archive_decision_card(paths: &MaestroPaths, target: &DecisionArchiveTarget) -> Result<()> {
    let source_relpath = Path::new(&target.record.id);
    archive_db::archive_virtual_card(
        paths,
        &target.record.id,
        &target.resolved.card,
        source_relpath,
    )?;
    if let Err(error) = card_store::remove_resolved(&target.resolved) {
        let _ = archive_db::delete_snapshots(paths, std::slice::from_ref(&target.record.id));
        return Err(error);
    }
    Ok(())
}

fn plan_from_input(input: DecisionSetInput) -> Result<DecisionSetPlan> {
    let schema_version = input.schema_version.unwrap_or(DECISION_SET_SCHEMA_VERSION);
    if schema_version != DECISION_SET_SCHEMA_VERSION {
        bail!("unsupported DecisionSet schema_version {schema_version}");
    }
    let input_hash = sha256_prefixed(
        &serde_json::to_vec(&input).context("failed to hash normalized DecisionSet input")?,
    );
    let title = required("title", input.title)?;
    if input.children.is_empty() {
        bail!("DecisionSet must include at least one child decision");
    }

    let id_tail = &input_hash["sha256:".len()..][..6];
    let set_id = format!("decset-{}-{id_tail}", slug_or_fallback(&title, "set"));
    let has_explicit_order = input.children.iter().any(|child| child.order.is_some());
    let duplicate_titles = duplicate_titles(&input.children);
    let mut keys = BTreeSet::new();
    let mut orders = BTreeSet::new();
    let mut children = Vec::with_capacity(input.children.len());

    for (index, child) in input.children.into_iter().enumerate() {
        let title = required("child title", child.title)?;
        let decision = required("child decision", child.decision)?;
        if duplicate_titles.contains(&title) && child.key.is_none() {
            bail!(
                "duplicate child title `{title}` requires an explicit unique key on each duplicate"
            );
        }
        let key = match child.key {
            Some(key) => {
                required("child key", key).map(|value| slug_or_fallback(&value, "child"))?
            }
            None => slug_or_fallback(&title, &format!("child-{}", index + 1)),
        };
        if !keys.insert(key.clone()) {
            bail!("duplicate child key `{key}`");
        }

        let order = match (has_explicit_order, child.order) {
            (true, Some(order)) if order > 0 => order,
            (true, Some(_)) => bail!("child order must be greater than zero"),
            (true, None) => bail!("all children need an explicit order when any child uses order"),
            (false, _) => (index + 1) as u32,
        };
        if !orders.insert(order) {
            bail!("duplicate child order {order}");
        }

        if child.rejected.iter().any(|value| value.trim().is_empty()) {
            bail!("child `{key}` has an empty rejected option");
        }
        if child.supersedes.iter().any(|value| value.trim().is_empty()) {
            bail!("child `{key}` has an empty supersedes id");
        }

        children.push(DecisionSetPlannedChild {
            summary: DecisionSetChildSummary {
                key: key.clone(),
                title: title.clone(),
                order,
                preview: clean_optional(child.preview.as_deref()),
                child_decision_id: None,
                locked_at: None,
            },
            key,
            title,
            order,
            context: clean_optional(child.context.as_deref()),
            decision,
            rejected: trim_list(child.rejected),
            preview: clean_optional(child.preview.as_deref()),
            supersedes: trim_list(child.supersedes),
        });
    }

    children.sort_by_key(|child| child.order);

    Ok(DecisionSetPlan {
        schema_version,
        set_id,
        title,
        feature: clean_optional(input.feature.as_deref()),
        project: clean_optional(input.project.as_deref()),
        source_approval: input.source_approval.map(text_from_input).transpose()?,
        advisor_review: input.advisor_review.map(text_from_input).transpose()?,
        input_hash,
        warnings: Vec::new(),
        children,
    })
}

fn child_id(plan: &DecisionSetPlan, child: &DecisionSetPlannedChild) -> String {
    format!(
        "dec-{}-{}",
        slug_or_fallback(&child.key, "child"),
        &plan.input_hash["sha256:".len()..][..4]
    )
}

fn duplicate_titles(children: &[DecisionSetChildInput]) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for child in children {
        let title = child.title.trim();
        if !title.is_empty() {
            *counts.entry(title.to_string()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(title, count)| (count > 1).then_some(title))
        .collect()
}

fn text_from_input(input: DecisionSetTextInput) -> Result<DecisionSetText> {
    Ok(DecisionSetText {
        summary: required("summary", input.summary)?,
        raw: clean_optional(input.raw.as_deref()),
    })
}

fn required(field: &str, value: String) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(value.to_string())
}

fn clean_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trim_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| clean_optional(Some(&value)))
        .collect()
}

fn strip_list_prefix(line: &str) -> String {
    let trimmed = line.trim();
    let bullet_stripped = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .unwrap_or(trimmed);
    let mut chars = bullet_stripped.char_indices();
    let Some((_, first)) = chars.next() else {
        return String::new();
    };
    if !first.is_ascii_alphanumeric() {
        return bullet_stripped.trim().to_string();
    }
    let Some((separator_index, separator)) = chars.next() else {
        return bullet_stripped.trim().to_string();
    };
    if matches!(separator, '.' | ')') {
        return bullet_stripped[separator_index + separator.len_utf8()..]
            .trim()
            .to_string();
    }
    bullet_stripped.trim().to_string()
}

fn slug_or_fallback(value: &str, fallback: &str) -> String {
    let slug = slugify_ascii(value);
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}
