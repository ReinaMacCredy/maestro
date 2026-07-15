use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::vnext::identity::{
    ContractRootIdV1, DecisionClosureIdV1, DesignClosureRequirementIdV1, DesignRevisionIdV1,
    DesignSourceBindingIdV1, IdentityError, StoreDomainIdV1, design_closure_requirement_identity,
    design_revision_identity, design_source_binding_identity,
};
use crate::domain::vnext::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{AdmittedCommittedActionV1, CommittedActionAuditV1, EvidenceRefV1, ExactRecordRefV1};

const DESIGN_SOURCE_BINDING_VERSION_V1: u64 = 1;
const DESIGN_CLOSURE_REQUIREMENT_VERSION_V1: u64 = 1;
const DESIGN_REVISION_VERSION_V1: u64 = 1;
const MAX_DESIGN_SLOTS_V1: usize = 4_096;
const MAX_DESIGN_SOURCES_V1: usize = 65_536;
const MAX_APPENDIX_ROOTS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DesignSlotIdV1(u16);

impl DesignSlotIdV1 {
    pub fn new(tag: u16) -> Result<Self, DesignV1Error> {
        if tag == 0 {
            return Err(DesignV1Error::InvalidSlotId);
        }
        Ok(Self(tag))
    }

    pub const fn tag(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum DesignSourceKindV1 {
    Intake = 1,
    Research = 2,
    DecisionResolution = 3,
    RepositoryPolicy = 4,
    ReviewEvidence = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum DesignSourceClassificationV1 {
    Normative = 1,
    Evidence = 2,
    ContextOnly = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignSourceBindingV1 {
    repository_installation_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    slot_id: DesignSlotIdV1,
    source_kind: DesignSourceKindV1,
    source_identity: ExactRecordRefV1,
    source_revision: ExactRecordRefV1,
    classification: DesignSourceClassificationV1,
    source_bytes: Vec<u8>,
    source_bytes_hash: [u8; 32],
    binding_id: DesignSourceBindingIdV1,
}

impl DesignSourceBindingV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "source binding construction requires every exact provenance field at once"
    )]
    pub fn new(
        repository_installation_id: StoreDomainIdV1,
        work_id: WorkIdV1,
        slot_id: DesignSlotIdV1,
        source_kind: DesignSourceKindV1,
        source_identity: ExactRecordRefV1,
        source_revision: ExactRecordRefV1,
        classification: DesignSourceClassificationV1,
        source_bytes: Vec<u8>,
    ) -> Result<Self, DesignV1Error> {
        if source_bytes.is_empty() {
            return Err(DesignV1Error::EmptySourceBytes);
        }
        let source_bytes_hash = super::bytes_hash_v1(&source_bytes);
        let value = source_binding_value(
            &repository_installation_id,
            work_id,
            slot_id,
            source_kind,
            source_identity,
            source_revision,
            classification,
            &source_bytes,
            &source_bytes_hash,
        );
        let binding_id = design_source_binding_identity(&value)?;
        Ok(Self {
            repository_installation_id,
            work_id,
            slot_id,
            source_kind,
            source_identity,
            source_revision,
            classification,
            source_bytes,
            source_bytes_hash,
            binding_id,
        })
    }

    pub fn binding_id(&self) -> &DesignSourceBindingIdV1 {
        &self.binding_id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn repository_installation_id(&self) -> &StoreDomainIdV1 {
        &self.repository_installation_id
    }

    pub const fn slot_id(&self) -> DesignSlotIdV1 {
        self.slot_id
    }

    pub fn source_bytes(&self) -> &[u8] {
        &self.source_bytes
    }

    pub const fn source_bytes_hash(&self) -> &[u8; 32] {
        &self.source_bytes_hash
    }

    pub const fn source_kind(&self) -> DesignSourceKindV1 {
        self.source_kind
    }

    pub const fn source_identity(&self) -> &ExactRecordRefV1 {
        &self.source_identity
    }

    pub const fn source_revision(&self) -> &ExactRecordRefV1 {
        &self.source_revision
    }

    pub const fn classification(&self) -> DesignSourceClassificationV1 {
        self.classification
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignClosureRequirementSnapshotV1 {
    repository_installation_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    required_slots: Vec<DesignSlotIdV1>,
    requirement_id: DesignClosureRequirementIdV1,
}

impl DesignClosureRequirementSnapshotV1 {
    pub fn new(
        repository_installation_id: StoreDomainIdV1,
        work_id: WorkIdV1,
        mut required_slots: Vec<DesignSlotIdV1>,
    ) -> Result<Self, DesignV1Error> {
        if required_slots.is_empty() || required_slots.len() > MAX_DESIGN_SLOTS_V1 {
            return Err(DesignV1Error::InvalidRequiredSlotCardinality);
        }
        required_slots.sort_unstable();
        if required_slots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DesignV1Error::DuplicateRequiredSlot);
        }
        let value =
            closure_requirement_value(&repository_installation_id, work_id, &required_slots);
        let requirement_id = design_closure_requirement_identity(&value)?;
        Ok(Self {
            repository_installation_id,
            work_id,
            required_slots,
            requirement_id,
        })
    }

    pub fn requirement_id(&self) -> &DesignClosureRequirementIdV1 {
        &self.requirement_id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn repository_installation_id(&self) -> &StoreDomainIdV1 {
        &self.repository_installation_id
    }

    pub fn required_slots(&self) -> &[DesignSlotIdV1] {
        &self.required_slots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesignSlotDispositionV1 {
    Satisfied {
        source_binding_ids: Vec<DesignSourceBindingIdV1>,
    },
    AuthorizedNotApplicable {
        authorization: Box<CommittedActionAuditV1>,
        evidence: EvidenceRefV1,
        source_binding_id: DesignSourceBindingIdV1,
    },
    Missing,
}

impl DesignSlotDispositionV1 {
    pub fn satisfied(
        mut source_binding_ids: Vec<DesignSourceBindingIdV1>,
    ) -> Result<Self, DesignV1Error> {
        if source_binding_ids.is_empty() || source_binding_ids.len() > MAX_DESIGN_SOURCES_V1 {
            return Err(DesignV1Error::InvalidSatisfiedSourceCardinality);
        }
        source_binding_ids.sort_unstable();
        if source_binding_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DesignV1Error::DuplicateSourceBinding);
        }
        Ok(Self::Satisfied { source_binding_ids })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "only a pending Repository Store design action may construct this disposition"
        )
    )]
    pub(crate) fn authorized_not_applicable(
        admitted_action: &AdmittedCommittedActionV1,
        evidence: EvidenceRefV1,
        source_binding_id: DesignSourceBindingIdV1,
    ) -> Self {
        Self::AuthorizedNotApplicable {
            authorization: Box::new(admitted_action.audit().clone()),
            evidence,
            source_binding_id,
        }
    }

    fn source_binding_ids(&self) -> Vec<DesignSourceBindingIdV1> {
        match self {
            Self::Satisfied { source_binding_ids } => source_binding_ids.clone(),
            Self::AuthorizedNotApplicable {
                source_binding_id, ..
            } => vec![*source_binding_id],
            Self::Missing => Vec::new(),
        }
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Satisfied { source_binding_ids } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                identity_array(source_binding_ids),
            ]),
            Self::AuthorizedNotApplicable {
                authorization,
                evidence,
                source_binding_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                authorization.canonical_value(),
                evidence.canonical_value(),
                CborValue::Bytes(source_binding_id.as_bytes().to_vec()),
            ]),
            Self::Missing => CborValue::Array(vec![CborValue::Unsigned(3)]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignSlotEntryV1 {
    slot_id: DesignSlotIdV1,
    disposition: DesignSlotDispositionV1,
}

impl DesignSlotEntryV1 {
    pub const fn new(slot_id: DesignSlotIdV1, disposition: DesignSlotDispositionV1) -> Self {
        Self {
            slot_id,
            disposition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignSlotManifestV1 {
    requirement_id: DesignClosureRequirementIdV1,
    entries: Vec<DesignSlotEntryV1>,
    missing_slots: Vec<DesignSlotIdV1>,
    referenced_source_bindings: Vec<DesignSourceBindingIdV1>,
}

impl DesignSlotManifestV1 {
    pub fn new(
        requirement: &DesignClosureRequirementSnapshotV1,
        sources: &[DesignSourceBindingV1],
        mut entries: Vec<DesignSlotEntryV1>,
    ) -> Result<Self, DesignV1Error> {
        entries.sort_by_key(|entry| entry.slot_id);
        let actual_slots: Vec<_> = entries.iter().map(|entry| entry.slot_id).collect();
        if actual_slots != requirement.required_slots {
            return Err(DesignV1Error::SlotManifestIsNotTotal);
        }

        let source_by_id: BTreeMap<_, _> = sources
            .iter()
            .map(|source| (*source.binding_id(), source))
            .collect();
        if source_by_id.len() != sources.len() {
            return Err(DesignV1Error::DuplicateSourceBinding);
        }
        let mut referenced = Vec::new();
        let mut missing_slots = Vec::new();
        for entry in &entries {
            if matches!(entry.disposition, DesignSlotDispositionV1::Missing) {
                missing_slots.push(entry.slot_id);
            }
            for source_id in entry.disposition.source_binding_ids() {
                let source = source_by_id
                    .get(&source_id)
                    .ok_or(DesignV1Error::UnknownSourceBinding)?;
                if source.slot_id != entry.slot_id
                    || source.work_id != requirement.work_id
                    || source.repository_installation_id != requirement.repository_installation_id
                {
                    return Err(DesignV1Error::SourceBindingOwnerMismatch);
                }
                referenced.push(source_id);
            }
        }
        referenced.sort_unstable();
        if referenced.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DesignV1Error::DuplicateSourceBindingReference);
        }
        let supplied: BTreeSet<_> = source_by_id.keys().copied().collect();
        let referenced_set: BTreeSet<_> = referenced.iter().copied().collect();
        if supplied != referenced_set {
            return Err(DesignV1Error::SourceClosureMismatch);
        }
        Ok(Self {
            requirement_id: *requirement.requirement_id(),
            entries,
            missing_slots,
            referenced_source_bindings: referenced,
        })
    }

    pub fn missing_slots(&self) -> &[DesignSlotIdV1] {
        &self.missing_slots
    }

    pub fn referenced_source_bindings(&self) -> &[DesignSourceBindingIdV1] {
        &self.referenced_source_bindings
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.requirement_id.as_bytes().to_vec()),
            CborValue::Array(
                self.entries
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            CborValue::Unsigned(u64::from(entry.slot_id.tag())),
                            entry.disposition.canonical_value(),
                        ])
                    })
                    .collect(),
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AppendixRootV1(ExactRecordRefV1);

impl AppendixRootV1 {
    pub const fn new(root: ExactRecordRefV1) -> Self {
        Self(root)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignRevisionV1 {
    repository_installation_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    parent_revision_id: Option<DesignRevisionIdV1>,
    requirement_id: DesignClosureRequirementIdV1,
    slot_manifest: DesignSlotManifestV1,
    source_bindings: Vec<DesignSourceBindingV1>,
    appendix_roots: Vec<AppendixRootV1>,
    revision_id: DesignRevisionIdV1,
}

impl DesignRevisionV1 {
    pub fn new(
        repository_installation_id: StoreDomainIdV1,
        work_id: WorkIdV1,
        parent_revision_id: Option<DesignRevisionIdV1>,
        requirement: &DesignClosureRequirementSnapshotV1,
        slot_manifest: DesignSlotManifestV1,
        mut source_bindings: Vec<DesignSourceBindingV1>,
        mut appendix_roots: Vec<AppendixRootV1>,
    ) -> Result<Self, DesignV1Error> {
        if requirement.work_id != work_id
            || requirement.repository_installation_id != repository_installation_id
            || slot_manifest.requirement_id != requirement.requirement_id
        {
            return Err(DesignV1Error::RevisionOwnerMismatch);
        }
        if source_bindings.len() > MAX_DESIGN_SOURCES_V1
            || appendix_roots.len() > MAX_APPENDIX_ROOTS_V1
        {
            return Err(DesignV1Error::RevisionClosureTooLarge);
        }
        source_bindings.sort_by_key(|binding| *binding.binding_id());
        appendix_roots.sort_unstable();
        if appendix_roots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DesignV1Error::DuplicateAppendixRoot);
        }
        let source_ids: Vec<_> = source_bindings
            .iter()
            .map(|binding| *binding.binding_id())
            .collect();
        if source_ids != slot_manifest.referenced_source_bindings {
            return Err(DesignV1Error::SourceClosureMismatch);
        }
        let value = revision_value(
            &repository_installation_id,
            work_id,
            parent_revision_id.as_ref(),
            requirement.requirement_id(),
            &slot_manifest,
            &source_bindings,
            &appendix_roots,
        );
        let revision_id = design_revision_identity(&value)?;
        Ok(Self {
            repository_installation_id,
            work_id,
            parent_revision_id,
            requirement_id: *requirement.requirement_id(),
            slot_manifest,
            source_bindings,
            appendix_roots,
            revision_id,
        })
    }

    pub fn revision_id(&self) -> &DesignRevisionIdV1 {
        &self.revision_id
    }

    pub fn parent_revision_id(&self) -> Option<&DesignRevisionIdV1> {
        self.parent_revision_id.as_ref()
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn repository_installation_id(&self) -> &StoreDomainIdV1 {
        &self.repository_installation_id
    }

    pub fn slot_manifest(&self) -> &DesignSlotManifestV1 {
        &self.slot_manifest
    }

    pub fn requirement_id(&self) -> &DesignClosureRequirementIdV1 {
        &self.requirement_id
    }

    pub fn source_bindings(&self) -> &[DesignSourceBindingV1] {
        &self.source_bindings
    }

    pub fn appendix_roots(&self) -> &[AppendixRootV1] {
        &self.appendix_roots
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DesignV1Error> {
        Ok(deterministic_cbor::encode(&revision_value(
            &self.repository_installation_id,
            self.work_id,
            self.parent_revision_id.as_ref(),
            &self.requirement_id,
            &self.slot_manifest,
            &self.source_bindings,
            &self.appendix_roots,
        ))?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesignAppendEligibilityV1 {
    Eligible,
    TerminalWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignStreamV1 {
    repository_installation_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    revisions: Vec<DesignRevisionV1>,
}

impl DesignStreamV1 {
    pub fn new(genesis: DesignRevisionV1) -> Result<Self, DesignV1Error> {
        if genesis.parent_revision_id.is_some() {
            return Err(DesignV1Error::GenesisHasParent);
        }
        Ok(Self {
            repository_installation_id: genesis.repository_installation_id,
            work_id: genesis.work_id,
            revisions: vec![genesis],
        })
    }

    pub fn append(
        &self,
        expected_head: &DesignRevisionIdV1,
        revision: DesignRevisionV1,
        eligibility: DesignAppendEligibilityV1,
    ) -> Result<Self, DesignV1Error> {
        if eligibility == DesignAppendEligibilityV1::TerminalWork {
            return Err(DesignV1Error::TerminalWorkRejectsDesignChange);
        }
        let current_head = self
            .revisions
            .last()
            .expect("invariant: Design stream contains genesis");
        if current_head.revision_id() != expected_head {
            return Err(DesignV1Error::StaleCandidateHead);
        }
        if revision.parent_revision_id() != Some(expected_head)
            || revision.work_id != self.work_id
            || revision.repository_installation_id != self.repository_installation_id
        {
            return Err(DesignV1Error::InvalidRevisionSuccessor);
        }
        let mut revisions = self.revisions.clone();
        revisions.push(revision);
        Ok(Self {
            repository_installation_id: self.repository_installation_id,
            work_id: self.work_id,
            revisions,
        })
    }

    pub fn revisions(&self) -> &[DesignRevisionV1] {
        &self.revisions
    }

    pub fn candidate_head(&self) -> &DesignRevisionV1 {
        self.revisions
            .last()
            .expect("invariant: Design stream contains genesis")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignReconciliationSnapshotV1 {
    design_revision_id: DesignRevisionIdV1,
    missing_slots: Vec<DesignSlotIdV1>,
    decision_closure_id: DecisionClosureIdV1,
    candidate_contract_root_id: ContractRootIdV1,
    decision_closure_complete: bool,
    component_provenance_total: bool,
    freshness_current: bool,
}

impl DesignReconciliationSnapshotV1 {
    pub fn new(
        revision: &DesignRevisionV1,
        decision_closure_id: DecisionClosureIdV1,
        candidate_contract_root_id: ContractRootIdV1,
        decision_closure_complete: bool,
        component_provenance_total: bool,
        freshness_current: bool,
    ) -> Self {
        Self {
            design_revision_id: *revision.revision_id(),
            missing_slots: revision.slot_manifest.missing_slots.clone(),
            decision_closure_id,
            candidate_contract_root_id,
            decision_closure_complete,
            component_provenance_total,
            freshness_current,
        }
    }

    pub fn reconcile(&self) -> DesignReconciliationV1 {
        DesignReconciliationV1 {
            design_revision_id: self.design_revision_id,
            missing_slots: self.missing_slots.clone(),
            decision_closure_id: self.decision_closure_id,
            candidate_contract_root_id: self.candidate_contract_root_id,
            decision_closure_complete: self.decision_closure_complete,
            component_provenance_total: self.component_provenance_total,
            freshness_current: self.freshness_current,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignReconciliationV1 {
    design_revision_id: DesignRevisionIdV1,
    missing_slots: Vec<DesignSlotIdV1>,
    decision_closure_id: DecisionClosureIdV1,
    candidate_contract_root_id: ContractRootIdV1,
    decision_closure_complete: bool,
    component_provenance_total: bool,
    freshness_current: bool,
}

impl DesignReconciliationV1 {
    pub fn missing_slots(&self) -> &[DesignSlotIdV1] {
        &self.missing_slots
    }

    pub fn is_clean(&self) -> bool {
        self.missing_slots.is_empty()
            && self.decision_closure_complete
            && self.component_provenance_total
            && self.freshness_current
    }

    pub fn finalization_inputs(&self) -> Result<DesignFinalizationInputsV1, DesignV1Error> {
        if !self.is_clean() {
            return Err(DesignV1Error::ReconciliationNotClean);
        }
        Ok(DesignFinalizationInputsV1 {
            design_revision_id: self.design_revision_id,
            decision_closure_id: self.decision_closure_id,
            candidate_contract_root_id: self.candidate_contract_root_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignFinalizationInputsV1 {
    design_revision_id: DesignRevisionIdV1,
    decision_closure_id: DecisionClosureIdV1,
    candidate_contract_root_id: ContractRootIdV1,
}

impl DesignFinalizationInputsV1 {
    pub fn design_revision_id(&self) -> &DesignRevisionIdV1 {
        &self.design_revision_id
    }

    pub fn decision_closure_id(&self) -> &DecisionClosureIdV1 {
        &self.decision_closure_id
    }

    pub fn candidate_contract_root_id(&self) -> &ContractRootIdV1 {
        &self.candidate_contract_root_id
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DesignV1Error {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("Design slot id must be positive")]
    InvalidSlotId,
    #[error("Design source bytes must be non-empty")]
    EmptySourceBytes,
    #[error("Design closure must require between 1 and 4096 slots")]
    InvalidRequiredSlotCardinality,
    #[error("Design closure repeats a required slot")]
    DuplicateRequiredSlot,
    #[error("satisfied Design slot must bind a finite non-empty source set")]
    InvalidSatisfiedSourceCardinality,
    #[error("Design source closure repeats a source binding")]
    DuplicateSourceBinding,
    #[error("Design slot manifest is not total over the exact requirement snapshot")]
    SlotManifestIsNotTotal,
    #[error("Design slot references an unknown source binding")]
    UnknownSourceBinding,
    #[error("Design source binding does not match repository, Work, or slot owner")]
    SourceBindingOwnerMismatch,
    #[error("Design source binding is referenced more than once")]
    DuplicateSourceBindingReference,
    #[error("Design Revision source closure does not exactly match the slot manifest")]
    SourceClosureMismatch,
    #[error("Design Revision owner or closure requirement does not match")]
    RevisionOwnerMismatch,
    #[error("Design Revision closure exceeds its finite v1 bound")]
    RevisionClosureTooLarge,
    #[error("Design Revision repeats an appendix root")]
    DuplicateAppendixRoot,
    #[error("Design genesis Revision must not have a parent")]
    GenesisHasParent,
    #[error("Design candidate head changed since it was read")]
    StaleCandidateHead,
    #[error("Design Revision is not the exact successor of the candidate head")]
    InvalidRevisionSuccessor,
    #[error("terminal Work rejects Design head changes")]
    TerminalWorkRejectsDesignChange,
    #[error("Design reconciliation is not clean and cannot feed finalization")]
    ReconciliationNotClean,
}

#[expect(
    clippy::too_many_arguments,
    reason = "canonical source identity includes every exact provenance field"
)]
fn source_binding_value(
    repository: &StoreDomainIdV1,
    work_id: WorkIdV1,
    slot: DesignSlotIdV1,
    kind: DesignSourceKindV1,
    identity: ExactRecordRefV1,
    revision: ExactRecordRefV1,
    classification: DesignSourceClassificationV1,
    source_bytes: &[u8],
    source_bytes_hash: &[u8; 32],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DESIGN_SOURCE_BINDING_VERSION_V1),
        CborValue::Bytes(repository.as_bytes().to_vec()),
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::Unsigned(u64::from(slot.tag())),
        CborValue::Unsigned(kind as u64),
        identity.canonical_value(),
        revision.canonical_value(),
        CborValue::Unsigned(classification as u64),
        CborValue::Bytes(source_bytes.to_vec()),
        CborValue::Bytes(source_bytes_hash.to_vec()),
    ])
}

fn closure_requirement_value(
    repository: &StoreDomainIdV1,
    work_id: WorkIdV1,
    slots: &[DesignSlotIdV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DESIGN_CLOSURE_REQUIREMENT_VERSION_V1),
        CborValue::Bytes(repository.as_bytes().to_vec()),
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::Array(
            slots
                .iter()
                .map(|slot| CborValue::Unsigned(u64::from(slot.tag())))
                .collect(),
        ),
    ])
}

fn revision_value(
    repository: &StoreDomainIdV1,
    work_id: WorkIdV1,
    parent: Option<&DesignRevisionIdV1>,
    requirement_id: &DesignClosureRequirementIdV1,
    manifest: &DesignSlotManifestV1,
    sources: &[DesignSourceBindingV1],
    appendices: &[AppendixRootV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DESIGN_REVISION_VERSION_V1),
        CborValue::Bytes(repository.as_bytes().to_vec()),
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::optional(parent.map(|id| CborValue::Bytes(id.as_bytes().to_vec()))),
        CborValue::Bytes(requirement_id.as_bytes().to_vec()),
        manifest.canonical_value(),
        CborValue::Array(
            sources
                .iter()
                .map(|source| CborValue::Bytes(source.binding_id.as_bytes().to_vec()))
                .collect(),
        ),
        CborValue::Array(
            appendices
                .iter()
                .map(|root| root.0.canonical_value())
                .collect(),
        ),
    ])
}

fn identity_array(values: &[DesignSourceBindingIdV1]) -> CborValue {
    CborValue::Array(
        values
            .iter()
            .map(|value| CborValue::Bytes(value.as_bytes().to_vec()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::design::AdmittedCommittedActionV1;

    fn digest(seed: u8) -> ExactRecordRefV1 {
        ExactRecordRefV1::from_digest([seed; 32])
    }

    fn repository() -> StoreDomainIdV1 {
        StoreDomainIdV1::parse(&format!("sha256:{}", "51".repeat(32)))
            .expect("Repository Store domain")
    }

    fn work() -> WorkIdV1 {
        WorkIdV1::derive("authorized-not-applicable-work").expect("Work identity")
    }

    #[test]
    fn authorized_not_applicable_requires_an_admitted_commit_audit_and_exact_source() {
        let slot = DesignSlotIdV1::new(7).expect("slot");
        let requirement = DesignClosureRequirementSnapshotV1::new(repository(), work(), vec![slot])
            .expect("closure requirement");
        let source = DesignSourceBindingV1::new(
            repository(),
            work(),
            slot,
            DesignSourceKindV1::ReviewEvidence,
            digest(41),
            digest(42),
            DesignSourceClassificationV1::Evidence,
            b"not applicable evidence".to_vec(),
        )
        .expect("source binding");
        let admitted = AdmittedCommittedActionV1::fixture("not-applicable");
        let disposition = DesignSlotDispositionV1::authorized_not_applicable(
            &admitted,
            digest(43).into(),
            *source.binding_id(),
        );
        let manifest = DesignSlotManifestV1::new(
            &requirement,
            std::slice::from_ref(&source),
            vec![DesignSlotEntryV1::new(slot, disposition)],
        )
        .expect("authorized not-applicable slot");

        assert!(manifest.missing_slots().is_empty());
        assert_eq!(
            manifest.referenced_source_bindings(),
            &[*source.binding_id()]
        );
    }
}
