use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchCorpus {
    Memory,
    Source,
}

impl SearchCorpus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Source => "source",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchDocument {
    pub id: String,
    pub corpus: SearchCorpus,
    pub kind: String,
    pub title: String,
    pub path: Option<String>,
    pub opener: Option<String>,
    pub archived: bool,
    pub feature: Option<String>,
    pub parent: Option<String>,
    pub fields: std::collections::BTreeMap<String, String>,
    pub segments: Vec<SearchSegment>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchSegment {
    pub id: String,
    pub field: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchHit {
    pub rank: usize,
    pub corpus: SearchCorpus,
    pub kind: String,
    pub id: String,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub title: String,
    pub snippet: String,
    pub score: f64,
    pub score_reasons: Vec<ScoreReason>,
    pub opener: Option<String>,
    pub archived: bool,
    pub feature: Option<String>,
    pub parent: Option<String>,
    pub symbol_kind: Option<String>,
    pub match_spans: Vec<MatchSpan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScoreReason {
    pub factor: String,
    pub value: f64,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MatchSpan {
    Source {
        line: u64,
        byte_start: usize,
        byte_end: usize,
    },
    Memory {
        segment_id: String,
        byte_start: usize,
        byte_end: usize,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corpus: Option<SearchCorpus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl SearchDiagnostic {
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            corpus: None,
            path: None,
            retryable: Some(false),
        }
    }

    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Info,
            code: code.to_string(),
            message: message.into(),
            corpus: None,
            path: None,
            retryable: None,
        }
    }

    pub fn with_corpus(mut self, corpus: SearchCorpus) -> Self {
        self.corpus = Some(corpus);
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchFreshness {
    pub corpus: SearchCorpus,
    pub shard: String,
    pub fresh: bool,
    pub repaired: bool,
    pub schema_version: String,
    pub manifest_entries: usize,
    pub vocabulary_version: String,
    pub artifact_graph_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_extractor_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_files: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_entries: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctags_symbols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_files: Option<usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skipped_by_reason: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct GrepEnvelope {
    pub version: u8,
    pub schema: &'static str,
    pub ok: bool,
    pub query: String,
    pub intent: Option<String>,
    pub intent_confidence: Option<String>,
    pub intent_reasons: Vec<String>,
    pub explicit_filter_overrides: Vec<String>,
    pub partial: bool,
    pub hits: Vec<SearchHit>,
    pub diagnostics: Vec<SearchDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SearchDiagnostic>,
    pub freshness: Vec<SearchFreshness>,
}

impl GrepEnvelope {
    pub fn success(query: &str, hits: Vec<SearchHit>, overrides: Vec<String>) -> Self {
        Self {
            version: 1,
            schema: "maestro.grep.v1",
            ok: true,
            query: query.to_string(),
            intent: Some(
                if hits.iter().all(|hit| hit.corpus == SearchCorpus::Source) {
                    "source"
                } else {
                    "memory"
                }
                .to_string(),
            ),
            intent_confidence: Some(
                if overrides.iter().any(|item| item == "corpus") {
                    "high"
                } else {
                    "medium"
                }
                .to_string(),
            ),
            intent_reasons: if overrides.iter().any(|item| item == "corpus") {
                vec!["explicit corpus filter".to_string()]
            } else if hits.iter().all(|hit| hit.corpus == SearchCorpus::Source) {
                vec!["source-shaped query uses source shard".to_string()]
            } else {
                vec!["memory shard available; source shard not enabled in this slice".to_string()]
            },
            explicit_filter_overrides: overrides,
            partial: false,
            hits,
            diagnostics: Vec::new(),
            error: None,
            freshness: Vec::new(),
        }
    }

    pub fn success_with_intent(
        query: &str,
        hits: Vec<SearchHit>,
        overrides: Vec<String>,
        intent: &str,
        confidence: &str,
        reasons: Vec<String>,
    ) -> Self {
        Self {
            version: 1,
            schema: "maestro.grep.v1",
            ok: true,
            query: query.to_string(),
            intent: Some(intent.to_string()),
            intent_confidence: Some(confidence.to_string()),
            intent_reasons: reasons,
            explicit_filter_overrides: overrides,
            partial: false,
            hits,
            diagnostics: Vec::new(),
            error: None,
            freshness: Vec::new(),
        }
    }

    pub fn error(query: &str, diagnostic: SearchDiagnostic) -> Self {
        Self::error_with_overrides(query, diagnostic, Vec::new())
    }

    pub fn error_with_overrides(
        query: &str,
        diagnostic: SearchDiagnostic,
        overrides: Vec<String>,
    ) -> Self {
        Self {
            version: 1,
            schema: "maestro.grep.v1",
            ok: false,
            query: query.to_string(),
            intent: None,
            intent_confidence: None,
            intent_reasons: Vec::new(),
            explicit_filter_overrides: overrides,
            partial: false,
            hits: Vec::new(),
            diagnostics: vec![diagnostic.clone()],
            error: Some(diagnostic),
            freshness: Vec::new(),
        }
    }

    pub fn with_freshness(mut self, freshness: Vec<SearchFreshness>) -> Self {
        self.freshness = freshness;
        self
    }
}
