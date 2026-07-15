use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::closed::{ActionAuthorityBasisKindV1, AuthorityContextKindV1};
use super::identity::{
    AuthorityContextIdV1, CmaBranchIdV1, ExecutorAssertionIdV1, GenesisGrantIdV1, GrantIdV1,
    MandateIdV1, PrincipalBindingIdV1, SessionIdV1, SlotIdV1,
};

const MAX_CONTEXT_TEXT_BYTES: usize = 256;
const MAX_REQUIRED_MANDATES: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityContextV1 {
    RepositoryAuthorityContext(RepositoryAuthorityContextV1),
    InstallationAuthorityContext(InstallationAuthorityContextV1),
}

impl AuthorityContextV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.authority-context-value.v1";

    pub fn repository(
        context_id: AuthorityContextIdV1,
        repository_installation_id: &str,
        store_generation: u64,
        authority_epoch: u64,
        trust_root_revision: u64,
    ) -> Result<Self, AuthorityContextError> {
        Ok(Self::RepositoryAuthorityContext(
            RepositoryAuthorityContextV1 {
                context_id,
                repository_installation_id: bounded_text(repository_installation_id)?,
                store_generation,
                authority_epoch,
                trust_root_revision: nonzero_revision(trust_root_revision)?,
            },
        ))
    }

    pub fn installation(
        context_id: AuthorityContextIdV1,
        installation_id: &str,
        protected_realm: &str,
        store_generation: u64,
        authority_epoch: u64,
        trust_root_revision: u64,
        locator_binding_revision: u64,
    ) -> Result<Self, AuthorityContextError> {
        Ok(Self::InstallationAuthorityContext(
            InstallationAuthorityContextV1 {
                context_id,
                installation_id: bounded_text(installation_id)?,
                protected_realm: bounded_text(protected_realm)?,
                store_generation,
                authority_epoch,
                trust_root_revision: nonzero_revision(trust_root_revision)?,
                locator_binding_revision: nonzero_revision(locator_binding_revision)?,
            },
        ))
    }

    pub const fn kind(&self) -> AuthorityContextKindV1 {
        match self {
            Self::RepositoryAuthorityContext(_) => {
                AuthorityContextKindV1::RepositoryAuthorityContext
            }
            Self::InstallationAuthorityContext(_) => {
                AuthorityContextKindV1::InstallationAuthorityContext
            }
        }
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        match self {
            Self::RepositoryAuthorityContext(context) => context.context_id,
            Self::InstallationAuthorityContext(context) => context.context_id,
        }
    }

    pub const fn store_generation(&self) -> u64 {
        match self {
            Self::RepositoryAuthorityContext(context) => context.store_generation,
            Self::InstallationAuthorityContext(context) => context.store_generation,
        }
    }

    pub const fn authority_epoch(&self) -> u64 {
        match self {
            Self::RepositoryAuthorityContext(context) => context.authority_epoch,
            Self::InstallationAuthorityContext(context) => context.authority_epoch,
        }
    }

    pub const fn trust_root_revision(&self) -> u64 {
        match self {
            Self::RepositoryAuthorityContext(context) => context.trust_root_revision,
            Self::InstallationAuthorityContext(context) => context.trust_root_revision,
        }
    }

    pub const fn schema_domain(&self) -> &'static str {
        Self::SCHEMA_DOMAIN
    }

    pub(crate) fn continue_at_store_generation(&self, store_generation: u64) -> Self {
        match self {
            Self::RepositoryAuthorityContext(context) => {
                Self::RepositoryAuthorityContext(RepositoryAuthorityContextV1 {
                    context_id: context.context_id,
                    repository_installation_id: context.repository_installation_id.clone(),
                    store_generation,
                    authority_epoch: context.authority_epoch,
                    trust_root_revision: context.trust_root_revision,
                })
            }
            Self::InstallationAuthorityContext(context) => {
                Self::InstallationAuthorityContext(InstallationAuthorityContextV1 {
                    context_id: context.context_id,
                    installation_id: context.installation_id.clone(),
                    protected_realm: context.protected_realm.clone(),
                    store_generation,
                    authority_epoch: context.authority_epoch,
                    trust_root_revision: context.trust_root_revision,
                    locator_binding_revision: context.locator_binding_revision,
                })
            }
        }
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        let mut fields = vec![CborValue::text(Self::SCHEMA_DOMAIN)?];
        match self {
            Self::RepositoryAuthorityContext(context) => {
                fields.extend([
                    CborValue::Unsigned(1),
                    CborValue::Bytes(context.context_id.as_bytes().to_vec()),
                    CborValue::text(&context.repository_installation_id)?,
                    CborValue::Unsigned(context.store_generation),
                    CborValue::Unsigned(context.authority_epoch),
                    CborValue::Unsigned(context.trust_root_revision),
                ]);
            }
            Self::InstallationAuthorityContext(context) => {
                fields.extend([
                    CborValue::Unsigned(2),
                    CborValue::Bytes(context.context_id.as_bytes().to_vec()),
                    CborValue::text(&context.installation_id)?,
                    CborValue::text(&context.protected_realm)?,
                    CborValue::Unsigned(context.store_generation),
                    CborValue::Unsigned(context.authority_epoch),
                    CborValue::Unsigned(context.trust_root_revision),
                    CborValue::Unsigned(context.locator_binding_revision),
                ]);
            }
        }
        Ok(CborValue::Array(fields))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryAuthorityContextV1 {
    context_id: AuthorityContextIdV1,
    repository_installation_id: String,
    store_generation: u64,
    authority_epoch: u64,
    trust_root_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationAuthorityContextV1 {
    context_id: AuthorityContextIdV1,
    installation_id: String,
    protected_realm: String,
    store_generation: u64,
    authority_epoch: u64,
    trust_root_revision: u64,
    locator_binding_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionAuthorityBasisV1 {
    OrdinaryLiveRuntime(OrdinaryAuthorityBasisV1),
    BootstrapControlG0(BootstrapControlG0AuthorityBasisV1),
    ContinuityMaintenance(ContinuityMaintenanceAuthorityBasisV1),
}

impl ActionAuthorityBasisV1 {
    pub const fn kind(&self) -> ActionAuthorityBasisKindV1 {
        match self {
            Self::OrdinaryLiveRuntime(_) => ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            Self::BootstrapControlG0(_) => ActionAuthorityBasisKindV1::BootstrapControlG0,
            Self::ContinuityMaintenance(_) => ActionAuthorityBasisKindV1::ContinuityMaintenance,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrdinaryAuthorityBasisV1 {
    pub binding_id: PrincipalBindingIdV1,
    pub session_id: SessionIdV1,
    pub terminal_grant_id: GrantIdV1,
    mandate_ids: Vec<MandateIdV1>,
}

impl OrdinaryAuthorityBasisV1 {
    pub fn new(
        binding_id: PrincipalBindingIdV1,
        session_id: SessionIdV1,
        terminal_grant_id: GrantIdV1,
        mandate_ids: Vec<MandateIdV1>,
    ) -> Result<Self, AuthorityContextError> {
        if mandate_ids.len() > MAX_REQUIRED_MANDATES {
            return Err(AuthorityContextError::TooManyMandates);
        }
        let mut canonical = mandate_ids;
        canonical.sort_unstable();
        canonical.dedup();
        Ok(Self {
            binding_id,
            session_id,
            terminal_grant_id,
            mandate_ids: canonical,
        })
    }

    pub fn mandate_ids(&self) -> &[MandateIdV1] {
        &self.mandate_ids
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapControlG0AuthorityBasisV1 {
    pub binding_id: PrincipalBindingIdV1,
    pub session_id: SessionIdV1,
    pub genesis_grant_id: GenesisGrantIdV1,
}

impl BootstrapControlG0AuthorityBasisV1 {
    pub const fn new(
        binding_id: PrincipalBindingIdV1,
        session_id: SessionIdV1,
        genesis_grant_id: GenesisGrantIdV1,
    ) -> Self {
        Self {
            binding_id,
            session_id,
            genesis_grant_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuityMaintenanceAuthorityBasisV1 {
    pub cma_branch_id: CmaBranchIdV1,
    pub slot_id: SlotIdV1,
    pub executor_assertion_id: ExecutorAssertionIdV1,
}

impl ContinuityMaintenanceAuthorityBasisV1 {
    pub const fn new(
        cma_branch_id: CmaBranchIdV1,
        slot_id: SlotIdV1,
        executor_assertion_id: ExecutorAssertionIdV1,
    ) -> Self {
        Self {
            cma_branch_id,
            slot_id,
            executor_assertion_id,
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityContextError {
    #[error("Authority context text must contain between 1 and 256 ASCII bytes")]
    InvalidContextText,
    #[error("Authority context revision must be nonzero")]
    ZeroRevision,
    #[error("ordinary authority basis exceeds the finite Mandate bound")]
    TooManyMandates,
}

fn bounded_text(value: &str) -> Result<String, AuthorityContextError> {
    if value.is_empty() || value.len() > MAX_CONTEXT_TEXT_BYTES || !value.is_ascii() {
        return Err(AuthorityContextError::InvalidContextText);
    }
    Ok(value.to_owned())
}

fn nonzero_revision(value: u64) -> Result<u64, AuthorityContextError> {
    if value == 0 {
        return Err(AuthorityContextError::ZeroRevision);
    }
    Ok(value)
}
