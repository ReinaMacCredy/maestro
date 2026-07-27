use thiserror::Error;

use crate::domain::vnext::migration::runtime::{
    InactiveImportErrorV1, InactiveStoreImportReceiptV1, InactiveStoreImportRequestV1,
};
use crate::domain::vnext::persistence::{StoreError, StoreStateV1, StoreV1};

pub fn import_inactive_store(
    store: &mut StoreV1,
    request: &InactiveStoreImportRequestV1,
    sealed_backup: &[u8],
) -> Result<InactiveStoreImportReceiptV1, InactiveImportOperationErrorV1> {
    request.verify_sealed_backup(sealed_backup)?;
    let (pre_state, pre_revision) = store.state()?;
    if pre_state != StoreStateV1::Inactive || store.active_head()?.is_some() {
        return Err(InactiveImportOperationErrorV1::DestinationNotInactive);
    }
    let candidate = store.import_inactive(sealed_backup)?;
    let (post_state, post_revision) = store.state()?;
    if post_state != StoreStateV1::Inactive || store.active_head()?.is_some() {
        return Err(InactiveImportOperationErrorV1::ImportActivatedStore);
    }
    Ok(InactiveStoreImportReceiptV1::from_candidate(
        request,
        &candidate,
        sealed_backup,
        pre_revision,
        post_revision,
    )?)
}

#[derive(Debug, Error)]
pub enum InactiveImportOperationErrorV1 {
    #[error("inactive import destination was not inactive and headless")]
    DestinationNotInactive,
    #[error("inactive import unexpectedly activated the destination Store")]
    ImportActivatedStore,
    #[error(transparent)]
    Contract(#[from] InactiveImportErrorV1),
    #[error(transparent)]
    Store(#[from] StoreError),
}
