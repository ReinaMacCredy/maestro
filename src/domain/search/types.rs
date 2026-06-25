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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
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
        }
    }

    pub fn error(query: &str, diagnostic: SearchDiagnostic) -> Self {
        Self {
            version: 1,
            schema: "maestro.grep.v1",
            ok: false,
            query: query.to_string(),
            intent: None,
            intent_confidence: None,
            intent_reasons: Vec::new(),
            explicit_filter_overrides: Vec::new(),
            partial: false,
            hits: Vec::new(),
            diagnostics: vec![diagnostic],
        }
    }
}
