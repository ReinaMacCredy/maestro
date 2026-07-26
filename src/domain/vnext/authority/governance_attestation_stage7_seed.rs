use super::governance_attestation::{
    GovernanceAttestationErrorV1, PlanningSchedulingPolicyInputV1,
};

pub(in crate::domain::vnext) fn publish_scheduling_policy_from_stage7(
    _planning: PlanningSchedulingPolicyInputV1,
) -> Result<[u8; 32], GovernanceAttestationErrorV1> {
    // Stage 7 replaces only this Authority-owned seed body with the live Store
    // transaction that mints the existing df3b Mandate-use and Action-binding
    // carriers. Planning never receives their constructors or Authority facts.
    Err(GovernanceAttestationErrorV1::InvalidAuthorityView)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage7_entry_is_callable_but_fail_closed_before_owner_integration() {
        let planning = PlanningSchedulingPolicyInputV1::from_stage7_planning(
            [4; 4], [5; 4], [1; 4], [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32],
        )
        .unwrap();
        assert_eq!(
            publish_scheduling_policy_from_stage7(planning),
            Err(GovernanceAttestationErrorV1::InvalidAuthorityView)
        );
    }
}
