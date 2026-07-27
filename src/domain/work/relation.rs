use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::identity::{ContractRootIdV1, StoreDomainIdV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{
    WorkIdV1, WorkLifecycleStateV1, WorkRecordWriterV1, WorkRelationIdV1, WorkRequirementIdV1,
    WorkRevisionV1,
};

pub const WORK_RELATION_VERSION_V1: u64 = 1;
pub const MAX_WORK_GRAPH_ENDPOINTS_V1: usize = 4_096;
pub const MAX_WORK_RELATIONS_V1: usize = 8_192;
pub const MAX_WORK_REQUIREMENTS_V1: usize = 4_096;
pub const MAX_PUBLISHED_ROOTS_PER_WORK_V1: usize = 256;
pub const MAX_STEP_REFERENCE_BYTES_V1: usize = 256;
pub const MAX_RELATION_TEXT_BYTES_V1: usize = 1_024;
pub const MAX_IDEMPOTENCY_KEY_BYTES_V1: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactStepRevisionRefV1 {
    step_id: String,
    revision: u64,
}

impl ExactStepRevisionRefV1 {
    pub fn new(step_id: impl Into<String>, revision: u64) -> Result<Self, WorkRelationError> {
        let step_id = step_id.into();
        if step_id.is_empty() || step_id.len() > MAX_STEP_REFERENCE_BYTES_V1 || !step_id.is_ascii()
        {
            return Err(WorkRelationError::InvalidStepReference);
        }
        if revision == 0 {
            return Err(WorkRelationError::InvalidStepRevision);
        }
        Ok(Self { step_id, revision })
    }

    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkRequirementScopeV1 {
    BeforeExecution,
    BeforeStep(ExactStepRevisionRefV1),
    BeforeCompletion,
}

impl WorkRequirementScopeV1 {
    pub const fn tag(&self) -> u64 {
        match self {
            Self::BeforeExecution => 1,
            Self::BeforeStep(_) => 2,
            Self::BeforeCompletion => 3,
        }
    }

    pub fn from_tag(
        tag: u64,
        step: Option<ExactStepRevisionRefV1>,
    ) -> Result<Self, WorkRelationError> {
        match (tag, step) {
            (1, None) => Ok(Self::BeforeExecution),
            (2, Some(step)) => Ok(Self::BeforeStep(step)),
            (3, None) => Ok(Self::BeforeCompletion),
            (1 | 3, Some(_)) | (2, None) => Err(WorkRelationError::InvalidScopePayload),
            _ => Err(WorkRelationError::UnknownRequirementScopeTag(tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkRequirementV1 {
    id: WorkRequirementIdV1,
    consumer_repository_id: StoreDomainIdV1,
    consumer_work_id: WorkIdV1,
    target_repository_id: StoreDomainIdV1,
    target_work_id: WorkIdV1,
    target_contract_root: ContractRootIdV1,
    scope: WorkRequirementScopeV1,
}

impl WorkRequirementV1 {
    pub fn new(
        id: WorkRequirementIdV1,
        consumer_repository_id: StoreDomainIdV1,
        consumer_work_id: WorkIdV1,
        target_repository_id: StoreDomainIdV1,
        target_work_id: WorkIdV1,
        target_contract_root: ContractRootIdV1,
        scope: WorkRequirementScopeV1,
    ) -> Result<Self, WorkRelationError> {
        if consumer_repository_id != target_repository_id {
            return Err(WorkRelationError::CrossRepository);
        }
        if consumer_work_id == target_work_id {
            return Err(WorkRelationError::SelfEdge);
        }
        if *target_contract_root.as_bytes() == [0_u8; 32] {
            return Err(WorkRelationError::EmptyContractRoot);
        }
        Ok(Self {
            id,
            consumer_repository_id,
            consumer_work_id,
            target_repository_id,
            target_work_id,
            target_contract_root,
            scope,
        })
    }

    pub fn id(&self) -> WorkRequirementIdV1 {
        self.id
    }

    pub fn consumer_repository_id(&self) -> StoreDomainIdV1 {
        self.consumer_repository_id
    }

    pub fn consumer_work_id(&self) -> WorkIdV1 {
        self.consumer_work_id
    }

    pub fn target_repository_id(&self) -> StoreDomainIdV1 {
        self.target_repository_id
    }

    pub fn target_work_id(&self) -> WorkIdV1 {
        self.target_work_id
    }

    pub fn target_contract_root(&self) -> ContractRootIdV1 {
        self.target_contract_root
    }

    pub fn scope(&self) -> &WorkRequirementScopeV1 {
        &self.scope
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, WorkRelationError> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(WORK_RELATION_VERSION_V1),
            CborValue::Bytes(self.id.as_bytes().to_vec()),
            CborValue::Bytes(self.consumer_repository_id.as_bytes().to_vec()),
            CborValue::Bytes(self.consumer_work_id.as_bytes().to_vec()),
            CborValue::Bytes(self.target_repository_id.as_bytes().to_vec()),
            CborValue::Bytes(self.target_work_id.as_bytes().to_vec()),
            CborValue::Bytes(self.target_contract_root.as_bytes().to_vec()),
            encode_scope(&self.scope),
        ]))?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkRelationKindV1 {
    SupersededBy,
    Corrects,
    Continues,
    Reference,
}

impl WorkRelationKindV1 {
    pub const ALL: [Self; 4] = [
        Self::SupersededBy,
        Self::Corrects,
        Self::Continues,
        Self::Reference,
    ];

    pub const fn tag(self) -> u64 {
        match self {
            Self::SupersededBy => 1,
            Self::Corrects => 2,
            Self::Continues => 3,
            Self::Reference => 4,
        }
    }

    pub fn from_tag(tag: u64) -> Result<Self, WorkRelationError> {
        match tag {
            1 => Ok(Self::SupersededBy),
            2 => Ok(Self::Corrects),
            3 => Ok(Self::Continues),
            4 => Ok(Self::Reference),
            _ => Err(WorkRelationError::UnknownRelationKindTag(tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkRelationEndpointV1 {
    repository_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    expected_revision: WorkRevisionV1,
    observed_contract_root: Option<ContractRootIdV1>,
}

impl WorkRelationEndpointV1 {
    pub fn new(
        repository_id: StoreDomainIdV1,
        work_id: WorkIdV1,
        expected_revision: WorkRevisionV1,
        observed_contract_root: Option<ContractRootIdV1>,
    ) -> Result<Self, WorkRelationError> {
        if observed_contract_root
            .as_ref()
            .is_some_and(|root| *root.as_bytes() == [0_u8; 32])
        {
            return Err(WorkRelationError::EmptyContractRoot);
        }
        Ok(Self {
            repository_id,
            work_id,
            expected_revision,
            observed_contract_root,
        })
    }

    pub fn repository_id(&self) -> StoreDomainIdV1 {
        self.repository_id
    }

    pub fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn expected_revision(&self) -> WorkRevisionV1 {
        self.expected_revision
    }

    pub fn observed_contract_root(&self) -> Option<ContractRootIdV1> {
        self.observed_contract_root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkRelationRecordV1 {
    id: WorkRelationIdV1,
    kind: WorkRelationKindV1,
    source: WorkRelationEndpointV1,
    target: WorkRelationEndpointV1,
    reason: String,
    provenance: String,
    idempotency_key: String,
}

impl WorkRelationRecordV1 {
    pub fn new(
        id: WorkRelationIdV1,
        kind: WorkRelationKindV1,
        source: WorkRelationEndpointV1,
        target: WorkRelationEndpointV1,
        reason: impl Into<String>,
        provenance: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, WorkRelationError> {
        if source.repository_id != target.repository_id {
            return Err(WorkRelationError::CrossRepository);
        }
        if source.work_id == target.work_id {
            return Err(WorkRelationError::SelfEdge);
        }
        let reason = validate_relation_text(reason.into(), "reason")?;
        let provenance = validate_relation_text(provenance.into(), "provenance")?;
        let idempotency_key = validate_idempotency_key(idempotency_key.into())?;
        Ok(Self {
            id,
            kind,
            source,
            target,
            reason,
            provenance,
            idempotency_key,
        })
    }

    pub fn id(&self) -> WorkRelationIdV1 {
        self.id
    }

    pub fn kind(&self) -> WorkRelationKindV1 {
        self.kind
    }

    pub fn source(&self) -> &WorkRelationEndpointV1 {
        &self.source
    }

    pub fn target(&self) -> &WorkRelationEndpointV1 {
        &self.target
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, WorkRelationError> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(WORK_RELATION_VERSION_V1),
            CborValue::Bytes(self.id.as_bytes().to_vec()),
            CborValue::Unsigned(self.kind.tag()),
            encode_endpoint(&self.source),
            encode_endpoint(&self.target),
            CborValue::text(&self.reason)?,
            CborValue::text(&self.provenance)?,
            CborValue::text(&self.idempotency_key)?,
        ]))?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkSnapshotV1 {
    repository_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    revision: WorkRevisionV1,
    state: WorkLifecycleStateV1,
    published_contract_roots: Vec<ContractRootIdV1>,
}

impl WorkSnapshotV1 {
    pub fn new(
        repository_id: StoreDomainIdV1,
        work_id: WorkIdV1,
        revision: WorkRevisionV1,
        state: WorkLifecycleStateV1,
        mut published_contract_roots: Vec<ContractRootIdV1>,
    ) -> Result<Self, WorkRelationError> {
        if published_contract_roots.len() > MAX_PUBLISHED_ROOTS_PER_WORK_V1 {
            return Err(WorkRelationError::PublishedRootLimitExceeded);
        }
        if published_contract_roots
            .iter()
            .any(|root| *root.as_bytes() == [0_u8; 32])
        {
            return Err(WorkRelationError::EmptyContractRoot);
        }
        published_contract_roots.sort_unstable();
        if published_contract_roots
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(WorkRelationError::DuplicatePublishedRoot);
        }
        Ok(Self {
            repository_id,
            work_id,
            revision,
            state,
            published_contract_roots,
        })
    }

    pub fn repository_id(&self) -> StoreDomainIdV1 {
        self.repository_id
    }

    pub fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn revision(&self) -> WorkRevisionV1 {
        self.revision
    }

    pub fn state(&self) -> &WorkLifecycleStateV1 {
        &self.state
    }

    pub fn published_contract_roots(&self) -> &[ContractRootIdV1] {
        &self.published_contract_roots
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkRelationAdmissionV1 {
    Inserted,
    AlreadyPresent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkRelationGraphV1 {
    repository_id: StoreDomainIdV1,
    endpoints: BTreeMap<WorkIdV1, WorkSnapshotV1>,
    requirements: Vec<WorkRequirementV1>,
    relations: Vec<WorkRelationRecordV1>,
}

impl WorkRelationGraphV1 {
    pub fn new(
        repository_id: StoreDomainIdV1,
        endpoints: Vec<WorkSnapshotV1>,
    ) -> Result<Self, WorkRelationError> {
        if endpoints.len() > MAX_WORK_GRAPH_ENDPOINTS_V1 {
            return Err(WorkRelationError::EndpointLimitExceeded);
        }
        let mut indexed = BTreeMap::new();
        for endpoint in endpoints {
            if endpoint.repository_id != repository_id {
                return Err(WorkRelationError::CrossRepository);
            }
            let work_id = endpoint.work_id;
            if indexed.insert(work_id, endpoint).is_some() {
                return Err(WorkRelationError::DuplicateEndpoint(work_id));
            }
        }
        Ok(Self {
            repository_id,
            endpoints: indexed,
            requirements: Vec::new(),
            relations: Vec::new(),
        })
    }

    pub fn repository_id(&self) -> StoreDomainIdV1 {
        self.repository_id
    }

    pub fn requirements(&self) -> &[WorkRequirementV1] {
        &self.requirements
    }

    pub fn relations(&self) -> &[WorkRelationRecordV1] {
        &self.relations
    }

    pub fn admit_requirement(
        &mut self,
        writer: WorkRecordWriterV1,
        requirement: WorkRequirementV1,
    ) -> Result<WorkRelationAdmissionV1, WorkRelationError> {
        if writer != WorkRecordWriterV1::Contract {
            return Err(WorkRelationError::ForeignRequirementWriter(writer));
        }
        if requirement.consumer_repository_id != self.repository_id
            || requirement.target_repository_id != self.repository_id
        {
            return Err(WorkRelationError::CrossRepository);
        }
        if let Some(existing) = self
            .requirements
            .iter()
            .find(|existing| existing.id == requirement.id)
        {
            return if existing == &requirement {
                Ok(WorkRelationAdmissionV1::AlreadyPresent)
            } else {
                Err(WorkRelationError::IdentityConflict)
            };
        }
        if self.requirements.len() >= MAX_WORK_REQUIREMENTS_V1 {
            return Err(WorkRelationError::RequirementLimitExceeded);
        }
        self.endpoint(requirement.consumer_work_id)?;
        let target = self.endpoint(requirement.target_work_id)?;
        if !target
            .published_contract_roots
            .contains(&requirement.target_contract_root)
        {
            return Err(WorkRelationError::UnknownTargetContractRoot);
        }
        if self.would_create_requirement_cycle(
            requirement.consumer_work_id,
            requirement.target_work_id,
        ) {
            return Err(WorkRelationError::Cycle);
        }
        self.requirements.push(requirement);
        Ok(WorkRelationAdmissionV1::Inserted)
    }

    pub fn admit_relation(
        &mut self,
        writer: WorkRecordWriterV1,
        relation: WorkRelationRecordV1,
    ) -> Result<WorkRelationAdmissionV1, WorkRelationError> {
        if writer != WorkRecordWriterV1::Work {
            return Err(WorkRelationError::ForeignRelationWriter(writer));
        }
        if relation.source.repository_id != self.repository_id
            || relation.target.repository_id != self.repository_id
        {
            return Err(WorkRelationError::CrossRepository);
        }
        if let Some(existing) = self.relations.iter().find(|item| item.id == relation.id) {
            return if existing == &relation {
                Ok(WorkRelationAdmissionV1::AlreadyPresent)
            } else {
                Err(WorkRelationError::IdentityConflict)
            };
        }
        if let Some(existing) = self
            .relations
            .iter()
            .find(|item| item.idempotency_key == relation.idempotency_key)
        {
            return if existing == &relation {
                Ok(WorkRelationAdmissionV1::AlreadyPresent)
            } else {
                Err(WorkRelationError::IdempotencyConflict)
            };
        }
        if self.relations.len() >= MAX_WORK_RELATIONS_V1 {
            return Err(WorkRelationError::RelationLimitExceeded);
        }
        let source = self.validate_endpoint(&relation.source)?;
        let target = self.validate_endpoint(&relation.target)?;
        match relation.kind {
            WorkRelationKindV1::SupersededBy => {
                if self.relations.iter().any(|existing| {
                    existing.kind == WorkRelationKindV1::SupersededBy
                        && existing.source.work_id == relation.source.work_id
                }) {
                    return Err(WorkRelationError::SupersessionCardinality);
                }
                match &source.state {
                    WorkLifecycleStateV1::Draft
                    | WorkLifecycleStateV1::Ready
                    | WorkLifecycleStateV1::Active
                    | WorkLifecycleStateV1::AwaitingAcceptance => {}
                    WorkLifecycleStateV1::Superseded { successor }
                        if *successor == relation.target.work_id => {}
                    WorkLifecycleStateV1::Superseded { .. } => {
                        return Err(WorkRelationError::SupersessionStateMismatch);
                    }
                    WorkLifecycleStateV1::Completed | WorkLifecycleStateV1::Cancelled => {
                        return Err(WorkRelationError::SupersessionSourceIneligible);
                    }
                }
            }
            WorkRelationKindV1::Corrects => {
                if target.state != WorkLifecycleStateV1::Completed {
                    return Err(WorkRelationError::CorrectionTargetNotCompleted);
                }
            }
            WorkRelationKindV1::Continues => {
                if !matches!(
                    target.state,
                    WorkLifecycleStateV1::Completed | WorkLifecycleStateV1::Cancelled
                ) {
                    return Err(WorkRelationError::ContinuationTargetIneligible);
                }
            }
            WorkRelationKindV1::Reference => {}
        }
        if relation.kind != WorkRelationKindV1::Reference
            && self.would_create_relation_cycle(
                relation.kind,
                relation.source.work_id,
                relation.target.work_id,
            )
        {
            return Err(WorkRelationError::Cycle);
        }
        self.relations.push(relation);
        Ok(WorkRelationAdmissionV1::Inserted)
    }

    fn endpoint(&self, work_id: WorkIdV1) -> Result<&WorkSnapshotV1, WorkRelationError> {
        self.endpoints
            .get(&work_id)
            .ok_or(WorkRelationError::UnknownEndpoint(work_id))
    }

    fn validate_endpoint(
        &self,
        endpoint: &WorkRelationEndpointV1,
    ) -> Result<&WorkSnapshotV1, WorkRelationError> {
        let snapshot = self.endpoint(endpoint.work_id)?;
        if snapshot.revision != endpoint.expected_revision {
            return Err(WorkRelationError::StaleEndpointRevision {
                work_id: endpoint.work_id,
                expected: endpoint.expected_revision.get(),
                actual: snapshot.revision.get(),
            });
        }
        if let Some(root) = endpoint.observed_contract_root
            && !snapshot.published_contract_roots.contains(&root)
        {
            return Err(WorkRelationError::UnknownObservedContractRoot);
        }
        Ok(snapshot)
    }

    fn would_create_requirement_cycle(&self, source: WorkIdV1, target: WorkIdV1) -> bool {
        reachable(
            target,
            source,
            self.requirements
                .iter()
                .map(|requirement| (requirement.consumer_work_id, requirement.target_work_id)),
        )
    }

    fn would_create_relation_cycle(
        &self,
        kind: WorkRelationKindV1,
        source: WorkIdV1,
        target: WorkIdV1,
    ) -> bool {
        reachable(
            target,
            source,
            self.relations
                .iter()
                .filter(move |relation| relation.kind == kind)
                .map(|relation| (relation.source.work_id, relation.target.work_id)),
        )
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkRelationError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("unknown Work Requirement enforcement-scope tag {0}")]
    UnknownRequirementScopeTag(u64),
    #[error("Work Requirement enforcement scope has an invalid payload")]
    InvalidScopePayload,
    #[error("unknown Work relation kind tag {0}")]
    UnknownRelationKindTag(u64),
    #[error("Work Requirement Step reference must contain 1..=256 ASCII bytes")]
    InvalidStepReference,
    #[error("Work Requirement Step revision must be positive")]
    InvalidStepRevision,
    #[error("Work relation endpoints must belong to one repository")]
    CrossRepository,
    #[error("Work relation endpoints must be distinct")]
    SelfEdge,
    #[error("Work Contract Root must not be all-zero")]
    EmptyContractRoot,
    #[error("Work relation {0} must contain 1..=1024 ASCII bytes")]
    InvalidRelationText(&'static str),
    #[error("Work relation idempotency key must contain 1..=256 ASCII bytes")]
    InvalidIdempotencyKey,
    #[error("Work graph exceeds its finite endpoint bound")]
    EndpointLimitExceeded,
    #[error("Work graph contains duplicate endpoint {0}")]
    DuplicateEndpoint(WorkIdV1),
    #[error("Work snapshot exceeds its finite published-root bound")]
    PublishedRootLimitExceeded,
    #[error("Work snapshot contains a duplicate published Contract Root")]
    DuplicatePublishedRoot,
    #[error("Work Requirement graph exceeds its finite v1 bound")]
    RequirementLimitExceeded,
    #[error("Work relation graph exceeds its finite v1 bound")]
    RelationLimitExceeded,
    #[error("{0:?} cannot write Contract-owned Work Requirements")]
    ForeignRequirementWriter(WorkRecordWriterV1),
    #[error("{0:?} cannot write Work-owned lineage or Reference records")]
    ForeignRelationWriter(WorkRecordWriterV1),
    #[error("Work relation identity already names different immutable content")]
    IdentityConflict,
    #[error("Work relation idempotency key already names different immutable content")]
    IdempotencyConflict,
    #[error("unknown Work relation endpoint {0}")]
    UnknownEndpoint(WorkIdV1),
    #[error("unknown exact published target Contract Root")]
    UnknownTargetContractRoot,
    #[error("unknown observed endpoint Contract Root")]
    UnknownObservedContractRoot,
    #[error("stale endpoint revision for {work_id}: expected {expected}, actual {actual}")]
    StaleEndpointRevision {
        work_id: WorkIdV1,
        expected: u64,
        actual: u64,
    },
    #[error("Work Requirement or lineage graph would contain a cycle")]
    Cycle,
    #[error("a predecessor may have at most one SupersededBy successor")]
    SupersessionCardinality,
    #[error("superseded Work state names a different successor")]
    SupersessionStateMismatch,
    #[error("completed or cancelled Work cannot be superseded")]
    SupersessionSourceIneligible,
    #[error("Corrects target Work must be completed")]
    CorrectionTargetNotCompleted,
    #[error("Continues target Work must be completed or cancelled")]
    ContinuationTargetIneligible,
}

fn validate_relation_text(value: String, field: &'static str) -> Result<String, WorkRelationError> {
    if value.is_empty() || value.len() > MAX_RELATION_TEXT_BYTES_V1 || !value.is_ascii() {
        return Err(WorkRelationError::InvalidRelationText(field));
    }
    Ok(value)
}

fn validate_idempotency_key(value: String) -> Result<String, WorkRelationError> {
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_BYTES_V1 || !value.is_ascii() {
        return Err(WorkRelationError::InvalidIdempotencyKey);
    }
    Ok(value)
}

fn encode_scope(scope: &WorkRequirementScopeV1) -> CborValue {
    let step = match scope {
        WorkRequirementScopeV1::BeforeStep(step) => Some(CborValue::Array(vec![
            CborValue::Text(step.step_id.clone()),
            CborValue::Unsigned(step.revision),
        ])),
        _ => None,
    };
    CborValue::Array(vec![
        CborValue::Unsigned(scope.tag()),
        CborValue::optional(step),
    ])
}

fn encode_endpoint(endpoint: &WorkRelationEndpointV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Bytes(endpoint.repository_id.as_bytes().to_vec()),
        CborValue::Bytes(endpoint.work_id.as_bytes().to_vec()),
        CborValue::Unsigned(endpoint.expected_revision.get()),
        CborValue::optional(
            endpoint
                .observed_contract_root
                .map(|root| CborValue::Bytes(root.as_bytes().to_vec())),
        ),
    ])
}

fn reachable(
    start: WorkIdV1,
    goal: WorkIdV1,
    edges: impl Iterator<Item = (WorkIdV1, WorkIdV1)>,
) -> bool {
    let mut adjacency: BTreeMap<WorkIdV1, Vec<WorkIdV1>> = BTreeMap::new();
    for (source, target) in edges {
        adjacency.entry(source).or_default().push(target);
    }
    let mut frontier = vec![start];
    let mut visited = BTreeSet::new();
    while let Some(current) = frontier.pop() {
        if current == goal {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        if let Some(next) = adjacency.get(&current) {
            frontier.extend(next.iter().copied());
        }
    }
    false
}
