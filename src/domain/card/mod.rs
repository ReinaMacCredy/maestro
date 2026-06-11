//! The card model: one typed entity (SPEC-beads-model.md) that folds features,
//! tasks, harness-backlog items, and decisions into a single flat
//! `.maestro/cards/<id>/` store.
//!
//! The CAS-backed store is the persistence seam; type-specific behavior stays in
//! the owning domain facades.

pub mod edit;
pub mod fold;
pub mod index;
pub mod query;
pub mod schema;
pub mod store;
