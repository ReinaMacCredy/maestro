use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub const CLAIM_ID_DOMAIN_V1: &str = "maestro.vnext.evidence.claim-id.v1";

macro_rules! evidence_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn parse(rendered: &str) -> Result<Self, EvidenceIdentityError> {
                Ok(Self(parse_rendered(rendered)?))
            }

            pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, EvidenceIdentityError> {
                require_nonzero(bytes, stringify!($name))?;
                Ok(Self(bytes))
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub fn into_bytes(self) -> [u8; 32] {
                self.0
            }

            pub fn render(&self) -> String {
                render_digest(self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.render())
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.render())
            }
        }
    };
}

evidence_identity!(ClaimIdV1);
evidence_identity!(ObservationRecordIdV1);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EvidenceIdentityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Evidence identity material {0} must not be the all-zero missing reference")]
    MissingReference(&'static str),
    #[error("Evidence identity must be canonical lowercase sha256:<64hex>")]
    InvalidRenderedIdentity,
}

pub(super) fn derive_claim_id(value: &CborValue) -> Result<ClaimIdV1, EvidenceIdentityError> {
    ClaimIdV1::from_bytes(domain_hash(CLAIM_ID_DOMAIN_V1, value)?)
}

pub(super) fn domain_hash(
    domain: &str,
    value: &CborValue,
) -> Result<[u8; 32], EvidenceIdentityError> {
    let bytes = deterministic_cbor::encode(&CborValue::Array(vec![
        CborValue::text(domain)?,
        value.clone(),
    ]))?;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    require_nonzero(digest, "derived digest")?;
    Ok(digest)
}

pub(super) fn require_nonzero(
    bytes: [u8; 32],
    label: &'static str,
) -> Result<(), EvidenceIdentityError> {
    if bytes == [0; 32] {
        Err(EvidenceIdentityError::MissingReference(label))
    } else {
        Ok(())
    }
}

fn parse_rendered(rendered: &str) -> Result<[u8; 32], EvidenceIdentityError> {
    let hexadecimal = rendered
        .strip_prefix("sha256:")
        .ok_or(EvidenceIdentityError::InvalidRenderedIdentity)?;
    if hexadecimal.len() != 64 || !hexadecimal.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(EvidenceIdentityError::InvalidRenderedIdentity);
    }
    let mut bytes = [0; 32];
    for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
        let high =
            hexadecimal_nibble(pair[0]).ok_or(EvidenceIdentityError::InvalidRenderedIdentity)?;
        let low =
            hexadecimal_nibble(pair[1]).ok_or(EvidenceIdentityError::InvalidRenderedIdentity)?;
        bytes[index] = (high << 4) | low;
    }
    require_nonzero(bytes, "rendered identity")?;
    if render_digest(bytes) != rendered {
        return Err(EvidenceIdentityError::InvalidRenderedIdentity);
    }
    Ok(bytes)
}

fn render_digest(bytes: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

fn hexadecimal_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}
