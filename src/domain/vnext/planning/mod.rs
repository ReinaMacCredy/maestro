//! Advisory-only Planning records, evaluation, and owner transitions.
#![expect(
    dead_code,
    reason = "Stage-7 candidate owner module remains inert until downstream integration"
)]

#[cfg(test)]
mod authority_test_adapter;
mod evaluation;
mod model;
mod publication;
mod state;

#[cfg(test)]
pub(crate) use authority_test_adapter::{
    AdmittedPlanningTransitionV1, PlanningAdmissionErrorV1, admit_planning_transition,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use evaluation::{
    SchedulingEvaluationInputV1, SchedulingSafetyFloorV1, classify_policy_diff,
    evaluate_scheduling, observation_closure_hash, owner_fact_closure_hash, proposal_closure_hash,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use model::{
    ActiveHarmDispositionV1, CounterfactualV1, OpportunityFactsV1, PlanningErrorV1,
    PlanningProposalDispositionKindV1, PlanningProposalDispositionV1, PlanningProposalIdV1,
    PlanningProposalV1, ProposalAdviceUnitV1, SchedulingAssessmentInputKeyV1,
    SchedulingAssessmentResultV1, SchedulingAssessmentV1, SchedulingEquivalenceClassV1,
    SchedulingOpportunityRefV1, SchedulingOpportunitySetV1, SchedulingPolicyBindingV1,
    SchedulingPolicySnapshotV1, SchedulingReasonV1, SemanticPolicyDiffKindV1, SemanticPolicyDiffV1,
};
#[allow(
    unused_imports,
    reason = "Stage-7 atomic Scheduling publication is frozen before its downstream adapters"
)]
pub(crate) use publication::{
    PublishedSchedulingPolicyBindingV1, SchedulingPolicyPublicationErrorV1,
    SchedulingPolicyPublicationInputV1, publish_scheduling_policy_binding,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use state::{
    PlanningMutationV1, PlanningRecordV1, PlanningStateErrorV1, PlanningStateV1,
    PlanningTransitionDispositionV1, PlanningTransitionV1, apply_planning_mutation,
};

#[cfg(test)]
mod tests;
