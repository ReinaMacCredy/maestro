use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::query::{self as card_query, body_of};
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::is_dir_backed;
use crate::domain::run;
use crate::domain::search::intent;
use crate::domain::search::lock;
use crate::domain::search::query::{self, ParsedQuery};
use crate::domain::search::types::{
    GrepEnvelope, MatchSpan, ScoreReason, SearchCorpus, SearchDiagnostic, SearchDocument,
    SearchFreshness, SearchHit, SearchSegment,
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

struct LoadedMemoryShard {
    shard: MemoryShard,
    freshness: SearchFreshness,
    diagnostics: Vec<SearchDiagnostic>,
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
    let _guard = lock::acquire_writer(paths)?;
    rebuild_memory_unlocked(paths)
}

pub(crate) fn rebuild_memory_unlocked(paths: &MaestroPaths) -> Result<MemoryRebuildReport> {
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
    grep_memory_parsed(paths, raw_query, &parsed)
}

pub fn card_list_grep_candidates(paths: &MaestroPaths, term: &str) -> Option<BTreeSet<String>> {
    if term.chars().count() < 3 {
        return None;
    }

    let needle = term.to_lowercase();
    let loaded = load_for_query(paths).ok()?;
    Some(
        loaded
            .shard
            .docs
            .iter()
            .filter(|doc| doc.kind != "run_evidence")
            .filter(|doc| card_list_doc_contains(doc, &needle))
            .map(|doc| doc.id.clone())
            .collect(),
    )
}

pub(crate) fn grep_memory_parsed(
    paths: &MaestroPaths,
    raw_query: &str,
    parsed: &ParsedQuery,
) -> GrepEnvelope {
    if parsed.filters.corpus.as_deref() == Some("source") {
        return GrepEnvelope::error_with_overrides(
            raw_query,
            SearchDiagnostic::error(
                "source_corpus_unavailable",
                "source corpus is not enabled in the memory-core slice",
            ),
            parsed.explicit_filter_overrides.clone(),
        );
    }
    if !parsed.regexes.is_empty() {
        return GrepEnvelope::error_with_overrides(
            raw_query,
            SearchDiagnostic::error(
                "invalid_filter",
                "regex atoms search source contents; use corpus:source or remove corpus:memory",
            ),
            parsed.explicit_filter_overrides.clone(),
        );
    }
    if let Some(invalid) = parsed
        .filters
        .kinds
        .iter()
        .find(|kind| !is_memory_kind(kind))
    {
        return GrepEnvelope::error_with_overrides(
            raw_query,
            SearchDiagnostic::error(
                "invalid_type",
                format!("type:{invalid} is not a Maestro-memory result kind"),
            ),
            parsed.explicit_filter_overrides.clone(),
        );
    }

    let loaded = match load_for_query(paths) {
        Ok(loaded) => loaded,
        Err(diagnostic) => {
            return GrepEnvelope::error_with_overrides(
                raw_query,
                diagnostic,
                parsed.explicit_filter_overrides.clone(),
            );
        }
    };
    let hits = score_documents(&loaded.shard.docs, parsed);
    let mut envelope =
        GrepEnvelope::success(raw_query, hits, parsed.explicit_filter_overrides.clone())
            .with_freshness(vec![loaded.freshness]);
    envelope.diagnostics = loaded.diagnostics;
    envelope
}

fn load_for_query(paths: &MaestroPaths) -> Result<LoadedMemoryShard, SearchDiagnostic> {
    match load_fresh(paths) {
        Ok(shard) => Ok(LoadedMemoryShard {
            freshness: memory_freshness(&shard, false),
            shard,
            diagnostics: Vec::new(),
        }),
        Err(error) => {
            let repair_reason = error.to_string();
            let _guard = lock::try_acquire_writer(paths)?;
            match rebuild_memory_unlocked(paths).and_then(|_| load_fresh(paths)) {
                Ok(shard) => Ok(LoadedMemoryShard {
                    freshness: memory_freshness(&shard, true),
                    shard,
                    diagnostics: vec![SearchDiagnostic::info(
                        "memory_shard_repaired",
                        format!(
                            "memory shard was stale or corrupt and was rebuilt before answering: {repair_reason}"
                        ),
                    )
                    .with_corpus(SearchCorpus::Memory)
                    .with_path(".maestro/index/search/memory.shard")
                    .with_retryable(false)],
                }),
                Err(error) => Err(SearchDiagnostic::error(
                    "memory_shard_unavailable",
                    format!("memory shard unavailable: {error}"),
                )
                .with_corpus(SearchCorpus::Memory)
                .with_path(".maestro/index/search/memory.shard")
                .with_retryable(true)),
            }
        }
    }
}

fn card_list_doc_contains(doc: &SearchDocument, needle: &str) -> bool {
    doc.title.to_lowercase().contains(needle)
        || doc
            .segments
            .iter()
            .any(|segment| segment.text.to_lowercase().contains(needle))
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

fn memory_freshness(shard: &MemoryShard, repaired: bool) -> SearchFreshness {
    SearchFreshness {
        corpus: SearchCorpus::Memory,
        shard: ".maestro/index/search/memory.shard".to_string(),
        fresh: true,
        repaired,
        schema_version: shard.schema_version.clone(),
        manifest_entries: shard.manifest.len(),
        vocabulary_version: intent::SYMBOLIC_VOCABULARY_VERSION.to_string(),
        artifact_graph_version: intent::ARTIFACT_GRAPH_VERSION.to_string(),
        outline_extractor_version: None,
        documents: Some(shard.docs.len()),
        indexed_files: None,
        outline_entries: None,
        ctags_symbols: None,
        skipped_files: None,
        skipped_by_reason: BTreeMap::new(),
    }
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
    if parsed
        .excluded_terms
        .iter()
        .any(|term| document_contains(doc, term, case_sensitive))
    {
        return None;
    }

    let required_terms = required_terms(parsed);
    let mut score = 0.0;
    let mut reasons = Vec::new();
    let mut chosen_snippet = None;
    let mut spans = Vec::new();
    let mut exact_match = false;
    let mut exact_terms = Vec::new();

    for term in &parsed.terms {
        if term.eq_ignore_ascii_case(&doc.id) {
            exact_match = true;
            exact_terms.push(term.clone());
            score += 4.0;
            chosen_snippet.get_or_insert_with(|| doc.title.clone());
            reasons.push(ScoreReason {
                factor: "exact_id".to_string(),
                value: 4.0,
                detail: "query exactly matched the Maestro card id".to_string(),
            });
        }
        if term.eq_ignore_ascii_case(&doc.title) {
            exact_match = true;
            exact_terms.push(term.clone());
            score += 3.5;
            chosen_snippet.get_or_insert_with(|| doc.title.clone());
            reasons.push(ScoreReason {
                factor: "exact_title".to_string(),
                value: 3.5,
                detail: "query exactly matched the Maestro title".to_string(),
            });
        }
        if doc
            .path
            .as_deref()
            .is_some_and(|path| path.eq_ignore_ascii_case(term))
        {
            exact_match = true;
            exact_terms.push(term.clone());
            score += 3.0;
            chosen_snippet
                .get_or_insert_with(|| doc.path.clone().unwrap_or_else(|| doc.title.clone()));
            reasons.push(ScoreReason {
                factor: "exact_path".to_string(),
                value: 3.0,
                detail: "query exactly matched the Maestro artifact path".to_string(),
            });
        }
        if term.split_whitespace().count() > 1
            && let Some(found) = best_segment_match(doc, term, case_sensitive)
        {
            score += 1.2;
            chosen_snippet
                .get_or_insert_with(|| snippet(&found.segment.text, found.start, found.end));
            spans.push(MatchSpan::Memory {
                segment_id: found.segment.id.clone(),
                byte_start: found.start,
                byte_end: found.end,
            });
            reasons.push(ScoreReason {
                factor: "phrase".to_string(),
                value: 1.2,
                detail: format!("quoted phrase matched {}", found.segment.field),
            });
        }
    }

    for term in &required_terms {
        if exact_terms
            .iter()
            .any(|exact| exact.eq_ignore_ascii_case(term))
        {
            continue;
        }
        if let Some(found) = best_segment_match(doc, term, case_sensitive) {
            let weight = field_weight(&found.segment.field);
            let value = bm25_like_value(term, &found.segment.text, weight);
            score += value;
            if chosen_snippet.is_none() {
                chosen_snippet = Some(snippet(&found.segment.text, found.start, found.end));
            }
            spans.push(MatchSpan::Memory {
                segment_id: found.segment.id.clone(),
                byte_start: found.start,
                byte_end: found.end,
            });
            reasons.push(ScoreReason {
                factor: "lexical".to_string(),
                value,
                detail: format!("BM25-like {} match", found.segment.field),
            });
        } else if let Some((alias, found)) = alias_match(doc, term, case_sensitive) {
            let value = 0.45 * field_weight(&found.segment.field);
            score += value;
            if chosen_snippet.is_none() {
                chosen_snippet = Some(snippet(&found.segment.text, found.start, found.end));
            }
            spans.push(MatchSpan::Memory {
                segment_id: found.segment.id.clone(),
                byte_start: found.start,
                byte_end: found.end,
            });
            reasons.push(ScoreReason {
                factor: "domain_alias".to_string(),
                value,
                detail: format!("{term} expanded to {alias}"),
            });
        } else {
            return None;
        }
    }

    if required_terms.is_empty() && !exact_match && reasons.is_empty() {
        return None;
    }

    if !required_terms.is_empty() && proximity_match(doc, &required_terms, case_sensitive) {
        score += 0.35;
        reasons.push(ScoreReason {
            factor: "proximity".to_string(),
            value: 0.35,
            detail: "meaningful query terms appeared close together".to_string(),
        });
    }

    if has_memory_intent_term(parsed) || !required_terms.is_empty() {
        let value = artifact_type_weight(&doc.kind);
        score += value;
        reasons.push(ScoreReason {
            factor: "artifact_type".to_string(),
            value,
            detail: format!("{} is a Maestro-memory artifact", doc.kind),
        });
    }

    if doc.kind == "feature" || doc.parent.is_some() || doc.feature.is_some() {
        score += 0.15;
        reasons.push(ScoreReason {
            factor: "maestro_graph".to_string(),
            value: 0.15,
            detail: "card participates in the Maestro feature/task graph".to_string(),
        });
    }

    let max = (required_terms.len().max(1) as f64 * 2.4) + 2.0;
    let mut normalized = (score / max).min(1.0);
    if exact_match {
        normalized = normalized.max(0.98);
    }
    Some(SearchHit {
        rank: 0,
        corpus: doc.corpus,
        kind: doc.kind.clone(),
        id: doc.id.clone(),
        path: doc.path.clone(),
        line: None,
        title: doc.title.clone(),
        snippet: chosen_snippet.unwrap_or_else(|| doc.title.clone()),
        score: normalized,
        score_reasons: reasons,
        opener: doc.opener.clone(),
        archived: doc.archived,
        feature: doc.feature.clone(),
        parent: doc.parent.clone(),
        symbol_kind: None,
        match_spans: spans,
    })
}

struct SegmentMatch<'a> {
    segment: &'a SearchSegment,
    start: usize,
    end: usize,
}

fn field_weight(field: &str) -> f64 {
    match field {
        "title" => 2.0,
        "spec.md" | "notes.md" | "qa.md" => 1.5,
        _ => 1.0,
    }
}

fn bm25_like_value(term: &str, text: &str, weight: f64) -> f64 {
    let length_norm = 1.0 + (text.split_whitespace().count() as f64 / 180.0);
    let term_weight = 1.0 + (term.chars().count().min(16) as f64 / 32.0);
    (weight * term_weight) / length_norm
}

fn required_terms(parsed: &ParsedQuery) -> Vec<String> {
    parsed
        .terms
        .iter()
        .flat_map(|term| term.split_whitespace())
        .map(|term| term.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '`'))
        .filter(|term| !term.is_empty())
        .filter(|term| !is_stopword(term))
        .map(str::to_string)
        .collect()
}

fn is_stopword(term: &str) -> bool {
    matches!(
        term.to_ascii_lowercase().as_str(),
        "a" | "an"
            | "and"
            | "are"
            | "did"
            | "do"
            | "does"
            | "for"
            | "how"
            | "in"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "should"
            | "the"
            | "to"
            | "was"
            | "we"
            | "were"
            | "what"
            | "when"
            | "where"
            | "who"
            | "why"
            | "with"
    )
}

fn has_memory_intent_term(parsed: &ParsedQuery) -> bool {
    parsed
        .terms
        .iter()
        .flat_map(|term| term.split_whitespace())
        .any(|term| {
            matches!(
                term.to_ascii_lowercase().as_str(),
                "why"
                    | "workflow"
                    | "decision"
                    | "decisions"
                    | "proof"
                    | "proofs"
                    | "history"
                    | "rationale"
                    | "notes"
                    | "note"
                    | "evidence"
            )
        })
}

fn artifact_type_weight(kind: &str) -> f64 {
    match kind {
        "decision" => 0.50,
        "feature" => 0.42,
        "task" => 0.34,
        "run_evidence" => 0.28,
        _ => 0.24,
    }
}

fn best_segment_match<'a>(
    doc: &'a SearchDocument,
    term: &str,
    case_sensitive: bool,
) -> Option<SegmentMatch<'a>> {
    doc.segments.iter().find_map(|segment| {
        find_term(&segment.text, term, case_sensitive).map(|(start, end)| SegmentMatch {
            segment,
            start,
            end,
        })
    })
}

fn alias_match<'a>(
    doc: &'a SearchDocument,
    term: &str,
    case_sensitive: bool,
) -> Option<(&'static str, SegmentMatch<'a>)> {
    aliases_for(term).iter().find_map(|alias| {
        best_segment_match(doc, alias, case_sensitive).map(|found| (*alias, found))
    })
}

fn aliases_for(term: &str) -> &'static [&'static str] {
    match term.to_ascii_lowercase().as_str() {
        "decision" | "decisions" => &["rationale", "decided", "because"],
        "proof" | "proofs" => &["evidence", "verification", "verify", "claim"],
        "history" => &["notes", "timeline", "decisions", "proof"],
        "workflow" => &["task", "feature", "status", "handoff"],
        "runtime" => &["session"],
        _ => &[],
    }
}

fn proximity_match(doc: &SearchDocument, terms: &[String], case_sensitive: bool) -> bool {
    if terms.len() < 2 {
        return false;
    }
    doc.segments.iter().any(|segment| {
        let mut positions = Vec::new();
        for term in terms {
            let Some((start, _)) = find_term(&segment.text, term, case_sensitive) else {
                return false;
            };
            positions.push(start);
        }
        let min = positions.iter().min().copied().unwrap_or_default();
        let max = positions.iter().max().copied().unwrap_or_default();
        max.saturating_sub(min) <= 120
    })
}

fn document_contains(doc: &SearchDocument, term: &str, case_sensitive: bool) -> bool {
    doc.id.eq_ignore_ascii_case(term)
        || doc.title.eq_ignore_ascii_case(term)
        || doc
            .path
            .as_deref()
            .is_some_and(|path| find_term(path, term, case_sensitive).is_some())
        || doc
            .segments
            .iter()
            .any(|segment| find_term(&segment.text, term, case_sensitive).is_some())
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
    intent::is_memory_kind(kind)
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
