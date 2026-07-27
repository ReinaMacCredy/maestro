#![allow(
    dead_code,
    unused_imports,
    reason = "Stage 9 is an isolated candidate until its integration commit exposes this facade"
)]

//! Installation transaction, custody, recovery, and currentness implementation seam.
//!
//! Installation consumes frozen Distribution and Migration facts. It owns the
//! exact host closure, protected locator candidate, and domain-local
//! currentness classification; it never treats a Receipt as bearer authority.

mod census;
mod closure;
mod consumer_materialization;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Installation-owned consumer snapshot before its Stage 9 through 11 consumers"
    )
)]
pub(in crate::domain) mod consumer_snapshot;
mod consumer_snapshot_stage11_seed;
mod currentness;
mod cutover;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes durable Installation finality before its Stage 9 and Stage 11 production consumers"
    )
)]
mod durable_finality;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the PreStore owner seed before Stage 11 integrates it"
    )
)]
mod durable_finality_stage11_seed;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the ActiveStore owner seed before Stage 9 integrates it"
    )
)]
mod durable_finality_stage9_seed;
mod resource_cutover;

pub use census::{
    InstallationCensusClassV1, InstallationCensusEntryV1, InstallationCensusErrorV1,
    InstallationCensusHeaderV1, InstallationCensusV1,
};
pub use closure::{
    HostActivationEntryV1, HostAdmissionStateV1, InstallationClosureErrorV1,
    RepositoryInstallationClosureV1, UserAgentInstallationClosureV1,
};
pub(in crate::domain) use consumer_materialization::{
    InstallationConsumerMaterializationErrorV1, Stage9ActiveConsumerMaterializationV1,
};
pub(crate) use consumer_snapshot::AgentResourceReleaseConsumerSealV1;
pub use currentness::{
    DomainCurrentnessV1, ObservedHostActivationV1, ObservedInstallationClosureV1,
    assess_user_agent_currentness,
};
pub use cutover::{
    ActiveStoreCutoverCandidateV1, CutoverDomainBindingV1, InstallationCutoverErrorV1,
    InstallationLocatorCandidateV1, PreStoreCutoverCandidateV1,
};
pub(crate) use resource_cutover::{
    AgentResourceCutoverErrorV1, AgentResourceReleaseAdmissionV1, AgentResourceReleaseOwnerFactsV1,
    AgentResourceTargetOwnerFactsV1, CommittedAgentResourceReleaseV1,
};

pub(crate) fn admit_installation_census_roots_v2(
    roots: &[impl AsRef<std::path::Path>],
    owner_currentness: [u8; 32],
) -> crate::foundation::core::secure_fs::SecureFsResult<
    crate::foundation::core::stage11_aggregate_census::InstallationAdmittedRootSourceV2,
> {
    crate::foundation::core::stage11_aggregate_census::admit_installation_roots_v2(
        roots,
        owner_currentness,
    )
}

#[cfg(test)]
pub(in crate::domain) fn consume_pre_store_with_test_owner<'locator>(
    locator_lease: crate::domain::persistence::protected_locator_lease::ProtectedLocatorLeaseV1<
        'locator,
    >,
    writes_before_dispatch: u64,
) -> bool {
    durable_finality::consume_pre_store_with_test_owner(locator_lease, writes_before_dispatch)
}

#[cfg(test)]
pub(in crate::domain) mod stage9_finality {
    use std::marker::PhantomData;
    use std::rc::Rc;

    use thiserror::Error;

    pub(in crate::domain) type Stage9ActiveStoreFinalityProviderSeedV1 =
        super::durable_finality_stage9_seed::Stage9ActiveStoreFinalitySeedV1;

    pub(in crate::domain) trait Stage9ActiveStoreFinalityProviderV1:
        super::durable_finality::ActiveStoreFinalityOwnerV1
    {
    }

    impl<T> Stage9ActiveStoreFinalityProviderV1 for T where
        T: super::durable_finality::ActiveStoreFinalityOwnerV1 + ?Sized
    {
    }

    pub(in crate::domain) struct Stage9ActiveStoreFinalityProviderBindingV1<'effect> {
        inner: &'effect mut dyn super::durable_finality::ActiveStoreFinalityOwnerV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain) struct Stage9ActiveStoreFinalityOperationV1<'effect> {
        inner: super::durable_finality::Stage9ActiveStoreFinalityOperationV1<'effect>,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain) struct Stage9ActiveStoreFinalityOutcomeV1 {
        class: Stage9ActiveStoreFinalityOutcomeClassV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    #[derive(Debug, Eq, PartialEq)]
    pub(in crate::domain) enum Stage9ActiveStoreFinalityOutcomeClassV1 {
        Committed,
        RecoveryRequired,
        InDoubt,
        IntegrityBlocked,
    }

    #[derive(Debug, Error, Eq, PartialEq)]
    #[error("the Stage 9 ActiveStore finality operation was refused")]
    pub(in crate::domain) struct Stage9ActiveStoreFinalityErrorV1;

    pub(in crate::domain) fn bind_finality_provider<'effect, P>(
        provider: &'effect mut P,
    ) -> Stage9ActiveStoreFinalityProviderBindingV1<'effect>
    where
        P: Stage9ActiveStoreFinalityProviderV1,
    {
        Stage9ActiveStoreFinalityProviderBindingV1 {
            inner: provider,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain) fn prepare_active_from_stage9_owner<'effect>(
        backend: Stage9ActiveStoreFinalityProviderBindingV1<'effect>,
    ) -> Result<Stage9ActiveStoreFinalityOperationV1<'effect>, Stage9ActiveStoreFinalityErrorV1>
    {
        super::durable_finality::prepare_active_from_stage9_owner(backend.inner)
            .map(|inner| Stage9ActiveStoreFinalityOperationV1 {
                inner,
                _not_send_or_sync: PhantomData,
            })
            .map_err(|_| Stage9ActiveStoreFinalityErrorV1)
    }

    pub(in crate::domain) fn execute_active_from_stage9_owner(
        operation: Stage9ActiveStoreFinalityOperationV1<'_>,
    ) -> Result<Stage9ActiveStoreFinalityOutcomeV1, Stage9ActiveStoreFinalityErrorV1> {
        super::durable_finality::execute_active_from_stage9_owner(operation.inner)
            .map(|outcome| Stage9ActiveStoreFinalityOutcomeV1 {
                class: match outcome.into_class() {
                    super::durable_finality::Stage9ActiveStoreFinalityOutcomeClassV1::Committed => {
                        Stage9ActiveStoreFinalityOutcomeClassV1::Committed
                    }
                    super::durable_finality::Stage9ActiveStoreFinalityOutcomeClassV1::RecoveryRequired => {
                        Stage9ActiveStoreFinalityOutcomeClassV1::RecoveryRequired
                    }
                    super::durable_finality::Stage9ActiveStoreFinalityOutcomeClassV1::InDoubt => {
                        Stage9ActiveStoreFinalityOutcomeClassV1::InDoubt
                    }
                    super::durable_finality::Stage9ActiveStoreFinalityOutcomeClassV1::IntegrityBlocked => {
                        Stage9ActiveStoreFinalityOutcomeClassV1::IntegrityBlocked
                    }
                },
                _not_send_or_sync: PhantomData,
            })
            .map_err(|_| Stage9ActiveStoreFinalityErrorV1)
    }

    impl Stage9ActiveStoreFinalityOutcomeV1 {
        pub(in crate::domain) fn into_class(self) -> Stage9ActiveStoreFinalityOutcomeClassV1 {
            self.class
        }
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(installation-stage9): Remove after Stage 9 consumes the frozen V2 ActiveStore finality facade"
    )
)]
pub(in crate::domain) mod stage9_finality_v2 {
    pub(in crate::domain) type Stage9ActiveStoreFinalityProviderSeedV2 =
        super::durable_finality_stage9_seed::Stage9ActiveStoreFinalitySeedV2;

    pub(in crate::domain) struct Stage9ActiveStoreFinalityOutcomeV2 {
        class: Stage9ActiveStoreFinalityOutcomeClassV2,
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub(in crate::domain) enum Stage9ActiveStoreFinalityOutcomeClassV2 {
        Committed,
        RecoveryRequired,
        InDoubt,
        IntegrityBlocked,
    }

    pub(in crate::domain) fn execute_active(
        provider: &mut Stage9ActiveStoreFinalityProviderSeedV2,
    ) -> Result<
        Stage9ActiveStoreFinalityOutcomeV2,
        super::durable_finality::DurableInstallationFinalityErrorV2,
    > {
        super::durable_finality::DurableInstallationFinalityBackendV2::capture(provider)
            .consume_active()
            .map(|outcome| Stage9ActiveStoreFinalityOutcomeV2 {
                class: match outcome {
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::Committed => {
                        Stage9ActiveStoreFinalityOutcomeClassV2::Committed
                    }
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::RecoveryRequired => {
                        Stage9ActiveStoreFinalityOutcomeClassV2::RecoveryRequired
                    }
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::InDoubt => {
                        Stage9ActiveStoreFinalityOutcomeClassV2::InDoubt
                    }
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::IntegrityBlocked => {
                        Stage9ActiveStoreFinalityOutcomeClassV2::IntegrityBlocked
                    }
                },
            })
    }

    impl Stage9ActiveStoreFinalityOutcomeV2 {
        pub(in crate::domain) fn into_class(self) -> Stage9ActiveStoreFinalityOutcomeClassV2 {
            self.class
        }
    }
}

#[cfg(test)]
mod candidate_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vnext_stage9_installation.rs"
    ));
    stage9_installation_candidate_tests!();
}

#[cfg(test)]
pub(in crate::domain) mod stage11_finality {
    use std::marker::PhantomData;
    use std::rc::Rc;

    use thiserror::Error;

    use crate::domain::persistence::protected_locator_lease::ProtectedLocatorLeaseV1;

    pub(in crate::domain) type Stage11PreStoreFinalityProviderSeedV1 =
        super::durable_finality_stage11_seed::Stage11PreStoreFinalitySeedV1;

    pub(in crate::domain) trait Stage11PreStoreFinalityProviderV1:
        super::durable_finality::PreStoreFinalityOwnerV1
    {
    }

    impl<T> Stage11PreStoreFinalityProviderV1 for T where
        T: super::durable_finality::PreStoreFinalityOwnerV1 + ?Sized
    {
    }

    pub(in crate::domain) struct Stage11PreStoreFinalityProviderBindingV1<'effect> {
        inner: &'effect mut dyn super::durable_finality::PreStoreFinalityOwnerV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain) struct Stage11PreStoreFinalityOperationV1<'effect> {
        inner: super::durable_finality::Stage11PreStoreFinalityOperationV1<'effect>,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain) struct Stage11PreStoreFinalityOutcomeV1 {
        class: Stage11PreStoreFinalityOutcomeClassV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    #[derive(Debug, Eq, PartialEq)]
    pub(in crate::domain) enum Stage11PreStoreFinalityOutcomeClassV1 {
        Committed,
        RecoveryRequired,
        InDoubt,
        IntegrityBlocked,
    }

    #[derive(Debug, Error, Eq, PartialEq)]
    #[error("the Stage 11 PreStore finality operation was refused")]
    pub(in crate::domain) struct Stage11PreStoreFinalityErrorV1;

    pub(in crate::domain) fn bind_finality_provider<'effect, P>(
        provider: &'effect mut P,
    ) -> Stage11PreStoreFinalityProviderBindingV1<'effect>
    where
        P: Stage11PreStoreFinalityProviderV1,
    {
        Stage11PreStoreFinalityProviderBindingV1 {
            inner: provider,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain) fn prepare_pre_store_from_stage11_owner<'effect>(
        backend: Stage11PreStoreFinalityProviderBindingV1<'effect>,
    ) -> Result<Stage11PreStoreFinalityOperationV1<'effect>, Stage11PreStoreFinalityErrorV1> {
        super::durable_finality::prepare_pre_store_from_stage11_owner(backend.inner)
            .map(|inner| Stage11PreStoreFinalityOperationV1 {
                inner,
                _not_send_or_sync: PhantomData,
            })
            .map_err(|_| Stage11PreStoreFinalityErrorV1)
    }

    pub(in crate::domain) fn execute_pre_store_from_stage11_owner<'locator>(
        operation: Stage11PreStoreFinalityOperationV1<'_>,
        locator_lease: ProtectedLocatorLeaseV1<'locator>,
    ) -> Result<Stage11PreStoreFinalityOutcomeV1, Stage11PreStoreFinalityErrorV1> {
        super::durable_finality::execute_pre_store_from_stage11_owner(
            operation.inner,
            locator_lease,
        )
        .map(|outcome| Stage11PreStoreFinalityOutcomeV1 {
            class: match outcome.into_class() {
                super::durable_finality::Stage11PreStoreFinalityOutcomeClassV1::Committed => {
                    Stage11PreStoreFinalityOutcomeClassV1::Committed
                }
                super::durable_finality::Stage11PreStoreFinalityOutcomeClassV1::RecoveryRequired => {
                    Stage11PreStoreFinalityOutcomeClassV1::RecoveryRequired
                }
                super::durable_finality::Stage11PreStoreFinalityOutcomeClassV1::InDoubt => {
                    Stage11PreStoreFinalityOutcomeClassV1::InDoubt
                }
                super::durable_finality::Stage11PreStoreFinalityOutcomeClassV1::IntegrityBlocked => {
                    Stage11PreStoreFinalityOutcomeClassV1::IntegrityBlocked
                }
            },
            _not_send_or_sync: PhantomData,
        })
        .map_err(|_| Stage11PreStoreFinalityErrorV1)
    }

    impl Stage11PreStoreFinalityOutcomeV1 {
        pub(in crate::domain) fn into_class(self) -> Stage11PreStoreFinalityOutcomeClassV1 {
            self.class
        }
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(installation-stage11): Remove after Stage 11 consumes the frozen V2 PreStore finality facade"
    )
)]
pub(in crate::domain) mod stage11_finality_v2 {
    use std::marker::PhantomData;
    use std::rc::Rc;

    use crate::domain::persistence::protected_locator_lease::ProtectedLocatorCandidateInputV2;
    use crate::domain::persistence::protected_locator_v2::ProtectedLocatorLeaseV2;

    pub(in crate::domain) type Stage11PreStoreFinalityProviderSeedV2 =
        super::durable_finality_stage11_seed::Stage11PreStoreFinalitySeedV2;

    pub(in crate::domain) fn capture_inert_candidate(
        currentness: super::durable_finality::InstallationFinalityCurrentnessV1,
        decision: super::durable_finality::PreStoreDecisionTupleV1,
        candidate: ProtectedLocatorCandidateInputV2,
    ) -> Stage11PreStoreFinalityProviderSeedV2 {
        Stage11PreStoreFinalityProviderSeedV2::from_installation_owner(
            currentness,
            decision,
            candidate,
        )
    }

    pub(in crate::domain) trait Stage11PreStoreFinalityProviderV2:
        super::durable_finality::PreStoreFinalityOwnerV2
    {
    }

    impl<T> Stage11PreStoreFinalityProviderV2 for T where
        T: super::durable_finality::PreStoreFinalityOwnerV2 + ?Sized
    {
    }

    pub(in crate::domain) struct Stage11PreStoreFinalityProviderBindingV2<'effect> {
        inner: &'effect mut dyn super::durable_finality::PreStoreFinalityOwnerV2,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain) struct Stage11PreStoreFinalityOutcomeV2 {
        class: Stage11PreStoreFinalityOutcomeClassV2,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub(in crate::domain) enum Stage11PreStoreFinalityOutcomeClassV2 {
        Committed,
        RecoveryRequired,
        InDoubt,
        IntegrityBlocked,
    }

    pub(in crate::domain) fn bind_owner_provider<P>(
        provider: &mut P,
    ) -> Stage11PreStoreFinalityProviderBindingV2<'_>
    where
        P: Stage11PreStoreFinalityProviderV2,
    {
        Stage11PreStoreFinalityProviderBindingV2 {
            inner: provider,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain) fn execute_pre_store(
        provider: Stage11PreStoreFinalityProviderBindingV2<'_>,
        locator: ProtectedLocatorLeaseV2<'_>,
    ) -> Result<
        Stage11PreStoreFinalityOutcomeV2,
        super::durable_finality::DurableInstallationFinalityErrorV2,
    > {
        super::durable_finality::DurableInstallationFinalityBackendV2::capture(provider.inner)
            .consume_pre_store(locator)
            .map(|outcome| Stage11PreStoreFinalityOutcomeV2 {
                class: match outcome {
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::Committed => {
                        Stage11PreStoreFinalityOutcomeClassV2::Committed
                    }
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::RecoveryRequired => {
                        Stage11PreStoreFinalityOutcomeClassV2::RecoveryRequired
                    }
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::InDoubt => {
                        Stage11PreStoreFinalityOutcomeClassV2::InDoubt
                    }
                    super::durable_finality::DurableInstallationFinalityOutcomeV2::IntegrityBlocked => {
                        Stage11PreStoreFinalityOutcomeClassV2::IntegrityBlocked
                    }
                },
                _not_send_or_sync: PhantomData,
            })
    }

    impl Stage11PreStoreFinalityOutcomeV2 {
        pub(in crate::domain) fn into_class(self) -> Stage11PreStoreFinalityOutcomeClassV2 {
            self.class
        }
    }
}

#[cfg(test)]
mod v2_finality_compile_tests {
    use super::{stage9_finality_v2, stage11_finality_v2};

    #[test]
    fn stage9_and_stage11_seeds_reach_only_the_owner_bound_v2_facades() {
        let mut active =
            stage9_finality_v2::Stage9ActiveStoreFinalityProviderSeedV2::test_unavailable();
        assert!(stage9_finality_v2::execute_active(&mut active).is_err());
        let _ = stage9_finality_v2::Stage9ActiveStoreFinalityOutcomeV2::into_class;

        let mut pre_store =
            stage11_finality_v2::Stage11PreStoreFinalityProviderSeedV2::test_unavailable();
        let _pre_store = stage11_finality_v2::bind_owner_provider(&mut pre_store);
        let _ = stage11_finality_v2::execute_pre_store;
        let _ = stage11_finality_v2::Stage11PreStoreFinalityOutcomeV2::into_class;
    }
}
