use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::vnext::contract::materialization::ContractConsequencePlanV1;
use crate::domain::vnext::identity::{
    ContractRootIdV1, DecisionResolutionIdV1, IdentityError, StoreDomainIdV1,
    decision_resolution_identity,
};
use crate::domain::vnext::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{
    AdmittedCommittedActionV1, AlternativeIdV1, CommittedActionAuditV1, DecisionIdV1,
    DecisionRevisionIdV1, DecisionSupersessionIdV1, ExactRecordRefV1,
    SupersessionAuthorizationReceiptRefV1, canonical_hash_v1, optional_digest_v1,
};

const ALTERNATIVE_VERSION_V1: u64 = 1;
const DECISION_REVISION_VERSION_V1: u64 = 1;
const RESOLUTION_VERSION_V1: u64 = 1;
const SUPERSESSION_VERSION_V1: u64 = 1;
const MAX_ALTERNATIVES_V1: usize = 256;
const MAX_DECISION_REVISIONS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlternativeConsequenceV1 {
    NoContractEffect,
    TypedConsequencePlan { plan: ContractConsequencePlanV1 },
}

impl AlternativeConsequenceV1 {
    pub const fn typed_plan(plan: ContractConsequencePlanV1) -> Self {
        Self::TypedConsequencePlan { plan }
    }

    pub const fn base_contract_root_id(&self) -> Option<&ContractRootIdV1> {
        match self {
            Self::NoContractEffect => None,
            Self::TypedConsequencePlan { plan } => Some(plan.base_root_id()),
        }
    }

    pub const fn typed_plan_value(&self) -> Option<&ContractConsequencePlanV1> {
        match self {
            Self::NoContractEffect => None,
            Self::TypedConsequencePlan { plan } => Some(plan),
        }
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::NoContractEffect => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::TypedConsequencePlan { plan } => {
                CborValue::Array(vec![CborValue::Unsigned(2), plan.canonical_value()])
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlternativeV1 {
    meaning: Vec<u8>,
    preview: Vec<u8>,
    consequence: AlternativeConsequenceV1,
    alternative_id: AlternativeIdV1,
}

impl AlternativeV1 {
    pub fn new(
        meaning: Vec<u8>,
        preview: Vec<u8>,
        consequence: AlternativeConsequenceV1,
    ) -> Result<Self, DecisionV1Error> {
        if meaning.is_empty() || preview.is_empty() {
            return Err(DecisionV1Error::EmptyAlternativeContent);
        }
        let value = alternative_value(&meaning, &preview, &consequence);
        let alternative_id = AlternativeIdV1::from_digest(canonical_hash_v1(
            "maestro.vnext.decision-alternative.v1",
            value,
        )?);
        Ok(Self {
            meaning,
            preview,
            consequence,
            alternative_id,
        })
    }

    pub const fn alternative_id(&self) -> &AlternativeIdV1 {
        &self.alternative_id
    }

    pub const fn consequence(&self) -> &AlternativeConsequenceV1 {
        &self.consequence
    }

    pub fn meaning(&self) -> &[u8] {
        &self.meaning
    }

    pub fn preview(&self) -> &[u8] {
        &self.preview
    }

    fn canonical_value(&self) -> CborValue {
        alternative_value(&self.meaning, &self.preview, &self.consequence)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionRevisionV1 {
    decision_id: DecisionIdV1,
    ordinal: u32,
    parent_revision_id: Option<DecisionRevisionIdV1>,
    question: Vec<u8>,
    subject_hash: ExactRecordRefV1,
    base_contract_root_id: ContractRootIdV1,
    alternatives: Vec<AlternativeV1>,
    revision_id: DecisionRevisionIdV1,
}

impl DecisionRevisionV1 {
    pub fn new(
        decision_id: DecisionIdV1,
        ordinal: u32,
        parent_revision_id: Option<DecisionRevisionIdV1>,
        question: Vec<u8>,
        subject_hash: ExactRecordRefV1,
        base_contract_root_id: ContractRootIdV1,
        alternatives: Vec<AlternativeV1>,
    ) -> Result<Self, DecisionV1Error> {
        if ordinal == 0
            || (ordinal == 1 && parent_revision_id.is_some())
            || (ordinal > 1 && parent_revision_id.is_none())
        {
            return Err(DecisionV1Error::InvalidRevisionParentage);
        }
        if question.is_empty() {
            return Err(DecisionV1Error::EmptyDecisionQuestion);
        }
        if !(2..=MAX_ALTERNATIVES_V1).contains(&alternatives.len()) {
            return Err(DecisionV1Error::AlternativeCardinality);
        }
        let ids: BTreeSet<_> = alternatives
            .iter()
            .map(|alternative| *alternative.alternative_id())
            .collect();
        if ids.len() != alternatives.len() {
            return Err(DecisionV1Error::DuplicateAlternative);
        }
        for alternative in &alternatives {
            if let Some(plan_base) = alternative.consequence.base_contract_root_id()
                && plan_base != &base_contract_root_id
            {
                return Err(DecisionV1Error::AlternativeBaseRootMismatch);
            }
        }
        let value = revision_value(
            &decision_id,
            ordinal,
            parent_revision_id.as_ref(),
            &question,
            subject_hash,
            &base_contract_root_id,
            &alternatives,
        );
        let revision_id = DecisionRevisionIdV1::from_digest(canonical_hash_v1(
            "maestro.vnext.decision-revision.v1",
            value,
        )?);
        Ok(Self {
            decision_id,
            ordinal,
            parent_revision_id,
            question,
            subject_hash,
            base_contract_root_id,
            alternatives,
            revision_id,
        })
    }

    pub const fn revision_id(&self) -> &DecisionRevisionIdV1 {
        &self.revision_id
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn parent_revision_id(&self) -> Option<&DecisionRevisionIdV1> {
        self.parent_revision_id.as_ref()
    }

    pub fn decision_id(&self) -> &DecisionIdV1 {
        &self.decision_id
    }

    pub fn alternatives(&self) -> &[AlternativeV1] {
        &self.alternatives
    }

    pub const fn base_contract_root_id(&self) -> &ContractRootIdV1 {
        &self.base_contract_root_id
    }

    pub fn question(&self) -> &[u8] {
        &self.question
    }

    pub const fn subject_hash(&self) -> &ExactRecordRefV1 {
        &self.subject_hash
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DecisionV1Error> {
        Ok(deterministic_cbor::encode(&revision_value(
            &self.decision_id,
            self.ordinal,
            self.parent_revision_id.as_ref(),
            &self.question,
            self.subject_hash,
            &self.base_contract_root_id,
            &self.alternatives,
        ))?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlternativeRejectionV1 {
    alternative_id: AlternativeIdV1,
    reason: Vec<u8>,
}

impl AlternativeRejectionV1 {
    pub fn new(alternative_id: AlternativeIdV1, reason: Vec<u8>) -> Result<Self, DecisionV1Error> {
        if reason.is_empty() {
            return Err(DecisionV1Error::EmptyRejectedAlternativeReason);
        }
        Ok(Self {
            alternative_id,
            reason,
        })
    }

    pub const fn alternative_id(&self) -> &AlternativeIdV1 {
        &self.alternative_id
    }

    pub fn reason(&self) -> &[u8] {
        &self.reason
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.alternative_id.as_bytes().to_vec()),
            CborValue::Bytes(self.reason.clone()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionV1 {
    decision_id: DecisionIdV1,
    revision_id: DecisionRevisionIdV1,
    selected_alternative_id: AlternativeIdV1,
    selected_consequence: AlternativeConsequenceV1,
    subject_hash: ExactRecordRefV1,
    base_contract_root_id: ContractRootIdV1,
    rationale: Vec<u8>,
    rejected_alternatives: Vec<AlternativeRejectionV1>,
    authorization: CommittedActionAuditV1,
    resolution_id: DecisionResolutionIdV1,
}

impl ResolutionV1 {
    pub fn resolution_id(&self) -> &DecisionResolutionIdV1 {
        &self.resolution_id
    }

    pub const fn revision_id(&self) -> &DecisionRevisionIdV1 {
        &self.revision_id
    }

    pub const fn selected_alternative_id(&self) -> &AlternativeIdV1 {
        &self.selected_alternative_id
    }

    pub fn rejected_alternatives(&self) -> &[AlternativeRejectionV1] {
        &self.rejected_alternatives
    }

    pub fn decision_id(&self) -> &DecisionIdV1 {
        &self.decision_id
    }

    pub const fn selected_consequence(&self) -> &AlternativeConsequenceV1 {
        &self.selected_consequence
    }

    pub const fn subject_hash(&self) -> &ExactRecordRefV1 {
        &self.subject_hash
    }

    pub const fn base_contract_root_id(&self) -> &ContractRootIdV1 {
        &self.base_contract_root_id
    }

    pub fn rationale(&self) -> &[u8] {
        &self.rationale
    }

    pub const fn authorization(&self) -> &CommittedActionAuditV1 {
        &self.authorization
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DecisionV1Error> {
        Ok(deterministic_cbor::encode(&resolution_value(
            &self.decision_id,
            &self.revision_id,
            &self.selected_alternative_id,
            &self.selected_consequence,
            self.subject_hash,
            &self.base_contract_root_id,
            &self.rationale,
            &self.rejected_alternatives,
            &self.authorization,
        ))?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalV1 {
    reason: Vec<u8>,
}

impl WithdrawalV1 {
    pub fn reason(&self) -> &[u8] {
        &self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionStateV1 {
    Open,
    Resolved(ResolutionV1),
    Withdrawn(WithdrawalV1),
    Superseded {
        resolution: ResolutionV1,
        supersession: DecisionSupersessionV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkDecisionEligibilityV1 {
    Eligible,
    TerminalWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionV1 {
    repository_installation_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    decision_id: DecisionIdV1,
    revisions: Vec<DecisionRevisionV1>,
    state: DecisionStateV1,
}

impl DecisionV1 {
    pub fn new(
        repository_installation_id: StoreDomainIdV1,
        work_id: WorkIdV1,
        decision_id: DecisionIdV1,
        genesis: DecisionRevisionV1,
    ) -> Result<Self, DecisionV1Error> {
        if genesis.decision_id != decision_id
            || genesis.ordinal != 1
            || genesis.parent_revision_id.is_some()
        {
            return Err(DecisionV1Error::InvalidDecisionGenesis);
        }
        Ok(Self {
            repository_installation_id,
            work_id,
            decision_id,
            revisions: vec![genesis],
            state: DecisionStateV1::Open,
        })
    }

    pub fn append_revision(
        &self,
        expected_head: &DecisionRevisionIdV1,
        revision: DecisionRevisionV1,
        eligibility: WorkDecisionEligibilityV1,
    ) -> Result<Self, DecisionV1Error> {
        self.require_open_and_eligible(eligibility)?;
        let head = self.head();
        if head.revision_id() != expected_head {
            return Err(DecisionV1Error::StaleDecisionHead);
        }
        if revision.decision_id != self.decision_id
            || revision.ordinal != head.ordinal + 1
            || revision.parent_revision_id() != Some(expected_head)
        {
            return Err(DecisionV1Error::InvalidRevisionSuccessor);
        }
        if self.revisions.len() >= MAX_DECISION_REVISIONS_V1 {
            return Err(DecisionV1Error::TooManyDecisionRevisions);
        }
        let mut revisions = self.revisions.clone();
        revisions.push(revision);
        Ok(Self {
            repository_installation_id: self.repository_installation_id,
            work_id: self.work_id,
            decision_id: self.decision_id.clone(),
            revisions,
            state: DecisionStateV1::Open,
        })
    }

    pub fn resolve(
        &self,
        expected_head: &DecisionRevisionIdV1,
        selected_alternative_id: &AlternativeIdV1,
        rationale: Vec<u8>,
        mut rejected_alternatives: Vec<AlternativeRejectionV1>,
        authorization: &AdmittedCommittedActionV1,
        eligibility: WorkDecisionEligibilityV1,
    ) -> Result<Self, DecisionV1Error> {
        self.require_open_and_eligible(eligibility)?;
        let head = self.head();
        if head.revision_id() != expected_head {
            return Err(DecisionV1Error::StaleDecisionHead);
        }
        if rationale.is_empty() {
            return Err(DecisionV1Error::EmptyResolutionRationale);
        }
        let selected = head
            .alternatives
            .iter()
            .find(|alternative| alternative.alternative_id() == selected_alternative_id)
            .ok_or(DecisionV1Error::UnknownSelectedAlternative)?;
        rejected_alternatives.sort_by_key(|rejection| rejection.alternative_id);
        if rejected_alternatives
            .windows(2)
            .any(|pair| pair[0].alternative_id == pair[1].alternative_id)
        {
            return Err(DecisionV1Error::DuplicateRejectedAlternative);
        }
        let mut expected_rejected: Vec<_> = head
            .alternatives
            .iter()
            .map(|alternative| *alternative.alternative_id())
            .filter(|alternative_id| alternative_id != selected_alternative_id)
            .collect();
        expected_rejected.sort_unstable();
        let actual_rejected: Vec<_> = rejected_alternatives
            .iter()
            .map(|rejection| rejection.alternative_id)
            .collect();
        if actual_rejected != expected_rejected {
            return Err(DecisionV1Error::IncompleteRejectedAlternativeReasons);
        }
        let resolution_value = resolution_value(
            &self.decision_id,
            head.revision_id(),
            selected_alternative_id,
            &selected.consequence,
            head.subject_hash,
            &head.base_contract_root_id,
            &rationale,
            &rejected_alternatives,
            authorization.audit(),
        );
        let resolution_id = decision_resolution_identity(&resolution_value)?;
        let resolution = ResolutionV1 {
            decision_id: self.decision_id.clone(),
            revision_id: *head.revision_id(),
            selected_alternative_id: *selected_alternative_id,
            selected_consequence: selected.consequence.clone(),
            subject_hash: head.subject_hash,
            base_contract_root_id: head.base_contract_root_id,
            rationale,
            rejected_alternatives,
            authorization: authorization.audit().clone(),
            resolution_id,
        };
        let mut closed = self.clone();
        closed.state = DecisionStateV1::Resolved(resolution);
        Ok(closed)
    }

    pub fn withdraw(
        &self,
        reason: Vec<u8>,
        eligibility: WorkDecisionEligibilityV1,
    ) -> Result<Self, DecisionV1Error> {
        self.require_open_and_eligible(eligibility)?;
        if reason.is_empty() {
            return Err(DecisionV1Error::EmptyWithdrawalReason);
        }
        let mut closed = self.clone();
        closed.state = DecisionStateV1::Withdrawn(WithdrawalV1 { reason });
        Ok(closed)
    }

    pub fn decision_id(&self) -> &DecisionIdV1 {
        &self.decision_id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn repository_installation_id(&self) -> &StoreDomainIdV1 {
        &self.repository_installation_id
    }

    pub fn revisions(&self) -> &[DecisionRevisionV1] {
        &self.revisions
    }

    pub fn head(&self) -> &DecisionRevisionV1 {
        self.revisions
            .last()
            .expect("invariant: Decision contains genesis")
    }

    pub const fn state(&self) -> &DecisionStateV1 {
        &self.state
    }

    pub const fn resolution(&self) -> Option<&ResolutionV1> {
        match &self.state {
            DecisionStateV1::Resolved(resolution)
            | DecisionStateV1::Superseded { resolution, .. } => Some(resolution),
            DecisionStateV1::Open | DecisionStateV1::Withdrawn(_) => None,
        }
    }

    fn require_open_and_eligible(
        &self,
        eligibility: WorkDecisionEligibilityV1,
    ) -> Result<(), DecisionV1Error> {
        if eligibility == WorkDecisionEligibilityV1::TerminalWork {
            return Err(DecisionV1Error::TerminalWorkRejectsDecisionChange);
        }
        if !matches!(self.state, DecisionStateV1::Open) {
            return Err(DecisionV1Error::DecisionNotOpen);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionSupersessionV1 {
    predecessor_id: DecisionIdV1,
    successor_id: DecisionIdV1,
    authorization_receipt: SupersessionAuthorizationReceiptRefV1,
    supersession_id: DecisionSupersessionIdV1,
}

impl DecisionSupersessionV1 {
    pub fn predecessor_id(&self) -> &DecisionIdV1 {
        &self.predecessor_id
    }

    pub fn successor_id(&self) -> &DecisionIdV1 {
        &self.successor_id
    }

    pub const fn supersession_id(&self) -> &DecisionSupersessionIdV1 {
        &self.supersession_id
    }

    pub const fn authorization_receipt(&self) -> &SupersessionAuthorizationReceiptRefV1 {
        &self.authorization_receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionLineageV1 {
    work_id: WorkIdV1,
    edges: Vec<DecisionSupersessionV1>,
}

impl DecisionLineageV1 {
    pub const fn new(work_id: WorkIdV1) -> Self {
        Self {
            work_id,
            edges: Vec::new(),
        }
    }

    pub fn supersede(
        &self,
        predecessor: &DecisionV1,
        successor: &DecisionV1,
        authorization_receipt: SupersessionAuthorizationReceiptRefV1,
    ) -> Result<(Self, DecisionV1), DecisionV1Error> {
        if predecessor.work_id != self.work_id || successor.work_id != self.work_id {
            return Err(DecisionV1Error::SupersessionWorkMismatch);
        }
        if predecessor.decision_id == successor.decision_id {
            return Err(DecisionV1Error::SelfSupersession);
        }
        if self
            .edges
            .iter()
            .any(|edge| edge.predecessor_id == predecessor.decision_id)
        {
            return Err(DecisionV1Error::SupersessionPredecessorAlreadyHasSuccessor);
        }
        if self
            .edges
            .iter()
            .any(|edge| edge.successor_id == successor.decision_id)
        {
            return Err(DecisionV1Error::SupersessionSuccessorAlreadyHasPredecessor);
        }
        if self.path_exists(&successor.decision_id, &predecessor.decision_id) {
            return Err(DecisionV1Error::SupersessionCycle);
        }
        let predecessor_resolution = match &predecessor.state {
            DecisionStateV1::Resolved(resolution) => resolution.clone(),
            _ => return Err(DecisionV1Error::SupersessionPredecessorNotResolved),
        };
        if !matches!(successor.state, DecisionStateV1::Resolved(_)) {
            return Err(DecisionV1Error::SupersessionSuccessorNotResolved);
        }
        let value = CborValue::Array(vec![
            CborValue::Unsigned(SUPERSESSION_VERSION_V1),
            CborValue::Bytes(self.work_id.as_bytes().to_vec()),
            predecessor.decision_id.canonical_value(),
            successor.decision_id.canonical_value(),
            authorization_receipt.canonical_value(),
        ]);
        let supersession_id = DecisionSupersessionIdV1::from_digest(canonical_hash_v1(
            "maestro.vnext.decision-supersession.v1",
            value,
        )?);
        let edge = DecisionSupersessionV1 {
            predecessor_id: predecessor.decision_id.clone(),
            successor_id: successor.decision_id.clone(),
            authorization_receipt,
            supersession_id,
        };
        let mut lineage = self.clone();
        lineage.edges.push(edge.clone());
        lineage
            .edges
            .sort_by(|left, right| left.predecessor_id.cmp(&right.predecessor_id));
        let mut superseded = predecessor.clone();
        superseded.state = DecisionStateV1::Superseded {
            resolution: predecessor_resolution,
            supersession: edge,
        };
        Ok((lineage, superseded))
    }

    pub fn edges(&self) -> &[DecisionSupersessionV1] {
        &self.edges
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    fn path_exists(&self, start: &DecisionIdV1, goal: &DecisionIdV1) -> bool {
        let adjacency: BTreeMap<_, _> = self
            .edges
            .iter()
            .map(|edge| (&edge.predecessor_id, &edge.successor_id))
            .collect();
        let mut current = Some(start);
        let mut seen = BTreeSet::new();
        while let Some(node) = current {
            if node == goal {
                return true;
            }
            if !seen.insert(node) {
                return false;
            }
            current = adjacency.get(node).copied();
        }
        false
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DecisionV1Error {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("Alternative meaning and preview must be non-empty")]
    EmptyAlternativeContent,
    #[error("typed consequence-plan kind must be positive")]
    InvalidConsequencePlanKind,
    #[error("Decision Revision must have 2..N Alternatives")]
    AlternativeCardinality,
    #[error("Decision Revision repeats an Alternative identity")]
    DuplicateAlternative,
    #[error("Decision Revision ordinal and parent do not form a closed chain")]
    InvalidRevisionParentage,
    #[error("Decision question must be non-empty")]
    EmptyDecisionQuestion,
    #[error("Alternative consequence-plan base does not match the Decision Revision base")]
    AlternativeBaseRootMismatch,
    #[error("Decision genesis does not bind ordinal one with no parent")]
    InvalidDecisionGenesis,
    #[error("Decision is not open")]
    DecisionNotOpen,
    #[error("terminal Work rejects Decision changes")]
    TerminalWorkRejectsDecisionChange,
    #[error("Decision head changed since it was read")]
    StaleDecisionHead,
    #[error("Decision Revision is not the exact next revision")]
    InvalidRevisionSuccessor,
    #[error("Decision exceeds its finite Revision bound")]
    TooManyDecisionRevisions,
    #[error("Resolution rationale must be non-empty")]
    EmptyResolutionRationale,
    #[error("Resolution selects an Alternative outside the exact Decision Revision")]
    UnknownSelectedAlternative,
    #[error("Resolution repeats a rejected Alternative")]
    DuplicateRejectedAlternative,
    #[error("Resolution must explain every and only unselected Alternative")]
    IncompleteRejectedAlternativeReasons,
    #[error("rejected Alternative reason must be non-empty")]
    EmptyRejectedAlternativeReason,
    #[error("withdrawal reason must be non-empty")]
    EmptyWithdrawalReason,
    #[error("Decision Supersession must stay within one Work")]
    SupersessionWorkMismatch,
    #[error("Decision cannot supersede itself")]
    SelfSupersession,
    #[error("Decision predecessor already has a successor")]
    SupersessionPredecessorAlreadyHasSuccessor,
    #[error("Decision successor already has a predecessor")]
    SupersessionSuccessorAlreadyHasPredecessor,
    #[error("Decision Supersession graph must remain acyclic")]
    SupersessionCycle,
    #[error("Decision Supersession predecessor must be resolved")]
    SupersessionPredecessorNotResolved,
    #[error("Decision Supersession successor must be independently resolved")]
    SupersessionSuccessorNotResolved,
    #[error("Decision batch must contain 2..N exact Resolutions")]
    BatchCardinality,
    #[error("Decision batch repeats a Resolution or Decision")]
    DuplicateBatchResolution,
    #[error("Decision Closure expected set does not match all supplied Work-owned Decisions")]
    IncompleteDecisionClosure,
    #[error("Decision Closure contains an open Decision")]
    DecisionClosureContainsOpenDecision,
    #[error("normative Resolution lacks a complete materialization")]
    UnappliedNormativeDecision,
    #[error(
        "Decision Closure contains a materialization for a non-normative or unknown Resolution"
    )]
    UnexpectedDecisionMaterialization,
    #[error("Decision Closure lineage and Decision terminal states disagree")]
    DecisionLineageMismatch,
    #[error(
        "Decision Closure cannot recompute one exact Resolution-plan-materialization root chain"
    )]
    DecisionMaterializationRootJoinMismatch,
}

fn alternative_value(
    meaning: &[u8],
    preview: &[u8],
    consequence: &AlternativeConsequenceV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(ALTERNATIVE_VERSION_V1),
        CborValue::Bytes(meaning.to_vec()),
        CborValue::Bytes(preview.to_vec()),
        consequence.canonical_value(),
    ])
}

fn revision_value(
    decision_id: &DecisionIdV1,
    ordinal: u32,
    parent: Option<&DecisionRevisionIdV1>,
    question: &[u8],
    subject_hash: ExactRecordRefV1,
    base_root: &ContractRootIdV1,
    alternatives: &[AlternativeV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DECISION_REVISION_VERSION_V1),
        decision_id.canonical_value(),
        CborValue::Unsigned(u64::from(ordinal)),
        optional_digest_v1(parent.map(DecisionRevisionIdV1::as_bytes)),
        CborValue::Bytes(question.to_vec()),
        subject_hash.canonical_value(),
        CborValue::Bytes(base_root.as_bytes().to_vec()),
        CborValue::Array(
            alternatives
                .iter()
                .map(AlternativeV1::canonical_value)
                .collect(),
        ),
    ])
}

#[expect(
    clippy::too_many_arguments,
    reason = "canonical Resolution identity includes every exact selection and authority binding"
)]
fn resolution_value(
    decision_id: &DecisionIdV1,
    revision_id: &DecisionRevisionIdV1,
    selected_alternative_id: &AlternativeIdV1,
    selected_consequence: &AlternativeConsequenceV1,
    subject_hash: ExactRecordRefV1,
    base_root: &ContractRootIdV1,
    rationale: &[u8],
    rejected: &[AlternativeRejectionV1],
    authorization: &CommittedActionAuditV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(RESOLUTION_VERSION_V1),
        decision_id.canonical_value(),
        CborValue::Bytes(revision_id.as_bytes().to_vec()),
        CborValue::Bytes(selected_alternative_id.as_bytes().to_vec()),
        selected_consequence.canonical_value(),
        subject_hash.canonical_value(),
        CborValue::Bytes(base_root.as_bytes().to_vec()),
        CborValue::Bytes(rationale.to_vec()),
        CborValue::Array(
            rejected
                .iter()
                .map(AlternativeRejectionV1::canonical_value)
                .collect(),
        ),
        authorization.canonical_value(),
    ])
}
