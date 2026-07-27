//! Stage-11's sealed handoff from the Foundation-owned aggregate census.
//!
//! This operation deliberately receives no root locator, admission list, or
//! optionality bit. Foundation owns that physical scope and issues the
//! continuation only after it has joined Repository and Installation
//! admissions. Stage 11 may consume the continuation, but it cannot recreate
//! or widen the observed root set.

use std::path::Path;

use thiserror::Error;

use crate::domain::persistence::{StoreError, StoreV1};
use crate::foundation::core::stage11_aggregate_census::{
    MigrationClassificationContinuationV2, SecureFsError, Stage11AggregateCensusComponentV2,
    bind_owner_provider_v2, census_from_stage11_owner_v2, descriptor_backed_provider_v2,
};

/// The non-physical facts Stage 11 may carry forward from Foundation's V2
/// aggregate census.
pub(crate) struct Stage11CensusContinuationV2 {
    admitted_root_set_id: [u8; 32],
    admitted_entry_count: u64,
    admitted_byte_total: u64,
    components: Vec<Stage11AggregateCensusComponentV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Stage11DescriptorCensusLimitsV2 {
    namespace_epoch: u64,
    maximum_entries: u64,
    maximum_bytes: u64,
    maximum_roots: u64,
    maximum_descriptors: u64,
    maximum_depth: u64,
    maximum_name_bytes: u64,
    revocation_revision: u64,
}

impl Stage11DescriptorCensusLimitsV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the finite Foundation scan tuple is deliberately explicit"
    )]
    pub(crate) const fn new(
        namespace_epoch: u64,
        maximum_entries: u64,
        maximum_bytes: u64,
        maximum_roots: u64,
        maximum_descriptors: u64,
        maximum_depth: u64,
        maximum_name_bytes: u64,
        revocation_revision: u64,
    ) -> Self {
        Self {
            namespace_epoch,
            maximum_entries,
            maximum_bytes,
            maximum_roots,
            maximum_descriptors,
            maximum_depth,
            maximum_name_bytes,
            revocation_revision,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum Stage11DescriptorCensusErrorV2 {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Foundation(#[from] SecureFsError),
}

/// Runs the production Foundation census from owner-issued Repository and
/// Installation admissions. Migration receives only the consumed, non-physical
/// continuation after the two passes and final live-fence check complete.
pub(crate) fn census_admitted_owner_roots_v2(
    store: &StoreV1,
    installation_roots: &[impl AsRef<Path>],
    repository_currentness: [u8; 32],
    installation_currentness: [u8; 32],
    invocation: [u8; 32],
    limits: Stage11DescriptorCensusLimitsV2,
) -> Result<Stage11CensusContinuationV2, Stage11DescriptorCensusErrorV2> {
    let repository = store.admit_repository_census_root_v2(repository_currentness)?;
    let installation = crate::domain::installation::admit_installation_census_roots_v2(
        installation_roots,
        installation_currentness,
    )?;
    let mut provider = descriptor_backed_provider_v2(
        repository,
        installation,
        invocation,
        limits.namespace_epoch,
        limits.maximum_entries,
        limits.maximum_bytes,
        limits.maximum_roots,
        limits.maximum_descriptors,
        limits.maximum_depth,
        limits.maximum_name_bytes,
        limits.revocation_revision,
    )?;
    let binding = bind_owner_provider_v2(&mut provider);
    let continuation = census_from_stage11_owner_v2(binding)?;
    Ok(consume_foundation_census_v2(continuation))
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
