//! Stage-7 composition over frozen Authority and owner-domain transitions.
#![expect(
    dead_code,
    reason = "Stage-7 candidate operation facade remains inert until downstream integration"
)]

use crate::domain::orchestration::runtime::{
    ActionFrontierViewV1, BoundedContinuationErrorV1, BoundedContinuationInputV1,
    BoundedContinuationOutcomeV1, ComposedRecipeAdviceV1, FrozenRecipeCatalogV1,
    RecipeEvaluationInputV1, RecipeRuntimeErrorV1, evaluate_bounded_continuation,
    evaluate_recipe_application,
};
use crate::domain::persistence::{StoreIdempotencyProbeV1, StorePublicationOutcomeV1, StoreV1};
use crate::domain::planning::{
    SchedulingPolicyPublicationErrorV1, SchedulingPolicyPublicationInputV1,
    publish_scheduling_policy_binding as publish_atomic_scheduling_policy_binding,
};

pub(crate) fn apply_recipe(
    catalog: &FrozenRecipeCatalogV1,
    frontier: &ActionFrontierViewV1,
    input: RecipeEvaluationInputV1,
) -> Result<ComposedRecipeAdviceV1, RecipeRuntimeErrorV1> {
    evaluate_recipe_application(catalog, frontier, input)
}

pub(crate) fn assess_bounded_continuation(
    catalog: &FrozenRecipeCatalogV1,
    advice: &ComposedRecipeAdviceV1,
    input: &BoundedContinuationInputV1,
) -> Result<BoundedContinuationOutcomeV1, BoundedContinuationErrorV1> {
    evaluate_bounded_continuation(catalog, advice, input)
}

pub(crate) fn publish_scheduling_policy_binding(
    store: &mut StoreV1,
    probe: &StoreIdempotencyProbeV1,
    input: SchedulingPolicyPublicationInputV1,
) -> Result<StorePublicationOutcomeV1, SchedulingPolicyPublicationErrorV1> {
    publish_atomic_scheduling_policy_binding(store, probe, input)
}

#[cfg(test)]
use crate::domain::authority::{ActionRequestIdV1, RepositoryAuthoritySelectionV1};
#[cfg(test)]
use crate::domain::coordination::{
    AdmittedCoordinationTransitionV1, CoordinationAdmissionErrorV1, CoordinationTransitionV1,
    admit_coordination_transition,
};
#[cfg(test)]
use crate::domain::persistence::{StoreGenerationV1, StorePublicationViewV1};
#[cfg(test)]
use crate::domain::planning::{
    AdmittedPlanningTransitionV1, PlanningAdmissionErrorV1, PlanningTransitionV1,
    admit_planning_transition,
};

#[cfg(test)]
pub(crate) fn prepare_coordination_action(
    view: &StorePublicationViewV1<'_>,
    generation: &StoreGenerationV1,
    request_id: ActionRequestIdV1,
    selection: RepositoryAuthoritySelectionV1,
    transition: CoordinationTransitionV1,
) -> Result<AdmittedCoordinationTransitionV1, CoordinationAdmissionErrorV1> {
    admit_coordination_transition(view, generation, request_id, selection, transition)
}

#[cfg(test)]
pub(crate) fn prepare_planning_action(
    view: &StorePublicationViewV1<'_>,
    generation: &StoreGenerationV1,
    request_id: ActionRequestIdV1,
    selection: RepositoryAuthoritySelectionV1,
    transition: PlanningTransitionV1,
) -> Result<AdmittedPlanningTransitionV1, PlanningAdmissionErrorV1> {
    admit_planning_transition(view, generation, request_id, selection, transition)
}
