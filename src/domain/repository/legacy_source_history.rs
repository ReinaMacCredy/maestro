#![allow(
    dead_code,
    reason = "V8 Repository history leaf awaits the bounded MainIntegration export checkpoint"
)]

use crate::domain::persistence::{
    StoreRoleV1, StoreV1,
    legacy_source_history::{
        LegacySourceHistorySelectorV1, StoreLegacySourceCurrentnessV1,
        StoreLegacySourceHistoryContextV1, StoreLegacySourceHistoryErrorV1,
        StoreLegacySourceHistoryProviderV1, StoreLegacySourceHistorySnapshotV1,
    },
};
use crate::foundation::core::legacy_loss_evidence::{
    LegacySourceHistoryKindV1, OwnerIssuedUnavailablePreexistingLossEvidenceSetV1,
};
use crate::foundation::core::legacy_quarantine::LegacyQuarantineOwnerDomainV3;

pub(in crate::domain::repository) struct LegacySourceHistorySnapshotV1(
    StoreLegacySourceHistorySnapshotV1,
);

impl LegacySourceHistorySnapshotV1 {
    pub(in crate::domain::repository) fn capture_present_file(
        store: &StoreV1,
        relative_locator: &[u8],
        context: StoreLegacySourceHistoryContextV1,
    ) -> Result<Self, RepositoryLegacySourceHistoryErrorV1> {
        if store.role() != StoreRoleV1::Repository {
            return Err(RepositoryLegacySourceHistoryErrorV1::WrongStoreRole);
        }
        Ok(Self(
            StoreLegacySourceHistorySnapshotV1::capture_present_file(
                store,
                LegacyQuarantineOwnerDomainV3::Repository,
                LegacySourceHistoryKindV1::RepositoryStore,
                relative_locator,
                context,
            )?,
        ))
    }

    pub(in crate::domain::repository) fn persist(
        self,
        store: &mut StoreV1,
    ) -> Result<(), RepositoryLegacySourceHistoryErrorV1> {
        self.0.persist(store)?;
        Ok(())
    }
}

pub(in crate::domain::repository) struct RepositoryLegacySourceHistoryProviderV1 {
    provider: StoreLegacySourceHistoryProviderV1,
}

impl RepositoryLegacySourceHistoryProviderV1 {
    pub(in crate::domain::repository) fn acquire(
        store: &StoreV1,
    ) -> Result<Self, RepositoryLegacySourceHistoryErrorV1> {
        Ok(Self {
            provider: StoreLegacySourceHistoryProviderV1::acquire(
                store,
                LegacyQuarantineOwnerDomainV3::Repository,
            )?,
        })
    }

    pub(in crate::domain::repository) fn issue_for_absent_sources(
        self,
        store: &StoreV1,
        currentness: StoreLegacySourceCurrentnessV1,
        absent_sources: &[LegacySourceHistorySelectorV1],
    ) -> Result<
        OwnerIssuedUnavailablePreexistingLossEvidenceSetV1,
        RepositoryLegacySourceHistoryErrorV1,
    > {
        Ok(self
            .provider
            .issue_for_absent_sources(store, currentness, absent_sources)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum RepositoryLegacySourceHistoryErrorV1 {
    #[error("Repository legacy source history requires a Repository Store")]
    WrongStoreRole,
    #[error(transparent)]
    History(#[from] StoreLegacySourceHistoryErrorV1),
}
