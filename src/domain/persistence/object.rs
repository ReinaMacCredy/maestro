use thiserror::Error;

use crate::domain::identity::{ManifestIdentityV1, SchemaIdV1, StoreObjectIdV1, derive_identity};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub const STORE_OBJECT_VERSION_V1: u64 = 1;
pub const MAX_STORE_OBJECT_REFERENCES: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreObjectV1 {
    schema_id: SchemaIdV1,
    value: CborValue,
    references: Vec<StoreObjectIdV1>,
    id: StoreObjectIdV1,
    canonical_bytes: Vec<u8>,
}

impl StoreObjectV1 {
    pub fn new(
        schema_id: SchemaIdV1,
        value: CborValue,
        references: Vec<StoreObjectIdV1>,
    ) -> Result<Self, StoreObjectError> {
        if references.len() > MAX_STORE_OBJECT_REFERENCES {
            return Err(StoreObjectError::TooManyReferences);
        }
        if references.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(StoreObjectError::ReferencesNotStrictlySorted);
        }
        let canonical_value = canonical_value(&schema_id, &value, &references);
        let canonical_bytes = deterministic_cbor::encode(&canonical_value)?;
        let id = derive_identity(&canonical_value)?;
        if references.contains(&id) {
            return Err(StoreObjectError::SelfReference);
        }
        Ok(Self {
            schema_id,
            value,
            references,
            id,
            canonical_bytes,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, StoreObjectError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(StoreObjectError::InvalidShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Bytes(schema),
            value,
            CborValue::Array(refs),
        ] = fields.as_slice()
        else {
            return Err(StoreObjectError::InvalidShape);
        };
        if *version != STORE_OBJECT_VERSION_V1 {
            return Err(StoreObjectError::UnknownVersion(*version));
        }
        let schema_id = identity_from_bytes(schema)?;
        let mut references = Vec::with_capacity(refs.len());
        for reference in refs {
            let CborValue::Bytes(bytes) = reference else {
                return Err(StoreObjectError::InvalidShape);
            };
            references.push(identity_from_bytes(bytes)?);
        }
        let object = Self::new(schema_id, value.clone(), references)?;
        if object.canonical_bytes != bytes {
            return Err(StoreObjectError::NonCanonicalBytes);
        }
        Ok(object)
    }

    pub fn id(&self) -> StoreObjectIdV1 {
        self.id
    }

    pub fn schema_id(&self) -> SchemaIdV1 {
        self.schema_id
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn references(&self) -> &[StoreObjectIdV1] {
        &self.references
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

fn canonical_value(
    schema_id: &SchemaIdV1,
    value: &CborValue,
    references: &[StoreObjectIdV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(STORE_OBJECT_VERSION_V1),
        CborValue::Bytes(schema_id.as_bytes().to_vec()),
        value.clone(),
        CborValue::Array(
            references
                .iter()
                .map(|reference| CborValue::Bytes(reference.as_bytes().to_vec()))
                .collect(),
        ),
    ])
}

fn identity_from_bytes<K>(bytes: &[u8]) -> Result<ManifestIdentityV1<K>, StoreObjectError>
where
    K: crate::domain::identity::IdentityKindV1,
{
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| StoreObjectError::InvalidIdentityLength)?;
    Ok(ManifestIdentityV1::from_digest(digest))
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StoreObjectError {
    #[error("Store Object has an invalid canonical shape")]
    InvalidShape,
    #[error("Store Object identity bytes must contain exactly 32 bytes")]
    InvalidIdentityLength,
    #[error("Store Object uses unsupported version {0}")]
    UnknownVersion(u64),
    #[error("Store Object references exceed the finite v1 limit")]
    TooManyReferences,
    #[error("Store Object references must be strictly identity-sorted and unique")]
    ReferencesNotStrictlySorted,
    #[error("Store Object cannot reference itself")]
    SelfReference,
    #[error("Store Object bytes are not the exact canonical encoding")]
    NonCanonicalBytes,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
}
