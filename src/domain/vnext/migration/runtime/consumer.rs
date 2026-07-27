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
    pub(in crate::domain::vnext::migration) fn evaluate_owner_snapshot_parts(
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

    pub(in crate::domain::vnext) fn evaluate_installation_snapshot(
        stage: ConsumerGateStageV1,
        protocol: MigrationProtocolClosureV1,
        snapshot: crate::domain::vnext::installation::consumer_snapshot::InstallationMigrationConsumerSnapshotV1,
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
