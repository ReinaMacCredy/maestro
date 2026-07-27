//! Atomic Scheduling Policy Binding publication through the frozen Authority facade.

use thiserror::Error;

use crate::domain::vnext::authority::governance_attestation_stage7_seed::publish_scheduling_policy_from_stage7;
use crate::domain::vnext::authority::{
    ActionRequestIdV1, AuthorityFacadeV1, PlanningRepositoryActionAuthorityV1,
    RepositoryAuthoritySelectionV1, RepositoryDownstreamActionLeafV1,
    RepositoryLeafAuthorityErrorV1,
};
use crate::domain::vnext::persistence::{
    StoreIdempotencyProbeV1, StoreObjectV1, StorePublicationOutcomeV1, StoreV1,
};

use super::{PlanningRecordV1, PlanningTransitionDispositionV1, PlanningTransitionV1};

#[derive(Debug)]
pub(crate) struct SchedulingPolicyPublicationInputV1 {
    pub(crate) request_id: ActionRequestIdV1,
    pub(crate) authority_selection: RepositoryAuthoritySelectionV1,
    pub(crate) transition: PlanningTransitionV1,
    pub(crate) request_object: StoreObjectV1,
    pub(crate) binding_object: StoreObjectV1,
}

pub(crate) fn publish_scheduling_policy_binding(
    store: &mut StoreV1,
    probe: &StoreIdempotencyProbeV1,
    input: SchedulingPolicyPublicationInputV1,
) -> Result<StorePublicationOutcomeV1, SchedulingPolicyPublicationErrorV1> {
    let SchedulingPolicyPublicationInputV1 {
        request_id,
        authority_selection,
        transition,
        request_object,
        binding_object,
    } = input;
    if transition.disposition() != PlanningTransitionDispositionV1::Committed
        || transition.action_literal() != "PublishSchedulingPolicyBinding"
        || transition.actor_session() != authority_selection.actor_session_id()
    {
        return Err(SchedulingPolicyPublicationErrorV1::InvalidTransition);
    }
    let [PlanningRecordV1::PolicyBinding(binding)] = transition.records() else {
        return Err(SchedulingPolicyPublicationErrorV1::InvalidTransition);
    };
    let safety_floor = transition
        .scheduling_safety_floor()
        .ok_or(SchedulingPolicyPublicationErrorV1::InvalidTransition)?;
    if binding_object.value() != &binding.canonical_value()
        || binding_object.id() == request_object.id()
    {
        return Err(SchedulingPolicyPublicationErrorV1::BindingObjectMismatch);
    }

    if binding.diff.candidate_policy_hash != binding.policy.semantic_hash
        || safety_floor.admits(&binding.policy).is_err()
    {
        return Err(SchedulingPolicyPublicationErrorV1::PolicyDiffMismatch);
    }

    let action = RepositoryDownstreamActionLeafV1::from_global_tag(105)
        .map_err(|_| SchedulingPolicyPublicationErrorV1::InvalidTransition)?;
    let authority = PlanningRepositoryActionAuthorityV1::new(
        authority_selection,
        action,
        transition.subject_commitment(),
        transition.owner_basis_commitment(),
        *binding_object.id().as_bytes(),
    )?;
    let mut facade = AuthorityFacadeV1::new(store);
    publish_scheduling_policy_from_stage7(
        &mut facade,
        probe,
        authority,
        request_id,
        request_object,
        binding_object,
        &binding.policy,
        safety_floor,
    )
    .map_err(|_| SchedulingPolicyPublicationErrorV1::AuthorityPublication)
}

#[derive(Debug, Error)]
pub(crate) enum SchedulingPolicyPublicationErrorV1 {
    #[error("the Planning transition is not one committed Action 105 by the selected Session")]
    InvalidTransition,
    #[error("the candidate Binding Store Object is not the exact Planning Binding")]
    BindingObjectMismatch,
    #[error("the Planning policy diff does not match the pinned classifier and Safety Floor")]
    PolicyDiffMismatch,
    #[error(transparent)]
    AuthorityInput(#[from] RepositoryLeafAuthorityErrorV1),
    #[error("Authority refused or failed the atomic Scheduling publication")]
    AuthorityPublication,
}
