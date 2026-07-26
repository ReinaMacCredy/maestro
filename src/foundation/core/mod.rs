#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes aggregate census before its Stage 11 production consumer"
    )
)]
pub(crate) mod aggregate_census;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the aggregate census owner seed before Stage 11 integrates it"
    )
)]
mod aggregate_census_stage11_seed;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(foundation-stage11): Remove after Stage 11 integrates the frozen aggregate census facade"
    )
)]
pub(crate) mod stage11_aggregate_census {
    use super::aggregate_census_stage11_seed;
    use super::secure_fs::{InventoryRowV1, SecureFsResult};

    pub(crate) struct Stage11AggregateCensusBackendSeedV1 {
        inner: aggregate_census_stage11_seed::Stage11AggregateCensusBackendSeedV1,
    }

    pub(crate) struct Stage11AggregateCensusOutputV1<'scan> {
        inner: aggregate_census_stage11_seed::Stage11AggregateCensusOutputV1<'scan>,
    }

    pub(crate) struct Stage11AggregateCensusComponentV1 {
        inner: aggregate_census_stage11_seed::Stage11AggregateCensusComponentV1,
    }

    pub(crate) fn acquire_seed() -> Stage11AggregateCensusBackendSeedV1 {
        Stage11AggregateCensusBackendSeedV1 {
            inner: aggregate_census_stage11_seed::acquire(),
        }
    }

    pub(crate) fn census_from_stage11_owner<'scan>(
        backend: &'scan mut Stage11AggregateCensusBackendSeedV1,
    ) -> SecureFsResult<Stage11AggregateCensusOutputV1<'scan>> {
        aggregate_census_stage11_seed::census_from_stage11_owner(&mut backend.inner)
            .map(|inner| Stage11AggregateCensusOutputV1 { inner })
    }

    impl Stage11AggregateCensusOutputV1<'_> {
        pub(crate) fn into_parts(
            self,
        ) -> ([u8; 32], u64, u64, Vec<Stage11AggregateCensusComponentV1>) {
            let (admitted_set, entries, bytes, roots) = self.inner.into_parts();
            let roots = roots
                .into_iter()
                .map(|inner| Stage11AggregateCensusComponentV1 { inner })
                .collect();
            (admitted_set, entries, bytes, roots)
        }
    }

    impl Stage11AggregateCensusComponentV1 {
        pub(crate) fn into_parts(self) -> ([u8; 32], [u8; 32], [u8; 32], Vec<InventoryRowV1>) {
            self.inner.into_parts()
        }
    }
}
pub mod backup;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the retired singular census port remains only as migration-proof history"
    )
)]
pub(crate) mod descriptor_census_platform;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the retired singular census seed remains only as migration-proof history"
    )
)]
mod descriptor_census_platform_stage11_seed;
pub mod deterministic_cbor;
pub mod diff;
pub mod error;
pub mod fs;
pub mod git;
pub mod hash;
pub mod managed_blocks;
pub mod managed_path;
pub mod paths;
pub mod retention;

const _: fn(
    &secure_fs::SecureRoot,
    secure_fs::DescriptorCensusLimitsV1,
) -> secure_fs::SecureFsResult<secure_fs::DescriptorAnchoredCensusV1> =
    descriptor_census_platform::census;
pub mod safe_write;
pub mod schema;
pub mod secure_fs;
pub mod session;
pub mod slug;
pub mod table;
pub mod time;
