//! Intake implementation seam over frozen submission and observation contracts.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Intake before the Stage 6 Action adapter is integrated"
)]

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use crate::domain::vnext::authority::AdmittedRepositoryActionV1;

const SOURCE_DOMAIN_V1: &[u8] = b"maestro.vnext.intake-source-artifact.v1";
const FINDING_DOMAIN_V1: &[u8] = b"maestro.vnext.intake-finding.v1";
const DISPOSITION_DOMAIN_V1: &[u8] = b"maestro.vnext.intake-disposition.v1";
const RECORD_INTAKE_SOURCE_TAG_V1: u64 = 139;
const PUBLISH_INTAKE_FINDING_TAG_V1: u64 = 140;
const DISPOSE_INTAKE_SOURCE_TAG_V1: u64 = 141;
const MAX_SOURCE_REFS_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceArtifactIdV1([u8; 32]);

impl SourceArtifactIdV1 {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct IntakeFindingIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct IntakeDispositionIdV1([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntakeSourceKindV1 {
    Plan,
    ProductRequirement,
    PullRequest,
    Conversation,
    ExternalDocument,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntakeRiskClassV1 {
    Ordinary,
    Sensitive,
    ExecutableInstruction,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntakeConsentStateV1 {
    Retained,
    Redacted,
    Withdrawn,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceArtifactV1 {
    id: SourceArtifactIdV1,
    source_kind: IntakeSourceKindV1,
    content_commitment: [u8; 32],
    attribution_commitment: [u8; 32],
    captured_at_ref: [u8; 32],
    consent_state: IntakeConsentStateV1,
    redaction_policy_ref: [u8; 32],
    risk_class: IntakeRiskClassV1,
}

impl SourceArtifactV1 {
    pub(crate) fn new(
        source_kind: IntakeSourceKindV1,
        content_commitment: [u8; 32],
        attribution_commitment: [u8; 32],
        captured_at_ref: [u8; 32],
        consent_state: IntakeConsentStateV1,
        redaction_policy_ref: [u8; 32],
        risk_class: IntakeRiskClassV1,
    ) -> Result<Self, IntakeErrorV1> {
        require_nonzero(content_commitment)?;
        require_nonzero(attribution_commitment)?;
        require_nonzero(captured_at_ref)?;
        require_nonzero(redaction_policy_ref)?;
        let id = SourceArtifactIdV1(hash_fields(&[
            SOURCE_DOMAIN_V1,
            &[source_kind as u8],
            &content_commitment,
            &attribution_commitment,
            &captured_at_ref,
            &[consent_state as u8],
            &redaction_policy_ref,
            &[risk_class as u8],
        ]));
        Ok(Self {
            id,
            source_kind,
            content_commitment,
            attribution_commitment,
            captured_at_ref,
            consent_state,
            redaction_policy_ref,
            risk_class,
        })
    }

    pub(crate) const fn id(&self) -> SourceArtifactIdV1 {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntakeFindingKindV1 {
    ScopeCandidate,
    AcceptanceCandidate,
    NonGoal,
    Unknown,
    Risk,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntakeFindingV1 {
    id: IntakeFindingIdV1,
    source_refs: Vec<SourceArtifactIdV1>,
    kind: IntakeFindingKindV1,
    finding_commitment: [u8; 32],
    provenance_commitment: [u8; 32],
}

impl IntakeFindingV1 {
    pub(crate) fn new(
        mut source_refs: Vec<SourceArtifactIdV1>,
        kind: IntakeFindingKindV1,
        finding_commitment: [u8; 32],
        provenance_commitment: [u8; 32],
    ) -> Result<Self, IntakeErrorV1> {
        require_nonzero(finding_commitment)?;
        require_nonzero(provenance_commitment)?;
        if source_refs.is_empty() || source_refs.len() > MAX_SOURCE_REFS_V1 {
            return Err(IntakeErrorV1::InvalidSourceClosure);
        }
        source_refs.sort();
        source_refs.dedup();
        let source_hash = hash_digests(source_refs.iter().map(|source| source.0));
        let id = IntakeFindingIdV1(hash_fields(&[
            FINDING_DOMAIN_V1,
            &source_hash,
            &[kind as u8],
            &finding_commitment,
            &provenance_commitment,
        ]));
        Ok(Self {
            id,
            source_refs,
            kind,
            finding_commitment,
            provenance_commitment,
        })
    }

    pub(crate) const fn id(&self) -> IntakeFindingIdV1 {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntakeSourceDispositionKindV1 {
    Superseded,
    ConsentWithdrawn,
    RetentionExpired,
    SecurityErased,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntakeSourceDispositionV1 {
    id: IntakeDispositionIdV1,
    source_id: SourceArtifactIdV1,
    kind: IntakeSourceDispositionKindV1,
    authorization_receipt_ref: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntakeLedgerV1 {
    snapshot_ref: [u8; 32],
    sources: BTreeMap<SourceArtifactIdV1, SourceArtifactV1>,
    findings: BTreeMap<IntakeFindingIdV1, IntakeFindingV1>,
    dispositions: BTreeMap<SourceArtifactIdV1, IntakeSourceDispositionV1>,
}

impl IntakeLedgerV1 {
    pub(crate) fn empty(snapshot_ref: [u8; 32]) -> Result<Self, IntakeErrorV1> {
        require_nonzero(snapshot_ref)?;
        Ok(Self {
            snapshot_ref,
            sources: BTreeMap::new(),
            findings: BTreeMap::new(),
            dispositions: BTreeMap::new(),
        })
    }

    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    #[cfg(test)]
    pub(crate) fn record_source(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        source: SourceArtifactV1,
    ) -> Result<SourceArtifactIdV1, IntakeErrorV1> {
        self.require_action(admitted, RECORD_INTAKE_SOURCE_TAG_V1)?;
        if self.sources.contains_key(&source.id) {
            return Err(IntakeErrorV1::DuplicateSource);
        }
        let id = source.id;
        self.sources.insert(id, source);
        self.advance(admitted);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn publish_finding(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        finding: IntakeFindingV1,
    ) -> Result<IntakeFindingIdV1, IntakeErrorV1> {
        self.require_action(admitted, PUBLISH_INTAKE_FINDING_TAG_V1)?;
        if finding.source_refs.iter().any(|source| {
            self.dispositions.contains_key(source)
                || self
                    .sources
                    .get(source)
                    .is_none_or(|source| source.consent_state == IntakeConsentStateV1::Withdrawn)
        }) {
            return Err(IntakeErrorV1::InvalidSourceClosure);
        }
        if self.findings.contains_key(&finding.id) {
            return Err(IntakeErrorV1::DuplicateFinding);
        }
        let id = finding.id;
        self.findings.insert(id, finding);
        self.advance(admitted);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn dispose_source(
        &mut self,
        admitted: &AdmittedRepositoryActionV1,
        source_id: SourceArtifactIdV1,
        kind: IntakeSourceDispositionKindV1,
    ) -> Result<IntakeDispositionIdV1, IntakeErrorV1> {
        self.require_action(admitted, DISPOSE_INTAKE_SOURCE_TAG_V1)?;
        if !self.sources.contains_key(&source_id) {
            return Err(IntakeErrorV1::UnknownSource);
        }
        if self.dispositions.contains_key(&source_id) {
            return Err(IntakeErrorV1::SourceAlreadyDisposed);
        }
        let receipt_ref = *admitted.authorization_receipt().id().as_bytes();
        let id = IntakeDispositionIdV1(hash_fields(&[
            DISPOSITION_DOMAIN_V1,
            &source_id.0,
            &[kind as u8],
            &receipt_ref,
        ]));
        self.dispositions.insert(
            source_id,
            IntakeSourceDispositionV1 {
                id,
                source_id,
                kind,
                authorization_receipt_ref: receipt_ref,
            },
        );
        self.advance(admitted);
        Ok(id)
    }

    pub(crate) fn projection(&self) -> IntakeProjectionV1 {
        let sources = self
            .sources
            .values()
            .filter(|source| {
                !self.dispositions.contains_key(&source.id)
                    && source.consent_state != IntakeConsentStateV1::Withdrawn
            })
            .map(|source| IntakeProjectedSourceV1 {
                id: source.id,
                consent_state: source.consent_state,
                risk_class: source.risk_class,
            })
            .collect();
        let findings = self
            .findings
            .values()
            .filter(|finding| {
                finding.source_refs.iter().all(|source| {
                    !self.dispositions.contains_key(source)
                        && self.sources.get(source).is_some_and(|source| {
                            source.consent_state != IntakeConsentStateV1::Withdrawn
                        })
                })
            })
            .map(|finding| IntakeProjectedFindingV1 {
                id: finding.id,
                kind: finding.kind,
                trust: IntakeFindingTrustV1::UntrustedDesignInput,
            })
            .collect();
        IntakeProjectionV1 {
            snapshot_ref: self.snapshot_ref,
            sources,
            findings,
        }
    }

    #[cfg(test)]
    fn require_action(
        &self,
        admitted: &AdmittedRepositoryActionV1,
        expected_tag: u64,
    ) -> Result<(), IntakeErrorV1> {
        if admitted.action().global_tag() != expected_tag {
            return Err(IntakeErrorV1::WrongAuthorityAction);
        }
        if admitted.current_snapshot_id().as_bytes() != &self.snapshot_ref {
            return Err(IntakeErrorV1::StaleSnapshot);
        }
        Ok(())
    }

    #[cfg(test)]
    fn advance(&mut self, admitted: &AdmittedRepositoryActionV1) {
        self.snapshot_ref = *admitted.successor_snapshot().id().as_bytes();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntakeFindingTrustV1 {
    UntrustedDesignInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntakeProjectedSourceV1 {
    id: SourceArtifactIdV1,
    consent_state: IntakeConsentStateV1,
    risk_class: IntakeRiskClassV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntakeProjectedFindingV1 {
    id: IntakeFindingIdV1,
    kind: IntakeFindingKindV1,
    trust: IntakeFindingTrustV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntakeProjectionV1 {
    snapshot_ref: [u8; 32],
    sources: Vec<IntakeProjectedSourceV1>,
    findings: Vec<IntakeProjectedFindingV1>,
}

impl IntakeProjectionV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) fn findings(&self) -> &[IntakeProjectedFindingV1] {
        &self.findings
    }
}

fn hash_digests(values: impl IntoIterator<Item = [u8; 32]>) -> [u8; 32] {
    let values = values.into_iter().collect::<Vec<_>>();
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

fn require_nonzero(value: [u8; 32]) -> Result<(), IntakeErrorV1> {
    if value == [0; 32] {
        return Err(IntakeErrorV1::InvalidReference);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum IntakeErrorV1 {
    #[error("Intake reference is invalid")]
    InvalidReference,
    #[error("Intake source already exists")]
    DuplicateSource,
    #[error("Intake source does not exist")]
    UnknownSource,
    #[error("Intake source closure is invalid")]
    InvalidSourceClosure,
    #[error("Intake finding already exists")]
    DuplicateFinding,
    #[error("Intake source is already disposed")]
    SourceAlreadyDisposed,
    #[error("Authority admitted a different Intake action")]
    WrongAuthorityAction,
    #[error("Intake snapshot is stale")]
    StaleSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn source_identity_binds_attribution_consent_redaction_and_risk() {
        let source = SourceArtifactV1::new(
            IntakeSourceKindV1::ProductRequirement,
            digest(1),
            digest(2),
            digest(3),
            IntakeConsentStateV1::Redacted,
            digest(4),
            IntakeRiskClassV1::ExecutableInstruction,
        )
        .unwrap();
        assert_ne!(source.id().0, [0; 32]);
        assert_eq!(source.risk_class, IntakeRiskClassV1::ExecutableInstruction);
    }

    #[test]
    fn finding_remains_explicitly_untrusted() {
        let source = SourceArtifactIdV1(digest(1));
        let finding = IntakeFindingV1::new(
            vec![source],
            IntakeFindingKindV1::AcceptanceCandidate,
            digest(2),
            digest(3),
        )
        .unwrap();
        assert_ne!(finding.id().0, [0; 32]);
        assert_eq!(finding.kind, IntakeFindingKindV1::AcceptanceCandidate);
    }

    #[test]
    fn empty_projection_is_snapshot_bound_and_non_promoting() {
        let ledger = IntakeLedgerV1::empty(digest(9)).unwrap();
        let projection = ledger.projection();
        assert_eq!(projection.snapshot_ref(), digest(9));
        assert!(projection.findings().is_empty());
    }
}
