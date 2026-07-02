use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::search::query::ParsedQuery;
use crate::domain::search::types::{
    GrepEnvelope, SearchCorpus, SearchDiagnostic, TranscriptRedactionMetadata,
};
use crate::foundation::core::fs::{append_text_file, ensure_dir};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::safe_write::write_string_atomic;

pub const TRANSCRIPT_HOME_ENV: &str = "MAESTRO_TRANSCRIPT_HOME";
const SEGMENT_SCHEMA_VERSION: &str = "maestro.transcript.segment.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptProvider {
    Codex,
    Claude,
    Factory,
}

impl TranscriptProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Factory => "factory",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptConsentScope {
    Project,
    Global,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptConsentRecord {
    pub provider: TranscriptProvider,
    pub workspace: String,
    pub scope: TranscriptConsentScope,
    pub granted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptSegmentInput {
    pub provider: TranscriptProvider,
    pub session_id: String,
    pub segment_id: String,
    pub source_kind: String,
    pub workspace: String,
    pub text: String,
    pub raw_tool_arguments: Option<String>,
    pub raw_tool_output: Option<String>,
    pub raw_reasoning: Option<String>,
    pub raw_environment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptStoredSegment {
    pub schema_version: String,
    pub provider: TranscriptProvider,
    pub session_id: String,
    pub segment_id: String,
    pub source_kind: String,
    pub workspace: String,
    pub authority: String,
    pub proof_eligible: bool,
    pub redacted_text: String,
    pub redacted_text_hash: String,
    pub redaction: TranscriptRedactionMetadata,
    pub excluded_fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactorySessionDescriptor {
    pub provider: TranscriptProvider,
    pub session_id: String,
    pub session_path: Option<String>,
    pub directory_path: Option<String>,
    pub workspace: Option<String>,
    pub mission_id: Option<String>,
    pub session_type: Option<String>,
    pub title: Option<String>,
    pub message_count: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryMissionDescriptor {
    pub provider: TranscriptProvider,
    pub mission_id: String,
    pub base_session_id: Option<String>,
    pub workspace: Option<String>,
    pub worker_session_ids: Vec<String>,
    pub state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptStore {
    root: PathBuf,
}

impl TranscriptStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn consent_file(&self) -> PathBuf {
        self.root.join("consent.json")
    }

    pub fn segment_file(&self, provider: TranscriptProvider, session_id: &str) -> PathBuf {
        self.root
            .join("segments")
            .join(provider.as_str())
            .join(format!("{}.jsonl", safe_component(session_id)))
    }

    pub fn consent_records(&self) -> Result<Vec<TranscriptConsentRecord>> {
        match std::fs::read_to_string(self.consent_file()) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    Ok(Vec::new())
                } else {
                    serde_json::from_str(&contents).with_context(|| {
                        format!("failed to parse {}", self.consent_file().display())
                    })
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to read {}", self.consent_file().display())),
        }
    }

    pub fn grant_consent(
        &self,
        record: TranscriptConsentRecord,
    ) -> Result<TranscriptConsentRecord> {
        ensure_dir(&self.root)?;
        let mut records = self.consent_records()?;
        records.retain(|existing| {
            !(existing.provider == record.provider && existing.workspace == record.workspace)
        });
        records.push(record.clone());
        records.sort_by(|left, right| {
            left.provider
                .as_str()
                .cmp(right.provider.as_str())
                .then_with(|| left.workspace.cmp(&right.workspace))
        });
        write_string_atomic(
            self.consent_file(),
            &serde_json::to_string_pretty(&records)?,
        )?;
        Ok(record)
    }

    pub fn has_consent(&self, provider: TranscriptProvider, workspace: &str) -> bool {
        self.consent_records().is_ok_and(|records| {
            records.iter().any(|record| {
                record.provider == provider && record.workspace == workspace && record.granted
            })
        })
    }

    pub fn append_redacted_segment(
        &self,
        input: TranscriptSegmentInput,
    ) -> Result<TranscriptStoredSegment> {
        if !self.has_consent(input.provider, &input.workspace) {
            bail!(
                "transcript consent missing for {} workspace {}",
                input.provider.as_str(),
                input.workspace
            );
        }

        let mut excluded_fields = Vec::new();
        note_raw_exclusion(
            &mut excluded_fields,
            "raw_tool_arguments",
            &input.raw_tool_arguments,
        );
        note_raw_exclusion(
            &mut excluded_fields,
            "raw_tool_output",
            &input.raw_tool_output,
        );
        note_raw_exclusion(&mut excluded_fields, "raw_reasoning", &input.raw_reasoning);
        note_raw_exclusion(
            &mut excluded_fields,
            "raw_environment",
            &input.raw_environment,
        );

        let (redacted_text, mut redaction_exclusions) = redact_text(&input.text);
        redaction_exclusions.extend(excluded_fields.iter().cloned());
        redaction_exclusions.sort();
        redaction_exclusions.dedup();

        let stored = TranscriptStoredSegment {
            schema_version: SEGMENT_SCHEMA_VERSION.to_string(),
            provider: input.provider,
            session_id: input.session_id,
            segment_id: input.segment_id,
            source_kind: input.source_kind,
            workspace: input.workspace,
            authority: "transcript_context".to_string(),
            proof_eligible: false,
            redacted_text_hash: sha256_prefixed(redacted_text.as_bytes()),
            redacted_text,
            redaction: TranscriptRedactionMetadata {
                state: "redacted".to_string(),
                excluded: redaction_exclusions,
            },
            excluded_fields,
        };
        let line = format!("{}\n", serde_json::to_string(&stored)?);
        append_text_file(
            self.segment_file(stored.provider, &stored.session_id),
            "",
            &line,
        )?;
        Ok(stored)
    }
}

impl Default for TranscriptConsentRecord {
    fn default() -> Self {
        Self {
            provider: TranscriptProvider::Codex,
            workspace: String::new(),
            scope: TranscriptConsentScope::Project,
            granted: false,
            reason: None,
        }
    }
}

pub fn resolve_transcript_home(
    env_override: Option<&Path>,
    user_home: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = env_override.filter(|path| !path.as_os_str().is_empty()) {
        return Some(path.to_path_buf());
    }
    user_home.map(|home| home.join(".maestro/transcripts"))
}

pub fn global_transcript_home() -> Option<PathBuf> {
    let env_override = std::env::var_os(TRANSCRIPT_HOME_ENV).map(PathBuf::from);
    let user_home = std::env::var_os("HOME").map(PathBuf::from);
    resolve_transcript_home(env_override.as_deref(), user_home.as_deref())
}

pub fn parse_claude_transcript_jsonl(
    contents: &str,
    session_id: &str,
    workspace: &str,
) -> Result<Vec<TranscriptSegmentInput>> {
    let mut segments = Vec::new();
    for (idx, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .with_context(|| format!("failed to parse Claude transcript line {}", idx + 1))?;
        let Some(record_type) = value.get("type").and_then(Value::as_str) else {
            continue;
        };
        let segment_id = value
            .get("uuid")
            .or_else(|| value.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{session_id}:{}", idx + 1));
        match record_type {
            "user" | "assistant" => {
                let Some(text) = json_text(value.get("content")) else {
                    continue;
                };
                segments.push(TranscriptSegmentInput {
                    provider: TranscriptProvider::Claude,
                    session_id: session_id.to_string(),
                    segment_id,
                    source_kind: format!("claude_{record_type}_message"),
                    workspace: workspace.to_string(),
                    text,
                    raw_tool_arguments: None,
                    raw_tool_output: None,
                    raw_reasoning: None,
                    raw_environment: None,
                });
            }
            "tool_use" | "tool_result" => {
                let tool_name = value
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                segments.push(TranscriptSegmentInput {
                    provider: TranscriptProvider::Claude,
                    session_id: session_id.to_string(),
                    segment_id,
                    source_kind: format!("claude_{record_type}"),
                    workspace: workspace.to_string(),
                    text: format!("{}: {tool_name}", record_type.replace('_', " ")),
                    raw_tool_arguments: value.get("tool_input").map(Value::to_string),
                    raw_tool_output: value.get("tool_output").map(json_value_string),
                    raw_reasoning: None,
                    raw_environment: None,
                });
            }
            _ => {}
        }
    }
    Ok(segments)
}

pub fn parse_factory_discovery_index(contents: &str) -> Result<Vec<FactorySessionDescriptor>> {
    let value: Value =
        serde_json::from_str(contents).context("failed to parse Factory discovery index")?;
    let entries = value
        .get("entries")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("Factory discovery index missing entries object"))?;
    let mut parsed = Vec::new();
    let sorted: BTreeMap<_, _> = entries.iter().collect();
    for (key, entry) in sorted {
        parsed.push(FactorySessionDescriptor {
            provider: TranscriptProvider::Factory,
            session_id: string_field(entry, "id").unwrap_or_else(|| key.to_string()),
            session_path: string_field(entry, "sessionPath"),
            directory_path: string_field(entry, "directoryPath"),
            workspace: string_field(entry, "cwd"),
            mission_id: string_field(entry, "decompMissionId"),
            session_type: string_field(entry, "decompSessionType"),
            title: string_field(entry, "sessionTitle").or_else(|| string_field(entry, "title")),
            message_count: entry.get("messageCount").and_then(Value::as_u64),
        });
    }
    Ok(parsed)
}

pub fn parse_factory_mission_state(contents: &str) -> Result<FactoryMissionDescriptor> {
    let value: Value =
        serde_json::from_str(contents).context("failed to parse Factory mission state")?;
    let mission_id = string_field(&value, "missionId")
        .ok_or_else(|| anyhow::anyhow!("Factory mission state missing missionId"))?;
    let worker_session_ids = value
        .get("workerSessionIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    Ok(FactoryMissionDescriptor {
        provider: TranscriptProvider::Factory,
        mission_id,
        base_session_id: string_field(&value, "baseSessionId"),
        workspace: string_field(&value, "workingDirectory"),
        worker_session_ids,
        state: string_field(&value, "state"),
    })
}

pub(crate) fn unavailable_diagnostic() -> SearchDiagnostic {
    SearchDiagnostic::error(
        "transcript_corpus_unavailable",
        "transcript corpus is not configured; enable provider/project consent and run `maestro index rebuild --transcript`",
    )
    .with_corpus(SearchCorpus::Transcript)
    .with_path(".maestro/index/transcripts")
    .with_retryable(false)
}

pub(crate) fn unavailable_envelope(raw_query: &str, parsed: &ParsedQuery) -> GrepEnvelope {
    let mut envelope = GrepEnvelope::error_with_overrides(
        raw_query,
        unavailable_diagnostic(),
        parsed.explicit_filter_overrides.clone(),
    );
    envelope.intent = Some("transcript".to_string());
    envelope.intent_confidence = Some("high".to_string());
    envelope.intent_reasons = vec!["explicit transcript corpus filter".to_string()];
    envelope
}

pub(crate) fn attach_unavailable(mut envelope: GrepEnvelope) -> GrepEnvelope {
    if envelope.ok {
        envelope.partial = true;
        envelope.diagnostics.push(unavailable_diagnostic());
    }
    envelope
}

fn note_raw_exclusion(excluded_fields: &mut Vec<String>, field: &str, value: &Option<String>) {
    if value.is_some() {
        excluded_fields.push(field.to_string());
    }
}

fn json_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => non_empty(text),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(text) => non_empty(text),
                    Value::Object(_) => {
                        item.get("text").and_then(Value::as_str).and_then(non_empty)
                    }
                    _ => None,
                })
                .collect();
            non_empty(&parts.join("\n"))
        }
        Value::Object(_) => value?
            .get("text")
            .and_then(Value::as_str)
            .and_then(non_empty),
        _ => None,
    }
}

fn json_value_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).and_then(non_empty)
}

fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn redact_text(text: &str) -> (String, Vec<String>) {
    let mut redacted = text.to_string();
    let mut exclusions = Vec::new();
    for (code, pattern, replacement) in [
        (
            "openai_key",
            r"\bsk-[A-Za-z0-9][A-Za-z0-9_-]{6,}\b",
            "[REDACTED]",
        ),
        (
            "secret_assignment",
            r#"(?i)\b(password|token|api[_-]?key|secret)\s*[:=]\s*[^\s"']+"#,
            "$1=[REDACTED]",
        ),
    ] {
        let regex = Regex::new(pattern).expect("invariant: transcript redaction regex compiles");
        if regex.is_match(&redacted) {
            redacted = regex.replace_all(&redacted, replacement).into_owned();
            exclusions.push(code.to_string());
        }
    }
    (redacted, exclusions)
}

fn safe_component(value: &str) -> String {
    let mut component = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            component.push(ch);
        } else {
            component.push('_');
        }
    }
    if component.is_empty() {
        "unknown".to_string()
    } else {
        component
    }
}
