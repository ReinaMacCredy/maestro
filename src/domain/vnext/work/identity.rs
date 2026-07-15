use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const MAX_WORK_ID_SEED_BYTES: usize = 256;

macro_rules! work_identity {
    ($name:ident, $domain:literal) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            bytes: [u8; 32],
        }

        impl $name {
            pub fn derive(seed: &str) -> Result<Self, WorkIdentityError> {
                if seed.is_empty() || seed.len() > MAX_WORK_ID_SEED_BYTES || !seed.is_ascii() {
                    return Err(WorkIdentityError::InvalidSeedLength);
                }
                let canonical = deterministic_cbor::encode(&CborValue::Array(vec![
                    CborValue::text($domain)?,
                    CborValue::text(seed)?,
                ]))?;
                Ok(Self {
                    bytes: Sha256::digest(canonical).into(),
                })
            }

            pub fn parse(rendered: &str) -> Result<Self, WorkIdentityError> {
                let hexadecimal = rendered
                    .strip_prefix("sha256:")
                    .ok_or(WorkIdentityError::InvalidRenderedIdentity)?;
                if hexadecimal.len() != 64
                    || !hexadecimal.as_bytes().iter().all(u8::is_ascii_hexdigit)
                {
                    return Err(WorkIdentityError::InvalidRenderedIdentity);
                }
                let mut bytes = [0_u8; 32];
                for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
                    let high = hexadecimal_nibble(pair[0])
                        .ok_or(WorkIdentityError::InvalidRenderedIdentity)?;
                    let low = hexadecimal_nibble(pair[1])
                        .ok_or(WorkIdentityError::InvalidRenderedIdentity)?;
                    bytes[index] = (high << 4) | low;
                }
                let identity = Self { bytes };
                if identity.render() != rendered {
                    return Err(WorkIdentityError::InvalidRenderedIdentity);
                }
                Ok(identity)
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.bytes
            }

            pub fn into_bytes(self) -> [u8; 32] {
                self.bytes
            }

            pub fn render(&self) -> String {
                render_digest(self.bytes)
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

work_identity!(WorkIdV1, "maestro.vnext.work-id.v1");
work_identity!(WorkSubmissionIdV1, "maestro.vnext.work-submission-id.v1");
work_identity!(WorkRelationIdV1, "maestro.vnext.work-relation-id.v1");
work_identity!(WorkRequirementIdV1, "maestro.vnext.work-requirement-id.v1");

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkIdentityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Work identity seed must contain between 1 and 256 ASCII bytes")]
    InvalidSeedLength,
    #[error("Work identity must be canonical lowercase sha256:<64hex>")]
    InvalidRenderedIdentity,
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

fn hexadecimal_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
