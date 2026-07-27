//! Stage-11's sealed handoff from the Foundation-owned aggregate census.
//!
//! This operation deliberately receives no root locator, admission list, or
//! optionality bit. Foundation owns that physical scope and issues the
//! continuation only after it has joined Repository and Installation
//! admissions. Stage 11 may consume the continuation, but it cannot recreate
//! or widen the observed root set.

use crate::foundation::core::stage11_aggregate_census::{
    MigrationClassificationContinuationV2, Stage11AggregateCensusComponentV2,
};

/// The non-physical facts Stage 11 may carry forward from Foundation's V2
/// aggregate census.
pub(crate) struct Stage11CensusContinuationV2 {
    admitted_root_set_id: [u8; 32],
    admitted_entry_count: u64,
    admitted_byte_total: u64,
    components: Vec<Stage11AggregateCensusComponentV2>,
}

impl Stage11CensusContinuationV2 {
    pub(crate) const fn admitted_root_set_id(&self) -> [u8; 32] {
        self.admitted_root_set_id
    }

    pub(crate) const fn admitted_entry_count(&self) -> u64 {
        self.admitted_entry_count
    }

    pub(crate) const fn admitted_byte_total(&self) -> u64 {
        self.admitted_byte_total
    }

    pub(crate) fn admitted_root_count(&self) -> usize {
        self.components.len()
    }
}

/// Consumes Foundation's owner-sealed V2 continuation without accepting a
/// replacement physical-root source.
///
/// The opaque component collection preserves Foundation's owner-bound census
/// scope for the later Stage-11 classification path. The PreStore finality
/// path remains deferred until Stage 9 provides the
/// `ProtectedLocatorLeaseV2` that Installation's `stage11_finality_v2`
/// consumer requires.
pub(crate) fn consume_foundation_census_v2(
    continuation: MigrationClassificationContinuationV2<'_>,
) -> Stage11CensusContinuationV2 {
    let (admitted_root_set_id, admitted_entry_count, admitted_byte_total, components) =
        continuation.consume_for_stage11();
    Stage11CensusContinuationV2 {
        admitted_root_set_id,
        admitted_entry_count,
        admitted_byte_total,
        components,
    }
}
