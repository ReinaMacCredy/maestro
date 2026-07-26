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
