use super::ActionRequestIdV1;
use super::facade::{
    AuthorityFacadeV1, AuthorityMaterializationPublicationErrorV1,
    PlanningRepositoryActionAuthorityV1, SchedulingPolicyMaterializationErrorV1,
    SchedulingPolicyPublicationInputV1, StoreIdempotencyProbeV1, StoreObjectV1,
    StorePublicationOutcomeV1,
};
use super::governance_attestation::PlanningSchedulingPolicyInputV1;
use crate::domain::vnext::identity::StoreObjectIdV1;

// TODO(Planning Stage 7): Remove these expectations when the Planning caller
// integrates this frozen Authority operation.
#[expect(
    dead_code,
    reason = "Stage 5 freezes the production-callable Stage 7 operation before Planning integrates"
)]
pub(in crate::domain::vnext) enum SchedulingPolicyPublicationKindV1 {
    EquivalentOrStrengthening,
    WeakeningOrIncomparableWithMandate,
}

#[expect(
    clippy::too_many_arguments,
    reason = "Stage 5 freezes the production-callable Stage 7 operation before Planning integrates"
)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the production-callable Stage 7 operation before Planning integrates"
    )
)]
pub(in crate::domain::vnext) fn publish_scheduling_policy_from_stage7(
    facade: &mut AuthorityFacadeV1<'_>,
    probe: &StoreIdempotencyProbeV1,
    authority: PlanningRepositoryActionAuthorityV1,
    request_id: ActionRequestIdV1,
    request_object: StoreObjectV1,
    binding_object: StoreObjectV1,
    current_binding_root: Option<StoreObjectIdV1>,
    planning: PlanningSchedulingPolicyInputV1,
    kind: SchedulingPolicyPublicationKindV1,
) -> Result<
    StorePublicationOutcomeV1,
    AuthorityMaterializationPublicationErrorV1<SchedulingPolicyMaterializationErrorV1>,
> {
    let input = SchedulingPolicyPublicationInputV1::new(
        request_id,
        request_object,
        binding_object,
        current_binding_root,
        planning,
    );
    match kind {
        SchedulingPolicyPublicationKindV1::EquivalentOrStrengthening => {
            facade.publish_scheduling_policy_without_downgrade(probe, authority, input)
        }
        SchedulingPolicyPublicationKindV1::WeakeningOrIncomparableWithMandate => {
            facade.publish_scheduling_policy_with_downgrade(probe, authority, input)
        }
    }
}
