//! Candidate-only Authority and continuity contracts.
//!
//! The semantic kernel is pure and deterministic. The public facade is the one
//! allowed edge to the canonical Store transaction; adapters, system clocks,
//! filesystems, networks, and alternate currentness are excluded.

mod action_basis;
mod bootstrap_catalog;
mod capacity;
mod closed;
mod context;
mod continuity;
mod downstream_action_basis;
mod evaluator;
mod facade;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Authority governance port before its Stage 7 production consumer"
    )
)]
pub(in crate::domain) mod governance_attestation;
pub(in crate::domain) mod governance_attestation_stage7_seed;
mod governance_floor;
mod grant;
mod identity;
mod legacy_removal_guard;
mod mandate;
pub(crate) mod materialization;
mod post_cut;
mod principal;
mod protected_diagnostic_envelope;
mod protected_diagnostic_envelope_stage8_seed;
mod publication;
mod result;
mod transition;

pub use action_basis::{
    AuthorityActionBasisErrorV1, AuthorityActionLeafV1, RepositoryActionLeafV1,
    RepositoryActionOwnerDispatchV1, exact_authority_basis_for_action,
};
pub use bootstrap_catalog::{
    BootstrapMandateTargetV1, BootstrapTargetDispositionV1, bootstrap_mandate_target_catalog,
};
pub use capacity::{
    CapacityError, CapacityUseDispositionV1, CmaEffectWithdrawalSlotFamilyV1,
    CmaObservationPublicationPurposeV1, CmaWithdrawalCapacityV1, CmaWithdrawalPurposeV1,
    GovernedCapacityDebitV1, GovernedCapacityKindV1, GovernedCapacityRootV1,
    GovernedCapacityTransitionV1, InstallationGovernedCapacitySlotKindV1,
    RepositoryGovernedCapacitySlotKindV1,
};
pub use closed::{
    ActionAuthorityBasisKindV1, AuthorityContextKindV1, AuthorityTagError, TransitionGuardKindV1,
};
pub use context::{
    ActionAuthorityBasisV1, AuthorityContextError, AuthorityContextV1,
    BootstrapControlG0AuthorityBasisV1, ContinuityMaintenanceAuthorityBasisV1,
    InstallationAuthorityContextV1, OrdinaryAuthorityBasisV1, RepositoryAuthorityContextV1,
};
pub use continuity::{
    AcceptedAuthorityTimeFloorV1, AdmittedTransitionGuardV1, AuthorityContinuityClassClosureV1,
    AuthorityContinuityClassDescriptorV1, AuthorityContinuityClosureError,
    AuthorityContinuityClosureIdV1, AuthorityContinuityClosureInputV1,
    AuthorityContinuityClosureV1, AuthorityContinuityCoverageDispositionV1,
    AuthorityContinuityCoverageObligationV1, AuthorityContinuityError,
    AuthorityContinuityFacetDispositionV1, AuthorityContinuityManifestV1,
    AuthorityContinuityOwnerContributionV1, AuthorityContinuityPredecessorV1,
    AuthorityContinuitySemanticCutV1, AuthorityContinuityStateError,
    AuthorityContinuityTotalityInputV1, AuthorityTransitionGuardAdmissionInputV1,
    ClassDispositionV1, ClosureFacetDispositionKindV1, ContinuityCarrierProfileStatusV1,
    ContinuityClassIdV1, ContinuityClosureFacetV1, ContinuityDisclosureV1, ContinuityExactRootV1,
    ContinuityGraphEdgeV1, ContinuityReferenceError, ContinuityReferenceV1,
    ContinuitySemanticOwnerV1, CoverageDispositionKindV1, CoverageObligationIdV1,
    GuardAdmissionKindV1, HTimeAcceptanceErrorV1, HTimeAcceptanceRelationV1, HTimeCarryBasisV1,
    HTimeContinuationContributionV1, InstallationAuthorityContinuityClassV1, OwnerContributionIdV1,
    RepositoryAuthorityContinuityClassV1, SuccessVisibleAuthorityContinuityStateV1,
    TransitionGuardOwnerCensusV1, TransitionGuardTermFactV1,
    installation_authority_continuity_totality_input,
    repository_authority_continuity_totality_input,
};
pub use downstream_action_basis::{
    RepositoryDownstreamActionErrorV1, RepositoryDownstreamActionLeafV1,
};
pub use evaluator::{
    AuthorityEvaluationErrorV1, AuthorityEvaluatorV1, AuthorityRevocationSetV1,
    BootstrapAuthoritySnapshotErrorV1, BootstrapAuthoritySnapshotV1,
    BootstrapContinuityTransitionProofV1, BootstrapInteractionSubjectV1,
    BootstrapMandateInteractionObservationJoinV1, BootstrapMandatePresentationObservationV1,
    BootstrapMandateResponseObservationV1, BootstrapResponseDispositionV1,
    ConsentSlotEvaluationFactsV1,
};
#[allow(
    unused_imports,
    reason = "Stage 5 freezes the released diagnostic envelope before its Stage 8 consumer"
)]
pub(crate) use facade::TrustedHostDiagnosticChallengeV1;
pub use facade::{
    AbsorbWorkAuthorityV1, AmendContractAuthorityV1, AppendDesignRevisionAuthorityV1,
    AuthorityFacadeV1, AuthorityPublicationError, BootstrapExecutionAuthorityV1,
    CancelWorkAuthorityV1, ContinuityMaintenanceExecutionAuthorityV1, CreateDraftWorkAuthorityV1,
    ExecutionAuthorityV1, ExecutionProducerV1, GenericExecutionAuthorityV1,
    PublishInitialContractAuthorityV1, RepositoryAuthenticatedHumanV1,
    RepositoryAuthoritySelectionV1, RepositoryDecisionAuthorityCarrierV1,
    RepositoryDecisionOptionMappingV1, RepositoryDecisionPresentationV1,
    RepositoryLeafAuthorityErrorV1, RepositoryPolicyComponentSetV1, RepositoryPolicySnapshotV1,
    RepositoryPolicyStrengthV1, RepositoryPolicyTransitionAuthorityV1,
    RepositoryPolicyTransitionKindV1, RepositoryPolicyTransitionV1, ResolveDecisionAuthorityV1,
    SubmitStepAuthorityV1, SubmitWorkCompletionAuthorityV1,
};
pub(crate) use facade::{
    AdmittedRepositoryActionV1, ContinuedRepositoryActionV1,
    PersistedEvidenceMutationAuthorityExpectationV1, RepositoryActionAdmissionInputV1,
    RepositoryAuthorityAdmissionErrorV1, RepositoryAuthorityArtifactsV1, admit_repository_action,
    admit_repository_authority_candidate, continue_durably_admitted_repository_action_attempt,
    continue_repository_action_attempt, current_authorization_receipt_is_persisted,
    current_repository_authority_time, validate_persisted_evidence_mutation_authority,
    validate_persisted_repository_action_basis,
};
pub use facade::{
    CoordinationRepositoryActionAuthorityV1, DistributionRepositoryActionAuthorityV1,
    IntakeRepositoryActionAuthorityV1, MemoryRepositoryActionAuthorityV1,
    PersistenceRepositoryActionAuthorityV1, PlanningRepositoryActionAuthorityV1,
    ResearchRepositoryActionAuthorityV1, SearchMaintenanceRepositoryActionAuthorityV1,
};
pub use grant::{
    AuthorityUseConstraintV1, AuthorityValidationError, BootstrapG0PathV1, BootstrapGenesisGrantV1,
    DelegationAncestryV1, DelegationV1, GrantDefinitionV1, GrantScopeV1, GrantV1,
    HalfOpenValidityV1, OrdinaryBoundedGrantV1, OrdinaryGrantDelegationV1, ScopeAtomV1,
    grant_is_revoked_by_closure, validate_delegation,
};
pub use identity::{
    ActionRequestIdV1, ActionResultIdV1, AuthorityBasisCommitmentIdV1, AuthorityContextIdV1,
    AuthorityContinuityManifestIdV1, AuthorityIdV1, AuthorityIdentityError,
    AuthorizationReceiptIdV1, BootstrapMandateIssuanceBindingIdV1, CapacityRootIdV1, CmaBranchIdV1,
    CmaWithdrawalCapacityIdV1, ConsentProtocolCommitmentIdV1, ConsentSlotCommitmentIdV1,
    DelegationIdV1, EffectReferenceIdV1, ExecutorAssertionIdV1, GenesisGrantIdV1, GrantIdV1,
    IdempotencyKeyIdV1, InteractionClosureIdV1, MandateIdV1, ObservationIdV1, PrincipalBindingIdV1,
    PrincipalIdV1, SessionIdV1, SlotIdV1, StateTokenIdV1, TargetActionCommitmentIdV1,
};
#[allow(
    unused_imports,
    reason = "MainIntegration keeps the exact removal capability domain-only"
)]
pub(in crate::domain) use legacy_removal_guard::LegacyRemovalGuardV2;
pub use mandate::{
    AuthorityMandateV1, BootstrapMandateEvaluationV1, BootstrapMandateIssuanceBindingV1,
    BootstrapMandateIssuanceV1, ConsentRequirementMemberV1, ConsentRoleV1,
    ConsentSlotBindingParameterV1, ConsentSlotDerivationErrorV1, IssueBootstrapMandateError,
    IssueBootstrapMandateInputV1, IssueBootstrapMandateRequestV1, NaturalMemberSubjectV1,
    TargetActionEffectKindV1, TargetActionOwnerV1, TargetActionProjectionErrorV1,
    TargetActionProjectionV1, TargetActionProtocolV1, TargetExpectedHeadsV1,
    issue_bootstrap_mandate,
};
pub use post_cut::{
    AuthorityContinuityPostCutConsequenceSetV1, AuthorityPostCutErrorV1,
    LinearizationCoverageWitnessV1, LinearizationFenceCarrierV1,
};
pub use principal::{
    AuthoritySnapshotV1, PrincipalBindingV1, RevocationSetV1, RevocationTargetV1, SessionV1,
    TrustedTimeV1, validate_ordinary_authority,
};
#[allow(
    unused_imports,
    reason = "Stage 5 freezes the released diagnostic envelope before its Stage 8 consumer"
)]
pub(crate) use protected_diagnostic_envelope::ProtectedContinuityDiagnosticReleasedEnvelopeV1;
pub use publication::{
    AuthorityPublicationKindV1, AuthorityPublicationLineageV1, AuthorityPublicationOutcomeV1,
    AuthorityPublicationPlanError, GrantActionIdentityV1, GrantAdministrationAuthorityV1,
    ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1,
    ISSUE_ROOT_ATTACHED_BOUNDED_GRANT_IDEMPOTENCY_NAMESPACE_V1, IssueBootstrapMandatePublicationV1,
    IssueRootAttachedBoundedGrantPublicationV1,
    REISSUE_ROOT_ATTACHED_GRANT_ONE_TO_ONE_IDEMPOTENCY_NAMESPACE_V1,
    REVOKE_GRANT_IDEMPOTENCY_NAMESPACE_V1, ReissueRootAttachedGrantOneToOnePublicationV1,
    RevokeGrantPublicationV1,
};
pub use result::{
    ActionOutcomeV1, ActionResultError, ActionResultV1, AuthorizationReceiptV1, ResponseOriginV1,
};
pub use transition::{TransitionGuardTermBundleV1, TransitionGuardTermV1};

#[cfg(test)]
pub(crate) use facade::test_support;
