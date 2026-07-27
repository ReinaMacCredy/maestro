use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MigrationDigestV1([u8; 32]);

impl MigrationDigestV1 {
    pub fn digest_bytes(bytes: &[u8]) -> Result<Self, MigrationIdentityErrorV1> {
        Self::from_digest(Sha256::digest(bytes).into())
    }

    pub fn identify(
        domain: &'static [u8],
        value: &CborValue,
    ) -> Result<Self, MigrationIdentityErrorV1> {
        if !domain.ends_with(&[0]) {
            return Err(MigrationIdentityErrorV1::InvalidDomainSeparator);
        }
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(deterministic_cbor::encode(value)?);
        Self::from_digest(hasher.finalize().into())
    }

    pub fn from_digest(bytes: [u8; 32]) -> Result<Self, MigrationIdentityErrorV1> {
        if bytes == [0; 32] {
            return Err(MigrationIdentityErrorV1::ZeroDigest);
        }
        Ok(Self(bytes))
    }

    pub fn parse_hex(value: &str) -> Result<Self, MigrationIdentityErrorV1> {
        if value.len() != 64
            || !value
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(MigrationIdentityErrorV1::InvalidHexadecimal);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hexadecimal_nibble(pair[0])? << 4) | hexadecimal_nibble(pair[1])?;
        }
        Self::from_digest(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn render_hex(&self) -> String {
        let mut rendered = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write;
            write!(&mut rendered, "{byte:02x}")
                .expect("invariant: hexadecimal rendering into String cannot fail");
        }
        rendered
    }

    pub(crate) fn canonical_value(self) -> CborValue {
        CborValue::Bytes(self.0.to_vec())
    }
}

impl fmt::Debug for MigrationDigestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MigrationDigestV1")
            .field(&self.render_hex())
            .finish()
    }
}

impl fmt::Display for MigrationDigestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render_hex())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MigrationIdentityErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("migration identity domain separator must end in NUL")]
    InvalidDomainSeparator,
    #[error("migration digest must be 64 lowercase hexadecimal characters")]
    InvalidHexadecimal,
    #[error("migration digest must not be zero")]
    ZeroDigest,
}

fn hexadecimal_nibble(byte: u8) -> Result<u8, MigrationIdentityErrorV1> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(MigrationIdentityErrorV1::InvalidHexadecimal),
    }
}
