use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, Result};

use crate::domain::harness::schema::{
    BacklogConfig, BacklogItem, EscalationPolicy, HistoryEntry, is_state_detector,
};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::write_string_if_unchanged;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{BACKLOG_SCHEMA_VERSION, Compat, classify};
use crate::foundation::core::time::utc_now_timestamp;

/// Load the Harness backlog, returning an empty V1 backlog when it does not exist.
pub fn load(paths: &MaestroPaths) -> Result<BacklogConfig> {
    Ok(load_with_snapshot(paths)?.backlog)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BacklogSnapshot {
    pub backlog: BacklogConfig,
    raw: Option<String>,
}

/// Load the Harness backlog with the exact bytes used for optimistic save.
pub(crate) fn load_with_snapshot(paths: &MaestroPaths) -> Result<BacklogSnapshot> {
    let path = backlog_path(paths)?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => Some(raw),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let backlog: BacklogConfig = match raw.as_deref() {
        Some(raw) => serde_yaml::from_str(raw)
            .with_context(|| format!("failed to parse {}", path.display()))?,
        None => BacklogConfig::empty(),
    };
    validate_schema(&path, &backlog)?;
    Ok(BacklogSnapshot { backlog, raw })
}

/// Persist a Harness backlog through the managed Harness path policy.
pub fn save(paths: &MaestroPaths, backlog: &BacklogConfig) -> Result<()> {
    let path = backlog_path(paths)?;
    validate_schema(&path, backlog)?;
    let raw = serde_yaml::to_string(backlog).context("failed to serialize backlog")?;
    write_string_atomic(&path, &raw).with_context(|| format!("failed to write {}", path.display()))
}

/// Persist a Harness backlog only if the store still matches the loaded snapshot.
pub(crate) fn save_with_snapshot(
    paths: &MaestroPaths,
    backlog: &BacklogConfig,
    snapshot: &BacklogSnapshot,
) -> Result<()> {
    let path = backlog_path(paths)?;
    validate_schema(&path, backlog)?;
    let raw = serde_yaml::to_string(backlog).context("failed to serialize backlog")?;
    write_string_if_unchanged(&path, snapshot.raw.as_deref(), &raw)
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Refresh proposals into the Harness backlog without applying them.
pub fn refresh(paths: &MaestroPaths, proposals: Vec<BacklogItem>) -> Result<BacklogConfig> {
    let mut snapshot = load_with_snapshot(paths)?;
    merge_proposals(&mut snapshot.backlog, proposals);
    save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok(snapshot.backlog)
}

/// Merge proposals into the backlog keyed on stable fingerprint and assign
/// deterministic ids. Re-detecting a terminal `measured` state note reopens it
/// (D6); a `proposed` note with no durable history that is no longer detected
/// is reconciled away (D4).
pub fn merge_proposals(backlog: &mut BacklogConfig, proposals: Vec<BacklogItem>) {
    merge_proposals_inner(backlog, proposals, true);
}

/// Merge agent-authored proposals without reconciling away detector-authored
/// ephemeral items that are absent from this single manual proposal call.
pub fn merge_proposals_preserving_absent(backlog: &mut BacklogConfig, proposals: Vec<BacklogItem>) {
    merge_proposals_inner(backlog, proposals, false);
}

fn merge_proposals_inner(
    backlog: &mut BacklogConfig,
    mut proposals: Vec<BacklogItem>,
    reconcile_absent_ephemeral: bool,
) {
    sanitize_existing_generated_evidence(backlog);
    let fresh_fingerprints = proposals
        .iter()
        .map(|proposal| proposal.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut fingerprints = backlog
        .items
        .iter()
        .map(|item| item.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut next = next_backlog_number(&backlog.items);
    proposals.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));

    for mut proposal in proposals {
        normalize_recurrence(&mut proposal);
        let fingerprint = proposal.fingerprint.clone();
        if !fingerprint.is_empty() && fingerprints.contains(&fingerprint) {
            if let Some(existing) = backlog
                .items
                .iter_mut()
                .find(|item| item.fingerprint == fingerprint && item.status != "dismissed")
            {
                reopen_if_regressed(existing);
                refresh_existing_recurrence(existing, &proposal);
                refresh_existing_evidence(existing, &proposal);
            }
            continue;
        }
        proposal.id = format!("hb-{next:03}");
        next += 1;
        fingerprints.insert(fingerprint);
        backlog.items.push(proposal);
    }

    if reconcile_absent_ephemeral {
        backlog
            .items
            .retain(|item| !is_ephemeral_reconcilable(item, &fresh_fingerprints));
    }
}

/// D6: a re-detected, terminal `measured` state note flips back to `proposed`
/// and logs the regression. Behavioral notes are kept as-is.
fn reopen_if_regressed(existing: &mut BacklogItem) {
    if existing.status == "measured" && is_state_detector(&existing.item_type) {
        existing.status = "proposed".to_string();
        existing.history.push(HistoryEntry {
            result: "regressed".to_string(),
            task: existing.spawned_task.clone(),
            note: None,
            at: utc_now_timestamp(),
        });
        // Drop the link so the next accept spawns a fresh task (impl-default (c)),
        // mirroring the D2 ineffective path. The old task stays in history.
        existing.spawned_task = None;
    }
}

/// D4: drop a `proposed` note with no durable history that the current
/// detection run no longer produces. Durable = a spawned task or any history.
fn is_ephemeral_reconcilable(item: &BacklogItem, fresh_fingerprints: &BTreeSet<String>) -> bool {
    item.status == "proposed"
        && item.spawned_task.is_none()
        && item.history.is_empty()
        && (item.provenance.is_empty() || item.provenance == "detector")
        && !fresh_fingerprints.contains(&item.fingerprint)
}

/// Derive stored priority from the active escalation policy.
pub fn apply_escalation_policy(backlog: &mut BacklogConfig, policy: &EscalationPolicy) {
    for item in &mut backlog.items {
        item.priority = policy.priority_for(item.sessions_hit.len(), &item.priority);
    }
}

fn normalize_recurrence(item: &mut BacklogItem) {
    if item.sessions_hit.is_empty() && !item.source.is_empty() {
        item.sessions_hit.push(item.source.clone());
    }
    item.sessions_hit.sort();
    item.sessions_hit.dedup();
    if item.occurrences == 0 {
        item.occurrences = item.sessions_hit.len();
    }
}

fn refresh_existing_recurrence(existing: &mut BacklogItem, proposal: &BacklogItem) {
    if existing.first_seen.is_empty() {
        existing.first_seen = proposal.first_seen.clone();
    }
    existing.last_seen = proposal.last_seen.clone();
    existing.occurrences = proposal.occurrences;
    existing.sessions_hit = proposal.sessions_hit.clone();
    normalize_recurrence(existing);
}

fn refresh_existing_evidence(existing: &mut BacklogItem, proposal: &BacklogItem) {
    if !matches!(
        existing.item_type.as_str(),
        "missing_verification" | "explicit_intervention" | "agent_audit"
    ) {
        return;
    }

    if existing.item_type != "missing_verification" {
        for evidence in &proposal.evidence {
            if !existing.evidence.contains(evidence) {
                existing.evidence.push(evidence.clone());
            }
        }
        return;
    }

    let mut refreshed = existing
        .evidence
        .iter()
        .filter(|evidence| !is_generated_missing_verification_evidence(evidence))
        .cloned()
        .collect::<Vec<_>>();
    for (index, evidence) in proposal.evidence.iter().enumerate() {
        let evidence = sanitize_missing_verification_evidence(evidence, index);
        if !refreshed.contains(&evidence) {
            refreshed.push(evidence);
        }
    }
    existing.evidence = refreshed;
}

fn sanitize_existing_generated_evidence(backlog: &mut BacklogConfig) {
    for item in &mut backlog.items {
        if item.item_type == "missing_verification" {
            let mut generated_index = 0;
            item.evidence = item
                .evidence
                .iter()
                .map(|evidence| {
                    if is_generated_missing_verification_evidence(evidence) {
                        let sanitized =
                            sanitize_missing_verification_evidence(evidence, generated_index);
                        generated_index += 1;
                        sanitized
                    } else {
                        evidence.to_string()
                    }
                })
                .collect();
        }
    }
}

fn is_generated_missing_verification_evidence(evidence: &str) -> bool {
    is_safe_missing_verification_evidence(evidence)
        || is_legacy_generated_missing_verification_evidence(evidence)
}

fn sanitize_missing_verification_evidence(evidence: &str, index: usize) -> String {
    if let Some(source) = safe_missing_verification_source(evidence) {
        return format!(
            "{} used verification command {} outside harness.yml",
            source,
            index + 1
        );
    }
    let Some((source, detail)) = evidence.split_once(" used ") else {
        return evidence.to_string();
    };
    if !detail.ends_with(" outside harness.yml") {
        return evidence.to_string();
    }
    let source = safe_verification_source(source);
    if source == "verification evidence" {
        return evidence.to_string();
    }
    format!(
        "{} used verification command {} outside harness.yml",
        source,
        index + 1
    )
}

fn is_safe_missing_verification_evidence(evidence: &str) -> bool {
    safe_missing_verification_source(evidence).is_some()
}

fn safe_missing_verification_source(evidence: &str) -> Option<&str> {
    let (source, command) = evidence.split_once(" used ")?;
    let label = command
        .strip_prefix("verification command ")
        .and_then(|label| label.strip_suffix(" outside harness.yml"))?;
    if safe_verification_source(source) == source.trim() && label.parse::<usize>().is_ok() {
        Some(source.trim())
    } else {
        None
    }
}

fn is_legacy_generated_missing_verification_evidence(evidence: &str) -> bool {
    let Some((source, detail)) = evidence.split_once(" used ") else {
        return false;
    };
    detail.ends_with(" outside harness.yml")
        && safe_verification_source(source) != "verification evidence"
}

fn safe_verification_source(source: &str) -> &'static str {
    let source = source.trim();
    if source == "task.yaml#verification" {
        return "task.yaml#verification";
    }
    if source == "verification.json" {
        return "verification.json";
    }
    if source == "verification.attempts/latest.json" {
        return "verification.attempts/latest.json";
    }
    if source.starts_with("verification.attempts/") {
        return "verification.attempts/archived attempt";
    }
    "verification evidence"
}

fn backlog_path(paths: &MaestroPaths) -> Result<std::path::PathBuf> {
    managed_path(
        paths,
        ".maestro/harness/backlog.yaml",
        SymlinkPolicy::RejectAllComponents,
    )
}

fn validate_schema(path: &std::path::Path, backlog: &BacklogConfig) -> Result<()> {
    if classify(&backlog.schema_version, BACKLOG_SCHEMA_VERSION) != Compat::Exact {
        // Use the typed error (same Display text) so a backlog schema mismatch
        // emits the `fix: run maestro doctor` hint like feature/decision stores,
        // rather than a bare bail with no remedy.
        return Err(MaestroError::SchemaMismatch {
            artifact: path.display().to_string(),
            expected: BACKLOG_SCHEMA_VERSION,
            found: backlog.schema_version.clone(),
        }
        .into());
    }
    Ok(())
}

fn next_backlog_number(items: &[BacklogItem]) -> u32 {
    items
        .iter()
        .filter_map(|item| item.id.strip_prefix("hb-"))
        .filter_map(|number| number.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::domain::harness::backlog;
    use crate::domain::harness::schema::BacklogItem;
    use crate::foundation::core::paths::MaestroPaths;

    fn temp_paths(name: &str) -> (PathBuf, MaestroPaths) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-{name}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        fs::create_dir_all(paths.harness_dir())
            .expect("invariant: harness dir should be creatable");
        (root, paths)
    }

    fn item(id: &str, title: &str) -> BacklogItem {
        BacklogItem {
            id: id.to_string(),
            fingerprint: id.to_string(),
            source: id.to_string(),
            provenance: "test".to_string(),
            topic: id.to_string(),
            item_type: "agent_audit".to_string(),
            title: title.to_string(),
            priority: "medium".to_string(),
            occurrences: 1,
            sessions_hit: vec![id.to_string()],
            first_seen: String::new(),
            last_seen: String::new(),
            status: "proposed".to_string(),
            evidence: vec![title.to_string()],
            spawned_task: None,
            dismissal_reason: None,
            history: Vec::new(),
        }
    }

    #[test]
    fn save_with_snapshot_rejects_stale_backlog_writer() {
        let (_root, paths) = temp_paths("backlog-stale-writer");
        let mut first = backlog::load_with_snapshot(&paths)
            .expect("invariant: first backlog load should succeed");
        let mut second = backlog::load_with_snapshot(&paths)
            .expect("invariant: second backlog load should succeed");

        second.backlog.items.push(item("hb-001", "second writer"));
        backlog::save_with_snapshot(&paths, &second.backlog, &second)
            .expect("invariant: second writer should save first");

        first.backlog.items.push(item("hb-002", "stale writer"));
        let error = backlog::save_with_snapshot(&paths, &first.backlog, &first)
            .expect_err("stale writer must be rejected");
        assert!(
            error.to_string().contains("failed to write")
                && format!("{error:#}").contains("changed since it was read; re-run"),
            "{error:#}"
        );
    }
}
