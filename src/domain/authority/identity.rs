use std::fmt;
use std::marker::PhantomData;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const MAX_AUTHORITY_ID_SEED_BYTES: usize = 256;

mod private {
    pub trait Sealed {}
}

pub trait AuthorityIdentityKindV1: private::Sealed {
    const DOMAIN: &'static str;
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthorityIdV1<K: AuthorityIdentityKindV1> {
    bytes: [u8; 32],
    marker: PhantomData<K>,
}

impl<K: AuthorityIdentityKindV1> AuthorityIdV1<K> {
    pub fn derive(seed: &str) -> Result<Self, AuthorityIdentityError> {
        if seed.is_empty() || seed.len() > MAX_AUTHORITY_ID_SEED_BYTES {
            return Err(AuthorityIdentityError::InvalidSeedLength);
        }
        let canonical = deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text(K::DOMAIN)?,
            CborValue::text(seed)?,
        ]))?;
        Ok(Self::from_digest(Sha256::digest(canonical).into()))
    }

    pub fn parse(rendered: &str) -> Result<Self, AuthorityIdentityError> {
        let hexadecimal = rendered
            .strip_prefix("sha256:")
            .ok_or(AuthorityIdentityError::InvalidRenderedIdentity)?;
        if hexadecimal.len() != 64 || !hexadecimal.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(AuthorityIdentityError::InvalidRenderedIdentity);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
            let high = hexadecimal_nibble(pair[0])
                .ok_or(AuthorityIdentityError::InvalidRenderedIdentity)?;
            let low = hexadecimal_nibble(pair[1])
                .ok_or(AuthorityIdentityError::InvalidRenderedIdentity)?;
            bytes[index] = (high << 4) | low;
        }
        let identity = Self::from_digest(bytes);
        if identity.render() != rendered {
            return Err(AuthorityIdentityError::InvalidRenderedIdentity);
        }
        Ok(identity)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn render(&self) -> String {
        let mut rendered = String::with_capacity(71);
        rendered.push_str("sha256:");
        for byte in self.bytes {
            use std::fmt::Write;
            write!(&mut rendered, "{byte:02x}")
                .expect("invariant: writing hexadecimal into String cannot fail");
        }
        rendered
    }

    pub(crate) fn from_digest(bytes: [u8; 32]) -> Self {
        Self {
            bytes,
            marker: PhantomData,
        }
    }
}

impl<K: AuthorityIdentityKindV1> fmt::Debug for AuthorityIdV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AuthorityIdV1")
            .field(&self.render())
            .finish()
    }
}

impl<K: AuthorityIdentityKindV1> fmt::Display for AuthorityIdV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityIdentityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Authority identity seed must contain between 1 and 256 ASCII bytes")]
    InvalidSeedLength,
    #[error("Authority identity must be canonical lowercase sha256:<64hex>")]
    InvalidRenderedIdentity,
}

macro_rules! authority_identity_kind {
    ($marker:ident, $alias:ident, $domain:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $marker {}

        impl private::Sealed for $marker {}

        impl AuthorityIdentityKindV1 for $marker {
            const DOMAIN: &'static str = $domain;
        }

        pub type $alias = AuthorityIdV1<$marker>;
    };
}

authority_identity_kind!(
    AuthorityContextIdentityKindV1,
    AuthorityContextIdV1,
    "maestro.vnext.authority-context.v1"
);
authority_identity_kind!(
    PrincipalIdentityKindV1,
    PrincipalIdV1,
    "maestro.vnext.principal.v1"
);
authority_identity_kind!(
    PrincipalBindingIdentityKindV1,
    PrincipalBindingIdV1,
    "maestro.vnext.principal-binding.v1"
);
authority_identity_kind!(
    SessionIdentityKindV1,
    SessionIdV1,
    "maestro.vnext.authority-session.v1"
);
authority_identity_kind!(
    GrantIdentityKindV1,
    GrantIdV1,
    "maestro.vnext.authority-grant.v1"
);
authority_identity_kind!(
    DelegationIdentityKindV1,
    DelegationIdV1,
    "maestro.vnext.authority-delegation.v1"
);
authority_identity_kind!(
    GenesisGrantIdentityKindV1,
    GenesisGrantIdV1,
    "maestro.vnext.authority-genesis-grant.v1"
);
authority_identity_kind!(
    MandateIdentityKindV1,
    MandateIdV1,
    "maestro.vnext.authority-mandate.v1"
);
authority_identity_kind!(
    CapacityRootIdentityKindV1,
    CapacityRootIdV1,
    "maestro.vnext.capacity-root.v1"
);
authority_identity_kind!(
    CmaWithdrawalCapacityIdentityKindV1,
    CmaWithdrawalCapacityIdV1,
    "maestro.vnext.cma-withdrawal-capacity.v1"
);
authority_identity_kind!(
    CmaBranchIdentityKindV1,
    CmaBranchIdV1,
    "maestro.vnext.cma-branch.v1"
);
authority_identity_kind!(
    SlotIdentityKindV1,
    SlotIdV1,
    "maestro.vnext.authority-slot.v1"
);
authority_identity_kind!(
    ExecutorAssertionIdentityKindV1,
    ExecutorAssertionIdV1,
    "maestro.vnext.executor-assertion.v1"
);
authority_identity_kind!(
    ActionRequestIdentityKindV1,
    ActionRequestIdV1,
    "maestro.vnext.action-request.v1"
);
authority_identity_kind!(
    IdempotencyKeyIdentityKindV1,
    IdempotencyKeyIdV1,
    "maestro.vnext.idempotency-key.v1"
);
authority_identity_kind!(
    ObservationIdentityKindV1,
    ObservationIdV1,
    "maestro.vnext.observation.v1"
);
authority_identity_kind!(
    ConsentProtocolCommitmentIdentityKindV1,
    ConsentProtocolCommitmentIdV1,
    "maestro.vnext.consent-protocol-commitment.v1"
);
authority_identity_kind!(
    TargetActionCommitmentIdentityKindV1,
    TargetActionCommitmentIdV1,
    "maestro.vnext.target-action-commitment.v1"
);
authority_identity_kind!(
    ConsentSlotCommitmentIdentityKindV1,
    ConsentSlotCommitmentIdV1,
    "maestro.vnext.consent-slot-commitment.v1"
);
authority_identity_kind!(
    InteractionClosureIdentityKindV1,
    InteractionClosureIdV1,
    "maestro.vnext.interaction-closure.v1"
);
authority_identity_kind!(
    AuthorityBasisCommitmentIdentityKindV1,
    AuthorityBasisCommitmentIdV1,
    "maestro.vnext.authority-basis-commitment.v1"
);
authority_identity_kind!(
    BootstrapMandateIssuanceBindingIdentityKindV1,
    BootstrapMandateIssuanceBindingIdV1,
    "maestro.vnext.bootstrap-mandate-issuance-binding.v1"
);
authority_identity_kind!(
    StateTokenIdentityKindV1,
    StateTokenIdV1,
    "maestro.vnext.authority-state-token.v1"
);
authority_identity_kind!(
    AuthorizationReceiptIdentityKindV1,
    AuthorizationReceiptIdV1,
    "maestro.vnext.authorization-receipt.v1"
);
authority_identity_kind!(
    ActionResultIdentityKindV1,
    ActionResultIdV1,
    "maestro.vnext.action-result.v1"
);
authority_identity_kind!(
    EffectReferenceIdentityKindV1,
    EffectReferenceIdV1,
    "maestro.vnext.effect-reference.v1"
);
authority_identity_kind!(
    AuthorityContinuityManifestIdentityKindV1,
    AuthorityContinuityManifestIdV1,
    "maestro.vnext.authority-continuity-manifest.v1"
);

fn hexadecimal_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
