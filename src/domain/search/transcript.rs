use crate::domain::search::query::ParsedQuery;
use crate::domain::search::types::{GrepEnvelope, SearchCorpus, SearchDiagnostic};

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
