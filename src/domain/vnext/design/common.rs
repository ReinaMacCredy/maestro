use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use crate::domain::vnext::authority::{
    ActionAuthorityBasisKindV1, AuthorityContextIdV1, AuthorizationReceiptV1, StateTokenIdV1,
};
use crate::domain::vnext::authority::{
    ActionOutcomeV1, ActionRequestIdV1, ActionResultIdV1, ActionResultV1, AuthorizationReceiptIdV1,
};
use crate::domain::vnext::identity::{
    StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const MAX_LOCAL_ID_SEED_BYTES_V1: usize = 256;

macro_rules! seeded_identity {
    ($name:ident, $domain:literal) => {
        #[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(seed: impl Into<String>) -> Result<Self, LocalIdentityErrorV1> {
                let seed = seed.into();
                validate_seed(&seed)?;
                Ok(Self(seed))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn canonical_value(&self) -> CborValue {
                CborValue::Text(self.0.clone())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

seeded_identity!(DecisionIdV1, "maestro.vnext.decision-id.v1");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactRecordRefV1([u8; 32]);

impl ExactRecordRefV1 {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) fn canonical_value(self) -> CborValue {
        CborValue::Bytes(self.0.to_vec())
    }
}

macro_rules! opaque_reference {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(ExactRecordRefV1);

        impl $name {
            pub const fn exact_ref(&self) -> &ExactRecordRefV1 {
                &self.0
            }

            pub(crate) fn canonical_value(self) -> CborValue {
                self.0.canonical_value()
            }
        }
    };
}

opaque_reference!(AuthorityRequirementRefV1);
opaque_reference!(AuthorizationReceiptRefV1);
opaque_reference!(EvidenceRefV1);
opaque_reference!(SupersessionAuthorizationReceiptRefV1);

impl From<ExactRecordRefV1> for EvidenceRefV1 {
    fn from(value: ExactRecordRefV1) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedActionAuditV1 {
    action_request_id: ActionRequestIdV1,
    action_result_id: ActionResultIdV1,
    primary_receipt_id: AuthorizationReceiptIdV1,
    transition_guard_object_id: StoreObjectIdV1,
    store_domain_id: StoreDomainIdV1,
    store_head_id: StoreHeadIdV1,
    store_generation_id: StoreGenerationIdV1,
}

impl CommittedActionAuditV1 {
    pub const fn action_request_id(&self) -> ActionRequestIdV1 {
        self.action_request_id
    }

    pub const fn action_result_id(&self) -> ActionResultIdV1 {
        self.action_result_id
    }

    pub const fn primary_receipt_id(&self) -> AuthorizationReceiptIdV1 {
        self.primary_receipt_id
    }

    pub const fn transition_guard_object_id(&self) -> StoreObjectIdV1 {
        self.transition_guard_object_id
    }

    pub const fn store_domain_id(&self) -> StoreDomainIdV1 {
        self.store_domain_id
    }

    pub const fn store_head_id(&self) -> StoreHeadIdV1 {
        self.store_head_id
    }

    pub const fn store_generation_id(&self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.action_request_id.as_bytes().to_vec()),
            CborValue::Bytes(self.action_result_id.as_bytes().to_vec()),
            CborValue::Bytes(self.primary_receipt_id.as_bytes().to_vec()),
            CborValue::Bytes(self.transition_guard_object_id.as_bytes().to_vec()),
            CborValue::Bytes(self.store_domain_id.as_bytes().to_vec()),
            CborValue::Bytes(self.store_head_id.as_bytes().to_vec()),
            CborValue::Bytes(self.store_generation_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Debug)]
pub struct AdmittedCommittedActionV1 {
    audit: CommittedActionAuditV1,
}

impl AdmittedCommittedActionV1 {
    pub(crate) fn from_store_commit(
        result: &ActionResultV1,
        transition_guard_object_id: StoreObjectIdV1,
        store_domain_id: StoreDomainIdV1,
        store_head_id: StoreHeadIdV1,
        store_generation_id: StoreGenerationIdV1,
    ) -> Result<Self, CommittedActionAdmissionErrorV1> {
        if result.outcome() != ActionOutcomeV1::Committed {
            return Err(CommittedActionAdmissionErrorV1::ResultNotCommitted);
        }
        let receipt = result
            .authorization_receipt()
            .ok_or(CommittedActionAdmissionErrorV1::MissingPrimaryReceipt)?;
        if receipt.request_id() != result.request_id() {
            return Err(CommittedActionAdmissionErrorV1::PrimaryReceiptRequestMismatch);
        }
        Ok(Self {
            audit: CommittedActionAuditV1 {
                action_request_id: result.request_id(),
                action_result_id: result.id(),
                primary_receipt_id: receipt.id(),
                transition_guard_object_id,
                store_domain_id,
                store_head_id,
                store_generation_id,
            },
        })
    }

    pub(crate) const fn audit(&self) -> &CommittedActionAuditV1 {
        &self.audit
    }

    #[cfg(test)]
    pub(crate) fn fixture(seed: &str) -> Self {
        let rendered = |byte: u8| format!("sha256:{}", format!("{byte:02x}").repeat(32));
        let request_id = ActionRequestIdV1::derive(&format!("{seed}-request"))
            .expect("test Action Request identity");
        let receipt = AuthorizationReceiptV1::new(
            request_id,
            AuthorityContextIdV1::derive(&format!("{seed}-context"))
                .expect("test Authority Context identity"),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive(&format!("{seed}-prior-state")).expect("test prior state token"),
            StateTokenIdV1::derive(&format!("{seed}-resulting-state"))
                .expect("test resulting state token"),
        )
        .expect("test Authorization Receipt");
        let result =
            ActionResultV1::new(request_id, ActionOutcomeV1::Committed, Some(receipt), None)
                .expect("test committed Action Result");
        Self::from_store_commit(
            &result,
            StoreObjectIdV1::parse(&rendered(204)).expect("test transition guard object"),
            StoreDomainIdV1::parse(&rendered(201)).expect("test Store domain identity"),
            StoreHeadIdV1::parse(&rendered(202)).expect("test Store head identity"),
            StoreGenerationIdV1::parse(&rendered(203)).expect("test Store generation identity"),
        )
        .expect("test admitted Store commit")
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CommittedActionAdmissionErrorV1 {
    #[error("authority-sensitive action requires a committed Action Result")]
    ResultNotCommitted,
    #[error("committed Action Result lacks its primary Authorization Receipt")]
    MissingPrimaryReceipt,
    #[error("primary Authorization Receipt does not bind the committed Action Request")]
    PrimaryReceiptRequestMismatch,
}

macro_rules! digest_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_identity!(AlternativeIdV1);
digest_identity!(DecisionRevisionIdV1);
digest_identity!(DecisionSupersessionIdV1);
digest_identity!(DecisionBatchReceiptIdV1);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LocalIdentityErrorV1 {
    #[error("local semantic identity seed must contain between 1 and 256 ASCII bytes")]
    InvalidSeed,
}

pub(crate) fn canonical_hash_v1(
    domain: &'static str,
    value: CborValue,
) -> Result<[u8; 32], CborError> {
    let bytes =
        deterministic_cbor::encode(&CborValue::Array(vec![CborValue::text(domain)?, value]))?;
    Ok(Sha256::digest(bytes).into())
}

pub(crate) fn bytes_hash_v1(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(crate) fn optional_digest_v1(value: Option<&[u8; 32]>) -> CborValue {
    CborValue::optional(value.map(|digest| CborValue::Bytes(digest.to_vec())))
}

fn validate_seed(seed: &str) -> Result<(), LocalIdentityErrorV1> {
    if seed.is_empty() || seed.len() > MAX_LOCAL_ID_SEED_BYTES_V1 || !seed.is_ascii() {
        return Err(LocalIdentityErrorV1::InvalidSeed);
    }
    Ok(())
}
