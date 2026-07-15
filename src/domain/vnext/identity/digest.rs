use std::fmt;
use std::marker::PhantomData;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

mod private {
    pub trait Sealed {}
}

pub trait IdentityKindV1: private::Sealed {
    const DOMAIN: &'static str;
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManifestIdentityV1<K: IdentityKindV1> {
    bytes: [u8; 32],
    marker: PhantomData<K>,
}

impl<K: IdentityKindV1> ManifestIdentityV1<K> {
    pub fn parse(rendered: &str) -> Result<Self, IdentityError> {
        let hexadecimal = rendered
            .strip_prefix("sha256:")
            .ok_or(IdentityError::InvalidRenderedIdentity)?;
        if hexadecimal.len() != 64 || !hexadecimal.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(IdentityError::InvalidRenderedIdentity);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
            let high = hexadecimal_nibble(pair[0]).ok_or(IdentityError::InvalidRenderedIdentity)?;
            let low = hexadecimal_nibble(pair[1]).ok_or(IdentityError::InvalidRenderedIdentity)?;
            bytes[index] = (high << 4) | low;
        }
        if Self::from_digest(bytes).render() != rendered {
            return Err(IdentityError::InvalidRenderedIdentity);
        }
        Ok(Self::from_digest(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.bytes
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

impl<K: IdentityKindV1> fmt::Debug for ManifestIdentityV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ManifestIdentityV1")
            .field(&self.render())
            .finish()
    }
}

impl<K: IdentityKindV1> fmt::Display for ManifestIdentityV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdentityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("identity must be canonical lowercase sha256:<64hex>")]
    InvalidRenderedIdentity,
}

macro_rules! identity_kind {
    ($marker:ident, $alias:ident, $domain:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $marker {}

        impl private::Sealed for $marker {}

        impl IdentityKindV1 for $marker {
            const DOMAIN: &'static str = $domain;
        }

        pub type $alias = ManifestIdentityV1<$marker>;
    };
}

identity_kind!(SchemaIdentityKindV1, SchemaIdV1, "maestro.vnext.schema.v1");
identity_kind!(
    DescriptorIdentityKindV1,
    DescriptorIdV1,
    "maestro.vnext.descriptor-id.v1"
);
identity_kind!(
    CatalogManifestIdentityKindV1,
    ManifestIdV1,
    "maestro.vnext.manifest-id.v1"
);
identity_kind!(
    ContractComponentIdentityKindV1,
    ContractComponentIdV1,
    "maestro.vnext.contract-component.v1"
);
identity_kind!(
    ContractRootIdentityKindV1,
    ContractRootIdV1,
    "maestro.vnext.candidate-contract-root.v1"
);
identity_kind!(
    DesignRevisionIdentityKindV1,
    DesignRevisionIdV1,
    "maestro.vnext.design-revision.v1"
);
identity_kind!(
    DesignClosureRequirementIdentityKindV1,
    DesignClosureRequirementIdV1,
    "maestro.vnext.design-closure-requirement.v1"
);
identity_kind!(
    DesignSourceBindingIdentityKindV1,
    DesignSourceBindingIdV1,
    "maestro.vnext.design-source-binding.v1"
);
identity_kind!(
    NoDesignExemptionIdentityKindV1,
    NoDesignExemptionIdV1,
    "maestro.vnext.no-design-exemption.v1"
);
identity_kind!(
    DecisionResolutionIdentityKindV1,
    DecisionResolutionIdV1,
    "maestro.vnext.decision-resolution.v1"
);
identity_kind!(
    DecisionMaterializationIdentityKindV1,
    DecisionMaterializationIdV1,
    "maestro.vnext.decision-materialization.v1"
);
identity_kind!(
    DecisionClosureIdentityKindV1,
    DecisionClosureIdV1,
    "maestro.vnext.decision-closure.v1"
);
identity_kind!(
    FinalizationInputIdentityKindV1,
    FinalizationInputIdV1,
    "maestro.vnext.design-finalization-input.v1"
);
identity_kind!(
    DesignFinalizationManifestIdentityKindV1,
    DesignFinalizationManifestIdV1,
    "maestro.vnext.design-finalization-manifest.v1"
);
identity_kind!(
    BuildHandoffIdentityKindV1,
    BuildHandoffIdV1,
    "maestro.vnext.build-handoff-projection.v1"
);
identity_kind!(
    Stage0ProofManifestIdentityKindV1,
    Stage0ProofManifestIdV1,
    "maestro.vnext.stage0-proof-manifest.v1"
);

pub fn design_revision_identity(value: &CborValue) -> Result<DesignRevisionIdV1, IdentityError> {
    derive_identity(value)
}

pub fn design_closure_requirement_identity(
    value: &CborValue,
) -> Result<DesignClosureRequirementIdV1, IdentityError> {
    derive_identity(value)
}

pub fn design_source_binding_identity(
    value: &CborValue,
) -> Result<DesignSourceBindingIdV1, IdentityError> {
    derive_identity(value)
}

pub fn no_design_exemption_identity(
    value: &CborValue,
) -> Result<NoDesignExemptionIdV1, IdentityError> {
    derive_identity(value)
}

pub fn decision_resolution_identity(
    value: &CborValue,
) -> Result<DecisionResolutionIdV1, IdentityError> {
    derive_identity(value)
}

pub fn decision_materialization_identity(
    value: &CborValue,
) -> Result<DecisionMaterializationIdV1, IdentityError> {
    derive_identity(value)
}

pub fn decision_closure_identity(value: &CborValue) -> Result<DecisionClosureIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn contract_component_identity(
    value: &CborValue,
) -> Result<ContractComponentIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn contract_root_identity(value: &CborValue) -> Result<ContractRootIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn finalization_input_identity(
    value: &CborValue,
) -> Result<FinalizationInputIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn design_finalization_manifest_identity(
    value: &CborValue,
) -> Result<DesignFinalizationManifestIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn build_handoff_identity(value: &CborValue) -> Result<BuildHandoffIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn stage0_proof_manifest_identity(
    value: &CborValue,
) -> Result<Stage0ProofManifestIdV1, IdentityError> {
    derive_identity(value)
}

pub(crate) fn derive_identity<K: IdentityKindV1>(
    value: &CborValue,
) -> Result<ManifestIdentityV1<K>, IdentityError> {
    hash_exact_array::<K>(vec![ascii_domain::<K>()?, value.clone()])
}

pub(crate) fn hash_exact_array<K: IdentityKindV1>(
    values: Vec<CborValue>,
) -> Result<ManifestIdentityV1<K>, IdentityError> {
    let bytes = deterministic_cbor::encode(&CborValue::Array(values))?;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    Ok(ManifestIdentityV1::from_digest(digest))
}

fn ascii_domain<K: IdentityKindV1>() -> Result<CborValue, CborError> {
    CborValue::text(K::DOMAIN)
}

fn hexadecimal_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}
