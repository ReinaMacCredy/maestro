//! Installation transaction, custody, recovery, and currentness implementation seam.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Installation-owned consumer snapshot before its Stage 9 through 11 consumers"
    )
)]
pub(in crate::domain::vnext) mod consumer_snapshot;
mod consumer_snapshot_stage11_seed;
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

#[cfg(test)]
pub(in crate::domain::vnext) fn consume_pre_store_with_test_owner<'locator>(
    locator_lease: crate::domain::vnext::persistence::protected_locator_lease::ProtectedLocatorLeaseV1<
        'locator,
    >,
    writes_before_dispatch: u64,
) -> bool {
    durable_finality::consume_pre_store_with_test_owner(locator_lease, writes_before_dispatch)
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(installation-stage9): Remove after Stage 9 integrates the frozen ActiveStore finality facade"
    )
)]
pub(in crate::domain::vnext) mod stage9_finality {
    use std::marker::PhantomData;
    use std::rc::Rc;

    use thiserror::Error;

    pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalitySeedV1 {
        inner: super::durable_finality_stage9_seed::Stage9ActiveStoreFinalitySeedV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalityOperationV1<'effect> {
        inner: super::durable_finality::Stage9ActiveStoreFinalityOperationV1<'effect>,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalityOutcomeV1 {
        class: Stage9ActiveStoreFinalityOutcomeClassV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    #[derive(Debug, Eq, PartialEq)]
    pub(in crate::domain::vnext) enum Stage9ActiveStoreFinalityOutcomeClassV1 {
        Committed,
        RecoveryRequired,
        InDoubt,
        IntegrityBlocked,
    }

    #[derive(Debug, Error, Eq, PartialEq)]
    #[error("the Stage 9 ActiveStore finality operation was refused")]
    pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalityErrorV1;

    pub(in crate::domain::vnext) fn acquire_finality_seed() -> Stage9ActiveStoreFinalitySeedV1 {
        Stage9ActiveStoreFinalitySeedV1 {
            inner: super::durable_finality_stage9_seed::acquire(),
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain::vnext) fn prepare_active_from_stage9_owner(
        backend: &mut Stage9ActiveStoreFinalitySeedV1,
    ) -> Result<Stage9ActiveStoreFinalityOperationV1<'_>, Stage9ActiveStoreFinalityErrorV1> {
        super::durable_finality::prepare_active_from_stage9_owner(&mut backend.inner)
            .map(|inner| Stage9ActiveStoreFinalityOperationV1 {
                inner,
                _not_send_or_sync: PhantomData,
            })
            .map_err(|_| Stage9ActiveStoreFinalityErrorV1)
    }

    pub(in crate::domain::vnext) fn execute_active_from_stage9_owner(
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
        pub(in crate::domain::vnext) fn into_class(
            self,
        ) -> Stage9ActiveStoreFinalityOutcomeClassV1 {
            self.class
        }
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(installation-stage11): Remove after Stage 11 integrates the frozen PreStore finality facade"
    )
)]
pub(in crate::domain::vnext) mod stage11_finality {
    use std::marker::PhantomData;
    use std::rc::Rc;

    use thiserror::Error;

    use crate::domain::vnext::persistence::protected_locator_lease::ProtectedLocatorLeaseV1;

    pub(in crate::domain::vnext) struct Stage11PreStoreFinalitySeedV1 {
        inner: super::durable_finality_stage11_seed::Stage11PreStoreFinalitySeedV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain::vnext) struct Stage11PreStoreFinalityOperationV1<'effect> {
        inner: super::durable_finality::Stage11PreStoreFinalityOperationV1<'effect>,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain::vnext) struct Stage11PreStoreFinalityOutcomeV1 {
        class: Stage11PreStoreFinalityOutcomeClassV1,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    #[derive(Debug, Eq, PartialEq)]
    pub(in crate::domain::vnext) enum Stage11PreStoreFinalityOutcomeClassV1 {
        Committed,
        RecoveryRequired,
        InDoubt,
        IntegrityBlocked,
    }

    #[derive(Debug, Error, Eq, PartialEq)]
    #[error("the Stage 11 PreStore finality operation was refused")]
    pub(in crate::domain::vnext) struct Stage11PreStoreFinalityErrorV1;

    pub(in crate::domain::vnext) fn acquire_finality_seed() -> Stage11PreStoreFinalitySeedV1 {
        Stage11PreStoreFinalitySeedV1 {
            inner: super::durable_finality_stage11_seed::acquire(),
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain::vnext) fn prepare_pre_store_from_stage11_owner(
        backend: &mut Stage11PreStoreFinalitySeedV1,
    ) -> Result<Stage11PreStoreFinalityOperationV1<'_>, Stage11PreStoreFinalityErrorV1> {
        super::durable_finality::prepare_pre_store_from_stage11_owner(&mut backend.inner)
            .map(|inner| Stage11PreStoreFinalityOperationV1 {
                inner,
                _not_send_or_sync: PhantomData,
            })
            .map_err(|_| Stage11PreStoreFinalityErrorV1)
    }

    pub(in crate::domain::vnext) fn execute_pre_store_from_stage11_owner<'locator>(
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
        pub(in crate::domain::vnext) fn into_class(self) -> Stage11PreStoreFinalityOutcomeClassV1 {
            self.class
        }
    }
}
