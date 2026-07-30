//! Search implementation seam over frozen observation and projection contracts.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Search before the Stage 6 Projection adapter is integrated"
)]

use std::cmp::Reverse;
use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use crate::domain::authority::AdmittedRepositoryActionV1;

const INDEX_DOMAIN_V1: &[u8] = b"maestro.vnext.search-index-generation.v1";
const MAX_DOCUMENTS_V1: usize = 16_384;
const MAX_TERMS_PER_DOCUMENT_V1: usize = 1_024;
const MAX_QUERY_TERMS_V1: usize = 32;
const MAX_TERM_BYTES_V1: usize = 128;
const MAX_HITS_V1: usize = 256;
const REBUILD_SEARCH_INDEX_TAG_V1: u64 = 130;
const PURGE_SEARCH_INDEX_TAG_V1: u64 = 131;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SearchTargetRefV1 {
    identity: [u8; 32],
    revision: [u8; 32],
}

impl SearchTargetRefV1 {
    pub(crate) fn new(identity: [u8; 32], revision: [u8; 32]) -> Option<Self> {
        nonzero(identity)?;
        nonzero(revision)?;
        Some(Self { identity, revision })
    }

    pub(crate) const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn revision(self) -> [u8; 32] {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub(crate) enum SearchSourceKindV1 {
    Canonical = 0,
    Work = 1,
    Artifact = 2,
    Memory = 3,
    Transcript = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchConsentV1 {
    Admitted,
    Withdrawn,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchDocumentV1 {
    target: SearchTargetRefV1,
    source_kind: SearchSourceKindV1,
    consent: SearchConsentV1,
    normalized_terms: Vec<String>,
}

impl SearchDocumentV1 {
    pub(crate) fn new(
        target: SearchTargetRefV1,
        source_kind: SearchSourceKindV1,
        consent: SearchConsentV1,
        terms: impl IntoIterator<Item = String>,
    ) -> Result<Self, SearchErrorV1> {
        let normalized_terms = normalize_terms(terms, MAX_TERMS_PER_DOCUMENT_V1)?;
        if normalized_terms.is_empty() {
            return Err(SearchErrorV1::EmptyDocument);
        }
        Ok(Self {
            target,
            source_kind,
            consent,
            normalized_terms,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedSearchDocumentV1 {
    target: SearchTargetRefV1,
    source_kind: SearchSourceKindV1,
    normalized_terms: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchIndexGenerationV1 {
    id: [u8; 32],
    source_snapshot_ref: [u8; 32],
    consent_policy_ref: [u8; 32],
    redaction_policy_ref: [u8; 32],
    documents: Vec<IndexedSearchDocumentV1>,
}

impl SearchIndexGenerationV1 {
    pub(crate) fn rebuild(
        source_snapshot_ref: [u8; 32],
        consent_policy_ref: [u8; 32],
        redaction_policy_ref: [u8; 32],
        documents: impl IntoIterator<Item = SearchDocumentV1>,
    ) -> Result<Self, SearchErrorV1> {
        nonzero(source_snapshot_ref).ok_or(SearchErrorV1::InvalidReference)?;
        nonzero(consent_policy_ref).ok_or(SearchErrorV1::InvalidReference)?;
        nonzero(redaction_policy_ref).ok_or(SearchErrorV1::InvalidReference)?;
        let mut documents = documents
            .into_iter()
            .filter(|document| document.consent == SearchConsentV1::Admitted)
            .map(|document| IndexedSearchDocumentV1 {
                target: document.target,
                source_kind: document.source_kind,
                normalized_terms: document.normalized_terms,
            })
            .collect::<Vec<_>>();
        if documents.len() > MAX_DOCUMENTS_V1 {
            return Err(SearchErrorV1::BoundExceeded);
        }
        documents.sort_by_key(|document| document.target);
        if documents
            .windows(2)
            .any(|pair| pair[0].target == pair[1].target)
        {
            return Err(SearchErrorV1::DuplicateTarget);
        }
        let id = index_identity(
            source_snapshot_ref,
            consent_policy_ref,
            redaction_policy_ref,
            &documents,
        );
        Ok(Self {
            id,
            source_snapshot_ref,
            consent_policy_ref,
            redaction_policy_ref,
            documents,
        })
    }

    pub(crate) const fn id(&self) -> [u8; 32] {
        self.id
    }

    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.source_snapshot_ref
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchQueryV1 {
    snapshot_ref: [u8; 32],
    normalized_terms: Vec<String>,
    source_filter: BTreeSet<SearchSourceKindV1>,
    limit: usize,
}

impl SearchQueryV1 {
    pub(crate) fn new(
        snapshot_ref: [u8; 32],
        terms: impl IntoIterator<Item = String>,
        source_filter: impl IntoIterator<Item = SearchSourceKindV1>,
        limit: usize,
    ) -> Result<Self, SearchErrorV1> {
        nonzero(snapshot_ref).ok_or(SearchErrorV1::InvalidReference)?;
        let normalized_terms = normalize_terms(terms, MAX_QUERY_TERMS_V1)?;
        if normalized_terms.is_empty() || limit == 0 || limit > MAX_HITS_V1 {
            return Err(SearchErrorV1::InvalidQuery);
        }
        Ok(Self {
            snapshot_ref,
            normalized_terms,
            source_filter: source_filter.into_iter().collect(),
            limit,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchProjectionFreshnessV1 {
    Current,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchProofEligibilityMetadataV1 {
    RequiresEvidenceAdmission,
    NotEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchHitV1 {
    target: SearchTargetRefV1,
    source_kind: SearchSourceKindV1,
    matched_terms: Vec<String>,
    proof_eligibility: SearchProofEligibilityMetadataV1,
}

impl SearchHitV1 {
    pub(crate) const fn target(&self) -> SearchTargetRefV1 {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchProjectionV1 {
    index_generation_ref: [u8; 32],
    snapshot_ref: [u8; 32],
    freshness: SearchProjectionFreshnessV1,
    hits: Vec<SearchHitV1>,
}

impl SearchProjectionV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) const fn freshness(&self) -> SearchProjectionFreshnessV1 {
        self.freshness
    }

    pub(crate) fn hits(&self) -> &[SearchHitV1] {
        &self.hits
    }
}

pub(crate) fn project_search(
    index: &SearchIndexGenerationV1,
    query: &SearchQueryV1,
) -> SearchProjectionV1 {
    let mut hits = index
        .documents
        .iter()
        .filter(|document| {
            query.source_filter.is_empty() || query.source_filter.contains(&document.source_kind)
        })
        .filter_map(|document| {
            let matched_terms = query
                .normalized_terms
                .iter()
                .filter(|term| document.normalized_terms.binary_search(term).is_ok())
                .cloned()
                .collect::<Vec<_>>();
            (!matched_terms.is_empty()).then_some(SearchHitV1 {
                target: document.target,
                source_kind: document.source_kind,
                matched_terms,
                proof_eligibility: if document.source_kind == SearchSourceKindV1::Canonical {
                    SearchProofEligibilityMetadataV1::RequiresEvidenceAdmission
                } else {
                    SearchProofEligibilityMetadataV1::NotEvidence
                },
            })
        })
        .collect::<Vec<_>>();
    hits.sort_by_key(|hit| (Reverse(hit.matched_terms.len()), hit.target));
    hits.truncate(query.limit);
    SearchProjectionV1 {
        index_generation_ref: index.id,
        snapshot_ref: query.snapshot_ref,
        freshness: if index.source_snapshot_ref == query.snapshot_ref {
            SearchProjectionFreshnessV1::Current
        } else {
            SearchProjectionFreshnessV1::Stale
        },
        hits,
    }
}

#[cfg(test)]
pub(crate) fn authorized_rebuild(
    admitted: &AdmittedRepositoryActionV1,
    expected_current_snapshot_ref: [u8; 32],
    consent_policy_ref: [u8; 32],
    redaction_policy_ref: [u8; 32],
    documents: impl IntoIterator<Item = SearchDocumentV1>,
) -> Result<SearchIndexGenerationV1, SearchErrorV1> {
    require_action_and_snapshot(
        admitted,
        REBUILD_SEARCH_INDEX_TAG_V1,
        expected_current_snapshot_ref,
    )?;
    SearchIndexGenerationV1::rebuild(
        *admitted.successor_snapshot().id().as_bytes(),
        consent_policy_ref,
        redaction_policy_ref,
        documents,
    )
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SearchIndexPurgeV1 {
    purged_index_ref: [u8; 32],
    successor_snapshot_ref: [u8; 32],
}

#[cfg(test)]
pub(crate) fn authorized_purge(
    admitted: &AdmittedRepositoryActionV1,
    expected_current_snapshot_ref: [u8; 32],
    purged_index_ref: [u8; 32],
) -> Result<SearchIndexPurgeV1, SearchErrorV1> {
    require_action_and_snapshot(
        admitted,
        PURGE_SEARCH_INDEX_TAG_V1,
        expected_current_snapshot_ref,
    )?;
    nonzero(purged_index_ref).ok_or(SearchErrorV1::InvalidReference)?;
    Ok(SearchIndexPurgeV1 {
        purged_index_ref,
        successor_snapshot_ref: *admitted.successor_snapshot().id().as_bytes(),
    })
}

#[cfg(test)]
fn require_action_and_snapshot(
    admitted: &AdmittedRepositoryActionV1,
    expected_action_tag: u64,
    expected_current_snapshot_ref: [u8; 32],
) -> Result<(), SearchErrorV1> {
    if admitted.action().global_tag() != expected_action_tag {
        return Err(SearchErrorV1::WrongAuthorityAction);
    }
    if admitted.current_snapshot_id().as_bytes() != &expected_current_snapshot_ref {
        return Err(SearchErrorV1::StaleSnapshot);
    }
    Ok(())
}

fn normalize_terms(
    terms: impl IntoIterator<Item = String>,
    maximum: usize,
) -> Result<Vec<String>, SearchErrorV1> {
    let mut normalized = terms
        .into_iter()
        .map(|term| term.trim().to_lowercase())
        .collect::<Vec<_>>();
    if normalized.len() > maximum
        || normalized
            .iter()
            .any(|term| term.is_empty() || term.len() > MAX_TERM_BYTES_V1 || term.contains('\0'))
    {
        return Err(SearchErrorV1::InvalidTerm);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn index_identity(
    source_snapshot_ref: [u8; 32],
    consent_policy_ref: [u8; 32],
    redaction_policy_ref: [u8; 32],
    documents: &[IndexedSearchDocumentV1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(INDEX_DOMAIN_V1);
    hash.update(source_snapshot_ref);
    hash.update(consent_policy_ref);
    hash.update(redaction_policy_ref);
    hash.update((documents.len() as u64).to_be_bytes());
    for document in documents {
        hash.update(document.target.identity);
        hash.update(document.target.revision);
        hash.update([document.source_kind as u8]);
        hash.update((document.normalized_terms.len() as u64).to_be_bytes());
        for term in &document.normalized_terms {
            hash.update((term.len() as u64).to_be_bytes());
            hash.update(term.as_bytes());
        }
    }
    hash.finalize().into()
}

fn nonzero(value: [u8; 32]) -> Option<[u8; 32]> {
    (value != [0; 32]).then_some(value)
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum SearchErrorV1 {
    #[error("Search reference is invalid")]
    InvalidReference,
    #[error("Search term is invalid")]
    InvalidTerm,
    #[error("Search document is empty")]
    EmptyDocument,
    #[error("Search bound is exceeded")]
    BoundExceeded,
    #[error("Search target is duplicated")]
    DuplicateTarget,
    #[error("Search query is invalid")]
    InvalidQuery,
    #[error("Authority admitted a different Search action")]
    WrongAuthorityAction,
    #[error("Search snapshot is stale")]
    StaleSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn document(
        identity: u8,
        revision: u8,
        source_kind: SearchSourceKindV1,
        consent: SearchConsentV1,
        terms: &[&str],
    ) -> SearchDocumentV1 {
        SearchDocumentV1::new(
            SearchTargetRefV1::new(digest(identity), digest(revision)).unwrap(),
            source_kind,
            consent,
            terms.iter().map(|term| (*term).to_owned()),
        )
        .unwrap()
    }

    #[test]
    fn identity_hashed_discriminants_are_pinned() {
        assert_eq!(SearchSourceKindV1::Canonical as u8, 0);
        assert_eq!(SearchSourceKindV1::Transcript as u8, 4);
    }

    #[test]
    fn rebuild_is_deterministic_and_excludes_withdrawn_sources() {
        let first = document(
            1,
            11,
            SearchSourceKindV1::Canonical,
            SearchConsentV1::Admitted,
            &["Stage", "Eight"],
        );
        let withdrawn = document(
            2,
            12,
            SearchSourceKindV1::Transcript,
            SearchConsentV1::Withdrawn,
            &["stage"],
        );
        let forward = SearchIndexGenerationV1::rebuild(
            digest(20),
            digest(21),
            digest(22),
            [first.clone(), withdrawn.clone()],
        )
        .unwrap();
        let reverse = SearchIndexGenerationV1::rebuild(
            digest(20),
            digest(21),
            digest(22),
            [withdrawn, first],
        )
        .unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward.documents.len(), 1);
    }

    #[test]
    fn projection_is_typed_stale_aware_and_non_authoritative() {
        let index = SearchIndexGenerationV1::rebuild(
            digest(20),
            digest(21),
            digest(22),
            [document(
                1,
                11,
                SearchSourceKindV1::Canonical,
                SearchConsentV1::Admitted,
                &["stage", "eight"],
            )],
        )
        .unwrap();
        let query = SearchQueryV1::new(digest(23), ["stage".to_owned()], [], 10).unwrap();
        let projection = project_search(&index, &query);
        assert_eq!(projection.freshness(), SearchProjectionFreshnessV1::Stale);
        assert_eq!(projection.hits()[0].target().identity(), digest(1));
        assert_eq!(
            projection.hits()[0].proof_eligibility,
            SearchProofEligibilityMetadataV1::RequiresEvidenceAdmission
        );
    }
}
mod intent;
mod lock;
pub mod memory;
mod outline;
pub mod query;
pub mod source;
pub mod transcript;
pub mod types;

pub use lock::{SearchWriterLock, acquire_writer};
pub(crate) use memory::rebuild_memory_unlocked;
pub use memory::{MemoryRebuildReport, card_list_grep_candidates, grep_memory, rebuild_memory};
pub(crate) use source::rebuild_source_unlocked;
pub use source::{
    SourceIndexHealth, SourceRebuildReport, grep, grep_source, rebuild_source, source_index_health,
};
pub(crate) use transcript::rebuild_transcript_unlocked;
pub use transcript::{TranscriptIndexHealth, TranscriptRebuildReport, transcript_index_health};
pub use types::{GrepEnvelope, SearchDiagnostic, SearchHit};
