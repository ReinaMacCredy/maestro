use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::digest::{
    CatalogManifestIdentityKindV1, DescriptorIdV1, DescriptorIdentityKindV1, IdentityError,
    ManifestIdV1, SchemaIdV1, hash_exact_array,
};
use super::schema::{SchemaClosureV1, SchemaError};

pub const MAX_MANIFEST_ROWS: usize = 65_536;
pub const MANIFEST_HEADER_VERSION_V1: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorDomainV1 {
    ObservationKind,
    EffectOrigin,
    ActionLeaf,
    RepositoryGovernedCapacity,
    InstallationGovernedCapacity,
    CeremonySpec,
    RepositoryAuthorityContinuity,
    InstallationAuthorityContinuity,
    ActionSpec,
}

impl DescriptorDomainV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObservationKind => "maestro.vnext.observation-kind.descriptor.v1",
            Self::EffectOrigin => "maestro.vnext.effect-origin.descriptor.v1",
            Self::ActionLeaf => "maestro.vnext.action-leaf.descriptor.v1",
            Self::RepositoryGovernedCapacity => {
                "maestro.vnext.governed-capacity.repository.descriptor.v1"
            }
            Self::InstallationGovernedCapacity => {
                "maestro.vnext.governed-capacity.installation.descriptor.v1"
            }
            Self::CeremonySpec => "maestro.vnext.ceremony-spec.descriptor.v1",
            Self::RepositoryAuthorityContinuity => {
                "maestro.vnext.authority-continuity.repository.descriptor.v1"
            }
            Self::InstallationAuthorityContinuity => {
                "maestro.vnext.authority-continuity.installation.descriptor.v1"
            }
            Self::ActionSpec => "maestro.vnext.action-spec.descriptor.v1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestDomainV1 {
    ObservationKind,
    EffectOrigin,
    ActionLeafCensus,
    RepositoryGovernedCapacity,
    InstallationGovernedCapacity,
    CeremonySpec,
    RepositoryAuthorityContinuity,
    InstallationAuthorityContinuity,
    ActionSpecCatalog,
}

impl ManifestDomainV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObservationKind => "maestro.vnext.observation-kind.manifest.v1",
            Self::EffectOrigin => "maestro.vnext.effect-origin.manifest.v1",
            Self::ActionLeafCensus => "maestro.vnext.action-leaf-census.manifest.v1",
            Self::RepositoryGovernedCapacity => {
                "maestro.vnext.governed-capacity.repository.manifest.v1"
            }
            Self::InstallationGovernedCapacity => {
                "maestro.vnext.governed-capacity.installation.manifest.v1"
            }
            Self::CeremonySpec => "maestro.vnext.ceremony-spec.manifest.v1",
            Self::RepositoryAuthorityContinuity => {
                "maestro.vnext.authority-continuity.repository.manifest.v1"
            }
            Self::InstallationAuthorityContinuity => {
                "maestro.vnext.authority-continuity.installation.manifest.v1"
            }
            Self::ActionSpecCatalog => "maestro.vnext.action-spec.catalog.v1",
        }
    }

    pub const fn descriptor_domain(self) -> DescriptorDomainV1 {
        match self {
            Self::ObservationKind => DescriptorDomainV1::ObservationKind,
            Self::EffectOrigin => DescriptorDomainV1::EffectOrigin,
            Self::ActionLeafCensus => DescriptorDomainV1::ActionLeaf,
            Self::RepositoryGovernedCapacity => DescriptorDomainV1::RepositoryGovernedCapacity,
            Self::InstallationGovernedCapacity => DescriptorDomainV1::InstallationGovernedCapacity,
            Self::CeremonySpec => DescriptorDomainV1::CeremonySpec,
            Self::RepositoryAuthorityContinuity => {
                DescriptorDomainV1::RepositoryAuthorityContinuity
            }
            Self::InstallationAuthorityContinuity => {
                DescriptorDomainV1::InstallationAuthorityContinuity
            }
            Self::ActionSpecCatalog => DescriptorDomainV1::ActionSpec,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestHeaderV1 {
    row_count: u64,
    maximum_tag: u64,
    content: CborValue,
}

impl ManifestHeaderV1 {
    pub fn new(
        row_count: u64,
        maximum_tag: u64,
        content: CborValue,
    ) -> Result<Self, ManifestIdentityError> {
        if row_count > MAX_MANIFEST_ROWS as u64 {
            return Err(ManifestIdentityError::TooManyRows);
        }
        deterministic_cbor::encode(&content)?;
        Ok(Self {
            row_count,
            maximum_tag,
            content,
        })
    }

    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    pub fn maximum_tag(&self) -> u64 {
        self.maximum_tag
    }

    pub fn content(&self) -> &CborValue {
        &self.content
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(MANIFEST_HEADER_VERSION_V1),
            CborValue::Unsigned(self.row_count),
            CborValue::Unsigned(self.maximum_tag),
            self.content.clone(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestRowV1 {
    numeric_tag: u64,
    descriptor_schema_id: SchemaIdV1,
    descriptor_id: DescriptorIdV1,
    descriptor_value: CborValue,
}

impl ManifestRowV1 {
    pub fn new(
        schema_closure: &SchemaClosureV1,
        numeric_tag: u64,
        descriptor_domain: DescriptorDomainV1,
        descriptor_schema_id: &SchemaIdV1,
        descriptor_value: CborValue,
    ) -> Result<Self, ManifestIdentityError> {
        if numeric_tag == 0 {
            return Err(ManifestIdentityError::ZeroRowTag);
        }
        if declared_numeric_tag(&descriptor_value) != Some(numeric_tag) {
            return Err(ManifestIdentityError::DescriptorTagMismatch);
        }
        schema_closure.validate_value(descriptor_schema_id, &descriptor_value)?;
        let descriptor_id =
            descriptor_identity(descriptor_domain, descriptor_schema_id, &descriptor_value)?;
        Ok(Self {
            numeric_tag,
            descriptor_schema_id: *descriptor_schema_id,
            descriptor_id,
            descriptor_value,
        })
    }

    pub fn numeric_tag(&self) -> u64 {
        self.numeric_tag
    }

    pub fn descriptor_id(&self) -> &DescriptorIdV1 {
        &self.descriptor_id
    }

    pub fn descriptor_schema_id(&self) -> &SchemaIdV1 {
        &self.descriptor_schema_id
    }

    pub fn descriptor_value(&self) -> &CborValue {
        &self.descriptor_value
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.numeric_tag),
            CborValue::Bytes(self.descriptor_id.as_bytes().to_vec()),
            self.descriptor_value.clone(),
        ])
    }
}

#[derive(Clone, Debug)]
pub struct ManifestValueV1 {
    domain: ManifestDomainV1,
    manifest_schema_id: SchemaIdV1,
    descriptor_schema_id: SchemaIdV1,
    header: ManifestHeaderV1,
    rows: Vec<ManifestRowV1>,
    manifest_id: ManifestIdV1,
}

impl ManifestValueV1 {
    pub fn new(
        schema_closure: &SchemaClosureV1,
        domain: ManifestDomainV1,
        manifest_schema_id: SchemaIdV1,
        descriptor_schema_id: SchemaIdV1,
        header: ManifestHeaderV1,
        rows: Vec<ManifestRowV1>,
    ) -> Result<Self, ManifestIdentityError> {
        if rows.len() > MAX_MANIFEST_ROWS {
            return Err(ManifestIdentityError::TooManyRows);
        }
        if header.row_count != rows.len() as u64 {
            return Err(ManifestIdentityError::HeaderRowCountMismatch);
        }
        let header_value = header.canonical_value();
        schema_closure.validate_value(&manifest_schema_id, &header_value)?;
        let mut previous_tag = None;
        for row in &rows {
            if row.descriptor_schema_id != descriptor_schema_id {
                return Err(ManifestIdentityError::DescriptorSchemaMismatch);
            }
            schema_closure.validate_value(&descriptor_schema_id, &row.descriptor_value)?;
            if row.numeric_tag == 0 || row.numeric_tag > header.maximum_tag {
                return Err(ManifestIdentityError::RowTagOutsideHeaderBound);
            }
            if previous_tag.is_some_and(|previous| previous >= row.numeric_tag) {
                return Err(ManifestIdentityError::RowsNotStrictlyTagSorted);
            }
            previous_tag = Some(row.numeric_tag);
            if declared_numeric_tag(&row.descriptor_value) != Some(row.numeric_tag) {
                return Err(ManifestIdentityError::DescriptorTagMismatch);
            }
            let recomputed = descriptor_identity(
                domain.descriptor_domain(),
                &descriptor_schema_id,
                &row.descriptor_value,
            )?;
            if recomputed != row.descriptor_id {
                return Err(ManifestIdentityError::DescriptorIdentityMismatch);
            }
        }
        let manifest_id = manifest_identity(
            domain,
            &manifest_schema_id,
            &descriptor_schema_id,
            &header,
            &rows,
        )?;
        Ok(Self {
            domain,
            manifest_schema_id,
            descriptor_schema_id,
            header,
            rows,
            manifest_id,
        })
    }

    pub fn domain(&self) -> ManifestDomainV1 {
        self.domain
    }

    pub fn manifest_schema_id(&self) -> &SchemaIdV1 {
        &self.manifest_schema_id
    }

    pub fn descriptor_schema_id(&self) -> &SchemaIdV1 {
        &self.descriptor_schema_id
    }

    pub fn header(&self) -> &ManifestHeaderV1 {
        &self.header
    }

    pub fn rows(&self) -> &[ManifestRowV1] {
        &self.rows
    }

    pub fn manifest_id(&self) -> &ManifestIdV1 {
        &self.manifest_id
    }
}

pub(crate) fn descriptor_identity(
    domain: DescriptorDomainV1,
    schema_id: &SchemaIdV1,
    descriptor_value: &CborValue,
) -> Result<DescriptorIdV1, ManifestIdentityError> {
    Ok(hash_exact_array::<DescriptorIdentityKindV1>(vec![
        CborValue::text(domain.as_str())?,
        CborValue::Bytes(schema_id.as_bytes().to_vec()),
        descriptor_value.clone(),
    ])?)
}

pub(crate) fn manifest_identity(
    domain: ManifestDomainV1,
    manifest_schema_id: &SchemaIdV1,
    descriptor_schema_id: &SchemaIdV1,
    header: &ManifestHeaderV1,
    rows: &[ManifestRowV1],
) -> Result<ManifestIdV1, ManifestIdentityError> {
    Ok(hash_exact_array::<CatalogManifestIdentityKindV1>(vec![
        CborValue::text(domain.as_str())?,
        CborValue::Bytes(manifest_schema_id.as_bytes().to_vec()),
        CborValue::Bytes(descriptor_schema_id.as_bytes().to_vec()),
        header.canonical_value(),
        CborValue::Array(rows.iter().map(ManifestRowV1::canonical_value).collect()),
    ])?)
}

#[derive(Debug, Error)]
pub enum ManifestIdentityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("manifest row tag must be positive")]
    ZeroRowTag,
    #[error("manifest row descriptor does not declare the row tag")]
    DescriptorTagMismatch,
    #[error("manifest contains more rows than the finite v1 limit")]
    TooManyRows,
    #[error("manifest rows are not strictly sorted by numeric tag")]
    RowsNotStrictlyTagSorted,
    #[error("manifest row tag is outside the content-bound header limit")]
    RowTagOutsideHeaderBound,
    #[error("manifest descriptor identity does not match its value")]
    DescriptorIdentityMismatch,
    #[error("manifest header row count does not equal the row closure")]
    HeaderRowCountMismatch,
    #[error("manifest row was validated against a different descriptor SchemaIdV1")]
    DescriptorSchemaMismatch,
}

fn declared_numeric_tag(value: &CborValue) -> Option<u64> {
    let CborValue::Array(values) = value else {
        return None;
    };
    match values.first() {
        Some(CborValue::Unsigned(tag)) => Some(*tag),
        _ => None,
    }
}
