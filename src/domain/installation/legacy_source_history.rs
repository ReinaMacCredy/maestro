#![allow(
    dead_code,
    reason = "V8 Installation history leaf awaits the bounded MainIntegration export checkpoint"
)]

use crate::domain::persistence::{
    StoreRoleV1, StoreV1,
    legacy_source_history::{
        LegacySourceHistorySelectorV1, StoreLegacySourceHistoryContextV1,
        StoreLegacySourceHistoryErrorV1, StoreLegacySourceHistoryProviderV1,
        StoreLegacySourceHistorySnapshotV1,
    },
};
use crate::foundation::core::legacy_loss_evidence::{
    FoundationLegacyLossEvidenceErrorV1, FoundationOwnerEvidenceMintV1, LegacySourceHistoryKindV1,
    OwnerUnavailablePreexistingLossEvidenceIssuerPortV1, owner_loss_evidence_issuer_sealed,
};
use crate::foundation::core::legacy_quarantine::LegacyQuarantineOwnerDomainV3;

pub(in crate::domain::installation) struct LegacySourceHistorySnapshotV1(
    StoreLegacySourceHistorySnapshotV1,
);

impl LegacySourceHistorySnapshotV1 {
    pub(in crate::domain::installation) fn capture_present_file(
        store: &StoreV1,
        relative_locator: &[u8],
        context: StoreLegacySourceHistoryContextV1,
    ) -> Result<Self, InstallationLegacySourceHistoryErrorV1> {
        if store.role() != StoreRoleV1::Installation {
            return Err(InstallationLegacySourceHistoryErrorV1::WrongStoreRole);
        }
        Ok(Self(
            StoreLegacySourceHistorySnapshotV1::capture_present_file(
                store,
                LegacyQuarantineOwnerDomainV3::Installation,
                LegacySourceHistoryKindV1::InstallationStore,
                relative_locator,
                context,
            )?,
        ))
    }

    pub(in crate::domain::installation) fn persist(
        self,
        store: &mut StoreV1,
    ) -> Result<(), InstallationLegacySourceHistoryErrorV1> {
        self.0.persist(store)?;
        Ok(())
    }
}

pub(in crate::domain::installation) struct InstallationUnavailablePreexistingLossEvidenceIssuerV1<
    'store,
> {
    provider: StoreLegacySourceHistoryProviderV1,
    store: &'store StoreV1,
    release_id: [u8; 32],
    absent_sources: Vec<LegacySourceHistorySelectorV1>,
    _not_send_or_sync: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl<'store> InstallationUnavailablePreexistingLossEvidenceIssuerV1<'store> {
    pub(in crate::domain::installation) fn prepare(
        store: &'store StoreV1,
        release_id: [u8; 32],
        absent_sources: Vec<LegacySourceHistorySelectorV1>,
    ) -> Result<Self, InstallationLegacySourceHistoryErrorV1> {
        Ok(Self {
            provider: StoreLegacySourceHistoryProviderV1::acquire(
                store,
                LegacyQuarantineOwnerDomainV3::Installation,
            )?,
            store,
            release_id,
            absent_sources,
            _not_send_or_sync: std::marker::PhantomData,
        })
    }
}

impl owner_loss_evidence_issuer_sealed::Sealed
    for InstallationUnavailablePreexistingLossEvidenceIssuerV1<'_>
{
}

impl OwnerUnavailablePreexistingLossEvidenceIssuerPortV1
    for InstallationUnavailablePreexistingLossEvidenceIssuerV1<'_>
{
    fn issue_for_foundation(
        self,
        mint: &mut FoundationOwnerEvidenceMintV1,
    ) -> Result<(), FoundationLegacyLossEvidenceErrorV1> {
        self.provider
            .issue_bound_absent_sources(self.store, mint, self.release_id, &self.absent_sources)
            .map_err(|_| FoundationLegacyLossEvidenceErrorV1::InvalidEvidenceSet)
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum InstallationLegacySourceHistoryErrorV1 {
    #[error("Installation legacy source history requires an Installation Store")]
    WrongStoreRole,
    #[error(transparent)]
    History(#[from] StoreLegacySourceHistoryErrorV1),
}
