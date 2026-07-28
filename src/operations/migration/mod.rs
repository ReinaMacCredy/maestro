//! Stage-11 offline migration operations.
//!
//! Production operations materialize a sealed quarantine and import an
//! already-sealed vNext backup into an inactive Store. Foundation owns the
//! aggregate physical census and Stage 11 receives only its V2 continuation.
//! These operations do not activate a Store, publish currentness, delete a
//! legacy namespace, or perform domain effects.

#[allow(
    dead_code,
    reason = "V2 Foundation continuation awaits the Stage-9 locator and PreStore owner handoff"
)]
mod census;
#[allow(
    dead_code,
    reason = "inactive Store import remains dormant pending Stage-9/10 owner parity"
)]
mod import;
#[cfg(test)]
mod legacy_census_v1;
#[allow(
    dead_code,
    reason = "the V3 continuation awaits MainIntegration owner wiring"
)]
mod live_set_v3;
#[allow(
    dead_code,
    reason = "sealed quarantine materialization remains dormant pending inventory closure"
)]
mod quarantine;

#[allow(
    unused_imports,
    reason = "the V2 continuation facade awaits the Stage-9 locator and PreStore owner handoff"
)]
pub(crate) use census::{Stage11CensusContinuationV2, consume_foundation_census_v2};
#[allow(
    unused_imports,
    reason = "the inactive import error facade is reserved for ordered Stage-9/10 integration"
)]
pub use import::InactiveImportOperationErrorV1;
#[allow(
    unused_imports,
    reason = "the inactive import operation is reserved for ordered Stage-9/10 integration"
)]
pub use import::import_inactive_store;
#[cfg(test)]
#[allow(
    unused_imports,
    reason = "V1 physical census evidence remains test-only while V2 Foundation continuation owns production scope"
)]
pub(crate) use legacy_census_v1::{
    DeclaredRootScanV1, MigrationCensusErrorV1, recensus_declared_roots,
};
#[allow(
    unused_imports,
    reason = "the V3 continuation facade awaits MainIntegration owner wiring"
)]
pub(crate) use live_set_v3::{
    Stage11LiveSetContinuationV3, Stage11LiveSetOperationErrorV3, Stage11PhysicalClosureV3,
    Stage11SealedCopyContinuationV3, execute_offline_live_set_v3,
};
#[allow(
    unused_imports,
    reason = "the quarantine receipt facade is reserved for ordered migration integration"
)]
pub use quarantine::QuarantineMaterializationReceiptV1;
#[allow(
    unused_imports,
    reason = "quarantine materialization is reserved for ordered migration integration"
)]
pub use quarantine::{QuarantineMaterializationErrorV1, materialize_sealed_quarantine};

#[cfg(test)]
mod tests;
