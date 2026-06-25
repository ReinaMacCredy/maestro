use std::cmp::Ordering;

use crate::domain::search::query::ParsedQuery;
use crate::domain::search::types::{ScoreReason, SearchCorpus, SearchHit};

pub(crate) const SYMBOLIC_VOCABULARY_VERSION: &str = "maestro.symbolic-vocabulary.v1";
pub(crate) const ARTIFACT_GRAPH_VERSION: &str = "maestro.artifact-graph.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntentKind {
    Memory,
    Source,
    Ambiguous,
}

impl IntentKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Source => "source",
            Self::Ambiguous => "ambiguous",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct IntentDecision {
    pub(crate) kind: IntentKind,
    pub(crate) confidence: &'static str,
    pub(crate) reasons: Vec<String>,
    pub(crate) route: Option<SearchCorpus>,
}

pub(crate) fn classify(parsed: &ParsedQuery) -> IntentDecision {
    if let Some(corpus) = parsed.filters.corpus.as_deref() {
        let kind = if corpus == "source" {
            IntentKind::Source
        } else {
            IntentKind::Memory
        };
        return IntentDecision {
            kind,
            confidence: "high",
            reasons: vec!["explicit corpus filter".to_string()],
            route: Some(if corpus == "source" {
                SearchCorpus::Source
            } else {
                SearchCorpus::Memory
            }),
        };
    }

    let mut reasons = Vec::new();
    if parsed.filters.sym.is_some() {
        reasons.push("explicit sym: filter".to_string());
        return source_override(reasons);
    }
    if !parsed.filters.file_globs.is_empty() || !parsed.filters.excluded_file_globs.is_empty() {
        reasons.push("explicit file: filter".to_string());
        return source_override(reasons);
    }
    if parsed.filters.lang.is_some() {
        reasons.push("explicit lang: filter".to_string());
        return source_override(reasons);
    }
    if !parsed.regexes.is_empty() {
        reasons.push("regex atom searches source contents".to_string());
        return source_override(reasons);
    }
    if parsed.filters.kinds.iter().any(|kind| is_source_kind(kind)) {
        reasons.push("explicit source type: filter".to_string());
        return source_override(reasons);
    }
    if parsed.filters.kinds.iter().any(|kind| is_memory_kind(kind))
        || parsed.filters.status.is_some()
        || parsed.filters.feature.is_some()
        || parsed.filters.runtime.is_some()
        || parsed.filters.event.is_some()
    {
        reasons.push("explicit Maestro-memory filter".to_string());
        return IntentDecision {
            kind: IntentKind::Memory,
            confidence: "high",
            reasons,
            route: Some(SearchCorpus::Memory),
        };
    }

    let terms = query_terms(parsed);
    let code_reasons: Vec<String> = terms.iter().filter_map(|term| code_reason(term)).collect();
    let memory_reasons: Vec<String> = terms
        .iter()
        .filter_map(|term| memory_reason(term))
        .collect();

    if !code_reasons.is_empty() && memory_reasons.is_empty() {
        return IntentDecision {
            kind: IntentKind::Source,
            confidence: "medium",
            reasons: code_reasons,
            route: None,
        };
    }
    if !memory_reasons.is_empty() && code_reasons.is_empty() {
        return IntentDecision {
            kind: IntentKind::Memory,
            confidence: "medium",
            reasons: memory_reasons,
            route: None,
        };
    }

    let mut reasons = Vec::new();
    if code_reasons.is_empty() && memory_reasons.is_empty() {
        reasons.push("ambiguous lexical query".to_string());
    } else {
        reasons.extend(memory_reasons);
        reasons.extend(code_reasons);
    }
    reasons.push("mixed ranking keeps memory/source results together".to_string());
    IntentDecision {
        kind: IntentKind::Ambiguous,
        confidence: "low",
        reasons,
        route: None,
    }
}

pub(crate) fn rerank(mut hits: Vec<SearchHit>, kind: IntentKind) -> Vec<SearchHit> {
    for hit in &mut hits {
        apply_intent_boost(hit, kind);
    }
    hits.sort_by(|a, b| compare_hits(a, b, kind));
    for (idx, hit) in hits.iter_mut().enumerate() {
        hit.rank = idx + 1;
    }
    hits
}

pub(crate) fn is_source_kind(kind: &str) -> bool {
    matches!(
        kind,
        "file"
            | "function"
            | "method"
            | "struct"
            | "enum"
            | "trait"
            | "impl"
            | "field"
            | "import"
            | "export"
            | "module"
            | "symbol"
    )
}

pub(crate) fn is_memory_kind(kind: &str) -> bool {
    matches!(
        kind,
        "card" | "decision" | "feature" | "task" | "proof" | "qa" | "run_summary" | "run_evidence"
    )
}

fn source_override(reasons: Vec<String>) -> IntentDecision {
    IntentDecision {
        kind: IntentKind::Source,
        confidence: "high",
        reasons,
        route: Some(SearchCorpus::Source),
    }
}

fn query_terms(parsed: &ParsedQuery) -> Vec<String> {
    parsed
        .terms
        .iter()
        .flat_map(|term| term.split_whitespace())
        .map(|term| term.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '`'))
        .filter(|term| !term.is_empty())
        .map(str::to_string)
        .collect()
}

fn code_reason(term: &str) -> Option<String> {
    let lower = term.to_ascii_lowercase();
    if lower.contains('/') || source_extension(&lower) {
        return Some("path-like code-shaped query".to_string());
    }
    if term.contains("::") || term.contains('_') || term.ends_with("()") {
        return Some("code-shaped symbol query".to_string());
    }
    if has_inner_uppercase(term) {
        return Some("code-shaped camel-case query".to_string());
    }
    None
}

fn memory_reason(term: &str) -> Option<String> {
    let lower = term.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "why"
            | "what"
            | "when"
            | "where"
            | "who"
            | "how"
            | "happened"
            | "workflow"
            | "decision"
            | "decisions"
            | "proof"
            | "proofs"
            | "history"
            | "rationale"
            | "because"
            | "notes"
            | "note"
            | "qa"
            | "evidence"
    ) {
        return Some("natural-language Maestro-memory query".to_string());
    }
    None
}

fn source_extension(term: &str) -> bool {
    matches!(
        std::path::Path::new(term)
            .extension()
            .and_then(|ext| ext.to_str()),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "go"
                | "java"
                | "kt"
                | "swift"
                | "c"
                | "cc"
                | "cpp"
                | "h"
                | "hpp"
                | "md"
        )
    )
}

fn has_inner_uppercase(term: &str) -> bool {
    let mut chars = term.chars();
    let _ = chars.next();
    chars.any(char::is_uppercase) && term.chars().any(char::is_lowercase)
}

fn apply_intent_boost(hit: &mut SearchHit, kind: IntentKind) {
    let boost = match (kind, hit.corpus) {
        (IntentKind::Memory, SearchCorpus::Memory) => 0.18,
        (IntentKind::Source, SearchCorpus::Source) => 0.18,
        (IntentKind::Ambiguous, SearchCorpus::Memory) => 0.04,
        _ => 0.0,
    };
    if boost == 0.0 {
        return;
    }
    hit.score = (hit.score + boost).min(1.0);
    hit.score_reasons.push(ScoreReason {
        factor: "intent_boost".to_string(),
        value: boost,
        detail: format!(
            "{} intent favors {} corpus",
            kind.as_str(),
            hit.corpus.as_str()
        ),
    });
}

fn compare_hits(a: &SearchHit, b: &SearchHit, kind: IntentKind) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| corpus_priority(a.corpus, kind).cmp(&corpus_priority(b.corpus, kind)))
        .then_with(|| a.kind.cmp(&b.kind))
        .then_with(|| a.path.cmp(&b.path))
        .then_with(|| a.id.cmp(&b.id))
}

fn corpus_priority(corpus: SearchCorpus, kind: IntentKind) -> u8 {
    match (kind, corpus) {
        (IntentKind::Source, SearchCorpus::Source) => 0,
        (IntentKind::Source, SearchCorpus::Memory) => 1,
        (_, SearchCorpus::Memory) => 0,
        (_, SearchCorpus::Source) => 1,
    }
}
