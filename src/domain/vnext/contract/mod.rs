//! Immutable candidate Contract components, roots, finalization, and handoff.

pub mod assembly;
pub mod component;
pub mod component_kind;
pub mod decision_closure;
pub mod finalization;
pub mod handoff;
pub mod materialization;
pub mod proof;
pub mod provenance;
pub mod root;
pub mod runtime;
pub use runtime::InitialContractStepPublicationV1;
