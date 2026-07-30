use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::authority::{PrincipalIdV1, SessionIdV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const MAX_REF_BYTES_V1: usize = 1_024;
const MAX_MESSAGE_BODY_REF_BYTES_V1: usize = 4_096;
const MAX_AUDIENCE_V1: usize = 1_024;
pub(crate) const MAX_SUBJECT_REFS_V1: usize = 1_024;
const MAX_HANDOFF_EVIDENCE_REFS_V1: usize = 1_024;

macro_rules! coordination_identity {
    ($name:ident, $domain:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub(crate) struct $name([u8; 32]);

        impl $name {
            pub(crate) fn derive(seed: &str) -> Result<Self, CoordinationErrorV1> {
                if seed.is_empty() || seed.len() > MAX_REF_BYTES_V1 {
                    return Err(CoordinationErrorV1::InvalidIdentity);
                }
                Ok(Self(domain_hash(
                    $domain,
                    &CborValue::Text(seed.to_owned()),
                )?))
            }

            pub(crate) fn from_digest(digest: [u8; 32]) -> Result<Self, CoordinationErrorV1> {
                require_digest(digest)?;
                Ok(Self(digest))
            }

            pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

coordination_identity!(ThreadIdV1, "maestro.vnext.coordination-thread-id.v1");
coordination_identity!(MessageIdV1, "maestro.vnext.coordination-message-id.v1");
coordination_identity!(FocusIdV1, "maestro.vnext.coordination-focus-id.v1");
coordination_identity!(ScopeIdV1, "maestro.vnext.coordination-scope-id.v1");
coordination_identity!(ConflictIdV1, "maestro.vnext.coordination-conflict-id.v1");

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RepositoryInstallationRefV1(String);

impl RepositoryInstallationRefV1 {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, CoordinationErrorV1> {
        let value = value.into();
        require_ref(&value)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CoordinationAddressV1 {
    Principal(PrincipalIdV1),
    Work {
        repository_installation: RepositoryInstallationRefV1,
        work_ref: String,
    },
    Repository {
        repository_installation: RepositoryInstallationRefV1,
    },
}

impl CoordinationAddressV1 {
    pub(crate) fn work(
        repository_installation: RepositoryInstallationRefV1,
        work_ref: impl Into<String>,
    ) -> Result<Self, CoordinationErrorV1> {
        let work_ref = work_ref.into();
        require_ref(&work_ref)?;
        Ok(Self::Work {
            repository_installation,
            work_ref,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::Principal(principal) => {
                CborValue::Array(vec![CborValue::Unsigned(1), bytes(principal.as_bytes())])
            }
            Self::Work {
                repository_installation,
                work_ref,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                text(repository_installation.as_str()),
                text(work_ref),
            ]),
            Self::Repository {
                repository_installation,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                text(repository_installation.as_str()),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudienceEligibilitySnapshotV1 {
    id: [u8; 32],
    address: CoordinationAddressV1,
    eligible_principals: Vec<PrincipalIdV1>,
    semantic_hash: [u8; 32],
}

impl AudienceEligibilitySnapshotV1 {
    pub(crate) fn new(
        address: CoordinationAddressV1,
        mut eligible_principals: Vec<PrincipalIdV1>,
    ) -> Result<Self, CoordinationErrorV1> {
        eligible_principals.sort_unstable();
        if eligible_principals.is_empty()
            || eligible_principals.len() > MAX_AUDIENCE_V1
            || eligible_principals
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || matches!(
                &address,
                CoordinationAddressV1::Principal(principal)
                    if eligible_principals.as_slice() != [*principal]
            )
        {
            return Err(CoordinationErrorV1::InvalidAudienceEligibility);
        }
        let value = CborValue::Array(vec![
            address.canonical_value(),
            CborValue::Array(
                eligible_principals
                    .iter()
                    .map(|principal| bytes(principal.as_bytes()))
                    .collect(),
            ),
        ]);
        let semantic_hash = domain_hash(
            "maestro.vnext.coordination-audience-eligibility-snapshot.v1",
            &value,
        )?;
        let id = domain_hash(
            "maestro.vnext.coordination-audience-eligibility-snapshot-id.v1",
            &bytes(&semantic_hash),
        )?;
        Ok(Self {
            id,
            address,
            eligible_principals,
            semantic_hash,
        })
    }

    pub(crate) const fn id(&self) -> &[u8; 32] {
        &self.id
    }

    pub(crate) const fn semantic_hash(&self) -> [u8; 32] {
        self.semantic_hash
    }

    pub(crate) const fn address(&self) -> &CoordinationAddressV1 {
        &self.address
    }

    pub(crate) fn admits(&self, principal: PrincipalIdV1) -> bool {
        self.eligible_principals.binary_search(&principal).is_ok()
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(&self.id),
            self.address.canonical_value(),
            CborValue::Array(
                self.eligible_principals
                    .iter()
                    .map(|principal| bytes(principal.as_bytes()))
                    .collect(),
            ),
            bytes(&self.semantic_hash),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudienceMemberV1 {
    snapshot: AudienceEligibilitySnapshotV1,
}

impl AudienceMemberV1 {
    pub(crate) const fn new(snapshot: AudienceEligibilitySnapshotV1) -> Self {
        Self { snapshot }
    }

    pub(crate) const fn snapshot(&self) -> &AudienceEligibilitySnapshotV1 {
        &self.snapshot
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        self.snapshot.canonical_value()
    }
}

pub(crate) fn validate_audience(
    audience: &[AudienceMemberV1],
) -> Result<[u8; 32], CoordinationErrorV1> {
    if audience.is_empty()
        || audience.len() > MAX_AUDIENCE_V1
        || audience
            .windows(2)
            .any(|pair| pair[0].snapshot().address() >= pair[1].snapshot().address())
    {
        return Err(CoordinationErrorV1::InvalidAudience);
    }
    domain_hash(
        "maestro.vnext.coordination-audience.v1",
        &CborValue::Array(
            audience
                .iter()
                .map(AudienceMemberV1::canonical_value)
                .collect(),
        ),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StoreOrderV1 {
    pub(crate) commit_sequence: u64,
    pub(crate) transaction_ordinal: u32,
}

impl StoreOrderV1 {
    pub(crate) fn new(
        commit_sequence: u64,
        transaction_ordinal: u32,
    ) -> Result<Self, CoordinationErrorV1> {
        if commit_sequence == 0 {
            return Err(CoordinationErrorV1::InvalidStoreOrder);
        }
        Ok(Self {
            commit_sequence,
            transaction_ordinal,
        })
    }

    pub(crate) fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.commit_sequence),
            CborValue::Unsigned(self.transaction_ordinal as u64),
        ])
    }
}

impl Ord for StoreOrderV1 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.commit_sequence, self.transaction_ordinal)
            .cmp(&(other.commit_sequence, other.transaction_ordinal))
    }
}

impl PartialOrd for StoreOrderV1 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactMessageRefV1 {
    pub(crate) message_id: MessageIdV1,
    pub(crate) semantic_hash: [u8; 32],
}

impl ExactMessageRefV1 {
    pub(crate) fn new(
        message_id: MessageIdV1,
        semantic_hash: [u8; 32],
    ) -> Result<Self, CoordinationErrorV1> {
        require_digest(semantic_hash)?;
        Ok(Self {
            message_id,
            semantic_hash,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.message_id.as_bytes()),
            bytes(&self.semantic_hash),
        ])
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CoordinationSubjectRefV1 {
    Repository(String),
    Work(String),
    Contract(String),
    StepBinding(String),
    Evidence(String),
    Branch(String),
    Commit(String),
    Target(String),
    Destination(String),
    Concern(String),
}

impl CoordinationSubjectRefV1 {
    pub(crate) fn validate(&self) -> Result<(), CoordinationErrorV1> {
        require_ref(self.value())
    }

    fn value(&self) -> &str {
        match self {
            Self::Repository(value)
            | Self::Work(value)
            | Self::Contract(value)
            | Self::StepBinding(value)
            | Self::Evidence(value)
            | Self::Branch(value)
            | Self::Commit(value)
            | Self::Target(value)
            | Self::Destination(value)
            | Self::Concern(value) => value,
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let tag = match self {
            Self::Repository(_) => 1,
            Self::Work(_) => 2,
            Self::Contract(_) => 3,
            Self::StepBinding(_) => 4,
            Self::Evidence(_) => 5,
            Self::Branch(_) => 6,
            Self::Commit(_) => 7,
            Self::Target(_) => 8,
            Self::Destination(_) => 9,
            Self::Concern(_) => 10,
        };
        CborValue::Array(vec![CborValue::Unsigned(tag), text(self.value())])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConflictResolutionKindV1 {
    Reconciled,
    Withdrawn,
    Superseded,
    UnableToResolve,
}

impl ConflictResolutionKindV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Reconciled => 1,
            Self::Withdrawn => 2,
            Self::Superseded => 3,
            Self::UnableToResolve => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HandoffV1 {
    pub(crate) subject: CoordinationSubjectRefV1,
    pub(crate) contract_ref: Option<String>,
    pub(crate) step_binding_ref: Option<String>,
    pub(crate) branch_ref: Option<String>,
    pub(crate) base_ref: Option<String>,
    pub(crate) head_ref: Option<String>,
    pub(crate) target_ref: Option<String>,
    pub(crate) destination_ref: Option<String>,
    pub(crate) concern_ref: Option<String>,
    pub(crate) commit_ref: Option<String>,
    pub(crate) evidence_refs: Vec<String>,
}

impl HandoffV1 {
    pub(crate) fn validate(&self) -> Result<(), CoordinationErrorV1> {
        self.subject.validate()?;
        for value in [
            self.contract_ref.as_ref(),
            self.step_binding_ref.as_ref(),
            self.branch_ref.as_ref(),
            self.base_ref.as_ref(),
            self.head_ref.as_ref(),
            self.target_ref.as_ref(),
            self.destination_ref.as_ref(),
            self.concern_ref.as_ref(),
            self.commit_ref.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            require_ref(value)?;
        }
        if self.evidence_refs.len() > MAX_HANDOFF_EVIDENCE_REFS_V1
            || !strictly_ordered_unique(&self.evidence_refs)
        {
            return Err(CoordinationErrorV1::InvalidHandoff);
        }
        self.evidence_refs
            .iter()
            .try_for_each(|value| require_ref(value))
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.subject.canonical_value(),
            optional_text(self.contract_ref.as_deref()),
            optional_text(self.step_binding_ref.as_deref()),
            optional_text(self.branch_ref.as_deref()),
            optional_text(self.base_ref.as_deref()),
            optional_text(self.head_ref.as_deref()),
            optional_text(self.target_ref.as_deref()),
            optional_text(self.destination_ref.as_deref()),
            optional_text(self.concern_ref.as_deref()),
            optional_text(self.commit_ref.as_deref()),
            text_array(&self.evidence_refs),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoordinationMessageContentV1 {
    Information {
        body_ref: String,
    },
    Question {
        body_ref: String,
    },
    Proposal {
        body_ref: String,
    },
    Handoff(HandoffV1),
    ConflictAssert {
        conflict_id: ConflictIdV1,
        concern_ref: String,
        explanation_ref: String,
    },
    ConflictResolve {
        conflict_id: ConflictIdV1,
        assert_ref: ExactMessageRefV1,
        resolution: ConflictResolutionKindV1,
        explanation_ref: String,
        evidence_refs: Vec<String>,
    },
    Intervention {
        body_ref: String,
    },
}

impl CoordinationMessageContentV1 {
    pub(crate) fn validate(&self) -> Result<(), CoordinationErrorV1> {
        match self {
            Self::Information { body_ref }
            | Self::Question { body_ref }
            | Self::Proposal { body_ref }
            | Self::Intervention { body_ref } => require_body_ref(body_ref),
            Self::Handoff(handoff) => handoff.validate(),
            Self::ConflictAssert {
                concern_ref,
                explanation_ref,
                ..
            } => {
                require_ref(concern_ref)?;
                require_body_ref(explanation_ref)
            }
            Self::ConflictResolve {
                explanation_ref,
                evidence_refs,
                ..
            } => {
                require_body_ref(explanation_ref)?;
                if evidence_refs.len() > MAX_HANDOFF_EVIDENCE_REFS_V1
                    || !strictly_ordered_unique(evidence_refs)
                {
                    return Err(CoordinationErrorV1::InvalidMessageContent);
                }
                evidence_refs
                    .iter()
                    .try_for_each(|value| require_ref(value))
            }
        }
    }

    pub(crate) fn conflict_id(&self) -> Option<ConflictIdV1> {
        match self {
            Self::ConflictAssert { conflict_id, .. }
            | Self::ConflictResolve { conflict_id, .. } => Some(*conflict_id),
            _ => None,
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::Information { body_ref } => tagged_text(1, body_ref),
            Self::Question { body_ref } => tagged_text(2, body_ref),
            Self::Proposal { body_ref } => tagged_text(3, body_ref),
            Self::Handoff(handoff) => {
                CborValue::Array(vec![CborValue::Unsigned(4), handoff.canonical_value()])
            }
            Self::ConflictAssert {
                conflict_id,
                concern_ref,
                explanation_ref,
            } => CborValue::Array(vec![
                CborValue::Unsigned(5),
                bytes(conflict_id.as_bytes()),
                text(concern_ref),
                text(explanation_ref),
            ]),
            Self::ConflictResolve {
                conflict_id,
                assert_ref,
                resolution,
                explanation_ref,
                evidence_refs,
            } => CborValue::Array(vec![
                CborValue::Unsigned(6),
                bytes(conflict_id.as_bytes()),
                assert_ref.canonical_value(),
                CborValue::Unsigned(resolution.tag()),
                text(explanation_ref),
                text_array(evidence_refs),
            ]),
            Self::Intervention { body_ref } => tagged_text(7, body_ref),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum FocusSubjectV1 {
    Work(String),
    StepBinding(String),
}

impl FocusSubjectV1 {
    pub(crate) fn validate(&self) -> Result<(), CoordinationErrorV1> {
        match self {
            Self::Work(value) | Self::StepBinding(value) => require_ref(value),
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::Work(value) => tagged_text(1, value),
            Self::StepBinding(value) => tagged_text(2, value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TrustedIntervalV1 {
    pub(crate) valid_from: u64,
    pub(crate) valid_until: u64,
}

impl TrustedIntervalV1 {
    pub(crate) fn new(valid_from: u64, valid_until: u64) -> Result<Self, CoordinationErrorV1> {
        if valid_from >= valid_until {
            return Err(CoordinationErrorV1::InvalidInterval);
        }
        Ok(Self {
            valid_from,
            valid_until,
        })
    }

    pub(crate) const fn contains(self, as_of: u64) -> bool {
        self.valid_from <= as_of && as_of < self.valid_until
    }

    pub(crate) fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.valid_from),
            CborValue::Unsigned(self.valid_until),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ScopeExtentV1 {
    Exact,
    Subtree,
}

impl ScopeExtentV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Exact => 1,
            Self::Subtree => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct NormalizedScopePathV1(String);

impl NormalizedScopePathV1 {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, CoordinationErrorV1> {
        let value = value.into();
        let canonical = value == "/"
            || (value.starts_with('/')
                && !value.ends_with('/')
                && !value.contains("//")
                && value.split('/').skip(1).all(|component| {
                    !component.is_empty() && component != "." && component != ".."
                }));
        if !canonical || value.len() > MAX_REF_BYTES_V1 || value.contains('\0') {
            return Err(CoordinationErrorV1::InvalidScopePath);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn contains(&self, other: &Self) -> bool {
        self.0 == "/"
            || self.0 == other.0
            || other
                .0
                .strip_prefix(&self.0)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ScopeAtomV1 {
    pub(crate) checkout_ref: String,
    pub(crate) path: NormalizedScopePathV1,
    pub(crate) extent: ScopeExtentV1,
}

impl ScopeAtomV1 {
    pub(crate) fn new(
        checkout_ref: impl Into<String>,
        path: NormalizedScopePathV1,
        extent: ScopeExtentV1,
    ) -> Result<Self, CoordinationErrorV1> {
        let checkout_ref = checkout_ref.into();
        require_ref(&checkout_ref)?;
        Ok(Self {
            checkout_ref,
            path,
            extent,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.checkout_ref),
            text(self.path.as_str()),
            CborValue::Unsigned(self.extent.tag()),
        ])
    }

    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        if self.checkout_ref != other.checkout_ref {
            return false;
        }
        match (self.extent, other.extent) {
            (ScopeExtentV1::Exact, ScopeExtentV1::Exact) => self.path == other.path,
            (ScopeExtentV1::Subtree, ScopeExtentV1::Exact) => self.path.contains(&other.path),
            (ScopeExtentV1::Exact, ScopeExtentV1::Subtree) => other.path.contains(&self.path),
            (ScopeExtentV1::Subtree, ScopeExtentV1::Subtree) => {
                self.path.contains(&other.path) || other.path.contains(&self.path)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WithdrawalReasonV1 {
    Replaced,
    Explicit,
    SecurityErasure,
}

impl WithdrawalReasonV1 {
    pub(crate) const fn tag(self) -> u64 {
        match self {
            Self::Replaced => 1,
            Self::Explicit => 2,
            Self::SecurityErasure => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeliverySubjectV1 {
    schema_ref: String,
    subject_ref: String,
    semantic_hash: [u8; 32],
}

impl DeliverySubjectV1 {
    pub(crate) fn from_frozen_ref(
        schema_ref: impl Into<String>,
        subject_ref: impl Into<String>,
        semantic_hash: [u8; 32],
    ) -> Result<Self, CoordinationErrorV1> {
        let schema_ref = schema_ref.into();
        let subject_ref = subject_ref.into();
        require_ref(&schema_ref)?;
        require_ref(&subject_ref)?;
        require_digest(semantic_hash)?;
        Ok(Self {
            schema_ref,
            subject_ref,
            semantic_hash,
        })
    }

    pub(crate) fn semantic_hash(&self) -> [u8; 32] {
        self.semantic_hash
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.schema_ref),
            text(&self.subject_ref),
            bytes(&self.semantic_hash),
        ])
    }
}

pub(crate) fn actor_value(principal: PrincipalIdV1, session: SessionIdV1) -> CborValue {
    CborValue::Array(vec![bytes(principal.as_bytes()), bytes(session.as_bytes())])
}

pub(crate) fn hash_value(domain: &str, value: &CborValue) -> Result<[u8; 32], CoordinationErrorV1> {
    domain_hash(domain, value)
}

pub(crate) fn bytes(value: &[u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

pub(crate) fn text(value: &str) -> CborValue {
    CborValue::Text(value.to_owned())
}

pub(crate) fn text_array(values: &[String]) -> CborValue {
    CborValue::Array(values.iter().map(|value| text(value)).collect())
}

pub(crate) fn optional_text(value: Option<&str>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), text(value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

pub(crate) fn optional_message_ref(value: Option<&ExactMessageRefV1>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), value.canonical_value()]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

fn tagged_text(tag: u64, value: &str) -> CborValue {
    CborValue::Array(vec![CborValue::Unsigned(tag), text(value)])
}

fn require_ref(value: &str) -> Result<(), CoordinationErrorV1> {
    if value.is_empty()
        || value.len() > MAX_REF_BYTES_V1
        || value.contains('\0')
        || value.trim() != value
    {
        return Err(CoordinationErrorV1::InvalidReference);
    }
    Ok(())
}

fn require_body_ref(value: &str) -> Result<(), CoordinationErrorV1> {
    if value.is_empty()
        || value.len() > MAX_MESSAGE_BODY_REF_BYTES_V1
        || value.contains('\0')
        || value.trim() != value
    {
        return Err(CoordinationErrorV1::InvalidMessageContent);
    }
    Ok(())
}

fn require_digest(value: [u8; 32]) -> Result<(), CoordinationErrorV1> {
    if value == [0; 32] {
        return Err(CoordinationErrorV1::InvalidDigest);
    }
    Ok(())
}

fn strictly_ordered_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn domain_hash(domain: &str, value: &CborValue) -> Result<[u8; 32], CoordinationErrorV1> {
    let envelope = CborValue::Array(vec![text(domain), value.clone()]);
    Ok(Sha256::digest(deterministic_cbor::encode(&envelope)?).into())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum CoordinationErrorV1 {
    #[error("Coordination identity is empty, oversized, or zero")]
    InvalidIdentity,
    #[error("Coordination references must be nonempty, bounded, trimmed, and NUL-free")]
    InvalidReference,
    #[error("Coordination semantic digests must be nonzero")]
    InvalidDigest,
    #[error("Thread audience must be nonempty, bounded, canonically ordered, and unique")]
    InvalidAudience,
    #[error("Audience eligibility must be nonempty, canonical, and exact for PrincipalAddress")]
    InvalidAudienceEligibility,
    #[error("Store order requires a positive commit sequence")]
    InvalidStoreOrder,
    #[error("Message content is outside the closed v1 grammar")]
    InvalidMessageContent,
    #[error("Handoff references are malformed, duplicated, or out of order")]
    InvalidHandoff,
    #[error("trusted validity must be a nonempty half-open interval")]
    InvalidInterval,
    #[error("Scope path is not an absolute normalized v1 path")]
    InvalidScopePath,
    #[error("too many typed subject references")]
    TooManySubjectReferences,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
