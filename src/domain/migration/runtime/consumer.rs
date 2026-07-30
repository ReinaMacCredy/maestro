use std::collections::BTreeSet;

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

use super::{MigrationDigestV1, MigrationIdentityErrorV1, NormalizedLocatorV1};

const CONSUMER_CLOSURE_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.consumer-closure.v1\0";
const CONSUMER_RECORD_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.consumer.v1\0";
const CONSUMER_CENSUS_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.consumer-census.v1\0";
const PROTOCOL_CLOSURE_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.protocol-closure.v1\0";
const MAX_CONSUMER_ROWS_V1: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConsumerSubjectV1 {
    LegacySource,
    CurrentTarget,
}

impl ConsumerSubjectV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::LegacySource => 1,
            Self::CurrentTarget => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConsumerGenerationV1 {
    LegacyV1,
    CurrentVNext,
    Unknown,
}

impl ConsumerGenerationV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::LegacyV1 => 1,
            Self::CurrentVNext => 2,
            Self::Unknown => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConsumerAccessV1 {
    ActiveRuntime,
    SealedMigrationReader,
    SealedAuditReader,
    PreAcceptRollbackReader,
    ProtectedRetentionHold,
}

impl ConsumerAccessV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::ActiveRuntime => 1,
            Self::SealedMigrationReader => 2,
            Self::SealedAuditReader => 3,
            Self::PreAcceptRollbackReader => 4,
            Self::ProtectedRetentionHold => 5,
        }
    }

    const fn is_sealed_reader(self) -> bool {
        matches!(
            self,
            Self::SealedMigrationReader | Self::SealedAuditReader | Self::PreAcceptRollbackReader
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationProtocolClosureV1 {
    association_schema_id: MigrationDigestV1,
    active_head_schema_id: MigrationDigestV1,
    pre_store_seal_schema_id: MigrationDigestV1,
    finality_edge_manifest_id: MigrationDigestV1,
    schema_read_write_set_id: MigrationDigestV1,
    writer_protocol_epoch_id: MigrationDigestV1,
    migration_epoch_id: MigrationDigestV1,
    release_id: Option<MigrationDigestV1>,
    id: MigrationDigestV1,
}

impl MigrationProtocolClosureV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "client compatibility binds the complete frozen protocol closure"
    )]
    pub fn new(
        association_schema_id: MigrationDigestV1,
        active_head_schema_id: MigrationDigestV1,
        pre_store_seal_schema_id: MigrationDigestV1,
        finality_edge_manifest_id: MigrationDigestV1,
        schema_read_write_set_id: MigrationDigestV1,
        writer_protocol_epoch_id: MigrationDigestV1,
        migration_epoch_id: MigrationDigestV1,
        release_id: Option<MigrationDigestV1>,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let required = [
            association_schema_id,
            active_head_schema_id,
            pre_store_seal_schema_id,
            finality_edge_manifest_id,
            schema_read_write_set_id,
            writer_protocol_epoch_id,
            migration_epoch_id,
        ];
        if required.iter().any(|id| id.as_bytes() == &[0; 32])
            || required.iter().collect::<BTreeSet<_>>().len() != required.len()
        {
            return Err(ConsumerClosureErrorV1::InvalidProtocolClosure);
        }
        let value = CborValue::Array(
            required
                .into_iter()
                .map(MigrationDigestV1::canonical_value)
                .chain(std::iter::once(CborValue::optional(
                    release_id.map(MigrationDigestV1::canonical_value),
                )))
                .collect(),
        );
        let id = MigrationDigestV1::identify(PROTOCOL_CLOSURE_DOMAIN_V1, &value)?;
        Ok(Self {
            association_schema_id,
            active_head_schema_id,
            pre_store_seal_schema_id,
            finality_edge_manifest_id,
            schema_read_write_set_id,
            writer_protocol_epoch_id,
            migration_epoch_id,
            release_id,
            id,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn release_id(&self) -> Option<MigrationDigestV1> {
        self.release_id
    }

    pub const fn schema_read_write_set_id(&self) -> MigrationDigestV1 {
        self.schema_read_write_set_id
    }

    pub const fn writer_protocol_epoch_id(&self) -> MigrationDigestV1 {
        self.writer_protocol_epoch_id
    }

    pub const fn migration_epoch_id(&self) -> MigrationDigestV1 {
        self.migration_epoch_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.association_schema_id.canonical_value(),
            self.active_head_schema_id.canonical_value(),
            self.pre_store_seal_schema_id.canonical_value(),
            self.finality_edge_manifest_id.canonical_value(),
            self.schema_read_write_set_id.canonical_value(),
            self.writer_protocol_epoch_id.canonical_value(),
            self.migration_epoch_id.canonical_value(),
            CborValue::optional(self.release_id.map(MigrationDigestV1::canonical_value)),
            self.id.canonical_value(),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientRefusalReasonV1 {
    OldProtocol,
    MixedProtocol,
    UnknownProtocol,
    ReleaseMismatch,
}

impl ClientRefusalReasonV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::OldProtocol => 1,
            Self::MixedProtocol => 2,
            Self::UnknownProtocol => 3,
            Self::ReleaseMismatch => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAdmissionV1 {
    ExactCurrent,
    OpaqueSealedOnly,
    RefusedBeforeCurrentness(ClientRefusalReasonV1),
}

impl ClientAdmissionV1 {
    pub const fn may_report_currentness(self) -> bool {
        matches!(self, Self::ExactCurrent)
    }

    pub const fn may_mutate(self) -> bool {
        matches!(self, Self::ExactCurrent)
    }

    fn canonical_value(self) -> CborValue {
        match self {
            Self::ExactCurrent => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::OpaqueSealedOnly => CborValue::Array(vec![CborValue::Unsigned(2)]),
            Self::RefusedBeforeCurrentness(reason) => CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Unsigned(reason.tag()),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerRecordV1 {
    id: MigrationDigestV1,
    locator: NormalizedLocatorV1,
    subject: ConsumerSubjectV1,
    generation: ConsumerGenerationV1,
    access: ConsumerAccessV1,
    autoloading: bool,
    bearer: bool,
    protocol: Option<MigrationProtocolClosureV1>,
}

impl ConsumerRecordV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "consumer closure binds the complete access posture"
    )]
    pub fn new(
        locator: NormalizedLocatorV1,
        subject: ConsumerSubjectV1,
        generation: ConsumerGenerationV1,
        access: ConsumerAccessV1,
        autoloading: bool,
        bearer: bool,
        protocol: Option<MigrationProtocolClosureV1>,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        if access.is_sealed_reader()
            && (subject != ConsumerSubjectV1::LegacySource || autoloading || bearer)
        {
            return Err(ConsumerClosureErrorV1::InvalidSealedReader);
        }
        if access == ConsumerAccessV1::ProtectedRetentionHold
            && (autoloading || bearer || protocol.is_some())
        {
            return Err(ConsumerClosureErrorV1::InvalidRetentionHold);
        }
        let id = MigrationDigestV1::identify(
            CONSUMER_RECORD_DOMAIN_V1,
            &CborValue::Array(vec![
                locator.canonical_value(),
                CborValue::Unsigned(subject.tag()),
                CborValue::Unsigned(generation.tag()),
                CborValue::Unsigned(access.tag()),
                CborValue::Bool(autoloading),
                CborValue::Bool(bearer),
                CborValue::optional(
                    protocol
                        .as_ref()
                        .map(MigrationProtocolClosureV1::canonical_value),
                ),
            ]),
        )?;
        Ok(Self {
            id,
            locator,
            subject,
            generation,
            access,
            autoloading,
            bearer,
            protocol,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn subject(&self) -> ConsumerSubjectV1 {
        self.subject
    }

    pub const fn generation(&self) -> ConsumerGenerationV1 {
        self.generation
    }

    pub const fn access(&self) -> ConsumerAccessV1 {
        self.access
    }

    pub const fn protocol(&self) -> Option<&MigrationProtocolClosureV1> {
        self.protocol.as_ref()
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.id.canonical_value(),
            self.locator.canonical_value(),
            CborValue::Unsigned(self.subject.tag()),
            CborValue::Unsigned(self.generation.tag()),
            CborValue::Unsigned(self.access.tag()),
            CborValue::Bool(self.autoloading),
            CborValue::Bool(self.bearer),
            CborValue::optional(
                self.protocol
                    .as_ref()
                    .map(MigrationProtocolClosureV1::canonical_value),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsumerCensusResolutionV1 {
    Observed(Box<ConsumerRecordV1>),
    Removed {
        last_consumer_id: MigrationDigestV1,
        removal_receipt_id: MigrationDigestV1,
    },
}

impl ConsumerCensusResolutionV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Observed(consumer) => {
                CborValue::Array(vec![CborValue::Unsigned(1), consumer.canonical_value()])
            }
            Self::Removed {
                last_consumer_id,
                removal_receipt_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                last_consumer_id.canonical_value(),
                removal_receipt_id.canonical_value(),
            ]),
        }
    }

    fn observed(&self) -> Option<&ConsumerRecordV1> {
        match self {
            Self::Observed(consumer) => Some(consumer),
            Self::Removed { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerCensusEntryV1 {
    source_row_id: MigrationDigestV1,
    resolution: ConsumerCensusResolutionV1,
}

impl ConsumerCensusEntryV1 {
    #[cfg(test)]
    pub fn observed(source_row_id: MigrationDigestV1, consumer: ConsumerRecordV1) -> Self {
        Self {
            source_row_id,
            resolution: ConsumerCensusResolutionV1::Observed(Box::new(consumer)),
        }
    }

    #[cfg(test)]
    pub fn removed(
        source_row_id: MigrationDigestV1,
        last_consumer_id: MigrationDigestV1,
        removal_receipt_id: MigrationDigestV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        if source_row_id.as_bytes() == &[0; 32]
            || last_consumer_id.as_bytes() == &[0; 32]
            || removal_receipt_id.as_bytes() == &[0; 32]
            || source_row_id == last_consumer_id
            || source_row_id == removal_receipt_id
            || last_consumer_id == removal_receipt_id
        {
            return Err(ConsumerClosureErrorV1::InvalidCensusEntry);
        }
        Ok(Self {
            source_row_id,
            resolution: ConsumerCensusResolutionV1::Removed {
                last_consumer_id,
                removal_receipt_id,
            },
        })
    }

    pub const fn source_row_id(&self) -> MigrationDigestV1 {
        self.source_row_id
    }

    pub const fn resolution(&self) -> &ConsumerCensusResolutionV1 {
        &self.resolution
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.source_row_id.canonical_value(),
            self.resolution.canonical_value(),
        ])
    }
}

#[cfg(test)]
pub trait Stage9Stage10ConsumerCensusAdapterV1 {
    fn authoritative_census_facts(
        &self,
    ) -> Result<(usize, [MigrationDigestV1; 3], Vec<ConsumerCensusEntryV1>), ConsumerClosureErrorV1>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeConsumerCensusV1 {
    source_manifest_id: MigrationDigestV1,
    owner_snapshot_id: MigrationDigestV1,
    closure_attestation_id: MigrationDigestV1,
    entries: Vec<ConsumerCensusEntryV1>,
    id: MigrationDigestV1,
}

impl AuthoritativeConsumerCensusV1 {
    fn from_owner_snapshot(
        expected_member_count: usize,
        source_manifest_id: MigrationDigestV1,
        owner_snapshot_id: MigrationDigestV1,
        closure_attestation_id: MigrationDigestV1,
        mut entries: Vec<ConsumerCensusEntryV1>,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let authorities = [
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
        ];
        if expected_member_count == 0
            || expected_member_count > MAX_CONSUMER_ROWS_V1
            || entries.len() != expected_member_count
            || authorities.iter().any(|id| id.as_bytes() == &[0; 32])
            || authorities.iter().collect::<BTreeSet<_>>().len() != authorities.len()
            || entries.iter().any(|entry| {
                entry.source_row_id.as_bytes() == &[0; 32]
                    || entry
                        .resolution
                        .observed()
                        .is_some_and(|consumer| consumer.id() == entry.source_row_id)
            })
        {
            return Err(ConsumerClosureErrorV1::InvalidAuthoritativeCensus);
        }
        entries.sort_by_key(ConsumerCensusEntryV1::source_row_id);
        if entries
            .windows(2)
            .any(|pair| pair[0].source_row_id == pair[1].source_row_id)
        {
            return Err(ConsumerClosureErrorV1::DuplicateCensusMember);
        }
        let observed_ids = entries
            .iter()
            .filter_map(|entry| entry.resolution.observed().map(ConsumerRecordV1::id))
            .collect::<Vec<_>>();
        if observed_ids.iter().collect::<BTreeSet<_>>().len() != observed_ids.len() {
            return Err(ConsumerClosureErrorV1::DuplicateConsumer);
        }
        let value = CborValue::Array(vec![
            source_manifest_id.canonical_value(),
            owner_snapshot_id.canonical_value(),
            closure_attestation_id.canonical_value(),
            CborValue::Unsigned(
                u64::try_from(expected_member_count)
                    .map_err(|_| ConsumerClosureErrorV1::ConsumerCountExceeded)?,
            ),
            CborValue::Array(
                entries
                    .iter()
                    .map(ConsumerCensusEntryV1::canonical_value)
                    .collect(),
            ),
        ]);
        let id = MigrationDigestV1::identify(CONSUMER_CENSUS_DOMAIN_V1, &value)?;
        Ok(Self {
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
            entries,
            id,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn source_manifest_id(&self) -> MigrationDigestV1 {
        self.source_manifest_id
    }

    pub const fn owner_snapshot_id(&self) -> MigrationDigestV1 {
        self.owner_snapshot_id
    }

    pub const fn closure_attestation_id(&self) -> MigrationDigestV1 {
        self.closure_attestation_id
    }

    pub fn entries(&self) -> &[ConsumerCensusEntryV1] {
        &self.entries
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.source_manifest_id.canonical_value(),
            self.owner_snapshot_id.canonical_value(),
            self.closure_attestation_id.canonical_value(),
            CborValue::Array(
                self.entries
                    .iter()
                    .map(ConsumerCensusEntryV1::canonical_value)
                    .collect(),
            ),
            self.id.canonical_value(),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsumerGateStageV1 {
    BeforeSemanticCurrentness,
    ProtectedRetention,
    PhysicalPruning,
}

impl ConsumerGateStageV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::BeforeSemanticCurrentness => 1,
            Self::ProtectedRetention => 2,
            Self::PhysicalPruning => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PruneSubjectV1 {
    installation_domain_id: MigrationDigestV1,
    inventory_id: MigrationDigestV1,
    consumer_census_id: MigrationDigestV1,
    protocol_closure_id: MigrationDigestV1,
    id: MigrationDigestV1,
}

impl PruneSubjectV1 {
    pub fn new(
        installation_domain_id: MigrationDigestV1,
        inventory_id: MigrationDigestV1,
        consumer_census_id: MigrationDigestV1,
        protocol_closure_id: MigrationDigestV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let fields = [
            installation_domain_id,
            inventory_id,
            consumer_census_id,
            protocol_closure_id,
        ];
        if fields.iter().any(|id| id.as_bytes() == &[0; 32])
            || fields.into_iter().collect::<BTreeSet<_>>().len() != fields.len()
        {
            return Err(ConsumerClosureErrorV1::InvalidPrunePrerequisites);
        }
        let id = MigrationDigestV1::identify(
            b"maestro.vnext.migration.prune-subject.v1\0",
            &CborValue::Array(
                fields
                    .into_iter()
                    .map(MigrationDigestV1::canonical_value)
                    .collect(),
            ),
        )?;
        Ok(Self {
            installation_domain_id,
            inventory_id,
            consumer_census_id,
            protocol_closure_id,
            id,
        })
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.installation_domain_id.canonical_value(),
            self.inventory_id.canonical_value(),
            self.consumer_census_id.canonical_value(),
            self.protocol_closure_id.canonical_value(),
            self.id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PruneCurrentnessV1 {
    store_generation_id: MigrationDigestV1,
    authority_epoch_id: MigrationDigestV1,
    consumer_snapshot_id: MigrationDigestV1,
    retention_revision: u64,
    id: MigrationDigestV1,
}

impl PruneCurrentnessV1 {
    pub fn new(
        store_generation_id: MigrationDigestV1,
        authority_epoch_id: MigrationDigestV1,
        consumer_snapshot_id: MigrationDigestV1,
        retention_revision: u64,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let fields = [
            store_generation_id,
            authority_epoch_id,
            consumer_snapshot_id,
        ];
        if retention_revision == 0
            || fields.iter().any(|id| id.as_bytes() == &[0; 32])
            || fields.into_iter().collect::<BTreeSet<_>>().len() != fields.len()
        {
            return Err(ConsumerClosureErrorV1::InvalidPrunePrerequisites);
        }
        let id = MigrationDigestV1::identify(
            b"maestro.vnext.migration.prune-currentness.v1\0",
            &CborValue::Array(vec![
                store_generation_id.canonical_value(),
                authority_epoch_id.canonical_value(),
                consumer_snapshot_id.canonical_value(),
                CborValue::Unsigned(retention_revision),
            ]),
        )?;
        Ok(Self {
            store_generation_id,
            authority_epoch_id,
            consumer_snapshot_id,
            retention_revision,
            id,
        })
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.store_generation_id.canonical_value(),
            self.authority_epoch_id.canonical_value(),
            self.consumer_snapshot_id.canonical_value(),
            CborValue::Unsigned(self.retention_revision),
            self.id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnerMintedPruneReceiptV1 {
    subject: PruneSubjectV1,
    currentness: PruneCurrentnessV1,
    proof_id: MigrationDigestV1,
    id: MigrationDigestV1,
}

impl OwnerMintedPruneReceiptV1 {
    fn mint(
        domain: &'static [u8],
        subject: PruneSubjectV1,
        currentness: PruneCurrentnessV1,
        proof_id: MigrationDigestV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        if proof_id.as_bytes() == &[0; 32] || proof_id == subject.id || proof_id == currentness.id {
            return Err(ConsumerClosureErrorV1::InvalidPrunePrerequisites);
        }
        let id = MigrationDigestV1::identify(
            domain,
            &CborValue::Array(vec![
                subject.canonical_value(),
                currentness.canonical_value(),
                proof_id.canonical_value(),
            ]),
        )?;
        Ok(Self {
            subject,
            currentness,
            proof_id,
            id,
        })
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.subject.canonical_value(),
            self.currentness.canonical_value(),
            self.proof_id.canonical_value(),
            self.id.canonical_value(),
        ])
    }
}

macro_rules! owner_prune_receipt {
    ($name:ident, $constructor:ident, $domain:literal) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(OwnerMintedPruneReceiptV1);

        impl $name {
            pub(crate) fn $constructor(
                subject: PruneSubjectV1,
                currentness: PruneCurrentnessV1,
                proof_id: MigrationDigestV1,
            ) -> Result<Self, ConsumerClosureErrorV1> {
                OwnerMintedPruneReceiptV1::mint($domain, subject, currentness, proof_id).map(Self)
            }
        }
    };
}

owner_prune_receipt!(
    CustodyProofReceiptV1,
    from_installation_owner,
    b"maestro.vnext.migration.prune-custody-receipt.v1\0"
);
owner_prune_receipt!(
    RemovalAuthorityReceiptV1,
    from_authority_owner,
    b"maestro.vnext.migration.prune-authority-receipt.v1\0"
);
owner_prune_receipt!(
    RollbackSafetyReceiptV1,
    from_persistence_owner,
    b"maestro.vnext.migration.prune-rollback-safety-receipt.v1\0"
);
owner_prune_receipt!(
    ErasureSafetyReceiptV1,
    from_persistence_owner,
    b"maestro.vnext.migration.prune-erasure-safety-receipt.v1\0"
);
owner_prune_receipt!(
    LegacyRemovalAuthorizationReceiptV1,
    from_migration_owner,
    b"maestro.vnext.migration.legacy-removal-authorization-receipt.v1\0"
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrunePrerequisitesV1 {
    custody: Option<CustodyProofReceiptV1>,
    authority: Option<RemovalAuthorityReceiptV1>,
    rollback_safety: Option<RollbackSafetyReceiptV1>,
    erasure_safety: Option<ErasureSafetyReceiptV1>,
    removal_authorization: Option<LegacyRemovalAuthorizationReceiptV1>,
}

impl PrunePrerequisitesV1 {
    pub(crate) fn from_owner_receipts(
        custody: CustodyProofReceiptV1,
        authority: RemovalAuthorityReceiptV1,
        rollback_safety: RollbackSafetyReceiptV1,
        erasure_safety: ErasureSafetyReceiptV1,
        removal_authorization: LegacyRemovalAuthorizationReceiptV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let bindings = [
            &custody.0,
            &authority.0,
            &rollback_safety.0,
            &erasure_safety.0,
            &removal_authorization.0,
        ];
        let first = bindings[0];
        if bindings.iter().any(|binding| {
            binding.subject != first.subject || binding.currentness != first.currentness
        }) || bindings
            .iter()
            .map(|binding| binding.id)
            .collect::<BTreeSet<_>>()
            .len()
            != bindings.len()
        {
            return Err(ConsumerClosureErrorV1::InvalidPrunePrerequisites);
        }
        Ok(Self {
            custody: Some(custody),
            authority: Some(authority),
            rollback_safety: Some(rollback_safety),
            erasure_safety: Some(erasure_safety),
            removal_authorization: Some(removal_authorization),
        })
    }

    pub const fn blocked() -> Self {
        Self {
            custody: None,
            authority: None,
            rollback_safety: None,
            erasure_safety: None,
            removal_authorization: None,
        }
    }

    pub const fn complete(&self) -> bool {
        self.custody.is_some()
            && self.authority.is_some()
            && self.rollback_safety.is_some()
            && self.erasure_safety.is_some()
            && self.removal_authorization.is_some()
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::optional(
                self.custody
                    .as_ref()
                    .map(|receipt| receipt.0.canonical_value()),
            ),
            CborValue::optional(
                self.authority
                    .as_ref()
                    .map(|receipt| receipt.0.canonical_value()),
            ),
            CborValue::optional(
                self.rollback_safety
                    .as_ref()
                    .map(|receipt| receipt.0.canonical_value()),
            ),
            CborValue::optional(
                self.erasure_safety
                    .as_ref()
                    .map(|receipt| receipt.0.canonical_value()),
            ),
            CborValue::optional(
                self.removal_authorization
                    .as_ref()
                    .map(|receipt| receipt.0.canonical_value()),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerClosureV1 {
    stage: ConsumerGateStageV1,
    protocol: MigrationProtocolClosureV1,
    census: AuthoritativeConsumerCensusV1,
    consumers: Vec<ConsumerRecordV1>,
    admissions: Vec<(MigrationDigestV1, ClientAdmissionV1)>,
    blocking_consumer_ids: Vec<MigrationDigestV1>,
    prune_prerequisites: PrunePrerequisitesV1,
    id: MigrationDigestV1,
}

impl ConsumerClosureV1 {
    pub(in crate::domain::migration) fn evaluate_owner_snapshot_parts(
        stage: ConsumerGateStageV1,
        protocol: MigrationProtocolClosureV1,
        expected_member_count: usize,
        authority_ids: [MigrationDigestV1; 3],
        entries: Vec<ConsumerCensusEntryV1>,
        prune_prerequisites: PrunePrerequisitesV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let [
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
        ] = authority_ids;
        let census = AuthoritativeConsumerCensusV1::from_owner_snapshot(
            expected_member_count,
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
            entries,
        )?;
        Self::evaluate(stage, protocol, census, prune_prerequisites)
    }

    pub(in crate::domain) fn evaluate_installation_snapshot(
        stage: ConsumerGateStageV1,
        protocol: MigrationProtocolClosureV1,
        snapshot: crate::domain::installation::consumer_snapshot::InstallationMigrationConsumerSnapshotV1,
        prune_prerequisites: PrunePrerequisitesV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let (
            expected_member_count,
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
            entries,
        ) = snapshot.into_parts();
        Self::evaluate_owner_snapshot_parts(
            stage,
            protocol,
            expected_member_count,
            [
                source_manifest_id,
                owner_snapshot_id,
                closure_attestation_id,
            ],
            entries,
            prune_prerequisites,
        )
    }

    #[cfg(test)]
    pub fn evaluate_from_adapter<A: Stage9Stage10ConsumerCensusAdapterV1>(
        stage: ConsumerGateStageV1,
        protocol: MigrationProtocolClosureV1,
        adapter: &A,
        prune_prerequisites: PrunePrerequisitesV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let (expected_member_count, authority_ids, entries) =
            adapter.authoritative_census_facts()?;
        let [
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
        ] = authority_ids;
        let census = AuthoritativeConsumerCensusV1::from_owner_snapshot(
            expected_member_count,
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
            entries,
        )?;
        Self::evaluate(stage, protocol, census, prune_prerequisites)
    }

    fn evaluate(
        stage: ConsumerGateStageV1,
        protocol: MigrationProtocolClosureV1,
        census: AuthoritativeConsumerCensusV1,
        prune_prerequisites: PrunePrerequisitesV1,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let mut consumers = census
            .entries()
            .iter()
            .filter_map(|entry| entry.resolution().observed().cloned())
            .collect::<Vec<_>>();
        consumers.sort_by_key(ConsumerRecordV1::id);
        if consumers.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(ConsumerClosureErrorV1::DuplicateConsumer);
        }
        let mut admissions = Vec::with_capacity(consumers.len());
        let mut blocking = Vec::new();
        for consumer in &consumers {
            let admission = client_admission(consumer, &protocol);
            let blocks = match stage {
                ConsumerGateStageV1::BeforeSemanticCurrentness => {
                    (consumer.access == ConsumerAccessV1::ActiveRuntime
                        && (consumer.subject == ConsumerSubjectV1::LegacySource
                            || admission != ClientAdmissionV1::ExactCurrent))
                        || matches!(admission, ClientAdmissionV1::RefusedBeforeCurrentness(_))
                }
                ConsumerGateStageV1::ProtectedRetention => match consumer.subject {
                    ConsumerSubjectV1::LegacySource => {
                        !consumer.access.is_sealed_reader()
                            && consumer.access != ConsumerAccessV1::ProtectedRetentionHold
                    }
                    ConsumerSubjectV1::CurrentTarget => {
                        consumer.access == ConsumerAccessV1::ActiveRuntime
                            && admission != ClientAdmissionV1::ExactCurrent
                    }
                },
                ConsumerGateStageV1::PhysicalPruning => {
                    consumer.subject == ConsumerSubjectV1::LegacySource
                        || (consumer.access == ConsumerAccessV1::ActiveRuntime
                            && admission != ClientAdmissionV1::ExactCurrent)
                }
            };
            admissions.push((consumer.id, admission));
            if blocks {
                blocking.push(consumer.id);
            }
        }
        if stage == ConsumerGateStageV1::PhysicalPruning && !prune_prerequisites.complete() {
            blocking.push(MigrationDigestV1::identify(
                b"maestro.vnext.migration.prune-prerequisites-blocked.v1\0",
                &prune_prerequisites.canonical_value(),
            )?);
        }
        blocking.sort();
        let value = CborValue::Array(vec![
            CborValue::Unsigned(stage.tag()),
            protocol.canonical_value(),
            census.canonical_value(),
            CborValue::Array(
                consumers
                    .iter()
                    .map(ConsumerRecordV1::canonical_value)
                    .collect(),
            ),
            CborValue::Array(
                admissions
                    .iter()
                    .map(|(id, admission)| {
                        CborValue::Array(vec![id.canonical_value(), admission.canonical_value()])
                    })
                    .collect(),
            ),
            CborValue::Array(
                blocking
                    .iter()
                    .copied()
                    .map(MigrationDigestV1::canonical_value)
                    .collect(),
            ),
            prune_prerequisites.canonical_value(),
        ]);
        let id = MigrationDigestV1::identify(CONSUMER_CLOSURE_DOMAIN_V1, &value)?;
        Ok(Self {
            stage,
            protocol,
            census,
            consumers,
            admissions,
            blocking_consumer_ids: blocking,
            prune_prerequisites,
            id,
        })
    }

    pub const fn stage(&self) -> ConsumerGateStageV1 {
        self.stage
    }

    pub const fn protocol(&self) -> &MigrationProtocolClosureV1 {
        &self.protocol
    }

    pub const fn census(&self) -> &AuthoritativeConsumerCensusV1 {
        &self.census
    }

    pub fn consumers(&self) -> &[ConsumerRecordV1] {
        &self.consumers
    }

    pub fn admissions(&self) -> &[(MigrationDigestV1, ClientAdmissionV1)] {
        &self.admissions
    }

    pub fn blocking_consumer_ids(&self) -> &[MigrationDigestV1] {
        &self.blocking_consumer_ids
    }

    pub const fn gate_passed(&self) -> bool {
        self.blocking_consumer_ids.is_empty()
    }

    pub const fn prune_prerequisites(&self) -> &PrunePrerequisitesV1 {
        &self.prune_prerequisites
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }
}

fn client_admission(
    consumer: &ConsumerRecordV1,
    expected: &MigrationProtocolClosureV1,
) -> ClientAdmissionV1 {
    if consumer.access.is_sealed_reader()
        || consumer.access == ConsumerAccessV1::ProtectedRetentionHold
    {
        return ClientAdmissionV1::OpaqueSealedOnly;
    }
    match (consumer.generation, consumer.protocol.as_ref()) {
        (ConsumerGenerationV1::LegacyV1, _) => {
            ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::OldProtocol)
        }
        (ConsumerGenerationV1::Unknown, _) | (_, None) => {
            ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::UnknownProtocol)
        }
        (ConsumerGenerationV1::CurrentVNext, Some(observed)) if observed == expected => {
            ClientAdmissionV1::ExactCurrent
        }
        (ConsumerGenerationV1::CurrentVNext, Some(observed))
            if observed.release_id() != expected.release_id() =>
        {
            ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::ReleaseMismatch)
        }
        (ConsumerGenerationV1::CurrentVNext, Some(_)) => {
            ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::MixedProtocol)
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ConsumerClosureErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("migration protocol closure contains zero or duplicate required identities")]
    InvalidProtocolClosure,
    #[error("sealed readers must be legacy-source, nonautoloading, and non-bearer")]
    InvalidSealedReader,
    #[error("retention holds cannot autoload, bear authority, or interpret a client protocol")]
    InvalidRetentionHold,
    #[error("consumer count exceeds the finite v1 bound")]
    ConsumerCountExceeded,
    #[error("consumer appears more than once in the closure")]
    DuplicateConsumer,
    #[error("authoritative consumer census is empty, incomplete, oversized, or unbound")]
    InvalidAuthoritativeCensus,
    #[error("authoritative consumer census contains the same source member more than once")]
    DuplicateCensusMember,
    #[error("consumer census entry contains a zero, aliased, or contradictory identity")]
    InvalidCensusEntry,
    #[error("prune prerequisites contain a zero or duplicate proof identity")]
    InvalidPrunePrerequisites,
}

#[cfg(test)]
mod cohort_observation_tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    use crate::domain::identity::{ContractRootIdV1, SchemaIdV1};
    use crate::domain::migration::runtime::{
        ByteTotalInventoryV1, CancellationClassificationV1, ClassificationSetV1, ClientAdmissionV1,
        ClientRefusalReasonV1, ConsumerAccessV1, ConsumerCensusEntryV1, ConsumerClosureV1,
        ConsumerGateStageV1, ConsumerGenerationV1, ConsumerRecordV1, ConsumerSubjectV1,
        CutoverAcceptanceV1, DeclaredRootV1, DeterministicIdentityMapV1, EffectCrossingV1,
        IdentityMapEntryV1, IdentityMappingBasisV1, InventoryDomainV1, InventoryNodeKindV1,
        InventoryPayloadV1, InventoryRowV1, MigrationDigestV1, MigrationDispositionV1,
        MigrationProtocolClosureV1, NormalizedLocatorV1, ProtectedV1RollbackOutcomeV1,
        PrunePrerequisitesV1, QuarantineEntryV1, RollbackAssessmentV1, RollbackDispositionV1,
        RollbackRestoreErrorV1, SealedQuarantineManifestV1, SourceClassificationV1,
        restore_protected_exact_v1,
    };
    use crate::domain::persistence::{
        StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreObjectV1, StoreRoleV1,
        StoreStateV1, StoreV1,
    };
    use crate::foundation::core::deterministic_cbor::CborValue;
    use crate::operations::migration::import_inactive_store;

    const INSTANCE_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/vnext/stage11/migration_instances.v1.jsonl");
    const COHORT_FIXTURE: &[u8] = include_bytes!(
        "../../../../tools/vnext_contracts/final_chain/fixtures/migration-cohorts.v1.json"
    );
    const COHORT_OBSERVATION_SCHEMA: &str = "maestro.external.vnext-final-cohort-observation.v1";
    const ROUTE_OBSERVATION_SCHEMA: &str =
        "maestro.external.vnext-final-cohort-route-observation.v1";
    const EXPECTED_INSTANCE_DIGEST: &str =
        "b4f869a16328f0ccb640a5c3cc9a7c6bd9b1d295c614eb82fddfb680496498d5";
    const EXPECTED_ROW_COUNTS: [(&str, usize); 3] =
        [("c325", 325), ("e204", 204), ("skill_ledger", 35)];
    static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug)]
    struct TemporaryRoot(PathBuf);

    impl TemporaryRoot {
        fn new() -> Self {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary =
                fs::canonicalize(env::temp_dir()).expect("canonical temporary directory");
            let path = temporary.join(format!(
                "maestro-stage11-cohort-{}-{sequence}",
                std::process::id()
            ));
            if path.exists() {
                fs::remove_dir_all(&path).expect("remove stale Stage11 cohort root");
            }
            fs::create_dir(&path).expect("create Stage11 cohort root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove Stage11 cohort root");
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct FrozenRows {
        digest: MigrationDigestV1,
        counts: BTreeMap<String, usize>,
        row_count: usize,
    }

    impl FrozenRows {
        fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
            let digest = MigrationDigestV1::digest_bytes(bytes).map_err(|_| "fixture digest")?;
            if digest.render_hex() != EXPECTED_INSTANCE_DIGEST {
                return Err("fixture identity differs");
            }
            let mut lines = bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty());
            let header: Value =
                serde_json::from_slice(lines.next().ok_or("fixture header missing")?)
                    .map_err(|_| "fixture header invalid")?;
            if header["schema"] != "maestro.vnext.stage11.migration-instances.v1" {
                return Err("fixture schema differs");
            }
            let declared = header["row_counts"]
                .as_object()
                .ok_or("fixture row counts missing")?;
            let mut counts = BTreeMap::<String, usize>::new();
            let mut ordinals = BTreeMap::<String, usize>::new();
            for raw in lines {
                let row: Value = serde_json::from_slice(raw).map_err(|_| "fixture row invalid")?;
                let family = row["family"]
                    .as_str()
                    .ok_or("fixture family missing")?
                    .to_string();
                let next = counts.get(&family).copied().unwrap_or(0) + 1;
                if row["ordinal"].as_u64() != Some(next as u64) || row.get("row").is_none() {
                    return Err("fixture row coverage differs");
                }
                counts.insert(family.clone(), next);
                ordinals.insert(family, next);
            }
            let expected = EXPECTED_ROW_COUNTS
                .into_iter()
                .map(|(family, count)| (family.to_string(), count))
                .collect::<BTreeMap<_, _>>();
            if counts != expected
                || declared.len() != expected.len()
                || expected.iter().any(|(family, count)| {
                    declared.get(family).and_then(Value::as_u64) != Some(*count as u64)
                })
                || ordinals != expected
            {
                return Err("fixture family closure differs");
            }
            let row_count = counts.values().sum();
            Ok(Self {
                digest,
                counts,
                row_count,
            })
        }

        fn typed_binding(&self) -> String {
            format!(
                "rows=c325:{},e204:{},skill_ledger:{};total={};fixture=sha256:{}",
                self.counts["c325"],
                self.counts["e204"],
                self.counts["skill_ledger"],
                self.row_count,
                self.digest
            )
        }
    }

    fn digest(label: &[u8], fixture: &[u8]) -> MigrationDigestV1 {
        let mut bytes = Vec::with_capacity(label.len() + fixture.len() + 1);
        bytes.extend_from_slice(label);
        bytes.push(0);
        bytes.extend_from_slice(fixture);
        MigrationDigestV1::digest_bytes(&bytes).expect("fixture-derived nonzero digest")
    }

    fn locator(value: &str) -> NormalizedLocatorV1 {
        NormalizedLocatorV1::new(value.as_bytes().to_vec()).expect("normalized cohort locator")
    }

    fn protocol(fixture: &[u8]) -> MigrationProtocolClosureV1 {
        MigrationProtocolClosureV1::new(
            digest(b"association-schema", fixture),
            digest(b"active-head-schema", fixture),
            digest(b"pre-store-seal-schema", fixture),
            digest(b"finality-edge-manifest", fixture),
            digest(b"schema-read-write-set", fixture),
            digest(b"writer-protocol-epoch", fixture),
            digest(b"migration-epoch", fixture),
            Some(digest(b"release", fixture)),
        )
        .expect("fixture-derived protocol closure")
    }

    fn consumer_closure(
        fixture: &[u8],
        record: ConsumerRecordV1,
        expected: MigrationProtocolClosureV1,
    ) -> ConsumerClosureV1 {
        ConsumerClosureV1::evaluate_owner_snapshot_parts(
            ConsumerGateStageV1::BeforeSemanticCurrentness,
            expected,
            1,
            [
                digest(b"consumer-source-manifest", fixture),
                digest(b"consumer-owner-snapshot", fixture),
                digest(b"consumer-closure-attestation", fixture),
            ],
            vec![ConsumerCensusEntryV1::observed(
                digest(b"consumer-observation", fixture),
                record,
            )],
            PrunePrerequisitesV1::blocked(),
        )
        .expect("typed consumer closure")
    }

    fn old_reader_route(rows: &FrozenRows, fixture: &[u8]) -> String {
        let expected = protocol(fixture);
        let sealed = ConsumerRecordV1::new(
            locator("/stage11/cohort/old-reader"),
            ConsumerSubjectV1::LegacySource,
            ConsumerGenerationV1::LegacyV1,
            ConsumerAccessV1::SealedMigrationReader,
            false,
            false,
            None,
        )
        .expect("sealed old reader");
        let closure = consumer_closure(fixture, sealed, expected.clone());
        assert!(closure.gate_passed());
        assert_eq!(
            closure.admissions(),
            &[(
                closure.consumers()[0].id(),
                ClientAdmissionV1::OpaqueSealedOnly
            )]
        );

        let active_old = ConsumerRecordV1::new(
            locator("/stage11/cohort/old-reader-active-mutant"),
            ConsumerSubjectV1::LegacySource,
            ConsumerGenerationV1::LegacyV1,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            None,
        )
        .expect("active old-reader mutant");
        let refused = consumer_closure(fixture, active_old, expected);
        assert!(!refused.gate_passed());
        assert!(matches!(
            refused.admissions(),
            [(
                _,
                ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::OldProtocol)
            )]
        ));
        format!("opaque_sealed_only;{}", rows.typed_binding())
    }

    fn new_reader_route(rows: &FrozenRows, fixture: &[u8]) -> (String, ConsumerClosureV1) {
        let expected = protocol(fixture);
        let current = ConsumerRecordV1::new(
            locator("/stage11/cohort/new-reader"),
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::CurrentVNext,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            Some(expected.clone()),
        )
        .expect("exact current reader");
        let closure = consumer_closure(fixture, current, expected.clone());
        assert!(closure.gate_passed());
        assert_eq!(
            closure.admissions(),
            &[(closure.consumers()[0].id(), ClientAdmissionV1::ExactCurrent)]
        );

        let mut mutant = fixture.to_vec();
        let last = mutant.last_mut().expect("nonempty fixture");
        *last ^= 1;
        let mixed = ConsumerRecordV1::new(
            locator("/stage11/cohort/new-reader-protocol-mutant"),
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::CurrentVNext,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            Some(protocol(&mutant)),
        )
        .expect("mixed protocol mutant");
        let refused = consumer_closure(fixture, mixed, expected);
        assert!(!refused.gate_passed());
        assert!(matches!(
            refused.admissions(),
            [(
                _,
                ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::ReleaseMismatch)
            )]
        ));
        (format!("exact_current;{}", rows.typed_binding()), closure)
    }

    struct WriterOutcome {
        typed_result: String,
        request_id: MigrationDigestV1,
    }

    fn writer_commit_route(
        rows: &FrozenRows,
        fixture: &[u8],
        consumers: &ConsumerClosureV1,
        temporary: &TemporaryRoot,
    ) -> WriterOutcome {
        let root = DeclaredRootV1::new(
            locator("/stage11/frozen"),
            locator("/stage11/frozen"),
            InventoryDomainV1::Repository,
            false,
        )
        .expect("fixture root");
        let row = InventoryRowV1::new(
            root.id(),
            locator("/stage11/frozen/migration_instances.v1.jsonl"),
            locator("/stage11/frozen/migration_instances.v1.jsonl"),
            InventoryDomainV1::Repository,
            InventoryNodeKindV1::RegularFile,
            InventoryPayloadV1::from_bytes(fixture).expect("fixture payload"),
            digest(b"fixture-metadata", fixture),
        )
        .expect("fixture inventory row");
        let inventory =
            ByteTotalInventoryV1::new(vec![root], vec![row.clone()]).expect("fixture inventory");

        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"stage11-cohort-writer")
            .expect("writer domain");
        let compatibility =
            StoreCompatibilityV1::stage0_successor().expect("frozen Store compatibility");
        let object = StoreObjectV1::new(
            SchemaIdV1::parse(&format!("sha256:{}", digest(b"schema", fixture)))
                .expect("fixture schema identity"),
            CborValue::Bytes(fixture.to_vec()),
            vec![],
        )
        .expect("fixture Store object");
        let mut source = StoreV1::create(temporary.path().join("writer-source"), domain.clone())
            .expect("source Store");
        source.put_object(&object).expect("persist fixture object");
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            ContractRootIdV1::parse(&format!("sha256:{}", digest(b"contract-root", fixture)))
                .expect("fixture Contract Root"),
            compatibility,
            vec![object.id()],
        )
        .expect("source Generation");
        source
            .publish_generation(&generation, None)
            .expect("publish fixture Generation");
        let backup = source.seal_export().expect("sealed fixture export");

        let target_id = MigrationDigestV1::from_digest(object.id().into_bytes())
            .expect("fixture target identity");
        let classifications = ClassificationSetV1::new(
            &inventory,
            vec![
                SourceClassificationV1::new(
                    row.source_id(),
                    MigrationDispositionV1::OpaquePreserved,
                    digest(b"writer-classification", fixture),
                    Some(target_id),
                    None,
                    false,
                    CancellationClassificationV1::NotCancellationLike,
                )
                .expect("writer classification"),
            ],
        )
        .expect("writer classification set");
        let target_map = DeterministicIdentityMapV1::new(
            &classifications,
            vec![
                IdentityMapEntryV1::new(
                    row.source_id(),
                    target_id,
                    IdentityMappingBasisV1::HistoricalOpaque {
                        preservation_proof_id: digest(b"preservation-proof", fixture),
                    },
                )
                .expect("writer identity map row"),
            ],
        )
        .expect("writer identity map");
        let quarantine = SealedQuarantineManifestV1::new(
            &inventory,
            &classifications,
            locator("/stage11/quarantine"),
            vec![],
        )
        .expect("empty writer quarantine");
        let request = crate::domain::migration::runtime::InactiveStoreImportRequestV1::new(
            &inventory,
            &classifications,
            &target_map,
            &quarantine,
            consumers,
            backup.canonical_bytes(),
            object.id(),
        )
        .expect("inactive writer request");

        let mut corrupted = backup.canonical_bytes().to_vec();
        corrupted[0] ^= 1;
        let mut refused = StoreV1::create(temporary.path().join("writer-refused"), domain.clone())
            .expect("refused Store");
        assert!(import_inactive_store(&mut refused, &request, &corrupted).is_err());
        assert_eq!(
            refused.state().expect("refused Store state").0,
            StoreStateV1::Inactive
        );
        assert!(refused.active_head().expect("refused Store head").is_none());

        let mut destination = StoreV1::create(temporary.path().join("writer-destination"), domain)
            .expect("destination Store");
        let receipt = import_inactive_store(&mut destination, &request, backup.canonical_bytes())
            .expect("inactive writer commit");
        assert_eq!(
            destination.state().expect("destination state").0,
            StoreStateV1::Inactive
        );
        assert!(
            destination
                .active_head()
                .expect("destination head")
                .is_none()
        );
        let restored = destination
            .read_object(object.id())
            .expect("read committed fixture object");
        assert_eq!(restored, object);
        assert_eq!(restored.value(), &CborValue::Bytes(fixture.to_vec()));
        assert!(!receipt.activated());
        assert!(!receipt.claims_currentness());
        assert_eq!(
            receipt.candidate_root_id().as_bytes(),
            object.id().as_bytes()
        );
        let receipt_digest =
            MigrationDigestV1::digest_bytes(receipt.canonical_bytes()).expect("receipt digest");
        WriterOutcome {
            typed_result: format!(
                "inactive_store_commit;{};receipt=sha256:{receipt_digest}",
                rows.typed_binding()
            ),
            request_id: request.id(),
        }
    }

    fn rollback_route(rows: &FrozenRows, fixture: &[u8], attempt: MigrationDigestV1) -> String {
        let root = DeclaredRootV1::new(
            locator("/stage11/rollback-source"),
            locator("/stage11/rollback-source"),
            InventoryDomainV1::Repository,
            false,
        )
        .expect("rollback root");
        let row = InventoryRowV1::new(
            root.id(),
            locator("/stage11/rollback-source/migration_instances.v1.jsonl"),
            locator("/stage11/rollback-source/migration_instances.v1.jsonl"),
            InventoryDomainV1::Repository,
            InventoryNodeKindV1::RegularFile,
            InventoryPayloadV1::from_bytes(fixture).expect("rollback payload"),
            digest(b"rollback-metadata", fixture),
        )
        .expect("rollback inventory row");
        let inventory =
            ByteTotalInventoryV1::new(vec![root], vec![row.clone()]).expect("rollback inventory");
        let entry = QuarantineEntryV1::new(
            &row,
            fixture.to_vec(),
            digest(b"rollback-reason", fixture),
            digest(b"rollback-recovery", fixture),
        )
        .expect("sealed rollback entry");
        let classifications = ClassificationSetV1::new(
            &inventory,
            vec![
                SourceClassificationV1::new(
                    row.source_id(),
                    MigrationDispositionV1::Quarantined,
                    entry.reason_id(),
                    None,
                    Some(entry.id()),
                    false,
                    CancellationClassificationV1::NotCancellationLike,
                )
                .expect("rollback classification"),
            ],
        )
        .expect("rollback classification set");
        SealedQuarantineManifestV1::new(
            &inventory,
            &classifications,
            locator("/stage11/rollback-quarantine"),
            vec![entry.clone()],
        )
        .expect("sealed rollback manifest");

        let eligible = RollbackAssessmentV1::assess_cutover_observation(
            attempt,
            Some(attempt),
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::ProvenNotCrossed,
        )
        .expect("eligible rollback assessment");
        let restored = restore_protected_exact_v1(&eligible, &entry, fixture)
            .expect("protected exact-v1 restore");
        let ProtectedV1RollbackOutcomeV1::Restored {
            source_id,
            source_sha256,
            bytes,
        } = restored
        else {
            panic!("eligible rollback was refused");
        };
        assert_eq!(source_id, row.source_id());
        assert_eq!(source_sha256, rows.digest);
        assert_eq!(bytes, fixture);

        let mut corrupted = fixture.to_vec();
        corrupted[0] ^= 1;
        assert_eq!(
            restore_protected_exact_v1(&eligible, &entry, &corrupted),
            Err(RollbackRestoreErrorV1::ProtectedBytesMismatch)
        );

        for (observed_attempt_id, acceptance, effect_crossing, expected) in [
            (
                Some(digest(b"stale-attempt", fixture)),
                CutoverAcceptanceV1::PreAccept,
                EffectCrossingV1::ProvenNotCrossed,
                RollbackDispositionV1::RefusedStaleHost,
            ),
            (
                Some(attempt),
                CutoverAcceptanceV1::Accepted,
                EffectCrossingV1::ConfirmedCrossed,
                RollbackDispositionV1::VNextFreshGenerationRecoveryOnly,
            ),
        ] {
            let refused = RollbackAssessmentV1::assess_cutover_observation(
                attempt,
                observed_attempt_id,
                acceptance,
                effect_crossing,
            )
            .expect("typed rollback refusal assessment");
            assert_eq!(
                restore_protected_exact_v1(&refused, &entry, fixture)
                    .expect("typed rollback refusal"),
                ProtectedV1RollbackOutcomeV1::Refused {
                    disposition: expected
                }
            );
        }
        format!(
            "restored_exact_v1_and_refused_stale_or_crossed;{}",
            rows.typed_binding()
        )
    }

    #[derive(Debug)]
    struct RouteOutcomes {
        old_reader: String,
        new_reader: String,
        writer: String,
        rollback: String,
    }

    fn execute_real_routes() -> RouteOutcomes {
        let rows = FrozenRows::parse(INSTANCE_FIXTURE).expect("exact frozen migration fixture");
        assert_eq!(rows.row_count, 564);
        let mut mutant = INSTANCE_FIXTURE.to_vec();
        mutant[0] ^= 1;
        assert_eq!(
            FrozenRows::parse(&mutant),
            Err("fixture identity differs"),
            "fixture byte substitution must fail before any route executes"
        );

        let old_reader = old_reader_route(&rows, INSTANCE_FIXTURE);
        let (new_reader, current_consumers) = new_reader_route(&rows, INSTANCE_FIXTURE);
        let temporary = TemporaryRoot::new();
        let writer = writer_commit_route(&rows, INSTANCE_FIXTURE, &current_consumers, &temporary);
        let rollback = rollback_route(&rows, INSTANCE_FIXTURE, writer.request_id);
        RouteOutcomes {
            old_reader,
            new_reader,
            writer: writer.typed_result,
            rollback,
        }
    }

    fn prefixed_digest(bytes: &[u8]) -> String {
        format!(
            "sha256:{}",
            MigrationDigestV1::digest_bytes(bytes).expect("nonzero observation digest")
        )
    }

    fn binding(path: &str, bytes: &[u8]) -> Value {
        json!({
            "path": path,
            "byte_length": bytes.len(),
            "sha256": prefixed_digest(bytes),
        })
    }

    fn executable_binding() -> Value {
        let target = PathBuf::from(
            env::var_os("CARGO_TARGET_DIR").expect("receipt requires CARGO_TARGET_DIR"),
        );
        let target = fs::canonicalize(target).expect("canonical target root");
        let executable =
            fs::canonicalize(env::current_exe().expect("current test executable path"))
                .expect("canonical test executable");
        let relative = executable
            .strip_prefix(&target)
            .expect("test executable is rooted in CARGO_TARGET_DIR");
        let path = relative
            .to_str()
            .expect("UTF-8 test executable path")
            .replace('\\', "/");
        assert!(!path.is_empty() && !path.split('/').any(|part| part == ".."));
        let bytes = fs::read(&executable).expect("read invoked test executable");
        let mut value = binding(&path, &bytes);
        value
            .as_object_mut()
            .expect("binding object")
            .insert("root".to_string(), Value::String("target".to_string()));
        value
    }

    fn route_observation(proof_id: &str, route: &str, typed_result: &str) -> Value {
        json!({
            "schema_version": ROUTE_OBSERVATION_SCHEMA,
            "proof_id": proof_id,
            "route": route,
            "typed_result": typed_result,
            "status": "observed",
        })
    }

    fn validate_receipt_shape(receipt: &Value, proof_id: &str, cohort_identity: &str) {
        let object = receipt.as_object().expect("cohort receipt object");
        assert_eq!(object.len(), 5);
        assert_eq!(receipt["schema_version"], COHORT_OBSERVATION_SCHEMA);
        assert_eq!(receipt["proof_id"], proof_id);
        assert_eq!(receipt["cohort_identity"], cohort_identity);
        let executables = receipt["executables"]
            .as_object()
            .expect("executable bindings");
        assert_eq!(executables.len(), 3);
        for role in ["old_reader", "new_reader", "writer"] {
            let binding = executables[role]
                .as_object()
                .expect("rooted executable binding");
            assert_eq!(binding.len(), 4);
            assert_eq!(binding["root"], "target");
            assert!(
                binding["byte_length"]
                    .as_u64()
                    .is_some_and(|length| length > 0)
            );
            assert!(
                binding["sha256"]
                    .as_str()
                    .is_some_and(|value| value.len() == 71 && value.starts_with("sha256:"))
            );
        }
        let outcomes = receipt["outcomes"].as_object().expect("route outcomes");
        assert_eq!(outcomes.len(), 4);
        for route in ["old_reader", "new_reader", "writer", "rollback"] {
            let outcome = outcomes[route].as_object().expect("typed route outcome");
            assert_eq!(outcome.len(), 2);
            assert!(
                outcome["typed_result"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            );
            let observation = outcome["observation"]
                .as_object()
                .expect("route observation binding");
            assert_eq!(observation.len(), 3);
            assert!(
                observation["byte_length"]
                    .as_u64()
                    .is_some_and(|length| length > 0)
            );
        }
    }

    fn emit_receipt_after_assertions(outcomes: &RouteOutcomes) {
        let Some(receipt_path) = env::var_os("MAESTRO_FINAL_PROOF_RECEIPT").map(PathBuf::from)
        else {
            return;
        };
        let proof_id =
            env::var("MAESTRO_FINAL_PROOF_ID").expect("cohort receipt requires proof id");
        assert!(proof_id.starts_with("s11-"));
        let cohort_path = PathBuf::from(
            env::var_os("MAESTRO_MIGRATION_COHORT_PATH")
                .expect("cohort receipt requires migration cohort path"),
        );
        let cohort_bytes = fs::read(&cohort_path).expect("read invoked migration cohort");
        assert_eq!(
            cohort_bytes, COHORT_FIXTURE,
            "invoked cohort descriptor bytes differ from the frozen cohort"
        );
        let descriptor: Value =
            serde_json::from_slice(&cohort_bytes).expect("valid migration cohort descriptor");
        assert_eq!(
            descriptor["fixture"]["sha256"],
            format!("sha256:{EXPECTED_INSTANCE_DIGEST}")
        );
        assert_eq!(
            descriptor["required_outcomes"],
            json!([
                "old_reader_typed_compatibility_or_refusal",
                "new_reader_typed_acceptance",
                "writer_typed_commit",
                "rollback_typed_restore"
            ])
        );

        let output_root = receipt_path.parent().expect("receipt output root");
        fs::create_dir_all(output_root).expect("create receipt output root");
        let route_results = [
            ("old_reader", outcomes.old_reader.as_str()),
            ("new_reader", outcomes.new_reader.as_str()),
            ("writer", outcomes.writer.as_str()),
            ("rollback", outcomes.rollback.as_str()),
        ];
        let mut outcome_values = serde_json::Map::new();
        for (route, typed_result) in route_results {
            let file_name = format!("{proof_id}-{route}-route-observation.v1.json");
            let observation = route_observation(&proof_id, route, typed_result);
            let bytes = serde_json::to_vec(&observation).expect("serialize route observation");
            fs::write(output_root.join(&file_name), &bytes).expect("write route observation");
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &fs::read(output_root.join(&file_name)).expect("read route observation")
                )
                .expect("parse route observation"),
                observation
            );
            outcome_values.insert(
                route.to_string(),
                json!({
                    "typed_result": typed_result,
                    "observation": binding(&file_name, &bytes),
                }),
            );
        }

        let executable = executable_binding();
        let cohort_identity = prefixed_digest(&cohort_bytes);
        let receipt = json!({
            "schema_version": COHORT_OBSERVATION_SCHEMA,
            "proof_id": proof_id,
            "cohort_identity": cohort_identity,
            "executables": {
                "old_reader": executable.clone(),
                "new_reader": executable.clone(),
                "writer": executable,
            },
            "outcomes": Value::Object(outcome_values),
        });
        validate_receipt_shape(&receipt, &proof_id, &cohort_identity);
        let bytes = serde_json::to_vec(&receipt).expect("serialize cohort observation");
        fs::write(&receipt_path, &bytes).expect("write final cohort observation receipt");
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(&receipt_path).expect("read final cohort receipt")
            )
            .expect("parse final cohort receipt"),
            receipt
        );
    }

    #[test]
    fn frozen_cohort_migration_observes_real_reader_and_writer_routes() {
        let outcomes = execute_real_routes();
        assert!(outcomes.old_reader.starts_with("opaque_sealed_only;rows="));
        assert!(outcomes.new_reader.starts_with("exact_current;rows="));
        assert!(outcomes.writer.starts_with("inactive_store_commit;rows="));
        assert!(outcomes.old_reader.contains(EXPECTED_INSTANCE_DIGEST));
        assert!(outcomes.new_reader.contains(EXPECTED_INSTANCE_DIGEST));
        assert!(outcomes.writer.contains(EXPECTED_INSTANCE_DIGEST));
        emit_receipt_after_assertions(&outcomes);
    }

    #[test]
    fn frozen_cohort_rollback_observes_restore_and_refusal_routes() {
        let outcomes = execute_real_routes();
        assert!(
            outcomes
                .rollback
                .starts_with("restored_exact_v1_and_refused_stale_or_crossed;rows=")
        );
        assert!(outcomes.rollback.contains(EXPECTED_INSTANCE_DIGEST));
        emit_receipt_after_assertions(&outcomes);
    }
}
