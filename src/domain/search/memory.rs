use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::query::{self as card_query, body_of};
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::is_dir_backed;
use crate::domain::run;
use crate::domain::search::query::{self, ParsedQuery};
use crate::domain::search::types::{
    GrepEnvelope, MatchSpan, ScoreReason, SearchCorpus, SearchDiagnostic, SearchDocument,
    SearchHit, SearchSegment,
};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::{write_atomic, write_string_atomic};

const MEMORY_SHARD_SCHEMA_VERSION: &str = "maestro.memory-shard.v1";
const SEARCH_MANIFEST_SCHEMA_VERSION: &str = "maestro.search-manifest.v1";
const MEMORY_SHARD_MAGIC: &[u8] = b"MAESTRO_MEMORY_SHARD_V1\n";
const MEMORY_SIDECARS: &[&str] = &["notes.md", "spec.md", "qa.md", "decisions.yaml"];

#[derive(Debug)]
pub struct MemoryRebuildReport {
    pub docs: usize,
    pub live_docs: usize,
    pub archived_docs: usize,
    pub run_evidence_docs: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct MemoryShard {
    schema_version: String,
    manifest: Vec<ManifestEntry>,
    docs: Vec<SearchDocument>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct ManifestEntry {
    path: String,
    mtime_ns: u64,
    len: u64,
}

#[derive(Debug, Serialize)]
struct SearchManifest {
    schema_version: String,
    memory_schema_version: String,
    memory_docs: usize,
}

pub fn rebuild_memory(paths: &MaestroPaths) -> Result<MemoryRebuildReport> {
    let live = card_query::scan_with_paths(paths)?;
    let archived = card_query::scan_dir_with_paths(&paths.archive_cards_dir())?;
    let run_evidence = run::load_run_evidence(paths)?;

    let mut docs = Vec::new();
    for (card, path) in &live {
        docs.push(document_for_card(card, path, false)?);
    }
    for (card, path) in &archived {
        docs.push(document_for_card(card, path, true)?);
    }
    for record in &run_evidence.records {
        docs.push(document_for_run_evidence(record));
    }

    let shard = MemoryShard {
        schema_version: MEMORY_SHARD_SCHEMA_VERSION.to_string(),
        manifest: manifest(paths)?,
        docs,
    };
    ensure_dir(paths.search_index_dir())?;
    write_atomic(paths.memory_shard_file(), &encode_shard(&shard)?)?;
    write_search_manifest(paths, shard.docs.len())?;

    Ok(MemoryRebuildReport {
        docs: shard.docs.len(),
        live_docs: live.len(),
        archived_docs: archived.len(),
        run_evidence_docs: run_evidence.records.len(),
    })
}

pub fn grep_memory(paths: &MaestroPaths, raw_query: &str) -> GrepEnvelope {
    let parsed = match query::parse(raw_query) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return GrepEnvelope::error(raw_query, diagnostic),
    };
    if parsed.filters.corpus.as_deref() == Some("source") {
        return GrepEnvelope::error(
            raw_query,
            SearchDiagnostic::error(
                "source_corpus_unavailable",
                "source corpus is not enabled in the memory-core slice",
            ),
        );
    }
    if let Some(invalid) = parsed
        .filters
        .kinds
        .iter()
        .find(|kind| !is_memory_kind(kind))
    {
        return GrepEnvelope::error(
            raw_query,
            SearchDiagnostic::error(
                "invalid_type",
                format!("type:{invalid} is not a Maestro-memory result kind"),
            ),
        );
    }

    let shard = match load_fresh(paths) {
        Ok(shard) => shard,
        Err(_) => match rebuild_memory(paths).and_then(|_| load_fresh(paths)) {
            Ok(shard) => shard,
            Err(error) => {
                return GrepEnvelope::error(
                    raw_query,
                    SearchDiagnostic {
                        severity: crate::domain::search::types::DiagnosticSeverity::Error,
                        code: "memory_shard_unavailable".to_string(),
                        message: format!("memory shard unavailable: {error}"),
                        corpus: Some(SearchCorpus::Memory),
                        path: Some(".maestro/index/search/memory.shard".to_string()),
                        retryable: Some(true),
                    },
                );
            }
        },
    };
    let hits = score_documents(&shard.docs, &parsed);
    GrepEnvelope::success(raw_query, hits, parsed.explicit_filter_overrides.clone())
}

fn load_fresh(paths: &MaestroPaths) -> Result<MemoryShard> {
    let bytes = std::fs::read(paths.memory_shard_file()).context("failed to read memory shard")?;
    let shard = decode_shard(&bytes)?;
    if shard.schema_version != MEMORY_SHARD_SCHEMA_VERSION {
        bail!("memory shard schema is stale");
    }
    if shard.manifest != manifest(paths)? {
        bail!("memory shard manifest is stale");
    }
    Ok(shard)
}

fn write_search_manifest(paths: &MaestroPaths, memory_docs: usize) -> Result<()> {
    let manifest = SearchManifest {
        schema_version: SEARCH_MANIFEST_SCHEMA_VERSION.to_string(),
        memory_schema_version: MEMORY_SHARD_SCHEMA_VERSION.to_string(),
        memory_docs,
    };
    let contents =
        serde_json::to_string_pretty(&manifest).context("failed to serialize search manifest")?;
    write_string_atomic(paths.search_manifest_file(), &contents)
}

fn encode_shard(shard: &MemoryShard) -> Result<Vec<u8>> {
    let mut bytes = MEMORY_SHARD_MAGIC.to_vec();
    let payload = serde_json::to_vec(shard).context("failed to serialize memory shard")?;
    bytes.extend(payload);
    Ok(bytes)
}

fn decode_shard(bytes: &[u8]) -> Result<MemoryShard> {
    let Some(payload) = bytes.strip_prefix(MEMORY_SHARD_MAGIC) else {
        bail!("memory shard magic header is missing");
    };
    serde_json::from_slice(payload).context("failed to parse memory shard")
}

fn document_for_card(card: &Card, path: &Path, archived: bool) -> Result<SearchDocument> {
    let mut fields = BTreeMap::new();
    fields.insert("status".to_string(), card.status.clone());
    if let Some(parent) = &card.parent {
        fields.insert("parent".to_string(), parent.clone());
    }
    if let Some(project) = &card.project {
        fields.insert("project".to_string(), project.clone());
    }

    let kind = memory_kind_for_card(card);
    let mut segments = vec![SearchSegment {
        id: "title".to_string(),
        field: "title".to_string(),
        text: card.title.clone(),
    }];
    if let Some(body) = body_of(card) {
        segments.push(SearchSegment {
            id: "body".to_string(),
            field: "body".to_string(),
            text: body,
        });
    }
    if !card.extra.is_empty() {
        segments.push(SearchSegment {
            id: "record".to_string(),
            field: "record".to_string(),
            text: serde_yaml::to_string(&card.extra).context("failed to serialize card payload")?,
        });
    }
    if is_dir_backed(path)
        && let Some(dir) = path.parent()
    {
        for sidecar in MEMORY_SIDECARS {
            let sidecar_path = dir.join(sidecar);
            if let Ok(text) = std::fs::read_to_string(&sidecar_path) {
                segments.push(SearchSegment {
                    id: sidecar.trim_end_matches(".md").replace('.', "_"),
                    field: sidecar.to_string(),
                    text,
                });
            }
        }
    }

    Ok(SearchDocument {
        id: card.id.clone(),
        corpus: SearchCorpus::Memory,
        kind: kind.to_string(),
        title: card.title.clone(),
        path: Some(relative_label(
            path,
            &std::env::current_dir().unwrap_or_default(),
        )),
        opener: Some(format!("maestro card show {}", card.id)),
        archived,
        feature: card
            .parent
            .clone()
            .filter(|_| card.card_type != CardType::Feature),
        parent: card.parent.clone(),
        fields,
        segments,
    })
}

fn document_for_run_evidence(record: &run::RunEvidenceRecord) -> SearchDocument {
    let mut fields = BTreeMap::new();
    fields.insert("type".to_string(), "run_evidence".to_string());
    if let Some(agent) = &record.agent {
        fields.insert("runtime".to_string(), agent.clone());
    }
    if let Some(task_id) = &record.task_id {
        fields.insert("task_id".to_string(), task_id.clone());
    }
    let body = format!(
        "session_id: {}\nagent: {}\ntask_id: {}\nduration_seconds: {}\nhuman_interventions: {}",
        record.session_id,
        record.agent.as_deref().unwrap_or(""),
        record.task_id.as_deref().unwrap_or(""),
        record
            .duration_seconds
            .map(|value| value.to_string())
            .unwrap_or_default(),
        record.human_interventions
    );
    SearchDocument {
        id: format!("run_evidence:{}", record.session_id),
        corpus: SearchCorpus::Memory,
        kind: "run_evidence".to_string(),
        title: format!("run evidence {}", record.session_id),
        path: None,
        opener: None,
        archived: false,
        feature: None,
        parent: record.task_id.clone(),
        fields,
        segments: vec![SearchSegment {
            id: "run_evidence".to_string(),
            field: "run_evidence".to_string(),
            text: body,
        }],
    }
}

fn memory_kind_for_card(card: &Card) -> &'static str {
    match card.card_type {
        CardType::Feature => "feature",
        CardType::Task | CardType::Bug | CardType::Chore => "task",
        CardType::Decision => "decision",
        CardType::Idea => "card",
    }
}

fn score_documents(docs: &[SearchDocument], parsed: &ParsedQuery) -> Vec<SearchHit> {
    let case_sensitive = query::literal_case_sensitive(parsed);
    let mut hits: Vec<SearchHit> = docs
        .iter()
        .filter(|doc| filters_match(doc, parsed))
        .filter_map(|doc| score_document(doc, parsed, case_sensitive))
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.id.cmp(&b.id))
    });
    for (idx, hit) in hits.iter_mut().enumerate() {
        hit.rank = idx + 1;
    }
    hits
}

fn filters_match(doc: &SearchDocument, parsed: &ParsedQuery) -> bool {
    if !parsed.filters.kinds.is_empty()
        && !parsed.filters.kinds.iter().any(|kind| kind == &doc.kind)
    {
        return false;
    }
    if let Some(status) = &parsed.filters.status
        && doc.fields.get("status") != Some(status)
    {
        return false;
    }
    if let Some(feature) = &parsed.filters.feature
        && doc.feature.as_deref() != Some(feature)
        && doc.id != *feature
    {
        return false;
    }
    if let Some(runtime) = &parsed.filters.runtime
        && doc.fields.get("runtime") != Some(runtime)
    {
        return false;
    }
    if let Some(event) = &parsed.filters.event
        && doc.fields.get("event") != Some(event)
    {
        return false;
    }
    true
}

fn score_document(
    doc: &SearchDocument,
    parsed: &ParsedQuery,
    case_sensitive: bool,
) -> Option<SearchHit> {
    let mut score = 0.0;
    let mut reasons = Vec::new();
    let mut chosen_snippet = None;
    let mut spans = Vec::new();

    for term in &parsed.terms {
        let mut matched_term = false;
        for segment in &doc.segments {
            if let Some((start, end)) = find_term(&segment.text, term, case_sensitive) {
                let weight = field_weight(&segment.field);
                score += weight;
                matched_term = true;
                if chosen_snippet.is_none() {
                    chosen_snippet = Some(snippet(&segment.text, start, end));
                }
                spans.push(MatchSpan::Memory {
                    segment_id: segment.id.clone(),
                    byte_start: start,
                    byte_end: end,
                });
                reasons.push(ScoreReason {
                    factor: "lexical".to_string(),
                    value: weight,
                    detail: format!("{} match", segment.field),
                });
                break;
            }
        }
        if !matched_term {
            return None;
        }
    }

    let max = (parsed.terms.len() as f64 * 2.0).max(1.0);
    Some(SearchHit {
        rank: 0,
        corpus: doc.corpus,
        kind: doc.kind.clone(),
        id: doc.id.clone(),
        path: doc.path.clone(),
        line: None,
        title: doc.title.clone(),
        snippet: chosen_snippet.unwrap_or_else(|| doc.title.clone()),
        score: (score / max).min(1.0),
        score_reasons: reasons,
        opener: doc.opener.clone(),
        archived: doc.archived,
        feature: doc.feature.clone(),
        parent: doc.parent.clone(),
        symbol_kind: None,
        match_spans: spans,
    })
}

fn field_weight(field: &str) -> f64 {
    match field {
        "title" => 2.0,
        "spec.md" | "notes.md" | "qa.md" => 1.5,
        _ => 1.0,
    }
}

fn find_term(text: &str, term: &str, case_sensitive: bool) -> Option<(usize, usize)> {
    if case_sensitive {
        text.find(term).map(|start| (start, start + term.len()))
    } else {
        let haystack = text.to_lowercase();
        let needle = term.to_lowercase();
        haystack
            .find(&needle)
            .map(|start| (start, start + needle.len()))
    }
}

fn snippet(text: &str, start: usize, end: usize) -> String {
    let prefix = text[..start]
        .char_indices()
        .rev()
        .nth(40)
        .map_or(0, |(idx, _)| idx);
    let suffix = text[end..]
        .char_indices()
        .nth(80)
        .map_or(text.len(), |(idx, _)| end + idx);
    text[prefix..suffix]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_memory_kind(kind: &str) -> bool {
    matches!(
        kind,
        "card" | "decision" | "feature" | "task" | "proof" | "qa" | "run_summary" | "run_evidence"
    )
}

fn manifest(paths: &MaestroPaths) -> Result<Vec<ManifestEntry>> {
    let mut entries = Vec::new();
    for root in [paths.cards_dir(), paths.archive_cards_dir()] {
        collect_files(&root, &paths.maestro_dir(), &mut entries, |_| true)?;
    }
    collect_files(
        &paths.runs_dir(),
        &paths.maestro_dir(),
        &mut entries,
        |path| path.file_name().and_then(|name| name.to_str()) == Some("run_evidence.yaml"),
    )?;
    entries.sort();
    Ok(entries)
}

fn collect_files(
    dir: &Path,
    base: &Path,
    entries: &mut Vec<ManifestEntry>,
    include_file: impl Copy + Fn(&Path) -> bool,
) -> Result<()> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in read_dir {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        if metadata.is_dir() {
            collect_files(&path, base, entries, include_file)?;
        } else if metadata.is_file() && include_file(&path) {
            entries.push(ManifestEntry {
                path: relative_label(&path, base),
                mtime_ns: modified_ns(&metadata),
                len: metadata.len(),
            });
        }
    }
    Ok(())
}

fn modified_ns(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default()
}

fn relative_label(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::core::time::utc_now_timestamp;

    #[test]
    fn memory_card_document_includes_sidecars_and_expected_metadata() {
        let now = utc_now_timestamp();
        let mut card = Card::new(
            "demo-feature",
            CardType::Feature,
            "Runtime Search",
            "proposed",
            &now,
        );
        card.description = Some("Decision proof history".to_string());
        let doc = document_for_card(
            &card,
            Path::new(".maestro/cards/demo-feature/card.yaml"),
            false,
        )
        .expect("document should build");
        assert_eq!(doc.corpus, SearchCorpus::Memory);
        assert_eq!(doc.kind, "feature");
        assert!(doc.segments.iter().any(|segment| segment.field == "title"));
        assert!(
            doc.segments
                .iter()
                .any(|segment| segment.text.contains("Decision proof"))
        );
    }

    #[test]
    fn memory_hits_include_structured_reason_and_span() {
        let doc = SearchDocument {
            id: "dec-demo".to_string(),
            corpus: SearchCorpus::Memory,
            kind: "decision".to_string(),
            title: "Agent runtime decision".to_string(),
            path: None,
            opener: Some("maestro decision show dec-demo".to_string()),
            archived: false,
            feature: Some("runtime".to_string()),
            parent: Some("runtime".to_string()),
            fields: BTreeMap::new(),
            segments: vec![SearchSegment {
                id: "title".to_string(),
                field: "title".to_string(),
                text: "Agent runtime decision".to_string(),
            }],
        };
        let parsed = query::parse("runtime type:decision corpus:memory").expect("query parses");
        let hits = score_documents(&[doc], &parsed);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].corpus, SearchCorpus::Memory);
        assert_eq!(hits[0].kind, "decision");
        assert_eq!(hits[0].score_reasons[0].factor, "lexical");
        assert!(!hits[0].match_spans.is_empty());
    }
}
