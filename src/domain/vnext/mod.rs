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
    use super::authority::governance_attestation_stage7_seed::publish_scheduling_policy_from_stage7;

    #[test]
    fn stage7_sibling_can_name_and_call_only_the_seed_owned_entry() {
        let _ = publish_scheduling_policy_from_stage7;
    }
}

#[cfg(test)]
mod stage11_frozen_owner_seed_compile_probe {
    use super::installation::stage11_finality::{
        Stage11PreStoreFinalityOperationV1, Stage11PreStoreFinalityOutcomeClassV1,
        Stage11PreStoreFinalityProviderBindingV1, Stage11PreStoreFinalityProviderSeedV1,
        bind_finality_provider, execute_pre_store_from_stage11_owner,
        prepare_pre_store_from_stage11_owner,
    };
    use super::persistence::protected_locator_lease::ProtectedLocatorLeaseV1;
    use crate::foundation::core::stage11_aggregate_census::{
        Stage11AggregateCensusOutputV1, Stage11AggregateCensusProviderBindingV1,
        Stage11AggregateCensusProviderSeedV1, bind_owner_provider, census_from_stage11_owner,
    };

    fn stage11_can_consume_the_frozen_census_output(census: Stage11AggregateCensusOutputV1<'_>) {
        let (_, _, _, roots) = census.into_parts();
        for root in roots {
            let _ = root.into_parts();
        }
    }

    fn stage11_can_execute_the_frozen_pre_store_operation<'effect, 'locator>(
        operation: Stage11PreStoreFinalityOperationV1<'effect>,
        locator_lease: ProtectedLocatorLeaseV1<'locator>,
    ) {
        if let Ok(outcome) = execute_pre_store_from_stage11_owner(operation, locator_lease) {
            let _: Stage11PreStoreFinalityOutcomeClassV1 = outcome.into_class();
        }
    }

    #[test]
    fn stage11_sibling_can_name_and_call_only_the_frozen_owner_entries() {
        let mut census_provider = Stage11AggregateCensusProviderSeedV1::test_unavailable();
        let mut finality_provider = Stage11PreStoreFinalityProviderSeedV1::test_unavailable();
        let census: Stage11AggregateCensusProviderBindingV1<'_> =
            bind_owner_provider(&mut census_provider);
        assert!(census_from_stage11_owner(census).is_err());
        let finality: Stage11PreStoreFinalityProviderBindingV1<'_> =
            bind_finality_provider(&mut finality_provider);
        assert!(prepare_pre_store_from_stage11_owner(finality).is_err());
        let _ = stage11_can_consume_the_frozen_census_output;
        let _ = stage11_can_execute_the_frozen_pre_store_operation;
    }
}

#[cfg(test)]
mod stage9_frozen_owner_seed_compile_probe {
    use super::installation::stage9_finality::{
        Stage9ActiveStoreFinalityOutcomeClassV1, execute_active_from_stage9_owner,
        prepare_active_from_stage9_owner,
    };
    use super::installation::stage9_finality::{
        Stage9ActiveStoreFinalityProviderBindingV1, Stage9ActiveStoreFinalityProviderSeedV1,
        Stage9ActiveStoreFinalityProviderV1, bind_finality_provider,
    };

    fn stage9_can_call_the_frozen_active_store_owner_entry<P>(provider: &mut P)
    where
        P: Stage9ActiveStoreFinalityProviderV1,
    {
        let binding = bind_finality_provider(provider);
        if let Ok(operation) = prepare_active_from_stage9_owner(binding)
            && let Ok(outcome) = execute_active_from_stage9_owner(operation)
        {
            let _: Stage9ActiveStoreFinalityOutcomeClassV1 = outcome.into_class();
        }
    }

    #[test]
    fn stage9_sibling_can_name_and_call_only_the_frozen_owner_entry() {
        let mut provider = Stage9ActiveStoreFinalityProviderSeedV1::test_unavailable();
        let binding: Stage9ActiveStoreFinalityProviderBindingV1<'_> =
            bind_finality_provider(&mut provider);
        assert!(prepare_active_from_stage9_owner(binding).is_err());
        stage9_can_call_the_frozen_active_store_owner_entry(&mut provider);
    }
}

#[cfg(test)]
mod stage11_consumer_closure_compile_probe {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::installation::consumer_snapshot::{
        ConsumerClosureDurableLinearizationV1, PreCurrentnessConsumerStageV1,
        acquire_stage11_durable_linearization, stage11_test_durable_root,
        stage11_test_successful_durable_linearization,
    };

    fn accept_external_owner_operation(
        _operation: ConsumerClosureDurableLinearizationV1<PreCurrentnessConsumerStageV1>,
    ) {
    }

    #[test]
    fn stage11_sibling_can_use_but_not_construct_the_frozen_operation() {
        let durable_root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "maestro-stage11-consumer-probe-{}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&durable_root);
        accept_external_owner_operation(
            acquire_stage11_durable_linearization::<PreCurrentnessConsumerStageV1>(
                stage11_test_durable_root(&durable_root).unwrap(),
            )
            .unwrap(),
        );
        accept_external_owner_operation(stage11_test_successful_durable_linearization(Rc::new(
            Cell::new(0),
        )));
        std::fs::remove_dir_all(durable_root).unwrap();
    }
}
