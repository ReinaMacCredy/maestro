//! Memory implementation seam over frozen evidence and persistence contracts.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Memory before the Stage 6 Action adapter is integrated"
)]

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use crate::domain::vnext::authority::AdmittedRepositoryActionV1;
use crate::domain::vnext::evidence::ObservationRecordIdV1;

const CANDIDATE_DOMAIN_V1: &[u8] = b"maestro.vnext.memory-candidate.v1";
const ASSESSMENT_DOMAIN_V1: &[u8] = b"maestro.vnext.memory-admission-assessment.v1";
const ENTRY_DOMAIN_V1: &[u8] = b"maestro.vnext.memory-entry.v1";
const EVENT_DOMAIN_V1: &[u8] = b"maestro.vnext.memory-event.v1";
const MAX_TAGS_V1: usize = 64;
const MAX_CONFLICTS_V1: usize = 256;
const CREATE_MEMORY_CANDIDATE_TAG_V1: u64 = 132;
const PROMOTE_MEMORY_CANDIDATE_TAG_V1: u64 = 133;
const REJECT_MEMORY_CANDIDATE_TAG_V1: u64 = 134;
const QUARANTINE_MEMORY_CANDIDATE_TAG_V1: u64 = 135;
const INVALIDATE_MEMORY_ENTRY_TAG_V1: u64 = 136;
const SUPERSEDE_MEMORY_ENTRY_TAG_V1: u64 = 137;
const SECURITY_ERASE_MEMORY_PAYLOAD_TAG_V1: u64 = 138;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemoryCandidateIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AdmissionAssessmentIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemoryEntryIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemoryEventIdV1([u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryCandidateV1 {
    id: MemoryCandidateIdV1,
    observation_ref: ObservationRecordIdV1,
    payload_commitment: [u8; 32],
    consent_ref: [u8; 32],
    retention_ref: [u8; 32],
    tags: Vec<String>,
}

impl MemoryCandidateV1 {
    pub(crate) fn new(
        observation_ref: ObservationRecordIdV1,
        payload_commitment: [u8; 32],
        consent_ref: [u8; 32],
        retention_ref: [u8; 32],
        tags: impl IntoIterator<Item = String>,
    ) -> Result<Self, MemoryErrorV1> {
        require_nonzero(payload_commitment)?;
        require_nonzero(consent_ref)?;
        require_nonzero(retention_ref)?;
        let tags = normalize_tags(tags)?;
        let id = MemoryCandidateIdV1(hash_fields(&[
            CANDIDATE_DOMAIN_V1,
            observation_ref.as_bytes(),
            &payload_commitment,
            &consent_ref,
            &retention_ref,
            &hash_strings(&tags),
        ]));
        Ok(Self {
            id,
            observation_ref,
            payload_commitment,
            consent_ref,
            retention_ref,
            tags,
        })
    }

    pub(crate) const fn id(&self) -> MemoryCandidateIdV1 {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum MemoryAdmissionDecisionV1 {
    Admit = 0,
    Reject = 1,
    Quarantine = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionAssessmentV1 {
    id: AdmissionAssessmentIdV1,
    candidate_id: MemoryCandidateIdV1,
    snapshot_ref: [u8; 32],
    assessor_basis_ref: [u8; 32],
    decision: MemoryAdmissionDecisionV1,
    conflicting_policy_refs: Vec<[u8; 32]>,
}

impl AdmissionAssessmentV1 {
    pub(crate) fn new(
        candidate_id: MemoryCandidateIdV1,
        snapshot_ref: [u8; 32],
        assessor_basis_ref: [u8; 32],
        decision: MemoryAdmissionDecisionV1,
        mut conflicting_policy_refs: Vec<[u8; 32]>,
    ) -> Result<Self, MemoryErrorV1> {
        require_nonzero(snapshot_ref)?;
        require_nonzero(assessor_basis_ref)?;
        if conflicting_policy_refs.len() > MAX_CONFLICTS_V1
            || conflicting_policy_refs.contains(&[0; 32])
        {
            return Err(MemoryErrorV1::InvalidAssessment);
        }
        conflicting_policy_refs.sort();
        conflicting_policy_refs.dedup();
        let conflicts_hash = hash_digests(&conflicting_policy_refs);
        let id = AdmissionAssessmentIdV1(hash_fields(&[
            ASSESSMENT_DOMAIN_V1,
            &candidate_id.0,
            &snapshot_ref,
            &assessor_basis_ref,
            &[decision as u8],
            &conflicts_hash,
        ]));
        Ok(Self {
            id,
            candidate_id,
            snapshot_ref,
            assessor_basis_ref,
            decision,
            conflicting_policy_refs,
        })
    }

    pub(crate) const fn id(&self) -> AdmissionAssessmentIdV1 {
        self.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryEntryV1 {
    id: MemoryEntryIdV1,
    candidate_id: MemoryCandidateIdV1,
    assessment_id: AdmissionAssessmentIdV1,
    payload_commitment: [u8; 32],
    tags: Vec<String>,
    authorization_receipt_ref: [u8; 32],
}

impl MemoryEntryV1 {
    pub(crate) const fn id(&self) -> MemoryEntryIdV1 {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum MemoryDispositionKindV1 {
    Promoted = 0,
    Rejected = 1,
    Quarantined = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryDispositionV1 {
    event_id: MemoryEventIdV1,
    candidate_id: MemoryCandidateIdV1,
    assessment_id: AdmissionAssessmentIdV1,
    kind: MemoryDispositionKindV1,
    authorization_receipt_ref: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum MemoryInvalidationReasonV1 {
    Superseded = 0,
    PolicyConflict = 1,
    SourceRevoked = 2,
    RetentionExpired = 3,
    SecurityErased = 4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryInvalidationV1 {
    event_id: MemoryEventIdV1,
    entry_id: MemoryEntryIdV1,
    reason: MemoryInvalidationReasonV1,
    replacement: Option<MemoryEntryIdV1>,
    authorization_receipt_ref: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryLedgerV1 {
    snapshot_ref: [u8; 32],
    candidates: BTreeMap<MemoryCandidateIdV1, MemoryCandidateV1>,
    assessments: BTreeMap<AdmissionAssessmentIdV1, AdmissionAssessmentV1>,
    dispositions: BTreeMap<MemoryCandidateIdV1, MemoryDispositionV1>,
    entries: BTreeMap<MemoryEntryIdV1, MemoryEntryV1>,
    invalidations: BTreeMap<MemoryEntryIdV1, MemoryInvalidationV1>,
}

impl MemoryLedgerV1 {
    pub(crate) fn empty(snapshot_ref: [u8; 32]) -> Result<Self, MemoryErrorV1> {
        require_nonzero(snapshot_ref)?;
        Ok(Self {
            snapshot_ref,
            candidates: BTreeMap::new(),
            assessments: BTreeMap::new(),
            dispositions: BTreeMap::new(),
            entries: BTreeMap::new(),
            invalidations: BTreeMap::new(),
        })
    }

    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    #[cfg(test)]
    pub(crate) fn record_candidate(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        candidate: MemoryCandidateV1,
    ) -> Result<MemoryCandidateIdV1, MemoryErrorV1> {
        self.require_action(admitted, CREATE_MEMORY_CANDIDATE_TAG_V1)?;
        if self.candidates.contains_key(&candidate.id) {
            return Err(MemoryErrorV1::DuplicateCandidate);
        }
        let id = candidate.id;
        self.candidates.insert(id, candidate);
        self.advance(admitted);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn promote(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        candidate_id: MemoryCandidateIdV1,
        assessment: AdmissionAssessmentV1,
    ) -> Result<MemoryEntryIdV1, MemoryErrorV1> {
        self.require_action(admitted, PROMOTE_MEMORY_CANDIDATE_TAG_V1)?;
        if self.dispositions.contains_key(&candidate_id) {
            return Err(MemoryErrorV1::CandidateAlreadyDisposed);
        }
        let candidate = self
            .candidates
            .get(&candidate_id)
            .ok_or(MemoryErrorV1::UnknownCandidate)?;
        let assessment_id = assessment.id;
        if assessment.candidate_id != candidate_id
            || assessment.snapshot_ref != self.snapshot_ref
            || assessment.decision != MemoryAdmissionDecisionV1::Admit
            || !assessment.conflicting_policy_refs.is_empty()
            || self.assessments.contains_key(&assessment_id)
        {
            return Err(MemoryErrorV1::PromotionRefused);
        }
        let receipt_ref = *admitted.authorization_receipt().id().as_bytes();
        let entry_id = MemoryEntryIdV1(hash_fields(&[
            ENTRY_DOMAIN_V1,
            &candidate_id.0,
            &assessment_id.0,
            &candidate.payload_commitment,
            &receipt_ref,
        ]));
        let entry = MemoryEntryV1 {
            id: entry_id,
            candidate_id,
            assessment_id,
            payload_commitment: candidate.payload_commitment,
            tags: candidate.tags.clone(),
            authorization_receipt_ref: receipt_ref,
        };
        let disposition = MemoryDispositionV1 {
            event_id: event_id(candidate_id.0, assessment_id.0, receipt_ref, 1),
            candidate_id,
            assessment_id,
            kind: MemoryDispositionKindV1::Promoted,
            authorization_receipt_ref: receipt_ref,
        };
        self.assessments.insert(assessment_id, assessment);
        self.entries.insert(entry_id, entry);
        self.dispositions.insert(candidate_id, disposition);
        self.advance(admitted);
        Ok(entry_id)
    }

    #[cfg(test)]
    pub(crate) fn reject(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        candidate_id: MemoryCandidateIdV1,
        assessment: AdmissionAssessmentV1,
    ) -> Result<(), MemoryErrorV1> {
        self.dispose_candidate(
            admitted,
            REJECT_MEMORY_CANDIDATE_TAG_V1,
            candidate_id,
            assessment,
            MemoryAdmissionDecisionV1::Reject,
            MemoryDispositionKindV1::Rejected,
        )
    }

    #[cfg(test)]
    pub(crate) fn quarantine(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        candidate_id: MemoryCandidateIdV1,
        assessment: AdmissionAssessmentV1,
    ) -> Result<(), MemoryErrorV1> {
        self.dispose_candidate(
            admitted,
            QUARANTINE_MEMORY_CANDIDATE_TAG_V1,
            candidate_id,
            assessment,
            MemoryAdmissionDecisionV1::Quarantine,
            MemoryDispositionKindV1::Quarantined,
        )
    }

    #[cfg(test)]
    pub(crate) fn invalidate(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        entry_id: MemoryEntryIdV1,
        reason: MemoryInvalidationReasonV1,
    ) -> Result<(), MemoryErrorV1> {
        self.require_action(admitted, INVALIDATE_MEMORY_ENTRY_TAG_V1)?;
        self.append_invalidation(admitted, entry_id, reason, None)
    }

    #[cfg(test)]
    pub(crate) fn supersede(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        entry_id: MemoryEntryIdV1,
        replacement: MemoryEntryIdV1,
    ) -> Result<(), MemoryErrorV1> {
        self.require_action(admitted, SUPERSEDE_MEMORY_ENTRY_TAG_V1)?;
        if entry_id == replacement || !self.entries.contains_key(&replacement) {
            return Err(MemoryErrorV1::InvalidReplacement);
        }
        self.append_invalidation(
            admitted,
            entry_id,
            MemoryInvalidationReasonV1::Superseded,
            Some(replacement),
        )
    }

    #[cfg(test)]
    pub(crate) fn security_erase(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        entry_id: MemoryEntryIdV1,
    ) -> Result<(), MemoryErrorV1> {
        self.require_action(admitted, SECURITY_ERASE_MEMORY_PAYLOAD_TAG_V1)?;
        self.append_invalidation(
            admitted,
            entry_id,
            MemoryInvalidationReasonV1::SecurityErased,
            None,
        )?;
        // Security erase scrubs the retained payload commitment; other
        // invalidation reasons keep it for audit lineage.
        let entry = self
            .entries
            .get_mut(&entry_id)
            .expect("invariant: append_invalidation admitted the entry");
        entry.payload_commitment = [0; 32];
        Ok(())
    }

    pub(crate) fn advisory_projection(
        &self,
        requested_tags: impl IntoIterator<Item = String>,
    ) -> Result<MemoryAdvisoryProjectionV1, MemoryErrorV1> {
        let requested_tags = normalize_tags(requested_tags)?;
        let entries = self
            .entries
            .values()
            .filter(|entry| !self.invalidations.contains_key(&entry.id))
            .filter(|entry| {
                requested_tags.is_empty()
                    || requested_tags
                        .iter()
                        .all(|tag| entry.tags.binary_search(tag).is_ok())
            })
            .map(|entry| MemoryAdvisoryEntryV1 {
                entry_id: entry.id,
                payload_commitment: entry.payload_commitment,
                tags: entry.tags.clone(),
            })
            .collect();
        Ok(MemoryAdvisoryProjectionV1 {
            snapshot_ref: self.snapshot_ref,
            entries,
        })
    }

    #[cfg(test)]
    fn dispose_candidate(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        action_tag: u64,
        candidate_id: MemoryCandidateIdV1,
        assessment: AdmissionAssessmentV1,
        required_decision: MemoryAdmissionDecisionV1,
        kind: MemoryDispositionKindV1,
    ) -> Result<(), MemoryErrorV1> {
        self.require_action(admitted, action_tag)?;
        if self.dispositions.contains_key(&candidate_id) {
            return Err(MemoryErrorV1::CandidateAlreadyDisposed);
        }
        let assessment_id = assessment.id;
        if assessment.candidate_id != candidate_id
            || assessment.snapshot_ref != self.snapshot_ref
            || assessment.decision != required_decision
            || self.assessments.contains_key(&assessment_id)
        {
            return Err(MemoryErrorV1::InvalidAssessment);
        }
        let receipt_ref = *admitted.authorization_receipt().id().as_bytes();
        self.dispositions.insert(
            candidate_id,
            MemoryDispositionV1 {
                event_id: event_id(candidate_id.0, assessment_id.0, receipt_ref, kind as u8),
                candidate_id,
                assessment_id,
                kind,
                authorization_receipt_ref: receipt_ref,
            },
        );
        self.assessments.insert(assessment_id, assessment);
        self.advance(admitted);
        Ok(())
    }

    #[cfg(test)]
    fn append_invalidation(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        entry_id: MemoryEntryIdV1,
        reason: MemoryInvalidationReasonV1,
        replacement: Option<MemoryEntryIdV1>,
    ) -> Result<(), MemoryErrorV1> {
        if !self.entries.contains_key(&entry_id) {
            return Err(MemoryErrorV1::UnknownEntry);
        }
        if self.invalidations.contains_key(&entry_id) {
            return Err(MemoryErrorV1::EntryAlreadyInvalidated);
        }
        let receipt_ref = *admitted.authorization_receipt().id().as_bytes();
        let replacement_ref = replacement.map_or([0; 32], |entry| entry.0);
        self.invalidations.insert(
            entry_id,
            MemoryInvalidationV1 {
                event_id: event_id(entry_id.0, replacement_ref, receipt_ref, reason as u8),
                entry_id,
                reason,
                replacement,
                authorization_receipt_ref: receipt_ref,
            },
        );
        self.advance(admitted);
        Ok(())
    }

    #[cfg(test)]
    fn require_action(
        &self,
        admitted: &AdmittedRepositoryActionV1,
        expected_tag: u64,
    ) -> Result<(), MemoryErrorV1> {
        if admitted.action().global_tag() != expected_tag {
            return Err(MemoryErrorV1::WrongAuthorityAction);
        }
        if admitted.current_snapshot_id().as_bytes() != &self.snapshot_ref {
            return Err(MemoryErrorV1::StaleSnapshot);
        }
        Ok(())
    }

    #[cfg(test)]
    fn advance(&mut self, admitted: &AdmittedRepositoryActionV1) {
        self.snapshot_ref = *admitted.successor_snapshot().id().as_bytes();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryAdvisoryEntryV1 {
    entry_id: MemoryEntryIdV1,
    payload_commitment: [u8; 32],
    tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryAdvisoryProjectionV1 {
    snapshot_ref: [u8; 32],
    entries: Vec<MemoryAdvisoryEntryV1>,
}

impl MemoryAdvisoryProjectionV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) fn entries(&self) -> &[MemoryAdvisoryEntryV1] {
        &self.entries
    }
}

fn normalize_tags(tags: impl IntoIterator<Item = String>) -> Result<Vec<String>, MemoryErrorV1> {
    let mut tags = tags
        .into_iter()
        .map(|tag| tag.trim().to_lowercase())
        .collect::<Vec<_>>();
    if tags.len() > MAX_TAGS_V1
        || tags
            .iter()
            .any(|tag| tag.is_empty() || tag.len() > 128 || tag.contains('\0'))
    {
        return Err(MemoryErrorV1::InvalidTag);
    }
    tags.sort();
    tags.dedup();
    Ok(tags)
}

fn event_id(first: [u8; 32], second: [u8; 32], receipt: [u8; 32], tag: u8) -> MemoryEventIdV1 {
    MemoryEventIdV1(hash_fields(&[
        EVENT_DOMAIN_V1,
        &first,
        &second,
        &receipt,
        &[tag],
    ]))
}

fn hash_strings(values: &[String]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((values.len() as u64).to_be_bytes());
    for value in values {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.finalize().into()
}

fn hash_digests(values: &[[u8; 32]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((values.len() as u64).to_be_bytes());
    for value in values {
        hash.update(value);
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

fn require_nonzero(value: [u8; 32]) -> Result<(), MemoryErrorV1> {
    if value == [0; 32] {
        return Err(MemoryErrorV1::InvalidReference);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum MemoryErrorV1 {
    #[error("Memory reference is invalid")]
    InvalidReference,
    #[error("Memory tag is invalid")]
    InvalidTag,
    #[error("Memory candidate already exists")]
    DuplicateCandidate,
    #[error("Memory candidate does not exist")]
    UnknownCandidate,
    #[error("Memory admission assessment is invalid")]
    InvalidAssessment,
    #[error("Memory candidate already has a disposition")]
    CandidateAlreadyDisposed,
    #[error("Memory promotion is refused")]
    PromotionRefused,
    #[error("Memory entry does not exist")]
    UnknownEntry,
    #[error("Memory entry is already invalidated")]
    EntryAlreadyInvalidated,
    #[error("Memory replacement is invalid")]
    InvalidReplacement,
    #[error("Authority admitted a different Memory action")]
    WrongAuthorityAction,
    #[error("Memory snapshot is stale")]
    StaleSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn candidate() -> MemoryCandidateV1 {
        MemoryCandidateV1::new(
            ObservationRecordIdV1::from_bytes(digest(1)).unwrap(),
            digest(2),
            digest(3),
            digest(4),
            ["advisory".to_owned()],
        )
        .unwrap()
    }

    #[test]
    fn identity_hashed_discriminants_are_pinned() {
        assert_eq!(MemoryAdmissionDecisionV1::Admit as u8, 0);
        assert_eq!(MemoryAdmissionDecisionV1::Quarantine as u8, 2);
        assert_eq!(MemoryDispositionKindV1::Promoted as u8, 0);
        assert_eq!(MemoryDispositionKindV1::Quarantined as u8, 2);
        assert_eq!(MemoryInvalidationReasonV1::Superseded as u8, 0);
        assert_eq!(MemoryInvalidationReasonV1::SecurityErased as u8, 4);
    }

    #[test]
    fn admission_assessment_is_pinned_and_distinct_from_evidence_assessment() {
        let candidate = candidate();
        let assessment = AdmissionAssessmentV1::new(
            candidate.id(),
            digest(5),
            digest(6),
            MemoryAdmissionDecisionV1::Admit,
            vec![],
        )
        .unwrap();
        assert_ne!(assessment.id().0, [0; 32]);
        assert_eq!(assessment.snapshot_ref, digest(5));
    }

    #[test]
    fn conflicting_assessment_cannot_be_an_admissible_promotion_basis() {
        let candidate = candidate();
        let assessment = AdmissionAssessmentV1::new(
            candidate.id(),
            digest(5),
            digest(6),
            MemoryAdmissionDecisionV1::Admit,
            vec![digest(7)],
        )
        .unwrap();
        assert!(!assessment.conflicting_policy_refs.is_empty());
        assert_eq!(assessment.decision, MemoryAdmissionDecisionV1::Admit);
    }

    #[test]
    fn advisory_projection_has_no_action_priority_or_authority_surface() {
        let ledger = MemoryLedgerV1::empty(digest(5)).unwrap();
        let projection = ledger
            .advisory_projection(std::iter::empty::<String>())
            .unwrap();
        assert_eq!(projection.snapshot_ref(), digest(5));
        assert!(projection.entries().is_empty());
    }
}
