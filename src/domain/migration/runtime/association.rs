#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::execution::h3_withdrawal_publication::{
    ConsumedH3NativeCancelledMemberV1, H3MigrationFinalityV1, H3NativeCancelledClassificationV1,
    H3NativeCancelledSourceMemberV1, H3NativeCancelledTargetMemberV1,
    VerifiedH3WithdrawalPublicationUseV1,
};
use crate::domain::migration::MigrationCutoverError;
use crate::domain::migration::{
    ActiveStoreFinalityV1, MigrationCutoverAssociationV1, PreStoreFinalityV1, ReleaseBindingV1,
};
use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

use super::{
    ByteTotalInventoryV1, CancellationClassificationV1, ClassificationSetV1, ConsumerClosureV1,
    ConsumerGateStageV1, DeterministicIdentityMapV1, InactiveStoreImportReceiptV1,
    InactiveStoreImportRequestV1, MigrationDigestV1, MigrationIdentityErrorV1,
    RollbackAssessmentV1, RollbackDispositionV1, SealedQuarantineManifestV1,
};

const ASSOCIATION_MEANING_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.association-meaning.v1\0";
const ASSOCIATION_IDENTITY_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.association.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationAssociationMeaningV1 {
    inventory_id: MigrationDigestV1,
    classification_set_id: MigrationDigestV1,
    target_set_id: MigrationDigestV1,
    quarantine_set_id: MigrationDigestV1,
    consumer_set_id: MigrationDigestV1,
    consumer_census_id: MigrationDigestV1,
    inactive_import_receipt_id: MigrationDigestV1,
    destination_domain_id: MigrationDigestV1,
    candidate_store_root_id: MigrationDigestV1,
    protocol_release_id: Option<MigrationDigestV1>,
    schema_read_write_set_id: MigrationDigestV1,
    writer_protocol_epoch_id: MigrationDigestV1,
    migration_epoch_id: MigrationDigestV1,
    rollback_assessment_id: MigrationDigestV1,
    native_cancellation_count: u64,
    id: MigrationDigestV1,
}

impl MigrationAssociationMeaningV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "association meaning binds every Stage-11 proof family"
    )]
    pub fn new(
        inventory: &ByteTotalInventoryV1,
        classifications: &ClassificationSetV1,
        target_map: &DeterministicIdentityMapV1,
        quarantine: &SealedQuarantineManifestV1,
        consumers: &ConsumerClosureV1,
        import_request: &InactiveStoreImportRequestV1,
        import_receipt: &InactiveStoreImportReceiptV1,
        rollback: &RollbackAssessmentV1,
    ) -> Result<Self, MigrationAssociationErrorV1> {
        if classifications.inventory_id() != inventory.id()
            || target_map.classification_set_id() != classifications.id()
            || quarantine.inventory_id() != inventory.id()
            || quarantine.classification_set_id() != classifications.id()
            || import_request.inventory_id() != inventory.id()
            || import_request.classification_set_id() != classifications.id()
            || import_request.target_set_id() != target_map.id()
            || import_request.quarantine_set_id() != quarantine.id()
            || import_request.consumer_set_id() != consumers.id()
            || import_request.consumer_census_id() != consumers.census().id()
            || import_request.protocol_closure_id() != consumers.protocol().id()
            || import_receipt.request_id() != import_request.id()
        {
            return Err(MigrationAssociationErrorV1::ProofSetMismatch);
        }
        if consumers.stage() != ConsumerGateStageV1::BeforeSemanticCurrentness
            || !consumers.gate_passed()
            || consumers.census().entries().is_empty()
        {
            return Err(MigrationAssociationErrorV1::ConsumerGateNotClosed);
        }
        if rollback.cutover_attempt_id() != import_receipt.request_id() {
            return Err(MigrationAssociationErrorV1::RollbackAttemptMismatch);
        }
        // The association seals a pre-accept, effect-free world; any other
        // disposition means the attempt advanced or the host is stale, and
        // recording it here would bury the refusal the assessment computed.
        if rollback.disposition() != RollbackDispositionV1::ProtectedExactV1RollbackEligible {
            return Err(MigrationAssociationErrorV1::RollbackNotEligible);
        }
        let value = CborValue::Array(vec![
            inventory.id().canonical_value(),
            classifications.id().canonical_value(),
            target_map.id().canonical_value(),
            quarantine.id().canonical_value(),
            consumers.id().canonical_value(),
            consumers.census().id().canonical_value(),
            import_receipt.id().canonical_value(),
            import_receipt.destination_domain_id().canonical_value(),
            import_receipt.candidate_root_id().canonical_value(),
            CborValue::optional(
                consumers
                    .protocol()
                    .release_id()
                    .map(MigrationDigestV1::canonical_value),
            ),
            consumers
                .protocol()
                .schema_read_write_set_id()
                .canonical_value(),
            consumers
                .protocol()
                .writer_protocol_epoch_id()
                .canonical_value(),
            consumers.protocol().migration_epoch_id().canonical_value(),
            rollback.id().canonical_value(),
            CborValue::Unsigned(
                u64::try_from(classifications.native_cancellation_count())
                    .map_err(|_| MigrationAssociationErrorV1::H3CarrierCountMismatch)?,
            ),
        ]);
        let id = MigrationDigestV1::identify(ASSOCIATION_MEANING_DOMAIN_V1, &value)?;
        Ok(Self {
            inventory_id: inventory.id(),
            classification_set_id: classifications.id(),
            target_set_id: target_map.id(),
            quarantine_set_id: quarantine.id(),
            consumer_set_id: consumers.id(),
            consumer_census_id: consumers.census().id(),
            inactive_import_receipt_id: import_receipt.id(),
            destination_domain_id: import_receipt.destination_domain_id(),
            candidate_store_root_id: import_receipt.candidate_root_id(),
            protocol_release_id: consumers.protocol().release_id(),
            schema_read_write_set_id: consumers.protocol().schema_read_write_set_id(),
            writer_protocol_epoch_id: consumers.protocol().writer_protocol_epoch_id(),
            migration_epoch_id: consumers.protocol().migration_epoch_id(),
            rollback_assessment_id: rollback.id(),
            native_cancellation_count: u64::try_from(classifications.native_cancellation_count())
                .map_err(|_| {
                MigrationAssociationErrorV1::H3CarrierCountMismatch
            })?,
            id,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn inventory_id(&self) -> MigrationDigestV1 {
        self.inventory_id
    }

    pub const fn target_set_id(&self) -> MigrationDigestV1 {
        self.target_set_id
    }

    pub const fn quarantine_set_id(&self) -> MigrationDigestV1 {
        self.quarantine_set_id
    }

    pub const fn consumer_set_id(&self) -> MigrationDigestV1 {
        self.consumer_set_id
    }

    pub const fn candidate_store_root_id(&self) -> MigrationDigestV1 {
        self.candidate_store_root_id
    }

    pub const fn destination_domain_id(&self) -> MigrationDigestV1 {
        self.destination_domain_id
    }

    pub const fn protocol_release_id(&self) -> Option<MigrationDigestV1> {
        self.protocol_release_id
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

    pub const fn native_cancellation_count(&self) -> u64 {
        self.native_cancellation_count
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.inventory_id.canonical_value(),
            self.classification_set_id.canonical_value(),
            self.target_set_id.canonical_value(),
            self.quarantine_set_id.canonical_value(),
            self.consumer_set_id.canonical_value(),
            self.consumer_census_id.canonical_value(),
            self.inactive_import_receipt_id.canonical_value(),
            self.destination_domain_id.canonical_value(),
            self.candidate_store_root_id.canonical_value(),
            CborValue::optional(
                self.protocol_release_id
                    .map(MigrationDigestV1::canonical_value),
            ),
            self.schema_read_write_set_id.canonical_value(),
            self.writer_protocol_epoch_id.canonical_value(),
            self.migration_epoch_id.canonical_value(),
            self.rollback_assessment_id.canonical_value(),
            CborValue::Unsigned(self.native_cancellation_count),
            self.id.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationAssociationFinalityV1 {
    ActiveStore(ActiveStoreFinalityV1),
    PreStore(PreStoreFinalityV1),
}

impl MigrationAssociationFinalityV1 {
    fn association(&self) -> &MigrationCutoverAssociationV1 {
        match self {
            Self::ActiveStore(finality) => &finality.parts().association,
            Self::PreStore(finality) => &finality.parts().association,
        }
    }
}

#[cfg(test)]
pub trait Stage9CutoverAssociationAdapterV1 {
    fn cutover_finality(
        &self,
        meaning: &MigrationAssociationMeaningV1,
    ) -> Result<TestOnlyStage9CutoverFinalityV1, MigrationAssociationErrorV1>;
}

#[cfg(test)]
pub type TestOnlyStage9CutoverFinalityV1 = MigrationAssociationFinalityV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationAssociationV1 {
    meaning: MigrationAssociationMeaningV1,
    finality: MigrationAssociationFinalityV1,
    id: MigrationDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct H3NativeCancelledMemberDescriptorV1 {
    source_id: MigrationDigestV1,
    target_id: MigrationDigestV1,
    withdrawal_publication_id: MigrationDigestV1,
    effect_lineage_id: MigrationDigestV1,
    authorizing_branch_id: MigrationDigestV1,
    result_id: MigrationDigestV1,
    idempotency_id: MigrationDigestV1,
}

pub(in crate::domain) struct H3NativeCancelledMigrationMemberV1<'tx> {
    publication: VerifiedH3WithdrawalPublicationUseV1<'tx>,
    descriptor: H3NativeCancelledMemberDescriptorV1,
    member_identity: [u8; 32],
    inventory_row: [u8; 32],
    source_inventory: [u8; 32],
    source_inventory_rows: [u8; 32],
    display_identity: [u8; 32],
    resolved_identity: [u8; 32],
    custody_transition: [u8; 32],
    declared_target_closure: [u8; 32],
    quarantine_roots: [u8; 32],
    protected_roots: [u8; 32],
    consumer_gate: [u8; 32],
    claims_catalog_verification: [u8; 32],
    optional_release: Option<[u8; 32]>,
    classification_set_id: MigrationDigestV1,
}

impl<'tx> H3NativeCancelledMigrationMemberV1<'tx> {
    pub(in crate::domain) fn bind(
        publication: VerifiedH3WithdrawalPublicationUseV1<'tx>,
        source_id: MigrationDigestV1,
        inventory: &ByteTotalInventoryV1,
        classifications: &ClassificationSetV1,
        target_map: &DeterministicIdentityMapV1,
        quarantine: &SealedQuarantineManifestV1,
        consumers: &ConsumerClosureV1,
    ) -> Result<Self, MigrationAssociationErrorV1> {
        if classifications.inventory_id() != inventory.id()
            || target_map.classification_set_id() != classifications.id()
            || quarantine.inventory_id() != inventory.id()
            || quarantine.classification_set_id() != classifications.id()
        {
            return Err(MigrationAssociationErrorV1::ProofSetMismatch);
        }
        if consumers.stage() != ConsumerGateStageV1::BeforeSemanticCurrentness
            || !consumers.gate_passed()
            || consumers.census().entries().is_empty()
        {
            return Err(MigrationAssociationErrorV1::ConsumerGateNotClosed);
        }
        let source = inventory
            .row(source_id)
            .ok_or(MigrationAssociationErrorV1::H3MemberCoverageMismatch)?;
        let classification = classifications
            .row(source_id)
            .ok_or(MigrationAssociationErrorV1::H3MemberCoverageMismatch)?;
        let CancellationClassificationV1::NativeCancelled(join) = classification.cancellation()
        else {
            return Err(MigrationAssociationErrorV1::H3MemberCoverageMismatch);
        };
        let target = target_map
            .row(source_id)
            .ok_or(MigrationAssociationErrorV1::H3MemberCoverageMismatch)?;
        if join.source_id() != source_id
            || classification.target_id() != Some(join.target_id())
            || target.target_id() != join.target_id()
        {
            return Err(MigrationAssociationErrorV1::H3MemberContradiction);
        }

        let inventory_row = source.h3_row_commitment()?.into_bytes();
        let source_inventory = inventory.id().into_bytes();
        let source_inventory_rows = inventory.h3_rows_commitment()?.into_bytes();
        let display_identity =
            MigrationDigestV1::digest_bytes(source.display_locator().as_bytes())?.into_bytes();
        let resolved_identity =
            MigrationDigestV1::digest_bytes(source.resolved_locator().as_bytes())?.into_bytes();
        let effect_lineage = join.id().into_bytes();
        let custody_transition = target.h3_custody_transition()?.into_bytes();
        let target_id = join.target_id();
        let withdrawal_publication_id = join.publication_consumption_id();
        let member_identity = h3_native_cancelled_member_identity(
            withdrawal_publication_id.into_bytes(),
            source_id.into_bytes(),
            target_id.into_bytes(),
            inventory_row,
            target_id.into_bytes(),
            effect_lineage,
            custody_transition,
        );
        if member_identity == [0; 32] {
            return Err(MigrationAssociationErrorV1::H3MemberContradiction);
        }

        Ok(Self {
            publication,
            descriptor: H3NativeCancelledMemberDescriptorV1 {
                source_id,
                target_id,
                withdrawal_publication_id,
                effect_lineage_id: join.id(),
                authorizing_branch_id: join.authority_basis_id(),
                result_id: join.result_id(),
                idempotency_id: join.idempotency_id(),
            },
            member_identity,
            inventory_row,
            source_inventory,
            source_inventory_rows,
            display_identity,
            resolved_identity,
            custody_transition,
            declared_target_closure: target_map.id().into_bytes(),
            quarantine_roots: quarantine.id().into_bytes(),
            protected_roots: inventory.h3_protected_roots_commitment()?.into_bytes(),
            consumer_gate: consumers.id().into_bytes(),
            claims_catalog_verification: consumers
                .protocol()
                .schema_read_write_set_id()
                .into_bytes(),
            optional_release: consumers
                .protocol()
                .release_id()
                .map(MigrationDigestV1::into_bytes),
            classification_set_id: classifications.id(),
        })
    }

    fn consume(
        self,
        association: &MigrationCutoverAssociationV1,
        finality: H3MigrationFinalityV1<'_>,
    ) -> Result<ConsumedH3NativeCancelledMemberV1<'tx>, MigrationAssociationErrorV1> {
        let source = H3NativeCancelledSourceMemberV1::new(
            self.descriptor.source_id.into_bytes(),
            self.inventory_row,
            self.source_inventory,
            self.source_inventory_rows,
            self.display_identity,
            self.resolved_identity,
            self.descriptor.effect_lineage_id.into_bytes(),
            self.custody_transition,
        )
        .map_err(|_| MigrationAssociationErrorV1::H3MemberContradiction)?;
        let target = H3NativeCancelledTargetMemberV1::new(
            self.descriptor.target_id.into_bytes(),
            self.declared_target_closure,
            self.descriptor.target_id.into_bytes(),
            self.quarantine_roots,
            self.protected_roots,
            self.consumer_gate,
            self.claims_catalog_verification,
        )
        .map_err(|_| MigrationAssociationErrorV1::H3MemberContradiction)?;
        let classification = H3NativeCancelledClassificationV1::new(
            self.member_identity,
            self.descriptor.withdrawal_publication_id.into_bytes(),
            self.descriptor.authorizing_branch_id.into_bytes(),
            self.optional_release,
            self.descriptor.result_id.into_bytes(),
            self.descriptor.idempotency_id.into_bytes(),
        )
        .map_err(|_| MigrationAssociationErrorV1::H3MemberContradiction)?;
        self.publication
            .consume_native_cancelled_member_for_migration(
                association,
                finality,
                source,
                target,
                classification,
            )
            .map_err(|_| MigrationAssociationErrorV1::VerifiedH3WithdrawalRejected)
    }
}

pub(in crate::domain) struct H3VerifiedMigrationAssociationUseV1<'tx> {
    association: MigrationAssociationV1,
    _consumed_members: Vec<ConsumedH3NativeCancelledMemberV1<'tx>>,
}

impl H3VerifiedMigrationAssociationUseV1<'_> {
    pub(in crate::domain) const fn association(&self) -> &MigrationAssociationV1 {
        &self.association
    }
}

impl MigrationAssociationV1 {
    #[cfg(test)]
    pub(in crate::domain) fn from_verified_h3_native_cancelled_members<'tx>(
        meaning: MigrationAssociationMeaningV1,
        finality: MigrationAssociationFinalityV1,
        classifications: &ClassificationSetV1,
        mut h3_members: Vec<H3NativeCancelledMigrationMemberV1<'tx>>,
    ) -> Result<H3VerifiedMigrationAssociationUseV1<'tx>, MigrationAssociationErrorV1> {
        if u64::try_from(h3_members.len()).ok() != Some(meaning.native_cancellation_count) {
            return Err(MigrationAssociationErrorV1::H3CarrierCountMismatch);
        }
        if classifications.id() != meaning.classification_set_id {
            return Err(MigrationAssociationErrorV1::ProofSetMismatch);
        }

        let expected = classifications
            .rows()
            .iter()
            .filter_map(|row| match row.cancellation() {
                CancellationClassificationV1::NativeCancelled(join) => Some((
                    row.source_id(),
                    H3NativeCancelledMemberDescriptorV1 {
                        source_id: row.source_id(),
                        target_id: join.target_id(),
                        withdrawal_publication_id: join.publication_consumption_id(),
                        effect_lineage_id: join.id(),
                        authorizing_branch_id: join.authority_basis_id(),
                        result_id: join.result_id(),
                        idempotency_id: join.idempotency_id(),
                    },
                )),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut observed = BTreeMap::new();
        let mut member_identities = BTreeSet::new();
        let mut withdrawal_publications = BTreeSet::new();
        for member in &h3_members {
            if member.classification_set_id != meaning.classification_set_id
                || member.source_inventory != meaning.inventory_id.into_bytes()
                || member.declared_target_closure != meaning.target_set_id.into_bytes()
                || member.quarantine_roots != meaning.quarantine_set_id.into_bytes()
                || member.consumer_gate != meaning.consumer_set_id.into_bytes()
                || member.claims_catalog_verification
                    != meaning.schema_read_write_set_id.into_bytes()
                || member.optional_release
                    != meaning
                        .protocol_release_id
                        .map(MigrationDigestV1::into_bytes)
            {
                return Err(MigrationAssociationErrorV1::H3MemberContradiction);
            }
            if observed
                .insert(member.descriptor.source_id, member.descriptor)
                .is_some()
                || !member_identities.insert(member.member_identity)
                || !withdrawal_publications.insert(member.descriptor.withdrawal_publication_id)
            {
                return Err(MigrationAssociationErrorV1::H3MemberDuplicate);
            }
        }
        if observed.keys().ne(expected.keys()) {
            return Err(MigrationAssociationErrorV1::H3MemberCoverageMismatch);
        }
        if observed != expected {
            return Err(MigrationAssociationErrorV1::H3MemberContradiction);
        }

        let association = Self::assemble(meaning, finality)?;
        h3_members.sort_by_key(|member| member.descriptor.source_id);
        let mut consumed_members = Vec::with_capacity(h3_members.len());
        for member in h3_members {
            let h3_finality = match &association.finality {
                MigrationAssociationFinalityV1::ActiveStore(finality) => {
                    H3MigrationFinalityV1::ActiveStore(finality)
                }
                MigrationAssociationFinalityV1::PreStore(finality) => {
                    H3MigrationFinalityV1::PreStore(finality)
                }
            };
            consumed_members.push(member.consume(association.cutover(), h3_finality)?);
        }
        Ok(H3VerifiedMigrationAssociationUseV1 {
            association,
            _consumed_members: consumed_members,
        })
    }

    #[cfg(test)]
    pub fn from_stage9_adapter<A: Stage9CutoverAssociationAdapterV1>(
        meaning: MigrationAssociationMeaningV1,
        adapter: &A,
    ) -> Result<Self, MigrationAssociationErrorV1> {
        let finality = adapter.cutover_finality(&meaning)?;
        if meaning.native_cancellation_count != 0 {
            return Err(MigrationAssociationErrorV1::H3CarrierCountMismatch);
        }
        Self::assemble(meaning, finality)
    }

    fn assemble(
        meaning: MigrationAssociationMeaningV1,
        finality: MigrationAssociationFinalityV1,
    ) -> Result<Self, MigrationAssociationErrorV1> {
        let cutover = finality.association();
        let material = cutover.material();
        if material.association_id.as_bytes() != meaning.id().as_bytes()
            || material.inventory_id.as_bytes() != meaning.inventory_id.as_bytes()
            || material.target_set_id.as_bytes() != meaning.target_set_id.as_bytes()
            || material.quarantine_set_id.as_bytes() != meaning.quarantine_set_id.as_bytes()
            || material.consumer_set_id.as_bytes() != meaning.consumer_set_id.as_bytes()
            || material.candidate_store_root_id.as_bytes()
                != meaning.candidate_store_root_id.as_bytes()
            || material.schema_read_write_set_id.as_bytes()
                != meaning.schema_read_write_set_id.as_bytes()
            || material.writer_protocol_epoch_id.as_bytes()
                != meaning.writer_protocol_epoch_id.as_bytes()
            || material.migration_epoch_id.as_bytes() != meaning.migration_epoch_id.as_bytes()
        {
            return Err(MigrationAssociationErrorV1::ExternalBindingMismatch);
        }
        match (cutover.release(), meaning.protocol_release_id) {
            (ReleaseBindingV1::RepositoryAbsent, None) => {}
            (ReleaseBindingV1::InstallationExact(observed), Some(expected))
                if observed.as_bytes() == expected.as_bytes() => {}
            _ => return Err(MigrationAssociationErrorV1::ProtocolReleaseMismatch),
        }
        let id = MigrationDigestV1::identify(
            ASSOCIATION_IDENTITY_DOMAIN_V1,
            &CborValue::Array(vec![meaning.canonical_value(), cutover.canonical_value()]),
        )?;
        Ok(Self {
            meaning,
            finality,
            id,
        })
    }

    pub const fn meaning(&self) -> &MigrationAssociationMeaningV1 {
        &self.meaning
    }

    pub fn cutover(&self) -> &MigrationCutoverAssociationV1 {
        self.finality.association()
    }

    pub const fn finality(&self) -> &MigrationAssociationFinalityV1 {
        &self.finality
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MigrationAssociationErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error(transparent)]
    Cutover(#[from] MigrationCutoverError),
    #[error("migration association proof sets do not share one exact source closure")]
    ProofSetMismatch,
    #[error("migration association requires a passed before-currentness consumer gate")]
    ConsumerGateNotClosed,
    #[error("rollback assessment does not bind the inactive import attempt")]
    RollbackAttemptMismatch,
    #[error("rollback assessment does not attest protected exact-v1 eligibility")]
    RollbackNotEligible,
    #[error("external cutover bindings do not match the sealed Stage-11 meaning")]
    ExternalBindingMismatch,
    #[error("cutover Release binding does not match the consumer protocol closure")]
    ProtocolReleaseMismatch,
    #[error("native cancellation rows and verified H3 withdrawal carriers differ in count")]
    H3CarrierCountMismatch,
    #[error("native-cancelled H3 members do not exactly cover the Stage-11 classifications")]
    H3MemberCoverageMismatch,
    #[error("native-cancelled H3 member identity or withdrawal publication is duplicated")]
    H3MemberDuplicate,
    #[error("native-cancelled H3 member contradicts the closed Stage-11 association meaning")]
    H3MemberContradiction,
    #[error("verified H3 withdrawal publication was rejected for the exact migration finality")]
    VerifiedH3WithdrawalRejected,
}

fn h3_native_cancelled_member_identity(
    withdrawal_publication: [u8; 32],
    source_member_identity: [u8; 32],
    target_member_identity: [u8; 32],
    source_inventory_row: [u8; 32],
    cancelled_target: [u8; 32],
    effect_lineage: [u8; 32],
    custody_transition: [u8; 32],
) -> [u8; 32] {
    let mut writer = Sha256::new();
    writer.update(b"maestro.execution.h3-native-cancelled-member.v1\0");
    writer.update(withdrawal_publication);
    writer.update(source_member_identity);
    writer.update(target_member_identity);
    writer.update(source_inventory_row);
    writer.update(cancelled_target);
    writer.update(effect_lineage);
    writer.update(custody_transition);
    writer.finalize().into()
}
