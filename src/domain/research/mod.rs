//! Research implementation seam over frozen evidence contracts.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Research before the Stage 6 Action adapter is integrated"
)]

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use crate::domain::authority::AdmittedRepositoryActionV1;
use crate::domain::evidence::ObservationRecordIdV1;
use crate::domain::intake::SourceArtifactIdV1;

const QUESTION_DOMAIN_V1: &[u8] = b"maestro.vnext.research-question.v1";
const REVISION_DOMAIN_V1: &[u8] = b"maestro.vnext.research-question-revision.v1";
const SYNTHESIS_DOMAIN_V1: &[u8] = b"maestro.vnext.research-synthesis.v1";
const DISPOSITION_DOMAIN_V1: &[u8] = b"maestro.vnext.research-disposition.v1";
const BEGIN_RESEARCH_QUESTION_TAG_V1: u64 = 142;
const APPEND_RESEARCH_QUESTION_REVISION_TAG_V1: u64 = 143;
const PUBLISH_RESEARCH_SYNTHESIS_TAG_V1: u64 = 144;
const DISPOSE_RESEARCH_QUESTION_TAG_V1: u64 = 145;
const MAX_SOURCES_V1: usize = 512;
const MAX_CITATIONS_V1: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResearchQuestionIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResearchQuestionRevisionIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResearchSynthesisIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResearchDispositionIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ResearchSourceRefV1 {
    Intake(SourceArtifactIdV1),
    Evidence(ObservationRecordIdV1),
}

impl ResearchSourceRefV1 {
    fn digest(self) -> [u8; 32] {
        match self {
            Self::Intake(source) => *source.as_bytes(),
            Self::Evidence(observation) => *observation.as_bytes(),
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::Intake(_) => 1,
            Self::Evidence(_) => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResearchFreshnessV1 {
    Current,
    Stale,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResearchUncertaintyV1 {
    Bounded,
    Contradictory,
    Insufficient,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchQuestionRevisionV1 {
    id: ResearchQuestionRevisionIdV1,
    predecessor: Option<ResearchQuestionRevisionIdV1>,
    question_commitment: [u8; 32],
    source_set: Vec<ResearchSourceRefV1>,
    freshness: ResearchFreshnessV1,
    uncertainty: ResearchUncertaintyV1,
}

impl ResearchQuestionRevisionV1 {
    pub(crate) fn new(
        predecessor: Option<ResearchQuestionRevisionIdV1>,
        question_commitment: [u8; 32],
        mut source_set: Vec<ResearchSourceRefV1>,
        freshness: ResearchFreshnessV1,
        uncertainty: ResearchUncertaintyV1,
    ) -> Result<Self, ResearchErrorV1> {
        require_nonzero(question_commitment)?;
        if source_set.len() > MAX_SOURCES_V1 {
            return Err(ResearchErrorV1::BoundExceeded);
        }
        source_set.sort();
        source_set.dedup();
        let source_hash = source_set_hash(&source_set);
        let predecessor_ref = predecessor.map_or([0; 32], |revision| revision.0);
        let id = ResearchQuestionRevisionIdV1(hash_fields(&[
            REVISION_DOMAIN_V1,
            &predecessor_ref,
            &question_commitment,
            &source_hash,
            &[freshness as u8],
            &[uncertainty as u8],
        ]));
        Ok(Self {
            id,
            predecessor,
            question_commitment,
            source_set,
            freshness,
            uncertainty,
        })
    }

    pub(crate) const fn id(&self) -> ResearchQuestionRevisionIdV1 {
        self.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchQuestionV1 {
    id: ResearchQuestionIdV1,
    revisions: Vec<ResearchQuestionRevisionV1>,
    synthesis_ids: Vec<ResearchSynthesisIdV1>,
    disposition: Option<ResearchQuestionDispositionV1>,
}

impl ResearchQuestionV1 {
    pub(crate) fn begin(
        initial_revision: ResearchQuestionRevisionV1,
    ) -> Result<Self, ResearchErrorV1> {
        if initial_revision.predecessor.is_some() {
            return Err(ResearchErrorV1::InvalidRevision);
        }
        let id = ResearchQuestionIdV1(hash_fields(&[
            QUESTION_DOMAIN_V1,
            &initial_revision.id.0,
            &initial_revision.question_commitment,
        ]));
        Ok(Self {
            id,
            revisions: vec![initial_revision],
            synthesis_ids: Vec::new(),
            disposition: None,
        })
    }

    pub(crate) const fn id(&self) -> ResearchQuestionIdV1 {
        self.id
    }

    pub(crate) fn current_revision(&self) -> &ResearchQuestionRevisionV1 {
        self.revisions
            .last()
            .expect("invariant: a Research question always has an initial revision")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchCitationV1 {
    source: ResearchSourceRefV1,
    claim_commitment: [u8; 32],
}

impl ResearchCitationV1 {
    pub(crate) fn new(
        source: ResearchSourceRefV1,
        claim_commitment: [u8; 32],
    ) -> Result<Self, ResearchErrorV1> {
        require_nonzero(claim_commitment)?;
        Ok(Self {
            source,
            claim_commitment,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchSynthesisV1 {
    id: ResearchSynthesisIdV1,
    question_id: ResearchQuestionIdV1,
    question_revision_id: ResearchQuestionRevisionIdV1,
    synthesis_commitment: [u8; 32],
    citations: Vec<ResearchCitationV1>,
    freshness: ResearchFreshnessV1,
    uncertainty: ResearchUncertaintyV1,
}

impl ResearchSynthesisV1 {
    pub(crate) fn new(
        question_id: ResearchQuestionIdV1,
        revision: &ResearchQuestionRevisionV1,
        synthesis_commitment: [u8; 32],
        mut citations: Vec<ResearchCitationV1>,
        freshness: ResearchFreshnessV1,
        uncertainty: ResearchUncertaintyV1,
    ) -> Result<Self, ResearchErrorV1> {
        require_nonzero(synthesis_commitment)?;
        if citations.is_empty() || citations.len() > MAX_CITATIONS_V1 {
            return Err(ResearchErrorV1::InvalidCitationClosure);
        }
        citations.sort_by_key(|citation| (citation.source, citation.claim_commitment));
        citations.dedup_by_key(|citation| (citation.source, citation.claim_commitment));
        if citations
            .iter()
            .any(|citation| !revision.source_set.contains(&citation.source))
        {
            return Err(ResearchErrorV1::InvalidCitationClosure);
        }
        let citation_hash = citation_set_hash(&citations);
        let id = ResearchSynthesisIdV1(hash_fields(&[
            SYNTHESIS_DOMAIN_V1,
            &question_id.0,
            &revision.id.0,
            &synthesis_commitment,
            &citation_hash,
            &[freshness as u8],
            &[uncertainty as u8],
        ]));
        Ok(Self {
            id,
            question_id,
            question_revision_id: revision.id,
            synthesis_commitment,
            citations,
            freshness,
            uncertainty,
        })
    }

    pub(crate) const fn id(&self) -> ResearchSynthesisIdV1 {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResearchQuestionDispositionKindV1 {
    Satisfied,
    Blocked,
    Abandoned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchQuestionDispositionV1 {
    id: ResearchDispositionIdV1,
    kind: ResearchQuestionDispositionKindV1,
    terminal_revision_id: ResearchQuestionRevisionIdV1,
    authorization_receipt_ref: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchLedgerV1 {
    snapshot_ref: [u8; 32],
    questions: BTreeMap<ResearchQuestionIdV1, ResearchQuestionV1>,
    syntheses: BTreeMap<ResearchSynthesisIdV1, ResearchSynthesisV1>,
}

impl ResearchLedgerV1 {
    pub(crate) fn empty(snapshot_ref: [u8; 32]) -> Result<Self, ResearchErrorV1> {
        require_nonzero(snapshot_ref)?;
        Ok(Self {
            snapshot_ref,
            questions: BTreeMap::new(),
            syntheses: BTreeMap::new(),
        })
    }

    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    #[cfg(test)]
    pub(crate) fn begin_question(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        question: ResearchQuestionV1,
    ) -> Result<ResearchQuestionIdV1, ResearchErrorV1> {
        self.require_action(admitted, BEGIN_RESEARCH_QUESTION_TAG_V1)?;
        if self.questions.contains_key(&question.id) {
            return Err(ResearchErrorV1::DuplicateQuestion);
        }
        let id = question.id;
        self.questions.insert(id, question);
        self.advance(admitted);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn append_revision(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        question_id: ResearchQuestionIdV1,
        revision: ResearchQuestionRevisionV1,
    ) -> Result<ResearchQuestionRevisionIdV1, ResearchErrorV1> {
        self.require_action(admitted, APPEND_RESEARCH_QUESTION_REVISION_TAG_V1)?;
        let question = self
            .questions
            .get_mut(&question_id)
            .ok_or(ResearchErrorV1::UnknownQuestion)?;
        if question.disposition.is_some()
            || revision.predecessor != Some(question.current_revision().id)
            || question
                .revisions
                .iter()
                .any(|existing| existing.id == revision.id)
        {
            return Err(ResearchErrorV1::InvalidRevision);
        }
        let id = revision.id;
        question.revisions.push(revision);
        self.advance(admitted);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn publish_synthesis(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        synthesis: ResearchSynthesisV1,
    ) -> Result<ResearchSynthesisIdV1, ResearchErrorV1> {
        self.require_action(admitted, PUBLISH_RESEARCH_SYNTHESIS_TAG_V1)?;
        let question = self
            .questions
            .get_mut(&synthesis.question_id)
            .ok_or(ResearchErrorV1::UnknownQuestion)?;
        if question.disposition.is_some()
            || question.current_revision().id != synthesis.question_revision_id
            || self.syntheses.contains_key(&synthesis.id)
        {
            return Err(ResearchErrorV1::InvalidSynthesis);
        }
        let id = synthesis.id;
        question.synthesis_ids.push(id);
        self.syntheses.insert(id, synthesis);
        self.advance(admitted);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn dispose_question(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        question_id: ResearchQuestionIdV1,
        kind: ResearchQuestionDispositionKindV1,
    ) -> Result<ResearchDispositionIdV1, ResearchErrorV1> {
        self.require_action(admitted, DISPOSE_RESEARCH_QUESTION_TAG_V1)?;
        let question = self
            .questions
            .get_mut(&question_id)
            .ok_or(ResearchErrorV1::UnknownQuestion)?;
        if question.disposition.is_some() {
            return Err(ResearchErrorV1::QuestionAlreadyDisposed);
        }
        let terminal_revision_id = question.current_revision().id;
        let receipt_ref = *admitted.authorization_receipt().id().as_bytes();
        let id = ResearchDispositionIdV1(hash_fields(&[
            DISPOSITION_DOMAIN_V1,
            &question_id.0,
            &terminal_revision_id.0,
            &[kind as u8],
            &receipt_ref,
        ]));
        question.disposition = Some(ResearchQuestionDispositionV1 {
            id,
            kind,
            terminal_revision_id,
            authorization_receipt_ref: receipt_ref,
        });
        self.advance(admitted);
        Ok(id)
    }

    pub(crate) fn projection(&self) -> ResearchProjectionV1 {
        let questions = self
            .questions
            .values()
            .map(|question| ResearchQuestionProjectionV1 {
                question_id: question.id,
                revision_id: question.current_revision().id,
                freshness: question.current_revision().freshness,
                uncertainty: question.current_revision().uncertainty,
                terminal: question.disposition.is_some(),
                synthesis_count: question.synthesis_ids.len(),
                truth_status: ResearchTruthStatusV1::NonAuthoritativeDesignInput,
            })
            .collect();
        ResearchProjectionV1 {
            snapshot_ref: self.snapshot_ref,
            questions,
        }
    }

    #[cfg(test)]
    fn require_action(
        &self,
        admitted: &AdmittedRepositoryActionV1,
        expected_tag: u64,
    ) -> Result<(), ResearchErrorV1> {
        if admitted.action().global_tag() != expected_tag {
            return Err(ResearchErrorV1::WrongAuthorityAction);
        }
        if admitted.current_snapshot_id().as_bytes() != &self.snapshot_ref {
            return Err(ResearchErrorV1::StaleSnapshot);
        }
        Ok(())
    }

    #[cfg(test)]
    fn advance(&mut self, admitted: &AdmittedRepositoryActionV1) {
        self.snapshot_ref = *admitted.successor_snapshot().id().as_bytes();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResearchTruthStatusV1 {
    NonAuthoritativeDesignInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchQuestionProjectionV1 {
    question_id: ResearchQuestionIdV1,
    revision_id: ResearchQuestionRevisionIdV1,
    freshness: ResearchFreshnessV1,
    uncertainty: ResearchUncertaintyV1,
    terminal: bool,
    synthesis_count: usize,
    truth_status: ResearchTruthStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchProjectionV1 {
    snapshot_ref: [u8; 32],
    questions: Vec<ResearchQuestionProjectionV1>,
}

impl ResearchProjectionV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) fn questions(&self) -> &[ResearchQuestionProjectionV1] {
        &self.questions
    }
}

fn source_set_hash(sources: &[ResearchSourceRefV1]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((sources.len() as u64).to_be_bytes());
    for source in sources {
        hash.update([source.tag()]);
        hash.update(source.digest());
    }
    hash.finalize().into()
}

fn citation_set_hash(citations: &[ResearchCitationV1]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((citations.len() as u64).to_be_bytes());
    for citation in citations {
        hash.update([citation.source.tag()]);
        hash.update(citation.source.digest());
        hash.update(citation.claim_commitment);
    }
    hash.finalize().into()
}

fn hash_fields(fields: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for field in fields {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

fn require_nonzero(value: [u8; 32]) -> Result<(), ResearchErrorV1> {
    if value == [0; 32] {
        return Err(ResearchErrorV1::InvalidReference);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum ResearchErrorV1 {
    #[error("Research reference is invalid")]
    InvalidReference,
    #[error("Research bound is exceeded")]
    BoundExceeded,
    #[error("Research citation closure is invalid")]
    InvalidCitationClosure,
    #[error("Research question already exists")]
    DuplicateQuestion,
    #[error("Research question does not exist")]
    UnknownQuestion,
    #[error("Research question revision is invalid")]
    InvalidRevision,
    #[error("Research synthesis is invalid")]
    InvalidSynthesis,
    #[error("Research question is already disposed")]
    QuestionAlreadyDisposed,
    #[error("Authority admitted a different Research action")]
    WrongAuthorityAction,
    #[error("Research snapshot is stale")]
    StaleSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn source(byte: u8) -> ResearchSourceRefV1 {
        ResearchSourceRefV1::Evidence(ObservationRecordIdV1::from_bytes(digest(byte)).unwrap())
    }

    #[test]
    fn revision_identity_binds_sources_freshness_and_uncertainty() {
        let revision = ResearchQuestionRevisionV1::new(
            None,
            digest(1),
            vec![source(2)],
            ResearchFreshnessV1::Stale,
            ResearchUncertaintyV1::Contradictory,
        )
        .unwrap();
        assert_ne!(revision.id().0, [0; 32]);
        assert_eq!(revision.freshness, ResearchFreshnessV1::Stale);
    }

    #[test]
    fn synthesis_rejects_citation_outside_revision_source_set() {
        let revision = ResearchQuestionRevisionV1::new(
            None,
            digest(1),
            vec![source(2)],
            ResearchFreshnessV1::Current,
            ResearchUncertaintyV1::Bounded,
        )
        .unwrap();
        let question = ResearchQuestionV1::begin(revision.clone()).unwrap();
        let error = ResearchSynthesisV1::new(
            question.id(),
            &revision,
            digest(3),
            vec![ResearchCitationV1::new(source(4), digest(5)).unwrap()],
            ResearchFreshnessV1::Current,
            ResearchUncertaintyV1::Bounded,
        )
        .unwrap_err();
        assert_eq!(error, ResearchErrorV1::InvalidCitationClosure);
    }

    #[test]
    fn projection_marks_research_as_non_authoritative() {
        let ledger = ResearchLedgerV1::empty(digest(9)).unwrap();
        let projection = ledger.projection();
        assert_eq!(projection.snapshot_ref(), digest(9));
        assert!(projection.questions().is_empty());
    }

    #[test]
    fn question_refuses_a_non_root_initial_revision() {
        let predecessor = ResearchQuestionRevisionV1::new(
            None,
            digest(1),
            vec![source(2)],
            ResearchFreshnessV1::Current,
            ResearchUncertaintyV1::Bounded,
        )
        .unwrap();
        let revision = ResearchQuestionRevisionV1::new(
            Some(predecessor.id()),
            digest(3),
            vec![source(4)],
            ResearchFreshnessV1::Current,
            ResearchUncertaintyV1::Bounded,
        )
        .unwrap();
        assert_eq!(
            ResearchQuestionV1::begin(revision).unwrap_err(),
            ResearchErrorV1::InvalidRevision
        );
    }
}
mod legacy;

pub use legacy::*;
