use std::collections::BTreeMap;
use std::path::{Component, Path};
use std::process::Command;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::domain::search::intent::{self, IntentDecision};
use crate::domain::search::lock;
use crate::domain::search::memory;
use crate::domain::search::outline::{self, OutlineEntry};
use crate::domain::search::query::{self, ParsedQuery, QueryAtom, QueryExpr};
use crate::domain::search::types::{
    DiagnosticSeverity, GrepEnvelope, MatchSpan, ScoreReason, SearchCorpus, SearchDiagnostic,
    SearchFreshness, SearchHit,
};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_atomic;

const SOURCE_SHARD_SCHEMA_VERSION: &str = "source-shard.v2";
const SOURCE_SHARD_MAGIC: &[u8] = b"MAESTRO_SOURCE_SHARD_V2\n";
const MAX_SOURCE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_SOURCE_LINES: usize = 80_000;
const OUTLINE_KINDS: &[&str] = &[
    "function", "method", "struct", "enum", "trait", "impl", "field", "import", "export", "module",
];

#[derive(Debug, Serialize)]
pub struct SourceRebuildReport {
    pub indexed_files: usize,
    pub outline_entries: usize,
    pub ctags_symbols: usize,
    pub ctags_status: CtagsStatus,
    pub skipped_files: usize,
    pub skipped_by_reason: BTreeMap<String, usize>,
    pub representative_skips: Vec<SourceSkip>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CtagsStatus {
    pub available: bool,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexHealth {
    pub source_shard_present: bool,
    pub indexed_files: usize,
    pub outline_entries: usize,
    pub ctags_symbols: usize,
    pub ctags_status: CtagsStatus,
    pub supported_outline_languages: Vec<&'static str>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceSkip {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SourceShard {
    schema_version: String,
    manifest: Vec<ManifestEntry>,
    files: Vec<SourceFile>,
    outline_entries: Vec<OutlineEntry>,
    symbols: Vec<SymbolEntry>,
    ctags_status: CtagsStatus,
    postings: BTreeMap<String, Vec<SourcePosting>>,
    skipped: Vec<SourceSkip>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SourceFile {
    path: String,
    language: String,
    contents: String,
    line_offsets: Vec<LineOffset>,
    trigram_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct LineOffset {
    line: u64,
    byte_start: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SourcePosting {
    path: String,
    line: u64,
    byte_start: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SymbolEntry {
    path: String,
    name: String,
    kind: String,
    line: u64,
    signature: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct ManifestEntry {
    path: String,
    mtime_ns: u64,
    len: u64,
}

#[derive(Clone, Debug)]
struct SourceMatch {
    byte_start: usize,
    byte_end: usize,
    line: u64,
    snippet: String,
    factor: &'static str,
}

struct LoadedSourceShard {
    shard: SourceShard,
    freshness: SearchFreshness,
    diagnostics: Vec<SearchDiagnostic>,
}

pub fn grep(paths: &MaestroPaths, raw_query: &str) -> GrepEnvelope {
    let parsed = match query::parse(raw_query) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return GrepEnvelope::error(raw_query, diagnostic),
    };
    let decision = intent::classify(&parsed);

    if parsed.filters.corpus.as_deref() == Some("memory") && has_source_only_filter(&parsed) {
        return GrepEnvelope::error_with_overrides(
            raw_query,
            SearchDiagnostic::error(
                "invalid_filter",
                "source filters cannot be combined with corpus:memory",
            ),
            parsed.explicit_filter_overrides.clone(),
        );
    }

    match decision.route {
        Some(SearchCorpus::Memory) => with_intent(
            memory::grep_memory_parsed(paths, raw_query, &parsed),
            &decision,
        ),
        Some(SearchCorpus::Source) => {
            with_intent(grep_source_parsed(paths, raw_query, &parsed), &decision)
        }
        None => grep_mixed(paths, raw_query, &parsed, &decision),
    }
}

fn has_source_filter(parsed: &ParsedQuery) -> bool {
    !parsed.filters.file_globs.is_empty()
        || !parsed.filters.excluded_file_globs.is_empty()
        || parsed.filters.lang.is_some()
        || parsed.filters.sym.is_some()
}

fn has_source_only_filter(parsed: &ParsedQuery) -> bool {
    has_source_filter(parsed)
        || !parsed.regexes.is_empty()
        || parsed
            .filters
            .kinds
            .iter()
            .any(|kind| intent::is_source_kind(kind))
}

pub fn grep_source(paths: &MaestroPaths, raw_query: &str) -> GrepEnvelope {
    let parsed = match query::parse(raw_query) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return GrepEnvelope::error(raw_query, diagnostic),
    };
    grep_source_parsed(paths, raw_query, &parsed)
}

fn grep_mixed(
    paths: &MaestroPaths,
    raw_query: &str,
    parsed: &ParsedQuery,
    decision: &IntentDecision,
) -> GrepEnvelope {
    let memory_envelope = memory::grep_memory_parsed(paths, raw_query, parsed);
    let source_envelope = grep_source_parsed(paths, raw_query, parsed);
    if lock::is_lock_contention(&memory_envelope) {
        return memory_envelope;
    }
    if lock::is_lock_contention(&source_envelope) {
        return source_envelope;
    }
    let mut diagnostics = Vec::new();
    let mut hits = Vec::new();
    let mut freshness = Vec::new();

    if memory_envelope.ok {
        freshness.extend(memory_envelope.freshness);
        hits.extend(memory_envelope.hits);
    } else {
        diagnostics.extend(memory_envelope.diagnostics);
        freshness.extend(memory_envelope.freshness);
    }
    if source_envelope.ok {
        freshness.extend(source_envelope.freshness);
        hits.extend(source_envelope.hits);
    } else {
        diagnostics.extend(source_envelope.diagnostics);
        freshness.extend(source_envelope.freshness);
    }

    if hits.is_empty() && !diagnostics.is_empty() {
        return GrepEnvelope::error(raw_query, diagnostics.remove(0));
    }

    let mut envelope = GrepEnvelope::success_with_intent(
        raw_query,
        intent::rerank(hits, decision.kind),
        parsed.explicit_filter_overrides.clone(),
        decision.kind.as_str(),
        decision.confidence,
        decision.reasons.clone(),
    );
    envelope.partial = !diagnostics.is_empty();
    envelope.diagnostics = diagnostics;
    envelope.freshness = freshness;
    envelope
}

fn with_intent(mut envelope: GrepEnvelope, decision: &IntentDecision) -> GrepEnvelope {
    if envelope.ok {
        envelope.intent = Some(decision.kind.as_str().to_string());
        envelope.intent_confidence = Some(decision.confidence.to_string());
        envelope.intent_reasons = decision.reasons.clone();
        envelope.hits = intent::rerank(envelope.hits, decision.kind);
    }
    envelope
}

pub fn rebuild_source(paths: &MaestroPaths) -> Result<SourceRebuildReport> {
    let _guard = lock::acquire_writer(paths)?;
    rebuild_source_unlocked(paths)
}

pub(crate) fn rebuild_source_unlocked(paths: &MaestroPaths) -> Result<SourceRebuildReport> {
    let mut skipped = Vec::new();
    let candidates = source_manifest(paths, &mut skipped)?;
    let mut files = Vec::new();
    let mut outline_entries = Vec::new();
    let mut postings: BTreeMap<String, Vec<SourcePosting>> = BTreeMap::new();

    for entry in &candidates {
        let path = paths.repo_root().join(&entry.path);
        let bytes = std::fs::read(&path)
            .with_context(|| format!("failed to read source candidate {}", entry.path))?;
        let contents = match String::from_utf8(bytes) {
            Ok(contents) => contents,
            Err(_) => {
                skipped.push(SourceSkip {
                    path: entry.path.clone(),
                    reason: "binary".to_string(),
                });
                continue;
            }
        };
        let line_count = contents
            .lines()
            .count()
            .max(usize::from(!contents.is_empty()));
        if line_count > MAX_SOURCE_LINES {
            skipped.push(SourceSkip {
                path: entry.path.clone(),
                reason: "line_cap".to_string(),
            });
            continue;
        }
        let line_offsets = line_offsets(&contents);
        let mut trigram_count = 0;
        for (trigram, byte_start) in trigrams(&contents) {
            trigram_count += 1;
            postings.entry(trigram).or_default().push(SourcePosting {
                path: entry.path.clone(),
                line: line_for_byte(&line_offsets, byte_start),
                byte_start,
            });
        }
        let language = language_for_path(&entry.path).to_string();
        outline_entries.extend(outline::extract_outline(&entry.path, &language, &contents));
        files.push(SourceFile {
            path: entry.path.clone(),
            language,
            contents,
            line_offsets,
            trigram_count,
        });
    }

    let (ctags_status, symbols) = collect_ctags(paths, &candidates);
    let report = report(
        files.len(),
        outline_entries.len(),
        symbols.len(),
        ctags_status.clone(),
        &skipped,
    );
    let shard = SourceShard {
        schema_version: SOURCE_SHARD_SCHEMA_VERSION.to_string(),
        manifest: candidates,
        files,
        outline_entries,
        symbols,
        ctags_status,
        postings,
        skipped,
    };
    ensure_dir(paths.search_index_dir())?;
    write_atomic(paths.source_shard_file(), &encode_shard(&shard)?)?;
    Ok(report)
}

pub fn source_index_health(paths: &MaestroPaths) -> SourceIndexHealth {
    let supported_outline_languages = outline::extractor_health().supported_languages;
    match std::fs::read(paths.source_shard_file())
        .ok()
        .and_then(|bytes| decode_shard(&bytes).ok())
    {
        Some(shard) if shard.schema_version == SOURCE_SHARD_SCHEMA_VERSION => SourceIndexHealth {
            source_shard_present: true,
            indexed_files: shard.files.len(),
            outline_entries: shard.outline_entries.len(),
            ctags_symbols: shard.symbols.len(),
            ctags_status: shard.ctags_status,
            supported_outline_languages,
        },
        _ => SourceIndexHealth {
            source_shard_present: false,
            indexed_files: 0,
            outline_entries: 0,
            ctags_symbols: 0,
            ctags_status: current_ctags_status(),
            supported_outline_languages,
        },
    }
}

pub(crate) fn grep_source_parsed(
    paths: &MaestroPaths,
    raw_query: &str,
    parsed: &ParsedQuery,
) -> GrepEnvelope {
    if let Some(invalid) = parsed.filters.kinds.iter().find(|kind| {
        let kind = kind.as_str();
        kind != "file" && !OUTLINE_KINDS.contains(&kind)
    }) {
        return GrepEnvelope::error_with_overrides(
            raw_query,
            SearchDiagnostic::error(
                "invalid_type",
                format!("type:{invalid} is not a source result kind"),
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

    let hits = if parsed.filters.sym.is_some() {
        match search_symbols(&loaded.shard, parsed) {
            Ok(hits) => hits,
            Err(diagnostic) => {
                let mut envelope = GrepEnvelope::error_with_overrides(
                    raw_query,
                    diagnostic,
                    parsed.explicit_filter_overrides.clone(),
                )
                .with_freshness(vec![loaded.freshness]);
                envelope.diagnostics.extend(loaded.diagnostics);
                return envelope;
            }
        }
    } else {
        match search_shard(&loaded.shard, parsed) {
            Ok(hits) => hits,
            Err(diagnostic) => {
                let mut envelope = GrepEnvelope::error_with_overrides(
                    raw_query,
                    diagnostic,
                    parsed.explicit_filter_overrides.clone(),
                )
                .with_freshness(vec![loaded.freshness]);
                envelope.diagnostics.extend(loaded.diagnostics);
                return envelope;
            }
        }
    };
    let mut envelope =
        GrepEnvelope::success(raw_query, hits, parsed.explicit_filter_overrides.clone())
            .with_freshness(vec![loaded.freshness]);
    envelope.diagnostics = loaded.diagnostics;
    envelope
}

fn load_for_query(paths: &MaestroPaths) -> Result<LoadedSourceShard, SearchDiagnostic> {
    match load_fresh(paths) {
        Ok(shard) => Ok(LoadedSourceShard {
            freshness: source_freshness(&shard, false),
            shard,
            diagnostics: Vec::new(),
        }),
        Err(error) => {
            let repair_reason = error.to_string();
            let _guard = lock::try_acquire_writer(paths)?;
            match rebuild_source_unlocked(paths).and_then(|_| load_fresh(paths)) {
                Ok(shard) => Ok(LoadedSourceShard {
                    freshness: source_freshness(&shard, true),
                    shard,
                    diagnostics: vec![SearchDiagnostic::info(
                        "source_shard_repaired",
                        format!(
                            "source shard was stale or corrupt and was rebuilt before answering: {repair_reason}"
                        ),
                    )
                    .with_corpus(SearchCorpus::Source)
                    .with_path(".maestro/index/search/source.shard")
                    .with_retryable(false)],
                }),
                Err(error) => Err(SearchDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: "source_shard_unavailable".to_string(),
                    message: format!("source shard unavailable: {error}"),
                    corpus: Some(SearchCorpus::Source),
                    path: Some(".maestro/index/search/source.shard".to_string()),
                    retryable: Some(true),
                }),
            }
        }
    }
}

fn load_fresh(paths: &MaestroPaths) -> Result<SourceShard> {
    let bytes = std::fs::read(paths.source_shard_file()).context("failed to read source shard")?;
    let shard = decode_shard(&bytes)?;
    if shard.schema_version != SOURCE_SHARD_SCHEMA_VERSION {
        bail!("source shard schema is stale");
    }
    let mut skipped = Vec::new();
    if shard.manifest != source_manifest(paths, &mut skipped)? {
        bail!("source shard manifest is stale");
    }
    Ok(shard)
}

fn source_freshness(shard: &SourceShard, repaired: bool) -> SearchFreshness {
    SearchFreshness {
        corpus: SearchCorpus::Source,
        shard: ".maestro/index/search/source.shard".to_string(),
        fresh: true,
        repaired,
        schema_version: shard.schema_version.clone(),
        manifest_entries: shard.manifest.len(),
        vocabulary_version: intent::SYMBOLIC_VOCABULARY_VERSION.to_string(),
        artifact_graph_version: intent::ARTIFACT_GRAPH_VERSION.to_string(),
        outline_extractor_version: Some(outline::OUTLINE_EXTRACTOR_VERSION.to_string()),
        documents: None,
        indexed_files: Some(shard.files.len()),
        outline_entries: Some(shard.outline_entries.len()),
        ctags_symbols: Some(shard.symbols.len()),
        skipped_files: Some(shard.skipped.len()),
        skipped_by_reason: skipped_by_reason(&shard.skipped),
    }
}

fn search_shard(
    shard: &SourceShard,
    parsed: &ParsedQuery,
) -> Result<Vec<SearchHit>, SearchDiagnostic> {
    let case_sensitive = query::literal_case_sensitive(parsed);
    let mut hits = Vec::new();

    if wants_file_hits(parsed) {
        for file in &shard.files {
            if !source_filters_match(file, parsed)? {
                continue;
            }
            let searchable = file_search_text(file);
            if !evaluate_expr(&parsed.expr, &searchable, case_sensitive)? {
                continue;
            }
            let Some(first_match) = first_positive_match(file, parsed, case_sensitive)? else {
                continue;
            };
            let score = source_score(file, &first_match, parsed);
            hits.push(SearchHit {
                rank: 0,
                corpus: SearchCorpus::Source,
                kind: "file".to_string(),
                id: file.path.clone(),
                path: Some(file.path.clone()),
                line: Some(first_match.line),
                title: file.path.clone(),
                snippet: first_match.snippet,
                score,
                score_reasons: vec![
                    ScoreReason {
                        factor: first_match.factor.to_string(),
                        value: 1.0,
                        detail: "confirmed against stored source contents".to_string(),
                    },
                    ScoreReason {
                        factor: "trigram_shard".to_string(),
                        value: file.trigram_count as f64,
                        detail: "source shard stores positional trigram postings".to_string(),
                    },
                ],
                opener: Some(format!("{}:{}", file.path, first_match.line)),
                archived: false,
                feature: None,
                parent: None,
                symbol_kind: None,
                match_spans: vec![MatchSpan::Source {
                    line: first_match.line,
                    byte_start: first_match.byte_start,
                    byte_end: first_match.byte_end,
                }],
            });
        }
    }

    if wants_outline_hits(parsed) {
        for entry in &shard.outline_entries {
            let Some(file) = shard.files.iter().find(|file| file.path == entry.file) else {
                continue;
            };
            if !source_filters_match(file, parsed)? || !outline_type_matches(entry, parsed) {
                continue;
            }
            let searchable = outline_search_text(entry);
            if !evaluate_expr(&parsed.expr, &searchable, case_sensitive)? {
                continue;
            }
            let Some(first_match) =
                outline_first_match(entry, &searchable, parsed, case_sensitive)?
            else {
                continue;
            };
            hits.push(outline_hit(entry, &first_match, parsed));
        }
    }

    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
    for (idx, hit) in hits.iter_mut().enumerate() {
        hit.rank = idx + 1;
    }
    Ok(hits)
}

fn search_symbols(
    shard: &SourceShard,
    parsed: &ParsedQuery,
) -> Result<Vec<SearchHit>, SearchDiagnostic> {
    if !shard.ctags_status.available {
        return Err(SearchDiagnostic::error(
            "ctags_unavailable",
            format!(
                "{}; install universal-ctags and run `maestro index rebuild --source` for sym:",
                shard.ctags_status.message
            ),
        ));
    }
    let Some(symbol) = parsed.filters.sym.as_deref() else {
        return Ok(Vec::new());
    };
    let mut hits = Vec::new();
    for symbol_entry in &shard.symbols {
        if !symbol_entry.name.eq_ignore_ascii_case(symbol) {
            continue;
        }
        if let Some(file) = shard
            .files
            .iter()
            .find(|file| file.path == symbol_entry.path)
            && !source_filters_match(file, parsed)?
        {
            continue;
        }
        hits.push(SearchHit {
            rank: 0,
            corpus: SearchCorpus::Source,
            kind: "symbol".to_string(),
            id: format!(
                "{}:{}:{}",
                symbol_entry.path, symbol_entry.kind, symbol_entry.name
            ),
            path: Some(symbol_entry.path.clone()),
            line: Some(symbol_entry.line),
            title: symbol_entry.name.clone(),
            snippet: symbol_entry
                .signature
                .clone()
                .unwrap_or_else(|| symbol_entry.name.clone()),
            score: 0.95,
            score_reasons: vec![ScoreReason {
                factor: "ctags_definition".to_string(),
                value: 1.0,
                detail: "definition returned from universal-ctags JSON output".to_string(),
            }],
            opener: Some(format!("{}:{}", symbol_entry.path, symbol_entry.line)),
            archived: false,
            feature: None,
            parent: None,
            symbol_kind: Some(symbol_entry.kind.clone()),
            match_spans: vec![MatchSpan::Source {
                line: symbol_entry.line,
                byte_start: 0,
                byte_end: 0,
            }],
        });
    }
    hits.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.line.cmp(&b.line)));
    for (idx, hit) in hits.iter_mut().enumerate() {
        hit.rank = idx + 1;
    }
    Ok(hits)
}

fn source_filters_match(file: &SourceFile, parsed: &ParsedQuery) -> Result<bool, SearchDiagnostic> {
    if let Some(lang) = &parsed.filters.lang
        && lang != &file.language
    {
        return Ok(false);
    }
    if !parsed.filters.file_globs.is_empty()
        && !parsed
            .filters
            .file_globs
            .iter()
            .any(|pattern| glob_matches(pattern, &file.path))
    {
        return Ok(false);
    }
    if parsed
        .filters
        .excluded_file_globs
        .iter()
        .any(|pattern| glob_matches(pattern, &file.path))
    {
        return Ok(false);
    }
    Ok(true)
}

fn wants_file_hits(parsed: &ParsedQuery) -> bool {
    parsed.filters.kinds.is_empty() || parsed.filters.kinds.iter().any(|kind| kind == "file")
}

fn wants_outline_hits(parsed: &ParsedQuery) -> bool {
    parsed
        .filters
        .kinds
        .iter()
        .any(|kind| OUTLINE_KINDS.contains(&kind.as_str()))
}

fn outline_type_matches(entry: &OutlineEntry, parsed: &ParsedQuery) -> bool {
    parsed.filters.kinds.is_empty()
        || parsed
            .filters
            .kinds
            .iter()
            .any(|kind| kind == &entry.outline_kind)
}

fn outline_search_text(entry: &OutlineEntry) -> String {
    let mut text = format!(
        "{} {} {} {}",
        entry.name, entry.outline_kind, entry.signature, entry.file
    );
    if let Some(parent) = &entry.parent {
        text.push(' ');
        text.push_str(parent);
    }
    for member in &entry.members {
        text.push(' ');
        text.push_str(member);
    }
    text
}

fn outline_first_match(
    entry: &OutlineEntry,
    searchable: &str,
    parsed: &ParsedQuery,
    case_sensitive: bool,
) -> Result<Option<SourceMatch>, SearchDiagnostic> {
    for pattern in &parsed.regexes {
        let regex = regex_for(pattern, case_sensitive)?;
        if let Some(mat) = regex.find(searchable) {
            return Ok(Some(outline_match(entry, mat.start(), mat.end(), "regex")));
        }
    }
    for term in &parsed.terms {
        if let Some((start, end)) = find_literal(searchable, term, case_sensitive) {
            return Ok(Some(outline_match(entry, start, end, "outline")));
        }
    }
    Ok(None)
}

fn outline_match(
    entry: &OutlineEntry,
    relative_start: usize,
    relative_end: usize,
    factor: &'static str,
) -> SourceMatch {
    SourceMatch {
        byte_start: relative_start,
        byte_end: relative_end,
        line: entry.range.start_line,
        snippet: entry.signature.clone(),
        factor,
    }
}

fn outline_hit(entry: &OutlineEntry, first_match: &SourceMatch, parsed: &ParsedQuery) -> SearchHit {
    let mut score: f64 = 0.70;
    if parsed
        .terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case(&entry.name))
    {
        score += 0.15;
    }
    if parsed
        .filters
        .kinds
        .iter()
        .any(|kind| kind == &entry.outline_kind)
    {
        score += 0.10;
    }
    if entry.exported {
        score += 0.03;
    }
    SearchHit {
        rank: 0,
        corpus: SearchCorpus::Source,
        kind: "outline".to_string(),
        id: format!("{}:{}:{}", entry.file, entry.outline_kind, entry.name),
        path: Some(entry.file.clone()),
        line: Some(entry.range.start_line),
        title: entry.name.clone(),
        snippet: entry.signature.clone(),
        score: score.min(1.0),
        score_reasons: vec![
            ScoreReason {
                factor: first_match.factor.to_string(),
                value: 1.0,
                detail: "confirmed against source outline entry".to_string(),
            },
            ScoreReason {
                factor: "outline_kind".to_string(),
                value: 1.0,
                detail: entry.outline_kind.clone(),
            },
        ],
        opener: Some(format!("{}:{}", entry.file, entry.range.start_line)),
        archived: false,
        feature: None,
        parent: entry.parent.clone(),
        symbol_kind: Some(entry.outline_kind.clone()),
        match_spans: vec![MatchSpan::Source {
            line: first_match.line,
            byte_start: first_match.byte_start,
            byte_end: first_match.byte_end,
        }],
    }
}

fn evaluate_expr(
    expr: &QueryExpr,
    contents: &str,
    case_sensitive: bool,
) -> Result<bool, SearchDiagnostic> {
    match expr {
        QueryExpr::Atom(QueryAtom::Literal(term)) => {
            Ok(find_literal(contents, term, case_sensitive).is_some())
        }
        QueryExpr::Atom(QueryAtom::Regex(pattern)) => {
            Ok(regex_for(pattern, case_sensitive)?.find(contents).is_some())
        }
        QueryExpr::Not(inner) => Ok(!evaluate_expr(inner, contents, case_sensitive)?),
        QueryExpr::And(items) => {
            for item in items {
                if !evaluate_expr(item, contents, case_sensitive)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        QueryExpr::Or(items) => {
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
    file: &SourceFile,
    parsed: &ParsedQuery,
    case_sensitive: bool,
) -> Result<Option<SourceMatch>, SearchDiagnostic> {
    for pattern in &parsed.regexes {
        let regex = regex_for(pattern, case_sensitive)?;
        if let Some(mat) = regex.find(&file.contents) {
            return Ok(Some(source_match(file, mat.start(), mat.end(), "regex")));
        }
        if let Some(mat) = regex.find(&file.path) {
            return Ok(Some(path_match(file, mat.start(), mat.end())));
        }
    }
    for term in &parsed.terms {
        if is_path_like_term(term)
            && let Some((start, end)) = find_literal(&file.path, term, case_sensitive)
        {
            return Ok(Some(path_match(file, start, end)));
        }
        if let Some((start, end)) = find_literal(&file.contents, term, case_sensitive) {
            return Ok(Some(source_match(file, start, end, "lexical")));
        }
        if let Some((start, end)) = find_literal(&file.path, term, case_sensitive) {
            return Ok(Some(path_match(file, start, end)));
        }
    }
    Ok(None)
}

fn is_path_like_term(term: &str) -> bool {
    term.contains('/') || std::path::Path::new(term).extension().is_some()
}

fn file_search_text(file: &SourceFile) -> String {
    format!("{}\n{}", file.path, file.contents)
}

fn source_match(
    file: &SourceFile,
    byte_start: usize,
    byte_end: usize,
    factor: &'static str,
) -> SourceMatch {
    let line = line_for_byte(&file.line_offsets, byte_start);
    let line_start = line_start_for_byte(&file.line_offsets, byte_start);
    SourceMatch {
        byte_start: byte_start.saturating_sub(line_start),
        byte_end: byte_end.saturating_sub(line_start),
        line,
        snippet: line_snippet(&file.contents, byte_start),
        factor,
    }
}

fn path_match(file: &SourceFile, byte_start: usize, byte_end: usize) -> SourceMatch {
    SourceMatch {
        byte_start,
        byte_end,
        line: 1,
        snippet: file.path.clone(),
        factor: "exact_path",
    }
}

fn source_score(file: &SourceFile, first_match: &SourceMatch, parsed: &ParsedQuery) -> f64 {
    let mut score: f64 = 0.55;
    if parsed
        .filters
        .file_globs
        .iter()
        .any(|pattern| glob_matches(pattern, &file.path))
    {
        score += 0.15;
    }
    if parsed
        .filters
        .lang
        .as_ref()
        .is_some_and(|lang| lang == &file.language)
    {
        score += 0.15;
    }
    if first_match.factor == "regex" {
        score += 0.10;
    }
    if first_match.factor == "exact_path" {
        score += 0.35;
    }
    score.min(1.0)
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

fn line_offsets(contents: &str) -> Vec<LineOffset> {
    let mut offsets = vec![LineOffset {
        line: 1,
        byte_start: 0,
    }];
    for (idx, ch) in contents.char_indices() {
        if ch == '\n' && idx + 1 < contents.len() {
            offsets.push(LineOffset {
                line: offsets.len() as u64 + 1,
                byte_start: idx + 1,
            });
        }
    }
    offsets
}

fn line_for_byte(offsets: &[LineOffset], byte: usize) -> u64 {
    let mut line = 1;
    for offset in offsets {
        if offset.byte_start > byte {
            break;
        }
        line = offset.line;
    }
    line
}

fn line_start_for_byte(offsets: &[LineOffset], byte: usize) -> usize {
    let mut line_start = 0;
    for offset in offsets {
        if offset.byte_start > byte {
            break;
        }
        line_start = offset.byte_start;
    }
    line_start
}

fn trigrams(contents: &str) -> Vec<(String, usize)> {
    let chars: Vec<(usize, char)> = contents.char_indices().collect();
    let mut result = Vec::new();
    for window in chars.windows(3) {
        let trigram = window
            .iter()
            .flat_map(|(_, ch)| ch.to_lowercase())
            .collect::<String>();
        result.push((trigram, window[0].0));
    }
    result
}

fn encode_shard(shard: &SourceShard) -> Result<Vec<u8>> {
    let mut bytes = SOURCE_SHARD_MAGIC.to_vec();
    let payload = serde_json::to_vec(shard).context("failed to serialize source shard")?;
    bytes.extend(payload);
    Ok(bytes)
}

fn decode_shard(bytes: &[u8]) -> Result<SourceShard> {
    let Some(payload) = bytes.strip_prefix(SOURCE_SHARD_MAGIC) else {
        bail!("source shard magic header is missing");
    };
    serde_json::from_slice(payload).context("failed to parse source shard")
}

fn source_manifest(
    paths: &MaestroPaths,
    skipped: &mut Vec<SourceSkip>,
) -> Result<Vec<ManifestEntry>> {
    let repo = git2::Repository::discover(paths.repo_root()).ok();
    let mut entries = Vec::new();
    collect_source_files(
        paths,
        paths.repo_root(),
        repo.as_ref(),
        skipped,
        &mut entries,
    )?;
    entries.sort();
    Ok(entries)
}

fn collect_source_files(
    paths: &MaestroPaths,
    dir: &Path,
    repo: Option<&git2::Repository>,
    skipped: &mut Vec<SourceSkip>,
    entries: &mut Vec<ManifestEntry>,
) -> Result<()> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in read_dir {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        let rel = relative_label(&path, paths.repo_root());
        if metadata.is_dir() {
            if is_system_dir(&path) {
                continue;
            }
            if is_vendor_path(&path) {
                skipped.push(SourceSkip {
                    path: rel,
                    reason: "vendor".to_string(),
                });
                continue;
            }
            if is_generated_path(&path) {
                skipped.push(SourceSkip {
                    path: rel,
                    reason: "generated".to_string(),
                });
                continue;
            }
            collect_source_files(paths, &path, repo, skipped, entries)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        if repo
            .and_then(|repo| repo.status_should_ignore(Path::new(&rel)).ok())
            .unwrap_or(false)
        {
            skipped.push(SourceSkip {
                path: rel,
                reason: "gitignored".to_string(),
            });
            continue;
        }
        if is_vendor_path(&path) {
            skipped.push(SourceSkip {
                path: rel,
                reason: "vendor".to_string(),
            });
            continue;
        }
        if is_generated_path(&path) {
            skipped.push(SourceSkip {
                path: rel,
                reason: "generated".to_string(),
            });
            continue;
        }
        if metadata.len() > MAX_SOURCE_BYTES {
            skipped.push(SourceSkip {
                path: rel,
                reason: "oversized".to_string(),
            });
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("failed to inspect source candidate {}", path.display()))?;
        if bytes.contains(&0) {
            skipped.push(SourceSkip {
                path: rel,
                reason: "binary".to_string(),
            });
            continue;
        }
        if std::str::from_utf8(&bytes).is_err() {
            skipped.push(SourceSkip {
                path: rel,
                reason: "binary".to_string(),
            });
            continue;
        }
        entries.push(ManifestEntry {
            path: rel,
            mtime_ns: modified_ns(&metadata),
            len: metadata.len(),
        });
    }
    Ok(())
}

fn is_system_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".maestro" | "target" | "node_modules" | ".direnv" | ".DS_Store"
            )
        })
}

fn is_vendor_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if matches!(
                    name.to_str(),
                    Some("vendor" | "third_party" | "node_modules" | "bower_components")
                )
        )
    })
}

fn is_generated_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if name
                    .to_str()
                    .is_some_and(|part| matches!(part, "generated" | "dist" | "build"))
        )
    }) || path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.contains(".generated.")
                || name.ends_with(".min.js")
                || name.ends_with(".bundle.js")
                || name.ends_with(".pb.rs")
        })
}

fn language_for_path(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("ts" | "tsx") => "typescript",
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript",
        Some("py") => "python",
        Some("md" | "mdx") => "markdown",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        Some("json") => "json",
        Some("sh" | "bash" | "zsh") => "shell",
        _ => "text",
    }
}

fn collect_ctags(
    paths: &MaestroPaths,
    manifest: &[ManifestEntry],
) -> (CtagsStatus, Vec<SymbolEntry>) {
    let status = current_ctags_status();
    if !status.available {
        return (status, Vec::new());
    }

    let mut command = Command::new("ctags");
    command
        .current_dir(paths.repo_root())
        .arg("--output-format=json")
        .arg("--fields=+n")
        .arg("-f")
        .arg("-");
    for entry in manifest {
        command.arg(&entry.path);
    }

    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            return (
                CtagsStatus {
                    available: false,
                    message: format!("ctags failed to run: {error}"),
                },
                Vec::new(),
            );
        }
    };
    if !output.status.success() {
        return (
            CtagsStatus {
                available: false,
                message: format!(
                    "ctags failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            },
            Vec::new(),
        );
    }

    let mut symbols = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(name) = value.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(path) = value.get("path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("symbol")
            .to_string();
        symbols.push(SymbolEntry {
            path: relative_label(Path::new(path), paths.repo_root()),
            name: name.to_string(),
            kind,
            line: value
                .get("line")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1),
            signature: value
                .get("pattern")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        });
    }

    (
        CtagsStatus {
            available: true,
            message: "universal-ctags available".to_string(),
        },
        symbols,
    )
}

fn current_ctags_status() -> CtagsStatus {
    match Command::new("ctags").arg("--version").output() {
        Ok(output) if output.status.success() => CtagsStatus {
            available: true,
            message: "universal-ctags available".to_string(),
        },
        Ok(output) => CtagsStatus {
            available: false,
            message: format!(
                "ctags not usable: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        },
        Err(_) => CtagsStatus {
            available: false,
            message: "ctags optional missing".to_string(),
        },
    }
}

fn report(
    indexed_files: usize,
    outline_entries: usize,
    ctags_symbols: usize,
    ctags_status: CtagsStatus,
    skipped: &[SourceSkip],
) -> SourceRebuildReport {
    SourceRebuildReport {
        indexed_files,
        outline_entries,
        ctags_symbols,
        ctags_status,
        skipped_files: skipped.len(),
        skipped_by_reason: skipped_by_reason(skipped),
        representative_skips: skipped.iter().take(8).cloned().collect(),
    }
}

fn skipped_by_reason(skipped: &[SourceSkip]) -> BTreeMap<String, usize> {
    let mut by_reason = BTreeMap::new();
    for skip in skipped {
        *by_reason.entry(skip.reason.clone()).or_insert(0) += 1;
    }
    by_reason
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }
    if !pattern.contains(['*', '?', '[', ']']) {
        let prefix = pattern.trim_end_matches('/');
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    let regex = glob_regex(pattern);
    Regex::new(&regex).is_ok_and(|regex| regex.is_match(path))
}

fn glob_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        match chars[idx] {
            '*' if chars.get(idx + 1) == Some(&'*') => {
                regex.push_str(".*");
                idx += 2;
            }
            '*' => {
                regex.push_str("[^/]*");
                idx += 1;
            }
            '?' => {
                regex.push_str("[^/]");
                idx += 1;
            }
            ch => {
                regex.push_str(&regex::escape(&ch.to_string()));
                idx += 1;
            }
        }
    }
    regex.push('$');
    regex
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
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_search_returns_original_byte_offsets() {
        let text = "fn café_server() {}\n";
        let found = find_literal(text, "CAFÉ", false).expect("unicode case-insensitive match");
        assert_eq!(&text[found.0..found.1], "café");
        assert_eq!(found.0, 3);
        assert_eq!(
            find_literal("tiny suffix s", "src/domain/search/source.rs", false),
            None
        );
    }

    #[test]
    fn glob_supports_single_and_multi_segment_wildcards() {
        assert!(glob_matches("src/*.rs", "src/lib.rs"));
        assert!(!glob_matches("src/*.rs", "src/bin/main.rs"));
        assert!(glob_matches("src/**/*.rs", "src/bin/main.rs"));
    }
}
