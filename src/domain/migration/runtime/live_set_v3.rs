use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};
use crate::foundation::core::legacy_loss_evidence::FoundationValidatedUnavailablePreexistingLossReceiptV1;
use crate::foundation::core::legacy_quarantine::{
    FoundationLegacyPayloadStateV3, FoundationMigrationOverlapPairV1,
    FoundationMigrationSourceCaseV1, LegacyQuarantineOwnerDomainV3,
};

use super::{MigrationDigestV1, MigrationIdentityErrorV1};

const MAX_ROWS_V3: usize = 1_000_000;
const MAX_LOCATOR_BYTES_V3: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyOwnerDomainV3 {
    Repository,
    Installation,
    ProtectedPrimary,
}

impl LegacyOwnerDomainV3 {
    const fn tag(self) -> u64 {
        match self {
            Self::Repository => 1,
            Self::Installation => 2,
            Self::ProtectedPrimary => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyNodeKindV3 {
    RegularFile,
    SymbolicLink,
}

impl LegacyNodeKindV3 {
    const fn tag(self) -> u64 {
        match self {
            Self::RegularFile => 1,
            Self::SymbolicLink => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyPayloadStateV3 {
    Present,
    UnavailablePreexistingLoss,
}

impl LegacyPayloadStateV3 {
    const fn tag(self) -> u64 {
        match self {
            Self::Present => 1,
            Self::UnavailablePreexistingLoss => 2,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MembershipKeyV3 {
    identity: MigrationDigestV1,
    owner: LegacyOwnerDomainV3,
    object_identity: MigrationDigestV1,
    metadata_commitment: MigrationDigestV1,
    owner_currentness: MigrationDigestV1,
    owner_attestation: MigrationDigestV1,
    canonical_value: CborValue,
}

impl std::fmt::Debug for MembershipKeyV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MembershipKeyV3")
            .field("identity", &self.identity)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl MembershipKeyV3 {
    #[expect(
        clippy::too_many_arguments,
        reason = "MembershipKeyV3 is the locked lossless physical membership tuple"
    )]
    pub(crate) fn from_foundation(
        owner: LegacyOwnerDomainV3,
        root_binding: MigrationDigestV1,
        display_locator: Vec<u8>,
        resolved_locator_commitment: MigrationDigestV1,
        object_identity: MigrationDigestV1,
        node_kind: LegacyNodeKindV3,
        metadata_commitment: MigrationDigestV1,
        owner_currentness: MigrationDigestV1,
        owner_attestation: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        validate_locator(&display_locator)?;
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.membership-key.v3\0",
            &CborValue::Array(vec![
                CborValue::Unsigned(owner.tag()),
                root_binding.canonical_value(),
                CborValue::Bytes(display_locator.clone()),
                resolved_locator_commitment.canonical_value(),
                object_identity.canonical_value(),
                CborValue::Unsigned(node_kind.tag()),
                metadata_commitment.canonical_value(),
                owner_currentness.canonical_value(),
                owner_attestation.canonical_value(),
            ]),
        )?;
        let canonical_value = CborValue::Array(vec![
            identity.canonical_value(),
            CborValue::Unsigned(owner.tag()),
            root_binding.canonical_value(),
            CborValue::Bytes(display_locator),
            resolved_locator_commitment.canonical_value(),
            object_identity.canonical_value(),
            CborValue::Unsigned(node_kind.tag()),
            metadata_commitment.canonical_value(),
            owner_currentness.canonical_value(),
            owner_attestation.canonical_value(),
        ]);
        Ok(Self {
            identity,
            owner,
            object_identity,
            metadata_commitment,
            owner_currentness,
            owner_attestation,
            canonical_value,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    const fn owner(&self) -> LegacyOwnerDomainV3 {
        self.owner
    }

    const fn object_identity(&self) -> MigrationDigestV1 {
        self.object_identity
    }

    const fn metadata_commitment(&self) -> MigrationDigestV1 {
        self.metadata_commitment
    }

    const fn owner_currentness(&self) -> MigrationDigestV1 {
        self.owner_currentness
    }

    const fn owner_attestation(&self) -> MigrationDigestV1 {
        self.owner_attestation
    }

    fn canonical_value(&self) -> CborValue {
        self.canonical_value.clone()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SourceCaseV3 {
    identity: MigrationDigestV1,
    membership: MembershipKeyV3,
    foundation_invocation: MigrationDigestV1,
    payload_state: LegacyPayloadStateV3,
    logical_length: u64,
    content_sha256: MigrationDigestV1,
    metadata_commitment: MigrationDigestV1,
    canonical_value: CborValue,
}

impl std::fmt::Debug for SourceCaseV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceCaseV3")
            .field("identity", &self.identity)
            .field("membership_id", &self.membership.identity())
            .field("payload_state", &self.payload_state)
            .field("logical_length", &self.logical_length)
            .finish_non_exhaustive()
    }
}

impl SourceCaseV3 {
    pub(crate) fn from_foundation(
        membership: MembershipKeyV3,
        foundation_invocation: MigrationDigestV1,
        payload_state: LegacyPayloadStateV3,
        logical_length: u64,
        content_sha256: MigrationDigestV1,
        metadata_commitment: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        if metadata_commitment != membership.metadata_commitment() {
            return Err(LiveSetV3Error::MetadataMismatch);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.source-case.v3\0",
            &CborValue::Array(vec![
                membership.identity().canonical_value(),
                foundation_invocation.canonical_value(),
                CborValue::Unsigned(payload_state.tag()),
                CborValue::Unsigned(logical_length),
                content_sha256.canonical_value(),
                metadata_commitment.canonical_value(),
            ]),
        )?;
        let canonical_value = CborValue::Array(vec![
            identity.canonical_value(),
            membership.canonical_value(),
            foundation_invocation.canonical_value(),
            CborValue::Unsigned(payload_state.tag()),
            CborValue::Unsigned(logical_length),
            content_sha256.canonical_value(),
            metadata_commitment.canonical_value(),
        ]);
        Ok(Self {
            identity,
            membership,
            foundation_invocation,
            payload_state,
            logical_length,
            content_sha256,
            metadata_commitment,
            canonical_value,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn membership_id(&self) -> MigrationDigestV1 {
        self.membership.identity()
    }

    const fn membership(&self) -> &MembershipKeyV3 {
        &self.membership
    }

    pub const fn foundation_invocation(&self) -> MigrationDigestV1 {
        self.foundation_invocation
    }

    pub const fn payload_state(&self) -> LegacyPayloadStateV3 {
        self.payload_state
    }

    pub const fn logical_length(&self) -> u64 {
        self.logical_length
    }

    pub const fn content_sha256(&self) -> MigrationDigestV1 {
        self.content_sha256
    }

    pub const fn metadata_commitment(&self) -> MigrationDigestV1 {
        self.metadata_commitment
    }

    fn canonical_value(&self) -> CborValue {
        self.canonical_value.clone()
    }
}

pub(crate) struct FoundationMaterializedSourceCaseV3 {
    source_case: SourceCaseV3,
    source_token: [u8; 32],
    loss_receipt: Option<FoundationValidatedUnavailablePreexistingLossReceiptV1>,
}

impl FoundationMaterializedSourceCaseV3 {
    pub(crate) fn from_foundation_v2(
        source: FoundationMigrationSourceCaseV1,
        foundation_invocation: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        let parts = source.into_semantic();
        let owner = match parts.owner {
            LegacyQuarantineOwnerDomainV3::Repository => LegacyOwnerDomainV3::Repository,
            LegacyQuarantineOwnerDomainV3::Installation => LegacyOwnerDomainV3::Installation,
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary => {
                LegacyOwnerDomainV3::ProtectedPrimary
            }
        };
        let object_identity = MigrationDigestV1::from_digest(parts.object_identity)?;
        let content_sha256 = MigrationDigestV1::from_digest(parts.content_identity)?;
        let metadata_commitment = MigrationDigestV1::from_digest(parts.metadata_commitment)?;
        let membership_identity = MigrationDigestV1::from_digest(parts.membership_identity)?;
        let membership_value = deterministic_cbor::decode(&parts.membership_encoding)?;
        validate_materialized_identity(&membership_value, membership_identity)?;
        let membership = MembershipKeyV3 {
            identity: membership_identity,
            owner,
            object_identity,
            metadata_commitment,
            owner_currentness: MigrationDigestV1::from_digest(parts.owner_currentness)?,
            owner_attestation: MigrationDigestV1::from_digest(parts.owner_attestation)?,
            canonical_value: membership_value.clone(),
        };
        let payload_state = match parts.payload_state {
            FoundationLegacyPayloadStateV3::Present => LegacyPayloadStateV3::Present,
            FoundationLegacyPayloadStateV3::UnavailablePreexistingLoss => {
                LegacyPayloadStateV3::UnavailablePreexistingLoss
            }
        };
        let source_case_identity = MigrationDigestV1::from_digest(parts.source_case_identity)?;
        let source_case_value = deterministic_cbor::decode(&parts.source_case_encoding)?;
        validate_materialized_source_case(
            &source_case_value,
            source_case_identity,
            &membership_value,
            foundation_invocation,
            payload_state,
            parts.logical_byte_length,
            content_sha256,
            metadata_commitment,
        )?;
        let source_case = SourceCaseV3 {
            identity: source_case_identity,
            membership,
            foundation_invocation,
            payload_state,
            logical_length: parts.logical_byte_length,
            content_sha256,
            metadata_commitment,
            canonical_value: source_case_value,
        };
        Ok(Self {
            source_case,
            source_token: parts.source_token,
            loss_receipt: parts.loss_receipt,
        })
    }

    pub(crate) const fn source_case(&self) -> &SourceCaseV3 {
        &self.source_case
    }

    pub(crate) const fn source_token(&self) -> [u8; 32] {
        self.source_token
    }

    pub(crate) fn take_loss_receipt(
        &mut self,
    ) -> Option<FoundationValidatedUnavailablePreexistingLossReceiptV1> {
        self.loss_receipt.take()
    }

    pub(crate) fn into_source_case(self) -> SourceCaseV3 {
        self.source_case
    }
}

impl ProtectedPrimaryOverlapPairV1 {
    pub(crate) fn from_foundation_materialized(
        pair: FoundationMigrationOverlapPairV1,
    ) -> Result<Self, LiveSetV3Error> {
        let pair = pair.into_semantic();
        let identity = MigrationDigestV1::from_digest(pair.identity)?;
        let canonical_value = deterministic_cbor::decode(&pair.canonical_encoding)?;
        validate_materialized_identity(&canonical_value, identity)?;
        Ok(Self {
            identity,
            owner_source_case_id: MigrationDigestV1::from_digest(pair.owner_source_case_id)?,
            primary_source_case_id: MigrationDigestV1::from_digest(pair.primary_source_case_id)?,
            canonical_value,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySourceCaseManifestV3 {
    identity: MigrationDigestV1,
    foundation_invocation: MigrationDigestV1,
    admitted_set_id: MigrationDigestV1,
    rows: Vec<SourceCaseV3>,
}

impl LegacySourceCaseManifestV3 {
    pub(crate) fn new(
        foundation_invocation: MigrationDigestV1,
        admitted_set_id: MigrationDigestV1,
        mut rows: Vec<SourceCaseV3>,
    ) -> Result<Self, LiveSetV3Error> {
        validate_row_count(rows.len())?;
        rows.sort_by_key(SourceCaseV3::identity);
        if rows.is_empty()
            || rows.windows(2).any(|pair| {
                pair[0].identity() == pair[1].identity()
                    || pair[0].membership_id() == pair[1].membership_id()
            })
            || rows
                .iter()
                .any(|row| row.foundation_invocation() != foundation_invocation)
        {
            return Err(LiveSetV3Error::InvalidSourceManifest);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.legacy-source-case-manifest.v3\0",
            &CborValue::Array(vec![
                foundation_invocation.canonical_value(),
                admitted_set_id.canonical_value(),
                CborValue::Array(rows.iter().map(SourceCaseV3::canonical_value).collect()),
            ]),
        )?;
        Ok(Self {
            identity,
            foundation_invocation,
            admitted_set_id,
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn foundation_invocation(&self) -> MigrationDigestV1 {
        self.foundation_invocation
    }

    pub const fn admitted_set_id(&self) -> MigrationDigestV1 {
        self.admitted_set_id
    }

    pub fn rows(&self) -> &[SourceCaseV3] {
        &self.rows
    }

    fn by_id(&self) -> BTreeMap<MigrationDigestV1, &SourceCaseV3> {
        self.rows.iter().map(|row| (row.identity(), row)).collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage12SightingV2 {
    identity: MigrationDigestV1,
    source_case_id: MigrationDigestV1,
    policy_id: MigrationDigestV1,
    rule_id: MigrationDigestV1,
    matcher_id: MigrationDigestV1,
    matched_bytes_sha256: MigrationDigestV1,
    byte_offset: u64,
}

impl Stage12SightingV2 {
    pub(crate) fn new(
        source_case_id: MigrationDigestV1,
        policy_id: MigrationDigestV1,
        rule_id: MigrationDigestV1,
        matcher_id: MigrationDigestV1,
        matched_bytes: &[u8],
        byte_offset: u64,
    ) -> Result<Self, LiveSetV3Error> {
        if matched_bytes.is_empty() {
            return Err(LiveSetV3Error::InvalidSighting);
        }
        let matched_bytes_sha256 = MigrationDigestV1::digest_bytes(matched_bytes)?;
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.stage12-sighting.v2\0",
            &CborValue::Array(vec![
                source_case_id.canonical_value(),
                policy_id.canonical_value(),
                rule_id.canonical_value(),
                matcher_id.canonical_value(),
                matched_bytes_sha256.canonical_value(),
                CborValue::Unsigned(byte_offset),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_id,
            policy_id,
            rule_id,
            matcher_id,
            matched_bytes_sha256,
            byte_offset,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_id(&self) -> MigrationDigestV1 {
        self.source_case_id
    }

    pub const fn policy_id(&self) -> MigrationDigestV1 {
        self.policy_id
    }

    pub const fn rule_id(&self) -> MigrationDigestV1 {
        self.rule_id
    }

    pub const fn matcher_id(&self) -> MigrationDigestV1 {
        self.matcher_id
    }

    pub const fn matched_bytes_sha256(&self) -> MigrationDigestV1 {
        self.matched_bytes_sha256
    }

    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.identity.canonical_value(),
            self.source_case_id.canonical_value(),
            self.policy_id.canonical_value(),
            self.rule_id.canonical_value(),
            self.matcher_id.canonical_value(),
            self.matched_bytes_sha256.canonical_value(),
            CborValue::Unsigned(self.byte_offset),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage12SightingManifestV2 {
    identity: MigrationDigestV1,
    policy_id: MigrationDigestV1,
    candidate_commit_id: MigrationDigestV1,
    candidate_tree_id: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    rows: Vec<Stage12SightingV2>,
}

impl Stage12SightingManifestV2 {
    pub(crate) fn new(
        policy_id: MigrationDigestV1,
        candidate_commit_id: MigrationDigestV1,
        candidate_tree_id: MigrationDigestV1,
        source_cases: &LegacySourceCaseManifestV3,
        mut rows: Vec<Stage12SightingV2>,
    ) -> Result<Self, LiveSetV3Error> {
        validate_row_count(rows.len())?;
        rows.sort_by_key(Stage12SightingV2::identity);
        let source_ids = source_cases.by_id();
        if rows
            .windows(2)
            .any(|pair| pair[0].identity() == pair[1].identity())
            || rows.iter().any(|row| {
                row.policy_id() != policy_id || !source_ids.contains_key(&row.source_case_id())
            })
        {
            return Err(LiveSetV3Error::InvalidSightingManifest);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.stage12-sighting-manifest.v2\0",
            &CborValue::Array(vec![
                policy_id.canonical_value(),
                candidate_commit_id.canonical_value(),
                candidate_tree_id.canonical_value(),
                source_cases.identity().canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(Stage12SightingV2::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            identity,
            policy_id,
            candidate_commit_id,
            candidate_tree_id,
            source_case_manifest_id: source_cases.identity(),
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn policy_id(&self) -> MigrationDigestV1 {
        self.policy_id
    }

    pub const fn candidate_commit_id(&self) -> MigrationDigestV1 {
        self.candidate_commit_id
    }

    pub const fn candidate_tree_id(&self) -> MigrationDigestV1 {
        self.candidate_tree_id
    }

    pub const fn source_case_manifest_id(&self) -> MigrationDigestV1 {
        self.source_case_manifest_id
    }

    pub fn rows(&self) -> &[Stage12SightingV2] {
        &self.rows
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MigrationDispositionV3 {
    CurrentTypedSuccessor,
    AdmittedSealedAuditReader,
    RemoveAfterGuard,
    QuarantineOnly,
    UnavailablePreexistingLoss,
}

impl MigrationDispositionV3 {
    const fn tag(self) -> u64 {
        match self {
            Self::CurrentTypedSuccessor => 1,
            Self::AdmittedSealedAuditReader => 2,
            Self::RemoveAfterGuard => 3,
            Self::QuarantineOnly => 4,
            Self::UnavailablePreexistingLoss => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationClassificationV3 {
    identity: MigrationDigestV1,
    source_case_id: MigrationDigestV1,
    disposition: MigrationDispositionV3,
    reason_id: MigrationDigestV1,
}

impl MigrationClassificationV3 {
    pub(crate) fn new(
        source: &SourceCaseV3,
        disposition: MigrationDispositionV3,
        reason_id: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        if (source.payload_state() == LegacyPayloadStateV3::UnavailablePreexistingLoss)
            != (disposition == MigrationDispositionV3::UnavailablePreexistingLoss)
        {
            return Err(LiveSetV3Error::InvalidDisposition);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.classification.v3\0",
            &CborValue::Array(vec![
                source.identity().canonical_value(),
                CborValue::Unsigned(disposition.tag()),
                reason_id.canonical_value(),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_id: source.identity(),
            disposition,
            reason_id,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_id(&self) -> MigrationDigestV1 {
        self.source_case_id
    }

    pub const fn disposition(&self) -> MigrationDispositionV3 {
        self.disposition
    }

    pub const fn reason_id(&self) -> MigrationDigestV1 {
        self.reason_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.identity.canonical_value(),
            self.source_case_id.canonical_value(),
            CborValue::Unsigned(self.disposition.tag()),
            self.reason_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationClassificationManifestV3 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    policy_id: MigrationDigestV1,
    rows: Vec<MigrationClassificationV3>,
}

impl MigrationClassificationManifestV3 {
    pub(crate) fn new(
        source_cases: &LegacySourceCaseManifestV3,
        policy_id: MigrationDigestV1,
        mut rows: Vec<MigrationClassificationV3>,
    ) -> Result<Self, LiveSetV3Error> {
        rows.sort_by_key(MigrationClassificationV3::source_case_id);
        let expected = source_cases
            .rows()
            .iter()
            .map(SourceCaseV3::identity)
            .collect::<Vec<_>>();
        let observed = rows
            .iter()
            .map(MigrationClassificationV3::source_case_id)
            .collect::<Vec<_>>();
        if expected != observed {
            return Err(LiveSetV3Error::IncompleteClassification);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.classification-manifest.v3\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                policy_id.canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(MigrationClassificationV3::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            policy_id,
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_manifest_id(&self) -> MigrationDigestV1 {
        self.source_case_manifest_id
    }

    pub const fn policy_id(&self) -> MigrationDigestV1 {
        self.policy_id
    }

    pub fn rows(&self) -> &[MigrationClassificationV3] {
        &self.rows
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProtectedPrimaryOverlapPairV1 {
    identity: MigrationDigestV1,
    owner_source_case_id: MigrationDigestV1,
    primary_source_case_id: MigrationDigestV1,
    canonical_value: CborValue,
}

impl std::fmt::Debug for ProtectedPrimaryOverlapPairV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProtectedPrimaryOverlapPairV1")
            .field("identity", &self.identity)
            .field("owner_source_case_id", &self.owner_source_case_id)
            .field("primary_source_case_id", &self.primary_source_case_id)
            .finish_non_exhaustive()
    }
}

impl ProtectedPrimaryOverlapPairV1 {
    pub(crate) fn from_foundation(
        owner: &SourceCaseV3,
        primary: &SourceCaseV3,
        owner_mount_identity: MigrationDigestV1,
        primary_mount_identity: MigrationDigestV1,
        owner_provider_identity: MigrationDigestV1,
        primary_provider_identity: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        if owner.membership().owner() == LegacyOwnerDomainV3::ProtectedPrimary
            || primary.membership().owner() != LegacyOwnerDomainV3::ProtectedPrimary
            || owner.identity() == primary.identity()
            || owner.membership().object_identity() != primary.membership().object_identity()
            || owner_mount_identity != primary_mount_identity
            || owner_provider_identity != primary_provider_identity
            || owner.content_sha256() != primary.content_sha256()
            || owner.metadata_commitment() != primary.metadata_commitment()
        {
            return Err(LiveSetV3Error::InvalidOverlapPair);
        }
        let owner_source_case_id = owner.identity();
        let primary_source_case_id = primary.identity();
        let owner_attestation = owner.membership().owner_attestation();
        let primary_attestation = primary.membership().owner_attestation();
        let owner_object_identity = owner.membership().object_identity();
        let primary_object_identity = primary.membership().object_identity();
        let owner_content_commitment = owner.content_sha256();
        let primary_content_commitment = primary.content_sha256();
        let owner_metadata_commitment = owner.metadata_commitment();
        let primary_metadata_commitment = primary.metadata_commitment();
        let owner_currentness = owner.membership().owner_currentness();
        let primary_currentness = primary.membership().owner_currentness();
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.protected-primary-overlap-pair.v1\0",
            &CborValue::Array(
                [
                    owner_source_case_id,
                    primary_source_case_id,
                    owner_attestation,
                    primary_attestation,
                    owner_object_identity,
                    primary_object_identity,
                    owner_mount_identity,
                    primary_mount_identity,
                    owner_provider_identity,
                    primary_provider_identity,
                    owner_content_commitment,
                    primary_content_commitment,
                    owner_metadata_commitment,
                    primary_metadata_commitment,
                    owner_currentness,
                    primary_currentness,
                ]
                .into_iter()
                .map(MigrationDigestV1::canonical_value)
                .collect(),
            ),
        )?;
        let canonical_value = CborValue::Array(
            [
                identity,
                owner_source_case_id,
                primary_source_case_id,
                owner_attestation,
                primary_attestation,
                owner_object_identity,
                primary_object_identity,
                owner_mount_identity,
                primary_mount_identity,
                owner_provider_identity,
                primary_provider_identity,
                owner_content_commitment,
                primary_content_commitment,
                owner_metadata_commitment,
                primary_metadata_commitment,
                owner_currentness,
                primary_currentness,
            ]
            .into_iter()
            .map(MigrationDigestV1::canonical_value)
            .collect(),
        );
        Ok(Self {
            identity,
            owner_source_case_id,
            primary_source_case_id,
            canonical_value,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn owner_source_case_id(&self) -> MigrationDigestV1 {
        self.owner_source_case_id
    }

    pub const fn primary_source_case_id(&self) -> MigrationDigestV1 {
        self.primary_source_case_id
    }

    fn matches(&self, owner: &SourceCaseV3, primary: &SourceCaseV3) -> bool {
        owner.identity() == self.owner_source_case_id
            && primary.identity() == self.primary_source_case_id
            && owner.membership().owner() != LegacyOwnerDomainV3::ProtectedPrimary
            && primary.membership().owner() == LegacyOwnerDomainV3::ProtectedPrimary
    }

    fn canonical_value(&self) -> CborValue {
        self.canonical_value.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredOverlapManifestV2 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    rows: Vec<ProtectedPrimaryOverlapPairV1>,
}

impl DeclaredOverlapManifestV2 {
    pub(crate) fn new(
        source_cases: &LegacySourceCaseManifestV3,
        mut rows: Vec<ProtectedPrimaryOverlapPairV1>,
    ) -> Result<Self, LiveSetV3Error> {
        validate_row_count(rows.len())?;
        rows.sort_by_key(ProtectedPrimaryOverlapPairV1::identity);
        let source_ids = source_cases.by_id();
        let mut owners = BTreeSet::new();
        let mut primaries = BTreeSet::new();
        if rows.iter().any(|row| {
            source_ids
                .get(&row.owner_source_case_id())
                .zip(source_ids.get(&row.primary_source_case_id()))
                .is_none_or(|(owner, primary)| !row.matches(owner, primary))
                || !owners.insert(row.owner_source_case_id())
                || !primaries.insert(row.primary_source_case_id())
        }) {
            return Err(LiveSetV3Error::InvalidOverlapPair);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.declared-overlap-manifest.v2\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(ProtectedPrimaryOverlapPairV1::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub fn rows(&self) -> &[ProtectedPrimaryOverlapPairV1] {
        &self.rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailablePreexistingLossV3 {
    identity: MigrationDigestV1,
    source_case_id: MigrationDigestV1,
    historical_length: u64,
    historical_sha256: MigrationDigestV1,
    metadata_commitment: MigrationDigestV1,
    source_provenance_id: MigrationDigestV1,
    loss_evidence_id: MigrationDigestV1,
}

impl UnavailablePreexistingLossV3 {
    pub(crate) fn new(
        source: &SourceCaseV3,
        historical_length: u64,
        historical_sha256: MigrationDigestV1,
        metadata_commitment: MigrationDigestV1,
        source_provenance_id: MigrationDigestV1,
        loss_evidence_id: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        if source.payload_state() != LegacyPayloadStateV3::UnavailablePreexistingLoss
            || source.logical_length() != historical_length
            || source.content_sha256() != historical_sha256
            || source.metadata_commitment() != metadata_commitment
        {
            return Err(LiveSetV3Error::InvalidLossEvidence);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.unavailable-preexisting-loss.v3\0",
            &CborValue::Array(vec![
                source.identity().canonical_value(),
                CborValue::Unsigned(historical_length),
                historical_sha256.canonical_value(),
                metadata_commitment.canonical_value(),
                source_provenance_id.canonical_value(),
                loss_evidence_id.canonical_value(),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_id: source.identity(),
            historical_length,
            historical_sha256,
            metadata_commitment,
            source_provenance_id,
            loss_evidence_id,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_id(&self) -> MigrationDigestV1 {
        self.source_case_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.identity.canonical_value(),
            self.source_case_id.canonical_value(),
            CborValue::Unsigned(self.historical_length),
            self.historical_sha256.canonical_value(),
            self.metadata_commitment.canonical_value(),
            self.source_provenance_id.canonical_value(),
            self.loss_evidence_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailablePreexistingLossManifestV3 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    rows: Vec<UnavailablePreexistingLossV3>,
}

impl UnavailablePreexistingLossManifestV3 {
    pub(crate) fn new(
        source_cases: &LegacySourceCaseManifestV3,
        classifications: &MigrationClassificationManifestV3,
        mut rows: Vec<UnavailablePreexistingLossV3>,
    ) -> Result<Self, LiveSetV3Error> {
        rows.sort_by_key(UnavailablePreexistingLossV3::source_case_id);
        let expected = classifications
            .rows()
            .iter()
            .filter(|row| row.disposition() == MigrationDispositionV3::UnavailablePreexistingLoss)
            .map(MigrationClassificationV3::source_case_id)
            .collect::<Vec<_>>();
        let observed = rows
            .iter()
            .map(UnavailablePreexistingLossV3::source_case_id)
            .collect::<Vec<_>>();
        if expected != observed
            || rows
                .iter()
                .any(|row| !source_cases.by_id().contains_key(&row.source_case_id()))
        {
            return Err(LiveSetV3Error::InvalidLossEvidence);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.unavailable-preexisting-loss-manifest.v3\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                classifications.identity().canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(UnavailablePreexistingLossV3::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub fn rows(&self) -> &[UnavailablePreexistingLossV3] {
        &self.rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailablePreexistingLossV4 {
    identity: MigrationDigestV1,
    source_case_id: MigrationDigestV1,
    owner_snapshot_id: MigrationDigestV1,
    issuer_id: MigrationDigestV1,
    historical_tuple_id: MigrationDigestV1,
    owner_current_tuple_id: MigrationDigestV1,
    source_provenance_id: MigrationDigestV1,
    owner_admission_id: MigrationDigestV1,
    owner_currentness_id: MigrationDigestV1,
    foundation_loss_receipt_id: MigrationDigestV1,
    validation_invocation_id: MigrationDigestV1,
    pass_a_absence_id: MigrationDigestV1,
    pass_b_absence_id: MigrationDigestV1,
}

impl UnavailablePreexistingLossV4 {
    pub(crate) fn from_foundation(
        source: &SourceCaseV3,
        source_token: [u8; 32],
        receipt: FoundationValidatedUnavailablePreexistingLossReceiptV1,
    ) -> Result<Self, LiveSetV3Error> {
        if source.payload_state() != LegacyPayloadStateV3::UnavailablePreexistingLoss
            || receipt.source_token() != source_token
        {
            return Err(LiveSetV3Error::InvalidLossEvidence);
        }
        let owner_snapshot_id = MigrationDigestV1::from_digest(receipt.snapshot_id())?;
        let issuer_id = MigrationDigestV1::from_digest(receipt.issuer_id())?;
        let historical_tuple_id = MigrationDigestV1::from_digest(receipt.historical_tuple_id())?;
        let owner_current_tuple_id = MigrationDigestV1::from_digest(receipt.current_tuple_id())?;
        let source_provenance_id = MigrationDigestV1::from_digest(receipt.source_provenance_id())?;
        let owner_admission_id = MigrationDigestV1::from_digest(receipt.owner_admission_id())?;
        let owner_currentness_id = MigrationDigestV1::from_digest(receipt.owner_currentness_id())?;
        let foundation_loss_receipt_id = MigrationDigestV1::from_digest(receipt.identity())?;
        let validation_invocation_id =
            MigrationDigestV1::from_digest(receipt.validation_invocation())?;
        let pass_a_absence_id = MigrationDigestV1::from_digest(receipt.pass_a_absence_id())?;
        let pass_b_absence_id = MigrationDigestV1::from_digest(receipt.pass_b_absence_id())?;
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.unavailable-preexisting-loss.v4\0",
            &CborValue::Array(vec![
                source.identity().canonical_value(),
                owner_snapshot_id.canonical_value(),
                issuer_id.canonical_value(),
                historical_tuple_id.canonical_value(),
                owner_current_tuple_id.canonical_value(),
                source_provenance_id.canonical_value(),
                owner_admission_id.canonical_value(),
                owner_currentness_id.canonical_value(),
                foundation_loss_receipt_id.canonical_value(),
                validation_invocation_id.canonical_value(),
                pass_a_absence_id.canonical_value(),
                pass_b_absence_id.canonical_value(),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_id: source.identity(),
            owner_snapshot_id,
            issuer_id,
            historical_tuple_id,
            owner_current_tuple_id,
            source_provenance_id,
            owner_admission_id,
            owner_currentness_id,
            foundation_loss_receipt_id,
            validation_invocation_id,
            pass_a_absence_id,
            pass_b_absence_id,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_id(&self) -> MigrationDigestV1 {
        self.source_case_id
    }

    pub(crate) fn audit_currentness(&self) -> UnavailablePreexistingLossAuditCurrentnessV4 {
        UnavailablePreexistingLossAuditCurrentnessV4 {
            source_case_id: self.source_case_id,
            owner_currentness_id: self.owner_currentness_id,
            foundation_loss_receipt_id: self.foundation_loss_receipt_id,
            validation_invocation_id: self.validation_invocation_id,
            pass_a_absence_id: self.pass_a_absence_id,
            pass_b_absence_id: self.pass_b_absence_id,
        }
    }

    pub(crate) fn encode_canonical_audit(&self) -> Result<Vec<u8>, LiveSetV3Error> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.migration.unavailable-preexisting-loss.audit.v4")?,
            self.canonical_value(),
        ]))?)
    }

    pub(crate) fn decode_canonical_audit(
        bytes: &[u8],
        expected_currentness: &UnavailablePreexistingLossAuditCurrentnessV4,
    ) -> Result<Self, LiveSetV3Error> {
        let decoded = deterministic_cbor::decode(bytes)?;
        if deterministic_cbor::encode(&decoded)? != bytes {
            return Err(LiveSetV3Error::InvalidLossAudit);
        }
        let CborValue::Array(envelope) = decoded else {
            return Err(LiveSetV3Error::InvalidLossAudit);
        };
        let [CborValue::Text(schema), CborValue::Array(fields)] = envelope.as_slice() else {
            return Err(LiveSetV3Error::InvalidLossAudit);
        };
        if schema != "maestro.migration.unavailable-preexisting-loss.audit.v4" || fields.len() != 13
        {
            return Err(LiveSetV3Error::InvalidLossAudit);
        }
        let identity = migration_digest_field(&fields[0])?;
        let source_case_id = migration_digest_field(&fields[1])?;
        let owner_snapshot_id = migration_digest_field(&fields[2])?;
        let issuer_id = migration_digest_field(&fields[3])?;
        let historical_tuple_id = migration_digest_field(&fields[4])?;
        let owner_current_tuple_id = migration_digest_field(&fields[5])?;
        let source_provenance_id = migration_digest_field(&fields[6])?;
        let owner_admission_id = migration_digest_field(&fields[7])?;
        let owner_currentness_id = migration_digest_field(&fields[8])?;
        let foundation_loss_receipt_id = migration_digest_field(&fields[9])?;
        let validation_invocation_id = migration_digest_field(&fields[10])?;
        let pass_a_absence_id = migration_digest_field(&fields[11])?;
        let pass_b_absence_id = migration_digest_field(&fields[12])?;
        let recomputed = MigrationDigestV1::identify(
            b"maestro.migration.unavailable-preexisting-loss.v4\0",
            &CborValue::Array(
                [
                    source_case_id,
                    owner_snapshot_id,
                    issuer_id,
                    historical_tuple_id,
                    owner_current_tuple_id,
                    source_provenance_id,
                    owner_admission_id,
                    owner_currentness_id,
                    foundation_loss_receipt_id,
                    validation_invocation_id,
                    pass_a_absence_id,
                    pass_b_absence_id,
                ]
                .into_iter()
                .map(MigrationDigestV1::canonical_value)
                .collect(),
            ),
        )?;
        let decoded = Self {
            identity,
            source_case_id,
            owner_snapshot_id,
            issuer_id,
            historical_tuple_id,
            owner_current_tuple_id,
            source_provenance_id,
            owner_admission_id,
            owner_currentness_id,
            foundation_loss_receipt_id,
            validation_invocation_id,
            pass_a_absence_id,
            pass_b_absence_id,
        };
        if recomputed != identity || decoded.audit_currentness() != *expected_currentness {
            return Err(LiveSetV3Error::InvalidLossAudit);
        }
        Ok(decoded)
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.identity.canonical_value(),
            self.source_case_id.canonical_value(),
            self.owner_snapshot_id.canonical_value(),
            self.issuer_id.canonical_value(),
            self.historical_tuple_id.canonical_value(),
            self.owner_current_tuple_id.canonical_value(),
            self.source_provenance_id.canonical_value(),
            self.owner_admission_id.canonical_value(),
            self.owner_currentness_id.canonical_value(),
            self.foundation_loss_receipt_id.canonical_value(),
            self.validation_invocation_id.canonical_value(),
            self.pass_a_absence_id.canonical_value(),
            self.pass_b_absence_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnavailablePreexistingLossAuditCurrentnessV4 {
    source_case_id: MigrationDigestV1,
    owner_currentness_id: MigrationDigestV1,
    foundation_loss_receipt_id: MigrationDigestV1,
    validation_invocation_id: MigrationDigestV1,
    pass_a_absence_id: MigrationDigestV1,
    pass_b_absence_id: MigrationDigestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailablePreexistingLossManifestV4 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    rows: Vec<UnavailablePreexistingLossV4>,
}

impl UnavailablePreexistingLossManifestV4 {
    pub(crate) fn new(
        source_cases: &LegacySourceCaseManifestV3,
        classifications: &MigrationClassificationManifestV3,
        mut rows: Vec<UnavailablePreexistingLossV4>,
    ) -> Result<Self, LiveSetV3Error> {
        rows.sort_by_key(UnavailablePreexistingLossV4::source_case_id);
        let expected = classifications
            .rows()
            .iter()
            .filter(|row| row.disposition() == MigrationDispositionV3::UnavailablePreexistingLoss)
            .map(MigrationClassificationV3::source_case_id)
            .collect::<Vec<_>>();
        let observed = rows
            .iter()
            .map(UnavailablePreexistingLossV4::source_case_id)
            .collect::<Vec<_>>();
        if expected != observed
            || rows
                .iter()
                .any(|row| !source_cases.by_id().contains_key(&row.source_case_id()))
        {
            return Err(LiveSetV3Error::InvalidLossEvidence);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.unavailable-preexisting-loss-manifest.v4\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                classifications.identity().canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(UnavailablePreexistingLossV4::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            classification_manifest_id: classifications.identity(),
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub fn rows(&self) -> &[UnavailablePreexistingLossV4] {
        &self.rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedQuarantineEntryV3 {
    identity: MigrationDigestV1,
    source_case_id: MigrationDigestV1,
    copied_length: u64,
    copied_sha256: MigrationDigestV1,
    metadata_commitment: MigrationDigestV1,
    custody_record_id: MigrationDigestV1,
}

impl SealedQuarantineEntryV3 {
    pub(crate) fn from_copy(
        source: &SourceCaseV3,
        copied_bytes: &[u8],
        metadata_commitment: MigrationDigestV1,
        custody_record_id: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        let copied_length =
            u64::try_from(copied_bytes.len()).map_err(|_| LiveSetV3Error::LengthOverflow)?;
        let copied_sha256 = MigrationDigestV1::digest_bytes(copied_bytes)?;
        Self::from_custody(
            source,
            copied_length,
            copied_sha256,
            metadata_commitment,
            custody_record_id,
        )
    }

    pub(crate) fn from_custody(
        source: &SourceCaseV3,
        copied_length: u64,
        copied_sha256: MigrationDigestV1,
        metadata_commitment: MigrationDigestV1,
        custody_record_id: MigrationDigestV1,
    ) -> Result<Self, LiveSetV3Error> {
        if source.payload_state() != LegacyPayloadStateV3::Present
            || copied_length != source.logical_length()
            || copied_sha256 != source.content_sha256()
            || metadata_commitment != source.metadata_commitment()
        {
            return Err(LiveSetV3Error::CopyMismatch);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.sealed-quarantine-entry.v3\0",
            &CborValue::Array(vec![
                source.identity().canonical_value(),
                CborValue::Unsigned(copied_length),
                copied_sha256.canonical_value(),
                metadata_commitment.canonical_value(),
                custody_record_id.canonical_value(),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_id: source.identity(),
            copied_length,
            copied_sha256,
            metadata_commitment,
            custody_record_id,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_id(&self) -> MigrationDigestV1 {
        self.source_case_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.identity.canonical_value(),
            self.source_case_id.canonical_value(),
            CborValue::Unsigned(self.copied_length),
            self.copied_sha256.canonical_value(),
            self.metadata_commitment.canonical_value(),
            self.custody_record_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedQuarantineManifestV3 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    custody_lease_id: MigrationDigestV1,
    expected_old_id: MigrationDigestV1,
    rows: Vec<SealedQuarantineEntryV3>,
}

impl SealedQuarantineManifestV3 {
    pub(crate) fn new(
        source_cases: &LegacySourceCaseManifestV3,
        classifications: &MigrationClassificationManifestV3,
        custody_lease_id: MigrationDigestV1,
        expected_old_id: MigrationDigestV1,
        mut rows: Vec<SealedQuarantineEntryV3>,
    ) -> Result<Self, LiveSetV3Error> {
        rows.sort_by_key(SealedQuarantineEntryV3::source_case_id);
        let expected = source_cases
            .rows()
            .iter()
            .filter(|row| row.payload_state() == LegacyPayloadStateV3::Present)
            .map(SourceCaseV3::identity)
            .collect::<Vec<_>>();
        let observed = rows
            .iter()
            .map(SealedQuarantineEntryV3::source_case_id)
            .collect::<Vec<_>>();
        if expected != observed
            || classifications.source_case_manifest_id() != source_cases.identity()
        {
            return Err(LiveSetV3Error::PartialCopy);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.sealed-quarantine-manifest.v3\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                classifications.identity().canonical_value(),
                custody_lease_id.canonical_value(),
                expected_old_id.canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(SealedQuarantineEntryV3::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            classification_manifest_id: classifications.identity(),
            custody_lease_id,
            expected_old_id,
            rows,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn expected_old_id(&self) -> MigrationDigestV1 {
        self.expected_old_id
    }

    pub fn rows(&self) -> &[SealedQuarantineEntryV3] {
        &self.rows
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRollbackAssessmentV3 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    loss_manifest_id: MigrationDigestV1,
    quarantine_manifest_id: MigrationDigestV1,
    expected_old_id: MigrationDigestV1,
}

impl LegacyRollbackAssessmentV3 {
    pub(crate) fn assess(
        source_cases: &LegacySourceCaseManifestV3,
        classifications: &MigrationClassificationManifestV3,
        losses: &UnavailablePreexistingLossManifestV3,
        quarantine: &SealedQuarantineManifestV3,
    ) -> Result<Self, LiveSetV3Error> {
        if classifications.source_case_manifest_id() != source_cases.identity()
            || losses.source_case_manifest_id != source_cases.identity()
            || quarantine.source_case_manifest_id != source_cases.identity()
            || quarantine.classification_manifest_id != classifications.identity()
        {
            return Err(LiveSetV3Error::RollbackIncomplete);
        }
        let present = source_cases
            .rows()
            .iter()
            .filter(|source| source.payload_state() == LegacyPayloadStateV3::Present)
            .map(SourceCaseV3::identity)
            .collect::<Vec<_>>();
        let quarantined = quarantine
            .rows()
            .iter()
            .map(SealedQuarantineEntryV3::source_case_id)
            .collect::<Vec<_>>();
        let unavailable = source_cases
            .rows()
            .iter()
            .filter(|source| {
                source.payload_state() == LegacyPayloadStateV3::UnavailablePreexistingLoss
            })
            .map(SourceCaseV3::identity)
            .collect::<Vec<_>>();
        let loss_rows = losses
            .rows()
            .iter()
            .map(UnavailablePreexistingLossV3::source_case_id)
            .collect::<Vec<_>>();
        if present != quarantined || unavailable != loss_rows {
            return Err(LiveSetV3Error::RollbackIncomplete);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.legacy-rollback-assessment.v3\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                classifications.identity().canonical_value(),
                losses.identity().canonical_value(),
                quarantine.identity().canonical_value(),
                quarantine.expected_old_id().canonical_value(),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            classification_manifest_id: classifications.identity(),
            loss_manifest_id: losses.identity(),
            quarantine_manifest_id: quarantine.identity(),
            expected_old_id: quarantine.expected_old_id(),
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRollbackAssessmentV4 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    loss_manifest_id: MigrationDigestV1,
    quarantine_manifest_id: MigrationDigestV1,
    expected_old_id: MigrationDigestV1,
}

impl LegacyRollbackAssessmentV4 {
    pub(crate) fn assess(
        source_cases: &LegacySourceCaseManifestV3,
        classifications: &MigrationClassificationManifestV3,
        losses: &UnavailablePreexistingLossManifestV4,
        quarantine: &SealedQuarantineManifestV3,
    ) -> Result<Self, LiveSetV3Error> {
        if classifications.source_case_manifest_id() != source_cases.identity()
            || losses.source_case_manifest_id != source_cases.identity()
            || losses.classification_manifest_id != classifications.identity()
            || quarantine.source_case_manifest_id != source_cases.identity()
            || quarantine.classification_manifest_id != classifications.identity()
        {
            return Err(LiveSetV3Error::RollbackIncomplete);
        }
        let present = source_cases
            .rows()
            .iter()
            .filter(|source| source.payload_state() == LegacyPayloadStateV3::Present)
            .map(SourceCaseV3::identity)
            .collect::<Vec<_>>();
        let quarantined = quarantine
            .rows()
            .iter()
            .map(SealedQuarantineEntryV3::source_case_id)
            .collect::<Vec<_>>();
        let unavailable = source_cases
            .rows()
            .iter()
            .filter(|source| {
                source.payload_state() == LegacyPayloadStateV3::UnavailablePreexistingLoss
            })
            .map(SourceCaseV3::identity)
            .collect::<Vec<_>>();
        let loss_rows = losses
            .rows()
            .iter()
            .map(UnavailablePreexistingLossV4::source_case_id)
            .collect::<Vec<_>>();
        if present != quarantined || unavailable != loss_rows {
            return Err(LiveSetV3Error::RollbackIncomplete);
        }
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.legacy-rollback-assessment.v4\0",
            &CborValue::Array(vec![
                source_cases.identity().canonical_value(),
                classifications.identity().canonical_value(),
                losses.identity().canonical_value(),
                quarantine.identity().canonical_value(),
                quarantine.expected_old_id().canonical_value(),
            ]),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            classification_manifest_id: classifications.identity(),
            loss_manifest_id: losses.identity(),
            quarantine_manifest_id: quarantine.identity(),
            expected_old_id: quarantine.expected_old_id(),
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LegacyQuarantineEpochBasisV3 {
    foundation_closure_id: MigrationDigestV1,
    persistence_receipt_id: MigrationDigestV1,
    release_id: MigrationDigestV1,
    store_generation_id: MigrationDigestV1,
    store_head_id: MigrationDigestV1,
    namespace_epoch: u64,
    trust_root_id: MigrationDigestV1,
    provider_id: MigrationDigestV1,
    mount_id: MigrationDigestV1,
    anchor_id: MigrationDigestV1,
    fence_id: MigrationDigestV1,
    candidate_commit_id: MigrationDigestV1,
    candidate_tree_id: MigrationDigestV1,
    protected_primary_boundary_id: MigrationDigestV1,
    rollback_plan_id: MigrationDigestV1,
    currentness_id: MigrationDigestV1,
    revocation_revision: u64,
}

impl LegacyQuarantineEpochBasisV3 {
    #[expect(
        clippy::too_many_arguments,
        reason = "final Stage-11 currentness binds the complete owner and physical tuple"
    )]
    pub(crate) fn from_final_owner_recheck(
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV3,
        quarantine: &SealedQuarantineManifestV3,
        rollback: &LegacyRollbackAssessmentV3,
        foundation_closure_id: MigrationDigestV1,
        persistence_receipt_id: MigrationDigestV1,
        release_id: MigrationDigestV1,
        store_generation_id: MigrationDigestV1,
        store_head_id: MigrationDigestV1,
        namespace_epoch: u64,
        trust_root_id: MigrationDigestV1,
        provider_id: MigrationDigestV1,
        mount_id: MigrationDigestV1,
        anchor_id: MigrationDigestV1,
        fence_id: MigrationDigestV1,
        candidate_commit_id: MigrationDigestV1,
        candidate_tree_id: MigrationDigestV1,
        protected_primary_boundary_id: MigrationDigestV1,
        revocation_revision: u64,
    ) -> Result<Self, LiveSetV3Error> {
        if namespace_epoch == 0
            || revocation_revision == 0
            || rollback.source_case_manifest_id != source_cases.identity()
            || rollback.classification_manifest_id != classifications.identity()
            || rollback.loss_manifest_id != losses.identity()
            || rollback.quarantine_manifest_id != quarantine.identity()
            || rollback.expected_old_id != quarantine.expected_old_id()
            || sightings.source_case_manifest_id() != source_cases.identity()
            || sightings.candidate_commit_id() != candidate_commit_id
            || sightings.candidate_tree_id() != candidate_tree_id
            || classifications.source_case_manifest_id() != source_cases.identity()
            || overlaps.source_case_manifest_id != source_cases.identity()
        {
            return Err(LiveSetV3Error::CurrentnessMismatch);
        }
        let rollback_plan_id = rollback.identity();
        let mut basis = Self {
            foundation_closure_id,
            persistence_receipt_id,
            release_id,
            store_generation_id,
            store_head_id,
            namespace_epoch,
            trust_root_id,
            provider_id,
            mount_id,
            anchor_id,
            fence_id,
            candidate_commit_id,
            candidate_tree_id,
            protected_primary_boundary_id,
            rollback_plan_id,
            currentness_id: rollback_plan_id,
            revocation_revision,
        };
        basis.currentness_id = basis.calculate_currentness(
            source_cases,
            sightings,
            classifications,
            overlaps,
            losses,
            quarantine,
        )?;
        Ok(basis)
    }

    fn calculate_currentness(
        &self,
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV3,
        quarantine: &SealedQuarantineManifestV3,
    ) -> Result<MigrationDigestV1, LiveSetV3Error> {
        let mut currentness_fields = vec![
            source_cases.identity().canonical_value(),
            sightings.identity().canonical_value(),
            classifications.identity().canonical_value(),
            overlaps.identity().canonical_value(),
            losses.identity().canonical_value(),
            quarantine.identity().canonical_value(),
            self.rollback_plan_id.canonical_value(),
        ];
        currentness_fields.extend(
            [
                self.foundation_closure_id,
                self.persistence_receipt_id,
                self.release_id,
                self.store_generation_id,
                self.store_head_id,
                self.trust_root_id,
                self.provider_id,
                self.mount_id,
                self.anchor_id,
                self.fence_id,
                self.candidate_commit_id,
                self.candidate_tree_id,
                self.protected_primary_boundary_id,
            ]
            .into_iter()
            .map(MigrationDigestV1::canonical_value),
        );
        currentness_fields.push(CborValue::Unsigned(self.namespace_epoch));
        currentness_fields.push(CborValue::Unsigned(self.revocation_revision));
        Ok(MigrationDigestV1::identify(
            b"maestro.migration.legacy-quarantine-final-currentness.v3\0",
            &CborValue::Array(currentness_fields),
        )?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyQuarantineEpochV3 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    sighting_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    overlap_manifest_id: MigrationDigestV1,
    loss_manifest_id: MigrationDigestV1,
    quarantine_manifest_id: MigrationDigestV1,
    basis: LegacyQuarantineEpochBasisV3,
}

impl LegacyQuarantineEpochV3 {
    #[expect(
        clippy::too_many_arguments,
        reason = "epoch finalization revalidates all six joined manifests plus rollback and currentness"
    )]
    pub(crate) fn finalize(
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV3,
        quarantine: &SealedQuarantineManifestV3,
        rollback: &LegacyRollbackAssessmentV3,
        basis: LegacyQuarantineEpochBasisV3,
    ) -> Result<Self, LiveSetV3Error> {
        let recomputed_currentness = basis.calculate_currentness(
            source_cases,
            sightings,
            classifications,
            overlaps,
            losses,
            quarantine,
        )?;
        if basis.namespace_epoch == 0
            || basis.revocation_revision == 0
            || sightings.source_case_manifest_id() != source_cases.identity()
            || classifications.source_case_manifest_id() != source_cases.identity()
            || overlaps.source_case_manifest_id != source_cases.identity()
            || losses.source_case_manifest_id != source_cases.identity()
            || quarantine.source_case_manifest_id != source_cases.identity()
            || quarantine.classification_manifest_id != classifications.identity()
            || rollback.source_case_manifest_id != source_cases.identity()
            || rollback.classification_manifest_id != classifications.identity()
            || rollback.loss_manifest_id != losses.identity()
            || rollback.quarantine_manifest_id != quarantine.identity()
            || rollback.expected_old_id != quarantine.expected_old_id()
            || rollback.identity() != basis.rollback_plan_id
            || sightings.candidate_commit_id() != basis.candidate_commit_id
            || sightings.candidate_tree_id() != basis.candidate_tree_id
            || basis.currentness_id != recomputed_currentness
        {
            return Err(LiveSetV3Error::CurrentnessMismatch);
        }
        let mut fields = vec![
            source_cases.identity().canonical_value(),
            sightings.identity().canonical_value(),
            classifications.identity().canonical_value(),
            overlaps.identity().canonical_value(),
            losses.identity().canonical_value(),
            quarantine.identity().canonical_value(),
        ];
        fields.extend(
            [
                basis.foundation_closure_id,
                basis.persistence_receipt_id,
                basis.release_id,
                basis.store_generation_id,
                basis.store_head_id,
                basis.trust_root_id,
                basis.provider_id,
                basis.mount_id,
                basis.anchor_id,
                basis.fence_id,
                basis.candidate_commit_id,
                basis.candidate_tree_id,
                basis.protected_primary_boundary_id,
                basis.rollback_plan_id,
                basis.currentness_id,
            ]
            .into_iter()
            .map(MigrationDigestV1::canonical_value),
        );
        fields.push(CborValue::Unsigned(basis.namespace_epoch));
        fields.push(CborValue::Unsigned(basis.revocation_revision));
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.legacy-quarantine-epoch.v3\0",
            &CborValue::Array(fields),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            sighting_manifest_id: sightings.identity(),
            classification_manifest_id: classifications.identity(),
            overlap_manifest_id: overlaps.identity(),
            loss_manifest_id: losses.identity(),
            quarantine_manifest_id: quarantine.identity(),
            basis,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_manifest_id(&self) -> MigrationDigestV1 {
        self.source_case_manifest_id
    }

    pub const fn sighting_manifest_id(&self) -> MigrationDigestV1 {
        self.sighting_manifest_id
    }

    pub const fn classification_manifest_id(&self) -> MigrationDigestV1 {
        self.classification_manifest_id
    }

    pub const fn overlap_manifest_id(&self) -> MigrationDigestV1 {
        self.overlap_manifest_id
    }

    pub const fn loss_manifest_id(&self) -> MigrationDigestV1 {
        self.loss_manifest_id
    }

    pub const fn quarantine_manifest_id(&self) -> MigrationDigestV1 {
        self.quarantine_manifest_id
    }

    pub const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.basis.foundation_closure_id
    }

    pub const fn persistence_receipt_id(&self) -> MigrationDigestV1 {
        self.basis.persistence_receipt_id
    }

    pub const fn release_id(&self) -> MigrationDigestV1 {
        self.basis.release_id
    }

    pub const fn store_generation_id(&self) -> MigrationDigestV1 {
        self.basis.store_generation_id
    }

    pub const fn store_head_id(&self) -> MigrationDigestV1 {
        self.basis.store_head_id
    }

    pub const fn namespace_epoch(&self) -> u64 {
        self.basis.namespace_epoch
    }

    pub const fn trust_root_id(&self) -> MigrationDigestV1 {
        self.basis.trust_root_id
    }

    pub const fn provider_id(&self) -> MigrationDigestV1 {
        self.basis.provider_id
    }

    pub const fn mount_id(&self) -> MigrationDigestV1 {
        self.basis.mount_id
    }

    pub const fn anchor_id(&self) -> MigrationDigestV1 {
        self.basis.anchor_id
    }

    pub const fn fence_id(&self) -> MigrationDigestV1 {
        self.basis.fence_id
    }

    pub const fn candidate_commit_id(&self) -> MigrationDigestV1 {
        self.basis.candidate_commit_id
    }

    pub const fn candidate_tree_id(&self) -> MigrationDigestV1 {
        self.basis.candidate_tree_id
    }

    pub const fn protected_primary_boundary_id(&self) -> MigrationDigestV1 {
        self.basis.protected_primary_boundary_id
    }

    pub const fn rollback_plan_id(&self) -> MigrationDigestV1 {
        self.basis.rollback_plan_id
    }

    pub const fn currentness_id(&self) -> MigrationDigestV1 {
        self.basis.currentness_id
    }

    pub const fn revocation_revision(&self) -> u64 {
        self.basis.revocation_revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LegacyQuarantineEpochBasisV4 {
    foundation_closure_id: MigrationDigestV1,
    persistence_receipt_id: MigrationDigestV1,
    release_id: MigrationDigestV1,
    store_generation_id: MigrationDigestV1,
    store_head_id: MigrationDigestV1,
    namespace_epoch: u64,
    trust_root_id: MigrationDigestV1,
    provider_id: MigrationDigestV1,
    mount_id: MigrationDigestV1,
    anchor_id: MigrationDigestV1,
    fence_id: MigrationDigestV1,
    candidate_commit_id: MigrationDigestV1,
    candidate_tree_id: MigrationDigestV1,
    protected_primary_boundary_id: MigrationDigestV1,
    rollback_plan_id: MigrationDigestV1,
    currentness_id: MigrationDigestV1,
    revocation_revision: u64,
}

impl LegacyQuarantineEpochBasisV4 {
    #[expect(
        clippy::too_many_arguments,
        reason = "V4 finality binds the complete V4 loss, owner, and physical tuple"
    )]
    pub(crate) fn from_final_owner_recheck(
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV4,
        quarantine: &SealedQuarantineManifestV3,
        rollback: &LegacyRollbackAssessmentV4,
        foundation_closure_id: MigrationDigestV1,
        persistence_receipt_id: MigrationDigestV1,
        release_id: MigrationDigestV1,
        store_generation_id: MigrationDigestV1,
        store_head_id: MigrationDigestV1,
        namespace_epoch: u64,
        trust_root_id: MigrationDigestV1,
        provider_id: MigrationDigestV1,
        mount_id: MigrationDigestV1,
        anchor_id: MigrationDigestV1,
        fence_id: MigrationDigestV1,
        candidate_commit_id: MigrationDigestV1,
        candidate_tree_id: MigrationDigestV1,
        protected_primary_boundary_id: MigrationDigestV1,
        revocation_revision: u64,
    ) -> Result<Self, LiveSetV3Error> {
        if namespace_epoch == 0
            || revocation_revision == 0
            || rollback.source_case_manifest_id != source_cases.identity()
            || rollback.classification_manifest_id != classifications.identity()
            || rollback.loss_manifest_id != losses.identity()
            || rollback.quarantine_manifest_id != quarantine.identity()
            || rollback.expected_old_id != quarantine.expected_old_id()
            || sightings.source_case_manifest_id() != source_cases.identity()
            || sightings.candidate_commit_id() != candidate_commit_id
            || sightings.candidate_tree_id() != candidate_tree_id
            || classifications.source_case_manifest_id() != source_cases.identity()
            || overlaps.source_case_manifest_id != source_cases.identity()
        {
            return Err(LiveSetV3Error::CurrentnessMismatch);
        }
        let rollback_plan_id = rollback.identity();
        let mut basis = Self {
            foundation_closure_id,
            persistence_receipt_id,
            release_id,
            store_generation_id,
            store_head_id,
            namespace_epoch,
            trust_root_id,
            provider_id,
            mount_id,
            anchor_id,
            fence_id,
            candidate_commit_id,
            candidate_tree_id,
            protected_primary_boundary_id,
            rollback_plan_id,
            currentness_id: rollback_plan_id,
            revocation_revision,
        };
        basis.currentness_id = basis.calculate_currentness(
            source_cases,
            sightings,
            classifications,
            overlaps,
            losses,
            quarantine,
        )?;
        Ok(basis)
    }

    fn calculate_currentness(
        &self,
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV4,
        quarantine: &SealedQuarantineManifestV3,
    ) -> Result<MigrationDigestV1, LiveSetV3Error> {
        let mut fields = vec![
            source_cases.identity().canonical_value(),
            sightings.identity().canonical_value(),
            classifications.identity().canonical_value(),
            overlaps.identity().canonical_value(),
            losses.identity().canonical_value(),
            quarantine.identity().canonical_value(),
            self.rollback_plan_id.canonical_value(),
        ];
        fields.extend(
            [
                self.foundation_closure_id,
                self.persistence_receipt_id,
                self.release_id,
                self.store_generation_id,
                self.store_head_id,
                self.trust_root_id,
                self.provider_id,
                self.mount_id,
                self.anchor_id,
                self.fence_id,
                self.candidate_commit_id,
                self.candidate_tree_id,
                self.protected_primary_boundary_id,
            ]
            .into_iter()
            .map(MigrationDigestV1::canonical_value),
        );
        fields.push(CborValue::Unsigned(self.namespace_epoch));
        fields.push(CborValue::Unsigned(self.revocation_revision));
        MigrationDigestV1::identify(
            b"maestro.migration.legacy-quarantine-final-currentness.v4\0",
            &CborValue::Array(fields),
        )
        .map_err(Into::into)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyQuarantineEpochV4 {
    identity: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    sighting_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    overlap_manifest_id: MigrationDigestV1,
    loss_manifest_id: MigrationDigestV1,
    quarantine_manifest_id: MigrationDigestV1,
    basis: LegacyQuarantineEpochBasisV4,
}

impl LegacyQuarantineEpochV4 {
    #[expect(
        clippy::too_many_arguments,
        reason = "V4 epoch finalization revalidates every joined manifest and currentness field"
    )]
    pub(crate) fn finalize(
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV4,
        quarantine: &SealedQuarantineManifestV3,
        rollback: &LegacyRollbackAssessmentV4,
        basis: LegacyQuarantineEpochBasisV4,
    ) -> Result<Self, LiveSetV3Error> {
        let recomputed_currentness = basis.calculate_currentness(
            source_cases,
            sightings,
            classifications,
            overlaps,
            losses,
            quarantine,
        )?;
        if basis.namespace_epoch == 0
            || basis.revocation_revision == 0
            || sightings.source_case_manifest_id() != source_cases.identity()
            || classifications.source_case_manifest_id() != source_cases.identity()
            || overlaps.source_case_manifest_id != source_cases.identity()
            || losses.source_case_manifest_id != source_cases.identity()
            || losses.classification_manifest_id != classifications.identity()
            || quarantine.source_case_manifest_id != source_cases.identity()
            || quarantine.classification_manifest_id != classifications.identity()
            || rollback.source_case_manifest_id != source_cases.identity()
            || rollback.classification_manifest_id != classifications.identity()
            || rollback.loss_manifest_id != losses.identity()
            || rollback.quarantine_manifest_id != quarantine.identity()
            || rollback.expected_old_id != quarantine.expected_old_id()
            || rollback.identity() != basis.rollback_plan_id
            || sightings.candidate_commit_id() != basis.candidate_commit_id
            || sightings.candidate_tree_id() != basis.candidate_tree_id
            || basis.currentness_id != recomputed_currentness
        {
            return Err(LiveSetV3Error::CurrentnessMismatch);
        }
        let mut fields = vec![
            source_cases.identity().canonical_value(),
            sightings.identity().canonical_value(),
            classifications.identity().canonical_value(),
            overlaps.identity().canonical_value(),
            losses.identity().canonical_value(),
            quarantine.identity().canonical_value(),
        ];
        fields.extend(
            [
                basis.foundation_closure_id,
                basis.persistence_receipt_id,
                basis.release_id,
                basis.store_generation_id,
                basis.store_head_id,
                basis.trust_root_id,
                basis.provider_id,
                basis.mount_id,
                basis.anchor_id,
                basis.fence_id,
                basis.candidate_commit_id,
                basis.candidate_tree_id,
                basis.protected_primary_boundary_id,
                basis.rollback_plan_id,
                basis.currentness_id,
            ]
            .into_iter()
            .map(MigrationDigestV1::canonical_value),
        );
        fields.push(CborValue::Unsigned(basis.namespace_epoch));
        fields.push(CborValue::Unsigned(basis.revocation_revision));
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.legacy-quarantine-epoch.v4\0",
            &CborValue::Array(fields),
        )?;
        Ok(Self {
            identity,
            source_case_manifest_id: source_cases.identity(),
            sighting_manifest_id: sightings.identity(),
            classification_manifest_id: classifications.identity(),
            overlap_manifest_id: overlaps.identity(),
            loss_manifest_id: losses.identity(),
            quarantine_manifest_id: quarantine.identity(),
            basis,
        })
    }

    pub const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub const fn source_case_manifest_id(&self) -> MigrationDigestV1 {
        self.source_case_manifest_id
    }

    pub const fn sighting_manifest_id(&self) -> MigrationDigestV1 {
        self.sighting_manifest_id
    }

    pub const fn classification_manifest_id(&self) -> MigrationDigestV1 {
        self.classification_manifest_id
    }

    pub const fn overlap_manifest_id(&self) -> MigrationDigestV1 {
        self.overlap_manifest_id
    }

    pub const fn loss_manifest_id(&self) -> MigrationDigestV1 {
        self.loss_manifest_id
    }

    pub const fn quarantine_manifest_id(&self) -> MigrationDigestV1 {
        self.quarantine_manifest_id
    }

    pub const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.basis.foundation_closure_id
    }

    pub const fn persistence_receipt_id(&self) -> MigrationDigestV1 {
        self.basis.persistence_receipt_id
    }

    pub const fn release_id(&self) -> MigrationDigestV1 {
        self.basis.release_id
    }

    pub const fn store_generation_id(&self) -> MigrationDigestV1 {
        self.basis.store_generation_id
    }

    pub const fn store_head_id(&self) -> MigrationDigestV1 {
        self.basis.store_head_id
    }

    pub const fn namespace_epoch(&self) -> u64 {
        self.basis.namespace_epoch
    }

    pub const fn trust_root_id(&self) -> MigrationDigestV1 {
        self.basis.trust_root_id
    }

    pub const fn provider_id(&self) -> MigrationDigestV1 {
        self.basis.provider_id
    }

    pub const fn mount_id(&self) -> MigrationDigestV1 {
        self.basis.mount_id
    }

    pub const fn anchor_id(&self) -> MigrationDigestV1 {
        self.basis.anchor_id
    }

    pub const fn fence_id(&self) -> MigrationDigestV1 {
        self.basis.fence_id
    }

    pub const fn candidate_commit_id(&self) -> MigrationDigestV1 {
        self.basis.candidate_commit_id
    }

    pub const fn candidate_tree_id(&self) -> MigrationDigestV1 {
        self.basis.candidate_tree_id
    }

    pub const fn protected_primary_boundary_id(&self) -> MigrationDigestV1 {
        self.basis.protected_primary_boundary_id
    }

    pub const fn rollback_plan_id(&self) -> MigrationDigestV1 {
        self.basis.rollback_plan_id
    }

    pub const fn currentness_id(&self) -> MigrationDigestV1 {
        self.basis.currentness_id
    }

    pub const fn revocation_revision(&self) -> u64 {
        self.basis.revocation_revision
    }
}

fn validate_locator(locator: &[u8]) -> Result<(), LiveSetV3Error> {
    if locator.is_empty() || locator.len() > MAX_LOCATOR_BYTES_V3 || locator.contains(&0) {
        return Err(LiveSetV3Error::InvalidLocator);
    }
    Ok(())
}

fn validate_materialized_identity(
    value: &CborValue,
    expected: MigrationDigestV1,
) -> Result<(), LiveSetV3Error> {
    let CborValue::Array(fields) = value else {
        return Err(LiveSetV3Error::InvalidFoundationMaterialization);
    };
    if fields.first() != Some(&expected.canonical_value()) {
        return Err(LiveSetV3Error::InvalidFoundationMaterialization);
    }
    Ok(())
}

fn migration_digest_field(value: &CborValue) -> Result<MigrationDigestV1, LiveSetV3Error> {
    let CborValue::Bytes(bytes) = value else {
        return Err(LiveSetV3Error::InvalidLossAudit);
    };
    let digest =
        <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| LiveSetV3Error::InvalidLossAudit)?;
    Ok(MigrationDigestV1::from_digest(digest)?)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the opaque Foundation case is checked against every semantic field Migration may consume"
)]
fn validate_materialized_source_case(
    value: &CborValue,
    identity: MigrationDigestV1,
    membership: &CborValue,
    foundation_invocation: MigrationDigestV1,
    payload_state: LegacyPayloadStateV3,
    logical_length: u64,
    content_sha256: MigrationDigestV1,
    metadata_commitment: MigrationDigestV1,
) -> Result<(), LiveSetV3Error> {
    let expected = CborValue::Array(vec![
        identity.canonical_value(),
        membership.clone(),
        foundation_invocation.canonical_value(),
        CborValue::Unsigned(payload_state.tag()),
        CborValue::Unsigned(logical_length),
        content_sha256.canonical_value(),
        metadata_commitment.canonical_value(),
    ]);
    if value != &expected {
        return Err(LiveSetV3Error::InvalidFoundationMaterialization);
    }
    Ok(())
}

fn validate_row_count(count: usize) -> Result<(), LiveSetV3Error> {
    if count > MAX_ROWS_V3 {
        return Err(LiveSetV3Error::RowLimitExceeded);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum LiveSetV3Error {
    #[error("V3 lossless display locator is empty, contains NUL, or exceeds the finite limit")]
    InvalidLocator,
    #[error("V3 source metadata does not match its owner-minted membership")]
    MetadataMismatch,
    #[error("V3 source-case manifest is empty, duplicated, mixed-invocation, or unordered")]
    InvalidSourceManifest,
    #[error("V3 sighting has no exact matched bytes")]
    InvalidSighting,
    #[error("V3 sighting manifest contains an unknown, duplicate, or foreign-policy row")]
    InvalidSightingManifest,
    #[error("V3 Migration disposition contradicts source availability")]
    InvalidDisposition,
    #[error("V3 classification does not cover every source case exactly once")]
    IncompleteClassification,
    #[error("V3 protected-primary overlap is not a bijective owner-declared pair")]
    InvalidOverlapPair,
    #[error("V3 unavailable-preexisting-loss evidence is missing or inconsistent")]
    InvalidLossEvidence,
    #[error("V3 descriptor-serviced source copy differs from the admitted source case")]
    CopyMismatch,
    #[error("V3 quarantine does not contain one exact copy for every present source case")]
    PartialCopy,
    #[error("V3 rollback assessment does not cover every present preimage or admitted loss")]
    RollbackIncomplete,
    #[error("V3 currentness or joined manifest identity changed before epoch finalization")]
    CurrentnessMismatch,
    #[error("V3 manifest exceeds the finite row limit")]
    RowLimitExceeded,
    #[error("V3 source byte length exceeds u64")]
    LengthOverflow,
    #[error("Foundation materialized source or overlap encoding is malformed or inconsistent")]
    InvalidFoundationMaterialization,
    #[error("V4 unavailable-preexisting-loss audit is malformed, stale, or tampered")]
    InvalidLossAudit,
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &[u8]) -> MigrationDigestV1 {
        MigrationDigestV1::digest_bytes(label).expect("non-zero test digest")
    }

    fn loss_audit_fixture() -> UnavailablePreexistingLossV4 {
        let source_case_id = digest(b"audit-source");
        let owner_snapshot_id = digest(b"audit-snapshot");
        let issuer_id = digest(b"audit-issuer");
        let historical_tuple_id = digest(b"audit-history");
        let owner_current_tuple_id = digest(b"audit-current-tuple");
        let source_provenance_id = digest(b"audit-provenance");
        let owner_admission_id = digest(b"audit-admission");
        let owner_currentness_id = digest(b"audit-currentness");
        let foundation_loss_receipt_id = digest(b"audit-foundation-receipt");
        let validation_invocation_id = digest(b"audit-validation");
        let pass_a_absence_id = digest(b"audit-pass-a");
        let pass_b_absence_id = digest(b"audit-pass-b");
        let identity = MigrationDigestV1::identify(
            b"maestro.migration.unavailable-preexisting-loss.v4\0",
            &CborValue::Array(
                [
                    source_case_id,
                    owner_snapshot_id,
                    issuer_id,
                    historical_tuple_id,
                    owner_current_tuple_id,
                    source_provenance_id,
                    owner_admission_id,
                    owner_currentness_id,
                    foundation_loss_receipt_id,
                    validation_invocation_id,
                    pass_a_absence_id,
                    pass_b_absence_id,
                ]
                .into_iter()
                .map(MigrationDigestV1::canonical_value)
                .collect(),
            ),
        )
        .expect("loss identity");
        UnavailablePreexistingLossV4 {
            identity,
            source_case_id,
            owner_snapshot_id,
            issuer_id,
            historical_tuple_id,
            owner_current_tuple_id,
            source_provenance_id,
            owner_admission_id,
            owner_currentness_id,
            foundation_loss_receipt_id,
            validation_invocation_id,
            pass_a_absence_id,
            pass_b_absence_id,
        }
    }

    #[test]
    fn v4_loss_audit_survives_reload_and_rejects_tamper_or_stale_currentness() {
        let loss = loss_audit_fixture();
        let bytes = loss.encode_canonical_audit().expect("encode audit");
        let reloaded =
            UnavailablePreexistingLossV4::decode_canonical_audit(&bytes, &loss.audit_currentness())
                .expect("decode current audit");
        assert_eq!(reloaded, loss);

        let mut tampered = bytes.clone();
        let last = tampered.last_mut().expect("non-empty audit");
        *last ^= 1;
        assert!(matches!(
            UnavailablePreexistingLossV4::decode_canonical_audit(
                &tampered,
                &loss.audit_currentness()
            ),
            Err(LiveSetV3Error::InvalidLossAudit)
                | Err(LiveSetV3Error::CanonicalCbor(_))
                | Err(LiveSetV3Error::Identity(_))
        ));

        let mut stale = loss.audit_currentness();
        stale.owner_currentness_id = digest(b"stale-currentness");
        assert!(matches!(
            UnavailablePreexistingLossV4::decode_canonical_audit(&bytes, &stale),
            Err(LiveSetV3Error::InvalidLossAudit)
        ));
    }

    fn present_source_for(
        owner: LegacyOwnerDomainV3,
        object: MigrationDigestV1,
        bytes: &[u8],
    ) -> SourceCaseV3 {
        let content = MigrationDigestV1::digest_bytes(bytes).expect("content digest");
        let metadata = MigrationDigestV1::identify(
            b"maestro.migration.source-metadata.v3\0",
            &CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(bytes.len() as u64),
                object.canonical_value(),
                content.canonical_value(),
            ]),
        )
        .expect("metadata");
        let membership = MembershipKeyV3::from_foundation(
            owner,
            digest(b"root"),
            b"/owner/root/object".to_vec(),
            digest(b"resolved"),
            object,
            LegacyNodeKindV3::RegularFile,
            metadata,
            digest(b"owner-currentness"),
            digest(b"owner-attestation"),
        )
        .expect("membership");
        SourceCaseV3::from_foundation(
            membership,
            digest(b"foundation-invocation"),
            LegacyPayloadStateV3::Present,
            bytes.len() as u64,
            content,
            metadata,
        )
        .expect("source")
    }

    fn present_source(bytes: &[u8]) -> SourceCaseV3 {
        present_source_for(LegacyOwnerDomainV3::Repository, digest(b"object"), bytes)
    }

    #[test]
    fn protected_primary_overlap_requires_same_object_and_exact_bytes() {
        let owner = present_source_for(
            LegacyOwnerDomainV3::Repository,
            digest(b"owner-object"),
            b"owner bytes",
        );
        let primary = present_source_for(
            LegacyOwnerDomainV3::ProtectedPrimary,
            digest(b"primary-object"),
            b"primary bytes",
        );
        assert!(matches!(
            ProtectedPrimaryOverlapPairV1::from_foundation(
                &owner,
                &primary,
                digest(b"mount"),
                digest(b"mount"),
                digest(b"provider"),
                digest(b"provider"),
            ),
            Err(LiveSetV3Error::InvalidOverlapPair)
        ));
    }

    #[test]
    fn epoch_binds_every_typed_manifest_and_currentness_identity() {
        let bytes = b"exact legacy payload";
        let source = present_source(bytes);
        let sources = LegacySourceCaseManifestV3::new(
            source.foundation_invocation(),
            digest(b"admitted-set"),
            vec![source.clone()],
        )
        .expect("source manifest");
        let policy = digest(b"policy");
        let sightings = Stage12SightingManifestV2::new(
            policy,
            digest(b"candidate-commit"),
            digest(b"candidate-tree"),
            &sources,
            Vec::new(),
        )
        .expect("sighting manifest");
        let classification = MigrationClassificationV3::new(
            &source,
            MigrationDispositionV3::QuarantineOnly,
            digest(b"reason"),
        )
        .expect("classification");
        let classifications =
            MigrationClassificationManifestV3::new(&sources, policy, vec![classification])
                .expect("classification manifest");
        let overlaps =
            DeclaredOverlapManifestV2::new(&sources, Vec::new()).expect("overlap manifest");
        let losses =
            UnavailablePreexistingLossManifestV3::new(&sources, &classifications, Vec::new())
                .expect("empty loss manifest");
        let quarantine_entry = SealedQuarantineEntryV3::from_copy(
            &source,
            bytes,
            source.metadata_commitment(),
            digest(b"custody-record"),
        )
        .expect("quarantine entry");
        let quarantine = SealedQuarantineManifestV3::new(
            &sources,
            &classifications,
            digest(b"custody-lease"),
            digest(b"expected-old"),
            vec![quarantine_entry],
        )
        .expect("quarantine manifest");
        let rollback =
            LegacyRollbackAssessmentV3::assess(&sources, &classifications, &losses, &quarantine)
                .expect("rollback assessment");
        let basis = LegacyQuarantineEpochBasisV3::from_final_owner_recheck(
            &sources,
            &sightings,
            &classifications,
            &overlaps,
            &losses,
            &quarantine,
            &rollback,
            digest(b"foundation-closure"),
            digest(b"persistence-receipt"),
            digest(b"release"),
            digest(b"store-generation"),
            digest(b"store-head"),
            7,
            digest(b"trust-root"),
            digest(b"provider"),
            digest(b"mount"),
            digest(b"anchor"),
            digest(b"fence"),
            sightings.candidate_commit_id(),
            sightings.candidate_tree_id(),
            digest(b"primary-boundary"),
            11,
        )
        .expect("final currentness");
        let mut tampered_basis = basis;
        tampered_basis.currentness_id = digest(b"tampered-currentness");
        assert!(matches!(
            LegacyQuarantineEpochV3::finalize(
                &sources,
                &sightings,
                &classifications,
                &overlaps,
                &losses,
                &quarantine,
                &rollback,
                tampered_basis,
            ),
            Err(LiveSetV3Error::CurrentnessMismatch)
        ));
        let epoch = LegacyQuarantineEpochV3::finalize(
            &sources,
            &sightings,
            &classifications,
            &overlaps,
            &losses,
            &quarantine,
            &rollback,
            basis,
        )
        .expect("epoch");

        assert_eq!(epoch.source_case_manifest_id(), sources.identity());
        assert_eq!(epoch.sighting_manifest_id(), sightings.identity());
        assert_eq!(
            epoch.classification_manifest_id(),
            classifications.identity()
        );
        assert_eq!(epoch.overlap_manifest_id(), overlaps.identity());
        assert_eq!(epoch.loss_manifest_id(), losses.identity());
        assert_eq!(epoch.quarantine_manifest_id(), quarantine.identity());
        assert_eq!(epoch.trust_root_id(), basis.trust_root_id);
        assert_eq!(epoch.revocation_revision(), 11);
        assert_eq!(epoch.rollback_plan_id(), rollback.identity());
        assert_ne!(epoch.currentness_id(), digest(b"currentness-placeholder"));
    }

    #[test]
    fn unavailable_source_refuses_missing_independent_loss_evidence() {
        let present = present_source(b"historical bytes");
        let missing = SourceCaseV3::from_foundation(
            present.membership().clone(),
            present.foundation_invocation(),
            LegacyPayloadStateV3::UnavailablePreexistingLoss,
            present.logical_length(),
            present.content_sha256(),
            present.metadata_commitment(),
        )
        .expect("missing source");
        let sources = LegacySourceCaseManifestV3::new(
            missing.foundation_invocation(),
            digest(b"admitted-set"),
            vec![missing.clone()],
        )
        .expect("source manifest");
        let classification = MigrationClassificationV3::new(
            &missing,
            MigrationDispositionV3::UnavailablePreexistingLoss,
            digest(b"reason"),
        )
        .expect("classification");
        let classifications = MigrationClassificationManifestV3::new(
            &sources,
            digest(b"policy"),
            vec![classification],
        )
        .expect("classifications");

        assert!(matches!(
            UnavailablePreexistingLossManifestV3::new(&sources, &classifications, Vec::new()),
            Err(LiveSetV3Error::InvalidLossEvidence)
        ));
    }
}
