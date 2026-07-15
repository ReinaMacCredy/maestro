use thiserror::Error;

use crate::domain::vnext::identity::{StoreDomainIdV1, derive_identity};
use crate::foundation::core::deterministic_cbor::CborValue;

const MAX_STABLE_DOMAIN_KEY_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreRoleV1 {
    Repository,
    Installation,
}

impl StoreRoleV1 {
    pub const ALL: [Self; 2] = [Self::Repository, Self::Installation];

    pub const fn tag(self) -> u64 {
        match self {
            Self::Repository => 1,
            Self::Installation => 2,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Installation => "installation",
        }
    }

    pub(crate) fn from_tag(tag: u64) -> Result<Self, StoreDomainError> {
        match tag {
            1 => Ok(Self::Repository),
            2 => Ok(Self::Installation),
            _ => Err(StoreDomainError::UnknownRole(tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreDomainV1 {
    role: StoreRoleV1,
    id: StoreDomainIdV1,
}

impl StoreDomainV1 {
    pub fn derive(role: StoreRoleV1, stable_key: &[u8]) -> Result<Self, StoreDomainError> {
        if stable_key.is_empty() {
            return Err(StoreDomainError::EmptyStableKey);
        }
        if stable_key.len() > MAX_STABLE_DOMAIN_KEY_BYTES {
            return Err(StoreDomainError::StableKeyTooLarge);
        }
        let id = derive_identity(&CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Unsigned(role.tag()),
            CborValue::Bytes(stable_key.to_vec()),
        ]))?;
        Ok(Self { role, id })
    }

    pub fn from_identity(role: StoreRoleV1, id: StoreDomainIdV1) -> Self {
        Self { role, id }
    }

    pub fn role(&self) -> StoreRoleV1 {
        self.role
    }

    pub fn id(&self) -> StoreDomainIdV1 {
        self.id
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StoreDomainError {
    #[error("Store domain stable key must not be empty")]
    EmptyStableKey,
    #[error("Store domain stable key exceeds the finite v1 limit")]
    StableKeyTooLarge,
    #[error("unknown Store role tag {0}")]
    UnknownRole(u64),
    #[error(transparent)]
    Identity(#[from] crate::domain::vnext::identity::IdentityError),
}
