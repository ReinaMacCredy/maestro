//! Typed, inert literals for the MigrationCutoverContract successor.
//!
//! The successor cannot receive a ManifestId or current SchemaIds until every
//! rotated dependency is present. These types retain exact predecessor evidence
//! while making unresolved current identities explicit blocking values.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::CborValue;

pub const MIGRATION_CUTOVER_SCHEMA_COUNT: u64 = 12;
pub const MIGRATION_CUTOVER_INVARIANT_COUNT: u64 = 23;
pub const MIGRATION_CUTOVER_PREDECESSOR_COUNT: u64 = 10;
pub const MIGRATION_CUTOVER_COMPONENT_COUNT: u64 = 50;
pub const MIGRATION_CUTOVER_FINALITY_SCHEMA_ID_COUNT: u64 = 3;
pub const MIGRATION_CUTOVER_FINALITY_EDGE_ROW_COUNT: u64 = 11;
pub const MIGRATION_CUTOVER_READ_WRITE_COHORT_COUNT: u64 = 4;
pub const MIGRATION_CUTOVER_READ_WRITE_COHORT_ROW_COUNT: u64 = 46;
pub const C868_SCHEMA_COUNT: u64 = 38;
pub const C868_SUITE_COMPONENT_COUNT: u64 = 62;
pub const C868_RUNTIME_EDGE_COUNT: u64 = 61;
pub const EXPECTED_DELTA_SLOT_COUNT: usize = 8;

pub const PREDECESSOR_CONTRACT_MANIFEST_ID: [u8; 32] =
    decode_hex_32("60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6");
pub const PREDECESSOR_ARTIFACT_SHA256: [u8; 32] =
    decode_hex_32("f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9");
pub const PREDECESSOR_ASSOCIATION_SCHEMA_ID: [u8; 32] =
    decode_hex_32("fddd9d43b7f8662187b834a64ef5fb0ba96b2182b6218c1a2c1b5aaca0e26808");
pub const PREDECESSOR_ACTIVE_HEAD_SCHEMA_ID: [u8; 32] =
    decode_hex_32("55106c12ddae6246d8db91ec5f81b37b527b00214af163b60b30c43b401d44db");
pub const PREDECESSOR_PRESTORE_SEAL_SCHEMA_ID: [u8; 32] =
    decode_hex_32("dc376892ebcc68640b1c1795fe1f736d5c61a41bbd74d8fa5005aff049df23b5");
pub const PREDECESSOR_FINALITY_EDGE_MANIFEST_ID: [u8; 32] =
    decode_hex_32("026b61dd18923e40917167af14737124ec11b1cabdb69fdb2422bb50d4a80466");
pub const PREDECESSOR_READ_WRITE_SET_DESCRIPTOR_ID: [u8; 32] =
    decode_hex_32("99333b038139e952f55ae22bd82383679a978ce8c2559ac44eeaebc15b3addec");
pub const PREDECESSOR_WRITER_EPOCH_DESCRIPTOR_ID: [u8; 32] =
    decode_hex_32("f3e6d7c105193f278bcfdd744d7b715358a59ffc8b7b02c3f17fe1592d1c6e6b");
pub const PREDECESSOR_MIGRATION_EPOCH_DESCRIPTOR_ID: [u8; 32] =
    decode_hex_32("95d517009025279d79108c8cf81418cf101ff77fedd333326fde03ac223e0a69");
pub const PREDECESSOR_C868_SUITE_MANIFEST_ID: [u8; 32] =
    decode_hex_32("5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f");
pub const PREDECESSOR_C868_EDGE_MANIFEST_ID: [u8; 32] =
    decode_hex_32("917376f49f5ed01ab53a7a71f1527fc0b3fc03d2632b47b68333cf2ba7899fe2");
pub const PREDECESSOR_C868_ARTIFACT_SHA256: [u8; 32] =
    decode_hex_32("d55e34610d888fca3ec6995820e50fe744332748fe28b766be4c64bbd2672622");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CutoverCommitmentV1([u8; 32]);

impl CutoverCommitmentV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, MigrationCutoverError> {
        if bytes == [0; 32] {
            return Err(MigrationCutoverError::ZeroCommitment);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Bytes(self.0.to_vec())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CutoverDomainV1 {
    Repository,
    Installation,
}

impl CutoverDomainV1 {
    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::Repository => 1,
            Self::Installation => 2,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Installation => "installation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverDomainRefV1 {
    domain: CutoverDomainV1,
    domain_id: CutoverCommitmentV1,
    generation: u64,
    epoch: u64,
}

impl CutoverDomainRefV1 {
    pub fn new(
        domain: CutoverDomainV1,
        domain_id: CutoverCommitmentV1,
        generation: u64,
        epoch: u64,
    ) -> Result<Self, MigrationCutoverError> {
        if generation == 0 {
            return Err(MigrationCutoverError::ZeroGeneration);
        }
        if epoch == 0 {
            return Err(MigrationCutoverError::ZeroEpoch);
        }
        Ok(Self {
            domain,
            domain_id,
            generation,
            epoch,
        })
    }

    pub const fn domain(&self) -> CutoverDomainV1 {
        self.domain
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.domain.numeric_tag()),
            self.domain_id.canonical_value(),
            CborValue::Unsigned(self.generation),
            CborValue::Unsigned(self.epoch),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseBindingV1 {
    RepositoryAbsent,
    InstallationExact(CutoverCommitmentV1),
}

impl ReleaseBindingV1 {
    fn validate_domain(&self, domain: CutoverDomainV1) -> Result<(), MigrationCutoverError> {
        match (domain, self) {
            (CutoverDomainV1::Repository, Self::RepositoryAbsent)
            | (CutoverDomainV1::Installation, Self::InstallationExact(_)) => Ok(()),
            (CutoverDomainV1::Repository, Self::InstallationExact(_)) => {
                Err(MigrationCutoverError::RepositoryReleasePresent)
            }
            (CutoverDomainV1::Installation, Self::RepositoryAbsent) => {
                Err(MigrationCutoverError::InstallationReleaseMissing)
            }
        }
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::RepositoryAbsent => CborValue::Array(vec![CborValue::Unsigned(0)]),
            Self::InstallationExact(release_id) => {
                CborValue::Array(vec![CborValue::Unsigned(1), release_id.canonical_value()])
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationCutoverContextV1 {
    ActiveStore {
        distribution_commit_record_id: CutoverCommitmentV1,
    },
    PreStore {
        sealed_ceremony_attempt_id: CutoverCommitmentV1,
        candidate_seal_id: CutoverCommitmentV1,
        expected_old_root_id: CutoverCommitmentV1,
    },
}

impl MigrationCutoverContextV1 {
    pub const fn numeric_tag(&self) -> u64 {
        match self {
            Self::ActiveStore { .. } => 1,
            Self::PreStore { .. } => 2,
        }
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::ActiveStore {
                distribution_commit_record_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                distribution_commit_record_id.canonical_value(),
            ]),
            Self::PreStore {
                sealed_ceremony_attempt_id,
                candidate_seal_id,
                expected_old_root_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                sealed_ceremony_attempt_id.canonical_value(),
                candidate_seal_id.canonical_value(),
                expected_old_root_id.canonical_value(),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationCutoverMaterialV1 {
    pub association_id: CutoverCommitmentV1,
    pub inventory_id: CutoverCommitmentV1,
    pub target_set_id: CutoverCommitmentV1,
    pub quarantine_set_id: CutoverCommitmentV1,
    pub consumer_set_id: CutoverCommitmentV1,
    pub distribution_receipt_id: CutoverCommitmentV1,
    pub candidate_store_root_id: CutoverCommitmentV1,
    pub schema_read_write_set_id: CutoverCommitmentV1,
    pub writer_protocol_epoch_id: CutoverCommitmentV1,
    pub migration_epoch_id: CutoverCommitmentV1,
}

impl MigrationCutoverMaterialV1 {
    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.association_id.canonical_value(),
            self.inventory_id.canonical_value(),
            self.target_set_id.canonical_value(),
            self.quarantine_set_id.canonical_value(),
            self.consumer_set_id.canonical_value(),
            self.distribution_receipt_id.canonical_value(),
            self.candidate_store_root_id.canonical_value(),
            self.schema_read_write_set_id.canonical_value(),
            self.writer_protocol_epoch_id.canonical_value(),
            self.migration_epoch_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationCutoverAssociationV1 {
    domain_ref: CutoverDomainRefV1,
    release: ReleaseBindingV1,
    context: MigrationCutoverContextV1,
    material: MigrationCutoverMaterialV1,
}

impl MigrationCutoverAssociationV1 {
    pub fn new(
        domain_ref: CutoverDomainRefV1,
        release: ReleaseBindingV1,
        context: MigrationCutoverContextV1,
        material: MigrationCutoverMaterialV1,
    ) -> Result<Self, MigrationCutoverError> {
        release.validate_domain(domain_ref.domain())?;
        Ok(Self {
            domain_ref,
            release,
            context,
            material,
        })
    }

    pub const fn domain_ref(&self) -> &CutoverDomainRefV1 {
        &self.domain_ref
    }

    pub const fn release(&self) -> &ReleaseBindingV1 {
        &self.release
    }

    pub const fn context(&self) -> &MigrationCutoverContextV1 {
        &self.context
    }

    pub const fn material(&self) -> &MigrationCutoverMaterialV1 {
        &self.material
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            self.domain_ref.canonical_value(),
            self.release.canonical_value(),
            self.context.canonical_value(),
            self.material.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveStorePreconditionV1 {
    DistributionReceipt(CutoverCommitmentV1),
    DistributionCommitRecord {
        commit_record_id: CutoverCommitmentV1,
        receipt_id: CutoverCommitmentV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreOwningHeadV1 {
    pub association_id: CutoverCommitmentV1,
    pub distribution_commit_record_id: CutoverCommitmentV1,
    pub distribution_receipt_id: CutoverCommitmentV1,
    pub domain_ref: CutoverDomainRefV1,
    pub release: ReleaseBindingV1,
    pub candidate_store_root_id: CutoverCommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveStoreAtomicParticipantV1 {
    Association(CutoverCommitmentV1),
    OwningHead(ActiveStoreOwningHeadV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreFinalityPartsV1 {
    pub association: MigrationCutoverAssociationV1,
    pub ordered_preconditions: Vec<ActiveStorePreconditionV1>,
    pub atomic_participants: Vec<ActiveStoreAtomicParticipantV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreFinalityV1 {
    parts: ActiveStoreFinalityPartsV1,
}

impl ActiveStoreFinalityV1 {
    pub fn new(parts: ActiveStoreFinalityPartsV1) -> Result<Self, MigrationCutoverError> {
        validate_active_store_finality(&parts)?;
        Ok(Self { parts })
    }

    pub const fn parts(&self) -> &ActiveStoreFinalityPartsV1 {
        &self.parts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreStorePreconditionV1 {
    SealedCeremonyAttempt(CutoverCommitmentV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreStoreCandidateSealV1 {
    pub association_id: CutoverCommitmentV1,
    pub candidate_seal_id: CutoverCommitmentV1,
    pub sealed_ceremony_attempt_id: CutoverCommitmentV1,
    pub domain_ref: CutoverDomainRefV1,
    pub release: ReleaseBindingV1,
    pub candidate_store_root_id: CutoverCommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedExpectedOldCasV1 {
    pub association_id: CutoverCommitmentV1,
    pub expected_old_root_id: CutoverCommitmentV1,
    pub candidate_store_root_id: CutoverCommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreStoreAtomicParticipantV1 {
    Association(CutoverCommitmentV1),
    CandidateSeal(PreStoreCandidateSealV1),
    ProtectedExpectedOldCas(ProtectedExpectedOldCasV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreStoreFinalityPartsV1 {
    pub association: MigrationCutoverAssociationV1,
    pub ordered_preconditions: Vec<PreStorePreconditionV1>,
    pub atomic_participants: Vec<PreStoreAtomicParticipantV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreStoreFinalityV1 {
    parts: PreStoreFinalityPartsV1,
}

impl PreStoreFinalityV1 {
    pub fn new(parts: PreStoreFinalityPartsV1) -> Result<Self, MigrationCutoverError> {
        validate_pre_store_finality(&parts)?;
        Ok(Self { parts })
    }

    pub const fn parts(&self) -> &PreStoreFinalityPartsV1 {
        &self.parts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociationConsumptionSetV1 {
    association_ids: Vec<CutoverCommitmentV1>,
}

impl AssociationConsumptionSetV1 {
    pub fn new(association_ids: Vec<CutoverCommitmentV1>) -> Result<Self, MigrationCutoverError> {
        let mut unique = BTreeSet::new();
        for association_id in &association_ids {
            if !unique.insert(*association_id) {
                return Err(MigrationCutoverError::AssociationReused);
            }
        }
        Ok(Self { association_ids })
    }

    pub fn association_ids(&self) -> &[CutoverCommitmentV1] {
        &self.association_ids
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SuccessorDependencySlotV1 {
    PublicContract7138,
    BoundedRecoveryD116,
    CausalJoinH2,
    CancellationLabelH3,
    CoreCatalogEfa0,
    BehavioralSuiteC868,
    ReleaseBinding,
    WriterCompatibility,
}

impl SuccessorDependencySlotV1 {
    pub const ALL: [Self; EXPECTED_DELTA_SLOT_COUNT] = [
        Self::PublicContract7138,
        Self::BoundedRecoveryD116,
        Self::CausalJoinH2,
        Self::CancellationLabelH3,
        Self::CoreCatalogEfa0,
        Self::BehavioralSuiteC868,
        Self::ReleaseBinding,
        Self::WriterCompatibility,
    ];

    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::PublicContract7138 => 1,
            Self::BoundedRecoveryD116 => 2,
            Self::CausalJoinH2 => 3,
            Self::CancellationLabelH3 => 4,
            Self::CoreCatalogEfa0 => 5,
            Self::BehavioralSuiteC868 => 6,
            Self::ReleaseBinding => 7,
            Self::WriterCompatibility => 8,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicContract7138 => "7138_public_contract",
            Self::BoundedRecoveryD116 => "d116_bounded_recovery",
            Self::CausalJoinH2 => "h2_causal_join",
            Self::CancellationLabelH3 => "h3_cancellation_label",
            Self::CoreCatalogEfa0 => "efa0_core_catalogs",
            Self::BehavioralSuiteC868 => "c868_behavioral_suite",
            Self::ReleaseBinding => "release_binding",
            Self::WriterCompatibility => "writer_compatibility",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedDeltaRowV1 {
    pub slot: SuccessorDependencySlotV1,
    pub successor_id: Option<CutoverCommitmentV1>,
    pub blocking: bool,
}

impl ExpectedDeltaRowV1 {
    pub const fn unresolved(slot: SuccessorDependencySlotV1) -> Self {
        Self {
            slot,
            successor_id: None,
            blocking: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedDeltaManifestV1 {
    rows: Vec<ExpectedDeltaRowV1>,
}

impl ExpectedDeltaManifestV1 {
    pub fn new(rows: Vec<ExpectedDeltaRowV1>) -> Result<Self, MigrationCutoverError> {
        if rows.len() != EXPECTED_DELTA_SLOT_COUNT {
            return Err(MigrationCutoverError::ExpectedDeltaCoverage);
        }
        for (row, expected_slot) in rows.iter().zip(SuccessorDependencySlotV1::ALL) {
            if row.slot != expected_slot {
                return Err(MigrationCutoverError::ExpectedDeltaCoverage);
            }
            if !row.blocking || row.successor_id.is_some() {
                return Err(MigrationCutoverError::UnresolvedDependencyNotBlocking);
            }
        }
        Ok(Self { rows })
    }

    pub fn unresolved() -> Self {
        Self {
            rows: SuccessorDependencySlotV1::ALL
                .into_iter()
                .map(ExpectedDeltaRowV1::unresolved)
                .collect(),
        }
    }

    pub fn rows(&self) -> &[ExpectedDeltaRowV1] {
        &self.rows
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(
            self.rows
                .iter()
                .map(|row| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(row.slot.numeric_tag()),
                        CborValue::optional(
                            row.successor_id.map(CutoverCommitmentV1::canonical_value),
                        ),
                        CborValue::Bool(row.blocking),
                    ])
                })
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationCutoverSuccessorLiteralV1 {
    expected_deltas: ExpectedDeltaManifestV1,
    current_finality_schema_ids: [Option<CutoverCommitmentV1>; 3],
}

impl MigrationCutoverSuccessorLiteralV1 {
    pub fn new(
        expected_deltas: ExpectedDeltaManifestV1,
        current_finality_schema_ids: [Option<CutoverCommitmentV1>; 3],
    ) -> Result<Self, MigrationCutoverError> {
        let predecessors = [
            PREDECESSOR_ASSOCIATION_SCHEMA_ID,
            PREDECESSOR_ACTIVE_HEAD_SCHEMA_ID,
            PREDECESSOR_PRESTORE_SEAL_SCHEMA_ID,
        ];
        for (current, predecessor) in current_finality_schema_ids.iter().zip(predecessors) {
            if current.is_some_and(|id| id.as_bytes() == &predecessor) {
                return Err(MigrationCutoverError::PredecessorIdentityPromoted);
            }
            if current.is_some() {
                return Err(MigrationCutoverError::FabricatedSuccessorIdentity);
            }
        }
        Ok(Self {
            expected_deltas,
            current_finality_schema_ids,
        })
    }

    pub fn blocked() -> Self {
        Self {
            expected_deltas: ExpectedDeltaManifestV1::unresolved(),
            current_finality_schema_ids: [None, None, None],
        }
    }

    pub const fn successor_manifest_id(&self) -> Option<CutoverCommitmentV1> {
        None
    }

    pub const fn current_finality_schema_ids(&self) -> &[Option<CutoverCommitmentV1>; 3] {
        &self.current_finality_schema_ids
    }

    pub const fn expected_deltas(&self) -> &ExpectedDeltaManifestV1 {
        &self.expected_deltas
    }

    pub const fn h2_h3_can_promote_causal_evidence(&self) -> bool {
        false
    }

    pub const fn filenames_or_sidecars_are_authority(&self) -> bool {
        false
    }

    pub const fn old_reader_admission_allowed(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MigrationCutoverError {
    #[error("cutover commitments cannot be all-zero")]
    ZeroCommitment,
    #[error("cutover Generation must be non-zero")]
    ZeroGeneration,
    #[error("cutover Epoch must be non-zero")]
    ZeroEpoch,
    #[error("Repository cutover must not carry Release")]
    RepositoryReleasePresent,
    #[error("Installation cutover must carry exact Release")]
    InstallationReleaseMissing,
    #[error("finality uses the wrong cutover context")]
    WrongContext,
    #[error("finality preconditions are missing, duplicated, reordered, or mismatched")]
    InvalidFinalityPreconditions,
    #[error("atomic finality participants are missing, duplicated, reordered, or mismatched")]
    InvalidAtomicParticipants,
    #[error("association, receipt, commit, seal, CAS, or head identity does not match")]
    FinalityIdentityMismatch,
    #[error("domain, Generation, Epoch, Release, or candidate root does not match")]
    FinalityBindingMismatch,
    #[error("MigrationCutoverAssociation was consumed more than once")]
    AssociationReused,
    #[error("expected-delta manifest must contain exactly the eight ordered dependency slots")]
    ExpectedDeltaCoverage,
    #[error("an unresolved successor dependency must remain blocking and have no identity")]
    UnresolvedDependencyNotBlocking,
    #[error("predecessor evidence cannot be promoted as a current successor identity")]
    PredecessorIdentityPromoted,
    #[error("successor identity cannot be fabricated while dependencies are unresolved")]
    FabricatedSuccessorIdentity,
}

fn validate_active_store_finality(
    parts: &ActiveStoreFinalityPartsV1,
) -> Result<(), MigrationCutoverError> {
    let association = &parts.association;
    let MigrationCutoverContextV1::ActiveStore {
        distribution_commit_record_id,
    } = association.context()
    else {
        return Err(MigrationCutoverError::WrongContext);
    };
    let receipt_id = association.material().distribution_receipt_id;
    let expected_preconditions = [
        ActiveStorePreconditionV1::DistributionReceipt(receipt_id),
        ActiveStorePreconditionV1::DistributionCommitRecord {
            commit_record_id: *distribution_commit_record_id,
            receipt_id,
        },
    ];
    if parts.ordered_preconditions.as_slice() != expected_preconditions {
        return Err(MigrationCutoverError::InvalidFinalityPreconditions);
    }
    let [association_participant, head_participant] = parts.atomic_participants.as_slice() else {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    };
    if association_participant
        != &ActiveStoreAtomicParticipantV1::Association(association.material().association_id)
    {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    }
    let ActiveStoreAtomicParticipantV1::OwningHead(head) = head_participant else {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    };
    if head.association_id != association.material().association_id
        || head.distribution_commit_record_id != *distribution_commit_record_id
        || head.distribution_receipt_id != receipt_id
    {
        return Err(MigrationCutoverError::FinalityIdentityMismatch);
    }
    if head.domain_ref != *association.domain_ref()
        || head.release != *association.release()
        || head.candidate_store_root_id != association.material().candidate_store_root_id
    {
        return Err(MigrationCutoverError::FinalityBindingMismatch);
    }
    Ok(())
}

fn validate_pre_store_finality(
    parts: &PreStoreFinalityPartsV1,
) -> Result<(), MigrationCutoverError> {
    let association = &parts.association;
    let MigrationCutoverContextV1::PreStore {
        sealed_ceremony_attempt_id,
        candidate_seal_id,
        expected_old_root_id,
    } = association.context()
    else {
        return Err(MigrationCutoverError::WrongContext);
    };
    let expected_preconditions = [PreStorePreconditionV1::SealedCeremonyAttempt(
        *sealed_ceremony_attempt_id,
    )];
    if parts.ordered_preconditions.as_slice() != expected_preconditions {
        return Err(MigrationCutoverError::InvalidFinalityPreconditions);
    }
    let [association_participant, seal_participant, cas_participant] =
        parts.atomic_participants.as_slice()
    else {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    };
    if association_participant
        != &PreStoreAtomicParticipantV1::Association(association.material().association_id)
    {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    }
    let PreStoreAtomicParticipantV1::CandidateSeal(seal) = seal_participant else {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    };
    let PreStoreAtomicParticipantV1::ProtectedExpectedOldCas(cas) = cas_participant else {
        return Err(MigrationCutoverError::InvalidAtomicParticipants);
    };
    if seal.association_id != association.material().association_id
        || seal.candidate_seal_id != *candidate_seal_id
        || seal.sealed_ceremony_attempt_id != *sealed_ceremony_attempt_id
        || cas.association_id != association.material().association_id
        || cas.expected_old_root_id != *expected_old_root_id
    {
        return Err(MigrationCutoverError::FinalityIdentityMismatch);
    }
    if seal.domain_ref != *association.domain_ref()
        || seal.release != *association.release()
        || seal.candidate_store_root_id != association.material().candidate_store_root_id
        || cas.candidate_store_root_id != association.material().candidate_store_root_id
    {
        return Err(MigrationCutoverError::FinalityBindingMismatch);
    }
    Ok(())
}

const fn decode_hex_32(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        panic!("invariant: SHA-256 hex must contain 64 bytes");
    }
    let mut output = [0_u8; 32];
    let mut index = 0;
    while index < output.len() {
        output[index] =
            (decode_nibble(bytes[index * 2]) << 4) | decode_nibble(bytes[index * 2 + 1]);
        index += 1;
    }
    output
}

const fn decode_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invariant: SHA-256 hex must be lowercase hexadecimal"),
    }
}
