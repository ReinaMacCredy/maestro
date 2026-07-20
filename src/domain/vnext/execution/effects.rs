use sha2::{Digest, Sha256};
use thiserror::Error;

use super::control_head::{
    EffectIntentControlErrorV1, EffectIntentControlHeadV1, EffectIntentControlHealthV1,
    EffectIntentControlMutationV1, EffectIntentControlPublicationCommitmentsV1,
    EffectIntentControlRevisionPartsV1, EffectIntentControlRevisionV1,
    EffectIntentControlTransitionV1, EffectIntentControlWriterTermV1,
    SameHomeWriterFencingReceiptV1, derive_candidate_revision,
};
use super::dispatch_state::{
    DispatchAttemptOutcomeV1, DispatchAttemptStateV1, DispatchAttemptTerminalV1,
    DispatchBindingPartsV1, DispatchBindingV1, DispatchCommitmentV1, DispatchCrossingSealV1,
    DispatchStateError, PreSealLocallyRejectedV1, ReservedUnsealedV1, SealedDispatchOutcomeV1,
    SealedDispatchTerminalV1, SealedInFlightV1,
};
use super::effect_home::{
    ActiveStoreHomeV1, ActiveStoreOriginationFenceV1, ActiveStoreUseFenceV1,
    EffectIntentDomainKindV1, EffectIntentHomeError, EffectIntentHomeKindV1, EffectIntentHomeV1,
    EffectIntentOriginationFenceV1, EffectIntentUseFenceV1, HomeTokenV1, NoStoreCeremonyHomeV1,
    NoStoreCeremonyOriginationFenceV1, NoStoreCeremonyUseFenceV1, PreStoreCeremonyHomeV1,
    PreStoreCeremonyOriginationFenceV1, PreStoreCeremonyUseFenceV1, validate_fence_home,
};
use super::effect_routes::{CeremonyRequestModeV1, DispatchReservationModeV1};
use super::runtime::{
    AuthorizedExecutionActionV1, DispatchAttemptV1, EffectIntentIdV1, ExecutionActionV1,
    ExecutionAttemptOwnerV1, ExecutionAttemptV1, ExecutionRuntimeErrorV1, LeaseTermIdV1,
    ReconciliationAttemptIdV1, ReconciliationAttemptV1, RunReservationV1, RunSetV1, RunStateV1,
    StepAttemptIdV1, StepAttemptStateV1, StepExecutionTenureV1, StepLeaseIdV1,
};
use super::withdrawal::{
    EffectIntentLiveDispatchV1, EffectWithdrawalSlotFamilyV1, RemoteClassificationV1,
    WithdrawalAuthorityPathV1, WithdrawalError, WithdrawalRequestV1,
    action_withdrawal_catalog_cell_v1, exact_withdrawal_catalog_cell_v1, validate_withdrawal,
};
use crate::domain::vnext::authority::ActionRequestIdV1;
use crate::domain::vnext::step::StepBindingV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub const EFFECT_ORIGIN_MANIFEST_ID_V1: &str =
    "d28f8e573ddb450c427e628df121dbd516d0e5b05c03caf18d2757782dfd259d";
pub const EFFECT_ORIGIN_COUNT_V1: usize = 23;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EffectOriginKindV1 {
    StepEffectOrigin,
    CoordinationDeliveryEffectOrigin,
    EffectRemediationOrigin,
    BootstrapMandatePresentationDeliveryOrigin,
    BootstrapMandateResponseAcquisitionOrigin,
    GovernedReviewPresentationDeliveryOrigin,
    GovernedReviewResponseAcquisitionOrigin,
    TrustedTimeAcquisitionEffectOrigin,
    RecoveryExternalRegistrationEffectOrigin,
    RecoveryExternalStatusEffectOrigin,
    MaintenanceExecutorCurrentnessEffectOrigin,
    ProspectiveContinuityCarrierEffectOrigin,
    PlannedTurnoverHighWaterEffectOrigin,
    RepositoryRecoveryAdmissionEffectOrigin,
    InstallationRecoveryAdmissionEffectOrigin,
    GovernedReviewPublicationHighWaterEffectOrigin,
    DistributionArtifactAcquisitionEffectOrigin,
    DistributionFilesystemMutationEffectOrigin,
    DistributionManagerOperationEffectOrigin,
    DistributionBinaryActivationEffectOrigin,
    RepositoryGenerationActivationEffectOrigin,
    InstallationLocatorActivationEffectOrigin,
    CapabilityProbeEffectOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectOriginDescriptorV1 {
    kind: EffectOriginKindV1,
    tag: u8,
    name: &'static str,
    descriptor_id: &'static str,
    primary_owner_tag: u8,
    primary_owner_descriptor_id: &'static str,
    origin_source_owner_tag: u8,
    route_count: u8,
}

impl EffectOriginDescriptorV1 {
    pub const fn kind(self) -> EffectOriginKindV1 {
        self.kind
    }

    pub const fn tag(self) -> u8 {
        self.tag
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn descriptor_id(self) -> &'static str {
        self.descriptor_id
    }

    pub const fn primary_owner_tag(self) -> u8 {
        self.primary_owner_tag
    }

    pub const fn primary_owner_descriptor_id(self) -> &'static str {
        self.primary_owner_descriptor_id
    }

    pub const fn origin_source_owner_tag(self) -> u8 {
        self.origin_source_owner_tag
    }

    pub const fn route_count(self) -> u8 {
        self.route_count
    }
}

const EFFECT_PRIMARY_OWNER_DESCRIPTOR_ID_V1: &str =
    "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292";

macro_rules! effect_origin_descriptors {
    ($(($variant:ident, $tag:literal, $name:literal, $id:literal, $source:literal, $routes:literal)),+ $(,)?) => {
        pub const EFFECT_ORIGIN_DESCRIPTORS_V1: [EffectOriginDescriptorV1; EFFECT_ORIGIN_COUNT_V1] = [
            $(EffectOriginDescriptorV1 {
                kind: EffectOriginKindV1::$variant,
                tag: $tag,
                name: $name,
                descriptor_id: $id,
                primary_owner_tag: 6,
                primary_owner_descriptor_id: EFFECT_PRIMARY_OWNER_DESCRIPTOR_ID_V1,
                origin_source_owner_tag: $source,
                route_count: $routes,
            }),+
        ];
    };
}

effect_origin_descriptors!(
    (
        StepEffectOrigin,
        1,
        "StepEffectOrigin",
        "4d8e976464a22dbf681256045c3f79cbc224d66cc814650114bff5e9edcc153d",
        6,
        5
    ),
    (
        CoordinationDeliveryEffectOrigin,
        2,
        "CoordinationDeliveryEffectOrigin",
        "e3986983e9e8935dab48c19110f3be31bc60700ff884ee812bcd0070e58fdc3b",
        10,
        5
    ),
    (
        EffectRemediationOrigin,
        3,
        "EffectRemediationOrigin",
        "e37acdb0bc899a51aaf52773f6a3b1c6ec4bb9881ab2c5304a9a387cd22031d8",
        6,
        5
    ),
    (
        BootstrapMandatePresentationDeliveryOrigin,
        4,
        "BootstrapMandatePresentationDeliveryOrigin",
        "c6b87af81baa191900c3ba08a083fa9f5835645a9e4bb69b37f2bfa82fea5a79",
        9,
        5
    ),
    (
        BootstrapMandateResponseAcquisitionOrigin,
        5,
        "BootstrapMandateResponseAcquisitionOrigin",
        "6b90b686ebe233345c2e568762caa2979537352aaca8f83f33c3f41e64cf2110",
        9,
        5
    ),
    (
        GovernedReviewPresentationDeliveryOrigin,
        6,
        "GovernedReviewPresentationDeliveryOrigin",
        "523ba9425e8c6f8ff4906fa64eac912bd5361952b8d84a00147eab409f4ea005",
        9,
        5
    ),
    (
        GovernedReviewResponseAcquisitionOrigin,
        7,
        "GovernedReviewResponseAcquisitionOrigin",
        "5c483bde7587bb1a6ff8aac81ce3848c4e7d4cf86bace6f8320fa06c13273dcf",
        9,
        5
    ),
    (
        TrustedTimeAcquisitionEffectOrigin,
        8,
        "TrustedTimeAcquisitionEffectOrigin",
        "53aac25894337bdf22324aab00cc03fc874ec418610c642de13a05f90f09de06",
        9,
        5
    ),
    (
        RecoveryExternalRegistrationEffectOrigin,
        9,
        "RecoveryExternalRegistrationEffectOrigin",
        "c354fff42a7b3932983715dfd5afce78708e7828f40dd83faf6fef4149251934",
        9,
        5
    ),
    (
        RecoveryExternalStatusEffectOrigin,
        10,
        "RecoveryExternalStatusEffectOrigin",
        "e7a7050dfddb533cc7cd00ecfac3ca8ab12f60a04b88e4704c3fc6a5aec8b0a0",
        9,
        5
    ),
    (
        MaintenanceExecutorCurrentnessEffectOrigin,
        11,
        "MaintenanceExecutorCurrentnessEffectOrigin",
        "bf4166ebf9460ba4cc0a4d02f6c7e73a6d08a36a6999d37248f276ce1c395ab2",
        9,
        5
    ),
    (
        ProspectiveContinuityCarrierEffectOrigin,
        12,
        "ProspectiveContinuityCarrierEffectOrigin",
        "ebb487a0a08757f76b77dc1252a6ec25da68764aedeea02cc6351760f2bb202f",
        9,
        5
    ),
    (
        PlannedTurnoverHighWaterEffectOrigin,
        13,
        "PlannedTurnoverHighWaterEffectOrigin",
        "5a1856ce52a9e1707c4fa5a240e757d36b88a259938fd64f1659e84b0aea0a89",
        9,
        5
    ),
    (
        RepositoryRecoveryAdmissionEffectOrigin,
        14,
        "RepositoryRecoveryAdmissionEffectOrigin",
        "5e0e0d3e2eea45596a8a878dce30370971f0cb6cbf2e7ff1a6ce5d2427cc02cb",
        9,
        4
    ),
    (
        InstallationRecoveryAdmissionEffectOrigin,
        15,
        "InstallationRecoveryAdmissionEffectOrigin",
        "bd2de098ec00fb638e2f0332bba95200169634a11dda84282a75b53b5ee80bce",
        21,
        4
    ),
    (
        GovernedReviewPublicationHighWaterEffectOrigin,
        16,
        "GovernedReviewPublicationHighWaterEffectOrigin",
        "741db8295bd261fb2e28b739b98bc2fce6de384811fe66c0cb83151f5fd0ef17",
        9,
        5
    ),
    (
        DistributionArtifactAcquisitionEffectOrigin,
        17,
        "DistributionArtifactAcquisitionEffectOrigin",
        "b9061d83abdd45e6b796bcfd2bc667f563cdecebb7d86d1939349d2287e4d2f4",
        20,
        5
    ),
    (
        DistributionFilesystemMutationEffectOrigin,
        18,
        "DistributionFilesystemMutationEffectOrigin",
        "c69a73d6c87443cec4ec580feb5c5b61e8d63e2e0466dd8fb5f8b6474f97220c",
        20,
        5
    ),
    (
        DistributionManagerOperationEffectOrigin,
        19,
        "DistributionManagerOperationEffectOrigin",
        "914a4e8600b8691e214eb20476c109300b69559d88d6f6cd8c8c57b7204487ff",
        20,
        9
    ),
    (
        DistributionBinaryActivationEffectOrigin,
        20,
        "DistributionBinaryActivationEffectOrigin",
        "8e4b94733723aade5bd0d37ec4cd51ebe9d2f719344c3c244e5e9e52ac89c461",
        20,
        9
    ),
    (
        RepositoryGenerationActivationEffectOrigin,
        21,
        "RepositoryGenerationActivationEffectOrigin",
        "1a6bafef9d43da26e03b449d57dcaaa5cb53713af48ab6927d32ffcd428aa828",
        9,
        12
    ),
    (
        InstallationLocatorActivationEffectOrigin,
        22,
        "InstallationLocatorActivationEffectOrigin",
        "d0bbf910dc93a25ad768b3248c4f05541f050e3d49cd6e160ddf1bae6f832f70",
        21,
        16
    ),
    (
        CapabilityProbeEffectOrigin,
        23,
        "CapabilityProbeEffectOrigin",
        "86cb7d42c70e9070837542fcbc2d9caad7471704a7309ab4405bb3e86771d966",
        7,
        5
    ),
);

impl EffectOriginKindV1 {
    pub const ALL: [Self; EFFECT_ORIGIN_COUNT_V1] = [
        Self::StepEffectOrigin,
        Self::CoordinationDeliveryEffectOrigin,
        Self::EffectRemediationOrigin,
        Self::BootstrapMandatePresentationDeliveryOrigin,
        Self::BootstrapMandateResponseAcquisitionOrigin,
        Self::GovernedReviewPresentationDeliveryOrigin,
        Self::GovernedReviewResponseAcquisitionOrigin,
        Self::TrustedTimeAcquisitionEffectOrigin,
        Self::RecoveryExternalRegistrationEffectOrigin,
        Self::RecoveryExternalStatusEffectOrigin,
        Self::MaintenanceExecutorCurrentnessEffectOrigin,
        Self::ProspectiveContinuityCarrierEffectOrigin,
        Self::PlannedTurnoverHighWaterEffectOrigin,
        Self::RepositoryRecoveryAdmissionEffectOrigin,
        Self::InstallationRecoveryAdmissionEffectOrigin,
        Self::GovernedReviewPublicationHighWaterEffectOrigin,
        Self::DistributionArtifactAcquisitionEffectOrigin,
        Self::DistributionFilesystemMutationEffectOrigin,
        Self::DistributionManagerOperationEffectOrigin,
        Self::DistributionBinaryActivationEffectOrigin,
        Self::RepositoryGenerationActivationEffectOrigin,
        Self::InstallationLocatorActivationEffectOrigin,
        Self::CapabilityProbeEffectOrigin,
    ];

    pub const fn descriptor(self) -> EffectOriginDescriptorV1 {
        EFFECT_ORIGIN_DESCRIPTORS_V1[(self.tag() - 1) as usize]
    }

    pub const fn tag(self) -> u8 {
        match self {
            Self::StepEffectOrigin => 1,
            Self::CoordinationDeliveryEffectOrigin => 2,
            Self::EffectRemediationOrigin => 3,
            Self::BootstrapMandatePresentationDeliveryOrigin => 4,
            Self::BootstrapMandateResponseAcquisitionOrigin => 5,
            Self::GovernedReviewPresentationDeliveryOrigin => 6,
            Self::GovernedReviewResponseAcquisitionOrigin => 7,
            Self::TrustedTimeAcquisitionEffectOrigin => 8,
            Self::RecoveryExternalRegistrationEffectOrigin => 9,
            Self::RecoveryExternalStatusEffectOrigin => 10,
            Self::MaintenanceExecutorCurrentnessEffectOrigin => 11,
            Self::ProspectiveContinuityCarrierEffectOrigin => 12,
            Self::PlannedTurnoverHighWaterEffectOrigin => 13,
            Self::RepositoryRecoveryAdmissionEffectOrigin => 14,
            Self::InstallationRecoveryAdmissionEffectOrigin => 15,
            Self::GovernedReviewPublicationHighWaterEffectOrigin => 16,
            Self::DistributionArtifactAcquisitionEffectOrigin => 17,
            Self::DistributionFilesystemMutationEffectOrigin => 18,
            Self::DistributionManagerOperationEffectOrigin => 19,
            Self::DistributionBinaryActivationEffectOrigin => 20,
            Self::RepositoryGenerationActivationEffectOrigin => 21,
            Self::InstallationLocatorActivationEffectOrigin => 22,
            Self::CapabilityProbeEffectOrigin => 23,
        }
    }

    pub fn from_tag(tag: u8) -> Result<Self, EffectRuntimeErrorV1> {
        tag.checked_sub(1)
            .and_then(|index| Self::ALL.get(usize::from(index)))
            .copied()
            .ok_or(EffectRuntimeErrorV1::UnknownEffectOrigin(tag))
    }

    pub const fn is_step(self) -> bool {
        matches!(self, Self::StepEffectOrigin)
    }

    const fn action_route(self) -> Option<EffectActionRouteV1> {
        match self {
            Self::StepEffectOrigin
            | Self::EffectRemediationOrigin
            | Self::GovernedReviewPresentationDeliveryOrigin
            | Self::GovernedReviewResponseAcquisitionOrigin
            | Self::PlannedTurnoverHighWaterEffectOrigin
            | Self::GovernedReviewPublicationHighWaterEffectOrigin
            | Self::DistributionArtifactAcquisitionEffectOrigin
            | Self::DistributionFilesystemMutationEffectOrigin
            | Self::DistributionManagerOperationEffectOrigin
            | Self::DistributionBinaryActivationEffectOrigin
            | Self::CapabilityProbeEffectOrigin => Some(EffectActionRouteV1::Ordinary),
            Self::CoordinationDeliveryEffectOrigin => {
                Some(EffectActionRouteV1::CoordinationDelivery)
            }
            Self::BootstrapMandatePresentationDeliveryOrigin
            | Self::BootstrapMandateResponseAcquisitionOrigin => {
                Some(EffectActionRouteV1::Bootstrap)
            }
            Self::TrustedTimeAcquisitionEffectOrigin
            | Self::RecoveryExternalRegistrationEffectOrigin
            | Self::RecoveryExternalStatusEffectOrigin
            | Self::MaintenanceExecutorCurrentnessEffectOrigin
            | Self::ProspectiveContinuityCarrierEffectOrigin => {
                Some(EffectActionRouteV1::ContinuityMaintenance)
            }
            Self::RepositoryRecoveryAdmissionEffectOrigin
            | Self::InstallationRecoveryAdmissionEffectOrigin
            | Self::RepositoryGenerationActivationEffectOrigin
            | Self::InstallationLocatorActivationEffectOrigin => None,
        }
    }

    pub(crate) fn withdrawal_authority_path(
        self,
    ) -> Result<WithdrawalAuthorityPathV1, EffectRuntimeErrorV1> {
        withdrawal_path(self, require_action_route(self)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectActionRouteV1 {
    Ordinary,
    CoordinationDelivery,
    Bootstrap,
    ContinuityMaintenance,
}

impl EffectActionRouteV1 {
    const fn reserve(self) -> ExecutionActionV1 {
        match self {
            Self::Ordinary => ExecutionActionV1::OriginateEffectIntent,
            Self::CoordinationDelivery => ExecutionActionV1::OriginateCoordinationDelivery,
            Self::Bootstrap => ExecutionActionV1::ReserveBootstrapMandateInteractionEffect,
            Self::ContinuityMaintenance => ExecutionActionV1::ReserveContinuityMaintenanceEffect,
        }
    }

    const fn outcome(self) -> ExecutionActionV1 {
        match self {
            Self::Ordinary | Self::CoordinationDelivery => ExecutionActionV1::RecordDispatchOutcome,
            Self::Bootstrap => ExecutionActionV1::PublishBootstrapMandateInteractionOutcome,
            Self::ContinuityMaintenance => {
                ExecutionActionV1::PublishContinuityMaintenanceEffectOutcome
            }
        }
    }

    const fn reconcile(self) -> ExecutionActionV1 {
        match self {
            Self::Ordinary | Self::CoordinationDelivery => ExecutionActionV1::ReconcileEffectIntent,
            Self::Bootstrap => ExecutionActionV1::ReconcileBootstrapMandateInteractionEffect,
            Self::ContinuityMaintenance => ExecutionActionV1::ReconcileContinuityMaintenanceEffect,
        }
    }

    const fn withdraw(self) -> ExecutionActionV1 {
        match self {
            Self::Ordinary | Self::CoordinationDelivery => ExecutionActionV1::WithdrawEffectIntent,
            Self::Bootstrap => ExecutionActionV1::WithdrawBootstrapMandateInteractionEffect,
            Self::ContinuityMaintenance => ExecutionActionV1::WithdrawContinuityMaintenanceEffect,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepEffectOriginV1 {
    binding: StepBindingV1,
    attempt_id: StepAttemptIdV1,
    lease_id: StepLeaseIdV1,
    lease_fence: u64,
    lease_term_id: LeaseTermIdV1,
    lease_term_ordinal: u64,
}

impl StepEffectOriginV1 {
    pub fn from_live_tenure(
        tenure: &StepExecutionTenureV1,
        as_of: u64,
    ) -> Result<Self, EffectRuntimeErrorV1> {
        let attempt = tenure.attempt();
        let lease = tenure.lease();
        let term = tenure.current_term();
        if attempt.state() != StepAttemptStateV1::Live
            || lease.state() != StepAttemptStateV1::Live
            || !term.is_live_at(as_of)
            || attempt.id() != lease.attempt_id()
            || attempt.lease_id() != lease.id()
            || attempt.binding() != lease.binding()
            || attempt.fence() == 0
            || attempt.fence() != lease.fence()
            || term.id() != lease.current_term_id()
        {
            return Err(EffectRuntimeErrorV1::StepOriginRequiresLiveLeaseAuthority);
        }
        Ok(Self {
            binding: attempt.binding(),
            attempt_id: attempt.id(),
            lease_id: lease.id(),
            lease_fence: lease.fence(),
            lease_term_id: term.id(),
            lease_term_ordinal: term.ordinal(),
        })
    }

    pub const fn binding(self) -> StepBindingV1 {
        self.binding
    }

    pub const fn attempt_id(self) -> StepAttemptIdV1 {
        self.attempt_id
    }

    pub const fn lease_id(self) -> StepLeaseIdV1 {
        self.lease_id
    }

    pub const fn lease_fence(self) -> u64 {
        self.lease_fence
    }

    pub const fn lease_term_id(self) -> LeaseTermIdV1 {
        self.lease_term_id
    }

    pub const fn lease_term_ordinal(self) -> u64 {
        self.lease_term_ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonStepEffectOriginV1 {
    kind: EffectOriginKindV1,
    provenance_commitment: [u8; 32],
}

impl NonStepEffectOriginV1 {
    pub fn new(
        kind: EffectOriginKindV1,
        provenance_commitment: [u8; 32],
    ) -> Result<Self, EffectRuntimeErrorV1> {
        if kind.is_step() {
            return Err(EffectRuntimeErrorV1::StepOriginRequiresLiveLeaseAuthority);
        }
        require_nonzero(provenance_commitment)?;
        Ok(Self {
            kind,
            provenance_commitment,
        })
    }

    pub const fn kind(self) -> EffectOriginKindV1 {
        self.kind
    }

    pub const fn provenance_commitment(self) -> [u8; 32] {
        self.provenance_commitment
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectOriginV1 {
    Step(Box<StepEffectOriginV1>),
    NonStep(NonStepEffectOriginV1),
}

impl EffectOriginV1 {
    pub fn step(tenure: &StepExecutionTenureV1, as_of: u64) -> Result<Self, EffectRuntimeErrorV1> {
        Ok(Self::Step(Box::new(StepEffectOriginV1::from_live_tenure(
            tenure, as_of,
        )?)))
    }

    pub fn non_step(
        kind: EffectOriginKindV1,
        provenance_commitment: [u8; 32],
    ) -> Result<Self, EffectRuntimeErrorV1> {
        Ok(Self::NonStep(NonStepEffectOriginV1::new(
            kind,
            provenance_commitment,
        )?))
    }

    pub const fn kind(&self) -> EffectOriginKindV1 {
        match self {
            Self::Step(_) => EffectOriginKindV1::StepEffectOrigin,
            Self::NonStep(origin) => origin.kind(),
        }
    }

    pub fn step_authority(&self) -> Option<StepEffectOriginV1> {
        match self {
            Self::Step(origin) => Some(**origin),
            Self::NonStep(_) => None,
        }
    }

    pub fn originating_step_binding(&self) -> Option<StepBindingV1> {
        match self {
            Self::Step(origin) => Some(origin.binding()),
            Self::NonStep(_) => None,
        }
    }

    pub fn commitment(&self) -> Result<[u8; 32], EffectRuntimeErrorV1> {
        Ok(hash(&self.canonical_value())?)
    }

    pub fn reservation_action(&self) -> Result<ExecutionActionV1, EffectRuntimeErrorV1> {
        Ok(require_action_route(self.kind())?.reserve())
    }

    pub fn outcome_action(&self) -> Result<ExecutionActionV1, EffectRuntimeErrorV1> {
        Ok(require_action_route(self.kind())?.outcome())
    }

    pub fn reconciliation_action(&self) -> Result<ExecutionActionV1, EffectRuntimeErrorV1> {
        Ok(require_action_route(self.kind())?.reconcile())
    }

    pub fn withdrawal_action(&self) -> Result<ExecutionActionV1, EffectRuntimeErrorV1> {
        Ok(require_action_route(self.kind())?.withdraw())
    }

    pub(crate) fn withdrawal_authority_path(
        &self,
    ) -> Result<WithdrawalAuthorityPathV1, EffectRuntimeErrorV1> {
        self.kind().withdrawal_authority_path()
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Step(origin) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(u64::from(self.kind().tag())),
                step_binding_value(origin.binding()),
                bytes(origin.attempt_id().as_bytes()),
                bytes(origin.lease_id().as_bytes()),
                CborValue::Unsigned(origin.lease_fence()),
                bytes(origin.lease_term_id().as_bytes()),
                CborValue::Unsigned(origin.lease_term_ordinal()),
            ]),
            Self::NonStep(origin) => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Unsigned(u64::from(origin.kind().tag())),
                bytes(&origin.provenance_commitment()),
            ]),
        }
    }
}

macro_rules! effect_commitment {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn new(bytes: [u8; 32]) -> Result<Self, EffectRuntimeErrorV1> {
                require_nonzero(bytes)?;
                Ok(Self(bytes))
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

effect_commitment!(EffectSemanticUseV1);
effect_commitment!(EffectMaterialInputsV1);
effect_commitment!(EffectCredentialRequirementsV1);
effect_commitment!(EffectOriginationAuthorityCommitmentV1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectOriginationAuthorityV1 {
    commitment: EffectOriginationAuthorityCommitmentV1,
    action_request_id: Option<ActionRequestIdV1>,
    route: EffectOriginationAuthorityRouteV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectOriginationAuthorityRouteV1 {
    Action {
        action: ExecutionActionV1,
        reservation_mode: DispatchReservationModeV1,
    },
    Ceremony {
        ceremony_symbol_tag: u8,
        request_mode: CeremonyRequestModeV1,
    },
}

impl EffectOriginationAuthorityV1 {
    pub fn from_action(
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<Self, EffectRuntimeErrorV1> {
        let value = CborValue::Array(vec![
            CborValue::Unsigned(1),
            authority.request().canonical_value()?,
            authority.receipt().schema_value()?,
        ]);
        Ok(Self {
            commitment: EffectOriginationAuthorityCommitmentV1::new(hash(&value)?)?,
            action_request_id: Some(authority.request_id()),
            route: EffectOriginationAuthorityRouteV1::Action {
                action: authority.action(),
                reservation_mode: DispatchReservationModeV1::InitiateNew,
            },
        })
    }

    pub fn ceremony(
        commitment: [u8; 32],
        ceremony_symbol_tag: u8,
        request_mode: CeremonyRequestModeV1,
    ) -> Result<Self, EffectRuntimeErrorV1> {
        ceremony_symbol_descriptor_id(ceremony_symbol_tag)?;
        Ok(Self {
            commitment: EffectOriginationAuthorityCommitmentV1::new(commitment)?,
            action_request_id: None,
            route: EffectOriginationAuthorityRouteV1::Ceremony {
                ceremony_symbol_tag,
                request_mode,
            },
        })
    }

    pub const fn commitment(self) -> EffectOriginationAuthorityCommitmentV1 {
        self.commitment
    }

    pub const fn action_request_id(self) -> Option<ActionRequestIdV1> {
        self.action_request_id
    }

    pub const fn route(self) -> EffectOriginationAuthorityRouteV1 {
        self.route
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentOriginationV1 {
    pub home: EffectIntentHomeV1,
    pub origin: EffectOriginV1,
    pub origination_fence: EffectIntentOriginationFenceV1,
    pub semantic_use: EffectSemanticUseV1,
    pub material_inputs: EffectMaterialInputsV1,
    pub credential_requirements: EffectCredentialRequirementsV1,
    pub authority: EffectOriginationAuthorityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentDraftV1 {
    pub home: EffectIntentHomeV1,
    pub origin: EffectOriginV1,
    pub origination_fence: EffectIntentOriginationFenceV1,
    pub semantic_use: EffectSemanticUseV1,
    pub material_inputs: EffectMaterialInputsV1,
    pub credential_requirements: EffectCredentialRequirementsV1,
}

impl EffectIntentDraftV1 {
    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-intent-draft.v1")?,
            home_value(self.home)?,
            self.origin.canonical_value(),
            origination_fence_value(self.origination_fence)?,
            bytes(self.semantic_use.as_bytes()),
            bytes(self.material_inputs.as_bytes()),
            bytes(self.credential_requirements.as_bytes()),
        ]))
    }

    pub fn authorize(
        self,
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectIntentV1, EffectRuntimeErrorV1> {
        EffectIntentV1::originate(EffectIntentOriginationV1 {
            home: self.home,
            origin: self.origin,
            origination_fence: self.origination_fence,
            semantic_use: self.semantic_use,
            material_inputs: self.material_inputs,
            credential_requirements: self.credential_requirements,
            authority: EffectOriginationAuthorityV1::from_action(authority)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentV1 {
    id: EffectIntentIdV1,
    home: EffectIntentHomeV1,
    home_commitment: [u8; 32],
    origin: EffectOriginV1,
    origination_fence: EffectIntentOriginationFenceV1,
    origination_fence_commitment: [u8; 32],
    semantic_use: EffectSemanticUseV1,
    material_inputs: EffectMaterialInputsV1,
    credential_requirements: EffectCredentialRequirementsV1,
    origination_authority: EffectOriginationAuthorityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentInitialControlV1 {
    revision: EffectIntentControlRevisionV1,
    writer_term: EffectIntentControlWriterTermV1,
    head: EffectIntentControlHeadV1,
}

impl EffectIntentInitialControlV1 {
    pub const fn revision(&self) -> &EffectIntentControlRevisionV1 {
        &self.revision
    }

    pub const fn writer_term(&self) -> EffectIntentControlWriterTermV1 {
        self.writer_term
    }

    pub const fn head(&self) -> &EffectIntentControlHeadV1 {
        &self.head
    }

    pub fn into_parts(
        self,
    ) -> (
        EffectIntentControlRevisionV1,
        EffectIntentControlWriterTermV1,
        EffectIntentControlHeadV1,
    ) {
        (self.revision, self.writer_term, self.head)
    }
}

impl EffectIntentV1 {
    pub fn originate(origination: EffectIntentOriginationV1) -> Result<Self, EffectRuntimeErrorV1> {
        if origination.origination_fence.home_kind() != origination.home.kind() {
            return Err(EffectIntentHomeError::OriginationFenceHomeMismatch.into());
        }
        validate_origination_route(
            origination.origin.kind(),
            origination.home.kind(),
            origination.authority,
        )?;
        let home_value = home_value(origination.home)?;
        let origination_fence_value = origination_fence_value(origination.origination_fence)?;
        let home_commitment = hash(&home_value)?;
        let origination_fence_commitment = hash(&origination_fence_value)?;
        let value = effect_intent_value(
            home_value,
            &origination.origin,
            origination_fence_value,
            origination.semantic_use,
            origination.material_inputs,
            origination.credential_requirements,
            origination.authority,
        )?;
        Ok(Self {
            id: EffectIntentIdV1::from_bytes(hash(&value)?)?,
            home: origination.home,
            home_commitment,
            origin: origination.origin,
            origination_fence: origination.origination_fence,
            origination_fence_commitment,
            semantic_use: origination.semantic_use,
            material_inputs: origination.material_inputs,
            credential_requirements: origination.credential_requirements,
            origination_authority: origination.authority,
        })
    }

    pub const fn id(&self) -> EffectIntentIdV1 {
        self.id
    }

    pub const fn home(&self) -> EffectIntentHomeV1 {
        self.home
    }

    pub const fn home_commitment(&self) -> [u8; 32] {
        self.home_commitment
    }

    pub const fn origin(&self) -> &EffectOriginV1 {
        &self.origin
    }

    pub const fn origination_fence(&self) -> EffectIntentOriginationFenceV1 {
        self.origination_fence
    }

    pub const fn origination_fence_commitment(&self) -> [u8; 32] {
        self.origination_fence_commitment
    }

    pub const fn semantic_use(&self) -> EffectSemanticUseV1 {
        self.semantic_use
    }

    pub const fn material_inputs(&self) -> EffectMaterialInputsV1 {
        self.material_inputs
    }

    pub const fn credential_requirements(&self) -> EffectCredentialRequirementsV1 {
        self.credential_requirements
    }

    pub const fn origination_authority(&self) -> EffectOriginationAuthorityV1 {
        self.origination_authority
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        effect_intent_value(
            home_value(self.home)?,
            &self.origin,
            origination_fence_value(self.origination_fence)?,
            self.semantic_use,
            self.material_inputs,
            self.credential_requirements,
            self.origination_authority,
        )
    }

    pub fn persistence_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-intent.persistence-carrier.v1")?,
            bytes(self.id.as_bytes()),
            self.canonical_value()?,
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, EffectRuntimeErrorV1> {
        Ok(deterministic_cbor::encode(&self.persistence_value()?)?)
    }

    pub(crate) fn from_persistence_value(value: &CborValue) -> Result<Self, EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [CborValue::Text(persistence_domain), stored_id, canonical] = fields.as_slice() else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let CborValue::Array(parts) = canonical else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            CborValue::Text(manifest),
            CborValue::Text(descriptor),
            home,
            origin,
            origination_fence,
            semantic_use,
            material_inputs,
            credentials,
            authority_commitment,
            action_request,
            route,
        ] = parts.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if persistence_domain != "maestro.vnext.effect-intent.persistence-carrier.v1"
            || domain != "maestro.vnext.effect-intent.v1"
            || manifest != EFFECT_ORIGIN_MANIFEST_ID_V1
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let home = parse_active_store_home(home)?;
        let origin = parse_effect_origin(origin)?;
        if descriptor != origin.kind().descriptor().descriptor_id() {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let origination_fence = parse_active_store_origination_fence(origination_fence)?;
        let action_request_id = parse_optional_effect_digest(action_request)?
            .map(ActionRequestIdV1::from_digest)
            .ok_or(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?;
        let authority = EffectOriginationAuthorityV1 {
            commitment: EffectOriginationAuthorityCommitmentV1::new(exact_effect_digest(
                authority_commitment,
            )?)?,
            action_request_id: Some(action_request_id),
            route: parse_action_origination_route(route, origin.kind())?,
        };
        validate_origination_route(origin.kind(), home.kind(), authority)?;
        let home_commitment = hash(&home_value(home)?)?;
        let origination_fence_commitment = hash(&origination_fence_value(origination_fence)?)?;
        let intent = Self {
            id: EffectIntentIdV1::from_bytes(exact_effect_digest(stored_id)?)?,
            home,
            home_commitment,
            origin,
            origination_fence,
            origination_fence_commitment,
            semantic_use: EffectSemanticUseV1::new(exact_effect_digest(semantic_use)?)?,
            material_inputs: EffectMaterialInputsV1::new(exact_effect_digest(material_inputs)?)?,
            credential_requirements: EffectCredentialRequirementsV1::new(exact_effect_digest(
                credentials,
            )?)?,
            origination_authority: authority,
        };
        if intent.id != EffectIntentIdV1::from_bytes(hash(&intent.canonical_value()?)?)?
            || intent.persistence_value()? != *value
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(intent)
    }

    pub fn initial_active_store_control(
        &self,
        use_fence: EffectIntentUseFenceV1,
    ) -> Result<EffectIntentInitialControlV1, EffectRuntimeErrorV1> {
        if !matches!(self.home, EffectIntentHomeV1::ActiveStore(_)) {
            return Err(EffectRuntimeErrorV1::ActiveStoreControlRequired);
        }
        validate_fence_home(self.home, self.origination_fence, use_fence)?;
        let use_fence_commitment = hash(&use_fence_value(use_fence)?)?;
        let revision = EffectIntentControlRevisionV1::new(EffectIntentControlRevisionPartsV1 {
            intent: self.id,
            attempt_history: vec![],
            live_attempt: None,
            live_dispatch: EffectIntentLiveDispatchV1::None,
            classification: RemoteClassificationV1::Prepared,
            dispatch_fence_high_water: 0,
            run_set_revision: 1,
            runs_closed: true,
            material_commitment: *self.material_inputs.as_bytes(),
            credential_commitment: *self.credential_requirements.as_bytes(),
            use_fence_commitment,
            result_commitment: None,
            idempotency_commitment: None,
            health: EffectIntentControlHealthV1::Healthy,
        })?;
        let writer_issuance_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-control-writer-origination.v1")?,
            bytes(self.id.as_bytes()),
            bytes(self.origination_authority.commitment().as_bytes()),
            bytes(&use_fence_commitment),
        ]))?;
        let home = HomeTokenV1::new(self.home_commitment);
        let writer_term =
            EffectIntentControlWriterTermV1::originate(self.id, home, writer_issuance_commitment)?;
        let head = EffectIntentControlHeadV1::new(self.id, home, &revision, writer_term)?;
        Ok(EffectIntentInitialControlV1 {
            revision,
            writer_term,
            head,
        })
    }

    pub fn matches_control_revision(&self, revision: &EffectIntentControlRevisionV1) -> bool {
        let parts = revision.parts();
        parts.intent == self.id
            && parts.material_commitment == *self.material_inputs.as_bytes()
            && parts.credential_commitment == *self.credential_requirements.as_bytes()
    }

    pub fn validate_use_fence(
        &self,
        revision: &EffectIntentControlRevisionV1,
        use_fence: EffectIntentUseFenceV1,
    ) -> Result<[u8; 32], EffectRuntimeErrorV1> {
        validate_fence_home(self.home, self.origination_fence, use_fence)?;
        let exact_same_home = match (self.home, self.origination_fence, use_fence) {
            (
                EffectIntentHomeV1::ActiveStore(home),
                EffectIntentOriginationFenceV1::ActiveStore(origin),
                EffectIntentUseFenceV1::ActiveStore(current),
            ) => {
                home.stable_domain_id == origin.store
                    && home.stable_domain_id == current.same_stable_home
                    && home.semantic_namespace == origin.namespace
                    && home.semantic_namespace == current.namespace
                    && origin.generation == current.generation
                    && origin.epoch == current.epoch
                    && origin.material_token == current.material_token
                    && origin.current_authority_commitment == current.authority
                    && origin.credential_commitment == current.credentials
                    && origin.dispatch_reservation_or_fence == current.attempt_fence
            }
            (
                EffectIntentHomeV1::NoStoreCeremony(home),
                EffectIntentOriginationFenceV1::NoStoreCeremony(origin),
                EffectIntentUseFenceV1::NoStoreCeremony(current),
            ) => {
                home.protected_installation_realm == origin.protected_realm
                    && home.protected_installation_realm == current.same_home
                    && home.locator_candidate_branch == origin.locator_candidate_bundle
                    && origin.candidate_seal == current.branch_authority
                    && origin.carrier_incarnation == current.carrier_incarnation
                    && origin.expected_old_token == current.expected_old_token
                    && origin.attempt_id == current.attempt_id
            }
            (
                EffectIntentHomeV1::PreStoreCeremony(home),
                EffectIntentOriginationFenceV1::PreStoreCeremony(origin),
                EffectIntentUseFenceV1::PreStoreCeremony(current),
            ) => {
                home.allowed_pre_store_ceremony == current.same_home
                    && home.candidate_branch_or_destination == origin.branch_bundle
                    && home.inactive_destination_lineage == origin.inactive_destination
                    && origin.candidate_seal == current.branch_authority
                    && origin.carrier_identity == current.carrier
                    && origin.expected_old_token == current.expected_old_token
                    && origin.attempt_id == current.attempt_id
            }
            _ => false,
        };
        if !exact_same_home {
            return Err(EffectRuntimeErrorV1::UseFenceBasisMismatch);
        }
        let commitment = hash(&use_fence_value(use_fence)?)?;
        if commitment != revision.parts().use_fence_commitment {
            return Err(EffectRuntimeErrorV1::UseFenceNotCurrent);
        }
        Ok(commitment)
    }

    pub fn prepare_dispatch(
        &self,
        revision: &EffectIntentControlRevisionV1,
        preparation: EffectDispatchPreparationV1,
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<PreparedEffectDispatchV1, EffectRuntimeErrorV1> {
        validate_control_basis(self, revision)?;
        let route = require_action_route(self.origin.kind())?;
        require_action(authority, route.reserve())?;
        require_dispatch_request(self, revision, authority.request_id())?;
        if revision.health() != EffectIntentControlHealthV1::Healthy
            || revision.live_attempt().is_some()
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || !matches!(
                revision.classification(),
                RemoteClassificationV1::Prepared | RemoteClassificationV1::ConfirmedNotApplied
            )
            || !revision.runs_closed()
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        let use_fence_commitment =
            self.validate_dispatch_use_fence(revision, &preparation, authority)?;
        let dispatch_fence = revision
            .dispatch_fence_high_water()
            .checked_add(1)
            .ok_or(EffectRuntimeErrorV1::CounterOverflow)?;
        let attempt = DispatchAttemptV1::new(
            self.id,
            dispatch_fence,
            use_fence_commitment,
            self.origin.originating_step_binding(),
        )?;
        let run_set = RunSetV1::reserve_non_step_run_at_revision(
            &ExecutionAttemptV1::Dispatch(attempt),
            preparation.provider_run,
            revision.run_set_revision(),
        )?;
        let next_run_set_revision = run_set.revision();
        if next_run_set_revision
            != revision
                .run_set_revision()
                .checked_add(1)
                .ok_or(EffectRuntimeErrorV1::CounterOverflow)?
        {
            return Err(EffectRuntimeErrorV1::RunSetRevisionMismatch);
        }
        let binding = DispatchBindingV1::new(DispatchBindingPartsV1 {
            attempt_id: commitment(*attempt.id().as_bytes())?,
            attempt_revision: preparation.attempt_revision,
            effect_intent_home_id: commitment(self.home_commitment)?,
            effect_intent_use_fence_id: commitment(use_fence_commitment)?,
            application_envelope_id: commitment(preparation.application_envelope_commitment)?,
            provider_operation_contract_id: commitment(
                preparation.provider_operation_contract_commitment,
            )?,
            provider_scope_id: commitment(preparation.provider_scope_commitment)?,
            provider_key_id: commitment(preparation.provider_key_commitment)?,
            credential_id: commitment(*self.credential_requirements.as_bytes())?,
            authority_basis_id: commitment(preparation.authority_basis_commitment)?,
            dispatch_fence_id: commitment(hash(&CborValue::Array(vec![
                bytes(self.id.as_bytes()),
                CborValue::Unsigned(dispatch_fence),
            ]))?)?,
            material_stamp_id: commitment(preparation.material_stamp_commitment)?,
            run_set_revision_id: commitment(preparation.run_set_revision_commitment)?,
            accounting_basis_id: commitment(preparation.accounting_basis_commitment)?,
        })?;
        let dispatch = EffectDispatchAttemptV1 {
            attempt,
            state: DispatchAttemptStateV1::ReservedUnsealed(Box::new(ReservedUnsealedV1::new(
                binding,
            ))),
            reserve_action_request_id: authority.request_id(),
            seal_action_request_id: None,
            use_fence_commitment,
            originating_step_binding: self.origin.originating_step_binding(),
            terminal_classification: None,
            run_set,
        };
        let control_need = match revision.classification() {
            RemoteClassificationV1::Prepared => EffectControlTransitionNeedV1::ReserveDispatch {
                action_request_id: authority.request_id(),
                attempt: ExecutionAttemptOwnerV1::Dispatch(attempt.id()),
                next_dispatch_fence: dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment: use_fence_commitment,
            },
            RemoteClassificationV1::ConfirmedNotApplied => {
                EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied {
                    action_request_id: authority.request_id(),
                    attempt: ExecutionAttemptOwnerV1::Dispatch(attempt.id()),
                    next_dispatch_fence: dispatch_fence,
                    next_run_set_revision,
                    next_use_fence_commitment: use_fence_commitment,
                }
            }
            _ => return Err(EffectRuntimeErrorV1::IllegalEffectState),
        };
        Ok(PreparedEffectDispatchV1 {
            dispatch,
            control_need,
        })
    }

    fn validate_dispatch_use_fence(
        &self,
        revision: &EffectIntentControlRevisionV1,
        preparation: &EffectDispatchPreparationV1,
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<[u8; 32], EffectRuntimeErrorV1> {
        if revision.classification() == RemoteClassificationV1::Prepared {
            return self.validate_use_fence(revision, preparation.use_fence);
        }
        validate_fence_home(self.home, self.origination_fence, preparation.use_fence)?;
        let (EffectIntentHomeV1::ActiveStore(home), EffectIntentUseFenceV1::ActiveStore(current)) =
            (self.home, preparation.use_fence)
        else {
            return Err(EffectRuntimeErrorV1::UseFenceBasisMismatch);
        };
        let expected_attempt_fence = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-reservation-fence.v1")?,
            bytes(authority.request_id().as_bytes()),
            bytes(&preparation.provider_key_commitment),
            bytes(home.semantic_namespace.as_bytes()),
        ]))?;
        if home.stable_domain_id != current.same_stable_home
            || home.semantic_namespace != current.namespace
            || current.material_token.as_bytes() != self.material_inputs.as_bytes()
            || current.credentials.as_bytes() != self.credential_requirements.as_bytes()
            || current.authority.as_bytes() != &preparation.authority_basis_commitment
            || current.idempotency_binding.as_bytes()
                != authority.request().idempotency_key_id().as_bytes()
            || current.attempt_fence.as_bytes() != &expected_attempt_fence
            || current.provider_contract_guards.as_bytes()
                != &preparation.provider_operation_contract_commitment
            || current.generation.as_bytes() == &[0; 32]
            || current.epoch.as_bytes() == &[0; 32]
        {
            return Err(EffectRuntimeErrorV1::UseFenceBasisMismatch);
        }
        let commitment = hash(&use_fence_value(preparation.use_fence)?)?;
        if commitment == revision.parts().use_fence_commitment {
            return Err(EffectRuntimeErrorV1::UseFenceNotCurrent);
        }
        Ok(commitment)
    }

    pub fn prepare_reconciliation(
        &self,
        revision: &EffectIntentControlRevisionV1,
        preparation: EffectReconciliationPreparationV1,
        authority: &AuthorizedExecutionActionV1,
        prior_reconciliation_requests: &[ActionRequestIdV1],
    ) -> Result<PreparedEffectReconciliationV1, EffectRuntimeErrorV1> {
        validate_control_basis(self, revision)?;
        let route = require_action_route(self.origin.kind())?;
        require_action(authority, route.reconcile())?;
        require_fresh_request(self, authority.request_id(), prior_reconciliation_requests)?;
        if revision.live_attempt().is_some()
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || !matches!(
                revision.classification(),
                RemoteClassificationV1::Pending
                    | RemoteClassificationV1::InDoubt
                    | RemoteClassificationV1::PartiallyApplied
                    | RemoteClassificationV1::Conflicted
            )
            || !revision.runs_closed()
            || revision.health() == EffectIntentControlHealthV1::IntegrityBlocked
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        let read_plan_commitment = preparation.read_plan.commitment()?;
        let use_fence_commitment = self.validate_reconciliation_use_fence(
            revision,
            preparation.use_fence,
            authority,
            read_plan_commitment,
        )?;
        let attempt = ReconciliationAttemptV1::new(
            self.id,
            use_fence_commitment,
            read_plan_commitment,
            self.origin.originating_step_binding(),
            authority,
        )?;
        if preparation.read_run.semantic_operation_hash != read_plan_commitment
            || preparation.read_run.launch_ordinal != 1
            || preparation.read_run.current_step_term.is_some()
        {
            return Err(EffectRuntimeErrorV1::InvalidReconciliationReadRun);
        }
        let execution_attempt = ExecutionAttemptV1::Reconciliation(attempt);
        let run_set = RunSetV1::reserve_non_step_run_at_revision(
            &execution_attempt,
            preparation.read_run,
            revision.run_set_revision(),
        )?;
        let next_run_set_revision = run_set.revision();
        let reconciliation = EffectReconciliationAttemptV1 {
            attempt,
            starting_classification: revision.classification(),
            use_fence: preparation.use_fence,
            use_fence_commitment,
            read_plan: preparation.read_plan,
            run_set,
            read_execution_request_id: None,
            read_usage: None,
            originating_step_binding: self.origin.originating_step_binding(),
        };
        let control_need = EffectControlTransitionNeedV1::BeginReconciliation {
            action_request_id: authority.request_id(),
            attempt: ExecutionAttemptOwnerV1::Reconciliation(attempt.id()),
            next_run_set_revision,
            next_use_fence_commitment: use_fence_commitment,
        };
        Ok(PreparedEffectReconciliationV1 {
            reconciliation,
            control_need,
        })
    }

    fn validate_reconciliation_use_fence(
        &self,
        revision: &EffectIntentControlRevisionV1,
        use_fence: EffectIntentUseFenceV1,
        authority: &AuthorizedExecutionActionV1,
        read_plan_commitment: [u8; 32],
    ) -> Result<[u8; 32], EffectRuntimeErrorV1> {
        validate_fence_home(self.home, self.origination_fence, use_fence)?;
        let (EffectIntentHomeV1::ActiveStore(home), EffectIntentUseFenceV1::ActiveStore(current)) =
            (self.home, use_fence)
        else {
            return Err(EffectRuntimeErrorV1::UseFenceBasisMismatch);
        };
        let expected_attempt_fence = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-attempt-fence.v1")?,
            bytes(self.id.as_bytes()),
            bytes(authority.request_id().as_bytes()),
        ]))?;
        if home.stable_domain_id != current.same_stable_home
            || home.semantic_namespace != current.namespace
            || current.material_token.as_bytes() != self.material_inputs.as_bytes()
            || current.credentials.as_bytes() != self.credential_requirements.as_bytes()
            || current.idempotency_binding.as_bytes()
                != authority.request().idempotency_key_id().as_bytes()
            || current.attempt_fence.as_bytes() != &expected_attempt_fence
            || current.provider_contract_guards.as_bytes() != &read_plan_commitment
            || current.generation.as_bytes() == &[0; 32]
            || current.epoch.as_bytes() == &[0; 32]
            || current.authority.as_bytes() == &[0; 32]
        {
            return Err(EffectRuntimeErrorV1::UseFenceBasisMismatch);
        }
        let commitment = hash(&use_fence_value(use_fence)?)?;
        if commitment == revision.parts().use_fence_commitment {
            return Err(EffectRuntimeErrorV1::UseFenceNotCurrent);
        }
        Ok(commitment)
    }

    pub fn prepare_withdrawal(
        &self,
        revision: &EffectIntentControlRevisionV1,
        current_carrier: EffectWithdrawalCurrentCarrierV1,
        authority: &AuthorizedExecutionActionV1,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    ) -> Result<EffectWithdrawalV1, EffectRuntimeErrorV1> {
        validate_control_basis(self, revision)?;
        let route = require_action_route(self.origin.kind())?;
        require_action(authority, route.withdraw())?;
        require_fresh_request(self, authority.request_id(), &[])?;
        let path = withdrawal_path(self.origin.kind(), route)?;
        current_carrier.validate(self, revision, authority, path)?;
        let classification = revision.classification();
        let catalog_cell = action_withdrawal_catalog_cell_v1(
            classification,
            self.origin.kind().tag(),
            route.withdraw().global_tag(),
            path,
        )?;
        let request = WithdrawalRequestV1 {
            catalog_cell,
            home: self.home.kind(),
            path,
            live_dispatch: revision.live_dispatch(),
            classification,
            has_live_attempt: revision.live_attempt().is_some(),
            has_dispatch_fence: revision.live_dispatch() != EffectIntentLiveDispatchV1::None,
            has_seal: revision.live_dispatch() == EffectIntentLiveDispatchV1::Sealed,
            has_release_capability: false,
            runs_closed: revision.runs_closed(),
            same_home_current: true,
            authority_current: true,
            capacity_current: true,
            expected_old_head: true,
            expected_old_carrier: true,
        };
        validate_withdrawal(request)?;
        require_nonzero(result_commitment)?;
        require_nonzero(idempotency_commitment)?;
        if revision.live_attempt().is_some()
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || !matches!(
                revision.classification(),
                RemoteClassificationV1::Prepared | RemoteClassificationV1::ConfirmedNotApplied
            )
            || !revision.runs_closed()
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        let next_run_set_revision = revision
            .run_set_revision()
            .checked_add(1)
            .ok_or(EffectRuntimeErrorV1::CounterOverflow)?;
        Ok(EffectWithdrawalV1 {
            intent_id: self.id,
            request,
            control_need: EffectControlTransitionNeedV1::Withdraw {
                action_request_id: authority.request_id(),
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDispatchPreparationV1 {
    pub use_fence: EffectIntentUseFenceV1,
    pub attempt_revision: u64,
    pub application_envelope_commitment: [u8; 32],
    pub provider_operation_contract_commitment: [u8; 32],
    pub provider_scope_commitment: [u8; 32],
    pub provider_key_commitment: [u8; 32],
    pub authority_basis_commitment: [u8; 32],
    pub material_stamp_commitment: [u8; 32],
    pub run_set_revision_commitment: [u8; 32],
    pub accounting_basis_commitment: [u8; 32],
    pub provider_run: RunReservationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDispatchBindingInputsV1 {
    pub attempt_revision: u64,
    pub application_envelope_commitment: [u8; 32],
    pub provider_operation_contract_commitment: [u8; 32],
    pub provider_scope_commitment: [u8; 32],
    pub provider_key_commitment: [u8; 32],
    pub material_stamp_commitment: [u8; 32],
    pub run_set_revision_commitment: [u8; 32],
    pub accounting_basis_commitment: [u8; 32],
    pub provider_run: RunReservationV1,
}

impl EffectDispatchBindingInputsV1 {
    pub fn bind(
        self,
        use_fence: EffectIntentUseFenceV1,
        authority_basis_commitment: [u8; 32],
    ) -> EffectDispatchPreparationV1 {
        EffectDispatchPreparationV1 {
            use_fence,
            attempt_revision: self.attempt_revision,
            application_envelope_commitment: self.application_envelope_commitment,
            provider_operation_contract_commitment: self.provider_operation_contract_commitment,
            provider_scope_commitment: self.provider_scope_commitment,
            provider_key_commitment: self.provider_key_commitment,
            authority_basis_commitment,
            material_stamp_commitment: self.material_stamp_commitment,
            run_set_revision_commitment: self.run_set_revision_commitment,
            accounting_basis_commitment: self.accounting_basis_commitment,
            provider_run: self.provider_run,
        }
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        dispatch_binding_inputs_value(
            self.attempt_revision,
            self.application_envelope_commitment,
            self.provider_operation_contract_commitment,
            self.provider_scope_commitment,
            self.provider_key_commitment,
            self.material_stamp_commitment,
            self.run_set_revision_commitment,
            self.accounting_basis_commitment,
            &self.provider_run,
        )
    }
}

impl EffectDispatchPreparationV1 {
    fn binding_inputs_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        dispatch_binding_inputs_value(
            self.attempt_revision,
            self.application_envelope_commitment,
            self.provider_operation_contract_commitment,
            self.provider_scope_commitment,
            self.provider_key_commitment,
            self.material_stamp_commitment,
            self.run_set_revision_commitment,
            self.accounting_basis_commitment,
            &self.provider_run,
        )
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-preparation.v1")?,
            use_fence_value(self.use_fence)?,
            self.binding_inputs_value()?,
        ]))
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_binding_inputs_value(
    attempt_revision: u64,
    application_envelope_commitment: [u8; 32],
    provider_operation_contract_commitment: [u8; 32],
    provider_scope_commitment: [u8; 32],
    provider_key_commitment: [u8; 32],
    material_stamp_commitment: [u8; 32],
    run_set_revision_commitment: [u8; 32],
    accounting_basis_commitment: [u8; 32],
    provider_run: &RunReservationV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-dispatch-binding-inputs.v1")?,
        CborValue::Unsigned(attempt_revision),
        bytes(&application_envelope_commitment),
        bytes(&provider_operation_contract_commitment),
        bytes(&provider_scope_commitment),
        bytes(&provider_key_commitment),
        bytes(&material_stamp_commitment),
        bytes(&run_set_revision_commitment),
        bytes(&accounting_basis_commitment),
        CborValue::Array(vec![
            bytes(&provider_run.semantic_operation_hash),
            bytes(&provider_run.inputs_commitment),
            bytes(&provider_run.environment_commitment),
            bytes(&provider_run.target_commitment),
            bytes(&provider_run.execution_boundary_commitment),
            CborValue::Unsigned(provider_run.deadline),
            CborValue::Unsigned(u64::from(provider_run.launch_ordinal)),
            CborValue::optional(
                provider_run
                    .current_step_term
                    .map(|term| bytes(term.as_bytes())),
            ),
        ]),
    ]))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDispatchAttemptV1 {
    attempt: DispatchAttemptV1,
    state: DispatchAttemptStateV1,
    reserve_action_request_id: ActionRequestIdV1,
    seal_action_request_id: Option<ActionRequestIdV1>,
    use_fence_commitment: [u8; 32],
    originating_step_binding: Option<StepBindingV1>,
    terminal_classification: Option<RemoteClassificationV1>,
    run_set: RunSetV1,
}

impl EffectDispatchAttemptV1 {
    pub const fn attempt(&self) -> DispatchAttemptV1 {
        self.attempt
    }

    pub const fn state(&self) -> &DispatchAttemptStateV1 {
        &self.state
    }

    pub const fn reserve_action_request_id(&self) -> ActionRequestIdV1 {
        self.reserve_action_request_id
    }

    pub const fn seal_action_request_id(&self) -> Option<ActionRequestIdV1> {
        self.seal_action_request_id
    }

    pub const fn use_fence_commitment(&self) -> [u8; 32] {
        self.use_fence_commitment
    }

    pub const fn terminal_classification(&self) -> Option<RemoteClassificationV1> {
        self.terminal_classification
    }

    pub const fn run_set(&self) -> &RunSetV1 {
        &self.run_set
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-attempt.v1")?,
            bytes(self.attempt.id().as_bytes()),
            bytes(self.attempt.effect_intent_id().as_bytes()),
            CborValue::Unsigned(self.attempt.dispatch_fence()),
            bytes(&self.use_fence_commitment),
            bytes(self.reserve_action_request_id.as_bytes()),
            CborValue::optional(
                self.seal_action_request_id
                    .map(|request| bytes(request.as_bytes())),
            ),
            CborValue::optional(self.originating_step_binding.map(step_binding_value)),
            self.state.canonical_value(),
            CborValue::optional(
                self.terminal_classification
                    .map(remote_classification_value),
            ),
            self.run_set.canonical_value()?,
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, EffectRuntimeErrorV1> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            attempt_id,
            intent_id,
            CborValue::Unsigned(dispatch_fence),
            use_fence,
            reserve_request,
            seal_request,
            step_provenance,
            state,
            terminal_classification,
            run_set,
        ] = fields.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if domain != "maestro.vnext.effect-dispatch-attempt.v1" {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let originating_step_binding = parse_optional_step_binding(step_provenance)?;
        let attempt = DispatchAttemptV1::from_persisted(
            super::runtime::DispatchAttemptIdV1::from_bytes(exact_effect_digest(attempt_id)?)?,
            EffectIntentIdV1::from_bytes(exact_effect_digest(intent_id)?)?,
            *dispatch_fence,
            exact_effect_digest(use_fence)?,
            originating_step_binding,
        )?;
        let state = DispatchAttemptStateV1::from_canonical_value(state)?;
        if state.binding().attempt_id().as_bytes() != attempt.id().as_bytes() {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let execution_attempt = ExecutionAttemptV1::Dispatch(attempt);
        let transition_count = match &state {
            DispatchAttemptStateV1::ReservedUnsealed(_) => 1,
            DispatchAttemptStateV1::SealedInFlight(_) => 2,
            DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::PreSealLocallyRejected(_),
            ) => 2,
            DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::SealedDispatchTerminal(_),
            ) => 3,
        };
        let CborValue::Array(run_set_fields) = run_set else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let Some(CborValue::Unsigned(run_set_revision)) = run_set_fields.get(3) else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let initial_run_set_revision = run_set_revision
            .checked_sub(transition_count)
            .ok_or(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?;
        let run_set = RunSetV1::from_non_step_canonical_value_at_revision(
            run_set,
            &execution_attempt,
            initial_run_set_revision,
        )?;
        let dispatch = Self {
            attempt,
            state,
            reserve_action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(
                reserve_request,
            )?),
            seal_action_request_id: parse_optional_effect_digest(seal_request)?
                .map(ActionRequestIdV1::from_digest),
            use_fence_commitment: exact_effect_digest(use_fence)?,
            originating_step_binding,
            terminal_classification: parse_optional_remote_classification(terminal_classification)?,
            run_set,
        };
        let seal_expected = matches!(
            dispatch.state,
            DispatchAttemptStateV1::SealedInFlight(_)
                | DispatchAttemptStateV1::Terminal(
                    DispatchAttemptTerminalV1::SealedDispatchTerminal(_)
                )
        );
        let terminal_classification_valid =
            match (&dispatch.state, dispatch.terminal_classification) {
                (DispatchAttemptStateV1::Terminal(terminal), Some(classification)) => {
                    match terminal.outcome() {
                        DispatchAttemptOutcomeV1::LocallyRejected
                        | DispatchAttemptOutcomeV1::DefinitelyNotSent => {
                            classification == RemoteClassificationV1::ConfirmedNotApplied
                        }
                        DispatchAttemptOutcomeV1::AmbiguousTransport => {
                            classification == RemoteClassificationV1::InDoubt
                        }
                        DispatchAttemptOutcomeV1::ResponseReceived => matches!(
                            classification,
                            RemoteClassificationV1::Pending
                                | RemoteClassificationV1::InDoubt
                                | RemoteClassificationV1::ConfirmedApplied
                                | RemoteClassificationV1::ConfirmedNotApplied
                                | RemoteClassificationV1::PartiallyApplied
                                | RemoteClassificationV1::Conflicted
                        ),
                    }
                }
                (DispatchAttemptStateV1::Terminal(_), None) => false,
                (
                    DispatchAttemptStateV1::ReservedUnsealed(_)
                    | DispatchAttemptStateV1::SealedInFlight(_),
                    None,
                ) => true,
                (
                    DispatchAttemptStateV1::ReservedUnsealed(_)
                    | DispatchAttemptStateV1::SealedInFlight(_),
                    Some(_),
                ) => false,
            };
        if seal_expected != dispatch.seal_action_request_id.is_some()
            || matches!(dispatch.state, DispatchAttemptStateV1::Terminal(_))
                != dispatch.terminal_classification.is_some()
            || !terminal_classification_valid
            || dispatch.canonical_value()? != *value
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(dispatch)
    }

    pub(crate) fn from_persistence_carrier_value(
        value: &CborValue,
    ) -> Result<(Self, EffectControlTransitionNeedV1), EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            dispatch,
            control_need_value,
            run_binding,
        ] = fields.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if !matches!(
            domain.as_str(),
            "maestro.vnext.effect-dispatch-reservation-carrier.v1"
                | "maestro.vnext.effect-dispatch-seal-carrier.v1"
                | "maestro.vnext.effect-dispatch-terminal-carrier.v1"
        ) {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let dispatch = Self::from_canonical_value(dispatch)?;
        let control_need = EffectControlTransitionNeedV1::from_canonical_value(control_need_value)?;
        if run_set_binding_value(dispatch.run_set()) != *run_binding {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let owner = ExecutionAttemptOwnerV1::Dispatch(dispatch.attempt.id());
        let valid_phase = match (domain.as_str(), &dispatch.state, &control_need) {
            (
                "maestro.vnext.effect-dispatch-reservation-carrier.v1",
                DispatchAttemptStateV1::ReservedUnsealed(_),
                EffectControlTransitionNeedV1::ReserveDispatch {
                    action_request_id,
                    attempt,
                    next_dispatch_fence,
                    next_run_set_revision,
                    next_use_fence_commitment,
                },
            ) => {
                *action_request_id == dispatch.reserve_action_request_id
                    && *attempt == owner
                    && *next_dispatch_fence == dispatch.attempt.dispatch_fence()
                    && *next_run_set_revision == dispatch.run_set.revision()
                    && *next_use_fence_commitment == dispatch.use_fence_commitment
            }
            (
                "maestro.vnext.effect-dispatch-reservation-carrier.v1",
                DispatchAttemptStateV1::ReservedUnsealed(_),
                EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied {
                    action_request_id,
                    attempt,
                    next_dispatch_fence,
                    next_run_set_revision,
                    next_use_fence_commitment,
                },
            ) => {
                *action_request_id == dispatch.reserve_action_request_id
                    && *attempt == owner
                    && *next_dispatch_fence == dispatch.attempt.dispatch_fence()
                    && *next_run_set_revision == dispatch.run_set.revision()
                    && *next_use_fence_commitment == dispatch.use_fence_commitment
            }
            (
                "maestro.vnext.effect-dispatch-seal-carrier.v1",
                DispatchAttemptStateV1::SealedInFlight(_),
                EffectControlTransitionNeedV1::SealDispatch {
                    action_request_id,
                    attempt,
                    next_run_set_revision,
                },
            ) => {
                Some(*action_request_id) == dispatch.seal_action_request_id
                    && *attempt == owner
                    && *next_run_set_revision == dispatch.run_set.revision()
            }
            (
                "maestro.vnext.effect-dispatch-terminal-carrier.v1",
                DispatchAttemptStateV1::Terminal(_),
                EffectControlTransitionNeedV1::FinishDispatch {
                    attempt,
                    classification,
                    next_run_set_revision,
                    ..
                },
            ) => {
                *attempt == owner
                    && Some(*classification) == dispatch.terminal_classification
                    && *next_run_set_revision == dispatch.run_set.revision()
            }
            (
                "maestro.vnext.effect-dispatch-terminal-carrier.v1",
                DispatchAttemptStateV1::Terminal(
                    DispatchAttemptTerminalV1::SealedDispatchTerminal(terminal),
                ),
                EffectControlTransitionNeedV1::RecoverSealedInDoubt {
                    attempt,
                    next_run_set_revision,
                    ..
                },
            ) => {
                *attempt == owner
                    && terminal.outcome() == SealedDispatchOutcomeV1::AmbiguousTransport
                    && dispatch.terminal_classification == Some(RemoteClassificationV1::InDoubt)
                    && *next_run_set_revision == dispatch.run_set.revision()
            }
            _ => false,
        };
        if !valid_phase || control_need.canonical_value()? != *control_need_value {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok((dispatch, control_need))
    }

    pub fn crossing_seal_value(&self) -> Option<CborValue> {
        match &self.state {
            DispatchAttemptStateV1::SealedInFlight(state) => Some(state.seal().canonical_value()),
            DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::SealedDispatchTerminal(terminal),
            ) => Some(terminal.seal().canonical_value()),
            DispatchAttemptStateV1::ReservedUnsealed(_)
            | DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::PreSealLocallyRejected(_),
            ) => None,
        }
    }

    pub(crate) fn crossing_seal_commitment(&self) -> Option<[u8; 32]> {
        match &self.state {
            DispatchAttemptStateV1::SealedInFlight(state) => {
                Some(*state.seal().seal_id().as_bytes())
            }
            DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::SealedDispatchTerminal(terminal),
            ) => Some(*terminal.seal().seal_id().as_bytes()),
            DispatchAttemptStateV1::ReservedUnsealed(_)
            | DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::PreSealLocallyRejected(_),
            ) => None,
        }
    }

    pub(crate) fn terminal_outcome_payload(
        &self,
    ) -> Result<EffectDispatchOutcomePayloadV1, EffectRuntimeErrorV1> {
        match &self.state {
            DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::PreSealLocallyRejected(terminal),
            ) => Ok(EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment: *terminal.rejection_evidence_id().as_bytes(),
            }),
            DispatchAttemptStateV1::Terminal(
                DispatchAttemptTerminalV1::SealedDispatchTerminal(terminal),
            ) => {
                let evidence_commitment = *terminal.terminal_evidence_id().as_bytes();
                match terminal.outcome() {
                    SealedDispatchOutcomeV1::DefinitelyNotSent => {
                        Ok(EffectDispatchOutcomePayloadV1::DefinitelyNotSent {
                            evidence_commitment,
                        })
                    }
                    SealedDispatchOutcomeV1::AmbiguousTransport => {
                        Ok(EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                            evidence_commitment,
                        })
                    }
                    SealedDispatchOutcomeV1::ResponseReceived => {
                        Ok(EffectDispatchOutcomePayloadV1::ResponseReceived {
                            evidence_commitment,
                            classification: self
                                .terminal_classification
                                .ok_or(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
                        })
                    }
                }
            }
            DispatchAttemptStateV1::ReservedUnsealed(_)
            | DispatchAttemptStateV1::SealedInFlight(_) => {
                Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
            }
        }
    }

    pub fn seal_candidate(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        seal_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectDispatchSealCandidateV1, EffectRuntimeErrorV1> {
        let route = require_action_route(intent.origin.kind())?;
        self.seal_candidate_for_action(
            intent,
            revision,
            seal_commitment,
            authority,
            route.outcome(),
        )
    }

    pub fn recover_reserved_seal_candidate(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        seal_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectDispatchSealCandidateV1, EffectRuntimeErrorV1> {
        let route = require_action_route(intent.origin.kind())?;
        self.seal_candidate_for_action(
            intent,
            revision,
            seal_commitment,
            authority,
            route.reserve(),
        )
    }

    fn seal_candidate_for_action(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        seal_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
        action: ExecutionActionV1,
    ) -> Result<EffectDispatchSealCandidateV1, EffectRuntimeErrorV1> {
        validate_control_basis(intent, revision)?;
        require_action(authority, action)?;
        require_fresh_request(
            intent,
            authority.request_id(),
            &[self.reserve_action_request_id],
        )?;
        if self.attempt.effect_intent_id() != intent.id
            || revision.live_attempt() != Some(ExecutionAttemptOwnerV1::Dispatch(self.attempt.id()))
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::Reserved
            || revision.classification() != RemoteClassificationV1::Dispatching
            || !matches!(self.state, DispatchAttemptStateV1::ReservedUnsealed(_))
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        let seal =
            DispatchCrossingSealV1::new(commitment(seal_commitment)?, self.state.binding().clone());
        let state = DispatchAttemptStateV1::SealedInFlight(Box::new(SealedInFlightV1::new(
            self.state.binding().clone(),
            seal,
        )?));
        self.state.validate_transition_to(&state)?;
        let mut dispatch = self.clone();
        dispatch.state = state;
        dispatch.seal_action_request_id = Some(authority.request_id());
        let run = dispatch
            .run_set
            .runs()
            .first()
            .ok_or(EffectRuntimeErrorV1::MissingAttemptRun)?;
        dispatch.run_set.transition_non_step_run(
            run.id(),
            revision.run_set_revision(),
            RunStateV1::Active,
        )?;
        let next_run_set_revision = dispatch.run_set.revision();
        Ok(EffectDispatchSealCandidateV1 {
            dispatch,
            control_need: EffectControlTransitionNeedV1::SealDispatch {
                action_request_id: authority.request_id(),
                attempt: ExecutionAttemptOwnerV1::Dispatch(self.attempt.id()),
                next_run_set_revision,
            },
        })
    }

    pub fn terminal_candidate(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        outcome: EffectDispatchOutcomePayloadV1,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectDispatchTerminalCandidateV1, EffectRuntimeErrorV1> {
        let route = require_action_route(intent.origin.kind())?;
        self.terminal_candidate_for_action(
            intent,
            revision,
            outcome,
            result_commitment,
            idempotency_commitment,
            authority,
            route.outcome(),
        )
    }

    pub fn recover_reserved_rejection_candidate(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        outcome: EffectDispatchOutcomePayloadV1,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectDispatchTerminalCandidateV1, EffectRuntimeErrorV1> {
        if !matches!(
            outcome,
            EffectDispatchOutcomePayloadV1::LocallyRejected { .. }
        ) {
            return Err(EffectRuntimeErrorV1::RemoteOutcomeRequiresCrossingSeal);
        }
        let route = require_action_route(intent.origin.kind())?;
        self.terminal_candidate_for_action(
            intent,
            revision,
            outcome,
            result_commitment,
            idempotency_commitment,
            authority,
            route.reserve(),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "terminal candidate validation binds the complete authorized publication product"
    )]
    fn terminal_candidate_for_action(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        outcome: EffectDispatchOutcomePayloadV1,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
        action: ExecutionActionV1,
    ) -> Result<EffectDispatchTerminalCandidateV1, EffectRuntimeErrorV1> {
        validate_control_basis(intent, revision)?;
        if self.attempt.effect_intent_id() != intent.id
            || revision.live_attempt() != Some(ExecutionAttemptOwnerV1::Dispatch(self.attempt.id()))
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        require_action(authority, action)?;
        let mut prior = vec![self.reserve_action_request_id];
        prior.extend(self.seal_action_request_id);
        require_fresh_request(intent, authority.request_id(), &prior)?;
        require_nonzero(result_commitment)?;
        require_nonzero(idempotency_commitment)?;
        let classification = outcome.classification()?;
        let state = match outcome {
            EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment,
            } => {
                require_nonzero(evidence_commitment)?;
                DispatchAttemptStateV1::Terminal(DispatchAttemptTerminalV1::PreSealLocallyRejected(
                    Box::new(PreSealLocallyRejectedV1::new(
                        self.state.binding().clone(),
                        commitment(evidence_commitment)?,
                    )),
                ))
            }
            EffectDispatchOutcomePayloadV1::DefinitelyNotSent {
                evidence_commitment,
            }
            | EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment,
            }
            | EffectDispatchOutcomePayloadV1::ResponseReceived {
                evidence_commitment,
                ..
            } => {
                require_nonzero(evidence_commitment)?;
                let seal = match &self.state {
                    DispatchAttemptStateV1::SealedInFlight(state) => state.seal().clone(),
                    _ => return Err(EffectRuntimeErrorV1::RemoteOutcomeRequiresCrossingSeal),
                };
                let sealed_outcome =
                    SealedDispatchOutcomeV1::from_dispatch_outcome(outcome.kind())?;
                DispatchAttemptStateV1::Terminal(DispatchAttemptTerminalV1::SealedDispatchTerminal(
                    Box::new(SealedDispatchTerminalV1::new(
                        self.state.binding().clone(),
                        seal,
                        sealed_outcome,
                        commitment(evidence_commitment)?,
                    )?),
                ))
            }
        };
        self.state.validate_transition_to(&state)?;
        let mut dispatch = self.clone();
        dispatch.state = state;
        dispatch.terminal_classification = Some(classification);
        let run_terminal = match outcome {
            EffectDispatchOutcomePayloadV1::LocallyRejected { .. } => {
                RunStateV1::DefinitelyNotStarted
            }
            EffectDispatchOutcomePayloadV1::DefinitelyNotSent { .. } => RunStateV1::Failed,
            EffectDispatchOutcomePayloadV1::ResponseReceived { .. }
            | EffectDispatchOutcomePayloadV1::AmbiguousTransport { .. } => RunStateV1::Succeeded,
        };
        dispatch
            .run_set
            .close_non_step_runs(revision.run_set_revision(), run_terminal)?;
        let next_run_set_revision = dispatch.run_set.revision();
        Ok(EffectDispatchTerminalCandidateV1 {
            dispatch,
            classification,
            control_need: EffectControlTransitionNeedV1::FinishDispatch {
                action_request_id: authority.request_id(),
                attempt: ExecutionAttemptOwnerV1::Dispatch(self.attempt.id()),
                classification,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            },
        })
    }

    pub fn recover_sealed_in_doubt_candidate(
        &self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectDispatchTerminalCandidateV1, EffectRuntimeErrorV1> {
        validate_control_basis(intent, revision)?;
        let owner = ExecutionAttemptOwnerV1::Dispatch(self.attempt.id());
        if self.attempt.effect_intent_id() != intent.id
            || revision.live_attempt() != Some(owner)
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::Sealed
            || revision.classification() != RemoteClassificationV1::InDoubt
            || revision.runs_closed()
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        require_action(
            authority,
            require_action_route(intent.origin.kind())?.reconcile(),
        )?;
        let mut prior = vec![self.reserve_action_request_id];
        prior.extend(self.seal_action_request_id);
        require_fresh_request(intent, authority.request_id(), &prior)?;
        require_nonzero(result_commitment)?;
        require_nonzero(idempotency_commitment)?;
        let DispatchAttemptStateV1::SealedInFlight(sealed) = &self.state else {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        };
        let state =
            DispatchAttemptStateV1::Terminal(DispatchAttemptTerminalV1::SealedDispatchTerminal(
                Box::new(SealedDispatchTerminalV1::new(
                    self.state.binding().clone(),
                    sealed.seal().clone(),
                    SealedDispatchOutcomeV1::AmbiguousTransport,
                    commitment(result_commitment)?,
                )?),
            ));
        self.state.validate_transition_to(&state)?;
        let mut dispatch = self.clone();
        dispatch.state = state;
        dispatch.terminal_classification = Some(RemoteClassificationV1::InDoubt);
        dispatch
            .run_set
            .close_non_step_runs(revision.run_set_revision(), RunStateV1::Succeeded)?;
        let next_run_set_revision = dispatch.run_set.revision();
        Ok(EffectDispatchTerminalCandidateV1 {
            dispatch,
            classification: RemoteClassificationV1::InDoubt,
            control_need: EffectControlTransitionNeedV1::RecoverSealedInDoubt {
                action_request_id: authority.request_id(),
                attempt: owner,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            },
        })
    }

    pub const fn recovery_disposition(&self) -> DispatchRecoveryDispositionV1 {
        match &self.state {
            DispatchAttemptStateV1::ReservedUnsealed(_) => {
                DispatchRecoveryDispositionV1::ReservedUnsealedNoIo
            }
            DispatchAttemptStateV1::SealedInFlight(_) => {
                DispatchRecoveryDispositionV1::SealedInDoubtNoIo
            }
            DispatchAttemptStateV1::Terminal(terminal) => match terminal.outcome() {
                DispatchAttemptOutcomeV1::LocallyRejected
                | DispatchAttemptOutcomeV1::DefinitelyNotSent => {
                    DispatchRecoveryDispositionV1::TerminalConclusiveNotApplied
                }
                DispatchAttemptOutcomeV1::ResponseReceived => {
                    if matches!(
                        self.terminal_classification,
                        Some(
                            RemoteClassificationV1::Pending
                                | RemoteClassificationV1::InDoubt
                                | RemoteClassificationV1::PartiallyApplied
                                | RemoteClassificationV1::Conflicted
                        )
                    ) {
                        DispatchRecoveryDispositionV1::TerminalResponseUncertain
                    } else {
                        DispatchRecoveryDispositionV1::TerminalResponseConclusive
                    }
                }
                DispatchAttemptOutcomeV1::AmbiguousTransport => {
                    DispatchRecoveryDispositionV1::TerminalInDoubt
                }
            },
        }
    }

    pub(crate) fn validate_persisted_predecessor(
        &self,
        predecessor: &Self,
    ) -> Result<(), EffectRuntimeErrorV1> {
        if self.attempt != predecessor.attempt
            || self.reserve_action_request_id != predecessor.reserve_action_request_id
            || self.use_fence_commitment != predecessor.use_fence_commitment
            || self.originating_step_binding != predecessor.originating_step_binding
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        predecessor.state.validate_transition_to(&self.state)?;
        let mut expected_runs = predecessor.run_set.clone();
        let run_id = expected_runs
            .runs()
            .first()
            .ok_or(EffectRuntimeErrorV1::MissingAttemptRun)?
            .id();
        match &self.state {
            DispatchAttemptStateV1::SealedInFlight(_) => {
                if predecessor.seal_action_request_id.is_some()
                    || self.seal_action_request_id.is_none()
                    || self.terminal_classification.is_some()
                {
                    return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
                }
                expected_runs.transition_non_step_run(
                    run_id,
                    predecessor.run_set.revision(),
                    RunStateV1::Active,
                )?;
            }
            DispatchAttemptStateV1::Terminal(terminal) => {
                if self.seal_action_request_id != predecessor.seal_action_request_id
                    || self.terminal_classification.is_none()
                {
                    return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
                }
                let terminal_run_state = match terminal.outcome() {
                    DispatchAttemptOutcomeV1::LocallyRejected => RunStateV1::DefinitelyNotStarted,
                    DispatchAttemptOutcomeV1::DefinitelyNotSent => RunStateV1::Failed,
                    DispatchAttemptOutcomeV1::ResponseReceived
                    | DispatchAttemptOutcomeV1::AmbiguousTransport => RunStateV1::Succeeded,
                };
                expected_runs
                    .close_non_step_runs(predecessor.run_set.revision(), terminal_run_state)?;
            }
            DispatchAttemptStateV1::ReservedUnsealed(_) => {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            }
        }
        if expected_runs != self.run_set {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectDispatchOutcomePayloadV1 {
    LocallyRejected {
        evidence_commitment: [u8; 32],
    },
    DefinitelyNotSent {
        evidence_commitment: [u8; 32],
    },
    ResponseReceived {
        evidence_commitment: [u8; 32],
        classification: RemoteClassificationV1,
    },
    AmbiguousTransport {
        evidence_commitment: [u8; 32],
    },
}

impl EffectDispatchOutcomePayloadV1 {
    pub const fn kind(self) -> DispatchAttemptOutcomeV1 {
        match self {
            Self::LocallyRejected { .. } => DispatchAttemptOutcomeV1::LocallyRejected,
            Self::DefinitelyNotSent { .. } => DispatchAttemptOutcomeV1::DefinitelyNotSent,
            Self::ResponseReceived { .. } => DispatchAttemptOutcomeV1::ResponseReceived,
            Self::AmbiguousTransport { .. } => DispatchAttemptOutcomeV1::AmbiguousTransport,
        }
    }

    fn classification(self) -> Result<RemoteClassificationV1, EffectRuntimeErrorV1> {
        match self {
            Self::LocallyRejected { .. } | Self::DefinitelyNotSent { .. } => {
                Ok(RemoteClassificationV1::ConfirmedNotApplied)
            }
            Self::AmbiguousTransport { .. } => Ok(RemoteClassificationV1::InDoubt),
            Self::ResponseReceived { classification, .. }
                if matches!(
                    classification,
                    RemoteClassificationV1::Pending
                        | RemoteClassificationV1::InDoubt
                        | RemoteClassificationV1::ConfirmedApplied
                        | RemoteClassificationV1::ConfirmedNotApplied
                        | RemoteClassificationV1::PartiallyApplied
                        | RemoteClassificationV1::Conflicted
                ) =>
            {
                Ok(classification)
            }
            Self::ResponseReceived { .. } => Err(EffectRuntimeErrorV1::InvalidRemoteClassification),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedEffectDispatchV1 {
    dispatch: EffectDispatchAttemptV1,
    control_need: EffectControlTransitionNeedV1,
}

impl PreparedEffectDispatchV1 {
    pub const fn dispatch(&self) -> &EffectDispatchAttemptV1 {
        &self.dispatch
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub fn into_parts(self) -> (EffectDispatchAttemptV1, EffectControlTransitionNeedV1) {
        (self.dispatch, self.control_need)
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        dispatch_transition_carrier_value(
            "maestro.vnext.effect-dispatch-reservation-carrier.v1",
            &self.dispatch,
            &self.control_need,
            self.dispatch.run_set(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDispatchSealCandidateV1 {
    dispatch: EffectDispatchAttemptV1,
    control_need: EffectControlTransitionNeedV1,
}

impl EffectDispatchSealCandidateV1 {
    pub const fn dispatch(&self) -> &EffectDispatchAttemptV1 {
        &self.dispatch
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub fn into_parts(self) -> (EffectDispatchAttemptV1, EffectControlTransitionNeedV1) {
        (self.dispatch, self.control_need)
    }

    pub const fn grants_provider_release_capability(&self) -> bool {
        false
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        dispatch_transition_carrier_value(
            "maestro.vnext.effect-dispatch-seal-carrier.v1",
            &self.dispatch,
            &self.control_need,
            self.dispatch.run_set(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDispatchTerminalCandidateV1 {
    dispatch: EffectDispatchAttemptV1,
    classification: RemoteClassificationV1,
    control_need: EffectControlTransitionNeedV1,
}

impl EffectDispatchTerminalCandidateV1 {
    pub const fn dispatch(&self) -> &EffectDispatchAttemptV1 {
        &self.dispatch
    }

    pub const fn classification(&self) -> RemoteClassificationV1 {
        self.classification
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub fn into_parts(self) -> (EffectDispatchAttemptV1, EffectControlTransitionNeedV1) {
        (self.dispatch, self.control_need)
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        dispatch_transition_carrier_value(
            "maestro.vnext.effect-dispatch-terminal-carrier.v1",
            &self.dispatch,
            &self.control_need,
            self.dispatch.run_set(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchRecoveryDispositionV1 {
    ReservedUnsealedNoIo,
    SealedInDoubtNoIo,
    TerminalConclusiveNotApplied,
    TerminalResponseConclusive,
    TerminalResponseUncertain,
    TerminalInDoubt,
}

impl DispatchRecoveryDispositionV1 {
    pub const fn provider_io_operations(self) -> u8 {
        0
    }

    pub const fn reconstructs_release_capability(self) -> bool {
        false
    }

    pub const fn permits_synthetic_truth(self) -> bool {
        false
    }

    pub const fn permits_synthetic_retry(self) -> bool {
        false
    }

    pub const fn requires_reconciliation(self) -> bool {
        matches!(
            self,
            Self::SealedInDoubtNoIo | Self::TerminalResponseUncertain | Self::TerminalInDoubt
        )
    }

    pub const fn preserves_uncertainty(self) -> bool {
        matches!(
            self,
            Self::SealedInDoubtNoIo | Self::TerminalResponseUncertain | Self::TerminalInDoubt
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationReadOperationKindV1 {
    ProviderStatus,
    AccountState,
    TargetState,
    CorrelationStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationReadOperationClassificationV1 {
    EffectFreeRead,
    EffectingRead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectReconciliationReadPlanV1 {
    operation_kind: ReconciliationReadOperationKindV1,
    provider_commitment: [u8; 32],
    account_commitment: [u8; 32],
    target_commitment: [u8; 32],
    correlation_commitment: [u8; 32],
    credential_commitment: [u8; 32],
    visibility_commitment: [u8; 32],
    query_commitment: [u8; 32],
    evaluator_commitment: [u8; 32],
    max_requests: u16,
    max_pages: u16,
    max_bytes: u64,
    max_duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectReconciliationReadPlanPartsV1 {
    pub classification: ReconciliationReadOperationClassificationV1,
    pub operation_kind: ReconciliationReadOperationKindV1,
    pub provider_commitment: [u8; 32],
    pub account_commitment: [u8; 32],
    pub target_commitment: [u8; 32],
    pub correlation_commitment: [u8; 32],
    pub credential_commitment: [u8; 32],
    pub visibility_commitment: [u8; 32],
    pub query_commitment: [u8; 32],
    pub evaluator_commitment: [u8; 32],
    pub max_requests: u16,
    pub max_pages: u16,
    pub max_bytes: u64,
    pub max_duration_ms: u64,
}

impl EffectReconciliationReadPlanV1 {
    pub fn new(parts: EffectReconciliationReadPlanPartsV1) -> Result<Self, EffectRuntimeErrorV1> {
        if parts.classification != ReconciliationReadOperationClassificationV1::EffectFreeRead {
            return Err(EffectRuntimeErrorV1::EffectingReconciliationRead);
        }
        for commitment in [
            parts.provider_commitment,
            parts.account_commitment,
            parts.target_commitment,
            parts.correlation_commitment,
            parts.credential_commitment,
            parts.visibility_commitment,
            parts.query_commitment,
            parts.evaluator_commitment,
        ] {
            require_nonzero(commitment)?;
        }
        if parts.max_requests == 0
            || parts.max_pages == 0
            || parts.max_bytes == 0
            || parts.max_duration_ms == 0
        {
            return Err(EffectRuntimeErrorV1::InvalidReconciliationReadBounds);
        }
        Ok(Self {
            operation_kind: parts.operation_kind,
            provider_commitment: parts.provider_commitment,
            account_commitment: parts.account_commitment,
            target_commitment: parts.target_commitment,
            correlation_commitment: parts.correlation_commitment,
            credential_commitment: parts.credential_commitment,
            visibility_commitment: parts.visibility_commitment,
            query_commitment: parts.query_commitment,
            evaluator_commitment: parts.evaluator_commitment,
            max_requests: parts.max_requests,
            max_pages: parts.max_pages,
            max_bytes: parts.max_bytes,
            max_duration_ms: parts.max_duration_ms,
        })
    }

    pub const fn operation_kind(self) -> ReconciliationReadOperationKindV1 {
        self.operation_kind
    }

    pub const fn classification(self) -> ReconciliationReadOperationClassificationV1 {
        ReconciliationReadOperationClassificationV1::EffectFreeRead
    }

    pub const fn max_requests(self) -> u16 {
        self.max_requests
    }

    pub const fn max_pages(self) -> u16 {
        self.max_pages
    }

    pub const fn max_bytes(self) -> u64 {
        self.max_bytes
    }

    pub const fn max_duration_ms(self) -> u64 {
        self.max_duration_ms
    }

    pub const fn provider_commitment(self) -> [u8; 32] {
        self.provider_commitment
    }

    pub const fn account_commitment(self) -> [u8; 32] {
        self.account_commitment
    }

    pub const fn target_commitment(self) -> [u8; 32] {
        self.target_commitment
    }

    pub const fn correlation_commitment(self) -> [u8; 32] {
        self.correlation_commitment
    }

    pub const fn credential_commitment(self) -> [u8; 32] {
        self.credential_commitment
    }

    pub const fn visibility_commitment(self) -> [u8; 32] {
        self.visibility_commitment
    }

    pub const fn query_commitment(self) -> [u8; 32] {
        self.query_commitment
    }

    pub const fn evaluator_commitment(self) -> [u8; 32] {
        self.evaluator_commitment
    }

    pub fn canonical_value(self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-read-plan.v1")?,
            CborValue::Unsigned(match self.operation_kind {
                ReconciliationReadOperationKindV1::ProviderStatus => 1,
                ReconciliationReadOperationKindV1::AccountState => 2,
                ReconciliationReadOperationKindV1::TargetState => 3,
                ReconciliationReadOperationKindV1::CorrelationStatus => 4,
            }),
            CborValue::Unsigned(1),
            bytes(&self.provider_commitment),
            bytes(&self.account_commitment),
            bytes(&self.target_commitment),
            bytes(&self.correlation_commitment),
            bytes(&self.credential_commitment),
            bytes(&self.visibility_commitment),
            bytes(&self.query_commitment),
            bytes(&self.evaluator_commitment),
            CborValue::Unsigned(u64::from(self.max_requests)),
            CborValue::Unsigned(u64::from(self.max_pages)),
            CborValue::Unsigned(self.max_bytes),
            CborValue::Unsigned(self.max_duration_ms),
        ]))
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            CborValue::Unsigned(operation_kind),
            CborValue::Unsigned(classification),
            provider,
            account,
            target,
            correlation,
            credential,
            visibility,
            query,
            evaluator,
            CborValue::Unsigned(max_requests),
            CborValue::Unsigned(max_pages),
            CborValue::Unsigned(max_bytes),
            CborValue::Unsigned(max_duration_ms),
        ] = fields.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if domain != "maestro.vnext.effect-reconciliation-read-plan.v1" || *classification != 1 {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let operation_kind = match operation_kind {
            1 => ReconciliationReadOperationKindV1::ProviderStatus,
            2 => ReconciliationReadOperationKindV1::AccountState,
            3 => ReconciliationReadOperationKindV1::TargetState,
            4 => ReconciliationReadOperationKindV1::CorrelationStatus,
            _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
        };
        let plan = Self::new(EffectReconciliationReadPlanPartsV1 {
            classification: ReconciliationReadOperationClassificationV1::EffectFreeRead,
            operation_kind,
            provider_commitment: exact_effect_digest(provider)?,
            account_commitment: exact_effect_digest(account)?,
            target_commitment: exact_effect_digest(target)?,
            correlation_commitment: exact_effect_digest(correlation)?,
            credential_commitment: exact_effect_digest(credential)?,
            visibility_commitment: exact_effect_digest(visibility)?,
            query_commitment: exact_effect_digest(query)?,
            evaluator_commitment: exact_effect_digest(evaluator)?,
            max_requests: u16::try_from(*max_requests)
                .map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
            max_pages: u16::try_from(*max_pages)
                .map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
            max_bytes: *max_bytes,
            max_duration_ms: *max_duration_ms,
        })?;
        if plan.canonical_value()? != *value {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(plan)
    }

    pub fn commitment(self) -> Result<[u8; 32], EffectRuntimeErrorV1> {
        Ok(hash(&self.canonical_value()?)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectReconciliationReadUsageV1 {
    pub requests: u16,
    pub pages: u16,
    pub bytes: u64,
    pub duration_ms: u64,
    pub result_commitment: [u8; 32],
}

fn validate_reconciliation_read_usage(
    plan: EffectReconciliationReadPlanV1,
    usage: EffectReconciliationReadUsageV1,
) -> Result<(), EffectRuntimeErrorV1> {
    if usage.requests == 0
        || usage.requests > plan.max_requests
        || usage.pages == 0
        || usage.pages > plan.max_pages
        || usage.bytes == 0
        || usage.bytes > plan.max_bytes
        || usage.duration_ms == 0
        || usage.duration_ms > plan.max_duration_ms
    {
        return Err(EffectRuntimeErrorV1::ReconciliationReadBoundsExceeded);
    }
    require_nonzero(usage.result_commitment)
}

fn reconciliation_read_usage_value(usage: EffectReconciliationReadUsageV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(u64::from(usage.requests)),
        CborValue::Unsigned(u64::from(usage.pages)),
        CborValue::Unsigned(usage.bytes),
        CborValue::Unsigned(usage.duration_ms),
        bytes(&usage.result_commitment),
    ])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectReconciliationPreparationV1 {
    pub use_fence: EffectIntentUseFenceV1,
    pub read_plan: EffectReconciliationReadPlanV1,
    pub read_run: RunReservationV1,
}

impl EffectReconciliationPreparationV1 {
    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-preparation.v1")?,
            use_fence_value(self.use_fence)?,
            self.read_plan.canonical_value()?,
            CborValue::Array(vec![
                bytes(&self.read_run.semantic_operation_hash),
                bytes(&self.read_run.inputs_commitment),
                bytes(&self.read_run.environment_commitment),
                bytes(&self.read_run.target_commitment),
                bytes(&self.read_run.execution_boundary_commitment),
                CborValue::Unsigned(self.read_run.deadline),
                CborValue::Unsigned(u64::from(self.read_run.launch_ordinal)),
                CborValue::optional(
                    self.read_run
                        .current_step_term
                        .map(|term| bytes(term.as_bytes())),
                ),
            ]),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectReconciliationAttemptV1 {
    attempt: ReconciliationAttemptV1,
    starting_classification: RemoteClassificationV1,
    use_fence: EffectIntentUseFenceV1,
    use_fence_commitment: [u8; 32],
    read_plan: EffectReconciliationReadPlanV1,
    run_set: RunSetV1,
    read_execution_request_id: Option<ActionRequestIdV1>,
    read_usage: Option<EffectReconciliationReadUsageV1>,
    originating_step_binding: Option<StepBindingV1>,
}

impl EffectReconciliationAttemptV1 {
    pub const fn attempt(&self) -> ReconciliationAttemptV1 {
        self.attempt
    }

    pub const fn starting_classification(&self) -> RemoteClassificationV1 {
        self.starting_classification
    }

    pub const fn use_fence_commitment(&self) -> [u8; 32] {
        self.use_fence_commitment
    }

    pub const fn use_fence(&self) -> EffectIntentUseFenceV1 {
        self.use_fence
    }

    pub fn read_plan_commitment(&self) -> Result<[u8; 32], EffectRuntimeErrorV1> {
        self.read_plan.commitment()
    }

    pub const fn read_plan(&self) -> EffectReconciliationReadPlanV1 {
        self.read_plan
    }

    pub const fn run_set(&self) -> &RunSetV1 {
        &self.run_set
    }

    pub const fn read_execution_request_id(&self) -> Option<ActionRequestIdV1> {
        self.read_execution_request_id
    }

    pub const fn read_usage(&self) -> Option<EffectReconciliationReadUsageV1> {
        self.read_usage
    }

    pub const fn dispatch_operations(&self) -> u8 {
        0
    }

    pub const fn has_step_lease_authority(&self) -> bool {
        false
    }

    pub const fn may_mutate_originating_step(&self) -> bool {
        false
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-attempt.v1")?,
            bytes(self.attempt.id().as_bytes()),
            bytes(self.attempt.effect_intent_id().as_bytes()),
            bytes(self.attempt.action_request_id().as_bytes()),
            use_fence_value(self.use_fence)?,
            bytes(&self.use_fence_commitment),
            self.read_plan.canonical_value()?,
            self.run_set.canonical_value()?,
            CborValue::optional(
                self.read_execution_request_id
                    .map(|request| bytes(request.as_bytes())),
            ),
            CborValue::optional(self.read_usage.map(reconciliation_read_usage_value)),
            CborValue::Unsigned(remote_classification_tag(self.starting_classification)),
            CborValue::optional(self.originating_step_binding.map(step_binding_value)),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, EffectRuntimeErrorV1> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    fn from_canonical_value(value: &CborValue) -> Result<Self, EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            attempt_id,
            intent_id,
            reservation_request,
            use_fence_value,
            use_fence,
            read_plan,
            run_set,
            read_request,
            read_usage,
            CborValue::Unsigned(starting_classification),
            step_provenance,
        ] = fields.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if domain != "maestro.vnext.effect-reconciliation-attempt.v1" {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let read_plan = EffectReconciliationReadPlanV1::from_canonical_value(read_plan)?;
        let parsed_use_fence = parse_active_store_use_fence(use_fence_value)?;
        let use_fence_commitment = exact_effect_digest(use_fence)?;
        if hash(&self::use_fence_value(parsed_use_fence)?)? != use_fence_commitment {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let originating_step_binding = parse_optional_step_binding(step_provenance)?;
        let attempt = ReconciliationAttemptV1::from_persisted(
            ReconciliationAttemptIdV1::from_bytes(exact_effect_digest(attempt_id)?)?,
            EffectIntentIdV1::from_bytes(exact_effect_digest(intent_id)?)?,
            ActionRequestIdV1::from_digest(exact_effect_digest(reservation_request)?),
            use_fence_commitment,
            read_plan.commitment()?,
            originating_step_binding,
        )?;
        let read_execution_request_id =
            parse_optional_effect_digest(read_request)?.map(ActionRequestIdV1::from_digest);
        let read_usage = parse_optional_reconciliation_read_usage(read_usage)?;
        if read_execution_request_id.is_some() != read_usage.is_some() {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let terminal_read = read_usage.is_some();
        let transition_count = if terminal_read { 3 } else { 1 };
        let CborValue::Array(run_set_fields) = run_set else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let Some(CborValue::Unsigned(stored_revision)) = run_set_fields.get(3) else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let initial_revision = stored_revision
            .checked_sub(transition_count)
            .ok_or(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?;
        let execution_attempt = ExecutionAttemptV1::Reconciliation(attempt);
        let run_set = RunSetV1::from_non_step_canonical_value_at_revision(
            run_set,
            &execution_attempt,
            initial_revision,
        )?;
        let [run] = run_set.runs() else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let expected_state = if terminal_read {
            RunStateV1::Succeeded
        } else {
            RunStateV1::Reserved
        };
        if run.state() != expected_state {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        if let Some(usage) = read_usage {
            validate_reconciliation_read_usage(read_plan, usage)?;
        }
        let reconciliation = Self {
            attempt,
            starting_classification: parse_remote_classification(*starting_classification)?,
            use_fence: parsed_use_fence,
            use_fence_commitment,
            read_plan,
            run_set,
            read_execution_request_id,
            read_usage,
            originating_step_binding,
        };
        if reconciliation.canonical_value()? != *value {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(reconciliation)
    }

    pub(crate) fn from_persistence_carrier_value(
        value: &CborValue,
    ) -> Result<(Self, EffectControlTransitionNeedV1), EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            attempt,
            control_need_value,
            run_binding,
        ] = fields.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if !matches!(
            domain.as_str(),
            "maestro.vnext.effect-reconciliation-begin-carrier.v1"
                | "maestro.vnext.effect-reconciliation-read-carrier.v1"
                | "maestro.vnext.effect-reconciliation-terminal-carrier.v1"
        ) {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let attempt = Self::from_canonical_value(attempt)?;
        let control_need = EffectControlTransitionNeedV1::from_canonical_value(control_need_value)?;
        if run_set_binding_value(attempt.run_set()) != *run_binding {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let owner = ExecutionAttemptOwnerV1::Reconciliation(attempt.attempt.id());
        let phase_valid = match (domain.as_str(), &control_need) {
            (
                "maestro.vnext.effect-reconciliation-begin-carrier.v1",
                EffectControlTransitionNeedV1::BeginReconciliation {
                    action_request_id,
                    attempt: control_owner,
                    next_run_set_revision,
                    next_use_fence_commitment,
                },
            ) => {
                *action_request_id == attempt.attempt.action_request_id()
                    && *control_owner == owner
                    && *next_run_set_revision == attempt.run_set.revision()
                    && *next_use_fence_commitment == attempt.use_fence_commitment
                    && attempt.read_execution_request_id.is_none()
            }
            (
                "maestro.vnext.effect-reconciliation-read-carrier.v1",
                EffectControlTransitionNeedV1::RecordReconciliationRead {
                    action_request_id,
                    attempt: control_owner,
                    next_run_set_revision,
                    result_commitment,
                    idempotency_commitment,
                },
            ) => {
                Some(*action_request_id) == attempt.read_execution_request_id
                    && *control_owner == owner
                    && *next_run_set_revision == attempt.run_set.revision()
                    && *result_commitment != [0; 32]
                    && *idempotency_commitment != [0; 32]
                    && attempt.read_usage.is_some()
            }
            (
                "maestro.vnext.effect-reconciliation-terminal-carrier.v1",
                EffectControlTransitionNeedV1::FinishReconciliation {
                    action_request_id,
                    attempt: control_owner,
                    classification,
                    next_run_set_revision,
                    read_publication_commitment,
                    result_commitment,
                    idempotency_commitment,
                },
            ) => {
                *action_request_id == attempt.attempt.action_request_id()
                    && Some(*action_request_id) == attempt.read_execution_request_id
                    && *control_owner == owner
                    && *classification != RemoteClassificationV1::Prepared
                    && *classification != RemoteClassificationV1::Dispatching
                    && valid_reconciliation_refinement(
                        attempt.starting_classification,
                        *classification,
                    )
                    && *next_run_set_revision == attempt.run_set.revision()
                    && *read_publication_commitment != [0; 32]
                    && *result_commitment != [0; 32]
                    && *idempotency_commitment != [0; 32]
                    && attempt.read_usage.is_some()
            }
            _ => false,
        };
        if !phase_valid || control_need.canonical_value()? != *control_need_value {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok((attempt, control_need))
    }

    pub(crate) fn validate_persisted_predecessor(
        &self,
        predecessor: &Self,
    ) -> Result<(), EffectRuntimeErrorV1> {
        if self.attempt != predecessor.attempt
            || self.starting_classification != predecessor.starting_classification
            || self.use_fence != predecessor.use_fence
            || self.use_fence_commitment != predecessor.use_fence_commitment
            || self.read_plan != predecessor.read_plan
            || self.originating_step_binding != predecessor.originating_step_binding
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        match (
            predecessor.read_execution_request_id,
            self.read_execution_request_id,
            predecessor.read_usage,
            self.read_usage,
        ) {
            (None, Some(_), None, Some(_)) => {
                let [prior_run] = predecessor.run_set.runs() else {
                    return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
                };
                let [next_run] = self.run_set.runs() else {
                    return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
                };
                if prior_run.id() != next_run.id()
                    || prior_run.state() != RunStateV1::Reserved
                    || next_run.state() != RunStateV1::Succeeded
                    || self.run_set.revision()
                        != predecessor
                            .run_set
                            .revision()
                            .checked_add(2)
                            .ok_or(EffectRuntimeErrorV1::CounterOverflow)?
                {
                    return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
                }
            }
            (Some(prior_request), Some(next_request), Some(prior_usage), Some(next_usage))
                if prior_request == next_request
                    && prior_usage == next_usage
                    && predecessor.run_set == self.run_set => {}
            _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
        }
        Ok(())
    }

    pub fn execute_read_candidate(
        mut self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        usage: EffectReconciliationReadUsageV1,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectReconciliationReadCandidateV1, EffectRuntimeErrorV1> {
        validate_control_basis(intent, revision)?;
        let route = require_action_route(intent.origin.kind())?;
        require_action(authority, route.reconcile())?;
        if authority.request_id() != self.attempt.action_request_id()
            || revision.live_attempt()
                != Some(ExecutionAttemptOwnerV1::Reconciliation(self.attempt.id()))
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || revision.runs_closed()
            || revision.run_set_revision() != self.run_set.revision()
            || self.read_execution_request_id.is_some()
            || self.read_usage.is_some()
        {
            return Err(EffectRuntimeErrorV1::IllegalEffectState);
        }
        validate_reconciliation_read_usage(self.read_plan, usage)?;
        require_nonzero(result_commitment)?;
        require_nonzero(idempotency_commitment)?;
        let run_id = self
            .run_set
            .runs()
            .first()
            .ok_or(EffectRuntimeErrorV1::MissingAttemptRun)?
            .id();
        self.run_set.transition_non_step_run(
            run_id,
            revision.run_set_revision(),
            RunStateV1::Active,
        )?;
        self.run_set.transition_non_step_run(
            run_id,
            self.run_set.revision(),
            RunStateV1::Succeeded,
        )?;
        self.read_execution_request_id = Some(authority.request_id());
        self.read_usage = Some(usage);
        let next_run_set_revision = self.run_set.revision();
        let attempt_owner = ExecutionAttemptOwnerV1::Reconciliation(self.attempt.id());
        Ok(EffectReconciliationReadCandidateV1 {
            attempt: self,
            control_need: EffectControlTransitionNeedV1::RecordReconciliationRead {
                action_request_id: authority.request_id(),
                attempt: attempt_owner,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            },
        })
    }

    pub const fn request_additional_poll(&self) -> Result<(), EffectRuntimeErrorV1> {
        Err(EffectRuntimeErrorV1::SecondReconciliationPollRequiresNewAttempt)
    }

    pub fn finish_candidate(
        self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        outcome: EffectReconciliationOutcomeV1,
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<EffectReconciliationTerminalCandidateV1, EffectRuntimeErrorV1> {
        validate_control_basis(intent, revision)?;
        let route = require_action_route(intent.origin.kind())?;
        require_action(authority, route.reconcile())?;
        let execution_request_id = self
            .read_execution_request_id
            .ok_or(EffectRuntimeErrorV1::ReconciliationReadNotCompleted)?;
        let usage = self
            .read_usage
            .ok_or(EffectRuntimeErrorV1::ReconciliationReadNotCompleted)?;
        if authority.request_id() != self.attempt.action_request_id()
            || execution_request_id != self.attempt.action_request_id()
            || revision.intent() != self.attempt.effect_intent_id()
            || revision.live_attempt()
                != Some(ExecutionAttemptOwnerV1::Reconciliation(self.attempt.id()))
            || revision.live_dispatch() != EffectIntentLiveDispatchV1::None
            || revision.runs_closed()
            || revision.run_set_revision() != self.run_set.revision()
            || !self.run_set.all_terminal()
            || usage.result_commitment != outcome.read_result_commitment
            || !valid_reconciliation_refinement(
                self.starting_classification,
                outcome.classification,
            )
        {
            return Err(EffectRuntimeErrorV1::IllegalReconciliationRefinement);
        }
        require_nonzero(outcome.read_result_commitment)?;
        require_nonzero(outcome.result_commitment)?;
        require_nonzero(outcome.idempotency_commitment)?;
        let read_publication_commitment = revision
            .parts()
            .result_commitment
            .ok_or(EffectRuntimeErrorV1::ReconciliationReadNotCompleted)?;
        let next_run_set_revision = self.run_set.revision();
        let attempt_owner = ExecutionAttemptOwnerV1::Reconciliation(self.attempt.id());
        Ok(EffectReconciliationTerminalCandidateV1 {
            attempt: self,
            classification: outcome.classification,
            control_need: EffectControlTransitionNeedV1::FinishReconciliation {
                action_request_id: authority.request_id(),
                attempt: attempt_owner,
                classification: outcome.classification,
                next_run_set_revision,
                read_publication_commitment,
                result_commitment: outcome.result_commitment,
                idempotency_commitment: outcome.idempotency_commitment,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectReconciliationOutcomeV1 {
    pub classification: RemoteClassificationV1,
    pub read_result_commitment: [u8; 32],
    pub result_commitment: [u8; 32],
    pub idempotency_commitment: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedEffectReconciliationV1 {
    reconciliation: EffectReconciliationAttemptV1,
    control_need: EffectControlTransitionNeedV1,
}

impl PreparedEffectReconciliationV1 {
    pub const fn reconciliation(&self) -> &EffectReconciliationAttemptV1 {
        &self.reconciliation
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub fn into_parts(self) -> (EffectReconciliationAttemptV1, EffectControlTransitionNeedV1) {
        (self.reconciliation, self.control_need)
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        reconciliation_transition_carrier_value(
            "maestro.vnext.effect-reconciliation-begin-carrier.v1",
            &self.reconciliation,
            &self.control_need,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectReconciliationReadCandidateV1 {
    attempt: EffectReconciliationAttemptV1,
    control_need: EffectControlTransitionNeedV1,
}

impl EffectReconciliationReadCandidateV1 {
    pub const fn attempt(&self) -> &EffectReconciliationAttemptV1 {
        &self.attempt
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        reconciliation_transition_carrier_value(
            "maestro.vnext.effect-reconciliation-read-carrier.v1",
            &self.attempt,
            &self.control_need,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectReconciliationTerminalCandidateV1 {
    attempt: EffectReconciliationAttemptV1,
    classification: RemoteClassificationV1,
    control_need: EffectControlTransitionNeedV1,
}

impl EffectReconciliationTerminalCandidateV1 {
    pub const fn attempt(&self) -> &EffectReconciliationAttemptV1 {
        &self.attempt
    }

    pub const fn classification(&self) -> RemoteClassificationV1 {
        self.classification
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        reconciliation_transition_carrier_value(
            "maestro.vnext.effect-reconciliation-terminal-carrier.v1",
            &self.attempt,
            &self.control_need,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectWithdrawalCurrentCarrierV1 {
    home: EffectIntentHomeV1,
    authority_request_id: ActionRequestIdV1,
    capacity_path: WithdrawalAuthorityPathV1,
    expected_old_control_revision: [u8; 32],
    current_carrier_commitment: [u8; 32],
    expected_old_carrier_commitment: [u8; 32],
}

impl EffectWithdrawalCurrentCarrierV1 {
    pub fn new(
        home: EffectIntentHomeV1,
        authority_request_id: ActionRequestIdV1,
        capacity_path: WithdrawalAuthorityPathV1,
        expected_old_control_revision: [u8; 32],
        current_carrier_commitment: [u8; 32],
        expected_old_carrier_commitment: [u8; 32],
    ) -> Result<Self, EffectRuntimeErrorV1> {
        require_nonzero(expected_old_control_revision)?;
        require_nonzero(current_carrier_commitment)?;
        require_nonzero(expected_old_carrier_commitment)?;
        Ok(Self {
            home,
            authority_request_id,
            capacity_path,
            expected_old_control_revision,
            current_carrier_commitment,
            expected_old_carrier_commitment,
        })
    }

    fn validate(
        self,
        intent: &EffectIntentV1,
        revision: &EffectIntentControlRevisionV1,
        authority: &AuthorizedExecutionActionV1,
        expected_path: WithdrawalAuthorityPathV1,
    ) -> Result<(), EffectRuntimeErrorV1> {
        if self.home != intent.home
            || hash(&home_value(self.home)?)? != intent.home_commitment
            || self.authority_request_id != authority.request_id()
            || self.capacity_path != expected_path
            || self.expected_old_control_revision != *revision.id().as_bytes()
            || self.current_carrier_commitment != self.expected_old_carrier_commitment
        {
            return Err(EffectRuntimeErrorV1::WithdrawalCurrentCarrierMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectWithdrawalV1 {
    intent_id: EffectIntentIdV1,
    request: WithdrawalRequestV1,
    control_need: EffectControlTransitionNeedV1,
}

impl EffectWithdrawalV1 {
    pub const fn intent_id(&self) -> EffectIntentIdV1 {
        self.intent_id
    }

    pub const fn control_need(&self) -> &EffectControlTransitionNeedV1 {
        &self.control_need
    }

    pub const fn derived_request(&self) -> WithdrawalRequestV1 {
        self.request
    }

    pub const fn provider_io_operations(&self) -> u8 {
        0
    }

    pub const fn creates_attempt(&self) -> bool {
        false
    }

    pub const fn creates_run(&self) -> bool {
        false
    }

    pub fn persistence_carrier_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-withdrawal-carrier.v1")?,
            bytes(self.intent_id.as_bytes()),
            withdrawal_request_value(self.request)?,
            self.control_need.canonical_value()?,
            CborValue::Unsigned(0),
            CborValue::Unsigned(0),
            CborValue::Unsigned(0),
        ]))
    }

    pub(crate) fn from_persistence_carrier_value(
        value: &CborValue,
    ) -> Result<Self, EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [
            CborValue::Text(domain),
            intent,
            request,
            control_need,
            CborValue::Unsigned(provider_io),
            CborValue::Unsigned(attempts),
            CborValue::Unsigned(runs),
        ] = fields.as_slice()
        else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if domain != "maestro.vnext.effect-withdrawal-carrier.v1"
            || *provider_io != 0
            || *attempts != 0
            || *runs != 0
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let withdrawal = Self {
            intent_id: EffectIntentIdV1::from_bytes(exact_effect_digest(intent)?)?,
            request: parse_withdrawal_request(request)?,
            control_need: EffectControlTransitionNeedV1::from_canonical_value(control_need)?,
        };
        if !matches!(
            withdrawal.control_need,
            EffectControlTransitionNeedV1::Withdraw { .. }
        ) || withdrawal.persistence_carrier_value()? != *value
        {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(withdrawal)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the closed control-transition carrier owns each exact typed phase payload without a second indirection or separately current object"
)]
pub enum EffectControlTransitionNeedV1 {
    ReserveDispatch {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        next_dispatch_fence: u64,
        next_run_set_revision: u64,
        next_use_fence_commitment: [u8; 32],
    },
    SealDispatch {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
    },
    FinishDispatch {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        classification: RemoteClassificationV1,
        next_run_set_revision: u64,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    },
    BeginReconciliation {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
        next_use_fence_commitment: [u8; 32],
    },
    RecordReconciliationRead {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    },
    FinishReconciliation {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        classification: RemoteClassificationV1,
        next_run_set_revision: u64,
        read_publication_commitment: [u8; 32],
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    },
    Withdraw {
        action_request_id: ActionRequestIdV1,
        next_run_set_revision: u64,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    },
    MarkRecoveryRequired {
        action_request_id: ActionRequestIdV1,
    },
    RecoverReserved {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        dispatch_fence: u64,
    },
    RedispatchConclusiveNotApplied {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        next_dispatch_fence: u64,
        next_run_set_revision: u64,
        next_use_fence_commitment: [u8; 32],
    },
    RecoverSealedInDoubt {
        action_request_id: ActionRequestIdV1,
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    },
    MarkIntegrityBlocked {
        action_request_id: ActionRequestIdV1,
    },
    HandoffWriter {
        action_request_id: ActionRequestIdV1,
        fencing_receipt: SameHomeWriterFencingReceiptV1,
        successor_writer: EffectIntentControlWriterTermV1,
    },
}

impl EffectControlTransitionNeedV1 {
    pub const fn action_request_id(&self) -> ActionRequestIdV1 {
        match self {
            Self::ReserveDispatch {
                action_request_id, ..
            }
            | Self::SealDispatch {
                action_request_id, ..
            }
            | Self::FinishDispatch {
                action_request_id, ..
            }
            | Self::BeginReconciliation {
                action_request_id, ..
            }
            | Self::RecordReconciliationRead {
                action_request_id, ..
            }
            | Self::FinishReconciliation {
                action_request_id, ..
            }
            | Self::Withdraw {
                action_request_id, ..
            }
            | Self::MarkRecoveryRequired { action_request_id }
            | Self::RecoverReserved {
                action_request_id, ..
            }
            | Self::RedispatchConclusiveNotApplied {
                action_request_id, ..
            }
            | Self::RecoverSealedInDoubt {
                action_request_id, ..
            }
            | Self::MarkIntegrityBlocked { action_request_id }
            | Self::HandoffWriter {
                action_request_id, ..
            } => *action_request_id,
        }
    }

    fn mutation(&self) -> EffectIntentControlMutationV1 {
        match self {
            Self::ReserveDispatch {
                attempt,
                next_dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment,
                ..
            } => EffectIntentControlMutationV1::ReserveDispatch {
                attempt: *attempt,
                next_dispatch_fence: *next_dispatch_fence,
                next_run_set_revision: *next_run_set_revision,
                next_use_fence_commitment: *next_use_fence_commitment,
            },
            Self::SealDispatch {
                attempt,
                next_run_set_revision,
                ..
            } => EffectIntentControlMutationV1::SealDispatch {
                attempt: *attempt,
                next_run_set_revision: *next_run_set_revision,
            },
            Self::FinishDispatch {
                attempt,
                classification,
                next_run_set_revision,
                ..
            } => EffectIntentControlMutationV1::FinishDispatch {
                attempt: *attempt,
                classification: *classification,
                next_run_set_revision: *next_run_set_revision,
            },
            Self::BeginReconciliation {
                attempt,
                next_run_set_revision,
                next_use_fence_commitment,
                ..
            } => EffectIntentControlMutationV1::BeginReconciliation {
                attempt: *attempt,
                next_run_set_revision: *next_run_set_revision,
                next_use_fence_commitment: *next_use_fence_commitment,
            },
            Self::RecordReconciliationRead {
                attempt,
                next_run_set_revision,
                ..
            } => EffectIntentControlMutationV1::RecordReconciliationRead {
                attempt: *attempt,
                next_run_set_revision: *next_run_set_revision,
            },
            Self::FinishReconciliation {
                attempt,
                classification,
                next_run_set_revision,
                read_publication_commitment,
                ..
            } => EffectIntentControlMutationV1::FinishReconciliation {
                attempt: *attempt,
                classification: *classification,
                next_run_set_revision: *next_run_set_revision,
                read_result_commitment: *read_publication_commitment,
            },
            Self::Withdraw {
                next_run_set_revision,
                ..
            } => EffectIntentControlMutationV1::Withdraw {
                next_run_set_revision: *next_run_set_revision,
            },
            Self::MarkRecoveryRequired { .. } => {
                EffectIntentControlMutationV1::MarkRecoveryRequired
            }
            Self::RecoverReserved {
                attempt,
                dispatch_fence,
                ..
            } => EffectIntentControlMutationV1::RecoverReserved {
                attempt: *attempt,
                dispatch_fence: *dispatch_fence,
            },
            Self::RedispatchConclusiveNotApplied {
                attempt,
                next_dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment,
                ..
            } => EffectIntentControlMutationV1::RedispatchConclusiveNotApplied {
                attempt: *attempt,
                next_dispatch_fence: *next_dispatch_fence,
                next_run_set_revision: *next_run_set_revision,
                next_use_fence_commitment: *next_use_fence_commitment,
            },
            Self::RecoverSealedInDoubt {
                attempt,
                next_run_set_revision,
                ..
            } => EffectIntentControlMutationV1::RecoverSealedInDoubt {
                attempt: *attempt,
                next_run_set_revision: *next_run_set_revision,
            },
            Self::MarkIntegrityBlocked { .. } => {
                EffectIntentControlMutationV1::MarkIntegrityBlocked
            }
            Self::HandoffWriter {
                fencing_receipt,
                successor_writer,
                ..
            } => EffectIntentControlMutationV1::HandoffWriter(Box::new((
                *fencing_receipt,
                *successor_writer,
            ))),
        }
    }

    pub fn control_transition(
        &self,
        current_head: &EffectIntentControlHeadV1,
        current_revision: &EffectIntentControlRevisionV1,
        writer_term: EffectIntentControlWriterTermV1,
    ) -> Result<EffectIntentControlTransitionV1, EffectRuntimeErrorV1> {
        Ok(EffectIntentControlTransitionV1::new(
            current_head,
            current_revision,
            writer_term,
            self.mutation(),
            self.action_request_id(),
        )?)
    }

    pub(crate) fn persisted_candidate_revision(
        &self,
        current_revision: &EffectIntentControlRevisionV1,
    ) -> Result<EffectIntentControlRevisionV1, EffectRuntimeErrorV1> {
        let publication = match self {
            Self::FinishDispatch {
                result_commitment,
                idempotency_commitment,
                ..
            }
            | Self::RecordReconciliationRead {
                result_commitment,
                idempotency_commitment,
                ..
            }
            | Self::FinishReconciliation {
                result_commitment,
                idempotency_commitment,
                ..
            }
            | Self::Withdraw {
                result_commitment,
                idempotency_commitment,
                ..
            }
            | Self::RecoverSealedInDoubt {
                result_commitment,
                idempotency_commitment,
                ..
            } => Some(
                EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                    *result_commitment,
                    *idempotency_commitment,
                )?,
            ),
            Self::ReserveDispatch { .. }
            | Self::SealDispatch { .. }
            | Self::BeginReconciliation { .. }
            | Self::MarkRecoveryRequired { .. }
            | Self::RecoverReserved { .. }
            | Self::RedispatchConclusiveNotApplied { .. }
            | Self::MarkIntegrityBlocked { .. }
            | Self::HandoffWriter { .. } => None,
        };
        Ok(derive_candidate_revision(
            current_revision,
            &self.mutation(),
            publication,
        )?)
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectRuntimeErrorV1> {
        let value = match self {
            Self::ReserveDispatch {
                action_request_id,
                attempt,
                next_dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*next_dispatch_fence),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(next_use_fence_commitment),
            ]),
            Self::SealDispatch {
                action_request_id,
                attempt,
                next_run_set_revision,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*next_run_set_revision),
            ]),
            Self::FinishDispatch {
                action_request_id,
                attempt,
                classification,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                remote_classification_value(*classification),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(result_commitment),
                bytes(idempotency_commitment),
            ]),
            Self::BeginReconciliation {
                action_request_id,
                attempt,
                next_run_set_revision,
                next_use_fence_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(4),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(next_use_fence_commitment),
            ]),
            Self::RecordReconciliationRead {
                action_request_id,
                attempt,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(8),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(result_commitment),
                bytes(idempotency_commitment),
            ]),
            Self::FinishReconciliation {
                action_request_id,
                attempt,
                classification,
                next_run_set_revision,
                read_publication_commitment,
                result_commitment,
                idempotency_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(5),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                remote_classification_value(*classification),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(read_publication_commitment),
                bytes(result_commitment),
                bytes(idempotency_commitment),
            ]),
            Self::Withdraw {
                action_request_id,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(6),
                bytes(action_request_id.as_bytes()),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(result_commitment),
                bytes(idempotency_commitment),
            ]),
            Self::MarkRecoveryRequired { action_request_id } => CborValue::Array(vec![
                CborValue::Unsigned(7),
                bytes(action_request_id.as_bytes()),
            ]),
            Self::RecoverReserved {
                action_request_id,
                attempt,
                dispatch_fence,
            } => CborValue::Array(vec![
                CborValue::Unsigned(9),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*dispatch_fence),
            ]),
            Self::RedispatchConclusiveNotApplied {
                action_request_id,
                attempt,
                next_dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(10),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*next_dispatch_fence),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(next_use_fence_commitment),
            ]),
            Self::RecoverSealedInDoubt {
                action_request_id,
                attempt,
                next_run_set_revision,
                result_commitment,
                idempotency_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(11),
                bytes(action_request_id.as_bytes()),
                execution_owner_value(*attempt),
                CborValue::Unsigned(*next_run_set_revision),
                bytes(result_commitment),
                bytes(idempotency_commitment),
            ]),
            Self::MarkIntegrityBlocked { action_request_id } => CborValue::Array(vec![
                CborValue::Unsigned(12),
                bytes(action_request_id.as_bytes()),
            ]),
            Self::HandoffWriter {
                action_request_id,
                fencing_receipt,
                successor_writer,
            } => CborValue::Array(vec![
                CborValue::Unsigned(13),
                bytes(action_request_id.as_bytes()),
                fencing_receipt.canonical_value()?,
                successor_writer.canonical_value(),
            ]),
        };
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-control-transition-need.v1")?,
            value,
        ]))
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, EffectRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let [CborValue::Text(domain), body] = fields.as_slice() else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        if domain != "maestro.vnext.effect-control-transition-need.v1" {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        let CborValue::Array(fields) = body else {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        };
        let parsed = match fields.as_slice() {
            [
                CborValue::Unsigned(1),
                request,
                owner,
                CborValue::Unsigned(dispatch_fence),
                CborValue::Unsigned(run_revision),
                use_fence,
            ] => Self::ReserveDispatch {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                next_dispatch_fence: *dispatch_fence,
                next_run_set_revision: *run_revision,
                next_use_fence_commitment: exact_effect_digest(use_fence)?,
            },
            [
                CborValue::Unsigned(2),
                request,
                owner,
                CborValue::Unsigned(run_revision),
            ] => Self::SealDispatch {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                next_run_set_revision: *run_revision,
            },
            [
                CborValue::Unsigned(3),
                request,
                owner,
                CborValue::Unsigned(classification),
                CborValue::Unsigned(run_revision),
                result,
                idempotency,
            ] => Self::FinishDispatch {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                classification: parse_remote_classification(*classification)?,
                next_run_set_revision: *run_revision,
                result_commitment: exact_effect_digest(result)?,
                idempotency_commitment: exact_effect_digest(idempotency)?,
            },
            [
                CborValue::Unsigned(4),
                request,
                owner,
                CborValue::Unsigned(run_revision),
                use_fence,
            ] => Self::BeginReconciliation {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                next_run_set_revision: *run_revision,
                next_use_fence_commitment: exact_effect_digest(use_fence)?,
            },
            [
                CborValue::Unsigned(5),
                request,
                owner,
                CborValue::Unsigned(classification),
                CborValue::Unsigned(run_revision),
                read_publication,
                result,
                idempotency,
            ] => Self::FinishReconciliation {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                classification: parse_remote_classification(*classification)?,
                next_run_set_revision: *run_revision,
                read_publication_commitment: exact_effect_digest(read_publication)?,
                result_commitment: exact_effect_digest(result)?,
                idempotency_commitment: exact_effect_digest(idempotency)?,
            },
            [
                CborValue::Unsigned(6),
                request,
                CborValue::Unsigned(run_revision),
                result,
                idempotency,
            ] => Self::Withdraw {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                next_run_set_revision: *run_revision,
                result_commitment: exact_effect_digest(result)?,
                idempotency_commitment: exact_effect_digest(idempotency)?,
            },
            [CborValue::Unsigned(7), request] => Self::MarkRecoveryRequired {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
            },
            [
                CborValue::Unsigned(8),
                request,
                owner,
                CborValue::Unsigned(run_revision),
                result,
                idempotency,
            ] => Self::RecordReconciliationRead {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                next_run_set_revision: *run_revision,
                result_commitment: exact_effect_digest(result)?,
                idempotency_commitment: exact_effect_digest(idempotency)?,
            },
            [
                CborValue::Unsigned(9),
                request,
                owner,
                CborValue::Unsigned(dispatch_fence),
            ] => Self::RecoverReserved {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                dispatch_fence: *dispatch_fence,
            },
            [
                CborValue::Unsigned(10),
                request,
                owner,
                CborValue::Unsigned(dispatch_fence),
                CborValue::Unsigned(run_revision),
                use_fence,
            ] => Self::RedispatchConclusiveNotApplied {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                next_dispatch_fence: *dispatch_fence,
                next_run_set_revision: *run_revision,
                next_use_fence_commitment: exact_effect_digest(use_fence)?,
            },
            [
                CborValue::Unsigned(11),
                request,
                owner,
                CborValue::Unsigned(run_revision),
                result,
                idempotency,
            ] => Self::RecoverSealedInDoubt {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                attempt: super::runtime::parse_attempt_owner(owner)?,
                next_run_set_revision: *run_revision,
                result_commitment: exact_effect_digest(result)?,
                idempotency_commitment: exact_effect_digest(idempotency)?,
            },
            [CborValue::Unsigned(12), request] => Self::MarkIntegrityBlocked {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
            },
            [CborValue::Unsigned(13), request, receipt, writer] => Self::HandoffWriter {
                action_request_id: ActionRequestIdV1::from_digest(exact_effect_digest(request)?),
                fencing_receipt: SameHomeWriterFencingReceiptV1::from_canonical_value(receipt)?,
                successor_writer: EffectIntentControlWriterTermV1::from_canonical_value(writer)?,
            },
            _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
        };
        if parsed.canonical_value()? != *value {
            return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
        }
        Ok(parsed)
    }
}

fn validate_control_basis(
    intent: &EffectIntentV1,
    revision: &EffectIntentControlRevisionV1,
) -> Result<(), EffectRuntimeErrorV1> {
    if !intent.matches_control_revision(revision) {
        return Err(EffectRuntimeErrorV1::ControlBasisMismatch);
    }
    Ok(())
}

fn validate_origination_route(
    kind: EffectOriginKindV1,
    home: EffectIntentHomeKindV1,
    authority: EffectOriginationAuthorityV1,
) -> Result<(), EffectRuntimeErrorV1> {
    match authority.route() {
        EffectOriginationAuthorityRouteV1::Action {
            action,
            reservation_mode,
        } => {
            let route = kind
                .action_route()
                .ok_or(EffectRuntimeErrorV1::OriginRouteMismatch)?;
            if home != EffectIntentHomeKindV1::ActiveStore
                || reservation_mode != DispatchReservationModeV1::InitiateNew
                || action != route.reserve()
                || authority.action_request_id().is_none()
            {
                return Err(EffectRuntimeErrorV1::OriginRouteMismatch);
            }
        }
        EffectOriginationAuthorityRouteV1::Ceremony {
            ceremony_symbol_tag,
            request_mode,
        } => {
            if request_mode != CeremonyRequestModeV1::Initiate
                || authority.action_request_id().is_some()
                || !origin_allows_ceremony_symbol(kind, ceremony_symbol_tag)
            {
                return Err(EffectRuntimeErrorV1::OriginRouteMismatch);
            }
            let expected_home = if ceremony_symbol_tag == 1 {
                EffectIntentHomeKindV1::NoStoreCeremony
            } else {
                EffectIntentHomeKindV1::PreStoreCeremony
            };
            if home != expected_home {
                return Err(EffectRuntimeErrorV1::OriginRouteMismatch);
            }
        }
    }
    Ok(())
}

const fn origin_allows_ceremony_symbol(kind: EffectOriginKindV1, symbol_tag: u8) -> bool {
    matches!(
        (kind, symbol_tag),
        (
            EffectOriginKindV1::RepositoryRecoveryAdmissionEffectOrigin,
            10
        ) | (
            EffectOriginKindV1::InstallationRecoveryAdmissionEffectOrigin,
            11
        ) | (
            EffectOriginKindV1::DistributionManagerOperationEffectOrigin,
            9
        ) | (
            EffectOriginKindV1::DistributionBinaryActivationEffectOrigin,
            8
        ) | (
            EffectOriginKindV1::RepositoryGenerationActivationEffectOrigin,
            2 | 4 | 6
        ) | (
            EffectOriginKindV1::InstallationLocatorActivationEffectOrigin,
            1 | 3 | 5 | 7
        )
    )
}

fn ceremony_symbol_descriptor_id(tag: u8) -> Result<&'static str, EffectRuntimeErrorV1> {
    match tag {
        1 => Ok("b84549b141c2843cdcaeb7dd7c776af558d50a563967ab7f431de736f621ba4c"),
        2 => Ok("4e41ff2c9391d1611a823f7a21b7141ae5c9624344910e2f0c0169738605728d"),
        3 => Ok("07fd1f45d5791bb83435b1f3e4242d7ecb1fcc0b519d2592a5cd5305f4b8144a"),
        4 => Ok("00fce3bbb795d5ed59c5c5287922282373d54043cdddba5f830d928f57fa3a67"),
        5 => Ok("b82857862fcbc4933701c895f9a8ed1e3590a055bf4fecfc0bccb9623f593207"),
        6 => Ok("a167cc16f6b75a77c8209a3cf297fbacadb4cf412a04d67204d09d2a34df11cd"),
        7 => Ok("32bfc50f6b5cf86d40fdaa49fce1aae695c89cc809b39f9e3a3180b7043d7200"),
        8 => Ok("4a8acfe64af0e8e7b1b76e3119214444a0110e8ee66571fff8e28d04b9f3fa81"),
        9 => Ok("2128f75798074f5c2954f26b76ae38ae2bd0729edd9644567f9bb6723cda05eb"),
        10 => Ok("293a84066daee63560cf1f66e6ab8b22a49e63d4c404d300dd807c861eecd6fc"),
        11 => Ok("00802a252941f4d28bd87eca9c070a35f414d81105cce47cc527041454ab6316"),
        _ => Err(EffectRuntimeErrorV1::UnknownCeremonySymbol(tag)),
    }
}

fn origination_route_value(
    kind: EffectOriginKindV1,
    route: EffectOriginationAuthorityRouteV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(match route {
        EffectOriginationAuthorityRouteV1::Action {
            action,
            reservation_mode,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::text(kind.descriptor().descriptor_id())?,
            CborValue::Unsigned(action.global_tag()),
            CborValue::text(action.descriptor_id())?,
            CborValue::Unsigned(match reservation_mode {
                DispatchReservationModeV1::InitiateNew => 1,
                DispatchReservationModeV1::RecoverReserved => 2,
            }),
        ]),
        EffectOriginationAuthorityRouteV1::Ceremony {
            ceremony_symbol_tag,
            request_mode,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::text(kind.descriptor().descriptor_id())?,
            CborValue::Unsigned(u64::from(ceremony_symbol_tag)),
            CborValue::text(ceremony_symbol_descriptor_id(ceremony_symbol_tag)?)?,
            CborValue::Unsigned(match request_mode {
                CeremonyRequestModeV1::Initiate => 1,
                CeremonyRequestModeV1::RecoverReserved => 2,
                CeremonyRequestModeV1::ResolveResult => 3,
                CeremonyRequestModeV1::Withdraw => 4,
            }),
        ]),
    })
}

fn require_action_route(
    kind: EffectOriginKindV1,
) -> Result<EffectActionRouteV1, EffectRuntimeErrorV1> {
    kind.action_route()
        .ok_or(EffectRuntimeErrorV1::CeremonyOnlyEffectOrigin)
}

fn withdrawal_path(
    kind: EffectOriginKindV1,
    route: EffectActionRouteV1,
) -> Result<WithdrawalAuthorityPathV1, EffectRuntimeErrorV1> {
    Ok(match route {
        EffectActionRouteV1::Ordinary | EffectActionRouteV1::CoordinationDelivery => {
            WithdrawalAuthorityPathV1::Ordinary
        }
        EffectActionRouteV1::Bootstrap => WithdrawalAuthorityPathV1::BootstrapG0,
        EffectActionRouteV1::ContinuityMaintenance => {
            let slot = match kind {
                EffectOriginKindV1::TrustedTimeAcquisitionEffectOrigin => {
                    EffectWithdrawalSlotFamilyV1::MaintenanceExecutorCurrentness
                }
                EffectOriginKindV1::RecoveryExternalRegistrationEffectOrigin => {
                    EffectWithdrawalSlotFamilyV1::ProspectiveContinuityCarrier
                }
                EffectOriginKindV1::RecoveryExternalStatusEffectOrigin => {
                    EffectWithdrawalSlotFamilyV1::PlannedTurnoverHighWater
                }
                EffectOriginKindV1::MaintenanceExecutorCurrentnessEffectOrigin => {
                    EffectWithdrawalSlotFamilyV1::RepositoryRecoveryAdmission
                }
                EffectOriginKindV1::ProspectiveContinuityCarrierEffectOrigin => {
                    EffectWithdrawalSlotFamilyV1::InstallationRecoveryAdmission
                }
                _ => return Err(EffectRuntimeErrorV1::OriginRouteMismatch),
            };
            WithdrawalAuthorityPathV1::ContinuityMaintenance(slot)
        }
    })
}

fn require_action(
    authority: &AuthorizedExecutionActionV1,
    expected: ExecutionActionV1,
) -> Result<(), EffectRuntimeErrorV1> {
    if authority.action() != expected {
        return Err(EffectRuntimeErrorV1::WrongEffectAction);
    }
    Ok(())
}

fn require_fresh_request(
    intent: &EffectIntentV1,
    request_id: ActionRequestIdV1,
    prior: &[ActionRequestIdV1],
) -> Result<(), EffectRuntimeErrorV1> {
    if intent.origination_authority.action_request_id() == Some(request_id)
        || prior.contains(&request_id)
    {
        return Err(EffectRuntimeErrorV1::StaleActionRequest);
    }
    Ok(())
}

fn require_dispatch_request(
    intent: &EffectIntentV1,
    revision: &EffectIntentControlRevisionV1,
    request_id: ActionRequestIdV1,
) -> Result<(), EffectRuntimeErrorV1> {
    if revision.attempt_history().is_empty() {
        if intent.origination_authority.action_request_id().is_some()
            && intent.origination_authority.action_request_id() != Some(request_id)
        {
            return Err(EffectRuntimeErrorV1::DispatchOriginationRequestMismatch);
        }
    } else {
        require_fresh_request(intent, request_id, &[])?;
    }
    Ok(())
}

fn valid_reconciliation_refinement(
    previous: RemoteClassificationV1,
    next: RemoteClassificationV1,
) -> bool {
    match previous {
        RemoteClassificationV1::Pending => matches!(
            next,
            RemoteClassificationV1::Pending
                | RemoteClassificationV1::InDoubt
                | RemoteClassificationV1::ConfirmedApplied
                | RemoteClassificationV1::ConfirmedNotApplied
                | RemoteClassificationV1::PartiallyApplied
                | RemoteClassificationV1::Conflicted
        ),
        RemoteClassificationV1::InDoubt => matches!(
            next,
            RemoteClassificationV1::InDoubt
                | RemoteClassificationV1::ConfirmedApplied
                | RemoteClassificationV1::ConfirmedNotApplied
                | RemoteClassificationV1::PartiallyApplied
                | RemoteClassificationV1::Conflicted
        ),
        RemoteClassificationV1::PartiallyApplied => matches!(
            next,
            RemoteClassificationV1::PartiallyApplied
                | RemoteClassificationV1::ConfirmedApplied
                | RemoteClassificationV1::Conflicted
        ),
        RemoteClassificationV1::Conflicted => matches!(
            next,
            RemoteClassificationV1::Conflicted
                | RemoteClassificationV1::PartiallyApplied
                | RemoteClassificationV1::ConfirmedApplied
        ),
        _ => false,
    }
}

fn effect_intent_value(
    home: CborValue,
    origin: &EffectOriginV1,
    origination_fence: CborValue,
    semantic_use: EffectSemanticUseV1,
    material_inputs: EffectMaterialInputsV1,
    credential_requirements: EffectCredentialRequirementsV1,
    authority: EffectOriginationAuthorityV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-intent.v1")?,
        CborValue::text(EFFECT_ORIGIN_MANIFEST_ID_V1)?,
        CborValue::text(origin.kind().descriptor().descriptor_id())?,
        home,
        origin.canonical_value(),
        origination_fence,
        bytes(semantic_use.as_bytes()),
        bytes(material_inputs.as_bytes()),
        bytes(credential_requirements.as_bytes()),
        bytes(authority.commitment().as_bytes()),
        CborValue::optional(
            authority
                .action_request_id()
                .map(|request| bytes(request.as_bytes())),
        ),
        origination_route_value(origin.kind(), authority.route())?,
    ]))
}

fn home_value(home: EffectIntentHomeV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(match home {
        EffectIntentHomeV1::ActiveStore(home) => active_store_home_value(home)?,
        EffectIntentHomeV1::NoStoreCeremony(home) => no_store_home_value(home)?,
        EffectIntentHomeV1::PreStoreCeremony(home) => pre_store_home_value(home)?,
    })
}

fn active_store_home_value(home: ActiveStoreHomeV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(1),
        domain_kind_value(home.domain_kind),
        token_value(home.stable_domain_id)?,
        token_value(home.realm)?,
        token_value(home.semantic_namespace)?,
        token_value(home.home_qualified_semantic_uniqueness_namespace)?,
    ]))
}

fn no_store_home_value(home: NoStoreCeremonyHomeV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(2),
        token_value(home.protected_installation_realm)?,
        token_value(home.locator_candidate_branch)?,
        token_value(home.installation_context_genesis_ceremony)?,
    ]))
}

fn pre_store_home_value(home: PreStoreCeremonyHomeV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(3),
        token_value(home.allowed_pre_store_ceremony)?,
        domain_kind_value(home.destination_domain_kind),
        token_value(home.candidate_branch_or_destination)?,
        token_value(home.inactive_destination_lineage)?,
    ]))
}

fn origination_fence_value(
    fence: EffectIntentOriginationFenceV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(match fence {
        EffectIntentOriginationFenceV1::ActiveStore(fence) => {
            active_store_origination_value(fence)?
        }
        EffectIntentOriginationFenceV1::NoStoreCeremony(fence) => {
            no_store_origination_value(fence)?
        }
        EffectIntentOriginationFenceV1::PreStoreCeremony(fence) => {
            pre_store_origination_value(fence)?
        }
    })
}

fn active_store_origination_value(
    fence: ActiveStoreOriginationFenceV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(1),
        token_value(fence.store)?,
        token_value(fence.generation)?,
        token_value(fence.epoch)?,
        token_value(fence.namespace)?,
        token_value(fence.material_token)?,
        token_value(fence.action_request)?,
        token_value(fence.action_authority_basis)?,
        token_value(fence.receipt)?,
        token_value(fence.result)?,
        token_value(fence.effect_origin)?,
        token_value(fence.current_authority_commitment)?,
        token_value(fence.credential_commitment)?,
        token_value(fence.dispatch_reservation_or_fence)?,
    ]))
}

fn no_store_origination_value(
    fence: NoStoreCeremonyOriginationFenceV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(2),
        token_value(fence.ceremony_spec)?,
        token_value(fence.ceremony_manifest)?,
        token_value(fence.initiate_mode)?,
        token_value(fence.sealed_ceremony_attempt_commitment)?,
        token_value(fence.attempt_id)?,
        token_value(fence.protected_realm)?,
        token_value(fence.locator_candidate_bundle)?,
        token_value(fence.carrier_identity)?,
        token_value(fence.carrier_incarnation)?,
        token_value(fence.expected_old_token)?,
        token_value(fence.candidate_seal)?,
        token_value(fence.external_anchor)?,
        token_value(fence.idempotency_identity)?,
        token_value(fence.dispatch_fence)?,
    ]))
}

fn pre_store_origination_value(
    fence: PreStoreCeremonyOriginationFenceV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(3),
        token_value(fence.ceremony_spec)?,
        token_value(fence.ceremony_manifest)?,
        token_value(fence.initiate_mode)?,
        token_value(fence.sealed_ceremony_attempt_commitment)?,
        token_value(fence.attempt_id)?,
        token_value(fence.branch_bundle)?,
        token_value(fence.inactive_destination)?,
        token_value(fence.candidate_seal)?,
        token_value(fence.carrier_identity)?,
        token_value(fence.carrier_incarnation)?,
        token_value(fence.expected_old_token)?,
        token_value(fence.external_basis)?,
        token_value(fence.idempotency_identity)?,
        token_value(fence.dispatch_fence)?,
    ]))
}

fn use_fence_value(fence: EffectIntentUseFenceV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(match fence {
        EffectIntentUseFenceV1::ActiveStore(fence) => active_store_use_value(fence)?,
        EffectIntentUseFenceV1::NoStoreCeremony(fence) => no_store_use_value(fence)?,
        EffectIntentUseFenceV1::PreStoreCeremony(fence) => pre_store_use_value(fence)?,
    })
}

fn active_store_use_value(fence: ActiveStoreUseFenceV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(1),
        token_value(fence.same_stable_home)?,
        token_value(fence.generation)?,
        token_value(fence.epoch)?,
        token_value(fence.namespace)?,
        token_value(fence.material_token)?,
        token_value(fence.authority)?,
        token_value(fence.credentials)?,
        token_value(fence.attempt_fence)?,
        token_value(fence.idempotency_binding)?,
        token_value(fence.provider_contract_guards)?,
    ]))
}

fn no_store_use_value(fence: NoStoreCeremonyUseFenceV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(2),
        token_value(fence.same_home)?,
        token_value(fence.branch_authority)?,
        token_value(fence.carrier_incarnation)?,
        token_value(fence.expected_old_token)?,
        token_value(fence.attempt_id)?,
    ]))
}

fn pre_store_use_value(
    fence: PreStoreCeremonyUseFenceV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(3),
        token_value(fence.same_home)?,
        token_value(fence.branch_authority)?,
        token_value(fence.carrier)?,
        token_value(fence.expected_old_token)?,
        token_value(fence.attempt_id)?,
    ]))
}

fn dispatch_transition_carrier_value(
    schema: &'static str,
    dispatch: &EffectDispatchAttemptV1,
    control_need: &EffectControlTransitionNeedV1,
    run_set: &RunSetV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    if run_set.owner() != ExecutionAttemptOwnerV1::Dispatch(dispatch.attempt().id()) {
        return Err(EffectRuntimeErrorV1::RunSetOwnerMismatch);
    }
    Ok(CborValue::Array(vec![
        CborValue::text(schema)?,
        dispatch.canonical_value()?,
        control_need.canonical_value()?,
        run_set_binding_value(run_set),
    ]))
}

fn reconciliation_transition_carrier_value(
    schema: &'static str,
    reconciliation: &EffectReconciliationAttemptV1,
    control_need: &EffectControlTransitionNeedV1,
) -> Result<CborValue, EffectRuntimeErrorV1> {
    if reconciliation.run_set().owner()
        != ExecutionAttemptOwnerV1::Reconciliation(reconciliation.attempt().id())
    {
        return Err(EffectRuntimeErrorV1::RunSetOwnerMismatch);
    }
    Ok(CborValue::Array(vec![
        CborValue::text(schema)?,
        reconciliation.canonical_value()?,
        control_need.canonical_value()?,
        run_set_binding_value(reconciliation.run_set()),
    ]))
}

fn run_set_binding_value(run_set: &RunSetV1) -> CborValue {
    CborValue::Array(vec![
        execution_owner_value(run_set.owner()),
        CborValue::Unsigned(run_set.revision()),
        CborValue::Array(
            run_set
                .runs()
                .iter()
                .map(|run| {
                    CborValue::Array(vec![
                        bytes(run.id().as_bytes()),
                        execution_owner_value(run.owner()),
                        run_state_value(run.state()),
                        CborValue::Unsigned(u64::from(run.launch_ordinal())),
                    ])
                })
                .collect(),
        ),
    ])
}

const fn run_state_value(state: RunStateV1) -> CborValue {
    CborValue::Unsigned(match state {
        RunStateV1::Reserved => 1,
        RunStateV1::Active => 2,
        RunStateV1::DefinitelyNotStarted => 3,
        RunStateV1::Succeeded => 4,
        RunStateV1::Failed => 5,
        RunStateV1::Cancelled => 6,
        RunStateV1::TimedOut => 7,
        RunStateV1::Lost => 8,
        RunStateV1::Fenced => 9,
    })
}

pub(crate) fn execution_owner_value(owner: ExecutionAttemptOwnerV1) -> CborValue {
    match owner {
        ExecutionAttemptOwnerV1::Step(id) => {
            CborValue::Array(vec![CborValue::Unsigned(1), bytes(id.as_bytes())])
        }
        ExecutionAttemptOwnerV1::Dispatch(id) => {
            CborValue::Array(vec![CborValue::Unsigned(2), bytes(id.as_bytes())])
        }
        ExecutionAttemptOwnerV1::Reconciliation(id) => {
            CborValue::Array(vec![CborValue::Unsigned(3), bytes(id.as_bytes())])
        }
    }
}

const fn remote_classification_tag(classification: RemoteClassificationV1) -> u64 {
    match classification {
        RemoteClassificationV1::Prepared => 1,
        RemoteClassificationV1::Dispatching => 2,
        RemoteClassificationV1::Pending => 3,
        RemoteClassificationV1::InDoubt => 4,
        RemoteClassificationV1::ConfirmedApplied => 5,
        RemoteClassificationV1::ConfirmedNotApplied => 6,
        RemoteClassificationV1::PartiallyApplied => 7,
        RemoteClassificationV1::Conflicted => 8,
        RemoteClassificationV1::Cancelled => 9,
    }
}

const fn remote_classification_value(classification: RemoteClassificationV1) -> CborValue {
    CborValue::Unsigned(remote_classification_tag(classification))
}

fn withdrawal_catalog_cell_value(
    cell: super::withdrawal::WithdrawalCatalogCellV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(cell.classification_literal())?,
        CborValue::text(cell.compatibility_identity())?,
        CborValue::Unsigned(u64::from(cell.origin_tag())),
        CborValue::Unsigned(u64::from(cell.route_tag())),
        CborValue::Unsigned(cell.branch_tag()),
        CborValue::text(cell.role_literal())?,
        CborValue::text(cell.home_literal())?,
        CborValue::text(cell.catalog_descriptor_id())?,
        CborValue::text(cell.semantic_binding())?,
    ]))
}

fn withdrawal_request_value(request: WithdrawalRequestV1) -> Result<CborValue, CborError> {
    let path = match request.path {
        WithdrawalAuthorityPathV1::Ordinary => CborValue::Array(vec![CborValue::Unsigned(1)]),
        WithdrawalAuthorityPathV1::BootstrapG0 => CborValue::Array(vec![CborValue::Unsigned(2)]),
        WithdrawalAuthorityPathV1::ContinuityMaintenance(slot) => CborValue::Array(vec![
            CborValue::Unsigned(3),
            CborValue::Unsigned(match slot {
                EffectWithdrawalSlotFamilyV1::MaintenanceExecutorCurrentness => 1,
                EffectWithdrawalSlotFamilyV1::ProspectiveContinuityCarrier => 2,
                EffectWithdrawalSlotFamilyV1::PlannedTurnoverHighWater => 3,
                EffectWithdrawalSlotFamilyV1::RepositoryRecoveryAdmission => 4,
                EffectWithdrawalSlotFamilyV1::InstallationRecoveryAdmission => 5,
            }),
        ]),
        WithdrawalAuthorityPathV1::Ceremony => CborValue::Array(vec![CborValue::Unsigned(4)]),
    };
    Ok(CborValue::Array(vec![
        withdrawal_catalog_cell_value(request.catalog_cell)?,
        CborValue::Unsigned(match request.home {
            EffectIntentHomeKindV1::ActiveStore => 1,
            EffectIntentHomeKindV1::NoStoreCeremony => 2,
            EffectIntentHomeKindV1::PreStoreCeremony => 3,
        }),
        path,
        CborValue::Unsigned(match request.live_dispatch {
            EffectIntentLiveDispatchV1::None => 1,
            EffectIntentLiveDispatchV1::Reserved => 2,
            EffectIntentLiveDispatchV1::Sealed => 3,
        }),
        remote_classification_value(request.classification),
        CborValue::Bool(request.has_live_attempt),
        CborValue::Bool(request.has_dispatch_fence),
        CborValue::Bool(request.has_seal),
        CborValue::Bool(request.has_release_capability),
        CborValue::Bool(request.runs_closed),
        CborValue::Bool(request.same_home_current),
        CborValue::Bool(request.authority_current),
        CborValue::Bool(request.capacity_current),
        CborValue::Bool(request.expected_old_head),
        CborValue::Bool(request.expected_old_carrier),
    ]))
}

fn parse_withdrawal_request(
    value: &CborValue,
) -> Result<WithdrawalRequestV1, EffectRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let [
        CborValue::Array(catalog_cell),
        CborValue::Unsigned(home),
        CborValue::Array(path),
        CborValue::Unsigned(live_dispatch),
        CborValue::Unsigned(classification),
        CborValue::Bool(has_live_attempt),
        CborValue::Bool(has_dispatch_fence),
        CborValue::Bool(has_seal),
        CborValue::Bool(has_release_capability),
        CborValue::Bool(runs_closed),
        CborValue::Bool(same_home_current),
        CborValue::Bool(authority_current),
        CborValue::Bool(capacity_current),
        CborValue::Bool(expected_old_head),
        CborValue::Bool(expected_old_carrier),
    ] = fields.as_slice()
    else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let home = match home {
        1 => EffectIntentHomeKindV1::ActiveStore,
        2 => EffectIntentHomeKindV1::NoStoreCeremony,
        3 => EffectIntentHomeKindV1::PreStoreCeremony,
        _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    };
    let path = match path.as_slice() {
        [CborValue::Unsigned(1)] => WithdrawalAuthorityPathV1::Ordinary,
        [CborValue::Unsigned(2)] => WithdrawalAuthorityPathV1::BootstrapG0,
        [CborValue::Unsigned(3), CborValue::Unsigned(slot)] => {
            WithdrawalAuthorityPathV1::ContinuityMaintenance(match slot {
                1 => EffectWithdrawalSlotFamilyV1::MaintenanceExecutorCurrentness,
                2 => EffectWithdrawalSlotFamilyV1::ProspectiveContinuityCarrier,
                3 => EffectWithdrawalSlotFamilyV1::PlannedTurnoverHighWater,
                4 => EffectWithdrawalSlotFamilyV1::RepositoryRecoveryAdmission,
                5 => EffectWithdrawalSlotFamilyV1::InstallationRecoveryAdmission,
                _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
            })
        }
        [CborValue::Unsigned(4)] => WithdrawalAuthorityPathV1::Ceremony,
        _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    };
    let live_dispatch = match live_dispatch {
        1 => EffectIntentLiveDispatchV1::None,
        2 => EffectIntentLiveDispatchV1::Reserved,
        3 => EffectIntentLiveDispatchV1::Sealed,
        _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    };
    let [
        CborValue::Text(source_classification),
        CborValue::Text(compatibility_identity),
        CborValue::Unsigned(origin_tag),
        CborValue::Unsigned(route_tag),
        CborValue::Unsigned(branch_tag),
        CborValue::Text(role_literal),
        CborValue::Text(home_literal),
        CborValue::Text(catalog_descriptor_id),
        CborValue::Text(semantic_binding),
    ] = catalog_cell.as_slice()
    else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let classification = parse_remote_classification(*classification)?;
    if source_classification
        != classification
            .withdrawal_source_literal()
            .ok_or(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?
        || home_literal
            != match home {
                EffectIntentHomeKindV1::ActiveStore => "ActiveStoreHomeV1",
                EffectIntentHomeKindV1::NoStoreCeremony => "NoStoreCeremonyHomeV1",
                EffectIntentHomeKindV1::PreStoreCeremony => "PreStoreCeremonyHomeV1",
            }
    {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    }
    let catalog_cell = exact_withdrawal_catalog_cell_v1(
        classification,
        compatibility_identity,
        u8::try_from(*origin_tag).map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
        u8::try_from(*route_tag).map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
        *branch_tag,
        role_literal,
        home,
        catalog_descriptor_id,
        semantic_binding,
    )?;
    let request = WithdrawalRequestV1 {
        catalog_cell,
        home,
        path,
        live_dispatch,
        classification,
        has_live_attempt: *has_live_attempt,
        has_dispatch_fence: *has_dispatch_fence,
        has_seal: *has_seal,
        has_release_capability: *has_release_capability,
        runs_closed: *runs_closed,
        same_home_current: *same_home_current,
        authority_current: *authority_current,
        capacity_current: *capacity_current,
        expected_old_head: *expected_old_head,
        expected_old_carrier: *expected_old_carrier,
    };
    if withdrawal_request_value(request)? != *value {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    }
    Ok(request)
}

fn step_binding_value(binding: StepBindingV1) -> CborValue {
    let scope = binding.scope();
    CborValue::Array(vec![
        bytes(scope.repository_id().as_bytes()),
        bytes(scope.work_id().as_bytes()),
        bytes(binding.contract_generation_id().as_bytes()),
        bytes(binding.contract_root_id().as_bytes()),
        bytes(binding.step_id().as_bytes()),
        bytes(binding.revision_id().as_bytes()),
    ])
}

const fn domain_kind_value(kind: EffectIntentDomainKindV1) -> CborValue {
    CborValue::Unsigned(match kind {
        EffectIntentDomainKindV1::RepositoryDomain => 1,
        EffectIntentDomainKindV1::InstallationDomain => 2,
    })
}

fn token_value(token: HomeTokenV1) -> Result<CborValue, EffectRuntimeErrorV1> {
    require_nonzero(*token.as_bytes())?;
    Ok(bytes(token.as_bytes()))
}

fn parse_active_store_home(value: &CborValue) -> Result<EffectIntentHomeV1, EffectRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let [
        CborValue::Unsigned(1),
        CborValue::Unsigned(domain_kind),
        stable,
        realm,
        namespace,
        uniqueness,
    ] = fields.as_slice()
    else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    Ok(EffectIntentHomeV1::ActiveStore(ActiveStoreHomeV1 {
        domain_kind: match domain_kind {
            1 => EffectIntentDomainKindV1::RepositoryDomain,
            2 => EffectIntentDomainKindV1::InstallationDomain,
            _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
        },
        stable_domain_id: parse_home_token(stable)?,
        realm: parse_home_token(realm)?,
        semantic_namespace: parse_home_token(namespace)?,
        home_qualified_semantic_uniqueness_namespace: parse_home_token(uniqueness)?,
    }))
}

fn parse_active_store_origination_fence(
    value: &CborValue,
) -> Result<EffectIntentOriginationFenceV1, EffectRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let [
        CborValue::Unsigned(1),
        store,
        generation,
        epoch,
        namespace,
        material,
        request,
        basis,
        receipt,
        result,
        origin,
        authority,
        credentials,
        dispatch,
    ] = fields.as_slice()
    else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    Ok(EffectIntentOriginationFenceV1::ActiveStore(
        ActiveStoreOriginationFenceV1 {
            store: parse_home_token(store)?,
            generation: parse_home_token(generation)?,
            epoch: parse_home_token(epoch)?,
            namespace: parse_home_token(namespace)?,
            material_token: parse_home_token(material)?,
            action_request: parse_home_token(request)?,
            action_authority_basis: parse_home_token(basis)?,
            receipt: parse_home_token(receipt)?,
            result: parse_home_token(result)?,
            effect_origin: parse_home_token(origin)?,
            current_authority_commitment: parse_home_token(authority)?,
            credential_commitment: parse_home_token(credentials)?,
            dispatch_reservation_or_fence: parse_home_token(dispatch)?,
        },
    ))
}

fn parse_active_store_use_fence(
    value: &CborValue,
) -> Result<EffectIntentUseFenceV1, EffectRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let [
        CborValue::Unsigned(1),
        same_stable_home,
        generation,
        epoch,
        namespace,
        material_token,
        authority,
        credentials,
        attempt_fence,
        idempotency_binding,
        provider_contract_guards,
    ] = fields.as_slice()
    else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    Ok(EffectIntentUseFenceV1::ActiveStore(ActiveStoreUseFenceV1 {
        same_stable_home: parse_home_token(same_stable_home)?,
        generation: parse_home_token(generation)?,
        epoch: parse_home_token(epoch)?,
        namespace: parse_home_token(namespace)?,
        material_token: parse_home_token(material_token)?,
        authority: parse_home_token(authority)?,
        credentials: parse_home_token(credentials)?,
        attempt_fence: parse_home_token(attempt_fence)?,
        idempotency_binding: parse_home_token(idempotency_binding)?,
        provider_contract_guards: parse_home_token(provider_contract_guards)?,
    }))
}

fn parse_effect_origin(value: &CborValue) -> Result<EffectOriginV1, EffectRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    match fields.as_slice() {
        [
            CborValue::Unsigned(1),
            CborValue::Unsigned(kind),
            binding,
            attempt,
            lease,
            CborValue::Unsigned(lease_fence),
            term,
            CborValue::Unsigned(term_ordinal),
        ] => {
            if *kind != u64::from(EffectOriginKindV1::StepEffectOrigin.tag())
                || *lease_fence == 0
                || *term_ordinal == 0
            {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            }
            Ok(EffectOriginV1::Step(Box::new(StepEffectOriginV1 {
                binding: super::runtime::parse_step_binding(binding)?,
                attempt_id: StepAttemptIdV1::from_bytes(exact_effect_digest(attempt)?)?,
                lease_id: StepLeaseIdV1::from_bytes(exact_effect_digest(lease)?)?,
                lease_fence: *lease_fence,
                lease_term_id: LeaseTermIdV1::from_bytes(exact_effect_digest(term)?)?,
                lease_term_ordinal: *term_ordinal,
            })))
        }
        [
            CborValue::Unsigned(2),
            CborValue::Unsigned(kind),
            provenance,
        ] => {
            let kind = u8::try_from(*kind)
                .map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?;
            EffectOriginV1::non_step(
                EffectOriginKindV1::from_tag(kind)?,
                exact_effect_digest(provenance)?,
            )
        }
        _ => Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    }
}

fn parse_action_origination_route(
    value: &CborValue,
    kind: EffectOriginKindV1,
) -> Result<EffectOriginationAuthorityRouteV1, EffectRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let [
        CborValue::Unsigned(1),
        CborValue::Text(origin_descriptor),
        CborValue::Unsigned(action_tag),
        CborValue::Text(action_descriptor),
        CborValue::Unsigned(mode),
    ] = fields.as_slice()
    else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    let action = ExecutionActionV1::ALL
        .into_iter()
        .find(|action| action.global_tag() == *action_tag)
        .ok_or(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?;
    if origin_descriptor != kind.descriptor().descriptor_id()
        || action_descriptor != action.descriptor_id()
    {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    }
    Ok(EffectOriginationAuthorityRouteV1::Action {
        action,
        reservation_mode: match mode {
            1 => DispatchReservationModeV1::InitiateNew,
            2 => DispatchReservationModeV1::RecoverReserved,
            _ => return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
        },
    })
}

fn parse_home_token(value: &CborValue) -> Result<HomeTokenV1, EffectRuntimeErrorV1> {
    let digest = exact_effect_digest(value)?;
    require_nonzero(digest)?;
    Ok(HomeTokenV1::new(digest))
}

fn parse_optional_effect_digest(
    value: &CborValue,
) -> Result<Option<[u8; 32]>, EffectRuntimeErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), value] = fields.as_slice() else {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            };
            Ok(Some(exact_effect_digest(value)?))
        }
        _ => Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    }
}

fn parse_optional_reconciliation_read_usage(
    value: &CborValue,
) -> Result<Option<EffectReconciliationReadUsageV1>, EffectRuntimeErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), CborValue::Array(usage)] = fields.as_slice() else {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            };
            let [
                CborValue::Unsigned(requests),
                CborValue::Unsigned(pages),
                CborValue::Unsigned(bytes),
                CborValue::Unsigned(duration_ms),
                result_commitment,
            ] = usage.as_slice()
            else {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            };
            Ok(Some(EffectReconciliationReadUsageV1 {
                requests: u16::try_from(*requests)
                    .map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
                pages: u16::try_from(*pages)
                    .map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)?,
                bytes: *bytes,
                duration_ms: *duration_ms,
                result_commitment: exact_effect_digest(result_commitment)?,
            }))
        }
        _ => Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    }
}

fn parse_optional_step_binding(
    value: &CborValue,
) -> Result<Option<StepBindingV1>, EffectRuntimeErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), binding] = fields.as_slice() else {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            };
            Ok(Some(super::runtime::parse_step_binding(binding)?))
        }
        _ => Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    }
}

fn parse_optional_remote_classification(
    value: &CborValue,
) -> Result<Option<RemoteClassificationV1>, EffectRuntimeErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), CborValue::Unsigned(tag)] = fields.as_slice() else {
                return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
            };
            Ok(Some(parse_remote_classification(*tag)?))
        }
        _ => Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    }
}

fn parse_remote_classification(tag: u64) -> Result<RemoteClassificationV1, EffectRuntimeErrorV1> {
    match tag {
        1 => Ok(RemoteClassificationV1::Prepared),
        2 => Ok(RemoteClassificationV1::Dispatching),
        3 => Ok(RemoteClassificationV1::Pending),
        4 => Ok(RemoteClassificationV1::InDoubt),
        5 => Ok(RemoteClassificationV1::ConfirmedApplied),
        6 => Ok(RemoteClassificationV1::ConfirmedNotApplied),
        7 => Ok(RemoteClassificationV1::PartiallyApplied),
        8 => Ok(RemoteClassificationV1::Conflicted),
        9 => Ok(RemoteClassificationV1::Cancelled),
        _ => Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier),
    }
}

fn exact_effect_digest(value: &CborValue) -> Result<[u8; 32], EffectRuntimeErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
}

fn commitment(value: [u8; 32]) -> Result<DispatchCommitmentV1, EffectRuntimeErrorV1> {
    Ok(DispatchCommitmentV1::new(value)?)
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn require_nonzero(value: [u8; 32]) -> Result<(), EffectRuntimeErrorV1> {
    if value == [0; 32] {
        Err(EffectRuntimeErrorV1::MissingCommitment)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectRuntimeErrorV1 {
    #[error("stored Effect Intent or Attempt carrier is malformed or non-canonical")]
    InvalidStoredEffectCarrier,
    #[error("unknown frozen Effect Origin tag {0}")]
    UnknownEffectOrigin(u8),
    #[error("unknown frozen Ceremony symbol tag {0}")]
    UnknownCeremonySymbol(u8),
    #[error("Effect Origin, frozen route, Home, request mode, and authority do not agree")]
    OriginRouteMismatch,
    #[error(
        "Step Effect Origin requires an exact live Step Binding, Attempt, Lease fence, and Lease term"
    )]
    StepOriginRequiresLiveLeaseAuthority,
    #[error("effect commitment must not be all zero")]
    MissingCommitment,
    #[error(
        "Effect Intent control revision does not bind the same stable Intent, material, and credentials"
    )]
    ControlBasisMismatch,
    #[error("Effect Origin is ceremony-only and has no Action dispatch route")]
    CeremonyOnlyEffectOrigin,
    #[error("authorized Action Request does not own this Effect Origin route phase")]
    WrongEffectAction,
    #[error("Action Request is not fresh for this Effect Intent use")]
    StaleActionRequest,
    #[error("first Dispatch reservation must use the Effect Intent origination Action Request")]
    DispatchOriginationRequestMismatch,
    #[error("Effect Intent is not in the exact state required by this operation")]
    IllegalEffectState,
    #[error("active-Store Effect Intent control cannot be created for a ceremony-only Home")]
    ActiveStoreControlRequired,
    #[error(
        "Effect Intent use fence does not bind the exact immutable Home and origination carrier"
    )]
    UseFenceBasisMismatch,
    #[error("Effect Intent use fence is not the exact current Control Revision carrier")]
    UseFenceNotCurrent,
    #[error("remote Dispatch outcome requires the already-persisted crossing seal")]
    RemoteOutcomeRequiresCrossingSeal,
    #[error("remote classification is not legal for the typed Dispatch outcome")]
    InvalidRemoteClassification,
    #[error("reconciliation result would infer or erase remote truth")]
    IllegalReconciliationRefinement,
    #[error("Reconciliation read plan contains an effecting operation")]
    EffectingReconciliationRead,
    #[error(
        "Reconciliation read plan must have finite non-zero request, page, byte, and duration bounds"
    )]
    InvalidReconciliationReadBounds,
    #[error("Reconciliation Run must exactly bind the immutable read plan without a Step term")]
    InvalidReconciliationReadRun,
    #[error("Reconciliation read exceeded its sealed request, page, byte, or duration budget")]
    ReconciliationReadBoundsExceeded,
    #[error("a second Reconciliation poll or read-plan expansion requires a new Attempt")]
    SecondReconciliationPollRequiresNewAttempt,
    #[error("Reconciliation cannot finish before its one bounded read Run completes")]
    ReconciliationReadNotCompleted,
    #[error(
        "withdrawal did not carry the exact current Home, authority, capacity, Head, and carrier"
    )]
    WithdrawalCurrentCarrierMismatch,
    #[error("Effect Intent monotonic counter overflowed")]
    CounterOverflow,
    #[error("RunSet is not owned by the Effect Attempt in this persistence carrier")]
    RunSetOwnerMismatch,
    #[error("Effect Attempt must carry exactly one canonical Run before provider execution")]
    MissingAttemptRun,
    #[error("Effect control and owned RunSet revisions do not advance in one transaction")]
    RunSetRevisionMismatch,
    #[error(transparent)]
    Home(#[from] EffectIntentHomeError),
    #[error(transparent)]
    Execution(#[from] ExecutionRuntimeErrorV1),
    #[error(transparent)]
    Dispatch(#[from] DispatchStateError),
    #[error(transparent)]
    Withdrawal(#[from] WithdrawalError),
    #[error(transparent)]
    Control(#[from] EffectIntentControlErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::authority::{
        ActionAuthorityBasisKindV1, AuthorityContextIdV1, AuthorizationReceiptV1,
        IdempotencyKeyIdV1, StateTokenIdV1,
    };
    use crate::domain::vnext::execution::control_head::EffectIntentControlPublicationCommitmentsV1;
    use crate::domain::vnext::execution::runtime::CanonicalExecutionActionRequestV1;

    fn token(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn authorized(action: ExecutionActionV1, seed: &str) -> AuthorizedExecutionActionV1 {
        let request = CanonicalExecutionActionRequestV1::new(
            action,
            token(90),
            token(91),
            token(92),
            IdempotencyKeyIdV1::derive(seed).unwrap(),
        )
        .unwrap();
        let receipt = AuthorizationReceiptV1::new(
            request.request_id(),
            AuthorityContextIdV1::derive("stage4-effect-context").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive("stage4-effect-old-state").unwrap(),
            StateTokenIdV1::derive("stage4-effect-new-state").unwrap(),
        )
        .unwrap();
        AuthorizedExecutionActionV1::new(request, receipt).unwrap()
    }

    fn active_home() -> EffectIntentHomeV1 {
        EffectIntentHomeV1::ActiveStore(ActiveStoreHomeV1 {
            domain_kind: EffectIntentDomainKindV1::RepositoryDomain,
            stable_domain_id: HomeTokenV1::new(token(1)),
            realm: HomeTokenV1::new(token(2)),
            semantic_namespace: HomeTokenV1::new(token(3)),
            home_qualified_semantic_uniqueness_namespace: HomeTokenV1::new(token(4)),
        })
    }

    fn active_origination_fence() -> EffectIntentOriginationFenceV1 {
        EffectIntentOriginationFenceV1::ActiveStore(ActiveStoreOriginationFenceV1 {
            store: HomeTokenV1::new(token(1)),
            generation: HomeTokenV1::new(token(5)),
            epoch: HomeTokenV1::new(token(6)),
            namespace: HomeTokenV1::new(token(3)),
            material_token: HomeTokenV1::new(token(7)),
            action_request: HomeTokenV1::new(token(8)),
            action_authority_basis: HomeTokenV1::new(token(9)),
            receipt: HomeTokenV1::new(token(10)),
            result: HomeTokenV1::new(token(11)),
            effect_origin: HomeTokenV1::new(token(12)),
            current_authority_commitment: HomeTokenV1::new(token(13)),
            credential_commitment: HomeTokenV1::new(token(14)),
            dispatch_reservation_or_fence: HomeTokenV1::new(token(15)),
        })
    }

    fn active_use_fence(idempotency_seed: u8) -> EffectIntentUseFenceV1 {
        EffectIntentUseFenceV1::ActiveStore(ActiveStoreUseFenceV1 {
            same_stable_home: HomeTokenV1::new(token(1)),
            generation: HomeTokenV1::new(token(5)),
            epoch: HomeTokenV1::new(token(6)),
            namespace: HomeTokenV1::new(token(3)),
            material_token: HomeTokenV1::new(token(7)),
            authority: HomeTokenV1::new(token(13)),
            credentials: HomeTokenV1::new(token(14)),
            attempt_fence: HomeTokenV1::new(token(15)),
            idempotency_binding: HomeTokenV1::new(token(idempotency_seed)),
            provider_contract_guards: HomeTokenV1::new(token(16)),
        })
    }

    fn intent_and_control() -> (
        EffectIntentV1,
        AuthorizedExecutionActionV1,
        EffectIntentUseFenceV1,
        EffectIntentInitialControlV1,
    ) {
        let reserve = authorized(ExecutionActionV1::OriginateEffectIntent, "effect-originate");
        let intent = EffectIntentV1::originate(EffectIntentOriginationV1 {
            home: active_home(),
            origin: EffectOriginV1::non_step(
                EffectOriginKindV1::EffectRemediationOrigin,
                token(20),
            )
            .unwrap(),
            origination_fence: active_origination_fence(),
            semantic_use: EffectSemanticUseV1::new(token(21)).unwrap(),
            material_inputs: EffectMaterialInputsV1::new(token(22)).unwrap(),
            credential_requirements: EffectCredentialRequirementsV1::new(token(23)).unwrap(),
            authority: EffectOriginationAuthorityV1::from_action(&reserve).unwrap(),
        })
        .unwrap();
        let use_fence = active_use_fence(17);
        let control = intent.initial_active_store_control(use_fence).unwrap();
        (intent, reserve, use_fence, control)
    }

    fn dispatch_preparation(use_fence: EffectIntentUseFenceV1) -> EffectDispatchPreparationV1 {
        EffectDispatchPreparationV1 {
            use_fence,
            attempt_revision: 1,
            application_envelope_commitment: token(30),
            provider_operation_contract_commitment: token(31),
            provider_scope_commitment: token(32),
            provider_key_commitment: token(33),
            authority_basis_commitment: token(34),
            material_stamp_commitment: token(35),
            run_set_revision_commitment: token(36),
            accounting_basis_commitment: token(37),
            provider_run: RunReservationV1 {
                semantic_operation_hash: token(38),
                inputs_commitment: token(39),
                environment_commitment: token(40),
                target_commitment: token(41),
                execution_boundary_commitment: token(42),
                deadline: 100,
                launch_ordinal: 1,
                current_step_term: None,
            },
        }
    }

    fn reconciliation_plan() -> EffectReconciliationReadPlanV1 {
        EffectReconciliationReadPlanV1::new(EffectReconciliationReadPlanPartsV1 {
            classification: ReconciliationReadOperationClassificationV1::EffectFreeRead,
            operation_kind: ReconciliationReadOperationKindV1::ProviderStatus,
            provider_commitment: token(70),
            account_commitment: token(71),
            target_commitment: token(72),
            correlation_commitment: token(73),
            credential_commitment: token(74),
            visibility_commitment: token(75),
            query_commitment: token(76),
            evaluator_commitment: token(77),
            max_requests: 1,
            max_pages: 1,
            max_bytes: 1024,
            max_duration_ms: 500,
        })
        .unwrap()
    }

    fn reconciliation_use_fence(
        intent: &EffectIntentV1,
        authority: &AuthorizedExecutionActionV1,
        read_plan: EffectReconciliationReadPlanV1,
    ) -> EffectIntentUseFenceV1 {
        let attempt_fence = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-attempt-fence.v1").unwrap(),
            bytes(intent.id().as_bytes()),
            bytes(authority.request_id().as_bytes()),
        ]))
        .unwrap();
        EffectIntentUseFenceV1::ActiveStore(ActiveStoreUseFenceV1 {
            same_stable_home: HomeTokenV1::new(token(1)),
            generation: HomeTokenV1::new(token(78)),
            epoch: HomeTokenV1::new(token(79)),
            namespace: HomeTokenV1::new(token(3)),
            material_token: HomeTokenV1::new(token(22)),
            authority: HomeTokenV1::new(token(80)),
            credentials: HomeTokenV1::new(token(23)),
            attempt_fence: HomeTokenV1::new(attempt_fence),
            idempotency_binding: HomeTokenV1::new(
                *authority.request().idempotency_key_id().as_bytes(),
            ),
            provider_contract_guards: HomeTokenV1::new(read_plan.commitment().unwrap()),
        })
    }

    #[test]
    fn effect_origin_catalog_is_exact_and_closed() {
        assert_eq!(EffectOriginKindV1::ALL.len(), EFFECT_ORIGIN_COUNT_V1);
        assert_eq!(
            EffectOriginKindV1::ALL
                .iter()
                .map(|kind| usize::from(kind.descriptor().route_count()))
                .sum::<usize>(),
            139
        );
        for (index, kind) in EffectOriginKindV1::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(kind.tag()), index + 1);
            assert_eq!(EffectOriginKindV1::from_tag(kind.tag()).unwrap(), kind);
            assert_eq!(kind.descriptor().kind(), kind);
        }
        assert!(matches!(
            EffectOriginKindV1::from_tag(24),
            Err(EffectRuntimeErrorV1::UnknownEffectOrigin(24))
        ));
    }

    #[test]
    fn dispatch_control_carries_fresh_fence_and_preserves_crossing_uncertainty() {
        let (intent, reserve, use_fence, control) = intent_and_control();
        let prepared = intent
            .prepare_dispatch(
                control.revision(),
                dispatch_preparation(use_fence),
                &reserve,
            )
            .unwrap();
        let reserve_transition = prepared
            .control_need()
            .control_transition(control.head(), control.revision(), control.writer_term())
            .unwrap();
        let (reserved_revision, reserved_head) = reserve_transition
            .apply(
                control.head(),
                control.revision(),
                control.writer_term(),
                None,
            )
            .unwrap();
        assert_eq!(
            reserved_revision.live_dispatch(),
            EffectIntentLiveDispatchV1::Reserved
        );
        assert_eq!(
            reserved_revision.parts().use_fence_commitment,
            prepared.dispatch().use_fence_commitment()
        );

        let seal_authority = authorized(ExecutionActionV1::RecordDispatchOutcome, "effect-seal");
        let seal = prepared
            .dispatch()
            .seal_candidate(&intent, &reserved_revision, token(40), &seal_authority)
            .unwrap();
        assert!(!seal.grants_provider_release_capability());
        let seal_transition = seal
            .control_need()
            .control_transition(&reserved_head, &reserved_revision, control.writer_term())
            .unwrap();
        let (sealed_revision, sealed_head) = seal_transition
            .apply(
                &reserved_head,
                &reserved_revision,
                control.writer_term(),
                None,
            )
            .unwrap();
        assert_eq!(
            sealed_revision.live_dispatch(),
            EffectIntentLiveDispatchV1::Sealed
        );
        assert_eq!(
            sealed_revision.classification(),
            RemoteClassificationV1::InDoubt
        );
        assert_eq!(
            seal.dispatch().recovery_disposition(),
            DispatchRecoveryDispositionV1::SealedInDoubtNoIo
        );

        let finish_authority = authorized(
            ExecutionActionV1::RecordDispatchOutcome,
            "effect-finish-dispatch",
        );
        let terminal = seal
            .dispatch()
            .terminal_candidate(
                &intent,
                &sealed_revision,
                EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                    evidence_commitment: token(41),
                },
                token(42),
                token(43),
                &finish_authority,
            )
            .unwrap();
        let finish_transition = terminal
            .control_need()
            .control_transition(&sealed_head, &sealed_revision, control.writer_term())
            .unwrap();
        let publication =
            super::super::control_head::EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                token(44),
                token(45),
            )
            .unwrap();
        let (terminal_revision, _) = finish_transition
            .apply(
                &sealed_head,
                &sealed_revision,
                control.writer_term(),
                Some(publication),
            )
            .unwrap();
        assert_eq!(
            terminal_revision.live_dispatch(),
            EffectIntentLiveDispatchV1::None
        );
        assert_eq!(
            terminal_revision.classification(),
            RemoteClassificationV1::InDoubt
        );
        assert!(terminal_revision.runs_closed());
        assert_eq!(
            terminal.dispatch().recovery_disposition(),
            DispatchRecoveryDispositionV1::TerminalInDoubt
        );
    }

    #[test]
    fn persisted_dispatch_carrier_rejects_phase_owner_and_runset_mutants() {
        let (intent, reserve, use_fence, control) = intent_and_control();
        let prepared = intent
            .prepare_dispatch(
                control.revision(),
                dispatch_preparation(use_fence),
                &reserve,
            )
            .unwrap();
        let reserve_transition = prepared
            .control_need()
            .control_transition(control.head(), control.revision(), control.writer_term())
            .unwrap();
        let (reserved_revision, reserved_head) = reserve_transition
            .apply(
                control.head(),
                control.revision(),
                control.writer_term(),
                None,
            )
            .unwrap();
        let seal_authority = authorized(ExecutionActionV1::RecordDispatchOutcome, "decode-seal");
        let seal = prepared
            .dispatch()
            .seal_candidate(&intent, &reserved_revision, token(60), &seal_authority)
            .unwrap();
        let seal_transition = seal
            .control_need()
            .control_transition(&reserved_head, &reserved_revision, control.writer_term())
            .unwrap();
        let (sealed_revision, _) = seal_transition
            .apply(
                &reserved_head,
                &reserved_revision,
                control.writer_term(),
                None,
            )
            .unwrap();
        let finish_authority =
            authorized(ExecutionActionV1::RecordDispatchOutcome, "decode-terminal");
        let terminal = seal
            .dispatch()
            .terminal_candidate(
                &intent,
                &sealed_revision,
                EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                    evidence_commitment: token(61),
                },
                token(62),
                token(63),
                &finish_authority,
            )
            .unwrap();

        for value in [
            prepared.persistence_carrier_value().unwrap(),
            seal.persistence_carrier_value().unwrap(),
            terminal.persistence_carrier_value().unwrap(),
        ] {
            EffectDispatchAttemptV1::from_persistence_carrier_value(&value).unwrap();
        }

        let mut wrong_phase = seal.persistence_carrier_value().unwrap();
        let CborValue::Array(fields) = &mut wrong_phase else {
            unreachable!()
        };
        fields[0] =
            CborValue::text("maestro.vnext.effect-dispatch-reservation-carrier.v1").unwrap();
        assert!(matches!(
            EffectDispatchAttemptV1::from_persistence_carrier_value(&wrong_phase),
            Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
        ));

        let mut wrong_terminal_classification = terminal.persistence_carrier_value().unwrap();
        let CborValue::Array(fields) = &mut wrong_terminal_classification else {
            unreachable!()
        };
        let CborValue::Array(need) = &mut fields[2] else {
            unreachable!()
        };
        let CborValue::Array(body) = &mut need[1] else {
            unreachable!()
        };
        body[3] = remote_classification_value(RemoteClassificationV1::ConfirmedApplied);
        assert!(matches!(
            EffectDispatchAttemptV1::from_persistence_carrier_value(&wrong_terminal_classification),
            Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
        ));

        let mut erased_uncertainty = terminal.persistence_carrier_value().unwrap();
        let CborValue::Array(fields) = &mut erased_uncertainty else {
            unreachable!()
        };
        let CborValue::Array(dispatch) = &mut fields[1] else {
            unreachable!()
        };
        dispatch[9] = CborValue::optional(Some(remote_classification_value(
            RemoteClassificationV1::ConfirmedApplied,
        )));
        let CborValue::Array(need) = &mut fields[2] else {
            unreachable!()
        };
        let CborValue::Array(body) = &mut need[1] else {
            unreachable!()
        };
        body[3] = remote_classification_value(RemoteClassificationV1::ConfirmedApplied);
        assert!(matches!(
            EffectDispatchAttemptV1::from_persistence_carrier_value(&erased_uncertainty),
            Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
        ));

        let mut empty_run_set = prepared.persistence_carrier_value().unwrap();
        let CborValue::Array(fields) = &mut empty_run_set else {
            unreachable!()
        };
        let CborValue::Array(dispatch) = &mut fields[1] else {
            unreachable!()
        };
        let CborValue::Array(run_set) = &mut dispatch[10] else {
            unreachable!()
        };
        run_set[3] = CborValue::Unsigned(1);
        run_set[4] = CborValue::Array(vec![]);
        let CborValue::Array(run_binding) = &mut fields[3] else {
            unreachable!()
        };
        run_binding[1] = CborValue::Unsigned(1);
        run_binding[2] = CborValue::Array(vec![]);
        assert!(matches!(
            EffectDispatchAttemptV1::from_persistence_carrier_value(&empty_run_set),
            Err(EffectRuntimeErrorV1::Execution(
                ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier
            ))
        ));

        let mut foreign_owner = prepared.persistence_carrier_value().unwrap();
        let CborValue::Array(fields) = &mut foreign_owner else {
            unreachable!()
        };
        let CborValue::Array(dispatch) = &mut fields[1] else {
            unreachable!()
        };
        let CborValue::Array(run_set) = &mut dispatch[10] else {
            unreachable!()
        };
        run_set[1] = execution_owner_value(ExecutionAttemptOwnerV1::Reconciliation(
            ReconciliationAttemptIdV1::from_bytes(token(64)).unwrap(),
        ));
        assert!(matches!(
            EffectDispatchAttemptV1::from_persistence_carrier_value(&foreign_owner),
            Err(EffectRuntimeErrorV1::Execution(
                ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier
            ))
        ));
    }

    #[test]
    fn persisted_reconciliation_carrier_rejects_fence_owner_and_refinement_mutants() {
        let (intent, reserve, use_fence, control) = intent_and_control();
        let prepared = intent
            .prepare_dispatch(
                control.revision(),
                dispatch_preparation(use_fence),
                &reserve,
            )
            .unwrap();
        let reserve_transition = prepared
            .control_need()
            .control_transition(control.head(), control.revision(), control.writer_term())
            .unwrap();
        let (reserved_revision, reserved_head) = reserve_transition
            .apply(
                control.head(),
                control.revision(),
                control.writer_term(),
                None,
            )
            .unwrap();
        let seal_authority = authorized(ExecutionActionV1::RecordDispatchOutcome, "recon-seal");
        let seal = prepared
            .dispatch()
            .seal_candidate(&intent, &reserved_revision, token(81), &seal_authority)
            .unwrap();
        let seal_transition = seal
            .control_need()
            .control_transition(&reserved_head, &reserved_revision, control.writer_term())
            .unwrap();
        let (sealed_revision, sealed_head) = seal_transition
            .apply(
                &reserved_head,
                &reserved_revision,
                control.writer_term(),
                None,
            )
            .unwrap();
        let dispatch_terminal_authority = authorized(
            ExecutionActionV1::RecordDispatchOutcome,
            "recon-dispatch-terminal",
        );
        let dispatch_terminal = seal
            .dispatch()
            .terminal_candidate(
                &intent,
                &sealed_revision,
                EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                    evidence_commitment: token(82),
                },
                token(83),
                token(84),
                &dispatch_terminal_authority,
            )
            .unwrap();
        let dispatch_terminal_transition = dispatch_terminal
            .control_need()
            .control_transition(&sealed_head, &sealed_revision, control.writer_term())
            .unwrap();
        let dispatch_publication =
            EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                token(83),
                token(84),
            )
            .unwrap();
        let (dispatch_terminal_revision, dispatch_terminal_head) = dispatch_terminal_transition
            .apply(
                &sealed_head,
                &sealed_revision,
                control.writer_term(),
                Some(dispatch_publication),
            )
            .unwrap();

        let begin_authority = authorized(ExecutionActionV1::ReconcileEffectIntent, "recon-begin");
        let read_plan = reconciliation_plan();
        let begin = intent
            .prepare_reconciliation(
                &dispatch_terminal_revision,
                EffectReconciliationPreparationV1 {
                    use_fence: reconciliation_use_fence(&intent, &begin_authority, read_plan),
                    read_plan,
                    read_run: RunReservationV1 {
                        semantic_operation_hash: read_plan.commitment().unwrap(),
                        inputs_commitment: token(85),
                        environment_commitment: token(86),
                        target_commitment: token(87),
                        execution_boundary_commitment: token(88),
                        deadline: 200,
                        launch_ordinal: 1,
                        current_step_term: None,
                    },
                },
                &begin_authority,
                &[],
            )
            .unwrap();
        let begin_transition = begin
            .control_need()
            .control_transition(
                &dispatch_terminal_head,
                &dispatch_terminal_revision,
                control.writer_term(),
            )
            .unwrap();
        let (begin_revision, begin_head) = begin_transition
            .apply(
                &dispatch_terminal_head,
                &dispatch_terminal_revision,
                control.writer_term(),
                None,
            )
            .unwrap();
        let read = begin
            .reconciliation()
            .clone()
            .execute_read_candidate(
                &intent,
                &begin_revision,
                EffectReconciliationReadUsageV1 {
                    requests: 1,
                    pages: 1,
                    bytes: 256,
                    duration_ms: 100,
                    result_commitment: token(89),
                },
                token(90),
                token(91),
                &begin_authority,
            )
            .unwrap();
        let read_transition = read
            .control_need()
            .control_transition(&begin_head, &begin_revision, control.writer_term())
            .unwrap();
        let read_publication = EffectIntentControlPublicationCommitmentsV1::from_store_publication(
            token(90),
            token(91),
        )
        .unwrap();
        let (read_revision, read_head) = read_transition
            .apply(
                &begin_head,
                &begin_revision,
                control.writer_term(),
                Some(read_publication),
            )
            .unwrap();
        let terminal = read
            .attempt()
            .clone()
            .finish_candidate(
                &intent,
                &read_revision,
                EffectReconciliationOutcomeV1 {
                    classification: RemoteClassificationV1::ConfirmedApplied,
                    read_result_commitment: token(89),
                    result_commitment: token(92),
                    idempotency_commitment: token(93),
                },
                &begin_authority,
            )
            .unwrap();
        terminal
            .control_need()
            .control_transition(&read_head, &read_revision, control.writer_term())
            .unwrap();

        for value in [
            begin.persistence_carrier_value().unwrap(),
            read.persistence_carrier_value().unwrap(),
            terminal.persistence_carrier_value().unwrap(),
        ] {
            EffectReconciliationAttemptV1::from_persistence_carrier_value(&value).unwrap();
        }

        let mut rewritten_fence = begin.persistence_carrier_value().unwrap();
        let CborValue::Array(carrier) = &mut rewritten_fence else {
            unreachable!()
        };
        let CborValue::Array(attempt) = &mut carrier[1] else {
            unreachable!()
        };
        let CborValue::Array(fence) = &mut attempt[4] else {
            unreachable!()
        };
        fence[2] = bytes(&token(94));
        assert!(matches!(
            EffectReconciliationAttemptV1::from_persistence_carrier_value(&rewritten_fence),
            Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
        ));

        let mut wrong_owner = read.persistence_carrier_value().unwrap();
        let CborValue::Array(carrier) = &mut wrong_owner else {
            unreachable!()
        };
        let CborValue::Array(run_binding) = &mut carrier[3] else {
            unreachable!()
        };
        let CborValue::Array(owner) = &mut run_binding[0] else {
            unreachable!()
        };
        owner[0] = CborValue::Unsigned(2);
        assert!(matches!(
            EffectReconciliationAttemptV1::from_persistence_carrier_value(&wrong_owner),
            Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
        ));

        let mut illegal_refinement = terminal.persistence_carrier_value().unwrap();
        let CborValue::Array(carrier) = &mut illegal_refinement else {
            unreachable!()
        };
        let CborValue::Array(need) = &mut carrier[2] else {
            unreachable!()
        };
        let CborValue::Array(body) = &mut need[1] else {
            unreachable!()
        };
        body[3] = remote_classification_value(RemoteClassificationV1::Cancelled);
        assert!(matches!(
            EffectReconciliationAttemptV1::from_persistence_carrier_value(&illegal_refinement),
            Err(EffectRuntimeErrorV1::InvalidStoredEffectCarrier)
        ));
    }

    #[test]
    fn reconciliation_is_one_bounded_effect_free_read_with_fresh_requests() {
        let plan_parts = EffectReconciliationReadPlanPartsV1 {
            classification: ReconciliationReadOperationClassificationV1::EffectFreeRead,
            operation_kind: ReconciliationReadOperationKindV1::ProviderStatus,
            provider_commitment: token(50),
            account_commitment: token(51),
            target_commitment: token(52),
            correlation_commitment: token(53),
            credential_commitment: token(54),
            visibility_commitment: token(55),
            query_commitment: token(56),
            evaluator_commitment: token(57),
            max_requests: 1,
            max_pages: 1,
            max_bytes: 1024,
            max_duration_ms: 500,
        };
        let plan = EffectReconciliationReadPlanV1::new(plan_parts).unwrap();
        assert_eq!(
            plan.classification(),
            ReconciliationReadOperationClassificationV1::EffectFreeRead
        );
        let valid_usage = EffectReconciliationReadUsageV1 {
            requests: 1,
            pages: 1,
            bytes: 1024,
            duration_ms: 500,
            result_commitment: token(58),
        };
        validate_reconciliation_read_usage(plan, valid_usage).unwrap();
        for invalid in [
            EffectReconciliationReadUsageV1 {
                requests: 0,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                requests: 2,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                pages: 0,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                pages: 2,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                bytes: 0,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                bytes: 1025,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                duration_ms: 0,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                duration_ms: 501,
                ..valid_usage
            },
            EffectReconciliationReadUsageV1 {
                result_commitment: [0; 32],
                ..valid_usage
            },
        ] {
            assert!(matches!(
                validate_reconciliation_read_usage(plan, invalid),
                Err(EffectRuntimeErrorV1::ReconciliationReadBoundsExceeded
                    | EffectRuntimeErrorV1::MissingCommitment)
            ));
        }
        for invalid in [
            EffectReconciliationReadPlanPartsV1 {
                max_requests: 0,
                ..plan_parts
            },
            EffectReconciliationReadPlanPartsV1 {
                max_pages: 0,
                ..plan_parts
            },
            EffectReconciliationReadPlanPartsV1 {
                max_bytes: 0,
                ..plan_parts
            },
            EffectReconciliationReadPlanPartsV1 {
                max_duration_ms: 0,
                ..plan_parts
            },
        ] {
            assert!(matches!(
                EffectReconciliationReadPlanV1::new(invalid),
                Err(EffectRuntimeErrorV1::InvalidReconciliationReadBounds)
            ));
        }
        assert!(matches!(
            EffectReconciliationReadPlanV1::new(EffectReconciliationReadPlanPartsV1 {
                classification: ReconciliationReadOperationClassificationV1::EffectingRead,
                ..plan_parts
            }),
            Err(EffectRuntimeErrorV1::EffectingReconciliationRead)
        ));
    }
}
