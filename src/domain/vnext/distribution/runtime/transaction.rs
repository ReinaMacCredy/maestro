use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::authority::{
    ActionAuthorityBasisKindV1, ActionRequestIdV1, AuthorizationReceiptV1,
};
use crate::domain::vnext::distribution::{CommitmentV1, ReleaseIdV1};
use crate::domain::vnext::execution::EffectIntentIdV1;
use crate::domain::vnext::identity::SchemaIdV1;
use crate::domain::vnext::integration::public_literals::{
    ActionAuthorityBasisV1 as PublicActionAuthorityBasisV1, ActionRequestV1, OperationRequestV1,
    OrchestrationAttributionV1,
};
use crate::domain::vnext::persistence::StoreObjectV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{
    CustodyAssessmentV1, DistributionDomainKindV1, DistributionDomainRefV1,
    DistributionModelErrorV1, DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
    DistributionSnapshotTargetV1,
};

const MAX_TRANSACTION_TARGETS_V1: usize = 65_535;
const ACTION_REQUEST_SCHEMA_ID_V1: &str =
    "73f173d3654625a19278aa6c413714b04349f5d2500924ff0f780168a713192d";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DistributionActionV1 {
    ReserveDistributionTargets,
    AdoptManagedRegion,
    TransferWholeFileCustody,
    BeginDistributionTransaction,
    CaptureDistributionBeforeState,
    StageDistributionCandidate,
    ReserveDistributionEffect,
    PublishDistributionOccurrence,
    VerifyDistributionTarget,
    CommitDistributionTransaction,
    RecoverDistributionTransaction,
    RollbackDistributionTransaction,
    ActivateBinarySlot,
}

impl DistributionActionV1 {
    pub const ALL: [Self; 13] = [
        Self::ReserveDistributionTargets,
        Self::AdoptManagedRegion,
        Self::TransferWholeFileCustody,
        Self::BeginDistributionTransaction,
        Self::CaptureDistributionBeforeState,
        Self::StageDistributionCandidate,
        Self::ReserveDistributionEffect,
        Self::PublishDistributionOccurrence,
        Self::VerifyDistributionTarget,
        Self::CommitDistributionTransaction,
        Self::RecoverDistributionTransaction,
        Self::RollbackDistributionTransaction,
        Self::ActivateBinarySlot,
    ];

    pub const fn global_tag(self) -> u64 {
        117 + self as u64
    }

    pub const fn local_tag(self) -> u64 {
        1 + self as u64
    }

    pub const fn owner_tag(self) -> u64 {
        20
    }

    pub const fn literal(self) -> &'static str {
        match self {
            Self::ReserveDistributionTargets => "ReserveDistributionTargets",
            Self::AdoptManagedRegion => "AdoptManagedRegion",
            Self::TransferWholeFileCustody => "TransferWholeFileCustody",
            Self::BeginDistributionTransaction => "BeginDistributionTransaction",
            Self::CaptureDistributionBeforeState => "CaptureDistributionBeforeState",
            Self::StageDistributionCandidate => "StageDistributionCandidate",
            Self::ReserveDistributionEffect => "ReserveDistributionEffect",
            Self::PublishDistributionOccurrence => "PublishDistributionOccurrence",
            Self::VerifyDistributionTarget => "VerifyDistributionTarget",
            Self::CommitDistributionTransaction => "CommitDistributionTransaction",
            Self::RecoverDistributionTransaction => "RecoverDistributionTransaction",
            Self::RollbackDistributionTransaction => "RollbackDistributionTransaction",
            Self::ActivateBinarySlot => "ActivateBinarySlot",
        }
    }

    pub const fn descriptor_id(self) -> &'static str {
        match self {
            Self::ReserveDistributionTargets => {
                "bc76796a22070cb8fb343db3db372a49466dca1d20a7f8c8ce937b804b921b19"
            }
            Self::AdoptManagedRegion => {
                "b177e658a470818c2a53b8320da3bb0bb6bb19ab9bc6c53d02dbdf741671b237"
            }
            Self::TransferWholeFileCustody => {
                "f5c1eadeed317406bb964e308ac1b2d7dfae486835e4c9f39b2175655df2b708"
            }
            Self::BeginDistributionTransaction => {
                "eb0e7e41678e1218a2b681a9a63852fd6943638a6283ecbbc3f9f8dc0d45a6db"
            }
            Self::CaptureDistributionBeforeState => {
                "e064258d290cb25d72c4a3ecbe14caf7667b3138914bb779e79e4250686df91e"
            }
            Self::StageDistributionCandidate => {
                "0d0719afecf607ffc6eb501ed44c708b287f723c939a40d82957aac744d03e88"
            }
            Self::ReserveDistributionEffect => {
                "7c90d933fe22345895ec04e9fe9b32061839c5a68aa42c46676c1d53249e8d13"
            }
            Self::PublishDistributionOccurrence => {
                "a83398caf01d4a0c9d5f2b42a71f4b74abd6b73b4763ba7232a83d0f01be98ab"
            }
            Self::VerifyDistributionTarget => {
                "b5424286e0fbee75cc164d22e749896f51a57cad81b9d9a544c3decb36262fbf"
            }
            Self::CommitDistributionTransaction => {
                "f571d2fdf0e37b959fdf41651f79b086fc3a3ccea5767cbc45ec833bd3026531"
            }
            Self::RecoverDistributionTransaction => {
                "a39fff81e6ba55bf79bcb8c0002cd8d1acd406bc9a95aa700f23f0cd68bfed7f"
            }
            Self::RollbackDistributionTransaction => {
                "8e07516549d8e73405f63ab75a9e835477e758a6b84e2282b9b03a92d84ac11f"
            }
            Self::ActivateBinarySlot => {
                "d2e6ce9efda2b399a9c9457bb151e6494bfb2b48d14c4ec8fc6c52c0feecb637"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionMutationKindV1 {
    Install,
    Update,
    Repair,
    Migrate,
    Rollback,
    Uninstall,
}

impl DistributionMutationKindV1 {
    pub const fn numeric_tag(self) -> u64 {
        1 + self as u64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetEffectKindV1 {
    CreateVacantTarget,
    RewriteOwnedTarget,
    RewriteManagedBlock,
    RemoveOwnedTarget,
    AdoptManagedRegion,
    TransferWholeFileCustody,
    ActivateBinarySlot,
}

impl TargetEffectKindV1 {
    pub const fn numeric_tag(self) -> u64 {
        1 + self as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionPlanTargetV1 {
    pub target_tag: u64,
    pub target_identity_ref: DistributionScopedObjectRefV1,
    pub target_identity: CommitmentV1,
    pub custody: CustodyAssessmentV1,
    pub expected_preimage_commitment: CommitmentV1,
    pub candidate_commitment: Option<CommitmentV1>,
    pub effect_kind: TargetEffectKindV1,
    pub outside_prefix_commitment: Option<CommitmentV1>,
    pub outside_suffix_commitment: Option<CommitmentV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionPlanV1 {
    domain: DistributionDomainRefV1,
    mutation_kind: DistributionMutationKindV1,
    request_id: ActionRequestIdV1,
    request_or_ceremony_ref: DistributionScopedObjectRefV1,
    plan_ref: DistributionScopedObjectRefV1,
    idempotency_key_ref: DistributionScopedObjectRefV1,
    release_id: Option<ReleaseIdV1>,
    prior_commit_ref: Option<DistributionScopedObjectRefV1>,
    prior_receipt_ref: Option<DistributionScopedObjectRefV1>,
    selected_rollback_ref: Option<DistributionScopedObjectRefV1>,
    targets: Vec<DistributionPlanTargetV1>,
    meaning_digest: CommitmentV1,
}

impl DistributionPlanV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the frozen Distribution plan binds every authority and currentness dimension"
    )]
    pub fn new(
        domain: DistributionDomainRefV1,
        mutation_kind: DistributionMutationKindV1,
        request_id: ActionRequestIdV1,
        request_or_ceremony_ref: DistributionScopedObjectRefV1,
        plan_ref: DistributionScopedObjectRefV1,
        idempotency_key_ref: DistributionScopedObjectRefV1,
        release_id: Option<ReleaseIdV1>,
        prior_commit_ref: Option<DistributionScopedObjectRefV1>,
        prior_receipt_ref: Option<DistributionScopedObjectRefV1>,
        selected_rollback_ref: Option<DistributionScopedObjectRefV1>,
        targets: Vec<DistributionPlanTargetV1>,
    ) -> Result<Self, DistributionTransactionErrorV1> {
        validate_plan_refs(
            &domain,
            &request_or_ceremony_ref,
            &plan_ref,
            &idempotency_key_ref,
            prior_commit_ref.as_ref(),
            prior_receipt_ref.as_ref(),
            selected_rollback_ref.as_ref(),
        )?;
        validate_release_binding(&domain, release_id)?;
        if request_id.as_bytes() == &[0; 32]
            || targets.is_empty()
            || targets.len() > MAX_TRANSACTION_TARGETS_V1
        {
            return Err(DistributionTransactionErrorV1::InvalidPlan);
        }
        if (mutation_kind == DistributionMutationKindV1::Rollback)
            != selected_rollback_ref.is_some()
        {
            return Err(DistributionTransactionErrorV1::RollbackSelectionMismatch);
        }
        let mut prior_tag = 0;
        let mut identities = BTreeSet::new();
        for target in &targets {
            target.target_identity_ref.require_same_domain(&domain)?;
            target
                .target_identity_ref
                .require_kind(DistributionRuntimeObjectKindV1::CanonicalTargetIdentity)?;
            if target.target_tag <= prior_tag
                || target.target_identity.as_bytes() == &[0; 32]
                || target.expected_preimage_commitment.as_bytes() == &[0; 32]
                || !identities.insert(target.target_identity)
                || !target.custody.permits_mutation()
            {
                return Err(DistributionTransactionErrorV1::InvalidPlanTarget);
            }
            if target.effect_kind == TargetEffectKindV1::RewriteManagedBlock {
                if target.outside_prefix_commitment.is_none()
                    || target.outside_suffix_commitment.is_none()
                {
                    return Err(DistributionTransactionErrorV1::MissingOutsideByteFence);
                }
            } else if target.outside_prefix_commitment.is_some()
                || target.outside_suffix_commitment.is_some()
            {
                return Err(DistributionTransactionErrorV1::UnexpectedOutsideByteFence);
            }
            let removal = target.effect_kind == TargetEffectKindV1::RemoveOwnedTarget;
            if removal == target.candidate_commitment.is_some() {
                return Err(DistributionTransactionErrorV1::InvalidPlanTarget);
            }
            prior_tag = target.target_tag;
        }
        let value = plan_value(
            &domain,
            mutation_kind,
            request_id,
            &request_or_ceremony_ref,
            &plan_ref,
            &idempotency_key_ref,
            release_id,
            prior_commit_ref.as_ref(),
            prior_receipt_ref.as_ref(),
            selected_rollback_ref.as_ref(),
            &targets,
        )?;
        let meaning_digest =
            CommitmentV1::from_bytes(Sha256::digest(deterministic_cbor::encode(&value)?).into());
        Ok(Self {
            domain,
            mutation_kind,
            request_id,
            request_or_ceremony_ref,
            plan_ref,
            idempotency_key_ref,
            release_id,
            prior_commit_ref,
            prior_receipt_ref,
            selected_rollback_ref,
            targets,
            meaning_digest,
        })
    }

    pub const fn domain(&self) -> &DistributionDomainRefV1 {
        &self.domain
    }

    pub const fn mutation_kind(&self) -> DistributionMutationKindV1 {
        self.mutation_kind
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn request_or_ceremony_ref(&self) -> &DistributionScopedObjectRefV1 {
        &self.request_or_ceremony_ref
    }

    pub const fn plan_ref(&self) -> &DistributionScopedObjectRefV1 {
        &self.plan_ref
    }

    pub const fn idempotency_key_ref(&self) -> &DistributionScopedObjectRefV1 {
        &self.idempotency_key_ref
    }

    pub const fn release_id(&self) -> Option<ReleaseIdV1> {
        self.release_id
    }

    pub const fn prior_commit_ref(&self) -> Option<&DistributionScopedObjectRefV1> {
        self.prior_commit_ref.as_ref()
    }

    pub const fn prior_receipt_ref(&self) -> Option<&DistributionScopedObjectRefV1> {
        self.prior_receipt_ref.as_ref()
    }

    pub const fn selected_rollback_ref(&self) -> Option<&DistributionScopedObjectRefV1> {
        self.selected_rollback_ref.as_ref()
    }

    pub fn targets(&self) -> &[DistributionPlanTargetV1] {
        &self.targets
    }

    pub const fn meaning_digest(&self) -> CommitmentV1 {
        self.meaning_digest
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        plan_value(
            &self.domain,
            self.mutation_kind,
            self.request_id,
            &self.request_or_ceremony_ref,
            &self.plan_ref,
            &self.idempotency_key_ref,
            self.release_id,
            self.prior_commit_ref.as_ref(),
            self.prior_receipt_ref.as_ref(),
            self.selected_rollback_ref.as_ref(),
            &self.targets,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionPhaseAuthorizationV1 {
    pub action: DistributionActionV1,
    pub request_ref: DistributionScopedObjectRefV1,
    pub request: ActionRequestV1,
    receipt: AuthorizationReceiptV1,
}

impl DistributionPhaseAuthorizationV1 {
    pub fn new(
        action: DistributionActionV1,
        request_ref: DistributionScopedObjectRefV1,
        request: ActionRequestV1,
        receipt: AuthorizationReceiptV1,
    ) -> Self {
        Self {
            action,
            request_ref,
            request,
            receipt,
        }
    }

    pub const fn receipt(&self) -> &AuthorizationReceiptV1 {
        &self.receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DistributionAuthoritySetV1 {
    phases: Vec<DistributionPhaseAuthorizationV1>,
}

impl DistributionAuthoritySetV1 {
    fn admit(
        plan: &DistributionPlanV1,
        mut phases: Vec<DistributionPhaseAuthorizationV1>,
        current_objects: &[StoreObjectV1],
    ) -> Result<Self, DistributionTransactionErrorV1> {
        phases.sort_by_key(|phase| phase.action);
        let required = required_actions(plan);
        if phases.len() != required.len()
            || phases
                .iter()
                .map(|phase| phase.action)
                .collect::<BTreeSet<_>>()
                != required
        {
            return Err(DistributionTransactionErrorV1::IncompleteAuthoritySet);
        }
        let mut request_ids = BTreeSet::new();
        let mut request_object_ids = BTreeSet::new();
        let mut receipt_ids = BTreeSet::new();
        for phase in &phases {
            if !request_ids.insert(*phase.receipt.request_id().as_bytes())
                || !request_object_ids.insert(phase.request_ref.object_id())
                || !receipt_ids.insert(*phase.receipt.id().as_bytes())
            {
                return Err(DistributionTransactionErrorV1::AuthorityRequestMismatch);
            }
            validate_phase_authorization(plan, phase, current_objects)?;
        }
        let begin = phases
            .iter()
            .find(|phase| phase.action == DistributionActionV1::BeginDistributionTransaction)
            .ok_or(DistributionTransactionErrorV1::IncompleteAuthoritySet)?;
        if begin.request_ref != plan.request_or_ceremony_ref
            || begin.receipt.request_id() != plan.request_id
        {
            return Err(DistributionTransactionErrorV1::AuthorityRequestMismatch);
        }
        Ok(Self { phases })
    }

    fn receipts(&self) -> impl Iterator<Item = &AuthorizationReceiptV1> {
        self.phases.iter().map(|phase| &phase.receipt)
    }

    fn phases(&self) -> &[DistributionPhaseAuthorizationV1] {
        &self.phases
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorizedDistributionPlanV1 {
    plan: DistributionPlanV1,
    authority: DistributionAuthoritySetV1,
}

impl AuthorizedDistributionPlanV1 {
    pub(crate) fn from_current_authority(
        plan: DistributionPlanV1,
        phases: Vec<DistributionPhaseAuthorizationV1>,
        current_objects: &[StoreObjectV1],
    ) -> Result<Self, DistributionTransactionErrorV1> {
        let authority = DistributionAuthoritySetV1::admit(&plan, phases, current_objects)?;
        Ok(Self { plan, authority })
    }

    pub(crate) const fn plan(&self) -> &DistributionPlanV1 {
        &self.plan
    }

    pub(crate) fn authority_receipts(&self) -> impl Iterator<Item = &AuthorizationReceiptV1> {
        self.authority.receipts()
    }

    pub(crate) fn authority_phases(&self) -> &[DistributionPhaseAuthorizationV1] {
        self.authority.phases()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetPlanObservationV1 {
    pub target_tag: u64,
    pub target_identity: CommitmentV1,
    pub preimage_commitment: CommitmentV1,
    pub outside_prefix_commitment: Option<CommitmentV1>,
    pub outside_suffix_commitment: Option<CommitmentV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedTargetPreimageV1 {
    pub target_tag: u64,
    pub compared_preimage_commitment: CommitmentV1,
    pub snapshot_target: DistributionSnapshotTargetV1,
    pub effect_fence_commitment: CommitmentV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectCrossingDispositionV1 {
    Applied,
    DefinitelyNotApplied,
    InDoubt,
}

impl EffectCrossingDispositionV1 {
    pub const fn numeric_tag(self) -> u64 {
        1 + self as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectCrossingObservationV1 {
    pub target_tag: u64,
    pub effect_intent_id: EffectIntentIdV1,
    pub disposition: EffectCrossingDispositionV1,
    pub observed_postimage_commitment: Option<CommitmentV1>,
    pub outside_prefix_commitment: Option<CommitmentV1>,
    pub outside_suffix_commitment: Option<CommitmentV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationDispositionV1 {
    Exact,
    Mismatch,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionTransactionPhaseV1 {
    Planned,
    TargetsReserved,
    BeforeStateCaptured,
    CandidateStaged,
    EffectsReserved,
    EffectsCrossed,
    RecoveryRequired,
    Verified,
    CommitPrepared,
    Committed,
    RollingBack,
    RolledBack,
    AbortedBeforeEffects,
}

impl DistributionTransactionPhaseV1 {
    pub const fn numeric_tag(self) -> u64 {
        1 + self as u64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionRecoveryDirectiveV1 {
    AbortWithoutTargetWrite,
    ReconcileReservedEffects,
    ReconcileCrossingThenCommitOrRestore,
    RestoreCapturedPreimagesOnly,
    CompleteSameDomainPublication,
    NoRecoveryRequired,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DistributionTransactionV1 {
    authorized: AuthorizedDistributionPlanV1,
    phase: DistributionTransactionPhaseV1,
    journal_sequence: u64,
    captures: Vec<CapturedTargetPreimageV1>,
    effect_intents: Vec<(u64, EffectIntentIdV1)>,
    crossings: Vec<EffectCrossingObservationV1>,
    prepared_receipt_ref: Option<DistributionScopedObjectRefV1>,
    prepared_commit_ref: Option<DistributionScopedObjectRefV1>,
}

impl DistributionTransactionV1 {
    pub(crate) fn begin(
        authorized: AuthorizedDistributionPlanV1,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<Self, DistributionTransactionErrorV1> {
        validate_observations(authorized.plan(), &observations)?;
        Ok(Self {
            authorized,
            phase: DistributionTransactionPhaseV1::TargetsReserved,
            journal_sequence: 1,
            captures: Vec::new(),
            effect_intents: Vec::new(),
            crossings: Vec::new(),
            prepared_receipt_ref: None,
            prepared_commit_ref: None,
        })
    }

    pub const fn plan(&self) -> &DistributionPlanV1 {
        self.authorized.plan()
    }

    pub fn authority_receipts(&self) -> impl Iterator<Item = &AuthorizationReceiptV1> {
        self.authorized.authority_receipts()
    }

    pub(crate) fn revalidate_current_authority(
        &self,
        current_objects: &[StoreObjectV1],
    ) -> Result<(), DistributionTransactionErrorV1> {
        for phase in self.authorized.authority_phases() {
            validate_phase_authorization(self.plan(), phase, current_objects)?;
        }
        Ok(())
    }

    pub const fn phase(&self) -> DistributionTransactionPhaseV1 {
        self.phase
    }

    pub const fn journal_sequence(&self) -> u64 {
        self.journal_sequence
    }

    pub fn captures(&self) -> &[CapturedTargetPreimageV1] {
        &self.captures
    }

    pub fn effect_intents(&self) -> &[(u64, EffectIntentIdV1)] {
        &self.effect_intents
    }

    pub fn crossings(&self) -> &[EffectCrossingObservationV1] {
        &self.crossings
    }

    pub const fn prepared_receipt_ref(&self) -> Option<&DistributionScopedObjectRefV1> {
        self.prepared_receipt_ref.as_ref()
    }

    pub const fn prepared_commit_ref(&self) -> Option<&DistributionScopedObjectRefV1> {
        self.prepared_commit_ref.as_ref()
    }

    pub fn checkpoint_value(&self) -> Result<CborValue, DistributionTransactionErrorV1> {
        let captures = self
            .captures
            .iter()
            .map(|capture| {
                Ok(CborValue::Array(vec![
                    CborValue::Unsigned(capture.target_tag),
                    bytes(capture.compared_preimage_commitment),
                    capture
                        .snapshot_target
                        .canonical_value()
                        .map_err(|_| DistributionTransactionErrorV1::CheckpointEncoding)?,
                    bytes(capture.effect_fence_commitment),
                ]))
            })
            .collect::<Result<Vec<_>, DistributionTransactionErrorV1>>()?;
        let effects = self
            .effect_intents
            .iter()
            .map(|(tag, effect)| {
                CborValue::Array(vec![
                    CborValue::Unsigned(*tag),
                    CborValue::Bytes(effect.as_bytes().to_vec()),
                ])
            })
            .collect();
        let crossings = self
            .crossings
            .iter()
            .map(|crossing| {
                CborValue::Array(vec![
                    CborValue::Unsigned(crossing.target_tag),
                    CborValue::Bytes(crossing.effect_intent_id.as_bytes().to_vec()),
                    CborValue::Unsigned(crossing.disposition.numeric_tag()),
                    CborValue::optional(crossing.observed_postimage_commitment.map(bytes)),
                    CborValue::optional(crossing.outside_prefix_commitment.map(bytes)),
                    CborValue::optional(crossing.outside_suffix_commitment.map(bytes)),
                ])
            })
            .collect();
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.distribution-transaction-checkpoint.v1")?,
            self.plan().canonical_value()?,
            CborValue::Array(
                self.authorized
                    .authority_phases()
                    .iter()
                    .map(|phase| {
                        Ok(CborValue::Array(vec![
                            CborValue::Unsigned(phase.action.global_tag()),
                            phase.request_ref.canonical_value(),
                            phase.receipt.schema_value()?,
                        ]))
                    })
                    .collect::<Result<Vec<_>, CborError>>()?,
            ),
            CborValue::Unsigned(self.phase.numeric_tag()),
            CborValue::Unsigned(self.journal_sequence),
            CborValue::Array(captures),
            CborValue::Array(effects),
            CborValue::Array(crossings),
            optional_ref(self.prepared_receipt_ref.as_ref()),
            optional_ref(self.prepared_commit_ref.as_ref()),
        ]))
    }

    pub fn checkpoint_commitment(&self) -> Result<CommitmentV1, DistributionTransactionErrorV1> {
        Ok(CommitmentV1::from_bytes(
            Sha256::digest(deterministic_cbor::encode(&self.checkpoint_value()?)?).into(),
        ))
    }

    pub fn record_atomic_captures(
        &mut self,
        captures: Vec<CapturedTargetPreimageV1>,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::TargetsReserved)?;
        if captures.len() != self.plan().targets.len() {
            return Err(DistributionTransactionErrorV1::IncompleteTargetSet);
        }
        for (plan, capture) in self.plan().targets.iter().zip(&captures) {
            if capture.target_tag != plan.target_tag
                || capture.compared_preimage_commitment != plan.expected_preimage_commitment
                || capture.effect_fence_commitment.as_bytes() == &[0; 32]
                || capture.snapshot_target.target_tag != plan.target_tag
                || capture.snapshot_target.domain != *self.plan().domain()
                || capture.snapshot_target.canonical_target_identity_ref != plan.target_identity_ref
            {
                return Err(DistributionTransactionErrorV1::CaptureFenceMismatch);
            }
            capture
                .snapshot_target
                .validate()
                .map_err(|_| DistributionTransactionErrorV1::CaptureFenceMismatch)?;
        }
        self.captures = captures;
        self.advance(DistributionTransactionPhaseV1::BeforeStateCaptured)
    }

    pub fn record_candidate_staged(
        &mut self,
        staged_plan_digest: CommitmentV1,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::BeforeStateCaptured)?;
        if staged_plan_digest != self.plan().meaning_digest {
            return Err(DistributionTransactionErrorV1::CandidatePlanMismatch);
        }
        self.advance(DistributionTransactionPhaseV1::CandidateStaged)
    }

    pub fn record_effect_reservations(
        &mut self,
        reservations: Vec<(u64, EffectIntentIdV1)>,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::CandidateStaged)?;
        if reservations.len() != self.plan().targets.len() {
            return Err(DistributionTransactionErrorV1::IncompleteTargetSet);
        }
        let mut seen = BTreeSet::new();
        for (plan, (tag, effect_id)) in self.plan().targets.iter().zip(&reservations) {
            if plan.target_tag != *tag || !seen.insert(*effect_id) {
                return Err(DistributionTransactionErrorV1::EffectReservationMismatch);
            }
        }
        self.effect_intents = reservations;
        self.advance(DistributionTransactionPhaseV1::EffectsReserved)
    }

    pub fn record_effect_crossings(
        &mut self,
        crossings: Vec<EffectCrossingObservationV1>,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::EffectsReserved)?;
        if crossings.len() != self.effect_intents.len() {
            return Err(DistributionTransactionErrorV1::IncompleteTargetSet);
        }
        let plan_by_tag = self
            .plan()
            .targets
            .iter()
            .map(|target| (target.target_tag, target))
            .collect::<BTreeMap<_, _>>();
        for ((reserved_tag, reserved_effect), crossing) in
            self.effect_intents.iter().zip(&crossings)
        {
            let plan = plan_by_tag
                .get(reserved_tag)
                .ok_or(DistributionTransactionErrorV1::EffectReservationMismatch)?;
            if crossing.target_tag != *reserved_tag || crossing.effect_intent_id != *reserved_effect
            {
                return Err(DistributionTransactionErrorV1::EffectReservationMismatch);
            }
            match crossing.disposition {
                EffectCrossingDispositionV1::Applied => {
                    if crossing.observed_postimage_commitment != plan.candidate_commitment {
                        return Err(DistributionTransactionErrorV1::PostimageMismatch);
                    }
                    if plan.effect_kind == TargetEffectKindV1::RewriteManagedBlock
                        && (crossing.outside_prefix_commitment != plan.outside_prefix_commitment
                            || crossing.outside_suffix_commitment != plan.outside_suffix_commitment)
                    {
                        return Err(DistributionTransactionErrorV1::OutsideBytesChanged);
                    }
                }
                EffectCrossingDispositionV1::DefinitelyNotApplied => {
                    if crossing.observed_postimage_commitment.is_some() {
                        return Err(DistributionTransactionErrorV1::PostimageMismatch);
                    }
                }
                EffectCrossingDispositionV1::InDoubt => {}
            }
        }
        let recovery_required = crossings
            .iter()
            .any(|crossing| crossing.disposition != EffectCrossingDispositionV1::Applied);
        self.crossings = crossings;
        self.advance(if recovery_required {
            DistributionTransactionPhaseV1::RecoveryRequired
        } else {
            DistributionTransactionPhaseV1::EffectsCrossed
        })
    }

    pub fn record_verification(
        &mut self,
        dispositions: Vec<(u64, VerificationDispositionV1)>,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::EffectsCrossed)?;
        if dispositions.len() != self.plan().targets.len()
            || self
                .plan()
                .targets
                .iter()
                .zip(&dispositions)
                .any(|(plan, (tag, disposition))| {
                    plan.target_tag != *tag || *disposition != VerificationDispositionV1::Exact
                })
        {
            return Err(DistributionTransactionErrorV1::VerificationFailed);
        }
        self.advance(DistributionTransactionPhaseV1::Verified)
    }

    pub fn prepare_commit(
        &mut self,
        receipt_ref: DistributionScopedObjectRefV1,
        commit_ref: DistributionScopedObjectRefV1,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::Verified)?;
        receipt_ref.require_same_domain(self.plan().domain())?;
        receipt_ref.require_kind(DistributionRuntimeObjectKindV1::DistributionReceipt)?;
        commit_ref.require_same_domain(self.plan().domain())?;
        commit_ref.require_kind(DistributionRuntimeObjectKindV1::DistributionCommitRecord)?;
        self.prepared_receipt_ref = Some(receipt_ref);
        self.prepared_commit_ref = Some(commit_ref);
        self.advance(DistributionTransactionPhaseV1::CommitPrepared)
    }

    pub(crate) fn mark_committed(
        &mut self,
        committed_ref: &DistributionScopedObjectRefV1,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::CommitPrepared)?;
        if self.prepared_commit_ref.as_ref() != Some(committed_ref) {
            return Err(DistributionTransactionErrorV1::PublicationMismatch);
        }
        self.advance(DistributionTransactionPhaseV1::Committed)
    }

    pub fn begin_rollback(&mut self) -> Result<(), DistributionTransactionErrorV1> {
        if !matches!(
            self.phase,
            DistributionTransactionPhaseV1::EffectsCrossed
                | DistributionTransactionPhaseV1::RecoveryRequired
                | DistributionTransactionPhaseV1::Verified
                | DistributionTransactionPhaseV1::CommitPrepared
        ) {
            return Err(DistributionTransactionErrorV1::IllegalPhaseTransition);
        }
        self.advance(DistributionTransactionPhaseV1::RollingBack)
    }

    pub fn record_rollback_restored(
        &mut self,
        restored_target_tags: &[u64],
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.require_phase(DistributionTransactionPhaseV1::RollingBack)?;
        if restored_target_tags.len() != self.captures.len()
            || self
                .captures
                .iter()
                .zip(restored_target_tags)
                .any(|(capture, tag)| capture.target_tag != *tag)
        {
            return Err(DistributionTransactionErrorV1::RollbackRestoreMismatch);
        }
        self.advance(DistributionTransactionPhaseV1::RolledBack)
    }

    pub fn abort_before_effects(&mut self) -> Result<(), DistributionTransactionErrorV1> {
        if !matches!(
            self.phase,
            DistributionTransactionPhaseV1::TargetsReserved
                | DistributionTransactionPhaseV1::BeforeStateCaptured
                | DistributionTransactionPhaseV1::CandidateStaged
        ) {
            return Err(DistributionTransactionErrorV1::IllegalPhaseTransition);
        }
        self.advance(DistributionTransactionPhaseV1::AbortedBeforeEffects)
    }

    pub const fn recovery_directive(&self) -> DistributionRecoveryDirectiveV1 {
        match self.phase {
            DistributionTransactionPhaseV1::Planned
            | DistributionTransactionPhaseV1::TargetsReserved
            | DistributionTransactionPhaseV1::BeforeStateCaptured
            | DistributionTransactionPhaseV1::CandidateStaged => {
                DistributionRecoveryDirectiveV1::AbortWithoutTargetWrite
            }
            DistributionTransactionPhaseV1::EffectsReserved => {
                DistributionRecoveryDirectiveV1::ReconcileReservedEffects
            }
            DistributionTransactionPhaseV1::EffectsCrossed
            | DistributionTransactionPhaseV1::RecoveryRequired
            | DistributionTransactionPhaseV1::Verified => {
                DistributionRecoveryDirectiveV1::ReconcileCrossingThenCommitOrRestore
            }
            DistributionTransactionPhaseV1::CommitPrepared => {
                DistributionRecoveryDirectiveV1::CompleteSameDomainPublication
            }
            DistributionTransactionPhaseV1::RollingBack => {
                DistributionRecoveryDirectiveV1::RestoreCapturedPreimagesOnly
            }
            DistributionTransactionPhaseV1::Committed
            | DistributionTransactionPhaseV1::RolledBack
            | DistributionTransactionPhaseV1::AbortedBeforeEffects => {
                DistributionRecoveryDirectiveV1::NoRecoveryRequired
            }
        }
    }

    fn require_phase(
        &self,
        expected: DistributionTransactionPhaseV1,
    ) -> Result<(), DistributionTransactionErrorV1> {
        if self.phase != expected {
            return Err(DistributionTransactionErrorV1::IllegalPhaseTransition);
        }
        Ok(())
    }

    fn advance(
        &mut self,
        phase: DistributionTransactionPhaseV1,
    ) -> Result<(), DistributionTransactionErrorV1> {
        self.journal_sequence = self
            .journal_sequence
            .checked_add(1)
            .ok_or(DistributionTransactionErrorV1::JournalSequenceExhausted)?;
        self.phase = phase;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DistributionTransactionErrorV1 {
    #[error("Distribution plan is incomplete or outside its finite bounds")]
    InvalidPlan,
    #[error("Distribution plan target is unmanaged, duplicated, unordered, or unbound")]
    InvalidPlanTarget,
    #[error("Rollback mutation and selected ordinary snapshot must be present together")]
    RollbackSelectionMismatch,
    #[error("Installation-domain plan requires one exact Release")]
    InstallationReleaseMissing,
    #[error("Repository-domain plan must not carry a current Release")]
    RepositoryReleasePresent,
    #[error("managed-block mutation requires exact outside-byte fences")]
    MissingOutsideByteFence,
    #[error("whole-target mutation must not carry managed-block outside-byte fences")]
    UnexpectedOutsideByteFence,
    #[error("current persisted Authority does not authorize this exact request")]
    AuthorityUnavailable,
    #[error("Distribution transaction lacks one exact current phase authorization")]
    IncompleteAuthoritySet,
    #[error("Distribution phase authorization is not bound to the exact plan request")]
    AuthorityRequestMismatch,
    #[error("target state changed after planning; no Effect reservation is permitted")]
    StaleBeforeEffects,
    #[error("transaction evidence does not cover the exact planned target set")]
    IncompleteTargetSet,
    #[error("compare-and-capture result is not bound to the planned preimage and target")]
    CaptureFenceMismatch,
    #[error("staged candidate is not the exact authorized plan")]
    CandidatePlanMismatch,
    #[error("Effect reservation is missing, duplicated, or bound to another target")]
    EffectReservationMismatch,
    #[error("Effect postimage does not equal the exact planned candidate")]
    PostimageMismatch,
    #[error("managed-block Effect changed bytes outside the claimed marker bounds")]
    OutsideBytesChanged,
    #[error("target verification is non-exact or unavailable")]
    VerificationFailed,
    #[error("transaction phase transition is not legal")]
    IllegalPhaseTransition,
    #[error("same-domain publication did not commit the prepared Distribution head")]
    PublicationMismatch,
    #[error("rollback did not restore every exact captured preimage")]
    RollbackRestoreMismatch,
    #[error("transaction journal sequence is exhausted")]
    JournalSequenceExhausted,
    #[error("transaction checkpoint cannot encode its exact captured recovery state")]
    CheckpointEncoding,
    #[error(transparent)]
    Model(#[from] DistributionModelErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

fn validate_observations(
    plan: &DistributionPlanV1,
    observations: &[TargetPlanObservationV1],
) -> Result<(), DistributionTransactionErrorV1> {
    if observations.len() != plan.targets.len() {
        return Err(DistributionTransactionErrorV1::IncompleteTargetSet);
    }
    for (target, observed) in plan.targets.iter().zip(observations) {
        if target.target_tag != observed.target_tag
            || target.target_identity != observed.target_identity
            || target.expected_preimage_commitment != observed.preimage_commitment
            || target.outside_prefix_commitment != observed.outside_prefix_commitment
            || target.outside_suffix_commitment != observed.outside_suffix_commitment
        {
            return Err(DistributionTransactionErrorV1::StaleBeforeEffects);
        }
    }
    Ok(())
}

fn required_actions(plan: &DistributionPlanV1) -> BTreeSet<DistributionActionV1> {
    let mut required = BTreeSet::from([
        DistributionActionV1::ReserveDistributionTargets,
        DistributionActionV1::BeginDistributionTransaction,
        DistributionActionV1::CaptureDistributionBeforeState,
        DistributionActionV1::StageDistributionCandidate,
        DistributionActionV1::ReserveDistributionEffect,
        DistributionActionV1::PublishDistributionOccurrence,
        DistributionActionV1::VerifyDistributionTarget,
        DistributionActionV1::CommitDistributionTransaction,
        DistributionActionV1::RecoverDistributionTransaction,
        DistributionActionV1::RollbackDistributionTransaction,
    ]);
    for target in &plan.targets {
        match target.effect_kind {
            TargetEffectKindV1::AdoptManagedRegion => {
                required.insert(DistributionActionV1::AdoptManagedRegion);
            }
            TargetEffectKindV1::TransferWholeFileCustody => {
                required.insert(DistributionActionV1::TransferWholeFileCustody);
            }
            TargetEffectKindV1::ActivateBinarySlot => {
                required.insert(DistributionActionV1::ActivateBinarySlot);
            }
            _ => {}
        }
    }
    required
}

fn validate_phase_authorization(
    plan: &DistributionPlanV1,
    phase: &DistributionPhaseAuthorizationV1,
    current_objects: &[StoreObjectV1],
) -> Result<(), DistributionTransactionErrorV1> {
    OperationRequestV1::Action(phase.request.clone())
        .validate()
        .map_err(|_| DistributionTransactionErrorV1::AuthorityRequestMismatch)?;
    phase.request_ref.require_same_domain(&plan.domain)?;
    phase
        .request_ref
        .require_kind(DistributionRuntimeObjectKindV1::ActionRequestOrCeremony)?;
    let request_id = ActionRequestIdV1::parse(&phase.request.request_id)
        .map_err(|_| DistributionTransactionErrorV1::AuthorityRequestMismatch)?;
    let expected_action_ref = format!("sha256:{}", phase.action.descriptor_id());
    let expected_generation_ref = format!("sha256:{}", plan.domain.store_generation_id().to_hex());
    let expected_epoch_ref = format!("sha256:{}", plan.domain.authority_epoch_id().to_hex());
    let expected_typed_input = deterministic_cbor::encode(&plan.canonical_value()?)?;
    let expected_basis_kind = match &phase.request.authority_basis {
        PublicActionAuthorityBasisV1::Ordinary { .. } => {
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
        }
        PublicActionAuthorityBasisV1::BootstrapControl { .. } => {
            ActionAuthorityBasisKindV1::BootstrapControlG0
        }
        PublicActionAuthorityBasisV1::ContinuityMaintenance { .. } => {
            ActionAuthorityBasisKindV1::ContinuityMaintenance
        }
    };
    let expected_request_schema =
        SchemaIdV1::parse(&format!("sha256:{ACTION_REQUEST_SCHEMA_ID_V1}"))
            .expect("invariant: frozen ActionRequestV1 SchemaId is canonical");
    let request_object = current_objects
        .iter()
        .find(|object| object.id() == phase.request_ref.object_id())
        .ok_or(DistributionTransactionErrorV1::AuthorityRequestMismatch)?;
    if request_id != phase.receipt.request_id()
        || phase.receipt.basis_kind() != expected_basis_kind
        || phase.request.action_spec.exact_action_spec_ref != expected_action_ref
        || phase.request.material_dependency_stamp != *plan.meaning_digest.as_bytes()
        || phase.request.exact_store_generation_ref != expected_generation_ref
        || phase.request.exact_authority_epoch_ref != expected_epoch_ref
        || phase.request.typed_input_cbor != expected_typed_input
        || request_object.schema_id() != expected_request_schema
        || request_object.value() != &action_request_value(&phase.request)?
        || !crate::domain::vnext::authority::current_authorization_receipt_is_persisted(
            current_objects,
            &phase.receipt,
        )
        .map_err(|_| DistributionTransactionErrorV1::AuthorityUnavailable)?
    {
        return Err(DistributionTransactionErrorV1::AuthorityRequestMismatch);
    }
    Ok(())
}

fn action_request_value(request: &ActionRequestV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(request.schema_version),
        CborValue::text(request.request_id.clone())?,
        CborValue::text(request.idempotency_key.clone())?,
        CborValue::Bytes(request.semantic_request_hash.to_vec()),
        CborValue::Bytes(request.selected_packet_semantic_hash.to_vec()),
        CborValue::Array(vec![
            CborValue::text(request.action_spec.exact_action_spec_ref.clone())?,
            CborValue::text(request.action_spec.exact_schema_id.clone())?,
            CborValue::text(request.action_spec.exact_core_catalog_ref.clone())?,
            CborValue::text(request.action_spec.exact_public_catalog_ref.clone())?,
        ]),
        CborValue::Bytes(request.material_dependency_stamp.to_vec()),
        CborValue::text(request.exact_store_generation_ref.clone())?,
        CborValue::text(request.exact_authority_epoch_ref.clone())?,
        CborValue::text(request.valid_until_ref.clone())?,
        public_authority_basis_value(&request.authority_basis)?,
        CborValue::Bytes(request.typed_input_cbor.clone()),
        text_array(&request.evidence_refs)?,
        text_array(&request.prerequisite_receipt_refs)?,
        CborValue::optional(
            request
                .orchestration_attribution
                .as_ref()
                .map(orchestration_attribution_value)
                .transpose()?,
        ),
    ]))
}

fn public_authority_basis_value(
    basis: &PublicActionAuthorityBasisV1,
) -> Result<CborValue, CborError> {
    Ok(match basis {
        PublicActionAuthorityBasisV1::Ordinary {
            verified_principal_ref,
            current_session_ref,
            live_grant_refs,
            required_mandate_refs,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::text(verified_principal_ref.clone())?,
            CborValue::text(current_session_ref.clone())?,
            text_array(live_grant_refs)?,
            text_array(required_mandate_refs)?,
        ]),
        PublicActionAuthorityBasisV1::BootstrapControl {
            exact_bootstrap_scope_ref,
            current_executor_assertion_ref,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::text(exact_bootstrap_scope_ref.clone())?,
            CborValue::text(current_executor_assertion_ref.clone())?,
        ]),
        PublicActionAuthorityBasisV1::ContinuityMaintenance {
            exact_cma_branch_ref,
            maintenance_executor_assertion_ref,
            applicability_ref,
            phase_slot_ref,
        } => CborValue::Array(vec![
            CborValue::Unsigned(3),
            CborValue::text(exact_cma_branch_ref.clone())?,
            CborValue::text(maintenance_executor_assertion_ref.clone())?,
            CborValue::text(applicability_ref.clone())?,
            CborValue::text(phase_slot_ref.clone())?,
        ]),
    })
}

fn orchestration_attribution_value(
    attribution: &OrchestrationAttributionV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(attribution.exact_packet_recipe_binding_ref.clone())?,
        CborValue::text(attribution.exact_application_ref.clone())?,
        CborValue::Array(
            attribution
                .component_output_hashes
                .iter()
                .map(|hash| CborValue::Bytes(hash.to_vec()))
                .collect(),
        ),
        CborValue::Bytes(attribution.composed_advice_hash.to_vec()),
    ]))
}

fn text_array(values: &[String]) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(
        values
            .iter()
            .map(|value| CborValue::text(value.clone()))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "canonical Distribution plan identity binds every frozen dimension"
)]
fn plan_value(
    domain: &DistributionDomainRefV1,
    mutation_kind: DistributionMutationKindV1,
    request_id: ActionRequestIdV1,
    request_ref: &DistributionScopedObjectRefV1,
    plan_ref: &DistributionScopedObjectRefV1,
    idempotency_key_ref: &DistributionScopedObjectRefV1,
    release_id: Option<ReleaseIdV1>,
    prior_commit_ref: Option<&DistributionScopedObjectRefV1>,
    prior_receipt_ref: Option<&DistributionScopedObjectRefV1>,
    selected_rollback_ref: Option<&DistributionScopedObjectRefV1>,
    targets: &[DistributionPlanTargetV1],
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.distribution-plan.v1")?,
        domain.canonical_value(),
        CborValue::Unsigned(mutation_kind.numeric_tag()),
        CborValue::Bytes(request_id.as_bytes().to_vec()),
        request_ref.canonical_value(),
        plan_ref.canonical_value(),
        idempotency_key_ref.canonical_value(),
        CborValue::optional(release_id.map(bytes)),
        optional_ref(prior_commit_ref),
        optional_ref(prior_receipt_ref),
        optional_ref(selected_rollback_ref),
        CborValue::Array(
            targets
                .iter()
                .map(|target| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(target.target_tag),
                        target.target_identity_ref.canonical_value(),
                        bytes(target.target_identity),
                        CborValue::Unsigned(target.custody.class().numeric_tag()),
                        CborValue::optional(
                            target
                                .custody
                                .unmanaged_reason()
                                .map(|reason| CborValue::Unsigned(reason.numeric_tag())),
                        ),
                        CborValue::Bool(target.custody.has_preexisting_drift()),
                        bytes(target.expected_preimage_commitment),
                        CborValue::optional(target.candidate_commitment.map(bytes)),
                        CborValue::Unsigned(target.effect_kind.numeric_tag()),
                        CborValue::optional(target.outside_prefix_commitment.map(bytes)),
                        CborValue::optional(target.outside_suffix_commitment.map(bytes)),
                    ])
                })
                .collect(),
        ),
    ]))
}

fn validate_plan_refs(
    domain: &DistributionDomainRefV1,
    request_ref: &DistributionScopedObjectRefV1,
    plan_ref: &DistributionScopedObjectRefV1,
    idempotency_key_ref: &DistributionScopedObjectRefV1,
    prior_commit_ref: Option<&DistributionScopedObjectRefV1>,
    prior_receipt_ref: Option<&DistributionScopedObjectRefV1>,
    selected_rollback_ref: Option<&DistributionScopedObjectRefV1>,
) -> Result<(), DistributionTransactionErrorV1> {
    for (reference, kind) in [
        (
            request_ref,
            DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
        ),
        (plan_ref, DistributionRuntimeObjectKindV1::DistributionPlan),
        (
            idempotency_key_ref,
            DistributionRuntimeObjectKindV1::IdempotencyKey,
        ),
    ] {
        reference.require_same_domain(domain)?;
        reference.require_kind(kind)?;
    }
    for (reference, kind) in [
        (
            prior_commit_ref,
            DistributionRuntimeObjectKindV1::DistributionCommitRecord,
        ),
        (
            prior_receipt_ref,
            DistributionRuntimeObjectKindV1::DistributionReceipt,
        ),
        (
            selected_rollback_ref,
            DistributionRuntimeObjectKindV1::DistributionSnapshot,
        ),
    ] {
        if let Some(reference) = reference {
            reference.require_same_domain(domain)?;
            reference.require_kind(kind)?;
        }
    }
    Ok(())
}

fn validate_release_binding(
    domain: &DistributionDomainRefV1,
    release_id: Option<ReleaseIdV1>,
) -> Result<(), DistributionTransactionErrorV1> {
    if release_id.is_some_and(|release| release.as_bytes() == &[0; 32]) {
        return Err(DistributionTransactionErrorV1::InstallationReleaseMissing);
    }
    match (domain.kind(), release_id) {
        (DistributionDomainKindV1::RepositoryDomain, None)
        | (DistributionDomainKindV1::InstallationDomain, Some(_)) => Ok(()),
        (DistributionDomainKindV1::RepositoryDomain, Some(_)) => {
            Err(DistributionTransactionErrorV1::RepositoryReleasePresent)
        }
        (DistributionDomainKindV1::InstallationDomain, None) => {
            Err(DistributionTransactionErrorV1::InstallationReleaseMissing)
        }
    }
}

fn optional_ref(reference: Option<&DistributionScopedObjectRefV1>) -> CborValue {
    CborValue::optional(reference.map(DistributionScopedObjectRefV1::canonical_value))
}

fn bytes(value: CommitmentV1) -> CborValue {
    CborValue::Bytes(value.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::authority::{
        ActionAuthorityBasisKindV1, AuthorityContextIdV1, StateTokenIdV1,
    };
    use crate::domain::vnext::identity::StoreObjectIdV1;
    use crate::domain::vnext::integration::public_literals::{
        ActionAuthorityBasisV1, ActionSpecRefV1,
    };

    fn commitment(byte: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([byte; 32])
    }

    fn object_id(byte: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
    }

    fn domain() -> DistributionDomainRefV1 {
        DistributionDomainRefV1::new(
            DistributionDomainKindV1::InstallationDomain,
            commitment(1),
            commitment(2),
            commitment(3),
        )
        .unwrap()
    }

    fn scoped(
        domain: &DistributionDomainRefV1,
        kind: DistributionRuntimeObjectKindV1,
        byte: u8,
    ) -> DistributionScopedObjectRefV1 {
        DistributionScopedObjectRefV1::new(domain.clone(), kind, object_id(byte)).unwrap()
    }

    fn authorized_plan() -> AuthorizedDistributionPlanV1 {
        let domain = domain();
        let request_id = ActionRequestIdV1::derive("stage9-transaction-test").unwrap();
        let custody = CustodyAssessmentV1::assess(&super::super::CustodyBasisV1 {
            domain: domain.clone(),
            target_identity: commitment(10),
            alias_closure_id: commitment(11),
            receipt_ref: Some(scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
                12,
            )),
            claim_ref: Some(scoped(
                &domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                13,
            )),
            claimed_target_identity: Some(commitment(10)),
            resource_id: Some(commitment(14)),
            bundle_id: Some(commitment(15)),
            release_id: Some(commitment(16)),
            claimed_content_sha256: Some(commitment(17)),
            observed_content_sha256: Some(commitment(17)),
            managed_block: None,
            foreign_owner_observed: false,
            external_manager_observed: false,
            alias_ambiguous: false,
            unsafe_path_state: false,
        })
        .unwrap();
        let plan = DistributionPlanV1::new(
            domain.clone(),
            DistributionMutationKindV1::Update,
            request_id,
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                20,
            ),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionPlan,
                21,
            ),
            scoped(&domain, DistributionRuntimeObjectKindV1::IdempotencyKey, 22),
            Some(commitment(16)),
            Some(scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionCommitRecord,
                23,
            )),
            Some(scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
                24,
            )),
            None,
            vec![DistributionPlanTargetV1 {
                target_tag: 1,
                target_identity_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                    25,
                ),
                target_identity: commitment(10),
                custody,
                expected_preimage_commitment: commitment(30),
                candidate_commitment: Some(commitment(31)),
                effect_kind: TargetEffectKindV1::RewriteOwnedTarget,
                outside_prefix_commitment: None,
                outside_suffix_commitment: None,
            }],
        )
        .unwrap();
        let receipt = AuthorizationReceiptV1::new(
            request_id,
            AuthorityContextIdV1::derive("stage9-context").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive("stage9-prior").unwrap(),
            StateTokenIdV1::derive("stage9-resulting").unwrap(),
        )
        .unwrap();
        let request_ref = plan.request_or_ceremony_ref.clone();
        let request = ActionRequestV1 {
            schema_version: 1,
            request_id: request_id.render(),
            idempotency_key: "stage9-test-key".to_owned(),
            semantic_request_hash: *plan.meaning_digest().as_bytes(),
            selected_packet_semantic_hash: *commitment(50).as_bytes(),
            action_spec: ActionSpecRefV1 {
                exact_action_spec_ref: format!(
                    "sha256:{}",
                    DistributionActionV1::BeginDistributionTransaction.descriptor_id()
                ),
                exact_schema_id: format!("sha256:{}", commitment(51).to_hex()),
                exact_core_catalog_ref: "core-catalog:test".to_owned(),
                exact_public_catalog_ref: "public-catalog:test".to_owned(),
            },
            material_dependency_stamp: *plan.meaning_digest().as_bytes(),
            exact_store_generation_ref: format!(
                "sha256:{}",
                plan.domain().store_generation_id().to_hex()
            ),
            exact_authority_epoch_ref: format!(
                "sha256:{}",
                plan.domain().authority_epoch_id().to_hex()
            ),
            valid_until_ref: "validity:test".to_owned(),
            authority_basis: ActionAuthorityBasisV1::Ordinary {
                verified_principal_ref: "principal:test".to_owned(),
                current_session_ref: "session:test".to_owned(),
                live_grant_refs: vec![],
                required_mandate_refs: vec![],
            },
            typed_input_cbor: vec![0x80],
            evidence_refs: vec![],
            prerequisite_receipt_refs: vec![],
            orchestration_attribution: None,
        };
        AuthorizedDistributionPlanV1 {
            plan,
            authority: DistributionAuthoritySetV1 {
                phases: vec![DistributionPhaseAuthorizationV1::new(
                    DistributionActionV1::BeginDistributionTransaction,
                    request_ref,
                    request,
                    receipt,
                )],
            },
        }
    }

    fn observation() -> TargetPlanObservationV1 {
        TargetPlanObservationV1 {
            target_tag: 1,
            target_identity: commitment(10),
            preimage_commitment: commitment(30),
            outside_prefix_commitment: None,
            outside_suffix_commitment: None,
        }
    }

    fn capture() -> CapturedTargetPreimageV1 {
        let domain = domain();
        CapturedTargetPreimageV1 {
            target_tag: 1,
            compared_preimage_commitment: commitment(30),
            snapshot_target: DistributionSnapshotTargetV1 {
                target_tag: 1,
                domain: domain.clone(),
                canonical_target_identity_ref: scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                    25,
                ),
                prior_claim_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    13,
                )),
                content_object_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::ContentObject,
                    32,
                )),
                content_sha256: Some(commitment(17)),
                prior_absence: false,
                permissions_commitment_id: commitment(33),
                owner_metadata_commitment_id: commitment(34),
                managed_block_ref: None,
                restore_profile_id: commitment(35),
            },
            effect_fence_commitment: commitment(36),
        }
    }

    #[test]
    fn post_plan_drift_fails_before_an_effect_can_be_reserved() {
        let mut stale = observation();
        stale.preimage_commitment = commitment(99);
        assert!(matches!(
            DistributionTransactionV1::begin(authorized_plan(), vec![stale]),
            Err(DistributionTransactionErrorV1::StaleBeforeEffects)
        ));
    }

    #[test]
    fn exact_effect_and_commit_phase_chain_has_a_durable_checkpoint() {
        let mut transaction =
            DistributionTransactionV1::begin(authorized_plan(), vec![observation()]).unwrap();
        transaction.record_atomic_captures(vec![capture()]).unwrap();
        transaction
            .record_candidate_staged(transaction.plan().meaning_digest())
            .unwrap();
        let effect = EffectIntentIdV1::derive("stage9-effect").unwrap();
        transaction
            .record_effect_reservations(vec![(1, effect)])
            .unwrap();
        transaction
            .record_effect_crossings(vec![EffectCrossingObservationV1 {
                target_tag: 1,
                effect_intent_id: effect,
                disposition: EffectCrossingDispositionV1::Applied,
                observed_postimage_commitment: Some(commitment(31)),
                outside_prefix_commitment: None,
                outside_suffix_commitment: None,
            }])
            .unwrap();
        transaction
            .record_verification(vec![(1, VerificationDispositionV1::Exact)])
            .unwrap();
        let domain = domain();
        transaction
            .prepare_commit(
                scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionReceipt,
                    40,
                ),
                scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionCommitRecord,
                    41,
                ),
            )
            .unwrap();
        assert_eq!(
            transaction.recovery_directive(),
            DistributionRecoveryDirectiveV1::CompleteSameDomainPublication
        );
        assert_ne!(
            transaction.checkpoint_commitment().unwrap().as_bytes(),
            &[0; 32]
        );
    }

    #[test]
    fn captured_snapshot_must_bind_the_exact_planned_target_reference() {
        let mut transaction =
            DistributionTransactionV1::begin(authorized_plan(), vec![observation()]).unwrap();
        let mut substituted = capture();
        substituted.snapshot_target.canonical_target_identity_ref = scoped(
            transaction.plan().domain(),
            DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
            99,
        );
        assert!(matches!(
            transaction.record_atomic_captures(vec![substituted]),
            Err(DistributionTransactionErrorV1::CaptureFenceMismatch)
        ));
    }

    #[test]
    fn installation_plan_rejects_a_zero_release_identity() {
        let plan = authorized_plan().plan().clone();
        assert!(matches!(
            DistributionPlanV1::new(
                plan.domain,
                plan.mutation_kind,
                plan.request_id,
                plan.request_or_ceremony_ref,
                plan.plan_ref,
                plan.idempotency_key_ref,
                Some(CommitmentV1::from_bytes([0; 32])),
                plan.prior_commit_ref,
                plan.prior_receipt_ref,
                plan.selected_rollback_ref,
                plan.targets,
            ),
            Err(DistributionTransactionErrorV1::InstallationReleaseMissing)
        ));
    }

    #[test]
    fn in_doubt_effect_requires_exact_preimage_restore() {
        let mut transaction =
            DistributionTransactionV1::begin(authorized_plan(), vec![observation()]).unwrap();
        transaction.record_atomic_captures(vec![capture()]).unwrap();
        transaction
            .record_candidate_staged(transaction.plan().meaning_digest())
            .unwrap();
        let effect = EffectIntentIdV1::derive("stage9-indoubt-effect").unwrap();
        transaction
            .record_effect_reservations(vec![(1, effect)])
            .unwrap();
        transaction
            .record_effect_crossings(vec![EffectCrossingObservationV1 {
                target_tag: 1,
                effect_intent_id: effect,
                disposition: EffectCrossingDispositionV1::InDoubt,
                observed_postimage_commitment: None,
                outside_prefix_commitment: None,
                outside_suffix_commitment: None,
            }])
            .unwrap();
        assert_eq!(
            transaction.recovery_directive(),
            DistributionRecoveryDirectiveV1::ReconcileCrossingThenCommitOrRestore
        );
        transaction.begin_rollback().unwrap();
        assert!(matches!(
            transaction.record_rollback_restored(&[]),
            Err(DistributionTransactionErrorV1::RollbackRestoreMismatch)
        ));
        transaction.record_rollback_restored(&[1]).unwrap();
        assert_eq!(
            transaction.recovery_directive(),
            DistributionRecoveryDirectiveV1::NoRecoveryRequired
        );
    }
}
