use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

#[cfg(test)]
use crate::domain::execution::{
    ActiveStoreEffectSnapshotV1, ActiveStoreEffectWithdrawalOutcomeV1, EffectIntentLiveDispatchV1,
    RemoteClassificationV1,
};
use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

use super::{
    ByteTotalInventoryV1, InventoryNodeKindV1, MigrationDigestV1, MigrationIdentityErrorV1,
};

const CLASSIFICATION_IDENTITY_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.classification-set.v1\0";
const IDENTITY_MAP_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.identity-map.v1\0";
const CANCELLATION_JOIN_DOMAIN_V1: &[u8] =
    b"maestro.vnext.migration.native-cancellation-causal-join.v1\0";
const CANCELLATION_FACT_DOMAIN_V1: &[u8] =
    b"maestro.vnext.migration.stage4-native-cancellation-fact.v1\0";
const H3_CUSTODY_TRANSITION_DOMAIN_V1: &[u8] =
    b"maestro.vnext.migration.h3-custody-transition.v1\0";
const MAX_CLASSIFICATIONS_V1: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MigrationDispositionV1 {
    MappedNormative,
    MappedHistoricalNonBearer,
    OpaquePreserved,
    Quarantined,
    UnavailablePreexistingLoss,
}

impl MigrationDispositionV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::MappedNormative => 1,
            Self::MappedHistoricalNonBearer => 2,
            Self::OpaquePreserved => 3,
            Self::Quarantined => 4,
            Self::UnavailablePreexistingLoss => 5,
        }
    }

    const fn requires_target(self) -> bool {
        matches!(
            self,
            Self::MappedNormative | Self::MappedHistoricalNonBearer | Self::OpaquePreserved
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeCancellationStateV1 {
    Prepared,
    ConfirmedNotApplied,
}

impl NativeCancellationStateV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Prepared => 1,
            Self::ConfirmedNotApplied => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCancellationCausalJoinV1 {
    source_id: MigrationDigestV1,
    target_id: MigrationDigestV1,
    state: NativeCancellationStateV1,
    effect_intent_home_id: MigrationDigestV1,
    effect_origin_route_id: MigrationDigestV1,
    control_head_id: MigrationDigestV1,
    control_revision_id: MigrationDigestV1,
    live_dispatch_none_proof_id: MigrationDigestV1,
    attempt_absence_proof_id: MigrationDigestV1,
    run_closure_proof_id: MigrationDigestV1,
    authority_basis_id: MigrationDigestV1,
    capacity_debit_id: MigrationDigestV1,
    expected_old_head_id: MigrationDigestV1,
    withdrawal_receipt_id: MigrationDigestV1,
    result_id: MigrationDigestV1,
    idempotency_id: MigrationDigestV1,
    id: MigrationDigestV1,
}

impl NativeCancellationCausalJoinV1 {
    #[cfg(test)]
    pub fn test_only_from_stage4_publication(
        source_id: MigrationDigestV1,
        target_id: MigrationDigestV1,
        before: &ActiveStoreEffectSnapshotV1,
        outcome: &ActiveStoreEffectWithdrawalOutcomeV1,
        after: &ActiveStoreEffectSnapshotV1,
    ) -> Result<Self, ClassificationErrorV1> {
        let before_revision = before.control_revision();
        let after_revision = after.control_revision();
        let state = match before_revision.classification() {
            RemoteClassificationV1::Prepared => NativeCancellationStateV1::Prepared,
            RemoteClassificationV1::ConfirmedNotApplied => {
                NativeCancellationStateV1::ConfirmedNotApplied
            }
            _ => return Err(ClassificationErrorV1::Stage4CancellationSourceNotEligible),
        };
        let after_parts = after_revision.parts();
        let before_parts = before_revision.parts();
        let Some(result_commitment) = after_parts.result_commitment else {
            return Err(ClassificationErrorV1::Stage4CancellationPublicationMismatch);
        };
        let Some(idempotency_commitment) = after_parts.idempotency_commitment else {
            return Err(ClassificationErrorV1::Stage4CancellationPublicationMismatch);
        };
        if before.intent() != after.intent()
            || before.writer_term() != after.writer_term()
            || before.dispatch() != after.dispatch()
            || before.reconciliation() != after.reconciliation()
            || outcome.intent() != before.intent().id()
            || outcome.control_head() != after.control_head().id()
            || outcome.control_revision() != after_revision.id()
            || outcome.store_head().id() != after.state_binding().store_head_id()
            || before.state_binding().control_head() != Some(before.control_head().id())
            || after.state_binding().control_head() != Some(after.control_head().id())
            || before.state_binding().store_head_id() == after.state_binding().store_head_id()
            || before.state_binding().store_generation_id()
                == after.state_binding().store_generation_id()
            || before.control_head().id() == after.control_head().id()
            || before_revision.id() == after_revision.id()
            || before_revision.live_attempt().is_some()
            || before_revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || !before_revision.runs_closed()
            || after_revision.live_attempt().is_some()
            || after_revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || after_revision.classification() != RemoteClassificationV1::Cancelled
            || !after_revision.runs_closed()
            || before_revision.run_set_revision().checked_add(1)
                != Some(after_revision.run_set_revision())
            || before_parts.attempt_history != after_parts.attempt_history
            || before_parts.dispatch_fence_high_water != after_parts.dispatch_fence_high_water
            || before_parts.material_commitment != after_parts.material_commitment
            || before_parts.credential_commitment != after_parts.credential_commitment
            || before_parts.use_fence_commitment != after_parts.use_fence_commitment
            || before_parts.health != after_parts.health
            || outcome.provider_io_operations() != 0
        {
            return Err(ClassificationErrorV1::Stage4CancellationPublicationMismatch);
        }
        let intent_id = before.intent().id();
        let before_store_head = before.state_binding().store_head_id();
        let after_store_head = after.state_binding().store_head_id();
        let after_generation = after.state_binding().store_generation_id();
        let origin_commitment = before
            .intent()
            .origin()
            .commitment()
            .map_err(|_| ClassificationErrorV1::Stage4CancellationPublicationMismatch)?;
        let effect_intent_home_id = stage4_cancellation_fact(
            b"effect-intent-home",
            vec![
                bytes(intent_id.as_bytes()),
                bytes(&before.intent().home_commitment()),
            ],
        )?;
        let effect_origin_route_id = stage4_cancellation_fact(
            b"effect-origin-route",
            vec![bytes(intent_id.as_bytes()), bytes(&origin_commitment)],
        )?;
        let control_head_id = stage4_cancellation_fact(
            b"control-head",
            vec![bytes(outcome.control_head().as_bytes())],
        )?;
        let control_revision_id = stage4_cancellation_fact(
            b"control-revision",
            vec![bytes(outcome.control_revision().as_bytes())],
        )?;
        let live_dispatch_none_proof_id = stage4_cancellation_fact(
            b"live-dispatch-none",
            vec![bytes(after_revision.id().as_bytes())],
        )?;
        let attempt_absence_proof_id = stage4_cancellation_fact(
            b"live-attempt-absent",
            vec![bytes(after_revision.id().as_bytes())],
        )?;
        let run_closure_proof_id = stage4_cancellation_fact(
            b"run-closure",
            vec![
                bytes(after_revision.id().as_bytes()),
                CborValue::Unsigned(after_revision.run_set_revision()),
            ],
        )?;
        let authority_basis_id = stage4_cancellation_fact(
            b"authority-basis-committed-by-owner-publication",
            vec![
                bytes(after_store_head.as_bytes()),
                bytes(after_generation.as_bytes()),
            ],
        )?;
        let capacity_debit_id = stage4_cancellation_fact(
            b"capacity-debit-committed-by-owner-publication",
            vec![
                bytes(after_store_head.as_bytes()),
                bytes(after_generation.as_bytes()),
            ],
        )?;
        let expected_old_head_id = stage4_cancellation_fact(
            b"expected-old-control-head",
            vec![
                bytes(before_store_head.as_bytes()),
                bytes(before.control_head().id().as_bytes()),
                bytes(before_revision.id().as_bytes()),
            ],
        )?;
        let withdrawal_receipt_id = stage4_cancellation_fact(
            b"atomic-withdrawal-publication",
            vec![
                bytes(after_store_head.as_bytes()),
                bytes(outcome.control_head().as_bytes()),
                bytes(outcome.control_revision().as_bytes()),
            ],
        )?;
        let result_id = stage4_cancellation_fact(b"result", vec![bytes(&result_commitment)])?;
        let idempotency_id =
            stage4_cancellation_fact(b"idempotency", vec![bytes(&idempotency_commitment)])?;
        let commitments = [
            effect_intent_home_id,
            effect_origin_route_id,
            control_head_id,
            control_revision_id,
            live_dispatch_none_proof_id,
            attempt_absence_proof_id,
            run_closure_proof_id,
            authority_basis_id,
            capacity_debit_id,
            expected_old_head_id,
            withdrawal_receipt_id,
            result_id,
            idempotency_id,
        ];
        if commitments.iter().collect::<BTreeSet<_>>().len() != commitments.len() {
            return Err(ClassificationErrorV1::IncompleteOrDuplicateCausalJoin);
        }
        let value = CborValue::Array(
            [
                source_id.canonical_value(),
                target_id.canonical_value(),
                CborValue::Unsigned(state.tag()),
            ]
            .into_iter()
            .chain(
                commitments
                    .into_iter()
                    .map(MigrationDigestV1::canonical_value),
            )
            .collect(),
        );
        let id = MigrationDigestV1::identify(CANCELLATION_JOIN_DOMAIN_V1, &value)?;
        Ok(Self {
            source_id,
            target_id,
            state,
            effect_intent_home_id,
            effect_origin_route_id,
            control_head_id,
            control_revision_id,
            live_dispatch_none_proof_id,
            attempt_absence_proof_id,
            run_closure_proof_id,
            authority_basis_id,
            capacity_debit_id,
            expected_old_head_id,
            withdrawal_receipt_id,
            result_id,
            idempotency_id,
            id,
        })
    }

    #[cfg(test)]
    fn test_only_bound_publication(
        source_id: MigrationDigestV1,
        target_id: MigrationDigestV1,
        publication_consumption_id: MigrationDigestV1,
    ) -> Self {
        let digest =
            |byte| MigrationDigestV1::from_digest([byte; 32]).expect("nonzero test-only H3 digest");
        let mut join = Self {
            source_id,
            target_id,
            state: NativeCancellationStateV1::Prepared,
            effect_intent_home_id: digest(101),
            effect_origin_route_id: digest(102),
            control_head_id: digest(103),
            control_revision_id: digest(104),
            live_dispatch_none_proof_id: digest(105),
            attempt_absence_proof_id: digest(106),
            run_closure_proof_id: digest(107),
            authority_basis_id: digest(108),
            capacity_debit_id: digest(109),
            expected_old_head_id: digest(110),
            withdrawal_receipt_id: publication_consumption_id,
            result_id: digest(112),
            idempotency_id: digest(113),
            id: digest(114),
        };
        join.id = MigrationDigestV1::identify(CANCELLATION_JOIN_DOMAIN_V1, &join.canonical_value())
            .expect("test-only H3 identity");
        join
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub(super) const fn source_id(&self) -> MigrationDigestV1 {
        self.source_id
    }

    pub(super) const fn target_id(&self) -> MigrationDigestV1 {
        self.target_id
    }

    pub(super) const fn publication_consumption_id(&self) -> MigrationDigestV1 {
        self.withdrawal_receipt_id
    }

    pub(super) const fn authority_basis_id(&self) -> MigrationDigestV1 {
        self.authority_basis_id
    }

    pub(super) const fn result_id(&self) -> MigrationDigestV1 {
        self.result_id
    }

    pub(super) const fn idempotency_id(&self) -> MigrationDigestV1 {
        self.idempotency_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.source_id.canonical_value(),
            self.target_id.canonical_value(),
            CborValue::Unsigned(self.state.tag()),
            self.effect_intent_home_id.canonical_value(),
            self.effect_origin_route_id.canonical_value(),
            self.control_head_id.canonical_value(),
            self.control_revision_id.canonical_value(),
            self.live_dispatch_none_proof_id.canonical_value(),
            self.attempt_absence_proof_id.canonical_value(),
            self.run_closure_proof_id.canonical_value(),
            self.authority_basis_id.canonical_value(),
            self.capacity_debit_id.canonical_value(),
            self.expected_old_head_id.canonical_value(),
            self.withdrawal_receipt_id.canonical_value(),
            self.result_id.canonical_value(),
            self.idempotency_id.canonical_value(),
            self.id.canonical_value(),
        ])
    }
}

fn stage4_cancellation_fact(
    label: &[u8],
    fields: Vec<CborValue>,
) -> Result<MigrationDigestV1, MigrationIdentityErrorV1> {
    MigrationDigestV1::identify(
        CANCELLATION_FACT_DOMAIN_V1,
        &CborValue::Array(
            std::iter::once(CborValue::Bytes(label.to_vec()))
                .chain(fields)
                .collect(),
        ),
    )
}

fn bytes(value: &[u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancellationClassificationV1 {
    NotCancellationLike,
    CancelLikeLabelNonPromoting,
    NativeCancelled(Box<NativeCancellationCausalJoinV1>),
}

impl CancellationClassificationV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::NotCancellationLike => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::CancelLikeLabelNonPromoting => CborValue::Array(vec![CborValue::Unsigned(2)]),
            Self::NativeCancelled(join) => {
                CborValue::Array(vec![CborValue::Unsigned(3), join.canonical_value()])
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceClassificationV1 {
    source_id: MigrationDigestV1,
    disposition: MigrationDispositionV1,
    reason_id: MigrationDigestV1,
    target_id: Option<MigrationDigestV1>,
    quarantine_entry_id: Option<MigrationDigestV1>,
    authority_bearing_source: bool,
    cancellation: CancellationClassificationV1,
}

impl SourceClassificationV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "classification is a closed exact source disposition"
    )]
    pub fn new(
        source_id: MigrationDigestV1,
        disposition: MigrationDispositionV1,
        reason_id: MigrationDigestV1,
        target_id: Option<MigrationDigestV1>,
        quarantine_entry_id: Option<MigrationDigestV1>,
        authority_bearing_source: bool,
        cancellation: CancellationClassificationV1,
    ) -> Result<Self, ClassificationErrorV1> {
        if source_id.as_bytes() == &[0; 32]
            || reason_id.as_bytes() == &[0; 32]
            || target_id.is_some_and(|id| id.as_bytes() == &[0; 32])
            || quarantine_entry_id.is_some_and(|id| id.as_bytes() == &[0; 32])
        {
            return Err(ClassificationErrorV1::ZeroIdentity);
        }
        if disposition.requires_target() != target_id.is_some() {
            return Err(ClassificationErrorV1::TargetCardinalityMismatch);
        }
        if matches!(disposition, MigrationDispositionV1::Quarantined)
            != quarantine_entry_id.is_some()
        {
            return Err(ClassificationErrorV1::QuarantineCardinalityMismatch);
        }
        if authority_bearing_source
            && matches!(disposition, MigrationDispositionV1::MappedNormative)
        {
            return Err(ClassificationErrorV1::AuthorityPromotionForbidden);
        }
        match (&cancellation, disposition) {
            (
                CancellationClassificationV1::NativeCancelled(join),
                MigrationDispositionV1::MappedNormative,
            ) if target_id.is_some_and(|target| {
                join.source_id() == source_id && join.target_id() == target
            }) => {}
            (
                CancellationClassificationV1::NativeCancelled(_),
                MigrationDispositionV1::MappedNormative,
            ) => return Err(ClassificationErrorV1::CancellationJoinRowMismatch),
            (CancellationClassificationV1::NativeCancelled(_), _) => {
                return Err(ClassificationErrorV1::NativeCancellationRequiresNormativeMapping);
            }
            (
                CancellationClassificationV1::CancelLikeLabelNonPromoting,
                MigrationDispositionV1::MappedNormative,
            ) => {
                return Err(ClassificationErrorV1::CancellationLabelCannotPromote);
            }
            _ => {}
        }
        Ok(Self {
            source_id,
            disposition,
            reason_id,
            target_id,
            quarantine_entry_id,
            authority_bearing_source,
            cancellation,
        })
    }

    pub const fn source_id(&self) -> MigrationDigestV1 {
        self.source_id
    }

    pub const fn disposition(&self) -> MigrationDispositionV1 {
        self.disposition
    }

    pub const fn reason_id(&self) -> MigrationDigestV1 {
        self.reason_id
    }

    pub const fn target_id(&self) -> Option<MigrationDigestV1> {
        self.target_id
    }

    pub const fn quarantine_entry_id(&self) -> Option<MigrationDigestV1> {
        self.quarantine_entry_id
    }

    pub const fn authority_bearing_source(&self) -> bool {
        self.authority_bearing_source
    }

    pub const fn cancellation(&self) -> &CancellationClassificationV1 {
        &self.cancellation
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.source_id.canonical_value(),
            CborValue::Unsigned(self.disposition.tag()),
            self.reason_id.canonical_value(),
            CborValue::optional(self.target_id.map(MigrationDigestV1::canonical_value)),
            CborValue::optional(
                self.quarantine_entry_id
                    .map(MigrationDigestV1::canonical_value),
            ),
            CborValue::Bool(self.authority_bearing_source),
            self.cancellation.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationSetV1 {
    inventory_id: MigrationDigestV1,
    rows: Vec<SourceClassificationV1>,
    id: MigrationDigestV1,
}

impl ClassificationSetV1 {
    pub fn new(
        inventory: &ByteTotalInventoryV1,
        mut rows: Vec<SourceClassificationV1>,
    ) -> Result<Self, ClassificationErrorV1> {
        if inventory.id().as_bytes() == &[0; 32] {
            return Err(ClassificationErrorV1::ZeroIdentity);
        }
        if rows.is_empty() || rows.len() > MAX_CLASSIFICATIONS_V1 {
            return Err(ClassificationErrorV1::InvalidClassificationCount);
        }
        rows.sort_by_key(SourceClassificationV1::source_id);
        if rows
            .windows(2)
            .any(|pair| pair[0].source_id == pair[1].source_id)
        {
            return Err(ClassificationErrorV1::DuplicateSourceClassification);
        }
        if rows.len() != inventory.rows().len()
            || rows
                .iter()
                .zip(inventory.rows())
                .any(|(classification, source)| classification.source_id != source.source_id())
        {
            return Err(ClassificationErrorV1::ClassificationCoverageMismatch);
        }
        for (classification, source) in rows.iter().zip(inventory.rows()) {
            if matches!(
                source.kind(),
                InventoryNodeKindV1::UnavailablePreexistingLoss
            ) != matches!(
                classification.disposition,
                MigrationDispositionV1::UnavailablePreexistingLoss
            ) {
                return Err(ClassificationErrorV1::UnavailableDispositionMismatch);
            }
        }
        let cancellation_publications = rows
            .iter()
            .filter_map(|row| match &row.cancellation {
                CancellationClassificationV1::NativeCancelled(join) => {
                    Some(join.publication_consumption_id())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if cancellation_publications
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != cancellation_publications.len()
        {
            return Err(ClassificationErrorV1::Stage4PublicationReused);
        }
        let id = MigrationDigestV1::identify(
            CLASSIFICATION_IDENTITY_DOMAIN_V1,
            &CborValue::Array(vec![
                inventory.id().canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .map(SourceClassificationV1::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            inventory_id: inventory.id(),
            rows,
            id,
        })
    }

    pub const fn inventory_id(&self) -> MigrationDigestV1 {
        self.inventory_id
    }

    pub fn rows(&self) -> &[SourceClassificationV1] {
        &self.rows
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub fn native_cancellation_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| {
                matches!(
                    row.cancellation(),
                    CancellationClassificationV1::NativeCancelled(_)
                )
            })
            .count()
    }

    pub fn row(&self, source_id: MigrationDigestV1) -> Option<&SourceClassificationV1> {
        self.rows
            .binary_search_by_key(&source_id, SourceClassificationV1::source_id)
            .ok()
            .map(|index| &self.rows[index])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IdentityMappingBasisV1 {
    PreserveStable {
        exact_identity_proof_id: MigrationDigestV1,
    },
    ExplicitOwnerMap {
        owner_schema_id: MigrationDigestV1,
        semantic_join_id: MigrationDigestV1,
    },
    HistoricalOpaque {
        preservation_proof_id: MigrationDigestV1,
    },
}

impl IdentityMappingBasisV1 {
    fn contains_zero(self) -> bool {
        match self {
            Self::PreserveStable {
                exact_identity_proof_id,
            } => exact_identity_proof_id.as_bytes() == &[0; 32],
            Self::ExplicitOwnerMap {
                owner_schema_id,
                semantic_join_id,
            } => owner_schema_id.as_bytes() == &[0; 32] || semantic_join_id.as_bytes() == &[0; 32],
            Self::HistoricalOpaque {
                preservation_proof_id,
            } => preservation_proof_id.as_bytes() == &[0; 32],
        }
    }

    fn canonical_value(self) -> CborValue {
        match self {
            Self::PreserveStable {
                exact_identity_proof_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                exact_identity_proof_id.canonical_value(),
            ]),
            Self::ExplicitOwnerMap {
                owner_schema_id,
                semantic_join_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                owner_schema_id.canonical_value(),
                semantic_join_id.canonical_value(),
            ]),
            Self::HistoricalOpaque {
                preservation_proof_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                preservation_proof_id.canonical_value(),
            ]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityMapEntryV1 {
    source_id: MigrationDigestV1,
    target_id: MigrationDigestV1,
    basis: IdentityMappingBasisV1,
}

impl IdentityMapEntryV1 {
    pub fn new(
        source_id: MigrationDigestV1,
        target_id: MigrationDigestV1,
        basis: IdentityMappingBasisV1,
    ) -> Result<Self, ClassificationErrorV1> {
        if source_id.as_bytes() == &[0; 32]
            || target_id.as_bytes() == &[0; 32]
            || basis.contains_zero()
        {
            return Err(ClassificationErrorV1::ZeroIdentity);
        }
        Ok(Self {
            source_id,
            target_id,
            basis,
        })
    }

    pub const fn source_id(self) -> MigrationDigestV1 {
        self.source_id
    }

    pub const fn target_id(self) -> MigrationDigestV1 {
        self.target_id
    }

    pub const fn basis(self) -> IdentityMappingBasisV1 {
        self.basis
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            self.source_id.canonical_value(),
            self.target_id.canonical_value(),
            self.basis.canonical_value(),
        ])
    }

    pub(super) fn h3_custody_transition(
        self,
    ) -> Result<MigrationDigestV1, MigrationIdentityErrorV1> {
        MigrationDigestV1::identify(H3_CUSTODY_TRANSITION_DOMAIN_V1, &self.canonical_value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicIdentityMapV1 {
    classification_set_id: MigrationDigestV1,
    rows: Vec<IdentityMapEntryV1>,
    id: MigrationDigestV1,
}

impl DeterministicIdentityMapV1 {
    pub fn new(
        classifications: &ClassificationSetV1,
        mut rows: Vec<IdentityMapEntryV1>,
    ) -> Result<Self, ClassificationErrorV1> {
        rows.sort_by_key(|row| row.source_id);
        if classifications.id().as_bytes() == &[0; 32]
            || rows.iter().any(|row| {
                row.source_id.as_bytes() == &[0; 32] || row.target_id.as_bytes() == &[0; 32]
            })
        {
            return Err(ClassificationErrorV1::ZeroIdentity);
        }
        if rows
            .windows(2)
            .any(|pair| pair[0].source_id == pair[1].source_id)
        {
            return Err(ClassificationErrorV1::DuplicateIdentityMapping);
        }
        if rows
            .iter()
            .map(|row| row.target_id)
            .collect::<BTreeSet<_>>()
            .len()
            != rows.len()
        {
            return Err(ClassificationErrorV1::TargetIdentityReuse);
        }
        let expected = classifications
            .rows()
            .iter()
            .filter_map(|row| row.target_id().map(|target| (row.source_id(), target)))
            .collect::<BTreeMap<_, _>>();
        let observed = rows
            .iter()
            .map(|row| (row.source_id, row.target_id))
            .collect::<BTreeMap<_, _>>();
        if expected != observed {
            return Err(ClassificationErrorV1::IdentityMapCoverageMismatch);
        }
        for row in &rows {
            let classification = classifications
                .row(row.source_id)
                .ok_or(ClassificationErrorV1::IdentityMapCoverageMismatch)?;
            match (classification.disposition(), row.basis) {
                (
                    MigrationDispositionV1::MappedNormative,
                    IdentityMappingBasisV1::HistoricalOpaque { .. },
                )
                | (
                    MigrationDispositionV1::OpaquePreserved,
                    IdentityMappingBasisV1::ExplicitOwnerMap { .. },
                ) => return Err(ClassificationErrorV1::IdentityMappingBasisMismatch),
                _ => {}
            }
        }
        let id = MigrationDigestV1::identify(
            IDENTITY_MAP_DOMAIN_V1,
            &CborValue::Array(vec![
                classifications.id().canonical_value(),
                CborValue::Array(
                    rows.iter()
                        .copied()
                        .map(IdentityMapEntryV1::canonical_value)
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            classification_set_id: classifications.id(),
            rows,
            id,
        })
    }

    pub const fn classification_set_id(&self) -> MigrationDigestV1 {
        self.classification_set_id
    }

    pub fn rows(&self) -> &[IdentityMapEntryV1] {
        &self.rows
    }

    pub fn row(&self, source_id: MigrationDigestV1) -> Option<IdentityMapEntryV1> {
        self.rows
            .binary_search_by_key(&source_id, |row| row.source_id)
            .ok()
            .map(|index| self.rows[index])
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClassificationErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("migration classification count is outside the finite v1 bounds")]
    InvalidClassificationCount,
    #[error("every inventory source must have exactly one classification")]
    ClassificationCoverageMismatch,
    #[error("a source has more than one migration classification")]
    DuplicateSourceClassification,
    #[error("unavailable source and classification disposition do not match")]
    UnavailableDispositionMismatch,
    #[error("mapped disposition requires exactly one target identity")]
    TargetCardinalityMismatch,
    #[error("quarantined disposition requires exactly one quarantine-entry identity")]
    QuarantineCardinalityMismatch,
    #[error("legacy authority-bearing material cannot become normative")]
    AuthorityPromotionForbidden,
    #[error("native cancellation admission requires normative mapping")]
    NativeCancellationRequiresNormativeMapping,
    #[error("native cancellation publication is not bound to this exact source and target row")]
    CancellationJoinRowMismatch,
    #[error("cancel-like labels cannot promote without a unique complete H3 causal join")]
    CancellationLabelCannotPromote,
    #[error("native cancellation causal join is incomplete, zero, or duplicated")]
    IncompleteOrDuplicateCausalJoin,
    #[error("Stage-4 cancellation source is not prepared or confirmed_not_applied")]
    Stage4CancellationSourceNotEligible,
    #[error("Stage-4 cancellation facts do not form one atomic owner publication")]
    Stage4CancellationPublicationMismatch,
    #[error("one Stage-4 cancellation publication cannot be consumed by multiple source rows")]
    Stage4PublicationReused,
    #[error("identity map does not equal the complete classified target set")]
    IdentityMapCoverageMismatch,
    #[error("source identity appears more than once in the identity map")]
    DuplicateIdentityMapping,
    #[error("a target identity cannot be inferred as the image of multiple legacy sources")]
    TargetIdentityReuse,
    #[error("classification and identity-map identities must not be zero")]
    ZeroIdentity,
    #[error("identity mapping basis conflicts with the source disposition")]
    IdentityMappingBasisMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::migration::runtime::{
        DeclaredRootV1, InventoryDomainV1, InventoryPayloadV1, InventoryRowV1, NormalizedLocatorV1,
    };

    fn digest(byte: u8) -> MigrationDigestV1 {
        MigrationDigestV1::from_digest([byte; 32]).expect("nonzero test digest")
    }

    fn cancellation_inventory() -> ByteTotalInventoryV1 {
        let root_locator = NormalizedLocatorV1::new(b"/stage11/h3".to_vec()).expect("root locator");
        let root = DeclaredRootV1::new(
            root_locator.clone(),
            root_locator,
            InventoryDomainV1::Repository,
            false,
        )
        .expect("declared root");
        let rows = [b"first".as_slice(), b"second".as_slice()]
            .into_iter()
            .enumerate()
            .map(|(index, bytes)| {
                let locator =
                    NormalizedLocatorV1::new(format!("/stage11/h3/{}", index + 1).into_bytes())
                        .expect("row locator");
                InventoryRowV1::new(
                    root.id(),
                    locator.clone(),
                    locator,
                    InventoryDomainV1::Repository,
                    InventoryNodeKindV1::RegularFile,
                    InventoryPayloadV1::from_bytes(bytes).expect("payload"),
                    digest(10 + index as u8),
                )
                .expect("inventory row")
            })
            .collect();
        ByteTotalInventoryV1::new(vec![root], rows).expect("inventory")
    }

    #[test]
    fn h3_join_is_bound_to_exact_source_and_target() {
        let inventory = cancellation_inventory();
        let source = inventory.rows()[0].source_id();
        let target = digest(20);
        let join =
            NativeCancellationCausalJoinV1::test_only_bound_publication(source, target, digest(30));
        assert!(matches!(
            SourceClassificationV1::new(
                inventory.rows()[1].source_id(),
                MigrationDispositionV1::MappedNormative,
                digest(31),
                Some(target),
                None,
                false,
                CancellationClassificationV1::NativeCancelled(Box::new(join)),
            ),
            Err(ClassificationErrorV1::CancellationJoinRowMismatch)
        ));
    }

    #[test]
    fn h3_publication_can_be_consumed_by_only_one_classification_row() {
        let inventory = cancellation_inventory();
        let publication = digest(40);
        let rows = inventory
            .rows()
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let target = digest(50 + index as u8);
                let join = NativeCancellationCausalJoinV1::test_only_bound_publication(
                    source.source_id(),
                    target,
                    publication,
                );
                SourceClassificationV1::new(
                    source.source_id(),
                    MigrationDispositionV1::MappedNormative,
                    digest(60 + index as u8),
                    Some(target),
                    None,
                    false,
                    CancellationClassificationV1::NativeCancelled(Box::new(join)),
                )
                .expect("row-bound cancellation classification")
            })
            .collect();
        assert!(matches!(
            ClassificationSetV1::new(&inventory, rows),
            Err(ClassificationErrorV1::Stage4PublicationReused)
        ));
    }
}
