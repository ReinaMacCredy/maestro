//! Candidate-only Maestro vNext domain contracts.
//!
//! Modules under this namespace are inert until an exact candidate Contract
//! Root is separately authorized and published.

pub mod authority;
pub mod capability;
pub mod contract;
pub(crate) mod coordination;
pub mod design;
pub mod distribution;
pub mod evidence;
pub mod execution;
pub mod gate;
pub mod identity;
pub(crate) mod installation;
pub(crate) mod intake;
pub mod integration;
pub(crate) mod maturity;
pub(crate) mod memory;
pub mod migration;
pub mod orchestration;
pub mod persistence;
pub(crate) mod planning;
pub(crate) mod projection;
pub mod repository;
pub(crate) mod research;
pub(crate) mod search;
pub mod step;
pub(crate) mod transport;
pub mod work;

#[cfg(test)]
mod stage7_governance_seed_compile_probe {
    use super::authority::governance_attestation::PlanningSchedulingPolicyInputV1;
    use super::authority::governance_attestation_stage7_seed::{
        SchedulingPolicyPublicationKindV1, publish_scheduling_policy_from_stage7,
    };
    use super::authority::{
        ActionRequestIdV1, AuthorityFacadeV1, PlanningRepositoryActionAuthorityV1,
    };
    use super::identity::StoreObjectIdV1;
    use super::persistence::{StoreIdempotencyProbeV1, StoreObjectV1};

    struct Stage7SchedulingPolicyCallerV1<'caller, 'store> {
        facade: &'caller mut AuthorityFacadeV1<'store>,
        probe: &'caller StoreIdempotencyProbeV1,
        authority: PlanningRepositoryActionAuthorityV1,
        request_id: ActionRequestIdV1,
        request_object: StoreObjectV1,
        binding_object: StoreObjectV1,
        current_binding_root: Option<StoreObjectIdV1>,
        planning: PlanningSchedulingPolicyInputV1,
        kind: SchedulingPolicyPublicationKindV1,
    }

    fn stage7_can_call_the_complete_frozen_authority_operation(
        caller: Stage7SchedulingPolicyCallerV1<'_, '_>,
    ) {
        let _ = publish_scheduling_policy_from_stage7(
            caller.facade,
            caller.probe,
            caller.authority,
            caller.request_id,
            caller.request_object,
            caller.binding_object,
            caller.current_binding_root,
            caller.planning,
            caller.kind,
        );
    }

    #[test]
    fn stage7_sibling_can_name_and_call_only_the_seed_owned_entry() {
        let _ = stage7_can_call_the_complete_frozen_authority_operation;
    }
}

#[cfg(test)]
mod stage11_frozen_owner_seed_compile_probe {
    use super::installation::stage11_finality::{
        Stage11PreStoreFinalityOutcomeClassV1, execute_pre_store_from_stage11_owner,
        prepare_pre_store_from_stage11_owner,
    };
    use super::installation::stage11_finality::{
        Stage11PreStoreFinalitySeedV1, acquire_finality_seed,
    };
    use super::persistence::protected_locator_lease::ProtectedLocatorLeaseV1;
    use crate::foundation::core::stage11_aggregate_census::{
        Stage11AggregateCensusBackendSeedV1, acquire_seed as acquire_census_seed,
        census_from_stage11_owner,
    };

    fn stage11_can_call_both_frozen_owner_entries<'locator>(
        census_backend: &mut Stage11AggregateCensusBackendSeedV1,
        finality_backend: &mut Stage11PreStoreFinalitySeedV1,
        locator_lease: ProtectedLocatorLeaseV1<'locator>,
    ) {
        if let Ok(census) = census_from_stage11_owner(census_backend) {
            let (_, _, _, roots) = census.into_parts();
            for root in roots {
                let _ = root.into_parts();
            }
        }

        if let Ok(operation) = prepare_pre_store_from_stage11_owner(finality_backend)
            && let Ok(outcome) = execute_pre_store_from_stage11_owner(operation, locator_lease)
        {
            let _: Stage11PreStoreFinalityOutcomeClassV1 = outcome.into_class();
        }
    }

    #[test]
    fn stage11_sibling_can_name_and_call_only_the_frozen_owner_entries() {
        let _ = acquire_census_seed;
        let _ = acquire_finality_seed;
        let _ = stage11_can_call_both_frozen_owner_entries;
    }
}

#[cfg(test)]
mod stage9_frozen_owner_seed_compile_probe {
    use super::installation::stage9_finality::{
        Stage9ActiveStoreFinalityOutcomeClassV1, execute_active_from_stage9_owner,
        prepare_active_from_stage9_owner,
    };
    use super::installation::stage9_finality::{
        Stage9ActiveStoreFinalitySeedV1, acquire_finality_seed,
    };

    fn stage9_can_call_the_frozen_active_store_owner_entry(
        backend: &mut Stage9ActiveStoreFinalitySeedV1,
    ) {
        if let Ok(operation) = prepare_active_from_stage9_owner(backend)
            && let Ok(outcome) = execute_active_from_stage9_owner(operation)
        {
            let _: Stage9ActiveStoreFinalityOutcomeClassV1 = outcome.into_class();
        }
    }

    #[test]
    fn stage9_sibling_can_name_and_call_only_the_frozen_owner_entry() {
        let _ = acquire_finality_seed;
        let _ = stage9_can_call_the_frozen_active_store_owner_entry;
    }
}

#[cfg(test)]
mod stage11_consumer_closure_compile_probe {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::installation::consumer_snapshot::{
        ConsumerClosureDurableLinearizationV1, PreCurrentnessConsumerStageV1,
        acquire_stage11_durable_linearization, stage11_test_successful_durable_linearization,
    };

    fn accept_external_owner_operation(
        _operation: ConsumerClosureDurableLinearizationV1<PreCurrentnessConsumerStageV1>,
    ) {
    }

    #[test]
    fn stage11_sibling_can_use_but_not_construct_the_frozen_operation() {
        accept_external_owner_operation(
            acquire_stage11_durable_linearization::<PreCurrentnessConsumerStageV1>().unwrap(),
        );
        accept_external_owner_operation(stage11_test_successful_durable_linearization(Rc::new(
            Cell::new(0),
        )));
    }
}
