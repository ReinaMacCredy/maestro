//! Atomic Scheduling Policy Binding publication through the frozen Authority facade.

use thiserror::Error;

use crate::domain::vnext::authority::governance_attestation::PlanningSchedulingPolicyInputV1;
use crate::domain::vnext::authority::governance_attestation_stage7_seed::{
    SchedulingPolicyPublicationKindV1, publish_scheduling_policy_from_stage7,
};
use crate::domain::vnext::authority::{
    ActionRequestIdV1, AuthorityFacadeV1, PlanningRepositoryActionAuthorityV1,
    RepositoryAuthoritySelectionV1, RepositoryDownstreamActionLeafV1,
    RepositoryLeafAuthorityErrorV1,
};
use crate::domain::vnext::persistence::{
    StoreIdempotencyProbeV1, StoreObjectV1, StorePublicationOutcomeV1, StoreV1,
};

use super::{
    PlanningRecordV1, PlanningTransitionDispositionV1, PlanningTransitionV1,
    SchedulingPolicyBindingV1, SchedulingPolicySnapshotV1, SemanticPolicyDiffKindV1,
    classify_policy_diff,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PublishedSchedulingPolicyBindingV1 {
    pub(crate) binding: SchedulingPolicyBindingV1,
    pub(crate) object: StoreObjectV1,
}

#[derive(Debug)]
pub(crate) struct SchedulingPolicyPublicationInputV1 {
    pub(crate) request_id: ActionRequestIdV1,
    pub(crate) authority_selection: RepositoryAuthoritySelectionV1,
    pub(crate) transition: PlanningTransitionV1,
    pub(crate) request_object: StoreObjectV1,
    pub(crate) binding_object: StoreObjectV1,
    pub(crate) current_binding: Option<PublishedSchedulingPolicyBindingV1>,
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
        current_binding,
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

    let current_policy =
        validate_current_binding(binding, &binding_object, current_binding.as_ref())?;
    let expected_diff = classify_policy_diff(current_policy, &binding.policy, safety_floor)
        .map_err(|_| SchedulingPolicyPublicationErrorV1::PolicyDiffMismatch)?;
    if binding.diff != expected_diff {
        return Err(SchedulingPolicyPublicationErrorV1::PolicyDiffMismatch);
    }

    let current_rules = current_policy.map_or([0; 4], policy_strength);
    let candidate_rules = policy_strength(&binding.policy);
    if relation(current_rules, candidate_rules) != binding.diff.kind {
        return Err(SchedulingPolicyPublicationErrorV1::PolicyMeaningMismatch);
    }
    let requires_downgrade = matches!(
        binding.diff.kind,
        SemanticPolicyDiffKindV1::Weakening | SemanticPolicyDiffKindV1::Incomparable
    );

    let action = RepositoryDownstreamActionLeafV1::from_global_tag(105)
        .map_err(|_| SchedulingPolicyPublicationErrorV1::InvalidTransition)?;
    let authority = PlanningRepositoryActionAuthorityV1::new(
        authority_selection,
        action,
        transition.subject_commitment(),
        transition.owner_basis_commitment(),
        *binding_object.id().as_bytes(),
    )?;
    let current_binding_root = current_binding.as_ref().map(|current| current.object.id());
    let planning = PlanningSchedulingPolicyInputV1::from_stage7_planning(
        current_rules,
        candidate_rules,
        current_binding_root.map_or([0xA5; 32], |root| *root.as_bytes()),
        *binding_object.id().as_bytes(),
        *request_id.as_bytes(),
        *binding_object.id().as_bytes(),
        *probe.key_digest(),
        *probe.meaning_digest(),
    )
    .map_err(|_| SchedulingPolicyPublicationErrorV1::InvalidPlanningInput)?;
    let kind = if requires_downgrade {
        SchedulingPolicyPublicationKindV1::WeakeningOrIncomparableWithMandate
    } else {
        SchedulingPolicyPublicationKindV1::EquivalentOrStrengthening
    };
    let mut facade = AuthorityFacadeV1::new(store);
    publish_scheduling_policy_from_stage7(
        &mut facade,
        probe,
        authority,
        request_id,
        request_object,
        binding_object,
        current_binding_root,
        planning,
        kind,
    )
    .map_err(|_| SchedulingPolicyPublicationErrorV1::AuthorityPublication)
}

fn validate_current_binding<'binding>(
    candidate: &SchedulingPolicyBindingV1,
    candidate_object: &StoreObjectV1,
    current: Option<&'binding PublishedSchedulingPolicyBindingV1>,
) -> Result<Option<&'binding SchedulingPolicySnapshotV1>, SchedulingPolicyPublicationErrorV1> {
    match current {
        None if candidate.expected_old_binding_hash.is_none()
            && candidate.diff.old_policy_hash.is_none()
            && candidate_object.references().is_empty() =>
        {
            Ok(None)
        }
        Some(current)
            if current.object.value() == &current.binding.canonical_value()
                && current.object.schema_id() == candidate_object.schema_id()
                && candidate.expected_old_binding_hash == Some(current.binding.semantic_hash)
                && candidate.diff.old_policy_hash == Some(current.binding.policy.semantic_hash)
                && candidate_object.references() == [current.object.id()] =>
        {
            Ok(Some(&current.binding.policy))
        }
        _ => Err(SchedulingPolicyPublicationErrorV1::CurrentBindingMismatch),
    }
}

fn policy_strength(policy: &SchedulingPolicySnapshotV1) -> [u64; 4] {
    [
        u64::MAX - policy.foundation_maximum_total_time,
        u64::MAX - policy.fairness_maximum_deferral,
        u64::MAX - policy.hysteresis_window,
        u64::MAX - policy.overload_opportunity_limit,
    ]
}

fn relation(current: [u64; 4], candidate: [u64; 4]) -> SemanticPolicyDiffKindV1 {
    let greater = candidate
        .iter()
        .zip(current)
        .any(|(candidate, current)| *candidate > current);
    let lower = candidate
        .iter()
        .zip(current)
        .any(|(candidate, current)| *candidate < current);
    match (greater, lower) {
        (false, false) => SemanticPolicyDiffKindV1::Equivalent,
        (true, false) => SemanticPolicyDiffKindV1::Strengthening,
        (false, true) => SemanticPolicyDiffKindV1::Weakening,
        (true, true) => SemanticPolicyDiffKindV1::Incomparable,
    }
}

#[derive(Debug, Error)]
pub(crate) enum SchedulingPolicyPublicationErrorV1 {
    #[error("the Planning transition is not one committed Action 105 by the selected Session")]
    InvalidTransition,
    #[error("the candidate Binding Store Object is not the exact Planning Binding")]
    BindingObjectMismatch,
    #[error("the current Binding value, Store Object, or expected-old lineage does not match")]
    CurrentBindingMismatch,
    #[error("the Planning policy diff does not match the pinned classifier and Safety Floor")]
    PolicyDiffMismatch,
    #[error("the Planning policy strength relation does not match the persisted diff")]
    PolicyMeaningMismatch,
    #[error("Planning could not construct the typed Scheduling policy and safety input")]
    InvalidPlanningInput,
    #[error(transparent)]
    AuthorityInput(#[from] RepositoryLeafAuthorityErrorV1),
    #[error("Authority refused or failed the atomic Scheduling publication")]
    AuthorityPublication,
}
