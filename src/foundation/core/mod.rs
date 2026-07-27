#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes aggregate census before its Stage 11 production consumer"
    )
)]
mod aggregate_census;
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
        reason = "TODO(foundation-stage11): Remove after Stage 11 integrates the frozen aggregate census V2 facade"
    )
)]
pub(crate) mod stage11_aggregate_census {
    use std::marker::PhantomData;
    use std::rc::Rc;

    #[cfg(test)]
    use super::aggregate_census_stage11_seed;
    use super::secure_fs::InventoryRowV1;
    use super::secure_fs::SecureFsResult;

    #[cfg(test)]
    pub(crate) type Stage11AggregateCensusProviderSeedV1 =
        super::aggregate_census_stage11_seed::Stage11AggregateCensusBackendSeedV1;
    pub(crate) type Stage11AggregateCensusProviderSeedV2 =
        super::aggregate_census_stage11_seed::Stage11AggregateCensusBackendSeedV2;

    #[cfg(test)]
    pub(crate) trait Stage11AggregateCensusProviderV1:
        super::aggregate_census::AggregateCensusBackendV1
    {
    }

    #[cfg(test)]
    impl<T> Stage11AggregateCensusProviderV1 for T where
        T: super::aggregate_census::AggregateCensusBackendV1 + ?Sized
    {
    }

    pub(crate) trait Stage11AggregateCensusProviderV2:
        super::aggregate_census::AggregateCensusBackendV2
    {
    }

    impl<T> Stage11AggregateCensusProviderV2 for T where
        T: super::aggregate_census::AggregateCensusBackendV2 + ?Sized
    {
    }

    pub(crate) struct Stage11AggregateCensusProviderBindingV2<'scan> {
        inner: &'scan mut dyn super::aggregate_census::AggregateCensusBackendV2,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(crate) struct MigrationClassificationContinuationV2<'scan> {
        inner: super::aggregate_census::MigrationClassificationContinuationV2<'scan>,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(crate) struct Stage11AggregateCensusComponentV2 {
        inner: super::aggregate_census::AggregateComponentCensusV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    #[cfg(test)]
    pub(crate) struct Stage11AggregateCensusProviderBindingV1<'scan> {
        inner: &'scan mut dyn super::aggregate_census::AggregateCensusBackendV1,
    }

    #[cfg(test)]
    pub(crate) struct Stage11AggregateCensusOutputV1<'scan> {
        inner: aggregate_census_stage11_seed::Stage11AggregateCensusOutputV1<'scan>,
    }

    #[cfg(test)]
    pub(crate) struct Stage11AggregateCensusComponentV1 {
        inner: aggregate_census_stage11_seed::Stage11AggregateCensusComponentV1,
    }

    #[cfg(test)]
    pub(crate) fn bind_owner_provider<'scan, P>(
        provider: &'scan mut P,
    ) -> Stage11AggregateCensusProviderBindingV1<'scan>
    where
        P: Stage11AggregateCensusProviderV1,
    {
        Stage11AggregateCensusProviderBindingV1 { inner: provider }
    }

    pub(crate) fn bind_owner_provider_v2<P>(
        provider: &mut P,
    ) -> Stage11AggregateCensusProviderBindingV2<'_>
    where
        P: Stage11AggregateCensusProviderV2,
    {
        Stage11AggregateCensusProviderBindingV2 {
            inner: provider,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(crate) fn census_from_stage11_owner_v2(
        backend: Stage11AggregateCensusProviderBindingV2<'_>,
    ) -> SecureFsResult<MigrationClassificationContinuationV2<'_>> {
        super::aggregate_census::census_from_stage11_owner_v2(backend.inner).map(|inner| {
            MigrationClassificationContinuationV2 {
                inner,
                _not_send_or_sync: PhantomData,
            }
        })
    }

    impl MigrationClassificationContinuationV2<'_> {
        pub(crate) fn consume_for_stage11(
            self,
        ) -> ([u8; 32], u64, u64, Vec<Stage11AggregateCensusComponentV2>) {
            let (admitted_set, entries, bytes, roots) = self.inner.into_stage11_parts();
            let roots = roots
                .into_iter()
                .map(|inner| Stage11AggregateCensusComponentV2 {
                    inner,
                    _not_send_or_sync: PhantomData,
                })
                .collect();
            (admitted_set, entries, bytes, roots)
        }
    }

    impl Stage11AggregateCensusComponentV2 {
        pub(crate) fn consume_for_stage11(
            self,
        ) -> ([u8; 32], [u8; 32], [u8; 32], Vec<InventoryRowV1>) {
            (
                self.inner.resolved_identity,
                self.inner.inventory,
                self.inner.root_binding,
                self.inner.rows,
            )
        }
    }

    #[cfg(test)]
    pub(crate) fn census_from_stage11_owner<'scan>(
        backend: Stage11AggregateCensusProviderBindingV1<'scan>,
    ) -> SecureFsResult<Stage11AggregateCensusOutputV1<'scan>> {
        aggregate_census_stage11_seed::census_from_stage11_owner(backend.inner)
            .map(|inner| Stage11AggregateCensusOutputV1 { inner })
    }

    #[cfg(test)]
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

    #[cfg(test)]
    impl Stage11AggregateCensusComponentV1 {
        pub(crate) fn into_parts(self) -> ([u8; 32], [u8; 32], [u8; 32], Vec<InventoryRowV1>) {
            self.inner.into_parts()
        }
    }
}
pub mod backup;
#[cfg(test)]
pub(crate) mod descriptor_census_platform;
#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
mod aggregate_census_v2_compile_tests {
    use super::stage11_aggregate_census::{
        Stage11AggregateCensusProviderSeedV2, bind_owner_provider_v2, census_from_stage11_owner_v2,
    };

    #[test]
    fn stage11_seed_reaches_the_v2_aggregate_facade_and_defaults_fail_closed() {
        let mut seed = Stage11AggregateCensusProviderSeedV2::test_unavailable();
        let binding = bind_owner_provider_v2(&mut seed);
        assert!(census_from_stage11_owner_v2(binding).is_err());
        let _ =
            super::stage11_aggregate_census::MigrationClassificationContinuationV2::consume_for_stage11;
        let _ =
            super::stage11_aggregate_census::Stage11AggregateCensusComponentV2::consume_for_stage11;
    }
}
