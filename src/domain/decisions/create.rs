use anyhow::{Context, Result, anyhow, bail};

use crate::domain::card::schema::CardType;
use crate::domain::card::store as card_store;
use crate::domain::decisions::cards;
use crate::domain::decisions::query::{
    DecisionSource, decision_exists, normalize_decision_id, not_found,
};
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus, DecisionStore};
use crate::domain::feature;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionWriteReport {
    pub record: DecisionRecord,
    pub source: DecisionSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionLockReport {
    pub record: DecisionRecord,
    pub source: DecisionSource,
    pub note_line: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct SupersedeInputs<'a> {
    pub title: Option<&'a str>,
    pub decision: &'a str,
    pub reason: &'a str,
    pub rejected: &'a [String],
    pub preview: Option<&'a str>,
}

pub fn empty_store_yaml() -> Result<String> {
    serde_yaml::to_string(&DecisionStore::empty()).context("failed to serialize decisions store")
}

pub fn create_open(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
    project: Option<String>,
) -> Result<DecisionWriteReport> {
    if slugify_ascii(title).is_empty() {
        bail!("decision title must contain at least one ASCII letter or digit");
    }
    let feature = feature.map(str::trim).filter(|value| !value.is_empty());
    create_open_card(paths, title, context, feature, project)
}

fn create_open_card(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
    project: Option<String>,
) -> Result<DecisionWriteReport> {
    if let Some(feature_id) = feature {
        feature::ensure_exists(paths, feature_id)?;
    }
    // Card mode: typed slug id `dec-<slug>-<hex4>` (title + process nonce, SPEC
    // O3'), no reservation marker -- the create-time CAS (D1) guards collisions.
    let id = card_store::mint_card_id(paths, CardType::Decision, title);
    let record = open_record(id, title, context, feature);
    cards::create(paths, &record, project)?;
    Ok(DecisionWriteReport {
        record,
        source: cards::source_from_parent(feature),
    })
}

/// Build an open decision record. Shared so the legacy store push and the card
/// create derive from the same typed record.
fn open_record(
    id: String,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
) -> DecisionRecord {
    DecisionRecord {
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
    }
}

/// Inputs for the lock half of a one-shot open+lock.
#[derive(Clone, Copy, Debug)]
pub struct LockInputs<'a> {
    pub decision: &'a str,
    pub rejected: &'a [String],
    pub preview: Option<&'a str>,
    pub supersedes: &'a [String],
}

/// One-shot open+lock for pre-decided forks. The lock inputs are validated
/// before the card exists so a bad flag cannot strand a half-finished
/// decision; a lock failure after the create (e.g. a missing supersede
/// target) names the opened id and the finishing command.
pub fn create_locked(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
    inputs: LockInputs<'_>,
    project: Option<String>,
) -> Result<DecisionLockReport> {
    if inputs.decision.trim().is_empty() {
        bail!("--decision must not be empty");
    }
    if inputs.rejected.iter().any(|value| value.trim().is_empty()) {
        bail!("--rejected values must not be empty");
    }
    let report = create_open(paths, title, context, feature, project)?;
    let id = report.record.id;
    lock_card(
        paths,
        &id,
        inputs.decision,
        inputs.rejected,
        inputs.preview,
        inputs.supersedes,
    )
    .with_context(|| {
        format!(
            "decision {id} was opened but the lock failed; finish with `maestro decision lock {id}`"
        )
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
    lock_card(paths, &id, decision, rejected, preview, supersedes)
}

pub fn supersede(
    paths: &MaestroPaths,
    old_id: &str,
    inputs: SupersedeInputs<'_>,
) -> Result<DecisionLockReport> {
    if inputs.decision.trim().is_empty() {
        bail!("--decision must not be empty");
    }
    if inputs.reason.trim().is_empty() {
        bail!("--reason must not be empty");
    }
    if inputs.rejected.iter().any(|value| value.trim().is_empty()) {
        bail!("--rejected values must not be empty");
    }
    if let Some(title) = inputs.title
        && title.trim().is_empty()
    {
        bail!("--title must not be empty");
    }

    let old_id = normalize_decision_id(old_id)?;
    let Some((old_record, old_source, old_resolved)) = cards::load_one(paths, &old_id)? else {
        if decision_exists(paths, &old_id)? {
            bail!(
                "{old_id} is a frozen legacy decision; create a new decision from the old ruling and reference it in the reason"
            );
        }
        let Some(resolved) = card_store::resolve(paths, &old_id)? else {
            return Err(not_found(paths, &old_id));
        };
        bail!(
            "{} is a {} card, not a decision; run `maestro decision list` and supersede a locked decision id",
            old_id,
            resolved.card.card_type.as_str()
        );
    };
    if old_record.status == DecisionStatus::Open {
        bail!(
            "{} is open; lock it with `maestro decision lock {}` instead of superseding it",
            old_record.id,
            old_record.id
        );
    }
    if old_record.status == DecisionStatus::Superseded {
        let replacement = old_record
            .superseded_by
            .as_deref()
            .map(|id| format!("; active replacement: {id}"))
            .unwrap_or_default();
        bail!("{} is already superseded{replacement}", old_record.id);
    }
    if old_record
        .supersedes
        .iter()
        .any(|target| target == &old_record.id)
    {
        bail!(
            "{} contains a self-referential supersedes edge",
            old_record.id
        );
    }

    let title = inputs
        .title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&old_record.title);
    let opened = create_open(
        paths,
        title,
        Some(inputs.reason),
        old_record.feature.as_deref(),
        old_resolved.card.project.clone(),
    )
    .with_context(|| {
        format!(
            "decision {} was validated but the replacement create failed; no old decision metadata was changed",
            old_record.id
        )
    })?;
    let Some((mut new_record, _, new_resolved)) = cards::load_one(paths, &opened.record.id)? else {
        bail!(
            "decision {} was opened but could not be reloaded for locking",
            opened.record.id
        );
    };
    apply_lock(
        &mut new_record,
        inputs.decision,
        inputs.rejected,
        inputs.preview,
        std::slice::from_ref(&old_record.id),
        utc_now_timestamp(),
    );
    cards::save(&new_record, &new_resolved)?;

    let (mut current_old, _, current_old_resolved) = match cards::load_one(paths, &old_record.id) {
        Ok(Some(loaded)) => loaded,
        Ok(None) => {
            return Err(rollback_replacement_error(
                paths,
                &new_record.id,
                format!(
                    "decision {} was replaced by {} but disappeared before superseded metadata could be written",
                    old_record.id, new_record.id
                ),
            ));
        }
        Err(error) => {
            return Err(rollback_replacement_error(
                paths,
                &new_record.id,
                format!(
                    "decision {} was replaced by {} but could not be reloaded: {error:#}",
                    old_record.id, new_record.id
                ),
            ));
        }
    };
    if current_old.status != DecisionStatus::Locked {
        return Err(rollback_replacement_error(
            paths,
            &new_record.id,
            format!(
                "decision {} changed to {} while creating replacement {}; re-run against the current decision state",
                current_old.id,
                current_old.status.as_str(),
                new_record.id
            ),
        ));
    }
    current_old.status = DecisionStatus::Superseded;
    current_old.superseded_by = Some(new_record.id.clone());
    if let Err(error) = cards::save(&current_old, &current_old_resolved) {
        return Err(rollback_replacement_error(
            paths,
            &new_record.id,
            format!(
                "decision {} was replaced by {} but superseded metadata could not be written: {error:#}",
                current_old.id, new_record.id
            ),
        ));
    }
    let note_line = note_superseded_feature(paths, &current_old, &new_record)?;
    Ok(DecisionLockReport {
        record: new_record,
        source: old_source,
        note_line,
    })
}

fn rollback_replacement_error(
    paths: &MaestroPaths,
    replacement_id: &str,
    failure: String,
) -> anyhow::Error {
    let rollback = card_store::resolve(paths, replacement_id).and_then(|resolved| {
        if let Some(resolved) = resolved {
            card_store::remove_resolved(&resolved)
        } else {
            Ok(())
        }
    });
    match rollback {
        Ok(()) => anyhow!("{failure}; replacement {replacement_id} was rolled back"),
        Err(error) => {
            anyhow!("{failure}; rollback of replacement {replacement_id} failed: {error:#}")
        }
    }
}

fn lock_card(
    paths: &MaestroPaths,
    id: &str,
    decision: &str,
    rejected: &[String],
    preview: Option<&str>,
    supersedes: &[String],
) -> Result<DecisionLockReport> {
    let Some((mut record, source, resolved)) = cards::load_one(paths, id)? else {
        // The card lookup already failed, so a hit here is a frozen legacy
        // markdown decision (the migration never folds markdown). Same guard the
        // legacy path gives, reached via the cards-union `decision_exists`.
        if decision_exists(paths, id)? {
            bail!("{id} is a frozen legacy decision; create a new decision that supersedes it");
        }
        return Err(not_found(paths, id));
    };
    if record.status != DecisionStatus::Open {
        bail!(
            "{} is already {}; create a new decision to supersede it",
            record.id,
            record.status.as_str()
        );
    }

    let supersedes = validate_supersedes(paths, id, supersedes)?;
    apply_lock(
        &mut record,
        decision,
        rejected,
        preview,
        &supersedes,
        utc_now_timestamp(),
    );
    cards::save(&record, &resolved)?;

    for target in &supersedes {
        mark_superseded(paths, target, &record.id)?;
    }

    let note_line = note_locked_feature(paths, &record)?;
    Ok(DecisionLockReport {
        record,
        source,
        note_line,
    })
}

/// Normalize the supersede targets, reject a self-reference, and require each to
/// resolve (the cards-union `ensure_decision_exists` covers decision cards and
/// frozen legacy markdown alike).
fn validate_supersedes(
    paths: &MaestroPaths,
    id: &str,
    supersedes: &[String],
) -> Result<Vec<String>> {
    let supersedes = supersedes
        .iter()
        .map(|value| normalize_decision_id(value))
        .collect::<Result<Vec<_>>>()?;
    if supersedes.iter().any(|target| target == id) {
        bail!("{id} cannot supersede itself");
    }
    for target in &supersedes {
        ensure_decision_exists(paths, target)?;
    }
    Ok(supersedes)
}

fn note_superseded_feature(
    paths: &MaestroPaths,
    old_record: &DecisionRecord,
    new_record: &DecisionRecord,
) -> Result<Option<String>> {
    let Some(feature_id) = old_record.feature.as_deref() else {
        return Ok(None);
    };
    Ok(Some(
        feature::note(
            paths,
            feature_id,
            &format!(
                "{} supersedes {} -- {}",
                new_record.id, old_record.id, new_record.title
            ),
        )?
        .line,
    ))
}

/// Stamp a record locked: decision text, rejected alternatives, preview, the
/// resolved supersede targets, and the lock timestamp.
fn apply_lock(
    record: &mut DecisionRecord,
    decision: &str,
    rejected: &[String],
    preview: Option<&str>,
    supersedes: &[String],
    now: String,
) {
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
    record.supersedes = supersedes.to_vec();
    record.locked_at = Some(now);
}

/// Append the feature note for a locked decision, returning the note line. The
/// feature dir stays `features/<id>/` in both modes, so the note rides the
/// legacy feature tree regardless of the decision store.
fn note_locked_feature(paths: &MaestroPaths, record: &DecisionRecord) -> Result<Option<String>> {
    let Some(feature_id) = record.feature.as_deref() else {
        return Ok(None);
    };
    Ok(Some(
        feature::note(
            paths,
            feature_id,
            &format!("{} locked -- {}", record.id, record.title),
        )?
        .line,
    ))
}

fn ensure_decision_exists(paths: &MaestroPaths, id: &str) -> Result<()> {
    if decision_exists(paths, id)? {
        Ok(())
    } else {
        Err(not_found(paths, id))
    }
}

fn mark_superseded(paths: &MaestroPaths, id: &str, by: &str) -> Result<()> {
    if let Some((mut record, _source, resolved)) = cards::load_one(paths, id)? {
        record.status = DecisionStatus::Superseded;
        record.superseded_by = Some(by.to_string());
        cards::save(&record, &resolved)?;
    }
    Ok(())
}
