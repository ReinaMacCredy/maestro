use super::ActionRequestIdV1;
use super::facade::{
    AuthorityFacadeV1, AuthorityMaterializationPublicationErrorV1,
    PlanningRepositoryActionAuthorityV1, SchedulingPolicyMaterializationErrorV1,
    SchedulingPolicyPublicationInputV1, StoreIdempotencyProbeV1, StoreObjectV1,
    StorePublicationOutcomeV1,
};
use super::governance_attestation::PlanningSchedulingPolicyInputV1;
use crate::domain::vnext::planning::{SchedulingPolicySnapshotV1, SchedulingSafetyFloorV1};

#[expect(
    clippy::too_many_arguments,
    reason = "Stage 5 freezes the production-callable Stage 7 operation before Planning integrates"
)]
pub(in crate::domain::vnext) fn publish_scheduling_policy_from_stage7(
    facade: &mut AuthorityFacadeV1<'_>,
    probe: &StoreIdempotencyProbeV1,
    authority: PlanningRepositoryActionAuthorityV1,
    request_id: ActionRequestIdV1,
    request_object: StoreObjectV1,
    binding_object: StoreObjectV1,
    requested_policy: &SchedulingPolicySnapshotV1,
    scheduling_safety_floor: &SchedulingSafetyFloorV1,
) -> Result<
    StorePublicationOutcomeV1,
    AuthorityMaterializationPublicationErrorV1<SchedulingPolicyMaterializationErrorV1>,
> {
    let planning = PlanningSchedulingPolicyInputV1::from_stage7_planning(
        requested_policy,
        scheduling_safety_floor,
    )
    .map_err(SchedulingPolicyMaterializationErrorV1::from)
    .map_err(AuthorityMaterializationPublicationErrorV1::Prepare)?;
    let input = SchedulingPolicyPublicationInputV1::new(
        request_id,
        request_object,
        binding_object,
        planning,
    );
    facade.publish_scheduling_policy(probe, authority, input)
}

const _: fn() = || {
    let _ = publish_scheduling_policy_from_stage7;
};
