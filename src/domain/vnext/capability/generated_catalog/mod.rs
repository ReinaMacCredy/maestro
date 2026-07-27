//! Frozen generated capability catalog consumed by the Stage-6 transport.

mod catalog;

#[allow(
    unused_imports,
    reason = "Stage 6 preserves its frozen candidate facade before root integration"
)]
pub use catalog::{
    ACTION_REQUEST_SCHEMA_REF_V1, ACTION_SPEC_MANIFEST_REF_V1, ACTION_TAG_COUNT_V1,
    CEREMONY_REQUEST_SCHEMA_REF_V1, CEREMONY_SPEC_MANIFEST_REF_V1, CEREMONY_TAG_COUNT_V1,
    CatalogOwnerV1, CeremonyContextKindV1, GeneratedCapabilityCatalogV1, GeneratedCatalogErrorV1,
    OperationCatalogEntryV1, OperationCatalogKindV1, PUBLIC_CATALOG_REF_V1,
};
