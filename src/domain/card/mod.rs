//! The card model: one typed entity (SPEC-beads-model.md) that folds features,
//! tasks, harness-backlog items, and decisions into a single flat
//! `.maestro/cards/<id>/` store.
//!
//! Slice 1 (P1) is the additive data container plus its CAS-backed store; the
//! four existing entities are untouched until the migration and cutover slices.

pub mod edit;
pub mod fold;
pub mod index;
pub mod query;
pub mod schema;
pub mod store;
