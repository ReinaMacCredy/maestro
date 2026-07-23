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
