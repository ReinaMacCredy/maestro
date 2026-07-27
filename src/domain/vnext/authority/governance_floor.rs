use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::facade::{
    StoreGenerationV1, StoreHeadV1, StoreObjectError, StoreObjectV1, StorePublicationViewV1,
    StoreRoleV1,
};
use crate::domain::vnext::identity::{
    IdentityError, SchemaIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub(super) const REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_TAG_V1: usize = 25;
pub(super) const REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_DOMAIN_V1: &str =
    "maestro.vnext.repository-governance-floor-snapshot.v1";
const AUTHORITY_SCHEMA_DESCRIPTOR_DOMAIN_V1: &str =
    "maestro.vnext.stage2.authority.schema-descriptor.v1";
const REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_NAME_V1: &str = "RepositoryGovernanceFloorSnapshotV1";
const MAX_GOVERNANCE_REQUIREMENTS: usize = 256;
const MAX_REQUIREMENT_PARTICIPANTS: usize = 16;
const ACTION_PUBLISH_SCHEDULING_POLICY_BINDING_V1: u64 = 105;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes Action-68 rotation before its Authority workflow integrates"
    )
)]
const ACTION_ROTATE_REPOSITORY_GOVERNANCE_FLOOR_V1: u64 = 68;
const PLANNING_ACTION_OWNER_TAG_V1: u64 = 12;
const ACTION_105_PARTICIPANTS_V1: [u64; 3] = [1, 7, 14];
const AUTHORITY_SCHEDULING_SAFETY_MAXIMUMS_V1: [u64; 4] = [1_000; 4];
const AUTHORITY_SCHEDULING_SAFETY_FLOOR_VERSION_V1: u64 = 1;
const AUTHORITY_SCHEDULING_CLASSIFIER_REVISION_V1: u64 = 1;
const AUTHORITY_SCHEDULING_EVALUATOR_REVISION_V1: u64 = 1;

const SCHEMA_FIELDS_V1: [&str; 19] = [
    "repository_store_domain",
    "authority_context",
    "floor_revision",
    "predecessor",
    "activation_basis",
    "activation_generation_ordinal",
    "authority_epoch",
    "trust_root_revision",
    "trust_root_binding_commitment",
    "authority_transition_protocol_identity",
    "authority_transition_protocol_version",
    "minimum_assurance",
    "requirement_grammar_identity",
    "requirement_evaluator_identity",
    "requirement_evaluator_revision",
    "requirement_rows",
    "semantic_hash",
    "canonicalization_version",
    "protocol_version",
];

const SCHEMA_VARIANTS_V1: [&str; 3] = [
    "repository_genesis",
    "explicit_legacy_migration",
    "guarded_rotation",
];

const SCHEMA_INVARIANTS_V1: [&str; 12] = [
    "append_only_authority_schema_tag_25",
    "internal_non_public_schema",
    "exactly_one_direct_floor_root",
    "repository_governance_head_class_8_exact_closure",
    "gap_free_predecessor_history",
    "same_repository_and_authority_context",
    "action_105_requirement_exactly_once",
    "planning_authority_persistence_participants",
    "semantic_hash_recomputed",
    "old_writer_preserves_unknown_root",
    "restore_requires_exact_same_domain_chain",
    "immutable_non_authorizing_record",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub(super) enum RepositoryGovernanceFloorActivationBasisV1 {
    RepositoryGenesis = 1,
    ExplicitLegacyMigration = 2,
    GuardedRotation = 3,
}

impl TryFrom<u64> for RepositoryGovernanceFloorActivationBasisV1 {
    type Error = RepositoryGovernanceFloorErrorV1;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::RepositoryGenesis),
            2 => Ok(Self::ExplicitLegacyMigration),
            3 => Ok(Self::GuardedRotation),
            _ => Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RepositoryGovernanceRequirementExprV1 {
    All(Vec<Self>),
    ExactOwner(u64),
    ExactParticipants(Vec<u64>),
    MinimumAssurance(u64),
    CurrentAuthority,
    Unrevoked,
}

impl RepositoryGovernanceRequirementExprV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::All(rows) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Array(rows.iter().map(Self::canonical_value).collect()),
            ]),
            Self::ExactOwner(owner) => {
                CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(*owner)])
            }
            Self::ExactParticipants(participants) => CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Array(
                    participants
                        .iter()
                        .copied()
                        .map(CborValue::Unsigned)
                        .collect(),
                ),
            ]),
            Self::MinimumAssurance(assurance) => CborValue::Array(vec![
                CborValue::Unsigned(4),
                CborValue::Unsigned(*assurance),
            ]),
            Self::CurrentAuthority => CborValue::Array(vec![CborValue::Unsigned(5)]),
            Self::Unrevoked => CborValue::Array(vec![CborValue::Unsigned(6)]),
        }
    }

    fn decode(value: &CborValue, depth: usize) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        if depth > 16 {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        }
        let CborValue::Array(fields) = value else {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        };
        match fields.as_slice() {
            [CborValue::Unsigned(1), CborValue::Array(rows)]
                if !rows.is_empty() && rows.len() <= 16 =>
            {
                Ok(Self::All(
                    rows.iter()
                        .map(|row| Self::decode(row, depth + 1))
                        .collect::<Result<_, _>>()?,
                ))
            }
            [CborValue::Unsigned(2), CborValue::Unsigned(owner)] if *owner > 0 => {
                Ok(Self::ExactOwner(*owner))
            }
            [CborValue::Unsigned(3), CborValue::Array(participants)] => {
                let values = unsigned_rows(participants)?;
                validate_participants(&values)?;
                Ok(Self::ExactParticipants(values))
            }
            [CborValue::Unsigned(4), CborValue::Unsigned(assurance)] if *assurance > 0 => {
                Ok(Self::MinimumAssurance(*assurance))
            }
            [CborValue::Unsigned(5)] => Ok(Self::CurrentAuthority),
            [CborValue::Unsigned(6)] => Ok(Self::Unrevoked),
            _ => Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositoryGovernanceRequirementRowV1 {
    action_tag: u64,
    owner_tag: u64,
    participants: Vec<u64>,
    requirement_version: u64,
    requirement: RepositoryGovernanceRequirementExprV1,
}

impl RepositoryGovernanceRequirementRowV1 {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Stage 5 freezes the exact Action-105 requirement constructor for governance genesis and migration"
        )
    )]
    pub(super) fn action_105(minimum_assurance: u64) -> Self {
        Self {
            action_tag: ACTION_PUBLISH_SCHEDULING_POLICY_BINDING_V1,
            owner_tag: PLANNING_ACTION_OWNER_TAG_V1,
            participants: ACTION_105_PARTICIPANTS_V1.to_vec(),
            requirement_version: 1,
            requirement: RepositoryGovernanceRequirementExprV1::All(vec![
                RepositoryGovernanceRequirementExprV1::ExactOwner(PLANNING_ACTION_OWNER_TAG_V1),
                RepositoryGovernanceRequirementExprV1::ExactParticipants(
                    ACTION_105_PARTICIPANTS_V1.to_vec(),
                ),
                RepositoryGovernanceRequirementExprV1::MinimumAssurance(minimum_assurance),
                RepositoryGovernanceRequirementExprV1::CurrentAuthority,
                RepositoryGovernanceRequirementExprV1::Unrevoked,
            ]),
        }
    }

    fn validate(&self) -> Result<(), RepositoryGovernanceFloorErrorV1> {
        if self.action_tag == 0
            || self.owner_tag == 0
            || self.requirement_version == 0
            || self.participants.is_empty()
        {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        }
        validate_participants(&self.participants)?;
        RepositoryGovernanceRequirementExprV1::decode(&self.requirement.canonical_value(), 0)?;
        Ok(())
    }

    pub(super) fn minimum_assurance(&self) -> u64 {
        fn visit(requirement: &RepositoryGovernanceRequirementExprV1) -> Option<u64> {
            match requirement {
                RepositoryGovernanceRequirementExprV1::MinimumAssurance(value) => Some(*value),
                RepositoryGovernanceRequirementExprV1::All(rows) => {
                    rows.iter().filter_map(visit).max()
                }
                _ => None,
            }
        }
        visit(&self.requirement).unwrap_or(0)
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.action_tag),
            CborValue::Unsigned(self.owner_tag),
            CborValue::Array(
                self.participants
                    .iter()
                    .copied()
                    .map(CborValue::Unsigned)
                    .collect(),
            ),
            CborValue::Unsigned(self.requirement_version),
            self.requirement.canonical_value(),
        ])
    }

    fn decode(value: &CborValue) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        };
        let [
            CborValue::Unsigned(action_tag),
            CborValue::Unsigned(owner_tag),
            CborValue::Array(participants),
            CborValue::Unsigned(requirement_version),
            requirement,
        ] = fields.as_slice()
        else {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        };
        let row = Self {
            action_tag: *action_tag,
            owner_tag: *owner_tag,
            participants: unsigned_rows(participants)?,
            requirement_version: *requirement_version,
            requirement: RepositoryGovernanceRequirementExprV1::decode(requirement, 0)?,
        };
        row.validate()?;
        Ok(row)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositoryGovernanceFloorSnapshotV1 {
    repository_store_domain: [u8; 32],
    authority_context: [u8; 32],
    floor_revision: u64,
    predecessor: Option<StoreObjectIdV1>,
    activation_basis: RepositoryGovernanceFloorActivationBasisV1,
    activation_generation_ordinal: u64,
    authority_epoch: u64,
    trust_root_revision: u64,
    trust_root_binding_commitment: [u8; 32],
    authority_transition_protocol_identity: [u8; 32],
    authority_transition_protocol_version: u64,
    minimum_assurance: u64,
    requirement_grammar_identity: [u8; 32],
    requirement_evaluator_identity: [u8; 32],
    requirement_evaluator_revision: u64,
    requirement_rows: Vec<RepositoryGovernanceRequirementRowV1>,
    semantic_hash: [u8; 32],
    canonicalization_version: u64,
    protocol_version: u64,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes governance genesis and migration input before their Authority workflows integrate"
    )
)]
pub(super) struct RepositoryGovernanceFloorGenesisInputV1 {
    pub(super) repository_store_domain: [u8; 32],
    pub(super) authority_context: [u8; 32],
    pub(super) activation_generation_ordinal: u64,
    pub(super) authority_epoch: u64,
    pub(super) trust_root_revision: u64,
    pub(super) trust_root_binding_commitment: [u8; 32],
    pub(super) authority_transition_protocol_identity: [u8; 32],
    pub(super) minimum_assurance: u64,
    pub(super) requirement_grammar_identity: [u8; 32],
    pub(super) requirement_evaluator_identity: [u8; 32],
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the exact Action-68 rotation input before its Authority workflow integrates"
    )
)]
pub(super) struct RepositoryGovernanceFloorRotationInputV1 {
    pub(super) predecessor_id: StoreObjectIdV1,
    pub(super) activation_generation_ordinal: u64,
    pub(super) authority_epoch: u64,
    pub(super) trust_root_revision: u64,
    pub(super) trust_root_binding_commitment: [u8; 32],
    pub(super) minimum_assurance: u64,
    pub(super) requirement_rows: Vec<RepositoryGovernanceRequirementRowV1>,
    pub(super) admitted_action_tag: u64,
}

// TODO(Authority Stage 7/8): Remove this expectation when the first production
// governance genesis, migration, or Action-68 rotation caller integrates.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the owner-private governance publication constructors before their Authority workflow consumers integrate"
    )
)]
impl RepositoryGovernanceFloorSnapshotV1 {
    pub(super) fn repository_genesis(
        input: RepositoryGovernanceFloorGenesisInputV1,
    ) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        Self::initial(
            input,
            RepositoryGovernanceFloorActivationBasisV1::RepositoryGenesis,
        )
    }

    pub(super) fn explicit_legacy_migration(
        input: RepositoryGovernanceFloorGenesisInputV1,
    ) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        Self::initial(
            input,
            RepositoryGovernanceFloorActivationBasisV1::ExplicitLegacyMigration,
        )
    }

    fn initial(
        input: RepositoryGovernanceFloorGenesisInputV1,
        activation_basis: RepositoryGovernanceFloorActivationBasisV1,
    ) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        let mut snapshot = Self {
            repository_store_domain: input.repository_store_domain,
            authority_context: input.authority_context,
            floor_revision: 1,
            predecessor: None,
            activation_basis,
            activation_generation_ordinal: input.activation_generation_ordinal,
            authority_epoch: input.authority_epoch,
            trust_root_revision: input.trust_root_revision,
            trust_root_binding_commitment: input.trust_root_binding_commitment,
            authority_transition_protocol_identity: input.authority_transition_protocol_identity,
            authority_transition_protocol_version: 1,
            minimum_assurance: input.minimum_assurance,
            requirement_grammar_identity: input.requirement_grammar_identity,
            requirement_evaluator_identity: input.requirement_evaluator_identity,
            requirement_evaluator_revision: 1,
            requirement_rows: vec![RepositoryGovernanceRequirementRowV1::action_105(
                input.minimum_assurance,
            )],
            semantic_hash: [0; 32],
            canonicalization_version: 1,
            protocol_version: 1,
        };
        snapshot.semantic_hash = snapshot.recompute_semantic_hash()?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub(super) fn guarded_rotation(
        predecessor: &Self,
        input: RepositoryGovernanceFloorRotationInputV1,
    ) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        if input.admitted_action_tag != ACTION_ROTATE_REPOSITORY_GOVERNANCE_FLOOR_V1 {
            return Err(RepositoryGovernanceFloorErrorV1::RotationNotAdmitted);
        }
        let mut snapshot = Self {
            repository_store_domain: predecessor.repository_store_domain,
            authority_context: predecessor.authority_context,
            floor_revision: predecessor
                .floor_revision
                .checked_add(1)
                .ok_or(RepositoryGovernanceFloorErrorV1::InvalidSnapshot)?,
            predecessor: Some(input.predecessor_id),
            activation_basis: RepositoryGovernanceFloorActivationBasisV1::GuardedRotation,
            activation_generation_ordinal: input.activation_generation_ordinal,
            authority_epoch: input.authority_epoch,
            trust_root_revision: input.trust_root_revision,
            trust_root_binding_commitment: input.trust_root_binding_commitment,
            authority_transition_protocol_identity: predecessor
                .authority_transition_protocol_identity,
            authority_transition_protocol_version: predecessor
                .authority_transition_protocol_version,
            minimum_assurance: input.minimum_assurance,
            requirement_grammar_identity: predecessor.requirement_grammar_identity,
            requirement_evaluator_identity: predecessor.requirement_evaluator_identity,
            requirement_evaluator_revision: predecessor.requirement_evaluator_revision,
            requirement_rows: input.requirement_rows,
            semantic_hash: [0; 32],
            canonicalization_version: predecessor.canonicalization_version,
            protocol_version: predecessor.protocol_version,
        };
        snapshot.semantic_hash = snapshot.recompute_semantic_hash()?;
        snapshot.validate()?;
        if snapshot.activation_generation_ordinal <= predecessor.activation_generation_ordinal {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidLineage);
        }
        Ok(snapshot)
    }

    pub(super) fn to_store_object(
        &self,
    ) -> Result<StoreObjectV1, RepositoryGovernanceFloorErrorV1> {
        self.validate()?;
        StoreObjectV1::new(
            repository_governance_floor_schema_id()?,
            self.canonical_value()?,
            self.predecessor.into_iter().collect(),
        )
        .map_err(Into::into)
    }

    pub(super) fn decode_object(
        object: &StoreObjectV1,
    ) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        if object.schema_id() != repository_governance_floor_schema_id()? {
            return Err(RepositoryGovernanceFloorErrorV1::WrongSchema);
        }
        let snapshot = Self::decode_value(object.value())?;
        let expected_references = snapshot.predecessor.into_iter().collect::<Vec<_>>();
        if object.references() != expected_references {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidLineage);
        }
        Ok(snapshot)
    }

    pub(super) const fn revision(&self) -> u64 {
        self.floor_revision
    }

    pub(super) const fn semantic_hash(&self) -> [u8; 32] {
        self.semantic_hash
    }

    pub(super) const fn authority_context(&self) -> [u8; 32] {
        self.authority_context
    }

    pub(super) const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub(super) fn action_105_requirement(
        &self,
    ) -> Result<&RepositoryGovernanceRequirementRowV1, RepositoryGovernanceFloorErrorV1> {
        let mut matches = self
            .requirement_rows
            .iter()
            .filter(|row| row.action_tag == ACTION_PUBLISH_SCHEDULING_POLICY_BINDING_V1);
        let Some(row) = matches.next() else {
            return Err(RepositoryGovernanceFloorErrorV1::MissingAction105);
        };
        if matches.next().is_some()
            || row.owner_tag != PLANNING_ACTION_OWNER_TAG_V1
            || row.participants != ACTION_105_PARTICIPANTS_V1
        {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidAction105);
        }
        Ok(row)
    }

    fn validate(&self) -> Result<(), RepositoryGovernanceFloorErrorV1> {
        if [
            self.repository_store_domain,
            self.authority_context,
            self.trust_root_binding_commitment,
            self.authority_transition_protocol_identity,
            self.requirement_grammar_identity,
            self.requirement_evaluator_identity,
            self.semantic_hash,
        ]
        .contains(&[0; 32])
            || self.floor_revision == 0
            || self.activation_generation_ordinal == 0
            || self.authority_epoch == 0
            || self.trust_root_revision == 0
            || self.authority_transition_protocol_version == 0
            || self.minimum_assurance == 0
            || self.requirement_evaluator_revision == 0
            || self.canonicalization_version != 1
            || self.protocol_version != 1
            || self.requirement_rows.is_empty()
            || self.requirement_rows.len() > MAX_GOVERNANCE_REQUIREMENTS
            || (self.floor_revision == 1) != self.predecessor.is_none()
            || (self.floor_revision == 1
                && self.activation_basis
                    == RepositoryGovernanceFloorActivationBasisV1::GuardedRotation)
            || (self.floor_revision > 1
                && self.activation_basis
                    != RepositoryGovernanceFloorActivationBasisV1::GuardedRotation)
        {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        }
        let mut action_tags = BTreeSet::new();
        for row in &self.requirement_rows {
            row.validate()?;
            if !action_tags.insert(row.action_tag) {
                return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
            }
        }
        self.action_105_requirement()?;
        if self.semantic_hash != self.recompute_semantic_hash()? {
            return Err(RepositoryGovernanceFloorErrorV1::SemanticHashMismatch);
        }
        Ok(())
    }

    fn recompute_semantic_hash(&self) -> Result<[u8; 32], RepositoryGovernanceFloorErrorV1> {
        hash_value(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-governance-floor-semantic.v1")?,
            bytes(&self.authority_transition_protocol_identity),
            CborValue::Unsigned(self.authority_transition_protocol_version),
            CborValue::Unsigned(self.minimum_assurance),
            bytes(&self.requirement_grammar_identity),
            bytes(&self.requirement_evaluator_identity),
            CborValue::Unsigned(self.requirement_evaluator_revision),
            CborValue::Array(
                self.requirement_rows
                    .iter()
                    .map(RepositoryGovernanceRequirementRowV1::canonical_value)
                    .collect(),
            ),
        ]))
    }

    fn canonical_value(&self) -> Result<CborValue, RepositoryGovernanceFloorErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text(REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_DOMAIN_V1)?,
            bytes(&self.repository_store_domain),
            bytes(&self.authority_context),
            CborValue::Unsigned(self.floor_revision),
            CborValue::optional(self.predecessor.map(|identity| bytes(identity.as_bytes()))),
            CborValue::Unsigned(self.activation_basis as u64),
            CborValue::Unsigned(self.activation_generation_ordinal),
            CborValue::Unsigned(self.authority_epoch),
            CborValue::Unsigned(self.trust_root_revision),
            bytes(&self.trust_root_binding_commitment),
            bytes(&self.authority_transition_protocol_identity),
            CborValue::Unsigned(self.authority_transition_protocol_version),
            CborValue::Unsigned(self.minimum_assurance),
            bytes(&self.requirement_grammar_identity),
            bytes(&self.requirement_evaluator_identity),
            CborValue::Unsigned(self.requirement_evaluator_revision),
            CborValue::Array(
                self.requirement_rows
                    .iter()
                    .map(RepositoryGovernanceRequirementRowV1::canonical_value)
                    .collect(),
            ),
            bytes(&self.semantic_hash),
            CborValue::Unsigned(self.canonicalization_version),
            CborValue::Unsigned(self.protocol_version),
        ]))
    }

    fn decode_value(value: &CborValue) -> Result<Self, RepositoryGovernanceFloorErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        };
        let [
            CborValue::Text(domain),
            CborValue::Bytes(repository_store_domain),
            CborValue::Bytes(authority_context),
            CborValue::Unsigned(floor_revision),
            predecessor,
            CborValue::Unsigned(activation_basis),
            CborValue::Unsigned(activation_generation_ordinal),
            CborValue::Unsigned(authority_epoch),
            CborValue::Unsigned(trust_root_revision),
            CborValue::Bytes(trust_root_binding_commitment),
            CborValue::Bytes(authority_transition_protocol_identity),
            CborValue::Unsigned(authority_transition_protocol_version),
            CborValue::Unsigned(minimum_assurance),
            CborValue::Bytes(requirement_grammar_identity),
            CborValue::Bytes(requirement_evaluator_identity),
            CborValue::Unsigned(requirement_evaluator_revision),
            CborValue::Array(requirement_rows),
            CborValue::Bytes(semantic_hash),
            CborValue::Unsigned(canonicalization_version),
            CborValue::Unsigned(protocol_version),
        ] = fields.as_slice()
        else {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        };
        if domain != REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_DOMAIN_V1 {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
        }
        let snapshot = Self {
            repository_store_domain: exact_digest(repository_store_domain)?,
            authority_context: exact_digest(authority_context)?,
            floor_revision: *floor_revision,
            predecessor: optional_object_id(predecessor)?,
            activation_basis: (*activation_basis).try_into()?,
            activation_generation_ordinal: *activation_generation_ordinal,
            authority_epoch: *authority_epoch,
            trust_root_revision: *trust_root_revision,
            trust_root_binding_commitment: exact_digest(trust_root_binding_commitment)?,
            authority_transition_protocol_identity: exact_digest(
                authority_transition_protocol_identity,
            )?,
            authority_transition_protocol_version: *authority_transition_protocol_version,
            minimum_assurance: *minimum_assurance,
            requirement_grammar_identity: exact_digest(requirement_grammar_identity)?,
            requirement_evaluator_identity: exact_digest(requirement_evaluator_identity)?,
            requirement_evaluator_revision: *requirement_evaluator_revision,
            requirement_rows: requirement_rows
                .iter()
                .map(RepositoryGovernanceRequirementRowV1::decode)
                .collect::<Result<_, _>>()?,
            semantic_hash: exact_digest(semantic_hash)?,
            canonicalization_version: *canonicalization_version,
            protocol_version: *protocol_version,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

#[derive(Clone, Copy)]
pub(super) struct RepositoryGovernanceAuthorityCurrentnessV1 {
    pub(super) authority_context: [u8; 32],
    pub(super) authority_epoch: u64,
    pub(super) trust_root_revision: u64,
    pub(super) trust_root_binding_commitment: [u8; 32],
    pub(super) authority_state_token: [u8; 32],
    pub(super) authority_fence: [u8; 32],
    pub(super) revocation_revision: u64,
    pub(super) principal: [u8; 32],
    pub(super) binding: [u8; 32],
    pub(super) session: [u8; 32],
    pub(super) assurance_revision: u64,
    pub(super) trusted_time: u64,
}

#[derive(Clone, Copy)]
struct AuthoritySchedulingSafetyStateV1 {
    floor: [u64; 4],
    floor_identity: [u8; 32],
    floor_version: u64,
    floor_semantic_hash: [u8; 32],
    classifier_identity: [u8; 32],
    classifier_revision: u64,
    classifier_semantic_hash: [u8; 32],
    evaluator_identity: [u8; 32],
    evaluator_revision: u64,
    compatibility: [u8; 32],
    currentness: [u8; 32],
}

pub(super) struct RepositoryGovernanceFloorCurrentViewV1<'tx> {
    _view: &'tx StorePublicationViewV1<'tx>,
    snapshot: RepositoryGovernanceFloorSnapshotV1,
    direct_root: StoreObjectIdV1,
    history: Vec<StoreObjectIdV1>,
    head: StoreHeadIdV1,
    head_revision: u64,
    generation: StoreGenerationIdV1,
    generation_ordinal: u64,
    class_root: [u8; 32],
    authority_state_token: [u8; 32],
    authority_fence: [u8; 32],
    revocation_revision: u64,
    principal: [u8; 32],
    binding: [u8; 32],
    session: [u8; 32],
    assurance_revision: u64,
    trusted_time: u64,
    scheduling_safety: AuthoritySchedulingSafetyStateV1,
    commitment: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RepositoryGovernanceFloorCurrentViewV1<'_> {
    pub(super) fn retained_tuple_is_current(&self) -> bool {
        self._view.role() == StoreRoleV1::Repository
            && *self._view.domain().id().as_bytes() == self.snapshot.repository_store_domain
            && !self.history.is_empty()
            && self.history.last() == Some(&self.direct_root)
            && self.head.as_bytes() != &[0; 32]
            && self.head_revision > 0
            && self.generation.as_bytes() != &[0; 32]
            && self.generation_ordinal >= self.snapshot.activation_generation_ordinal
            && ![
                self.class_root,
                self.authority_state_token,
                self.authority_fence,
                self.principal,
                self.binding,
                self.session,
                self.scheduling_safety.floor_identity,
                self.scheduling_safety.floor_semantic_hash,
                self.scheduling_safety.classifier_identity,
                self.scheduling_safety.classifier_semantic_hash,
                self.scheduling_safety.evaluator_identity,
                self.scheduling_safety.compatibility,
                self.scheduling_safety.currentness,
                self.commitment,
            ]
            .contains(&[0; 32])
            && self.revocation_revision > 0
            && self.assurance_revision > 0
            && self.trusted_time > 0
            && self.scheduling_safety.floor != [0; 4]
            && self.scheduling_safety.floor_version > 0
            && self.scheduling_safety.classifier_revision > 0
            && self.scheduling_safety.evaluator_revision > 0
    }

    pub(super) const fn snapshot(&self) -> &RepositoryGovernanceFloorSnapshotV1 {
        &self.snapshot
    }

    pub(super) const fn direct_root(&self) -> StoreObjectIdV1 {
        self.direct_root
    }

    pub(super) const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    pub(super) const fn principal(&self) -> [u8; 32] {
        self.principal
    }

    pub(super) const fn binding(&self) -> [u8; 32] {
        self.binding
    }

    pub(super) const fn session(&self) -> [u8; 32] {
        self.session
    }

    pub(super) const fn assurance_revision(&self) -> u64 {
        self.assurance_revision
    }

    pub(super) const fn trusted_time(&self) -> u64 {
        self.trusted_time
    }

    pub(super) const fn scheduling_safety_floor(&self) -> [u64; 4] {
        self.scheduling_safety.floor
    }

    pub(super) const fn scheduling_classifier_revision(&self) -> u64 {
        self.scheduling_safety.classifier_revision
    }

    pub(super) const fn scheduling_evaluator_revision(&self) -> u64 {
        self.scheduling_safety.evaluator_revision
    }

    pub(super) fn preserved_by_roots(&self, roots: &[StoreObjectIdV1]) -> bool {
        roots
            .iter()
            .filter(|root| **root == self.direct_root)
            .count()
            == 1
    }
}

pub(super) fn resolve_repository_governance_floor_current_view<'tx>(
    view: &'tx StorePublicationViewV1<'tx>,
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    authority: RepositoryGovernanceAuthorityCurrentnessV1,
) -> Result<RepositoryGovernanceFloorCurrentViewV1<'tx>, RepositoryGovernanceFloorErrorV1> {
    if view.role() != StoreRoleV1::Repository
        || head.generation_id() != generation.id()
        || head.generation_ordinal() != generation.ordinal()
        || generation.domain() != view.domain()
    {
        return Err(RepositoryGovernanceFloorErrorV1::CurrentnessMismatch);
    }
    let schema_id = repository_governance_floor_schema_id()?;
    let by_id = active_objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<BTreeMap<_, _>>();
    let roots = generation
        .roots()
        .iter()
        .filter_map(|root| {
            by_id
                .get(root)
                .filter(|object| object.schema_id() == schema_id)
                .map(|object| object.id())
        })
        .collect::<Vec<_>>();
    let [direct_root] = roots.as_slice() else {
        return Err(RepositoryGovernanceFloorErrorV1::InvalidDirectRoot);
    };
    let direct_object = by_id
        .get(direct_root)
        .ok_or(RepositoryGovernanceFloorErrorV1::InvalidDirectRoot)?;
    let snapshot = RepositoryGovernanceFloorSnapshotV1::decode_object(direct_object)?;
    if snapshot.repository_store_domain != *view.domain().id().as_bytes()
        || snapshot.authority_context != authority.authority_context
        || snapshot.authority_epoch != authority.authority_epoch
        || snapshot.trust_root_revision != authority.trust_root_revision
        || snapshot.trust_root_binding_commitment != authority.trust_root_binding_commitment
        || snapshot.activation_generation_ordinal > generation.ordinal()
        || [
            authority.authority_state_token,
            authority.authority_fence,
            authority.principal,
            authority.binding,
            authority.session,
        ]
        .contains(&[0; 32])
        || authority.revocation_revision == 0
        || authority.assurance_revision == 0
        || authority.trusted_time == 0
    {
        return Err(RepositoryGovernanceFloorErrorV1::CurrentnessMismatch);
    }
    let history = validate_history(*direct_root, &by_id)?;
    let class_root = hash_value(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-governance-head-class-8.v1")?,
        CborValue::Array(
            history
                .iter()
                .map(|identity| bytes(identity.as_bytes()))
                .collect(),
        ),
    ]))?;
    let scheduling_safety = resolve_authority_scheduling_safety_state(&snapshot, authority)?;
    let commitment = current_view_commitment(
        view,
        head,
        generation,
        &snapshot,
        *direct_root,
        class_root,
        authority,
        scheduling_safety,
    )?;
    Ok(RepositoryGovernanceFloorCurrentViewV1 {
        _view: view,
        snapshot,
        direct_root: *direct_root,
        history,
        head: head.id(),
        head_revision: head.revision(),
        generation: generation.id(),
        generation_ordinal: generation.ordinal(),
        class_root,
        authority_state_token: authority.authority_state_token,
        authority_fence: authority.authority_fence,
        revocation_revision: authority.revocation_revision,
        principal: authority.principal,
        binding: authority.binding,
        session: authority.session,
        assurance_revision: authority.assurance_revision,
        trusted_time: authority.trusted_time,
        scheduling_safety,
        commitment,
        _not_send_or_sync: PhantomData,
    })
}

fn validate_history(
    direct_root: StoreObjectIdV1,
    objects: &BTreeMap<StoreObjectIdV1, &StoreObjectV1>,
) -> Result<Vec<StoreObjectIdV1>, RepositoryGovernanceFloorErrorV1> {
    let mut current = direct_root;
    let mut visited = BTreeSet::new();
    let mut reverse = Vec::new();
    let mut expected_revision = None;
    let mut expected_repository = None;
    let mut expected_context = None;
    loop {
        if !visited.insert(current) {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidLineage);
        }
        let object = objects
            .get(&current)
            .ok_or(RepositoryGovernanceFloorErrorV1::InvalidLineage)?;
        let snapshot = RepositoryGovernanceFloorSnapshotV1::decode_object(object)?;
        if let Some(revision) = expected_revision
            && snapshot.floor_revision + 1 != revision
        {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidLineage);
        }
        if expected_repository
            .is_some_and(|repository| repository != snapshot.repository_store_domain)
            || expected_context.is_some_and(|context| context != snapshot.authority_context)
        {
            return Err(RepositoryGovernanceFloorErrorV1::InvalidLineage);
        }
        expected_revision = Some(snapshot.floor_revision);
        expected_repository = Some(snapshot.repository_store_domain);
        expected_context = Some(snapshot.authority_context);
        reverse.push(current);
        match snapshot.predecessor {
            Some(predecessor) => current = predecessor,
            None if snapshot.floor_revision == 1 => break,
            None => return Err(RepositoryGovernanceFloorErrorV1::InvalidLineage),
        }
    }
    reverse.reverse();
    Ok(reverse)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the versioned current-view commitment binds every retained governance and safety dimension"
)]
fn current_view_commitment(
    view: &StorePublicationViewV1<'_>,
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    snapshot: &RepositoryGovernanceFloorSnapshotV1,
    direct_root: StoreObjectIdV1,
    class_root: [u8; 32],
    authority: RepositoryGovernanceAuthorityCurrentnessV1,
    scheduling_safety: AuthoritySchedulingSafetyStateV1,
) -> Result<[u8; 32], RepositoryGovernanceFloorErrorV1> {
    hash_value(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-governance-floor-current-view.v1")?,
        bytes(view.domain().id().as_bytes()),
        bytes(head.id().as_bytes()),
        CborValue::Unsigned(head.revision()),
        bytes(generation.id().as_bytes()),
        CborValue::Unsigned(generation.ordinal()),
        bytes(direct_root.as_bytes()),
        bytes(&class_root),
        bytes(&snapshot.semantic_hash),
        CborValue::Unsigned(snapshot.floor_revision),
        bytes(&authority.authority_context),
        CborValue::Unsigned(authority.authority_epoch),
        CborValue::Unsigned(authority.trust_root_revision),
        bytes(&authority.trust_root_binding_commitment),
        bytes(&authority.authority_state_token),
        bytes(&authority.authority_fence),
        CborValue::Unsigned(authority.revocation_revision),
        bytes(&authority.principal),
        bytes(&authority.binding),
        bytes(&authority.session),
        CborValue::Unsigned(authority.assurance_revision),
        CborValue::Unsigned(authority.trusted_time),
        CborValue::Array(
            scheduling_safety
                .floor
                .iter()
                .copied()
                .map(CborValue::Unsigned)
                .collect(),
        ),
        bytes(&scheduling_safety.floor_identity),
        CborValue::Unsigned(scheduling_safety.floor_version),
        bytes(&scheduling_safety.floor_semantic_hash),
        bytes(&scheduling_safety.classifier_identity),
        CborValue::Unsigned(scheduling_safety.classifier_revision),
        bytes(&scheduling_safety.classifier_semantic_hash),
        bytes(&scheduling_safety.evaluator_identity),
        CborValue::Unsigned(scheduling_safety.evaluator_revision),
        bytes(&scheduling_safety.compatibility),
        bytes(&scheduling_safety.currentness),
    ]))
}

fn resolve_authority_scheduling_safety_state(
    snapshot: &RepositoryGovernanceFloorSnapshotV1,
    authority: RepositoryGovernanceAuthorityCurrentnessV1,
) -> Result<AuthoritySchedulingSafetyStateV1, RepositoryGovernanceFloorErrorV1> {
    let floor = AUTHORITY_SCHEDULING_SAFETY_MAXIMUMS_V1.map(|maximum| {
        u64::MAX
            .checked_sub(maximum)
            .expect("invariant: the pinned Scheduling Safety maximum is below u64::MAX")
    });
    let floor_identity = hash_value(&CborValue::text(
        "maestro.vnext.scheduling-safety-floor.v1",
    )?)?;
    let floor_semantic_hash = hash_value(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.scheduling-safety-floor-semantics.v1")?,
        bytes(&floor_identity),
        CborValue::Unsigned(AUTHORITY_SCHEDULING_SAFETY_FLOOR_VERSION_V1),
        CborValue::Array(floor.iter().copied().map(CborValue::Unsigned).collect()),
    ]))?;
    let classifier_identity = hash_value(&CborValue::text(
        "maestro.vnext.scheduling-policy-diff-classifier.v1",
    )?)?;
    let classifier_semantic_hash = hash_value(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.scheduling-policy-diff-classifier-semantics.v1")?,
        bytes(&classifier_identity),
        CborValue::Unsigned(AUTHORITY_SCHEDULING_CLASSIFIER_REVISION_V1),
        bytes(&floor_semantic_hash),
    ]))?;
    let evaluator_identity = hash_value(&CborValue::text(
        "maestro.vnext.scheduling-safety-floor-evaluator.v1",
    )?)?;
    let compatibility = hash_value(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.scheduling-safety-compatibility.v1")?,
        bytes(&floor_semantic_hash),
        bytes(&classifier_semantic_hash),
        bytes(&evaluator_identity),
        CborValue::Unsigned(AUTHORITY_SCHEDULING_EVALUATOR_REVISION_V1),
    ]))?;
    let currentness = hash_value(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.authority-scheduling-safety-currentness.v1")?,
        bytes(&snapshot.semantic_hash),
        CborValue::Unsigned(snapshot.floor_revision),
        CborValue::Unsigned(snapshot.authority_epoch),
        CborValue::Unsigned(snapshot.trust_root_revision),
        bytes(&authority.authority_state_token),
        bytes(&authority.authority_fence),
        CborValue::Unsigned(authority.revocation_revision),
        bytes(&compatibility),
    ]))?;
    Ok(AuthoritySchedulingSafetyStateV1 {
        floor,
        floor_identity,
        floor_version: AUTHORITY_SCHEDULING_SAFETY_FLOOR_VERSION_V1,
        floor_semantic_hash,
        classifier_identity,
        classifier_revision: AUTHORITY_SCHEDULING_CLASSIFIER_REVISION_V1,
        classifier_semantic_hash,
        evaluator_identity,
        evaluator_revision: AUTHORITY_SCHEDULING_EVALUATOR_REVISION_V1,
        compatibility,
        currentness,
    })
}

pub(super) fn repository_governance_floor_schema_id() -> Result<SchemaIdV1, IdentityError> {
    let canonical = CborValue::Array(vec![
        CborValue::Unsigned(REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_TAG_V1 as u64),
        CborValue::text(REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_NAME_V1)?,
        CborValue::Array(
            SCHEMA_FIELDS_V1
                .iter()
                .map(|field| CborValue::text(*field))
                .collect::<Result<_, _>>()?,
        ),
        CborValue::Array(
            SCHEMA_VARIANTS_V1
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    Ok(CborValue::Array(vec![
                        CborValue::Unsigned(index as u64 + 1),
                        CborValue::text(*variant)?,
                    ]))
                })
                .collect::<Result<_, CborError>>()?,
        ),
        CborValue::Array(
            SCHEMA_INVARIANTS_V1
                .iter()
                .map(|invariant| CborValue::text(*invariant))
                .collect::<Result<_, _>>()?,
        ),
    ]);
    let envelope = CborValue::Array(vec![
        CborValue::text(AUTHORITY_SCHEMA_DESCRIPTOR_DOMAIN_V1)?,
        canonical,
    ]);
    let encoded = deterministic_cbor::encode(&envelope)?;
    Ok(SchemaIdV1::from_digest(Sha256::digest(encoded).into()))
}

fn validate_participants(values: &[u64]) -> Result<(), RepositoryGovernanceFloorErrorV1> {
    if values.is_empty()
        || values.len() > MAX_REQUIREMENT_PARTICIPANTS
        || values.contains(&0)
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
    }
    Ok(())
}

fn unsigned_rows(values: &[CborValue]) -> Result<Vec<u64>, RepositoryGovernanceFloorErrorV1> {
    values
        .iter()
        .map(|value| match value {
            CborValue::Unsigned(value) => Ok(*value),
            _ => Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot),
        })
        .collect()
}

fn optional_object_id(
    value: &CborValue,
) -> Result<Option<StoreObjectIdV1>, RepositoryGovernanceFloorErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), CborValue::Bytes(bytes)] => {
            Ok(Some(StoreObjectIdV1::from_digest(exact_digest(bytes)?)))
        }
        _ => Err(RepositoryGovernanceFloorErrorV1::InvalidSnapshot),
    }
}

fn exact_digest(bytes: &[u8]) -> Result<[u8; 32], RepositoryGovernanceFloorErrorV1> {
    bytes
        .try_into()
        .map_err(|_| RepositoryGovernanceFloorErrorV1::InvalidSnapshot)
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn hash_value(value: &CborValue) -> Result<[u8; 32], RepositoryGovernanceFloorErrorV1> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

#[derive(Debug, Error)]
pub(in crate::domain::vnext) enum RepositoryGovernanceFloorErrorV1 {
    #[error("repository governance floor snapshot is invalid")]
    InvalidSnapshot,
    #[error("repository governance floor semantic hash does not match")]
    SemanticHashMismatch,
    #[error("repository governance floor Store object has the wrong schema")]
    WrongSchema,
    #[error("repository governance floor history is not gap-free")]
    InvalidLineage,
    #[error("repository governance floor has no unique direct root")]
    InvalidDirectRoot,
    #[error("repository governance floor does not match current Authority state")]
    CurrentnessMismatch,
    #[error("repository governance floor is missing the Action 105 requirement")]
    MissingAction105,
    #[error("repository governance floor Action 105 requirement is invalid")]
    InvalidAction105,
    #[error("repository governance floor rotation is not admitted by Action 68")]
    RotationNotAdmitted,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> RepositoryGovernanceFloorGenesisInputV1 {
        RepositoryGovernanceFloorGenesisInputV1 {
            repository_store_domain: [1; 32],
            authority_context: [2; 32],
            activation_generation_ordinal: 3,
            authority_epoch: 4,
            trust_root_revision: 5,
            trust_root_binding_commitment: [6; 32],
            authority_transition_protocol_identity: [7; 32],
            minimum_assurance: 8,
            requirement_grammar_identity: [9; 32],
            requirement_evaluator_identity: [10; 32],
        }
    }

    #[test]
    fn tag_25_snapshot_round_trips_and_preserves_the_action_105_totality_row() {
        let snapshot = RepositoryGovernanceFloorSnapshotV1::repository_genesis(input()).unwrap();
        let object = snapshot.to_store_object().unwrap();
        assert_eq!(
            object.schema_id(),
            repository_governance_floor_schema_id().unwrap()
        );
        assert_eq!(
            RepositoryGovernanceFloorSnapshotV1::decode_object(&object).unwrap(),
            snapshot
        );
        let requirement = snapshot.action_105_requirement().unwrap();
        assert_eq!(requirement.action_tag, 105);
        assert_eq!(requirement.owner_tag, 12);
        assert_eq!(requirement.participants, [1, 7, 14]);
    }

    #[test]
    fn rotation_requires_action_68_and_a_gap_free_advancing_lineage() {
        let genesis = RepositoryGovernanceFloorSnapshotV1::repository_genesis(input()).unwrap();
        let genesis_object = genesis.to_store_object().unwrap();
        assert!(matches!(
            RepositoryGovernanceFloorSnapshotV1::guarded_rotation(
                &genesis,
                RepositoryGovernanceFloorRotationInputV1 {
                    predecessor_id: genesis_object.id(),
                    activation_generation_ordinal: 4,
                    authority_epoch: 4,
                    trust_root_revision: 5,
                    trust_root_binding_commitment: [6; 32],
                    minimum_assurance: 8,
                    requirement_rows: vec![RepositoryGovernanceRequirementRowV1::action_105(8)],
                    admitted_action_tag: 67,
                },
            ),
            Err(RepositoryGovernanceFloorErrorV1::RotationNotAdmitted)
        ));
        let rotated = RepositoryGovernanceFloorSnapshotV1::guarded_rotation(
            &genesis,
            RepositoryGovernanceFloorRotationInputV1 {
                predecessor_id: genesis_object.id(),
                activation_generation_ordinal: 4,
                authority_epoch: 4,
                trust_root_revision: 5,
                trust_root_binding_commitment: [6; 32],
                minimum_assurance: 8,
                requirement_rows: vec![RepositoryGovernanceRequirementRowV1::action_105(8)],
                admitted_action_tag: 68,
            },
        )
        .unwrap();
        assert_eq!(rotated.revision(), 2);
        assert_eq!(
            rotated.to_store_object().unwrap().references(),
            [genesis_object.id()]
        );
    }

    #[test]
    fn explicit_legacy_migration_is_a_distinct_genesis_basis() {
        let migrated =
            RepositoryGovernanceFloorSnapshotV1::explicit_legacy_migration(input()).unwrap();
        assert_eq!(
            migrated.activation_basis,
            RepositoryGovernanceFloorActivationBasisV1::ExplicitLegacyMigration
        );
        assert_eq!(migrated.revision(), 1);
        assert!(migrated.predecessor.is_none());
        assert_eq!(migrated.action_105_requirement().unwrap().action_tag, 105);
    }

    #[test]
    fn restore_or_gap_in_floor_history_refuses_while_the_exact_same_domain_chain_passes() {
        let genesis = RepositoryGovernanceFloorSnapshotV1::repository_genesis(input()).unwrap();
        let genesis_object = genesis.to_store_object().unwrap();
        let rotated = RepositoryGovernanceFloorSnapshotV1::guarded_rotation(
            &genesis,
            RepositoryGovernanceFloorRotationInputV1 {
                predecessor_id: genesis_object.id(),
                activation_generation_ordinal: 4,
                authority_epoch: 4,
                trust_root_revision: 5,
                trust_root_binding_commitment: [6; 32],
                minimum_assurance: 8,
                requirement_rows: vec![RepositoryGovernanceRequirementRowV1::action_105(8)],
                admitted_action_tag: 68,
            },
        )
        .unwrap();
        let rotated_object = rotated.to_store_object().unwrap();
        let complete = [
            (genesis_object.id(), &genesis_object),
            (rotated_object.id(), &rotated_object),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            validate_history(rotated_object.id(), &complete).unwrap(),
            vec![genesis_object.id(), rotated_object.id()]
        );

        let gap = [(rotated_object.id(), &rotated_object)]
            .into_iter()
            .collect();
        assert!(matches!(
            validate_history(rotated_object.id(), &gap),
            Err(RepositoryGovernanceFloorErrorV1::InvalidLineage)
        ));

        let mut restored_elsewhere = rotated;
        restored_elsewhere.repository_store_domain = [99; 32];
        restored_elsewhere.semantic_hash = restored_elsewhere.recompute_semantic_hash().unwrap();
        let restored_object = restored_elsewhere.to_store_object().unwrap();
        let cross_domain = [
            (genesis_object.id(), &genesis_object),
            (restored_object.id(), &restored_object),
        ]
        .into_iter()
        .collect();
        assert!(matches!(
            validate_history(restored_object.id(), &cross_domain),
            Err(RepositoryGovernanceFloorErrorV1::InvalidLineage)
        ));
    }

    #[test]
    fn semantic_or_action_105_substitution_refuses() {
        let snapshot = RepositoryGovernanceFloorSnapshotV1::repository_genesis(input()).unwrap();
        let mut wrong_hash = snapshot.clone();
        wrong_hash.semantic_hash = [99; 32];
        assert!(matches!(
            wrong_hash.validate(),
            Err(RepositoryGovernanceFloorErrorV1::SemanticHashMismatch)
        ));

        let mut wrong_owner = snapshot;
        wrong_owner.requirement_rows[0].owner_tag = 11;
        wrong_owner.semantic_hash = wrong_owner.recompute_semantic_hash().unwrap();
        assert!(matches!(
            wrong_owner.validate(),
            Err(RepositoryGovernanceFloorErrorV1::InvalidAction105)
        ));
    }
}
