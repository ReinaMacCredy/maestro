use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::domain::card::store as card_store;
use crate::domain::decisions::cards;
use crate::domain::decisions::query::{DecisionSource, decision_exists, normalize_decision_id};
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus, DecisionStore};
use crate::domain::feature;
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
    create_open_card(paths, title, context, feature)
}

fn create_open_card(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
) -> Result<DecisionWriteReport> {
    if let Some(feature_id) = feature {
        feature::ensure_exists(paths, feature_id)?;
    }
    // Card mode: content-addressed `card-<hash>` id (title + process nonce, SPEC
    // O3'), no reservation marker -- the create-time CAS (D1) guards collisions.
    let id = card_store::mint_card_id(paths, title);
    let record = open_record(id, title, context, feature);
    let home = cards::create(paths, &record)?;
    Ok(DecisionWriteReport {
        record,
        path: home.path().to_path_buf(),
        source: cards::source_from_parent(feature),
    })
}

/// Build an open decision record. Shared so the legacy store push and the card
/// create mint byte-identical records.
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
        bail!("decision not found: {id}");
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
        path: resolved.path().to_path_buf(),
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
        bail!("decision not found: {id}")
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
