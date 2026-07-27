//! Immutable Coordination records, owner transitions, and pure Inbox views.
#![expect(
    dead_code,
    reason = "Stage-7 candidate owner module remains inert until downstream integration"
)]

#[cfg(test)]
mod authority_test_adapter;
mod model;
mod projection;
mod state;

#[cfg(test)]
pub(crate) use authority_test_adapter::{
    AdmittedCoordinationTransitionV1, CoordinationAdmissionErrorV1, admit_coordination_transition,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use model::{
    AudienceEligibilitySnapshotV1, AudienceMemberV1, ConflictIdV1, ConflictResolutionKindV1,
    CoordinationAddressV1, CoordinationErrorV1, CoordinationMessageContentV1,
    CoordinationSubjectRefV1, DeliverySubjectV1, ExactMessageRefV1, FocusIdV1, FocusSubjectV1,
    HandoffV1, MessageIdV1, NormalizedScopePathV1, RepositoryInstallationRefV1, ScopeAtomV1,
    ScopeExtentV1, ScopeIdV1, StoreOrderV1, ThreadIdV1, TrustedIntervalV1, WithdrawalReasonV1,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use projection::{
    ConflictViewV1, CoordinationProjectionErrorV1, InboxContinuationV1, InboxPageV1, InboxQueryV1,
    InboxRowV1, PresenceConditionV1, PresenceDispositionV1, PresenceSignalKindV1, PresenceSignalV1,
    ScopeOverlapV1, conflict_view, project_inbox, project_presence, project_scope_overlaps,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use state::{
    CoordinationMutationV1, CoordinationRecordV1, CoordinationStateErrorV1, CoordinationStateV1,
    CoordinationTransitionV1, DeclarationWithdrawalV1, FocusDeclarationV1,
    MessageAcknowledgementV1, MessageV1, ScopeDeclarationV1, ThreadDescriptorV1,
    apply_coordination_mutation,
};

#[cfg(test)]
mod tests;
