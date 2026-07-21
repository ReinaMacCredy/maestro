use thiserror::Error;

use super::{ActionAuthorityBasisKindV1, RepositoryDownstreamActionLeafV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityActionLeafV1 {
    AllocateGovernedCapacitySlot,
    EstablishConsumptionCellRoot,
    IssueRootAttachedBoundedGrant,
    ReissueRootAttachedGrantOneToOne,
    RevokeGrant,
}

const ACTION_SPEC_PROTOCOL_REVISION_V1: u64 = 1;
const ACTION_SPEC_MANIFEST_ID_V1: &str =
    "7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970";
const ACTION_SPEC_GRAMMAR_ID_V1: &str =
    "b7ef635dcd29af4fc41f20cd670b726e5627c2f7210344d058e7c188ace69647";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryActionLeafV1 {
    CreateDraftWork,
    CancelWork,
    CompleteWork,
    AbsorbWork,
    SubmitWorkCompletion,
    RejectWorkCompletion,
    ReturnWorkForRepair,
    SubmitStep,
    SatisfyStep,
    RejectStepSubmission,
    RecoverStepSubmission,
    PublishInitialContract,
    AmendContract,
    AppendDesignRevision,
    ResolveDecision,
    AcquireStepExecution,
    RenewStepLeaseTerm,
    AbandonStepAttempt,
    OriginateEffectIntent,
    OriginateCoordinationDelivery,
    RecordDispatchOutcome,
    ReconcileEffectIntent,
    ReserveBootstrapMandateInteractionEffect,
    PublishBootstrapMandateInteractionOutcome,
    ReconcileBootstrapMandateInteractionEffect,
    ReserveContinuityMaintenanceEffect,
    PublishContinuityMaintenanceEffectOutcome,
    ReconcileContinuityMaintenanceEffect,
    WithdrawEffectIntent,
    WithdrawBootstrapMandateInteractionEffect,
    WithdrawContinuityMaintenanceEffect,
    PublishObservation,
    PublishAssessment,
    InvalidateAssessment,
    SecurityEraseEvidencePayload,
    PublishBootstrapMandatePresentationObservation,
    PublishBootstrapMandateResponseObservation,
    PublishContinuityMaintenanceObservation,
    Downstream(RepositoryDownstreamActionLeafV1),
}

impl RepositoryActionLeafV1 {
    pub const ALL: [Self; 38] = [
        Self::CreateDraftWork,
        Self::CancelWork,
        Self::CompleteWork,
        Self::AbsorbWork,
        Self::SubmitWorkCompletion,
        Self::RejectWorkCompletion,
        Self::ReturnWorkForRepair,
        Self::SubmitStep,
        Self::SatisfyStep,
        Self::RejectStepSubmission,
        Self::RecoverStepSubmission,
        Self::PublishInitialContract,
        Self::AmendContract,
        Self::AppendDesignRevision,
        Self::ResolveDecision,
        Self::AcquireStepExecution,
        Self::RenewStepLeaseTerm,
        Self::AbandonStepAttempt,
        Self::OriginateEffectIntent,
        Self::OriginateCoordinationDelivery,
        Self::RecordDispatchOutcome,
        Self::ReconcileEffectIntent,
        Self::ReserveBootstrapMandateInteractionEffect,
        Self::PublishBootstrapMandateInteractionOutcome,
        Self::ReconcileBootstrapMandateInteractionEffect,
        Self::ReserveContinuityMaintenanceEffect,
        Self::PublishContinuityMaintenanceEffectOutcome,
        Self::ReconcileContinuityMaintenanceEffect,
        Self::WithdrawEffectIntent,
        Self::WithdrawBootstrapMandateInteractionEffect,
        Self::WithdrawContinuityMaintenanceEffect,
        Self::PublishObservation,
        Self::PublishAssessment,
        Self::InvalidateAssessment,
        Self::SecurityEraseEvidencePayload,
        Self::PublishBootstrapMandatePresentationObservation,
        Self::PublishBootstrapMandateResponseObservation,
        Self::PublishContinuityMaintenanceObservation,
    ];

    pub const fn literal(self) -> &'static str {
        match self {
            Self::CreateDraftWork => "CreateDraftWork",
            Self::CancelWork => "CancelWork",
            Self::CompleteWork => "CompleteWork",
            Self::AbsorbWork => "AbsorbWork",
            Self::SubmitWorkCompletion => "SubmitWorkCompletion",
            Self::RejectWorkCompletion => "RejectWorkCompletion",
            Self::ReturnWorkForRepair => "ReturnWorkForRepair",
            Self::SubmitStep => "SubmitStep",
            Self::SatisfyStep => "SatisfyStep",
            Self::RejectStepSubmission => "RejectStepSubmission",
            Self::RecoverStepSubmission => "RecoverStepSubmission",
            Self::PublishInitialContract => "PublishInitialContract",
            Self::AmendContract => "AmendContract",
            Self::AppendDesignRevision => "AppendDesignRevision",
            Self::ResolveDecision => "ResolveDecision",
            Self::AcquireStepExecution => "AcquireStepExecution",
            Self::RenewStepLeaseTerm => "RenewStepLeaseTerm",
            Self::AbandonStepAttempt => "AbandonStepAttempt",
            Self::OriginateEffectIntent => "OriginateEffectIntent",
            Self::OriginateCoordinationDelivery => "OriginateCoordinationDelivery",
            Self::RecordDispatchOutcome => "RecordDispatchOutcome",
            Self::ReconcileEffectIntent => "ReconcileEffectIntent",
            Self::ReserveBootstrapMandateInteractionEffect => {
                "ReserveBootstrapMandateInteractionEffect"
            }
            Self::PublishBootstrapMandateInteractionOutcome => {
                "PublishBootstrapMandateInteractionOutcome"
            }
            Self::ReconcileBootstrapMandateInteractionEffect => {
                "ReconcileBootstrapMandateInteractionEffect"
            }
            Self::ReserveContinuityMaintenanceEffect => "ReserveContinuityMaintenanceEffect",
            Self::PublishContinuityMaintenanceEffectOutcome => {
                "PublishContinuityMaintenanceEffectOutcome"
            }
            Self::ReconcileContinuityMaintenanceEffect => "ReconcileContinuityMaintenanceEffect",
            Self::WithdrawEffectIntent => "WithdrawEffectIntent",
            Self::WithdrawBootstrapMandateInteractionEffect => {
                "WithdrawBootstrapMandateInteractionEffect"
            }
            Self::WithdrawContinuityMaintenanceEffect => "WithdrawContinuityMaintenanceEffect",
            Self::PublishObservation => "PublishObservation",
            Self::PublishAssessment => "PublishAssessment",
            Self::InvalidateAssessment => "InvalidateAssessment",
            Self::SecurityEraseEvidencePayload => "SecurityEraseEvidencePayload",
            Self::PublishBootstrapMandatePresentationObservation => {
                "PublishBootstrapMandatePresentationObservation"
            }
            Self::PublishBootstrapMandateResponseObservation => {
                "PublishBootstrapMandateResponseObservation"
            }
            Self::PublishContinuityMaintenanceObservation => {
                "PublishContinuityMaintenanceObservation"
            }
            Self::Downstream(action) => action.literal(),
        }
    }

    pub const fn global_tag(self) -> u64 {
        match self {
            Self::CreateDraftWork => 1,
            Self::CancelWork => 2,
            Self::CompleteWork => 3,
            Self::AbsorbWork => 4,
            Self::SubmitWorkCompletion => 5,
            Self::RejectWorkCompletion => 6,
            Self::ReturnWorkForRepair => 7,
            Self::SubmitStep => 8,
            Self::SatisfyStep => 9,
            Self::RejectStepSubmission => 10,
            Self::RecoverStepSubmission => 11,
            Self::PublishInitialContract => 12,
            Self::AmendContract => 13,
            Self::AppendDesignRevision => 15,
            Self::ResolveDecision => 20,
            Self::AcquireStepExecution => 23,
            Self::RenewStepLeaseTerm => 24,
            Self::AbandonStepAttempt => 25,
            Self::OriginateEffectIntent => 26,
            Self::OriginateCoordinationDelivery => 27,
            Self::RecordDispatchOutcome => 28,
            Self::ReconcileEffectIntent => 29,
            Self::ReserveBootstrapMandateInteractionEffect => 30,
            Self::PublishBootstrapMandateInteractionOutcome => 31,
            Self::ReconcileBootstrapMandateInteractionEffect => 32,
            Self::ReserveContinuityMaintenanceEffect => 33,
            Self::PublishContinuityMaintenanceEffectOutcome => 34,
            Self::ReconcileContinuityMaintenanceEffect => 35,
            Self::WithdrawEffectIntent => 36,
            Self::WithdrawBootstrapMandateInteractionEffect => 37,
            Self::WithdrawContinuityMaintenanceEffect => 38,
            Self::PublishObservation => 39,
            Self::PublishAssessment => 40,
            Self::InvalidateAssessment => 41,
            Self::SecurityEraseEvidencePayload => 42,
            Self::PublishBootstrapMandatePresentationObservation => 43,
            Self::PublishBootstrapMandateResponseObservation => 44,
            Self::PublishContinuityMaintenanceObservation => 45,
            Self::Downstream(action) => action.global_tag(),
        }
    }

    pub const fn owner_tag(self) -> u64 {
        match self {
            Self::CreateDraftWork
            | Self::CancelWork
            | Self::CompleteWork
            | Self::AbsorbWork
            | Self::SubmitWorkCompletion
            | Self::RejectWorkCompletion
            | Self::ReturnWorkForRepair => 1,
            Self::SubmitStep
            | Self::SatisfyStep
            | Self::RejectStepSubmission
            | Self::RecoverStepSubmission => 2,
            Self::PublishInitialContract | Self::AmendContract => 3,
            Self::AppendDesignRevision => 4,
            Self::ResolveDecision => 5,
            Self::AcquireStepExecution
            | Self::RenewStepLeaseTerm
            | Self::AbandonStepAttempt
            | Self::OriginateEffectIntent
            | Self::OriginateCoordinationDelivery
            | Self::RecordDispatchOutcome
            | Self::ReconcileEffectIntent
            | Self::ReserveBootstrapMandateInteractionEffect
            | Self::PublishBootstrapMandateInteractionOutcome
            | Self::ReconcileBootstrapMandateInteractionEffect
            | Self::ReserveContinuityMaintenanceEffect
            | Self::PublishContinuityMaintenanceEffectOutcome
            | Self::ReconcileContinuityMaintenanceEffect
            | Self::WithdrawEffectIntent
            | Self::WithdrawBootstrapMandateInteractionEffect
            | Self::WithdrawContinuityMaintenanceEffect => 6,
            Self::PublishObservation
            | Self::PublishAssessment
            | Self::InvalidateAssessment
            | Self::SecurityEraseEvidencePayload
            | Self::PublishBootstrapMandatePresentationObservation
            | Self::PublishBootstrapMandateResponseObservation
            | Self::PublishContinuityMaintenanceObservation => 7,
            Self::Downstream(action) => action.owner_tag(),
        }
    }

    pub const fn local_tag(self) -> u64 {
        match self {
            Self::CreateDraftWork | Self::SubmitStep | Self::PublishInitialContract => 1,
            Self::CancelWork
            | Self::SatisfyStep
            | Self::AmendContract
            | Self::AppendDesignRevision => 2,
            Self::CompleteWork | Self::RejectStepSubmission | Self::ResolveDecision => 3,
            Self::AbsorbWork | Self::RecoverStepSubmission => 4,
            Self::SubmitWorkCompletion => 5,
            Self::RejectWorkCompletion => 6,
            Self::ReturnWorkForRepair => 7,
            Self::AcquireStepExecution => 1,
            Self::RenewStepLeaseTerm => 2,
            Self::AbandonStepAttempt => 3,
            Self::OriginateEffectIntent => 4,
            Self::OriginateCoordinationDelivery => 5,
            Self::RecordDispatchOutcome => 6,
            Self::ReconcileEffectIntent => 7,
            Self::ReserveBootstrapMandateInteractionEffect => 8,
            Self::PublishBootstrapMandateInteractionOutcome => 9,
            Self::ReconcileBootstrapMandateInteractionEffect => 10,
            Self::ReserveContinuityMaintenanceEffect => 11,
            Self::PublishContinuityMaintenanceEffectOutcome => 12,
            Self::ReconcileContinuityMaintenanceEffect => 13,
            Self::WithdrawEffectIntent => 14,
            Self::WithdrawBootstrapMandateInteractionEffect => 15,
            Self::WithdrawContinuityMaintenanceEffect => 16,
            Self::PublishObservation => 1,
            Self::PublishAssessment => 2,
            Self::InvalidateAssessment => 3,
            Self::SecurityEraseEvidencePayload => 4,
            Self::PublishBootstrapMandatePresentationObservation => 5,
            Self::PublishBootstrapMandateResponseObservation => 6,
            Self::PublishContinuityMaintenanceObservation => 7,
            Self::Downstream(action) => action.local_tag(),
        }
    }

    pub const fn owner_descriptor_id(self) -> &'static str {
        match self {
            Self::CreateDraftWork
            | Self::CancelWork
            | Self::CompleteWork
            | Self::AbsorbWork
            | Self::SubmitWorkCompletion
            | Self::RejectWorkCompletion
            | Self::ReturnWorkForRepair => {
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee"
            }
            Self::SubmitStep
            | Self::SatisfyStep
            | Self::RejectStepSubmission
            | Self::RecoverStepSubmission => {
                "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e"
            }
            Self::PublishInitialContract | Self::AmendContract => {
                "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab"
            }
            Self::AppendDesignRevision => {
                "85aad446bae62f47851f719f74296bd2576f30894b95ccb4b3b0c59790a80dc5"
            }
            Self::ResolveDecision => {
                "a3d6c9c0dcd9b5e3447cf4dc45edf5d1b338c99dfc27a61df23966b7514ae9dc"
            }
            Self::AcquireStepExecution
            | Self::RenewStepLeaseTerm
            | Self::AbandonStepAttempt
            | Self::OriginateEffectIntent
            | Self::OriginateCoordinationDelivery
            | Self::RecordDispatchOutcome
            | Self::ReconcileEffectIntent
            | Self::ReserveBootstrapMandateInteractionEffect
            | Self::PublishBootstrapMandateInteractionOutcome
            | Self::ReconcileBootstrapMandateInteractionEffect
            | Self::ReserveContinuityMaintenanceEffect
            | Self::PublishContinuityMaintenanceEffectOutcome
            | Self::ReconcileContinuityMaintenanceEffect
            | Self::WithdrawEffectIntent
            | Self::WithdrawBootstrapMandateInteractionEffect
            | Self::WithdrawContinuityMaintenanceEffect => {
                "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292"
            }
            Self::PublishObservation
            | Self::PublishAssessment
            | Self::InvalidateAssessment
            | Self::SecurityEraseEvidencePayload
            | Self::PublishBootstrapMandatePresentationObservation
            | Self::PublishBootstrapMandateResponseObservation
            | Self::PublishContinuityMaintenanceObservation => {
                "56d3f71ffc62ecc71973ac2a51a076ae62f5686806737fdba9e6fa6051999bc9"
            }
            Self::Downstream(action) => action.owner_descriptor_id(),
        }
    }

    pub const fn descriptor_id(self) -> &'static str {
        match self {
            Self::CreateDraftWork => {
                "56ded201d62fbb94486581d13cc6a086b3e114ad889aa1a954841f7f646afc40"
            }
            Self::CancelWork => "b58d2fecb0f1b27146884f85847cb1b22575b32d8d6e92efe6608cf582420615",
            Self::CompleteWork => {
                "163de9814514910c9ca1d5b1f76ac982e0788bd8a81025eff5c551a0a923b5d2"
            }
            Self::AbsorbWork => "4fb2d35f4bd7c2169bec6bc51af840f325c419c28de0041d60b69bc4691125ea",
            Self::SubmitWorkCompletion => {
                "7d8083d10f75348f805e89e8fcd5f27d81face12d2d8ae1047a01c53fbfb2803"
            }
            Self::RejectWorkCompletion => {
                "d03d0a753eb0821b43f002de3eab1afb32a7ede75ff0a3588b0718292c0d7b3f"
            }
            Self::ReturnWorkForRepair => {
                "2fbc3d51f0b750cb9a1292d404ada520c17fac6c9fd960350188d97f3b6acc0b"
            }
            Self::SubmitStep => "c5a7079af2dafa9acc956477b3004b5fb21dd688b4022677416164c4485f96d6",
            Self::SatisfyStep => "a5887f9b87af5f6a5f466df222b3a76b2eca5762b33a5694b4ebcb61b3db127e",
            Self::RejectStepSubmission => {
                "b75460c72c4907e2893bb48559b2c19d99ecb4d27ab43a10adda4ee95dfbc62a"
            }
            Self::RecoverStepSubmission => {
                "130cb3ecfd8146ba869de9a1198b3c3b6b67b2c06101cecf62a955db5b587e13"
            }
            Self::PublishInitialContract => {
                "5c3bf7e45cc2e8348bb5a6ce403cf6d14f718c7ef4514ca01e60370174387234"
            }
            Self::AmendContract => {
                "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4"
            }
            Self::AppendDesignRevision => {
                "4235a07743d0fa3557f612b0d4dd499afcd02adadcc92facfcbc806645a99e83"
            }
            Self::ResolveDecision => {
                "4e05d1c7d9314a843d43538c399ece7df7da52793062c7ca43805e8f763f75ac"
            }
            Self::AcquireStepExecution => {
                "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2"
            }
            Self::RenewStepLeaseTerm => {
                "24abfd7630a5d743f5793319ade1ad3b6017a1a31cca632be4ba2a68fb4edf0b"
            }
            Self::AbandonStepAttempt => {
                "4ef55b490996ea62eedd4ef62a58db9f17a62d2b040ea4243a4b331eb04953da"
            }
            Self::OriginateEffectIntent => {
                "a2cf705f5d7ba987ae47efd9a8f9a8033e794b9858aa33f3812e4433c1350e26"
            }
            Self::OriginateCoordinationDelivery => {
                "f4d4592e5de4084bcbb1b28487919011aae9a7f3f5f60e2bb3751900d3c26700"
            }
            Self::RecordDispatchOutcome => {
                "568be4ebcfcf121a7d0c7b6aa956dbd281bd17a20c414bf07656923d30cc69d3"
            }
            Self::ReconcileEffectIntent => {
                "5d0b53b85e408badf53e310ca7c619ae0c3a0e3113be26d94dfafe5bf6d2a745"
            }
            Self::ReserveBootstrapMandateInteractionEffect => {
                "b4af8370f69b1aa4c8d93964b34e5e952c4a1e2a764d5f944680527c0430d782"
            }
            Self::PublishBootstrapMandateInteractionOutcome => {
                "946ecf3e7c06a8fd5104f23776a8e19cfd3ad6b3325a76abf949c4a08e0ab0d0"
            }
            Self::ReconcileBootstrapMandateInteractionEffect => {
                "7e885d84c662dbbf928c74d8607975719c4e2851ca475f26855fb4e91ea15d36"
            }
            Self::ReserveContinuityMaintenanceEffect => {
                "4d67ac86d16c81fc135effaa27662f74b373054aee804e9b3b6ae8ba26323bb2"
            }
            Self::PublishContinuityMaintenanceEffectOutcome => {
                "e9197d302312aaf18f576aed38358c5e39854e12a297dac37fcab3e1f53c8460"
            }
            Self::ReconcileContinuityMaintenanceEffect => {
                "2dc583f57f23f12026cef748bb61c0db72150d3ed31f4e6e31be77f8f63e1fa1"
            }
            Self::WithdrawEffectIntent => {
                "6df3b90a4963ffef04865a9e70b57f2040b2b2159b35cfb58347866ed6afe2f9"
            }
            Self::WithdrawBootstrapMandateInteractionEffect => {
                "00ae32e979c74e12f6ebc2f31da11890043495cb8042708edd3f5063c72f2a29"
            }
            Self::WithdrawContinuityMaintenanceEffect => {
                "d5ed8273857101d805748d83023ad067c909427223ae81ba4f9a77f770227d47"
            }
            Self::PublishObservation => {
                "f0432a48ae75c4c05a4e3c591ad3ac4fa754b495ec8707fb9aa5520681ebd3db"
            }
            Self::PublishAssessment => {
                "7310e5de3920e05dea886bc591814788d700a068502638344af0ab3c56143c39"
            }
            Self::InvalidateAssessment => {
                "bcdc52fecd8eda30931c04beb0b9cdd4ac57897f6e93bd3a49f16458bedb9fc3"
            }
            Self::SecurityEraseEvidencePayload => {
                "b9badfa33a6cb6501f24840beff07689de844938c02b0c8e2707ab399273be1f"
            }
            Self::PublishBootstrapMandatePresentationObservation => {
                "2e8cf3c7590acd29384c5590598f2d2906eaee0aa1ce061999703e6931a5996a"
            }
            Self::PublishBootstrapMandateResponseObservation => {
                "54043608c99a0277883bb095831e2f45d29a898e284987e94891c59281a94b6d"
            }
            Self::PublishContinuityMaintenanceObservation => {
                "46ec801444dabb9aabb291ea2960434c1542954db1d3dc70beec2734177c8892"
            }
            Self::Downstream(action) => action.descriptor_id(),
        }
    }

    pub const fn is_evidence_action(self) -> bool {
        matches!(
            self,
            Self::PublishObservation
                | Self::PublishAssessment
                | Self::InvalidateAssessment
                | Self::SecurityEraseEvidencePayload
                | Self::PublishBootstrapMandatePresentationObservation
                | Self::PublishBootstrapMandateResponseObservation
                | Self::PublishContinuityMaintenanceObservation
        )
    }

    pub const fn is_execution_action(self) -> bool {
        matches!(
            self,
            Self::AcquireStepExecution
                | Self::RenewStepLeaseTerm
                | Self::AbandonStepAttempt
                | Self::OriginateEffectIntent
                | Self::OriginateCoordinationDelivery
                | Self::RecordDispatchOutcome
                | Self::ReconcileEffectIntent
                | Self::ReserveBootstrapMandateInteractionEffect
                | Self::PublishBootstrapMandateInteractionOutcome
                | Self::ReconcileBootstrapMandateInteractionEffect
                | Self::ReserveContinuityMaintenanceEffect
                | Self::PublishContinuityMaintenanceEffectOutcome
                | Self::ReconcileContinuityMaintenanceEffect
                | Self::WithdrawEffectIntent
                | Self::WithdrawBootstrapMandateInteractionEffect
                | Self::WithdrawContinuityMaintenanceEffect
        )
    }

    pub const fn execution_authority_basis(self) -> Option<ActionAuthorityBasisKindV1> {
        Some(match self {
            Self::Downstream(_) => ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            Self::ReserveBootstrapMandateInteractionEffect
            | Self::PublishBootstrapMandateInteractionOutcome
            | Self::ReconcileBootstrapMandateInteractionEffect
            | Self::WithdrawBootstrapMandateInteractionEffect => {
                ActionAuthorityBasisKindV1::BootstrapControlG0
            }
            Self::ReserveContinuityMaintenanceEffect
            | Self::PublishContinuityMaintenanceEffectOutcome
            | Self::ReconcileContinuityMaintenanceEffect
            | Self::WithdrawContinuityMaintenanceEffect
            | Self::PublishContinuityMaintenanceObservation => {
                ActionAuthorityBasisKindV1::ContinuityMaintenance
            }
            Self::PublishBootstrapMandatePresentationObservation
            | Self::PublishBootstrapMandateResponseObservation => {
                ActionAuthorityBasisKindV1::BootstrapControlG0
            }
            _ if self.is_execution_action() || self.is_evidence_action() => {
                ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
            }
            _ => return None,
        })
    }

    pub const fn is_ordinary_execution_action(self) -> bool {
        self.is_execution_action()
            && matches!(
                self.execution_authority_basis(),
                Some(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime)
            )
    }

    pub const fn is_external_effect_action(self) -> bool {
        matches!(
            self,
            Self::OriginateEffectIntent
                | Self::OriginateCoordinationDelivery
                | Self::RecordDispatchOutcome
                | Self::ReconcileEffectIntent
                | Self::ReserveBootstrapMandateInteractionEffect
                | Self::PublishBootstrapMandateInteractionOutcome
                | Self::ReconcileBootstrapMandateInteractionEffect
                | Self::ReserveContinuityMaintenanceEffect
                | Self::PublishContinuityMaintenanceEffectOutcome
                | Self::ReconcileContinuityMaintenanceEffect
                | Self::WithdrawEffectIntent
                | Self::WithdrawBootstrapMandateInteractionEffect
                | Self::WithdrawContinuityMaintenanceEffect
        )
    }

    pub const fn protocol_revision(self) -> u64 {
        ACTION_SPEC_PROTOCOL_REVISION_V1
    }

    pub const fn manifest_id(self) -> &'static str {
        ACTION_SPEC_MANIFEST_ID_V1
    }

    pub const fn grammar_id(self) -> &'static str {
        ACTION_SPEC_GRAMMAR_ID_V1
    }
}

impl AuthorityActionLeafV1 {
    pub const ALL: [Self; 5] = [
        Self::AllocateGovernedCapacitySlot,
        Self::EstablishConsumptionCellRoot,
        Self::IssueRootAttachedBoundedGrant,
        Self::ReissueRootAttachedGrantOneToOne,
        Self::RevokeGrant,
    ];

    pub const fn literal(self) -> &'static str {
        match self {
            Self::AllocateGovernedCapacitySlot => "AllocateGovernedCapacitySlot",
            Self::EstablishConsumptionCellRoot => "EstablishConsumptionCellRoot",
            Self::IssueRootAttachedBoundedGrant => "IssueRootAttachedBoundedGrant",
            Self::ReissueRootAttachedGrantOneToOne => "ReissueRootAttachedGrantOneToOne",
            Self::RevokeGrant => "RevokeGrant",
        }
    }

    pub const fn authority_basis(self) -> ActionAuthorityBasisKindV1 {
        match self {
            Self::AllocateGovernedCapacitySlot
            | Self::EstablishConsumptionCellRoot
            | Self::IssueRootAttachedBoundedGrant => ActionAuthorityBasisKindV1::BootstrapControlG0,
            Self::ReissueRootAttachedGrantOneToOne | Self::RevokeGrant => {
                ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
            }
        }
    }

    pub fn parse_exact(literal: &str) -> Result<Self, AuthorityActionBasisErrorV1> {
        Self::ALL
            .into_iter()
            .find(|leaf| leaf.literal() == literal)
            .ok_or_else(|| AuthorityActionBasisErrorV1::UnknownActionLeaf(literal.to_owned()))
    }
}

pub fn exact_authority_basis_for_action(
    literal: &str,
) -> Result<ActionAuthorityBasisKindV1, AuthorityActionBasisErrorV1> {
    Ok(AuthorityActionLeafV1::parse_exact(literal)?.authority_basis())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityActionBasisErrorV1 {
    #[error("Authority Action leaf is unknown or unclassified: {0}")]
    UnknownActionLeaf(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_registry_has_no_cma_or_name_inference() {
        for leaf in AuthorityActionLeafV1::ALL {
            assert_ne!(
                leaf.authority_basis(),
                ActionAuthorityBasisKindV1::ContinuityMaintenance
            );
        }
        assert_eq!(
            exact_authority_basis_for_action("IssueRootAttachedBoundedGrant").unwrap(),
            ActionAuthorityBasisKindV1::BootstrapControlG0
        );
        assert_eq!(
            exact_authority_basis_for_action("AllocateGovernedCapacitySlot").unwrap(),
            ActionAuthorityBasisKindV1::BootstrapControlG0
        );
        assert_eq!(
            exact_authority_basis_for_action("EstablishConsumptionCellRoot").unwrap(),
            ActionAuthorityBasisKindV1::BootstrapControlG0
        );
        assert_eq!(
            exact_authority_basis_for_action("ReissueRootAttachedGrantOneToOne").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
        );
        assert_eq!(
            exact_authority_basis_for_action("RevokeGrant").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
        );
        assert!(exact_authority_basis_for_action("AdminRevokeGrant").is_err());
        assert!(exact_authority_basis_for_action("IssueRootAttachedBoundedGrantV2").is_err());
    }

    #[test]
    fn repository_stage_three_leaves_are_exact_frozen_catalog_members() {
        let rows = RepositoryActionLeafV1::ALL.map(|leaf| {
            (
                leaf.global_tag(),
                leaf.literal(),
                leaf.owner_tag(),
                leaf.local_tag(),
                leaf.owner_descriptor_id(),
                leaf.descriptor_id(),
                leaf.protocol_revision(),
                leaf.manifest_id(),
                leaf.grammar_id(),
            )
        });
        assert_eq!(rows.len(), 38);
        assert_eq!(rows[1].0, 2);
        assert_eq!(rows[1].1, "CancelWork");
        assert_eq!(
            rows[12].5,
            "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4"
        );
        assert_eq!(rows[14].0, 20);
        assert_eq!(rows[31].0, 39);
        assert_eq!(rows[31].1, "PublishObservation");
        assert_eq!(rows[32].1, "PublishAssessment");
        assert_eq!(rows[33].1, "InvalidateAssessment");
        assert_eq!(rows[34].1, "SecurityEraseEvidencePayload");
        assert_eq!(rows[35].0, 43);
        assert_eq!(rows[36].0, 44);
        assert_eq!(rows[37].0, 45);
        assert!(rows.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn downstream_leaves_preserve_ordinary_basis_without_becoming_execution_actions() {
        for action in RepositoryDownstreamActionLeafV1::all() {
            let repository_action = RepositoryActionLeafV1::Downstream(action);
            assert_eq!(
                repository_action.execution_authority_basis(),
                Some(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime)
            );
            assert!(!repository_action.is_execution_action());
            assert!(!repository_action.is_ordinary_execution_action());
            assert!(!repository_action.is_evidence_action());
            assert!(!repository_action.is_external_effect_action());
        }
    }

    #[test]
    fn protected_diagnostic_read_is_not_an_action_leaf() {
        assert!(RepositoryDownstreamActionLeafV1::parse_exact("ReadRepository").is_err());
        assert!(
            RepositoryDownstreamActionLeafV1::parse_exact("ProtectedContinuityDiagnosticRead")
                .is_err()
        );
        assert!(AuthorityActionLeafV1::parse_exact("ReadRepository").is_err());
    }

    #[test]
    fn repository_execution_leaves_match_the_exact_frozen_catalog() {
        let execution = RepositoryActionLeafV1::ALL
            .into_iter()
            .filter(|leaf| leaf.is_execution_action())
            .collect::<Vec<_>>();
        assert_eq!(execution.len(), 16);
        assert_eq!(
            execution
                .iter()
                .map(|leaf| leaf.global_tag())
                .collect::<Vec<_>>(),
            (23..=38).collect::<Vec<_>>()
        );
        assert_eq!(
            execution
                .iter()
                .map(|leaf| leaf.local_tag())
                .collect::<Vec<_>>(),
            (1..=16).collect::<Vec<_>>()
        );
        assert!(execution.iter().all(|leaf| leaf.owner_tag() == 6));
        assert!(execution.iter().all(|leaf| {
            leaf.owner_descriptor_id()
                == "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292"
        }));
        assert_eq!(
            execution
                .iter()
                .map(|leaf| (leaf.literal(), leaf.descriptor_id()))
                .collect::<Vec<_>>(),
            vec![
                (
                    "AcquireStepExecution",
                    "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2",
                ),
                (
                    "RenewStepLeaseTerm",
                    "24abfd7630a5d743f5793319ade1ad3b6017a1a31cca632be4ba2a68fb4edf0b",
                ),
                (
                    "AbandonStepAttempt",
                    "4ef55b490996ea62eedd4ef62a58db9f17a62d2b040ea4243a4b331eb04953da",
                ),
                (
                    "OriginateEffectIntent",
                    "a2cf705f5d7ba987ae47efd9a8f9a8033e794b9858aa33f3812e4433c1350e26",
                ),
                (
                    "OriginateCoordinationDelivery",
                    "f4d4592e5de4084bcbb1b28487919011aae9a7f3f5f60e2bb3751900d3c26700",
                ),
                (
                    "RecordDispatchOutcome",
                    "568be4ebcfcf121a7d0c7b6aa956dbd281bd17a20c414bf07656923d30cc69d3",
                ),
                (
                    "ReconcileEffectIntent",
                    "5d0b53b85e408badf53e310ca7c619ae0c3a0e3113be26d94dfafe5bf6d2a745",
                ),
                (
                    "ReserveBootstrapMandateInteractionEffect",
                    "b4af8370f69b1aa4c8d93964b34e5e952c4a1e2a764d5f944680527c0430d782",
                ),
                (
                    "PublishBootstrapMandateInteractionOutcome",
                    "946ecf3e7c06a8fd5104f23776a8e19cfd3ad6b3325a76abf949c4a08e0ab0d0",
                ),
                (
                    "ReconcileBootstrapMandateInteractionEffect",
                    "7e885d84c662dbbf928c74d8607975719c4e2851ca475f26855fb4e91ea15d36",
                ),
                (
                    "ReserveContinuityMaintenanceEffect",
                    "4d67ac86d16c81fc135effaa27662f74b373054aee804e9b3b6ae8ba26323bb2",
                ),
                (
                    "PublishContinuityMaintenanceEffectOutcome",
                    "e9197d302312aaf18f576aed38358c5e39854e12a297dac37fcab3e1f53c8460",
                ),
                (
                    "ReconcileContinuityMaintenanceEffect",
                    "2dc583f57f23f12026cef748bb61c0db72150d3ed31f4e6e31be77f8f63e1fa1",
                ),
                (
                    "WithdrawEffectIntent",
                    "6df3b90a4963ffef04865a9e70b57f2040b2b2159b35cfb58347866ed6afe2f9",
                ),
                (
                    "WithdrawBootstrapMandateInteractionEffect",
                    "00ae32e979c74e12f6ebc2f31da11890043495cb8042708edd3f5063c72f2a29",
                ),
                (
                    "WithdrawContinuityMaintenanceEffect",
                    "d5ed8273857101d805748d83023ad067c909427223ae81ba4f9a77f770227d47",
                ),
            ]
        );
        assert!(
            RepositoryActionLeafV1::ALL[..15]
                .iter()
                .all(|leaf| !leaf.is_execution_action())
        );
        assert_eq!(
            execution
                .iter()
                .filter(|leaf| leaf.is_ordinary_execution_action())
                .count(),
            8
        );
        assert_eq!(
            execution
                .iter()
                .filter(|leaf| {
                    leaf.execution_authority_basis()
                        == Some(ActionAuthorityBasisKindV1::BootstrapControlG0)
                })
                .count(),
            4
        );
        assert_eq!(
            execution
                .iter()
                .filter(|leaf| {
                    leaf.execution_authority_basis()
                        == Some(ActionAuthorityBasisKindV1::ContinuityMaintenance)
                })
                .count(),
            4
        );
    }
}
