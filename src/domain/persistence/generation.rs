use thiserror::Error;

use crate::domain::identity::{
    ContractRootIdV1, DescriptorIdV1, IdentityKindV1, ManifestIdV1, ManifestIdentityV1, SchemaIdV1,
    StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1, derive_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{StoreDomainV1, StoreRoleV1};

pub const STORE_GENERATION_VERSION_V1: u64 = 1;
pub const STORE_HEAD_VERSION_V1: u64 = 1;
pub const MAX_GENERATION_ROOTS: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreCompatibilityV1 {
    writer_compatibility_manifest_id: ManifestIdV1,
    association_schema_id: SchemaIdV1,
    finality_edge_manifest_id: ManifestIdV1,
    schema_read_write_set_descriptor_id: DescriptorIdV1,
    writer_protocol_epoch_id: DescriptorIdV1,
    migration_epoch_id: DescriptorIdV1,
}

impl StoreCompatibilityV1 {
    pub fn stage0_successor() -> Result<Self, crate::domain::identity::IdentityError> {
        Ok(Self::new(
            ManifestIdV1::parse(
                "sha256:67cec9563ec7bf576772caa37f99c7f984a680a09fe7cd0e0f5bd6ff0ba6b284",
            )?,
            SchemaIdV1::parse(
                "sha256:fddd9d43b7f8662187b834a64ef5fb0ba96b2182b6218c1a2c1b5aaca0e26808",
            )?,
            ManifestIdV1::parse(
                "sha256:026b61dd18923e40917167af14737124ec11b1cabdb69fdb2422bb50d4a80466",
            )?,
            DescriptorIdV1::parse(
                "sha256:ef6260aec968499ca90710ea2accfc9c62c848e98b5eebfd995961eefa7d24db",
            )?,
            DescriptorIdV1::parse(
                "sha256:f1966ce6ea8cc257caaba7afd3a09cccdfee5d95e133cf72d88f93b33fbf577c",
            )?,
            DescriptorIdV1::parse(
                "sha256:dbf2dce1a633daad6374645e24f181eac618ab0adb1da68a4ab1f92278c9761d",
            )?,
        ))
    }

    pub fn is_stage0_successor(&self) -> bool {
        Self::stage0_successor().is_ok_and(|current| current == *self)
    }

    pub fn new(
        writer_compatibility_manifest_id: ManifestIdV1,
        association_schema_id: SchemaIdV1,
        finality_edge_manifest_id: ManifestIdV1,
        schema_read_write_set_descriptor_id: DescriptorIdV1,
        writer_protocol_epoch_id: DescriptorIdV1,
        migration_epoch_id: DescriptorIdV1,
    ) -> Self {
        Self {
            writer_compatibility_manifest_id,
            association_schema_id,
            finality_edge_manifest_id,
            schema_read_write_set_descriptor_id,
            writer_protocol_epoch_id,
            migration_epoch_id,
        }
    }

    pub fn writer_compatibility_manifest_id(&self) -> ManifestIdV1 {
        self.writer_compatibility_manifest_id
    }

    pub fn association_schema_id(&self) -> SchemaIdV1 {
        self.association_schema_id
    }

    pub fn finality_edge_manifest_id(&self) -> ManifestIdV1 {
        self.finality_edge_manifest_id
    }

    pub fn schema_read_write_set_descriptor_id(&self) -> DescriptorIdV1 {
        self.schema_read_write_set_descriptor_id
    }

    pub fn writer_protocol_epoch_id(&self) -> DescriptorIdV1 {
        self.writer_protocol_epoch_id
    }

    pub fn migration_epoch_id(&self) -> DescriptorIdV1 {
        self.migration_epoch_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.writer_compatibility_manifest_id.as_bytes().to_vec()),
            CborValue::Bytes(self.association_schema_id.as_bytes().to_vec()),
            CborValue::Bytes(self.finality_edge_manifest_id.as_bytes().to_vec()),
            CborValue::Bytes(self.schema_read_write_set_descriptor_id.as_bytes().to_vec()),
            CborValue::Bytes(self.writer_protocol_epoch_id.as_bytes().to_vec()),
            CborValue::Bytes(self.migration_epoch_id.as_bytes().to_vec()),
        ])
    }

    fn decode(value: &CborValue) -> Result<Self, GenerationError> {
        let CborValue::Array(fields) = value else {
            return Err(GenerationError::InvalidShape);
        };
        let [
            CborValue::Bytes(manifest),
            CborValue::Bytes(association),
            CborValue::Bytes(finality),
            CborValue::Bytes(cohort),
            CborValue::Bytes(writer),
            CborValue::Bytes(migration),
        ] = fields.as_slice()
        else {
            return Err(GenerationError::InvalidShape);
        };
        Ok(Self::new(
            identity_from_bytes(manifest)?,
            identity_from_bytes(association)?,
            identity_from_bytes(finality)?,
            identity_from_bytes(cohort)?,
            identity_from_bytes(writer)?,
            identity_from_bytes(migration)?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreGenerationV1 {
    domain: StoreDomainV1,
    ordinal: u64,
    previous: Option<StoreGenerationIdV1>,
    contract_root_id: ContractRootIdV1,
    compatibility: StoreCompatibilityV1,
    roots: Vec<StoreObjectIdV1>,
    id: StoreGenerationIdV1,
}

impl StoreGenerationV1 {
    pub fn new(
        domain: StoreDomainV1,
        ordinal: u64,
        previous: Option<StoreGenerationIdV1>,
        contract_root_id: ContractRootIdV1,
        compatibility: StoreCompatibilityV1,
        roots: Vec<StoreObjectIdV1>,
    ) -> Result<Self, GenerationError> {
        if ordinal == 0 {
            return Err(GenerationError::ZeroMonotonicValue);
        }
        if (ordinal == 1) != previous.is_none() {
            return Err(GenerationError::InvalidPreviousGeneration);
        }
        if roots.is_empty() {
            return Err(GenerationError::EmptyRoots);
        }
        if roots.len() > MAX_GENERATION_ROOTS {
            return Err(GenerationError::TooManyRoots);
        }
        if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GenerationError::RootsNotStrictlySorted);
        }
        let value = generation_value(
            &domain,
            ordinal,
            previous.as_ref(),
            &contract_root_id,
            &compatibility,
            &roots,
        );
        let id = derive_identity(&value)?;
        Ok(Self {
            domain,
            ordinal,
            previous,
            contract_root_id,
            compatibility,
            roots,
            id,
        })
    }

    pub fn domain(&self) -> &StoreDomainV1 {
        &self.domain
    }

    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub fn previous(&self) -> Option<StoreGenerationIdV1> {
        self.previous
    }

    pub fn contract_root_id(&self) -> ContractRootIdV1 {
        self.contract_root_id
    }

    pub fn compatibility(&self) -> &StoreCompatibilityV1 {
        &self.compatibility
    }

    pub fn roots(&self) -> &[StoreObjectIdV1] {
        &self.roots
    }

    pub fn id(&self) -> StoreGenerationIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&generation_value(
            &self.domain,
            self.ordinal,
            self.previous.as_ref(),
            &self.contract_root_id,
            &self.compatibility,
            &self.roots,
        ))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GenerationError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(GenerationError::InvalidShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Unsigned(role_tag),
            CborValue::Bytes(domain_id),
            CborValue::Unsigned(ordinal),
            previous,
            CborValue::Bytes(contract_root_id),
            compatibility,
            CborValue::Array(roots),
        ] = fields.as_slice()
        else {
            return Err(GenerationError::InvalidShape);
        };
        if *version != STORE_GENERATION_VERSION_V1 {
            return Err(GenerationError::UnknownVersion(*version));
        }
        let role = StoreRoleV1::from_tag(*role_tag)?;
        let domain = StoreDomainV1::from_identity(role, identity_from_bytes(domain_id)?);
        let mut decoded_roots = Vec::with_capacity(roots.len());
        for root in roots {
            let CborValue::Bytes(bytes) = root else {
                return Err(GenerationError::InvalidShape);
            };
            decoded_roots.push(identity_from_bytes(bytes)?);
        }
        let generation = Self::new(
            domain,
            *ordinal,
            optional_identity_from_value(previous)?,
            identity_from_bytes(contract_root_id)?,
            StoreCompatibilityV1::decode(compatibility)?,
            decoded_roots,
        )?;
        if generation.canonical_bytes()? != bytes {
            return Err(GenerationError::NonCanonicalBytes);
        }
        Ok(generation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreHeadV1 {
    domain: StoreDomainV1,
    generation_id: StoreGenerationIdV1,
    generation_ordinal: u64,
    revision: u64,
    previous_head_id: Option<StoreHeadIdV1>,
    id: StoreHeadIdV1,
}

impl StoreHeadV1 {
    pub fn new(
        generation: &StoreGenerationV1,
        revision: u64,
        previous_head_id: Option<StoreHeadIdV1>,
    ) -> Result<Self, GenerationError> {
        if revision == 0 || revision != generation.ordinal() {
            return Err(GenerationError::InvalidHeadRevision);
        }
        if (revision == 1) != previous_head_id.is_none() {
            return Err(GenerationError::InvalidPreviousHead);
        }
        let value = head_value(
            generation.domain(),
            generation.id(),
            generation.ordinal(),
            revision,
            previous_head_id.as_ref(),
        );
        let id = derive_identity(&value)?;
        Ok(Self {
            domain: generation.domain().clone(),
            generation_id: generation.id(),
            generation_ordinal: generation.ordinal(),
            revision,
            previous_head_id,
            id,
        })
    }

    pub fn domain(&self) -> &StoreDomainV1 {
        &self.domain
    }

    pub fn generation_id(&self) -> StoreGenerationIdV1 {
        self.generation_id
    }

    pub fn generation_ordinal(&self) -> u64 {
        self.generation_ordinal
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn previous_head_id(&self) -> Option<StoreHeadIdV1> {
        self.previous_head_id
    }

    pub fn id(&self) -> StoreHeadIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&head_value(
            &self.domain,
            self.generation_id,
            self.generation_ordinal,
            self.revision,
            self.previous_head_id.as_ref(),
        ))
    }

    pub fn decode_for_generation(
        bytes: &[u8],
        generation: &StoreGenerationV1,
    ) -> Result<Self, GenerationError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(GenerationError::InvalidShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Unsigned(role_tag),
            CborValue::Bytes(domain_id),
            CborValue::Bytes(generation_id),
            CborValue::Unsigned(generation_ordinal),
            CborValue::Unsigned(revision),
            previous_head,
        ] = fields.as_slice()
        else {
            return Err(GenerationError::InvalidShape);
        };
        if *version != STORE_HEAD_VERSION_V1 {
            return Err(GenerationError::UnknownVersion(*version));
        }
        let decoded_domain: StoreDomainIdV1 = identity_from_bytes(domain_id)?;
        let decoded_generation: StoreGenerationIdV1 = identity_from_bytes(generation_id)?;
        if StoreRoleV1::from_tag(*role_tag)? != generation.domain().role()
            || decoded_domain != generation.domain().id()
            || decoded_generation != generation.id()
            || *generation_ordinal != generation.ordinal()
        {
            return Err(GenerationError::HeadGenerationMismatch);
        }
        let head = Self::new(
            generation,
            *revision,
            optional_identity_from_value(previous_head)?,
        )?;
        if head.canonical_bytes()? != bytes {
            return Err(GenerationError::NonCanonicalBytes);
        }
        Ok(head)
    }
}

fn generation_value(
    domain: &StoreDomainV1,
    ordinal: u64,
    previous: Option<&StoreGenerationIdV1>,
    contract_root_id: &ContractRootIdV1,
    compatibility: &StoreCompatibilityV1,
    roots: &[StoreObjectIdV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(STORE_GENERATION_VERSION_V1),
        CborValue::Unsigned(domain.role().tag()),
        CborValue::Bytes(domain.id().as_bytes().to_vec()),
        CborValue::Unsigned(ordinal),
        optional_identity(previous),
        CborValue::Bytes(contract_root_id.as_bytes().to_vec()),
        compatibility.canonical_value(),
        CborValue::Array(
            roots
                .iter()
                .map(|root| CborValue::Bytes(root.as_bytes().to_vec()))
                .collect(),
        ),
    ])
}

fn head_value(
    domain: &StoreDomainV1,
    generation_id: StoreGenerationIdV1,
    generation_ordinal: u64,
    revision: u64,
    previous_head_id: Option<&StoreHeadIdV1>,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(STORE_HEAD_VERSION_V1),
        CborValue::Unsigned(domain.role().tag()),
        CborValue::Bytes(domain.id().as_bytes().to_vec()),
        CborValue::Bytes(generation_id.as_bytes().to_vec()),
        CborValue::Unsigned(generation_ordinal),
        CborValue::Unsigned(revision),
        optional_identity(previous_head_id),
    ])
}

fn optional_identity<K>(
    identity: Option<&crate::domain::identity::ManifestIdentityV1<K>>,
) -> CborValue
where
    K: crate::domain::identity::IdentityKindV1,
{
    CborValue::optional(identity.map(|value| CborValue::Bytes(value.as_bytes().to_vec())))
}

fn optional_identity_from_value<K>(
    value: &CborValue,
) -> Result<Option<ManifestIdentityV1<K>>, GenerationError>
where
    K: IdentityKindV1,
{
    let CborValue::Array(fields) = value else {
        return Err(GenerationError::InvalidShape);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), CborValue::Bytes(bytes)] => Ok(Some(identity_from_bytes(bytes)?)),
        _ => Err(GenerationError::InvalidShape),
    }
}

fn identity_from_bytes<K>(bytes: &[u8]) -> Result<ManifestIdentityV1<K>, GenerationError>
where
    K: IdentityKindV1,
{
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| GenerationError::InvalidIdentityLength)?;
    Ok(ManifestIdentityV1::from_digest(digest))
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GenerationError {
    #[error("Store Generation has an invalid canonical shape")]
    InvalidShape,
    #[error("Store Generation identity bytes must contain exactly 32 bytes")]
    InvalidIdentityLength,
    #[error("Store Generation uses unsupported version {0}")]
    UnknownVersion(u64),
    #[error("Store Generation bytes are not the exact canonical encoding")]
    NonCanonicalBytes,
    #[error("Store Generation ordinals and epochs must be positive")]
    ZeroMonotonicValue,
    #[error("only ordinal one may omit the previous Store Generation")]
    InvalidPreviousGeneration,
    #[error("Store Generation must have at least one canonical root")]
    EmptyRoots,
    #[error("Store Generation roots exceed the finite v1 limit")]
    TooManyRoots,
    #[error("Store Generation roots must be strictly identity-sorted and unique")]
    RootsNotStrictlySorted,
    #[error("Store Head revision must equal the positive Generation ordinal")]
    InvalidHeadRevision,
    #[error("only the first Store Head may omit its previous Head identity")]
    InvalidPreviousHead,
    #[error("Store Head does not bind the supplied Store Generation")]
    HeadGenerationMismatch,
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Domain(#[from] super::StoreDomainError),
}
