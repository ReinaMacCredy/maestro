//! Claim and evidence collection for task verification.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::verify_task::{ClaimCheck, EVENT_PROOF_SOURCE_KIND, EvidenceText};
use crate::domain::run;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::EVENT_SCHEMA_VERSION;

pub(super) fn collect_evidence(
    paths: &MaestroPaths,
    task_dir: &Path,
    task_id: &str,
) -> Result<Vec<EvidenceText>> {
    let mut evidence = Vec::new();
    collect_task_artifact_text(task_dir, "evidence", &mut evidence)?;
    collect_task_artifact_text(task_dir, "proof", &mut evidence)?;
    collect_event_text(paths, task_id, &mut evidence)?;
    Ok(evidence)
}

pub(super) fn check_claims(claims: &[String], evidence: &[EvidenceText]) -> Vec<ClaimCheck> {
    claims
        .iter()
        .map(|claim| {
            let normalized_claim = normalize_claim(claim);
            let source = evidence
                .iter()
                .find(|source| {
                    source
                        .claims
                        .iter()
                        .any(|candidate| normalize_claim(candidate) == normalized_claim)
                })
                .map(|source| source.path.display().to_string());
            ClaimCheck {
                claim: claim.clone(),
                matched: source.is_some(),
                source,
            }
        })
        .collect()
}

fn collect_task_artifact_text(
    task_dir: &Path,
    dirname: &str,
    evidence: &mut Vec<EvidenceText>,
) -> Result<()> {
    let dir = task_dir.join(dirname);
    if !dir.is_dir() {
        return Ok(());
    }

    for path in text_files_under(&dir)? {
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let claims = proof_text_claims(&text);
        evidence.push(EvidenceText {
            kind: dirname.to_string(),
            path,
            text,
            claims,
        });
    }
    Ok(())
}

fn collect_event_text(
    paths: &MaestroPaths,
    task_id: &str,
    evidence: &mut Vec<EvidenceText>,
) -> Result<()> {
    let mut matched_by_path = BTreeMap::<PathBuf, (Vec<String>, Vec<String>)>::new();
    run::visit_managed_events(paths, |record| {
        let event = record.event();
        if event.task_id() == Some(task_id) && is_proof_event(event) {
            let (matched, claims) = matched_by_path
                .entry(record.path().to_path_buf())
                .or_default();
            claims.extend(event_claims(event));
            matched.push(record.raw_line().to_string());
        }
        Ok(())
    })?;
    for (path, (matched, claims)) in matched_by_path {
        evidence.push(EvidenceText {
            kind: EVENT_PROOF_SOURCE_KIND.to_string(),
            path,
            text: matched.join("\n"),
            claims,
        });
    }
    Ok(())
}

fn is_proof_event(event: &run::RunEvent) -> bool {
    matches!(event_kind(event), Some("proof" | "Proof" | "task_proof"))
        || is_phase4_tool_proof_event(event)
}

fn event_kind(event: &run::RunEvent) -> Option<&str> {
    event.alias_kind()
}

fn event_claims(event: &run::RunEvent) -> Vec<String> {
    let mut claims = Vec::new();
    if let Some(claim) = event.claim() {
        claims.push(claim.to_string());
    }
    if let Some(message) = event.message() {
        claims.push(message.to_string());
    }
    claims.extend(event.claims());
    if is_phase4_tool_proof_event(event) {
        claims.extend(phase4_tool_claims(event));
    }
    claims
}

fn is_phase4_tool_proof_event(event: &run::RunEvent) -> bool {
    event.schema_version() == Some(EVENT_SCHEMA_VERSION)
        && event.event_type() == Some("PostToolUse")
        && event.status() == Some("ok")
}

fn phase4_tool_claims(event: &run::RunEvent) -> Vec<String> {
    let mut claims = Vec::new();
    let tool_name = event.tool_name();
    let tool_input_hash = event.tool_input_hash();

    if let (Some(tool_name), Some(tool_input_hash)) = (tool_name, tool_input_hash) {
        claims.push(format!("{tool_name} {tool_input_hash}"));
    }
    claims
}

fn proof_text_claims(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("claim:")
                .or_else(|| trimmed.strip_prefix("Claim:"))
                .map(str::trim)
                .filter(|claim| !claim.is_empty())
                .map(str::to_string)
        })
        .collect()
}

fn normalize_claim(claim: &str) -> String {
    claim.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("failed to inspect {}", path.display()))?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    collect_files(&path, files)?;
                } else if file_type.is_file() {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}
