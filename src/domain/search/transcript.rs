use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::search::intent;
use crate::domain::search::query::ParsedQuery;
use crate::domain::search::types::{
    GrepEnvelope, MatchSpan, ScoreReason, SearchCorpus, SearchDiagnostic, SearchFreshness,
    SearchHit, TranscriptRedactionMetadata,
};
use crate::foundation::core::fs::{append_text_file, ensure_dir};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;

pub const TRANSCRIPT_HOME_ENV: &str = "MAESTRO_TRANSCRIPT_HOME";
const SEGMENT_SCHEMA_VERSION: &str = "maestro.transcript.segment.v1";
const TRANSCRIPT_VIEW_SCHEMA_VERSION: &str = "maestro.transcript-view.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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
pub struct CodexTranscript {
    pub provider: TranscriptProvider,
    pub session_id: String,
    pub cwd: Option<String>,
    pub workspace_roots: Vec<String>,
    pub segments: Vec<TranscriptSegmentInput>,
}

impl CodexTranscript {
    pub fn project_match_reasons(
        &self,
        current_workspace: &str,
        explicit_session: Option<&str>,
        scope_global: bool,
    ) -> Vec<String> {
        if scope_global {
            return vec!["scope_global".to_string()];
        }
        if explicit_session.is_some_and(|session| session == self.session_id) {
            return vec!["explicit_session".to_string()];
        }
        let mut reasons = Vec::new();
        if self
            .workspace_roots
            .iter()
            .any(|root| paths_overlap(root, current_workspace))
        {
            reasons.push("workspace_root".to_string());
        }
        if self
            .cwd
            .as_deref()
            .is_some_and(|cwd| paths_overlap(cwd, current_workspace))
        {
            reasons.push("cwd".to_string());
        }
        reasons.sort();
        reasons.dedup();
        reasons
    }

    pub fn visible_in_project_by_default(&self, current_workspace: &str) -> bool {
        !self
            .project_match_reasons(current_workspace, None, false)
            .is_empty()
    }
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

#[derive(Debug, Serialize)]
pub struct TranscriptRebuildReport {
    pub consent_records: usize,
    pub sessions: usize,
    pub segments: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptIndexHealth {
    pub configured: bool,
    pub consent_records: usize,
    pub manifest_present: bool,
    pub sessions: usize,
    pub segments: usize,
}

#[derive(Clone, Debug)]
struct TranscriptLoad {
    consent_records: Vec<TranscriptConsentRecord>,
    segments: Vec<TranscriptStoredSegment>,
}

#[derive(Clone, Debug)]
struct TranscriptMatch {
    byte_start: usize,
    byte_end: usize,
    snippet: String,
    factor: &'static str,
}

#[derive(Debug, Deserialize, Serialize)]
struct TranscriptViewManifest {
    schema_version: String,
    sessions: Vec<TranscriptViewSession>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TranscriptViewSession {
    provider: TranscriptProvider,
    session_id: String,
    workspace: String,
    segments: Vec<TranscriptViewSegment>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TranscriptViewSegment {
    segment_id: String,
    source_kind: String,
    authority: String,
    proof_eligible: bool,
    redacted_text_hash: String,
    redaction: TranscriptRedactionMetadata,
    project_match_reasons: Vec<String>,
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

pub(crate) fn rebuild_transcript_unlocked(paths: &MaestroPaths) -> Result<TranscriptRebuildReport> {
    let load = load_configured_store()
        .map_err(|diagnostic| anyhow::anyhow!("{}: {}", diagnostic.code, diagnostic.message))?;
    let target_workspace = paths.repo_root().display().to_string();
    let mut sessions: BTreeMap<(TranscriptProvider, String, String), Vec<TranscriptViewSegment>> =
        BTreeMap::new();

    for segment in consented_segments(&load) {
        let reasons = project_match_reasons(&segment, &target_workspace, None, false);
        if reasons.is_empty() {
            continue;
        }
        sessions
            .entry((
                segment.provider,
                segment.session_id.clone(),
                segment.workspace.clone(),
            ))
            .or_default()
            .push(TranscriptViewSegment {
                segment_id: segment.segment_id.clone(),
                source_kind: segment.source_kind.clone(),
                authority: segment.authority.clone(),
                proof_eligible: segment.proof_eligible,
                redacted_text_hash: segment.redacted_text_hash.clone(),
                redaction: segment.redaction.clone(),
                project_match_reasons: reasons,
            });
    }

    let sessions = sessions
        .into_iter()
        .map(
            |((provider, session_id, workspace), mut segments)| TranscriptViewSession {
                provider,
                session_id,
                workspace,
                segments: {
                    segments.sort_by(|left, right| left.segment_id.cmp(&right.segment_id));
                    segments
                },
            },
        )
        .collect::<Vec<_>>();
    let report = TranscriptRebuildReport {
        consent_records: load.consent_records.len(),
        sessions: sessions.len(),
        segments: sessions.iter().map(|session| session.segments.len()).sum(),
    };
    let manifest = TranscriptViewManifest {
        schema_version: TRANSCRIPT_VIEW_SCHEMA_VERSION.to_string(),
        sessions,
    };
    ensure_dir(paths.transcript_index_dir())?;
    write_string_atomic(
        paths.transcript_view_manifest_file(),
        &serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(report)
}

pub fn transcript_index_health(paths: &MaestroPaths) -> TranscriptIndexHealth {
    let configured = load_configured_store().ok();
    let consent_records = configured
        .as_ref()
        .map_or(0, |load| load.consent_records.len());
    match std::fs::read_to_string(paths.transcript_view_manifest_file())
        .ok()
        .and_then(|contents| serde_json::from_str::<TranscriptViewManifest>(&contents).ok())
    {
        Some(manifest) if manifest.schema_version == TRANSCRIPT_VIEW_SCHEMA_VERSION => {
            TranscriptIndexHealth {
                configured: configured.is_some(),
                consent_records,
                manifest_present: true,
                sessions: manifest.sessions.len(),
                segments: manifest
                    .sessions
                    .iter()
                    .map(|session| session.segments.len())
                    .sum(),
            }
        }
        _ => TranscriptIndexHealth {
            configured: configured.is_some(),
            consent_records,
            manifest_present: false,
            sessions: 0,
            segments: 0,
        },
    }
}

pub(crate) fn grep_transcript_parsed(
    paths: &MaestroPaths,
    raw_query: &str,
    parsed: &ParsedQuery,
) -> GrepEnvelope {
    let load = match load_configured_store() {
        Ok(load) => load,
        Err(diagnostic) => return unavailable_envelope_with(raw_query, parsed, diagnostic),
    };
    let hits = match search_segments(paths, &load, parsed) {
        Ok(hits) => hits,
        Err(diagnostic) => {
            return GrepEnvelope::error_with_overrides(
                raw_query,
                diagnostic,
                parsed.explicit_filter_overrides.clone(),
            );
        }
    };
    GrepEnvelope::success(raw_query, hits, parsed.explicit_filter_overrides.clone())
        .with_freshness(vec![transcript_freshness(&load, false)])
}

pub(crate) fn attach_results(
    paths: &MaestroPaths,
    raw_query: &str,
    parsed: &ParsedQuery,
    mut envelope: GrepEnvelope,
) -> GrepEnvelope {
    if !envelope.ok {
        return envelope;
    }

    let transcript = grep_transcript_parsed(paths, raw_query, parsed);
    if transcript.ok {
        envelope.hits.extend(transcript.hits);
        envelope.freshness.extend(transcript.freshness);
        envelope.diagnostics.extend(transcript.diagnostics);
        sort_and_rank_hits(&mut envelope.hits);
        return envelope;
    }

    envelope.partial = true;
    envelope.diagnostics.extend(transcript.diagnostics);
    envelope
}

pub fn parse_codex_transcript_jsonl(
    contents: &str,
    fallback_session_id: &str,
) -> Result<CodexTranscript> {
    let mut transcript = CodexTranscript {
        provider: TranscriptProvider::Codex,
        session_id: fallback_session_id.to_string(),
        cwd: None,
        workspace_roots: Vec::new(),
        segments: Vec::new(),
    };
    for (idx, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .with_context(|| format!("failed to parse Codex transcript line {}", idx + 1))?;
        match value.get("type").and_then(Value::as_str) {
            Some("session_meta") => apply_codex_session_meta(&mut transcript, &value),
            Some("response_item") => {
                if let Some(segment) = codex_response_segment(&transcript, &value, idx + 1) {
                    transcript.segments.push(segment);
                }
            }
            _ => {}
        }
    }
    Ok(transcript)
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
    unavailable_envelope_with(raw_query, parsed, unavailable_diagnostic())
}

fn unavailable_envelope_with(
    raw_query: &str,
    parsed: &ParsedQuery,
    diagnostic: SearchDiagnostic,
) -> GrepEnvelope {
    let mut envelope = GrepEnvelope::error_with_overrides(
        raw_query,
        diagnostic,
        parsed.explicit_filter_overrides.clone(),
    );
    envelope.intent = Some("transcript".to_string());
    envelope.intent_confidence = Some("high".to_string());
    envelope.intent_reasons = vec!["explicit transcript corpus filter".to_string()];
    envelope
}

fn load_configured_store() -> Result<TranscriptLoad, SearchDiagnostic> {
    let Some(home) = global_transcript_home() else {
        return Err(unavailable_diagnostic());
    };
    if !home.is_dir() {
        return Err(unavailable_diagnostic());
    }
    let store = TranscriptStore::new(&home);
    let consent_records = store
        .consent_records()
        .map_err(|error| transcript_store_diagnostic(format!("{error:#}")))?;
    let consent_records = consent_records
        .into_iter()
        .filter(|record| record.granted)
        .collect::<Vec<_>>();
    if consent_records.is_empty() {
        return Err(unavailable_diagnostic());
    }
    let segments = load_stored_segments(&home)
        .map_err(|error| transcript_store_diagnostic(format!("{error:#}")))?;
    Ok(TranscriptLoad {
        consent_records,
        segments,
    })
}

fn load_stored_segments(root: &Path) -> Result<Vec<TranscriptStoredSegment>> {
    let segments_dir = root.join("segments");
    if !segments_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    for provider in [
        TranscriptProvider::Codex,
        TranscriptProvider::Claude,
        TranscriptProvider::Factory,
    ] {
        let provider_dir = segments_dir.join(provider.as_str());
        if !provider_dir.is_dir() {
            continue;
        }
        let mut files = std::fs::read_dir(&provider_dir)
            .with_context(|| format!("failed to read {}", provider_dir.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("failed to read {}", provider_dir.display()))?;
        files.sort_by_key(|entry| entry.path());
        for entry in files {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            for (idx, line) in contents.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let segment: TranscriptStoredSegment =
                    serde_json::from_str(line).with_context(|| {
                        format!("failed to parse {} line {}", path.display(), idx + 1)
                    })?;
                if segment.schema_version != SEGMENT_SCHEMA_VERSION {
                    bail!(
                        "{} line {} has stale transcript segment schema {}",
                        path.display(),
                        idx + 1,
                        segment.schema_version
                    );
                }
                segments.push(segment);
            }
        }
    }
    Ok(segments)
}

fn transcript_store_diagnostic(message: String) -> SearchDiagnostic {
    SearchDiagnostic::error(
        "transcript_store_unreadable",
        format!("transcript store unreadable: {message}"),
    )
    .with_corpus(SearchCorpus::Transcript)
    .with_path(".maestro/index/transcripts")
    .with_retryable(false)
}

fn consented_segments(load: &TranscriptLoad) -> Vec<TranscriptStoredSegment> {
    load.segments
        .iter()
        .filter(|segment| consent_allows_segment(&load.consent_records, segment))
        .cloned()
        .collect()
}

fn consent_allows_segment(
    consent_records: &[TranscriptConsentRecord],
    segment: &TranscriptStoredSegment,
) -> bool {
    consent_records.iter().any(|record| {
        record.granted
            && record.provider == segment.provider
            && (record.scope == TranscriptConsentScope::Global
                || paths_overlap(&record.workspace, &segment.workspace))
    })
}

fn search_segments(
    paths: &MaestroPaths,
    load: &TranscriptLoad,
    parsed: &ParsedQuery,
) -> Result<Vec<SearchHit>, SearchDiagnostic> {
    let case_sensitive = crate::domain::search::query::literal_case_sensitive(parsed);
    let mut hits = Vec::new();
    let target_workspace = parsed
        .filters
        .workspace
        .clone()
        .unwrap_or_else(|| paths.repo_root().display().to_string());
    let scope_global = parsed.filters.scope.as_deref() == Some("global");
    let explicit_session = parsed.filters.session.as_deref();
    let provider_filter = parsed.filters.provider.as_deref();

    for segment in consented_segments(load) {
        if let Some(provider) = provider_filter
            && provider != segment.provider.as_str()
        {
            continue;
        }
        if let Some(session) = explicit_session
            && session != segment.session_id
        {
            continue;
        }

        let reasons =
            project_match_reasons(&segment, &target_workspace, explicit_session, scope_global);
        if reasons.is_empty() {
            continue;
        }
        if !evaluate_expr(&parsed.expr, &segment.redacted_text, case_sensitive)? {
            continue;
        }
        let Some(first_match) =
            first_positive_match(&segment.redacted_text, parsed, case_sensitive)?
        else {
            continue;
        };
        hits.push(transcript_hit(segment, first_match, reasons, parsed));
    }

    sort_and_rank_hits(&mut hits);
    Ok(hits)
}

fn transcript_hit(
    segment: TranscriptStoredSegment,
    first_match: TranscriptMatch,
    project_match_reasons: Vec<String>,
    parsed: &ParsedQuery,
) -> SearchHit {
    let mut score: f64 = 0.58;
    if parsed
        .filters
        .provider
        .as_deref()
        .is_some_and(|provider| provider == segment.provider.as_str())
    {
        score += 0.12;
    }
    if parsed
        .filters
        .session
        .as_deref()
        .is_some_and(|session| session == segment.session_id)
    {
        score += 0.16;
    }
    if first_match.factor == "regex" {
        score += 0.08;
    }

    SearchHit {
        rank: 0,
        corpus: SearchCorpus::Transcript,
        kind: "message".to_string(),
        id: format!(
            "{}:{}:{}",
            segment.provider.as_str(),
            segment.session_id,
            segment.segment_id
        ),
        path: None,
        line: None,
        title: format!("{} {}", segment.provider.as_str(), segment.source_kind),
        snippet: first_match.snippet,
        score: score.min(1.0),
        score_reasons: vec![ScoreReason {
            factor: format!("transcript_{}", first_match.factor),
            value: 1.0,
            detail: "confirmed against redacted transcript segment".to_string(),
        }],
        opener: Some(format!(
            "maestro session show {} --transcript",
            segment.session_id
        )),
        archived: false,
        feature: None,
        parent: None,
        symbol_kind: None,
        match_spans: vec![MatchSpan::Memory {
            segment_id: segment.segment_id.clone(),
            byte_start: first_match.byte_start,
            byte_end: first_match.byte_end,
        }],
        provider: Some(segment.provider.as_str().to_string()),
        session_id: Some(segment.session_id),
        authority: Some(segment.authority),
        proof_eligible: Some(segment.proof_eligible),
        source_kind: Some(segment.source_kind),
        project_match_reasons,
        redaction: Some(segment.redaction),
    }
}

fn project_match_reasons(
    segment: &TranscriptStoredSegment,
    target_workspace: &str,
    explicit_session: Option<&str>,
    scope_global: bool,
) -> Vec<String> {
    if scope_global {
        return vec!["scope_global".to_string()];
    }
    let mut reasons = Vec::new();
    if explicit_session.is_some_and(|session| session == segment.session_id) {
        reasons.push("explicit_session".to_string());
    }
    if paths_overlap(&segment.workspace, target_workspace) {
        reasons.push("workspace".to_string());
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn evaluate_expr(
    expr: &crate::domain::search::query::QueryExpr,
    contents: &str,
    case_sensitive: bool,
) -> Result<bool, SearchDiagnostic> {
    match expr {
        crate::domain::search::query::QueryExpr::Atom(
            crate::domain::search::query::QueryAtom::Literal(term),
        ) => Ok(find_literal(contents, term, case_sensitive).is_some()),
        crate::domain::search::query::QueryExpr::Atom(
            crate::domain::search::query::QueryAtom::Regex(pattern),
        ) => Ok(regex_for(pattern, case_sensitive)?.find(contents).is_some()),
        crate::domain::search::query::QueryExpr::Not(inner) => {
            Ok(!evaluate_expr(inner, contents, case_sensitive)?)
        }
        crate::domain::search::query::QueryExpr::And(items) => {
            for item in items {
                if !evaluate_expr(item, contents, case_sensitive)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        crate::domain::search::query::QueryExpr::Or(items) => {
            for item in items {
                if evaluate_expr(item, contents, case_sensitive)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn first_positive_match(
    contents: &str,
    parsed: &ParsedQuery,
    case_sensitive: bool,
) -> Result<Option<TranscriptMatch>, SearchDiagnostic> {
    for pattern in &parsed.regexes {
        let regex = regex_for(pattern, case_sensitive)?;
        if let Some(mat) = regex.find(contents) {
            return Ok(Some(transcript_match(
                contents,
                mat.start(),
                mat.end(),
                "regex",
            )));
        }
    }
    for term in &parsed.terms {
        if let Some((start, end)) = find_literal(contents, term, case_sensitive) {
            return Ok(Some(transcript_match(contents, start, end, "literal")));
        }
    }
    Ok(None)
}

fn transcript_match(
    contents: &str,
    byte_start: usize,
    byte_end: usize,
    factor: &'static str,
) -> TranscriptMatch {
    TranscriptMatch {
        byte_start,
        byte_end,
        snippet: line_snippet(contents, byte_start),
        factor,
    }
}

fn regex_for(pattern: &str, case_sensitive: bool) -> Result<Regex, SearchDiagnostic> {
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|error| SearchDiagnostic::error("parse_error", format!("invalid regex: {error}")))
}

fn find_literal(text: &str, needle: &str, case_sensitive: bool) -> Option<(usize, usize)> {
    if case_sensitive {
        return text.find(needle).map(|start| (start, start + needle.len()));
    }
    let needle_chars = lowercase_chars(needle);
    if needle_chars.is_empty() {
        return None;
    }
    let chars: Vec<(usize, String)> = text
        .char_indices()
        .map(|(idx, ch)| (idx, ch.to_lowercase().collect::<String>()))
        .collect();
    for start_idx in 0..chars.len() {
        if chars.len().saturating_sub(start_idx) < needle_chars.len() {
            continue;
        }
        if chars[start_idx..]
            .iter()
            .zip(needle_chars.iter())
            .all(|((_, hay), needle)| hay == needle)
        {
            let byte_start = chars[start_idx].0;
            let end_char_idx = start_idx + needle_chars.len();
            let byte_end = chars.get(end_char_idx).map_or(text.len(), |(idx, _)| *idx);
            return Some((byte_start, byte_end));
        }
    }
    None
}

fn lowercase_chars(text: &str) -> Vec<String> {
    text.chars()
        .map(|ch| ch.to_lowercase().collect::<String>())
        .collect()
}

fn line_snippet(contents: &str, byte_start: usize) -> String {
    let line_start = contents[..byte_start].rfind('\n').map_or(0, |idx| idx + 1);
    let line_end = contents[byte_start..]
        .find('\n')
        .map_or(contents.len(), |idx| byte_start + idx);
    contents[line_start..line_end].trim().to_string()
}

fn sort_and_rank_hits(hits: &mut [SearchHit]) {
    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.corpus.as_str().cmp(right.corpus.as_str()))
            .then_with(|| left.id.cmp(&right.id))
    });
    for (idx, hit) in hits.iter_mut().enumerate() {
        hit.rank = idx + 1;
    }
}

fn transcript_freshness(load: &TranscriptLoad, repaired: bool) -> SearchFreshness {
    SearchFreshness {
        corpus: SearchCorpus::Transcript,
        shard: ".maestro/index/transcripts/manifest.json".to_string(),
        fresh: true,
        repaired,
        schema_version: TRANSCRIPT_VIEW_SCHEMA_VERSION.to_string(),
        manifest_entries: consented_segments(load).len(),
        vocabulary_version: intent::SYMBOLIC_VOCABULARY_VERSION.to_string(),
        artifact_graph_version: intent::ARTIFACT_GRAPH_VERSION.to_string(),
        outline_extractor_version: None,
        documents: Some(consented_segments(load).len()),
        indexed_files: None,
        outline_entries: None,
        ctags_symbols: None,
        skipped_files: None,
        skipped_by_reason: BTreeMap::new(),
    }
}

fn note_raw_exclusion(excluded_fields: &mut Vec<String>, field: &str, value: &Option<String>) {
    if value.is_some() {
        excluded_fields.push(field.to_string());
    }
}

fn apply_codex_session_meta(transcript: &mut CodexTranscript, value: &Value) {
    let payload = value.get("payload").unwrap_or(value);
    if let Some(id) = string_field(payload, "id").or_else(|| string_field(value, "id")) {
        transcript.session_id = id;
    }
    if let Some(cwd) = string_field(payload, "cwd").or_else(|| string_field(value, "cwd")) {
        transcript.cwd = Some(cwd);
    }
    let roots = payload
        .get("workspace_roots")
        .or_else(|| payload.get("workspaceRoots"))
        .or_else(|| value.get("workspace_roots"))
        .or_else(|| value.get("workspaceRoots"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(non_empty);
    for root in roots {
        if !transcript.workspace_roots.contains(&root) {
            transcript.workspace_roots.push(root);
        }
    }
}

fn codex_response_segment(
    transcript: &CodexTranscript,
    value: &Value,
    ordinal: usize,
) -> Option<TranscriptSegmentInput> {
    let payload = value.get("payload")?;
    let payload_type = payload.get("type").and_then(Value::as_str)?;
    let workspace = transcript
        .cwd
        .clone()
        .or_else(|| transcript.workspace_roots.first().cloned())
        .unwrap_or_default();
    let segment_id = payload
        .get("id")
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}:{ordinal}", transcript.session_id));
    match payload_type {
        "message" => {
            let role = payload.get("role").and_then(Value::as_str)?;
            if !matches!(role, "user" | "assistant") {
                return None;
            }
            let text = json_text(payload.get("content"))?;
            if is_bootstrap_context_text(&text) {
                return None;
            }
            Some(TranscriptSegmentInput {
                provider: TranscriptProvider::Codex,
                session_id: transcript.session_id.clone(),
                segment_id,
                source_kind: format!("codex_{role}_message"),
                workspace,
                text,
                raw_tool_arguments: None,
                raw_tool_output: None,
                raw_reasoning: None,
                raw_environment: None,
            })
        }
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(TranscriptSegmentInput {
                provider: TranscriptProvider::Codex,
                session_id: transcript.session_id.clone(),
                segment_id,
                source_kind: "codex_tool_call".to_string(),
                workspace,
                text: format!("tool call: {name}"),
                raw_tool_arguments: payload.get("arguments").map(json_value_string),
                raw_tool_output: None,
                raw_reasoning: None,
                raw_environment: None,
            })
        }
        "function_call_output" => {
            let name = payload
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(TranscriptSegmentInput {
                provider: TranscriptProvider::Codex,
                session_id: transcript.session_id.clone(),
                segment_id,
                source_kind: "codex_tool_output".to_string(),
                workspace,
                text: format!("tool output: {name}"),
                raw_tool_arguments: None,
                raw_tool_output: payload.get("output").map(json_value_string),
                raw_reasoning: None,
                raw_environment: None,
            })
        }
        _ => None,
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

fn is_bootstrap_context_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("# AGENTS.md instructions")
        || (trimmed.contains("<INSTRUCTIONS>") && trimmed.contains("<environment_context>"))
}

fn paths_overlap(left: &str, right: &str) -> bool {
    let left = normalized_path(left);
    let right = normalized_path(right);
    left == right || left.starts_with(&right) || right.starts_with(&left)
}

fn normalized_path(path: &str) -> PathBuf {
    Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
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
