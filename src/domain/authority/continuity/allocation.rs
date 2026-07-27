use thiserror::Error;

use super::super::identity::{AuthorityContextIdV1, StateTokenIdV1};
use super::catalog::ContinuityReferenceV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreAllocatedContinuityStateTokenV1 {
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    store_publication_clock: u64,
    expected_predecessor: Option<StateTokenIdV1>,
    successor_state_token: StateTokenIdV1,
    allocation_commitment: ContinuityReferenceV1,
}

impl StoreAllocatedContinuityStateTokenV1 {
    pub(crate) fn from_store_commitments(
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        expected_predecessor: Option<StateTokenIdV1>,
        store_publication_clock: u64,
        token_commitment: [u8; 32],
        allocation_commitment: [u8; 32],
    ) -> Result<Self, StoreAllocationBindingErrorV1> {
        let successor_state_token = StateTokenIdV1::from_digest(token_commitment);
        if store_generation == 0
            || store_publication_clock == 0
            || token_commitment == [0; 32]
            || allocation_commitment == [0; 32]
            || expected_predecessor == Some(successor_state_token)
        {
            return Err(StoreAllocationBindingErrorV1::InvalidStoreAllocation);
        }
        Ok(Self {
            context_id,
            store_generation,
            store_publication_clock,
            expected_predecessor,
            successor_state_token,
            allocation_commitment: ContinuityReferenceV1::from_digest(allocation_commitment),
        })
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn store_generation(&self) -> u64 {
        self.store_generation
    }

    pub const fn expected_predecessor(&self) -> Option<StateTokenIdV1> {
        self.expected_predecessor
    }

    pub const fn store_publication_clock(&self) -> u64 {
        self.store_publication_clock
    }

    pub const fn successor_state_token(&self) -> StateTokenIdV1 {
        self.successor_state_token
    }

    pub const fn allocation_commitment(&self) -> ContinuityReferenceV1 {
        self.allocation_commitment
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum StoreAllocationBindingErrorV1 {
    #[error("Store allocation binding is zero, stale, or reuses its predecessor token")]
    InvalidStoreAllocation,
}
