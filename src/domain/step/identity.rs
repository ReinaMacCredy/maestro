use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::identity::StoreDomainIdV1;
use crate::domain::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const STEP_ID_DOMAIN_V1: &str = "maestro.vnext.step-id.v1";
const STEP_SUBMISSION_ID_DOMAIN_V1: &str = "maestro.vnext.step-submission-id.v1";
const MAX_STABLE_STEP_KEY_BYTES_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StepScopeV1 {
    repository_id: StoreDomainIdV1,
    work_id: WorkIdV1,
}

impl StepScopeV1 {
    pub fn new(repository_id: StoreDomainIdV1, work_id: WorkIdV1) -> Self {
        Self {
            repository_id,
            work_id,
        }
    }

    pub fn repository_id(&self) -> StoreDomainIdV1 {
        self.repository_id
    }

    pub fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub(super) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.repository_id.as_bytes().to_vec()),
            CborValue::Bytes(self.work_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StepIdV1 {
    scope: StepScopeV1,
    bytes: [u8; 32],
}

impl StepIdV1 {
    pub fn new(scope: StepScopeV1, stable_key: &str) -> Result<Self, StepIdentityError> {
        validate_stable_key(stable_key)?;
        let bytes = domain_hash(
            STEP_ID_DOMAIN_V1,
            &CborValue::Array(vec![
                scope.canonical_value(),
                CborValue::Text(stable_key.to_owned()),
            ]),
        )?;
        Ok(Self { scope, bytes })
    }

    pub fn parse(scope: StepScopeV1, rendered: &str) -> Result<Self, StepIdentityError> {
        Ok(Self {
            scope,
            bytes: parse_rendered(rendered)?,
        })
    }

    pub fn from_bytes(scope: StepScopeV1, bytes: [u8; 32]) -> Result<Self, StepIdentityError> {
        require_nonzero(bytes, "Step id")?;
        Ok(Self { scope, bytes })
    }

    pub fn scope(&self) -> StepScopeV1 {
        self.scope
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

    pub(super) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.scope.canonical_value(),
            CborValue::Bytes(self.bytes.to_vec()),
        ])
    }
}

impl fmt::Debug for StepIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StepIdV1")
            .field("scope", &self.scope)
            .field("identity", &self.render())
            .finish()
    }
}

impl fmt::Display for StepIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

macro_rules! content_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn parse(rendered: &str) -> Result<Self, StepIdentityError> {
                Ok(Self(parse_rendered(rendered)?))
            }

            pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, StepIdentityError> {
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

content_identity!(StepRevisionIdV1);
content_identity!(StepGraphSnapshotIdV1);
content_identity!(StepSubmissionIdV1);

impl StepSubmissionIdV1 {
    pub fn derive(seed: &str) -> Result<Self, StepIdentityError> {
        if seed.is_empty() || !seed.is_ascii() {
            return Err(StepIdentityError::InvalidSubmissionSeed);
        }
        if seed.len() > MAX_STABLE_STEP_KEY_BYTES_V1 {
            return Err(StepIdentityError::SubmissionSeedTooLong);
        }
        Self::from_bytes(domain_hash(
            STEP_SUBMISSION_ID_DOMAIN_V1,
            &CborValue::Text(seed.to_owned()),
        )?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepIdentityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Step identity material {0} must not be the all-zero missing reference")]
    MissingReference(&'static str),
    #[error("stable Step key must be non-empty canonical ASCII")]
    InvalidStableKey,
    #[error("stable Step key exceeds the finite v1 limit of {MAX_STABLE_STEP_KEY_BYTES_V1} bytes")]
    StableKeyTooLong,
    #[error("Step Submission identity seed must be non-empty canonical ASCII")]
    InvalidSubmissionSeed,
    #[error(
        "Step Submission identity seed exceeds the finite v1 limit of {MAX_STABLE_STEP_KEY_BYTES_V1} bytes"
    )]
    SubmissionSeedTooLong,
    #[error("identity must be canonical lowercase sha256:<64hex>")]
    InvalidRenderedIdentity,
}

pub(super) fn domain_hash(domain: &str, value: &CborValue) -> Result<[u8; 32], StepIdentityError> {
    let bytes = deterministic_cbor::encode(&CborValue::Array(vec![
        CborValue::text(domain)?,
        value.clone(),
    ]))?;
    Ok(Sha256::digest(bytes).into())
}

pub(super) fn require_nonzero(
    bytes: [u8; 32],
    label: &'static str,
) -> Result<(), StepIdentityError> {
    if bytes == [0; 32] {
        Err(StepIdentityError::MissingReference(label))
    } else {
        Ok(())
    }
}

fn validate_stable_key(stable_key: &str) -> Result<(), StepIdentityError> {
    if stable_key.is_empty() || !stable_key.is_ascii() {
        return Err(StepIdentityError::InvalidStableKey);
    }
    if stable_key.len() > MAX_STABLE_STEP_KEY_BYTES_V1 {
        return Err(StepIdentityError::StableKeyTooLong);
    }
    Ok(())
}

fn parse_rendered(rendered: &str) -> Result<[u8; 32], StepIdentityError> {
    let hexadecimal = rendered
        .strip_prefix("sha256:")
        .ok_or(StepIdentityError::InvalidRenderedIdentity)?;
    if hexadecimal.len() != 64 || !hexadecimal.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(StepIdentityError::InvalidRenderedIdentity);
    }
    let mut bytes = [0; 32];
    for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
        let high = hexadecimal_nibble(pair[0]).ok_or(StepIdentityError::InvalidRenderedIdentity)?;
        let low = hexadecimal_nibble(pair[1]).ok_or(StepIdentityError::InvalidRenderedIdentity)?;
        bytes[index] = (high << 4) | low;
    }
    require_nonzero(bytes, "rendered identity")?;
    if render_digest(bytes) != rendered {
        return Err(StepIdentityError::InvalidRenderedIdentity);
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
