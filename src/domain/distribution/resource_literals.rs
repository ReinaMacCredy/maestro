//! Frozen C868 Resource, Bundle, Census, and Release identity literals.
//!
//! The concrete identity DAG is one-way:
//! content -> Resource descriptor -> non-Release Bundle manifest -> census
//! manifest -> sole embedded Release manifest. Candidate Root, finalization, and
//! handoff remain downstream obligations and never feed back into these IDs.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::CborValue;

pub const C868_SCHEMA_COUNT: usize = 38;
pub const C868_SUITE_COMPONENT_COUNT: usize = 62;
pub const C868_RUNTIME_EDGE_COUNT: usize = 61;
pub const RESOURCE_DESCRIPTOR_FIELD_COUNT: usize = 24;
pub const DESCRIPTOR_ENVELOPE_SLOT_COUNT: usize = 3;
pub const MANIFEST_ENVELOPE_SLOT_COUNT: usize = 5;
pub const BUNDLE_KIND_COUNT: usize = 8;
pub const NON_RELEASE_BUNDLE_KIND_COUNT: usize = 7;
pub const MAX_RESOURCE_DEPENDENCIES: usize = 4_095;
pub const MAX_RESOURCES_PER_BUNDLE: usize = 4_096;
pub const MAX_BUNDLE_DEPENDENCIES: usize = 64;
pub const MAX_RELEASE_BUNDLES: usize = 256;
pub const MAX_RELEASE_RESOURCES: usize = 65_535;
pub const MAX_CONSUMERS: usize = 262_144;
pub const MAX_CONSUMER_EDGES: usize = 1_048_576;

pub const RESOURCE_DESCRIPTOR_DOMAIN: &str = "maestro.vnext.resource.descriptor.v1";
pub const BUNDLE_MANIFEST_DOMAIN: &str = "maestro.vnext.bundle.manifest.v1";
pub const RELEASE_MEMBERSHIP_DESCRIPTOR_DOMAIN: &str =
    "maestro.vnext.release-bundle-membership.descriptor.v1";
pub const RELEASE_CENSUS_ROW_DESCRIPTOR_DOMAIN: &str =
    "maestro.vnext.release-resource-census-row.descriptor.v1";
pub const RELEASE_CENSUS_MANIFEST_DOMAIN: &str =
    "maestro.vnext.release-resource-census.manifest.v1";
pub const EMBEDDED_RELEASE_MANIFEST_DOMAIN: &str =
    "maestro.vnext.embedded-release-bundle.manifest.v1";

pub const MANIFEST_HEADER_CORE_SCHEMA_ID: &str =
    "c44af35107292d936b693fcc1375e4ee082b657feea14c2fd4cb9f718ef93b8d";
pub const RESOURCE_DESCRIPTOR_SCHEMA_ID: &str =
    "78cc56e71ae16fa2539429601fb08e37970d32569d0fddfd12c2129b6344bcc9";
pub const BUNDLE_MANIFEST_HEADER_SCHEMA_ID: &str =
    "ab811246b97d67ed2414723b046cfa29734cd7e7060114592ec6bd74d6cf8f63";
pub const BUNDLE_MANIFEST_SCHEMA_ID: &str =
    "f2d7bb5d5b5ba81fed67b3d1e25c89285c893aa57491f59212d34c7fb51c5dd2";
pub const RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID: &str =
    "d99e73adb9ffb8db858424033eb9afabc9ceb3f17870ee3be14d8176daf56e77";
pub const EMBEDDED_RELEASE_HEADER_SCHEMA_ID: &str =
    "2eb8a479550b066cd39e472f6db303d43df256b7ca621e397c435bc2f5ac24a5";
pub const EMBEDDED_RELEASE_BUNDLE_SCHEMA_ID: &str =
    "0f64450315ba74ce5206d378e71a8d6c63041631d34a59b905af0e2f55a5721f";
pub const RELEASE_CENSUS_RESOURCE_SCHEMA_ID: &str =
    "fef2d185bda28eed49c8dc3288d720a7ec0bb43f38049501f46fc2284a1d8237";
pub const RELEASE_CENSUS_DIRECT_CONSUMER_SCHEMA_ID: &str =
    "24fb2f2ccf433b837a299da954ccac9c5567463466009845f3dc22fd5e08d3a7";
pub const RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID: &str =
    "82c01e900d537186647b5258745c45777842d5952fcfa62a90723473189c878a";
pub const RELEASE_RESOURCE_CENSUS_HEADER_SCHEMA_ID: &str =
    "7f27f443f927fee5ace98650d80a7c0566a03a236cf49b206c971c7917a6a08a";
pub const RELEASE_RESOURCE_CENSUS_SCHEMA_ID: &str =
    "6b43ddca6f7c18f9693de17d8915eee8a5a51df42b341ac862aac86a7dd108de";

const MANIFEST_IDENTITY_PROTOCOL_SHA256: &str =
    "807c478cdd7b84fa44c7bb27827f972dfe05e25b0d2339285dfe311b81cfc077";
const OWNER_PROTOCOL_ID: &str = "a21d3d2c1eb16604331c1d206df86ae2fa3263b012dd0de12cf0bb83d19074ca";
const DEFAULT_MIGRATION_PROFILE_ID: &str =
    "12bbaf6404b4943b1f8d3ef85ed12c3e2bf2b97b037fd0ad3f71876634f4909a";
const DEFAULT_PARITY_PROFILE_ID: &str =
    "6cf1432e99a82e54e4698789bb2aa58a79f7a3a0a28abe5b849064ec1a6e1545";
const DEFAULT_REMOVAL_PROFILE_ID: &str =
    "069552018c8211f81eedb347a9427c5b0ade70cb86da25de7d491742e673c043";
const DEFAULT_PROOF_PROFILE_ID: &str =
    "00c33d207de36dcf7a65a3ab60956a55b19cf7cc556fcb2bedfec07fcc6aaa24";
const DELTA_IDENTITY_DOMAIN: &str = "maestro.vnext.migration-cutover-expected-delta-successor.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommitmentV1([u8; 32]);

impl CommitmentV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(value: &str) -> Result<Self, ResourceReleaseError> {
        let value = value.strip_prefix("sha256:").unwrap_or(value);
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ResourceReleaseError::InvalidCommitment);
        }
        if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return Err(ResourceReleaseError::InvalidCommitment);
        }
        let mut bytes = [0_u8; 32];
        for (index, slot) in bytes.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                .map_err(|_| ResourceReleaseError::InvalidCommitment)?;
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
        }
        output
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Bytes(self.0.to_vec())
    }
}

pub type ResourceIdV1 = CommitmentV1;
pub type BundleIdV1 = CommitmentV1;
pub type ReleaseResourceCensusIdV1 = CommitmentV1;
pub type ReleaseIdV1 = CommitmentV1;
pub type ConsumerIdV1 = CommitmentV1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BundleKindV1 {
    Release,
    AgentBootstrap,
    Capability,
    Orchestration,
    SharedContract,
    Adapter,
    ExternalPattern,
    Migration,
}

impl BundleKindV1 {
    pub const ALL: [Self; BUNDLE_KIND_COUNT] = [
        Self::Release,
        Self::AgentBootstrap,
        Self::Capability,
        Self::Orchestration,
        Self::SharedContract,
        Self::Adapter,
        Self::ExternalPattern,
        Self::Migration,
    ];
    pub const NON_RELEASE_TOPOLOGY: [Self; NON_RELEASE_BUNDLE_KIND_COUNT] = [
        Self::Migration,
        Self::ExternalPattern,
        Self::SharedContract,
        Self::Orchestration,
        Self::Capability,
        Self::Adapter,
        Self::AgentBootstrap,
    ];

    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::Release => 1,
            Self::AgentBootstrap => 2,
            Self::Capability => 3,
            Self::Orchestration => 4,
            Self::SharedContract => 5,
            Self::Adapter => 6,
            Self::ExternalPattern => 7,
            Self::Migration => 8,
        }
    }

    const fn topology_rank(self) -> Option<u64> {
        match self {
            Self::Migration => Some(1),
            Self::ExternalPattern => Some(2),
            Self::SharedContract => Some(3),
            Self::Orchestration => Some(4),
            Self::Capability => Some(5),
            Self::Adapter => Some(6),
            Self::AgentBootstrap => Some(7),
            Self::Release => None,
        }
    }

    fn from_numeric_tag(tag: u64) -> Result<Self, ResourceReleaseError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.numeric_tag() == tag)
            .ok_or(ResourceReleaseError::InvalidClosedTag)
    }
}

macro_rules! closed_enum {
    ($name:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const fn numeric_tag(self) -> u64 {
                match self { $(Self::$variant => $tag),+ }
            }

            pub fn from_numeric_tag(tag: u64) -> Result<Self, ResourceReleaseError> {
                match tag {
                    $($tag => Ok(Self::$variant),)+
                    _ => Err(ResourceReleaseError::InvalidClosedTag),
                }
            }
        }
    };
}

closed_enum!(ContentEncodingV1 {
    OpaqueBytes = 1,
    Utf8Text = 2,
});
closed_enum!(ResourceKindV1 {
    Executable = 1,
    Signature = 2,
    BillOfMaterials = 3,
    AgentInstruction = 4,
    OrchestrationDefinition = 5,
    PublicContract = 6,
    AdapterArtifact = 7,
    ExternalPattern = 8,
    MigrationArtifact = 9,
    License = 10,
    ProvenanceManifest = 11,
});
closed_enum!(ResourceProvenanceKindV1 {
    FirstParty = 1,
    ThirdParty = 2,
});
closed_enum!(ResourceDispositionV1 {
    Retain = 1,
    Rewrite = 2,
    Replace = 3,
    MigrationOnly = 4,
    Remove = 5,
});
closed_enum!(TargetClassV1 {
    NoMaterialization = 1,
    WholeTarget = 2,
    ManagedBlock = 3,
    ActivationLink = 4,
    HostRegistration = 5,
    ExternalManagerTarget = 6,
});
closed_enum!(DirectConsumerKindV1 {
    Build = 1,
    Runtime = 2,
    Install = 3,
    Migration = 4,
    Proof = 5,
    Documentation = 6,
    RemovalReader = 7,
});

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnerRefV1 {
    pub owner_tag: u64,
    pub owner_profile_id: CommitmentV1,
}

impl OwnerRefV1 {
    pub fn new(
        owner_tag: u64,
        owner_profile_id: CommitmentV1,
    ) -> Result<Self, ResourceReleaseError> {
        if !(1..=20).contains(&owner_tag) {
            return Err(ResourceReleaseError::InvalidOwner);
        }
        Ok(Self {
            owner_tag,
            owner_profile_id,
        })
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.owner_tag),
            self.owner_profile_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRefV1 {
    pub resource_tag: u64,
    pub resource_id: ResourceIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BundleRefV1 {
    pub bundle_tag: u64,
    pub bundle_id: BundleIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDescriptorInputV1 {
    pub resource_tag: u64,
    pub stable_resource_key: String,
    pub content: Vec<u8>,
    pub content_encoding: ContentEncodingV1,
    pub media_type: String,
    pub resource_kind: ResourceKindV1,
    pub semantic_owner: OwnerRefV1,
    pub required_bundle_kind: BundleKindV1,
    pub provenance_kind: ResourceProvenanceKindV1,
    pub provenance_commitment_id: CommitmentV1,
    pub license_commitment_id: Option<CommitmentV1>,
    pub backward_dependencies: Vec<ResourceRefV1>,
    pub compatibility_profile_id: CommitmentV1,
    pub generator_commitment_id: Option<CommitmentV1>,
    pub target_policy_profile_id: CommitmentV1,
    pub custody_policy_profile_id: CommitmentV1,
    pub migration_profile_id: CommitmentV1,
    pub rollback_profile_id: CommitmentV1,
    pub uninstall_profile_id: CommitmentV1,
    pub retention_profile_id: CommitmentV1,
    pub removal_profile_id: CommitmentV1,
    pub disposition: ResourceDispositionV1,
    pub proof_profile_id: CommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDescriptorV1 {
    resource_tag: u64,
    stable_resource_key: String,
    required_bundle_kind: BundleKindV1,
    disposition: ResourceDispositionV1,
    owner: OwnerRefV1,
    dependencies: Vec<ResourceRefV1>,
    value: CborValue,
    envelope: CborValue,
    resource_id: ResourceIdV1,
    canonical_cbor: Vec<u8>,
}

impl ResourceDescriptorV1 {
    pub fn new(input: ResourceDescriptorInputV1) -> Result<Self, ResourceReleaseError> {
        validate_u64(input.resource_tag, 1, 4_096)?;
        validate_ascii(&input.stable_resource_key, 1, 512)?;
        validate_ascii(&input.media_type, 1, 128)?;
        OwnerRefV1::new(
            input.semantic_owner.owner_tag,
            input.semantic_owner.owner_profile_id,
        )?;
        validate_strict_resource_refs(&input.backward_dependencies, false)?;
        if input.backward_dependencies.len() > MAX_RESOURCE_DEPENDENCIES
            || input
                .backward_dependencies
                .iter()
                .any(|dependency| dependency.resource_tag >= input.resource_tag)
        {
            return Err(ResourceReleaseError::InvalidResourceDependency);
        }
        let content_length = u64::try_from(input.content.len())
            .map_err(|_| ResourceReleaseError::InvalidResourceDescriptor)?;
        let content_sha256 = CommitmentV1::from_bytes(Sha256::digest(&input.content).into());
        let value = CborValue::Array(vec![
            CborValue::Unsigned(input.resource_tag),
            CborValue::Text(input.stable_resource_key),
            content_sha256.canonical_value(),
            CborValue::Unsigned(content_length),
            CborValue::Unsigned(input.content_encoding.numeric_tag()),
            CborValue::Text(input.media_type),
            CborValue::Unsigned(input.resource_kind.numeric_tag()),
            input.semantic_owner.canonical_value(),
            CborValue::Unsigned(input.required_bundle_kind.numeric_tag()),
            CborValue::Unsigned(input.provenance_kind.numeric_tag()),
            input.provenance_commitment_id.canonical_value(),
            optional_commitment(input.license_commitment_id),
            CborValue::Array(
                input
                    .backward_dependencies
                    .iter()
                    .map(|dependency| {
                        CborValue::Array(vec![
                            CborValue::Unsigned(dependency.resource_tag),
                            dependency.resource_id.canonical_value(),
                        ])
                    })
                    .collect(),
            ),
            input.compatibility_profile_id.canonical_value(),
            optional_commitment(input.generator_commitment_id),
            input.target_policy_profile_id.canonical_value(),
            input.custody_policy_profile_id.canonical_value(),
            input.migration_profile_id.canonical_value(),
            input.rollback_profile_id.canonical_value(),
            input.uninstall_profile_id.canonical_value(),
            input.retention_profile_id.canonical_value(),
            input.removal_profile_id.canonical_value(),
            CborValue::Unsigned(input.disposition.numeric_tag()),
            input.proof_profile_id.canonical_value(),
        ]);
        let envelope = descriptor_envelope(
            RESOURCE_DESCRIPTOR_DOMAIN,
            RESOURCE_DESCRIPTOR_SCHEMA_ID,
            value,
        )?;
        Self::from_envelope(envelope)
    }

    pub fn from_envelope(envelope: CborValue) -> Result<Self, ResourceReleaseError> {
        let slots = exact_array(&envelope, DESCRIPTOR_ENVELOPE_SLOT_COUNT)?;
        exact_text(&slots[0], RESOURCE_DESCRIPTOR_DOMAIN)?;
        exact_commitment(&slots[1], RESOURCE_DESCRIPTOR_SCHEMA_ID)?;
        let coordinates = validate_resource_value(&slots[2])?;
        let canonical_cbor = encode_manifest_identity(&envelope)?;
        validate_manifest_identity_cbor(&canonical_cbor)?;
        let resource_id = sha256_commitment(&canonical_cbor);
        Ok(Self {
            resource_tag: coordinates.resource_tag,
            stable_resource_key: coordinates.stable_resource_key,
            required_bundle_kind: coordinates.required_bundle_kind,
            disposition: coordinates.disposition,
            owner: coordinates.owner,
            dependencies: coordinates.dependencies,
            value: slots[2].clone(),
            envelope,
            resource_id,
            canonical_cbor,
        })
    }

    pub const fn id(&self) -> ResourceIdV1 {
        self.resource_id
    }

    pub const fn resource_tag(&self) -> u64 {
        self.resource_tag
    }

    pub fn stable_resource_key(&self) -> &str {
        &self.stable_resource_key
    }

    pub const fn required_bundle_kind(&self) -> BundleKindV1 {
        self.required_bundle_kind
    }

    pub const fn disposition(&self) -> ResourceDispositionV1 {
        self.disposition
    }

    pub const fn owner(&self) -> OwnerRefV1 {
        self.owner
    }

    pub fn dependencies(&self) -> &[ResourceRefV1] {
        &self.dependencies
    }

    pub fn resource_ref(&self) -> ResourceRefV1 {
        ResourceRefV1 {
            resource_tag: self.resource_tag,
            resource_id: self.resource_id,
        }
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn envelope(&self) -> &CborValue {
        &self.envelope
    }

    pub fn canonical_cbor(&self) -> &[u8] {
        &self.canonical_cbor
    }

    fn validate(&self) -> Result<(), ResourceReleaseError> {
        if Self::from_envelope(self.envelope.clone())? != *self {
            return Err(ResourceReleaseError::InvalidResourceDescriptor);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleManifestInputV1 {
    pub bundle_tag: u64,
    pub bundle_kind: BundleKindV1,
    pub stable_bundle_key: String,
    pub semantic_version: String,
    pub compatibility_profile_id: CommitmentV1,
    pub resources: Vec<ResourceDescriptorV1>,
    pub dependency_bundles: Vec<BundleManifestV1>,
    pub provenance_commitment_id: CommitmentV1,
    pub license_commitment_id: Option<CommitmentV1>,
    pub package_policy_profile_id: CommitmentV1,
    pub supported_target_classes: Vec<TargetClassV1>,
    pub rollback_profile_id: CommitmentV1,
    pub uninstall_profile_id: CommitmentV1,
    pub retention_profile_id: CommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleManifestV1 {
    bundle_tag: u64,
    bundle_kind: BundleKindV1,
    resource_ids: Vec<ResourceIdV1>,
    dependency_bundles: Vec<BundleRefV1>,
    value: CborValue,
    envelope: CborValue,
    bundle_id: BundleIdV1,
    canonical_cbor: Vec<u8>,
}

impl BundleManifestV1 {
    pub fn new(input: BundleManifestInputV1) -> Result<Self, ResourceReleaseError> {
        validate_u64(input.bundle_tag, 1, 256)?;
        if input.bundle_kind == BundleKindV1::Release {
            return Err(ResourceReleaseError::InvalidBundleKind);
        }
        validate_ascii(&input.stable_bundle_key, 1, 512)?;
        validate_ascii(&input.semantic_version, 1, 128)?;
        if !(1..=MAX_RESOURCES_PER_BUNDLE).contains(&input.resources.len()) {
            return Err(ResourceReleaseError::InvalidBundleMembership);
        }
        if input.dependency_bundles.len() > MAX_BUNDLE_DEPENDENCIES {
            return Err(ResourceReleaseError::InvalidBundleDependency);
        }
        for resource in &input.resources {
            resource.validate()?;
            if resource.required_bundle_kind != input.bundle_kind {
                return Err(ResourceReleaseError::ResourceInWrongBundle);
            }
        }
        let resource_refs = input
            .resources
            .iter()
            .map(ResourceDescriptorV1::resource_ref)
            .collect::<Vec<_>>();
        validate_strict_resource_refs(&resource_refs, true)?;
        let dependency_refs = input
            .dependency_bundles
            .iter()
            .map(|bundle| BundleRefV1 {
                bundle_tag: bundle.bundle_tag,
                bundle_id: bundle.bundle_id,
            })
            .collect::<Vec<_>>();
        validate_strict_bundle_refs(&dependency_refs, false)?;
        if dependency_refs
            .iter()
            .any(|dependency| dependency.bundle_tag >= input.bundle_tag)
        {
            return Err(ResourceReleaseError::InvalidBundleDependency);
        }
        if !(1..=6).contains(&input.supported_target_classes.len()) {
            return Err(ResourceReleaseError::InvalidTargetClassClosure);
        }
        let target_tags = input
            .supported_target_classes
            .iter()
            .map(|target| target.numeric_tag())
            .collect::<Vec<_>>();
        validate_strict_tags(&target_tags, true)?;

        let dependency_coordinates = dependency_refs
            .iter()
            .map(|dependency| (dependency.bundle_tag, dependency.bundle_id))
            .collect::<Vec<_>>();
        let core = make_manifest_core(
            RESOURCE_DESCRIPTOR_SCHEMA_ID,
            RESOURCE_DESCRIPTOR_SCHEMA_ID,
            BUNDLE_MANIFEST_HEADER_SCHEMA_ID,
            BUNDLE_MANIFEST_SCHEMA_ID,
            &dependency_coordinates,
            input.resources.len(),
            resource_refs
                .last()
                .ok_or(ResourceReleaseError::InvalidBundleMembership)?
                .resource_tag,
        )?;
        let header = CborValue::Array(vec![
            core,
            CborValue::Unsigned(input.bundle_tag),
            CborValue::Text(input.stable_bundle_key),
            CborValue::Unsigned(input.bundle_kind.numeric_tag()),
            CborValue::Text(input.semantic_version),
            input.compatibility_profile_id.canonical_value(),
            CborValue::Array(
                dependency_refs
                    .iter()
                    .map(|dependency| {
                        CborValue::Array(vec![
                            CborValue::Unsigned(dependency.bundle_tag),
                            dependency.bundle_id.canonical_value(),
                        ])
                    })
                    .collect(),
            ),
            input.provenance_commitment_id.canonical_value(),
            optional_commitment(input.license_commitment_id),
            input.package_policy_profile_id.canonical_value(),
            CborValue::Array(
                target_tags
                    .iter()
                    .map(|tag| {
                        CborValue::Array(vec![CborValue::Unsigned(*tag), CborValue::Unsigned(*tag)])
                    })
                    .collect(),
            ),
            input.rollback_profile_id.canonical_value(),
            input.uninstall_profile_id.canonical_value(),
            input.retention_profile_id.canonical_value(),
        ]);
        let rows = CborValue::Array(
            input
                .resources
                .iter()
                .map(|resource| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(resource.resource_tag),
                        resource.resource_id.canonical_value(),
                        resource.value.clone(),
                    ])
                })
                .collect(),
        );
        let value = CborValue::Array(vec![header.clone(), rows.clone()]);
        let envelope = manifest_envelope(
            BUNDLE_MANIFEST_DOMAIN,
            BUNDLE_MANIFEST_SCHEMA_ID,
            RESOURCE_DESCRIPTOR_SCHEMA_ID,
            header,
            rows,
        )?;
        let canonical_cbor = encode_manifest_identity(&envelope)?;
        let bundle_id = sha256_commitment(&canonical_cbor);
        if contains_commitment(&value, bundle_id) {
            return Err(ResourceReleaseError::IdentityBackreference);
        }
        let bundle = Self {
            bundle_tag: input.bundle_tag,
            bundle_kind: input.bundle_kind,
            resource_ids: input
                .resources
                .iter()
                .map(ResourceDescriptorV1::id)
                .collect(),
            dependency_bundles: dependency_refs,
            value,
            envelope,
            bundle_id,
            canonical_cbor,
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub const fn id(&self) -> BundleIdV1 {
        self.bundle_id
    }

    pub const fn bundle_tag(&self) -> u64 {
        self.bundle_tag
    }

    pub const fn kind(&self) -> BundleKindV1 {
        self.bundle_kind
    }

    pub fn resource_ids(&self) -> &[ResourceIdV1] {
        &self.resource_ids
    }

    pub fn dependencies(&self) -> &[BundleRefV1] {
        &self.dependency_bundles
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn envelope(&self) -> &CborValue {
        &self.envelope
    }

    pub fn canonical_cbor(&self) -> &[u8] {
        &self.canonical_cbor
    }

    fn validate(&self) -> Result<(), ResourceReleaseError> {
        validate_bundle_manifest(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectConsumerInputV1 {
    pub locator: String,
    pub semantic_owner: OwnerRefV1,
    pub consumer_kind: DirectConsumerKindV1,
    pub resources: Vec<ResourceRefV1>,
    pub provenance_commitment_id: CommitmentV1,
    pub disposition: ResourceDispositionV1,
    pub migration_profile_id: CommitmentV1,
    pub proof_profile_id: CommitmentV1,
    pub removal_profile_id: CommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectConsumerV1 {
    input: DirectConsumerInputV1,
}

impl DirectConsumerV1 {
    pub fn new(input: DirectConsumerInputV1) -> Result<Self, ResourceReleaseError> {
        validate_ascii(&input.locator, 1, 4_096)?;
        OwnerRefV1::new(
            input.semantic_owner.owner_tag,
            input.semantic_owner.owner_profile_id,
        )?;
        if input.resources.len() > MAX_RELEASE_RESOURCES {
            return Err(ResourceReleaseError::InvalidConsumerEdge);
        }
        validate_strict_resource_refs(&input.resources, true)?;
        Ok(Self { input })
    }

    pub fn from_resources(
        mut input: DirectConsumerInputV1,
        resources: &[ResourceDescriptorV1],
    ) -> Result<Self, ResourceReleaseError> {
        input.resources = resources
            .iter()
            .map(ResourceDescriptorV1::resource_ref)
            .collect();
        Self::new(input)
    }

    pub fn resources(&self) -> &[ResourceRefV1] {
        &self.input.resources
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Text(self.input.locator.clone()),
            self.input.semantic_owner.canonical_value(),
            CborValue::Unsigned(self.input.consumer_kind.numeric_tag()),
            CborValue::Array(
                self.input
                    .resources
                    .iter()
                    .map(|resource| {
                        CborValue::Array(vec![
                            CborValue::Unsigned(resource.resource_tag),
                            resource.resource_id.canonical_value(),
                        ])
                    })
                    .collect(),
            ),
            self.input.provenance_commitment_id.canonical_value(),
            CborValue::Unsigned(self.input.disposition.numeric_tag()),
            self.input.migration_profile_id.canonical_value(),
            self.input.proof_profile_id.canonical_value(),
            self.input.removal_profile_id.canonical_value(),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConsumerEdgeV1 {
    pub consumer_id: ConsumerIdV1,
    pub resource_tag: u64,
    pub resource_id: ResourceIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseResourceCensusInputV1 {
    pub release_key: String,
    pub release_version: String,
    pub platform_qualifier: String,
    pub resources: Vec<ResourceDescriptorV1>,
    pub bundles: Vec<BundleManifestV1>,
    pub direct_consumers: Vec<DirectConsumerV1>,
    pub source_inventory_digest: CommitmentV1,
    pub consumer_inventory_digest: CommitmentV1,
    pub build_graph_digest: CommitmentV1,
    pub resource_locators: Option<BTreeMap<ResourceIdV1, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseResourceCensusV1 {
    resource_ids: Vec<ResourceIdV1>,
    bundle_ids: Vec<BundleIdV1>,
    consumer_edges: Vec<ConsumerEdgeV1>,
    resource_locators: BTreeMap<ResourceIdV1, String>,
    value: CborValue,
    envelope: CborValue,
    census_id: ReleaseResourceCensusIdV1,
    canonical_cbor: Vec<u8>,
}

impl ReleaseResourceCensusV1 {
    pub fn new(input: ReleaseResourceCensusInputV1) -> Result<Self, ResourceReleaseError> {
        validate_ascii(&input.release_key, 1, 512)?;
        validate_ascii(&input.release_version, 1, 128)?;
        validate_ascii(&input.platform_qualifier, 1, 256)?;
        validate_bundle_topology(&input.bundles)?;
        validate_resource_and_bundle_closure(&input.resources, &input.bundles)?;
        if input.direct_consumers.len() > MAX_CONSUMERS {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }

        let bundle_by_resource = bundle_by_resource(&input.bundles)?;
        let resource_ids = input
            .resources
            .iter()
            .map(ResourceDescriptorV1::id)
            .collect::<BTreeSet<_>>();
        let resource_locators = match input.resource_locators {
            Some(locators) => {
                if locators.keys().copied().collect::<BTreeSet<_>>() != resource_ids {
                    return Err(ResourceReleaseError::InvalidResourceLocatorClosure);
                }
                for locator in locators.values() {
                    validate_ascii(locator, 1, 4_096)?;
                }
                locators
            }
            None => input
                .resources
                .iter()
                .map(|resource| (resource.id(), resource.stable_resource_key.clone()))
                .collect(),
        };

        let mut entries = Vec::new();
        for resource in &input.resources {
            let bundle = bundle_by_resource
                .get(&resource.id())
                .ok_or(ResourceReleaseError::InvalidCensusClosure)?;
            let locator = resource_locators
                .get(&resource.id())
                .ok_or(ResourceReleaseError::InvalidResourceLocatorClosure)?;
            let resource_value = resource_value_array(resource)?;
            let census_resource = CborValue::Array(vec![
                CborValue::Text(locator.clone()),
                resource.owner.canonical_value(),
                resource.id().canonical_value(),
                bundle.id().canonical_value(),
                resource_value[10].clone(),
                resource_value[15].clone(),
                resource_value[22].clone(),
                resource_value[17].clone(),
                resource_value[23].clone(),
                resource_value[21].clone(),
            ]);
            let entry_tag = usize_to_u64(entries.len() + 1)?;
            entries.push(CborValue::Array(vec![
                CborValue::Unsigned(entry_tag),
                CborValue::Array(vec![CborValue::Unsigned(1), census_resource]),
                CborValue::Array(vec![CborValue::Unsigned(0)]),
            ]));
        }

        let known_resources = input
            .resources
            .iter()
            .map(|resource| (resource.id(), resource.resource_tag))
            .collect::<BTreeMap<_, _>>();
        let mut edge_count = known_resources
            .keys()
            .copied()
            .map(|resource_id| (resource_id, 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut consumer_edges = Vec::new();
        for consumer in &input.direct_consumers {
            for resource in consumer.resources() {
                if known_resources.get(&resource.resource_id) != Some(&resource.resource_tag) {
                    return Err(ResourceReleaseError::InvalidConsumerEdge);
                }
            }
            let consumer_value = consumer.canonical_value();
            let consumer_id = identity_of_value(&consumer_value)?;
            for resource in consumer.resources() {
                consumer_edges.push(ConsumerEdgeV1 {
                    consumer_id,
                    resource_tag: resource.resource_tag,
                    resource_id: resource.resource_id,
                });
                *edge_count
                    .get_mut(&resource.resource_id)
                    .ok_or(ResourceReleaseError::InvalidConsumerEdge)? += 1;
            }
            let entry_tag = usize_to_u64(entries.len() + 1)?;
            entries.push(CborValue::Array(vec![
                CborValue::Unsigned(entry_tag),
                CborValue::Array(vec![CborValue::Unsigned(0)]),
                CborValue::Array(vec![CborValue::Unsigned(1), consumer_value]),
            ]));
        }
        if consumer_edges.len() > MAX_CONSUMER_EDGES {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }
        for resource in &input.resources {
            if resource.disposition == ResourceDispositionV1::Remove
                && edge_count.get(&resource.id()).copied().unwrap_or_default() != 0
            {
                return Err(ResourceReleaseError::RemoveHasDirectConsumers);
            }
        }

        let descriptor_rows = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Ok(CborValue::Array(vec![
                    CborValue::Unsigned(usize_to_u64(index + 1)?),
                    release_census_entry_descriptor_id(entry)?.canonical_value(),
                    entry.clone(),
                ]))
            })
            .collect::<Result<Vec<_>, ResourceReleaseError>>()?;
        let bundle_dependencies = input
            .bundles
            .iter()
            .map(|bundle| (bundle.bundle_tag, bundle.bundle_id))
            .collect::<Vec<_>>();
        let core = make_manifest_core(
            RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID,
            RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID,
            RELEASE_RESOURCE_CENSUS_HEADER_SCHEMA_ID,
            RELEASE_RESOURCE_CENSUS_SCHEMA_ID,
            &bundle_dependencies,
            descriptor_rows.len(),
            descriptor_rows.len() as u64,
        )?;
        let header = CborValue::Array(vec![
            core,
            CborValue::Text(input.release_key),
            CborValue::Text(input.release_version),
            CborValue::Text(input.platform_qualifier),
            CborValue::Array(
                input
                    .bundles
                    .iter()
                    .map(|bundle| {
                        CborValue::Array(vec![
                            CborValue::Unsigned(bundle.bundle_tag),
                            bundle.bundle_id.canonical_value(),
                        ])
                    })
                    .collect(),
            ),
            CborValue::Unsigned(usize_to_u64(input.resources.len())?),
            CborValue::Unsigned(usize_to_u64(input.direct_consumers.len())?),
            CborValue::Unsigned(usize_to_u64(consumer_edges.len())?),
            input.source_inventory_digest.canonical_value(),
            input.consumer_inventory_digest.canonical_value(),
            input.build_graph_digest.canonical_value(),
            CborValue::Unsigned(1),
        ]);
        let rows = CborValue::Array(descriptor_rows);
        let envelope = manifest_envelope(
            RELEASE_CENSUS_MANIFEST_DOMAIN,
            RELEASE_RESOURCE_CENSUS_SCHEMA_ID,
            RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID,
            header,
            rows,
        )?;
        Self::from_envelope(envelope, &input.resources, &input.bundles)
    }

    pub fn from_envelope(
        envelope: CborValue,
        resources: &[ResourceDescriptorV1],
        bundles: &[BundleManifestV1],
    ) -> Result<Self, ResourceReleaseError> {
        validate_bundle_topology(bundles)?;
        validate_resource_and_bundle_closure(resources, bundles)?;
        let slots = exact_array(&envelope, MANIFEST_ENVELOPE_SLOT_COUNT)?;
        exact_text(&slots[0], RELEASE_CENSUS_MANIFEST_DOMAIN)?;
        exact_commitment(&slots[1], RELEASE_RESOURCE_CENSUS_SCHEMA_ID)?;
        exact_commitment(&slots[2], RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID)?;
        let header = exact_array(&slots[3], 12)?;
        let rows = any_array(&slots[4])?;
        if rows.is_empty() || rows.len() > MAX_RELEASE_RESOURCES + MAX_CONSUMERS {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }
        validate_ascii(as_text(&header[1])?, 1, 512)?;
        validate_ascii(as_text(&header[2])?, 1, 128)?;
        validate_ascii(as_text(&header[3])?, 1, 256)?;
        if as_unsigned(&header[11])? != 1 {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }

        let expected_bundle_rows = bundles
            .iter()
            .map(|bundle| {
                CborValue::Array(vec![
                    CborValue::Unsigned(bundle.bundle_tag),
                    bundle.bundle_id.canonical_value(),
                ])
            })
            .collect::<Vec<_>>();
        if any_array(&header[4])? != expected_bundle_rows.as_slice() {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }
        let bundle_by_resource = bundle_by_resource(bundles)?;
        let known_resources = resources
            .iter()
            .map(|resource| (resource.id(), resource))
            .collect::<BTreeMap<_, _>>();
        let mut resource_ids = Vec::new();
        let mut resource_locators = BTreeMap::new();
        let mut consumer_edges = Vec::new();
        let mut edge_count = known_resources
            .keys()
            .copied()
            .map(|resource_id| (resource_id, 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut row_tags = Vec::new();
        let mut consumer_count = 0_usize;
        for row in rows {
            let row = exact_array(row, 3)?;
            let entry_tag = as_unsigned(&row[0])?;
            validate_u64(entry_tag, 1, (MAX_RELEASE_RESOURCES + MAX_CONSUMERS) as u64)?;
            row_tags.push(entry_tag);
            let entry = exact_array(&row[2], 3)?;
            if as_unsigned(&entry[0])? != entry_tag
                || as_commitment(&row[1])? != release_census_entry_descriptor_id(&row[2])?
            {
                return Err(ResourceReleaseError::InvalidCensusDescriptor);
            }
            let resource_branch = parse_optional(&entry[1])?;
            let consumer_branch = parse_optional(&entry[2])?;
            match (resource_branch, consumer_branch) {
                (Some(value), None) => {
                    let value = exact_array(value, 10)?;
                    let locator = as_text(&value[0])?;
                    validate_ascii(locator, 1, 4_096)?;
                    let owner = parse_owner(&value[1])?;
                    let resource_id = as_commitment(&value[2])?;
                    let owning_bundle_id = as_commitment(&value[3])?;
                    let resource = known_resources
                        .get(&resource_id)
                        .ok_or(ResourceReleaseError::InvalidCensusClosure)?;
                    let owning_bundle = bundle_by_resource
                        .get(&resource_id)
                        .ok_or(ResourceReleaseError::InvalidCensusClosure)?;
                    let resource_value = resource_value_array(resource)?;
                    if owner != resource.owner
                        || owning_bundle_id != owning_bundle.id()
                        || as_commitment(&value[4])? != as_commitment(&resource_value[10])?
                        || as_commitment(&value[5])? != as_commitment(&resource_value[15])?
                        || ResourceDispositionV1::from_numeric_tag(as_unsigned(&value[6])?)?
                            != resource.disposition
                        || as_commitment(&value[7])? != as_commitment(&resource_value[17])?
                        || as_commitment(&value[8])? != as_commitment(&resource_value[23])?
                        || as_commitment(&value[9])? != as_commitment(&resource_value[21])?
                    {
                        return Err(ResourceReleaseError::InvalidCensusCoordinates);
                    }
                    if resource_locators
                        .insert(resource_id, locator.to_owned())
                        .is_some()
                    {
                        return Err(ResourceReleaseError::InvalidCensusClosure);
                    }
                    resource_ids.push(resource_id);
                }
                (None, Some(consumer_value)) => {
                    consumer_count += 1;
                    let consumer_id = identity_of_value(consumer_value)?;
                    let value = exact_array(consumer_value, 9)?;
                    validate_ascii(as_text(&value[0])?, 1, 4_096)?;
                    parse_owner(&value[1])?;
                    DirectConsumerKindV1::from_numeric_tag(as_unsigned(&value[2])?)?;
                    let resource_rows = any_array(&value[3])?;
                    if resource_rows.is_empty() || resource_rows.len() > MAX_RELEASE_RESOURCES {
                        return Err(ResourceReleaseError::InvalidConsumerEdge);
                    }
                    let mut refs = Vec::new();
                    for resource_row in resource_rows {
                        let resource_row = exact_array(resource_row, 2)?;
                        refs.push(ResourceRefV1 {
                            resource_tag: as_unsigned(&resource_row[0])?,
                            resource_id: as_commitment(&resource_row[1])?,
                        });
                    }
                    validate_strict_resource_refs(&refs, true)?;
                    for resource_ref in refs {
                        let resource = known_resources
                            .get(&resource_ref.resource_id)
                            .ok_or(ResourceReleaseError::InvalidConsumerEdge)?;
                        if resource.resource_tag != resource_ref.resource_tag {
                            return Err(ResourceReleaseError::InvalidConsumerEdge);
                        }
                        consumer_edges.push(ConsumerEdgeV1 {
                            consumer_id,
                            resource_tag: resource_ref.resource_tag,
                            resource_id: resource_ref.resource_id,
                        });
                        *edge_count
                            .get_mut(&resource_ref.resource_id)
                            .ok_or(ResourceReleaseError::InvalidConsumerEdge)? += 1;
                    }
                    as_commitment(&value[4])?;
                    ResourceDispositionV1::from_numeric_tag(as_unsigned(&value[5])?)?;
                    as_commitment(&value[6])?;
                    as_commitment(&value[7])?;
                    as_commitment(&value[8])?;
                }
                _ => return Err(ResourceReleaseError::InvalidCensusDescriptor),
            }
        }
        validate_strict_tags(&row_tags, true)?;
        if resource_ids
            != resources
                .iter()
                .map(ResourceDescriptorV1::id)
                .collect::<Vec<_>>()
        {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }
        if as_unsigned(&header[5])? != usize_to_u64(resource_ids.len())?
            || as_unsigned(&header[6])? != usize_to_u64(consumer_count)?
            || as_unsigned(&header[7])? != usize_to_u64(consumer_edges.len())?
        {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }
        as_commitment(&header[8])?;
        as_commitment(&header[9])?;
        as_commitment(&header[10])?;
        let dependencies = bundles
            .iter()
            .map(|bundle| (bundle.bundle_tag, bundle.bundle_id))
            .collect::<Vec<_>>();
        validate_manifest_core(
            &header[0],
            ManifestCoreExpectations {
                generated_sum_schema_id: RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID,
                descriptor_schema_id: RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID,
                header_schema_id: RELEASE_RESOURCE_CENSUS_HEADER_SCHEMA_ID,
                manifest_schema_id: RELEASE_RESOURCE_CENSUS_SCHEMA_ID,
                dependency_manifest_ids: &dependencies,
                row_count: rows.len(),
                max_row_tag: *row_tags
                    .last()
                    .ok_or(ResourceReleaseError::InvalidCensusClosure)?,
            },
        )?;
        for resource in resources {
            if resource.disposition == ResourceDispositionV1::Remove
                && edge_count.get(&resource.id()).copied().unwrap_or_default() != 0
            {
                return Err(ResourceReleaseError::RemoveHasDirectConsumers);
            }
        }
        consumer_edges.sort();
        let value = CborValue::Array(vec![slots[3].clone(), slots[4].clone()]);
        let canonical_cbor = encode_manifest_identity(&envelope)?;
        validate_manifest_identity_cbor(&canonical_cbor)?;
        let census_id = sha256_commitment(&canonical_cbor);
        Ok(Self {
            resource_ids,
            bundle_ids: bundles.iter().map(BundleManifestV1::id).collect(),
            consumer_edges,
            resource_locators,
            value,
            envelope,
            census_id,
            canonical_cbor,
        })
    }

    pub const fn id(&self) -> ReleaseResourceCensusIdV1 {
        self.census_id
    }

    pub fn resource_ids(&self) -> &[ResourceIdV1] {
        &self.resource_ids
    }

    pub fn bundle_ids(&self) -> &[BundleIdV1] {
        &self.bundle_ids
    }

    pub fn consumer_edges(&self) -> &[ConsumerEdgeV1] {
        &self.consumer_edges
    }

    pub fn resource_locator(&self, resource_id: ResourceIdV1) -> Option<&str> {
        self.resource_locators.get(&resource_id).map(String::as_str)
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn envelope(&self) -> &CborValue {
        &self.envelope
    }

    pub fn canonical_cbor(&self) -> &[u8] {
        &self.canonical_cbor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedReleaseInputV1 {
    pub release_key: String,
    pub release_version: String,
    pub platform_qualifier: String,
    pub core_contract_root_id: CommitmentV1,
    pub binary_compatibility_id: CommitmentV1,
    pub public_catalog_id: CommitmentV1,
    pub compatibility_profile_id: CommitmentV1,
    pub rollback_profile_id: CommitmentV1,
    pub uninstall_profile_id: CommitmentV1,
    pub retention_profile_id: CommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedReleaseBundleV1 {
    bundle_ids: Vec<BundleIdV1>,
    census_id: ReleaseResourceCensusIdV1,
    value: CborValue,
    envelope: CborValue,
    release_id: ReleaseIdV1,
    canonical_cbor: Vec<u8>,
}

impl EmbeddedReleaseBundleV1 {
    pub fn new(
        input: EmbeddedReleaseInputV1,
        resources: &[ResourceDescriptorV1],
        bundles: &[BundleManifestV1],
        census: &ReleaseResourceCensusV1,
    ) -> Result<Self, ResourceReleaseError> {
        validate_ascii(&input.release_key, 1, 512)?;
        validate_ascii(&input.release_version, 1, 128)?;
        validate_ascii(&input.platform_qualifier, 1, 256)?;
        validate_bundle_topology(bundles)?;
        let rebuilt_census =
            ReleaseResourceCensusV1::from_envelope(census.envelope.clone(), resources, bundles)?;
        if rebuilt_census != *census {
            return Err(ResourceReleaseError::InvalidCensusClosure);
        }
        if census.bundle_ids != bundles.iter().map(BundleManifestV1::id).collect::<Vec<_>>() {
            return Err(ResourceReleaseError::InvalidReleaseClosure);
        }
        let by_id = bundles
            .iter()
            .map(|bundle| (bundle.id(), bundle))
            .collect::<BTreeMap<_, _>>();
        let mut membership_rows = Vec::new();
        for bundle in bundles {
            let dependency_tags = bundle
                .dependency_bundles
                .iter()
                .map(|dependency| {
                    let resolved = by_id
                        .get(&dependency.bundle_id)
                        .ok_or(ResourceReleaseError::InvalidBundleDependency)?;
                    if resolved.bundle_tag != dependency.bundle_tag {
                        return Err(ResourceReleaseError::InvalidBundleDependency);
                    }
                    Ok(dependency.bundle_tag)
                })
                .collect::<Result<Vec<_>, ResourceReleaseError>>()?;
            let membership = CborValue::Array(vec![
                CborValue::Unsigned(bundle.bundle_tag),
                CborValue::Unsigned(bundle.bundle_kind.numeric_tag()),
                bundle.bundle_id.canonical_value(),
                CborValue::Array(
                    dependency_tags
                        .iter()
                        .map(|tag| CborValue::Array(vec![CborValue::Unsigned(*tag)]))
                        .collect(),
                ),
            ]);
            membership_rows.push(CborValue::Array(vec![
                CborValue::Unsigned(bundle.bundle_tag),
                descriptor_identity(
                    RELEASE_MEMBERSHIP_DESCRIPTOR_DOMAIN,
                    RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
                    &membership,
                )?
                .canonical_value(),
                membership,
            ]));
        }
        let last_bundle_tag = bundles
            .last()
            .ok_or(ResourceReleaseError::InvalidBundleTopology)?
            .bundle_tag;
        let mut dependencies = bundles
            .iter()
            .map(|bundle| (bundle.bundle_tag, bundle.bundle_id))
            .collect::<Vec<_>>();
        dependencies.push((last_bundle_tag + 1, census.census_id));
        let core = make_manifest_core(
            RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
            RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
            EMBEDDED_RELEASE_HEADER_SCHEMA_ID,
            EMBEDDED_RELEASE_BUNDLE_SCHEMA_ID,
            &dependencies,
            membership_rows.len(),
            last_bundle_tag,
        )?;
        let header = CborValue::Array(vec![
            core,
            CborValue::Text(input.release_key),
            CborValue::Unsigned(BundleKindV1::Release.numeric_tag()),
            CborValue::Text(input.release_version),
            CborValue::Text(input.platform_qualifier),
            input.core_contract_root_id.canonical_value(),
            input.binary_compatibility_id.canonical_value(),
            input.public_catalog_id.canonical_value(),
            census.census_id.canonical_value(),
            input.compatibility_profile_id.canonical_value(),
            input.rollback_profile_id.canonical_value(),
            input.uninstall_profile_id.canonical_value(),
            input.retention_profile_id.canonical_value(),
        ]);
        let rows = CborValue::Array(membership_rows);
        let value = CborValue::Array(vec![header.clone(), rows.clone()]);
        let envelope = manifest_envelope(
            EMBEDDED_RELEASE_MANIFEST_DOMAIN,
            EMBEDDED_RELEASE_BUNDLE_SCHEMA_ID,
            RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
            header,
            rows,
        )?;
        let canonical_cbor = encode_manifest_identity(&envelope)?;
        let release_id = sha256_commitment(&canonical_cbor);
        if contains_commitment(&value, release_id) {
            return Err(ResourceReleaseError::IdentityBackreference);
        }
        let release = Self {
            bundle_ids: bundles.iter().map(BundleManifestV1::id).collect(),
            census_id: census.census_id,
            value,
            envelope,
            release_id,
            canonical_cbor,
        };
        validate_embedded_release(&release, bundles, census)?;
        Ok(release)
    }

    pub const fn release_id(&self) -> ReleaseIdV1 {
        self.release_id
    }

    pub const fn census_id(&self) -> ReleaseResourceCensusIdV1 {
        self.census_id
    }

    pub fn bundle_ids(&self) -> &[BundleIdV1] {
        &self.bundle_ids
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn envelope(&self) -> &CborValue {
        &self.envelope
    }

    pub fn canonical_cbor(&self) -> &[u8] {
        &self.canonical_cbor
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeltaIdentityKindV1 {
    Schema,
    Manifest,
    Resource,
    Bundle,
    Census,
    Release,
}

impl DeltaIdentityKindV1 {
    pub const ALL: [Self; 6] = [
        Self::Schema,
        Self::Manifest,
        Self::Resource,
        Self::Bundle,
        Self::Census,
        Self::Release,
    ];

    const fn numeric_tag(self) -> u64 {
        match self {
            Self::Schema => 1,
            Self::Manifest => 2,
            Self::Resource => 3,
            Self::Bundle => 4,
            Self::Census => 5,
            Self::Release => 6,
        }
    }
}

closed_enum!(DeltaDispositionV1 {
    Introduce = 1,
    Preserve = 2,
    Rotate = 3,
    Retire = 4,
});

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityDeltaEntryV1 {
    pub identity_kind: DeltaIdentityKindV1,
    pub logical_key: String,
    pub predecessor: Option<CommitmentV1>,
    pub successor: CommitmentV1,
    pub disposition: DeltaDispositionV1,
    pub source_artifact: String,
    pub source_artifact_sha256: CommitmentV1,
}

impl IdentityDeltaEntryV1 {
    pub fn new(
        identity_kind: DeltaIdentityKindV1,
        logical_key: impl Into<String>,
        predecessor: Option<CommitmentV1>,
        successor: CommitmentV1,
        disposition: DeltaDispositionV1,
        source_artifact: impl Into<String>,
        source_artifact_sha256: CommitmentV1,
    ) -> Result<Self, ResourceReleaseError> {
        let entry = Self {
            identity_kind,
            logical_key: logical_key.into(),
            predecessor,
            successor,
            disposition,
            source_artifact: source_artifact.into(),
            source_artifact_sha256,
        };
        validate_ascii(&entry.logical_key, 1, 4_096)?;
        validate_ascii(&entry.source_artifact, 1, 4_096)?;
        let valid_transition = match (entry.predecessor, entry.disposition) {
            (None, DeltaDispositionV1::Introduce) => true,
            (Some(old), DeltaDispositionV1::Preserve) => old == entry.successor,
            (Some(old), DeltaDispositionV1::Rotate) => old != entry.successor,
            _ => false,
        };
        if !valid_transition {
            return Err(ResourceReleaseError::InvalidIdentityDelta);
        }
        Ok(entry)
    }

    fn canonical_value(&self, ordinal: usize) -> Result<CborValue, ResourceReleaseError> {
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(usize_to_u64(ordinal)?),
            CborValue::Unsigned(self.identity_kind.numeric_tag()),
            CborValue::Text(self.logical_key.clone()),
            optional_commitment(self.predecessor),
            self.successor.canonical_value(),
            CborValue::Unsigned(self.disposition.numeric_tag()),
            CborValue::Text(self.source_artifact.clone()),
            self.source_artifact_sha256.canonical_value(),
        ]))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DownstreamObligationKindV1 {
    CandidateRoot,
    CandidateFinalization,
    CandidateHandoff,
}

impl DownstreamObligationKindV1 {
    pub const ALL: [Self; 3] = [
        Self::CandidateRoot,
        Self::CandidateFinalization,
        Self::CandidateHandoff,
    ];

    const fn numeric_tag(self) -> u64 {
        match self {
            Self::CandidateRoot => 1,
            Self::CandidateFinalization => 2,
            Self::CandidateHandoff => 3,
        }
    }

    const fn logical_key(self) -> &'static str {
        match self {
            Self::CandidateRoot => "candidate-root",
            Self::CandidateFinalization => "candidate-finalization",
            Self::CandidateHandoff => "candidate-handoff",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DownstreamIdentityObligationV1 {
    pub kind: DownstreamObligationKindV1,
    pub depends_on_release: ReleaseIdV1,
}

impl DownstreamIdentityObligationV1 {
    pub const fn new(kind: DownstreamObligationKindV1, depends_on_release: ReleaseIdV1) -> Self {
        Self {
            kind,
            depends_on_release,
        }
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.numeric_tag()),
            CborValue::Text(self.kind.logical_key().to_owned()),
            self.depends_on_release.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedDeltaClosureV1 {
    id: CommitmentV1,
    entries: Vec<IdentityDeltaEntryV1>,
    downstream_obligations: Vec<DownstreamIdentityObligationV1>,
}

impl ExpectedDeltaClosureV1 {
    pub fn new(
        mut entries: Vec<IdentityDeltaEntryV1>,
        mut downstream_obligations: Vec<DownstreamIdentityObligationV1>,
    ) -> Result<Self, ResourceReleaseError> {
        entries.sort_by(|left, right| {
            (left.identity_kind, &left.logical_key).cmp(&(right.identity_kind, &right.logical_key))
        });
        downstream_obligations.sort_by_key(|obligation| obligation.kind);
        let kinds = entries
            .iter()
            .map(|entry| entry.identity_kind)
            .collect::<BTreeSet<_>>();
        let keys = entries
            .iter()
            .map(|entry| (entry.identity_kind, entry.logical_key.as_str()))
            .collect::<BTreeSet<_>>();
        let successors = entries
            .iter()
            .map(|entry| entry.successor)
            .collect::<BTreeSet<_>>();
        if kinds != DeltaIdentityKindV1::ALL.into_iter().collect()
            || keys.len() != entries.len()
            || successors.len() != entries.len()
        {
            return Err(ResourceReleaseError::IncompleteIdentityDelta);
        }
        let obligation_kinds = downstream_obligations
            .iter()
            .map(|obligation| obligation.kind)
            .collect::<Vec<_>>();
        let release_ids = downstream_obligations
            .iter()
            .map(|obligation| obligation.depends_on_release)
            .collect::<BTreeSet<_>>();
        let release_successors = entries
            .iter()
            .filter(|entry| entry.identity_kind == DeltaIdentityKindV1::Release)
            .map(|entry| entry.successor)
            .collect::<BTreeSet<_>>();
        if obligation_kinds != DownstreamObligationKindV1::ALL
            || release_ids.len() != 1
            || release_ids != release_successors
        {
            return Err(ResourceReleaseError::InvalidDownstreamIdentityObligations);
        }
        let entry_values = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| entry.canonical_value(index + 1))
            .collect::<Result<Vec<_>, _>>()?;
        let obligation_values = downstream_obligations
            .iter()
            .copied()
            .map(DownstreamIdentityObligationV1::canonical_value)
            .collect();
        let value = CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Text("maestro.vnext.exact-identity-delta.v3".to_owned()),
            CborValue::Array(entry_values),
            CborValue::Array(obligation_values),
        ]);
        let envelope = CborValue::Array(vec![
            CborValue::Text(DELTA_IDENTITY_DOMAIN.to_owned()),
            value,
        ]);
        let id = identity_of_value(&envelope)?;
        Ok(Self {
            id,
            entries,
            downstream_obligations,
        })
    }

    pub const fn id(&self) -> CommitmentV1 {
        self.id
    }

    pub fn entries(&self) -> &[IdentityDeltaEntryV1] {
        &self.entries
    }

    pub fn downstream_obligations(&self) -> &[DownstreamIdentityObligationV1] {
        &self.downstream_obligations
    }
}

pub fn validate_release_closure(
    resources: &[ResourceDescriptorV1],
    bundles: &[BundleManifestV1],
    census: &ReleaseResourceCensusV1,
    release: &EmbeddedReleaseBundleV1,
) -> Result<(), ResourceReleaseError> {
    validate_release_closure_with_installed_state(resources, bundles, census, release, &[])
}

pub fn validate_release_closure_with_installed_state(
    resources: &[ResourceDescriptorV1],
    bundles: &[BundleManifestV1],
    census: &ReleaseResourceCensusV1,
    release: &EmbeddedReleaseBundleV1,
    installed_state_ids: &[CommitmentV1],
) -> Result<(), ResourceReleaseError> {
    validate_bundle_topology(bundles)?;
    validate_resource_and_bundle_closure(resources, bundles)?;
    let resource_ids = resources
        .iter()
        .map(ResourceDescriptorV1::id)
        .collect::<BTreeSet<_>>();
    for resource in resources {
        resource.validate()?;
        let value_without_dependencies = resource_value_without_dependencies(resource)?;
        for resource_id in &resource_ids {
            if contains_commitment(&value_without_dependencies, *resource_id) {
                return Err(ResourceReleaseError::IdentityBackreference);
            }
        }
        for dependency in &resource.dependencies {
            let resolved = resources
                .iter()
                .find(|candidate| candidate.id() == dependency.resource_id)
                .ok_or(ResourceReleaseError::InvalidResourceDependency)?;
            if resolved.resource_tag != dependency.resource_tag {
                return Err(ResourceReleaseError::InvalidResourceDependency);
            }
        }
    }
    let rebuilt_census =
        ReleaseResourceCensusV1::from_envelope(census.envelope.clone(), resources, bundles)?;
    if rebuilt_census != *census {
        return Err(ResourceReleaseError::InvalidCensusClosure);
    }
    validate_embedded_release(release, bundles, census)?;
    if release.census_id != census.census_id || release.bundle_ids != census.bundle_ids {
        return Err(ResourceReleaseError::InvalidReleaseClosure);
    }

    let upward_ids = bundles
        .iter()
        .map(BundleManifestV1::id)
        .chain([census.id(), release.release_id()])
        .chain(installed_state_ids.iter().copied())
        .collect::<Vec<_>>();
    for resource in resources {
        reject_commitments(resource.value(), &upward_ids)?;
    }
    for bundle in bundles {
        let allowed = bundle
            .dependency_bundles
            .iter()
            .map(|dependency| dependency.bundle_id)
            .collect::<BTreeSet<_>>();
        let forbidden = bundles
            .iter()
            .map(BundleManifestV1::id)
            .filter(|bundle_id| !allowed.contains(bundle_id))
            .chain([census.id(), release.release_id()])
            .chain(installed_state_ids.iter().copied())
            .collect::<Vec<_>>();
        reject_commitments(bundle.value(), &forbidden)?;
    }
    reject_commitments(
        census.value(),
        &std::iter::once(release.release_id())
            .chain(installed_state_ids.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    reject_commitments(
        release.value(),
        &std::iter::once(release.release_id())
            .chain(installed_state_ids.iter().copied())
            .collect::<Vec<_>>(),
    )
}

pub fn release_census_entry_descriptor_id(
    entry: &CborValue,
) -> Result<CommitmentV1, ResourceReleaseError> {
    descriptor_identity(
        RELEASE_CENSUS_ROW_DESCRIPTOR_DOMAIN,
        RELEASE_RESOURCE_CENSUS_ENTRY_SCHEMA_ID,
        entry,
    )
}

#[derive(Debug)]
struct ResourceCoordinates {
    resource_tag: u64,
    stable_resource_key: String,
    required_bundle_kind: BundleKindV1,
    disposition: ResourceDispositionV1,
    owner: OwnerRefV1,
    dependencies: Vec<ResourceRefV1>,
}

fn validate_resource_value(value: &CborValue) -> Result<ResourceCoordinates, ResourceReleaseError> {
    let fields = exact_array(value, RESOURCE_DESCRIPTOR_FIELD_COUNT)?;
    let resource_tag = as_unsigned(&fields[0])?;
    validate_u64(resource_tag, 1, 4_096)?;
    let stable_resource_key = as_text(&fields[1])?;
    validate_ascii(stable_resource_key, 1, 512)?;
    as_commitment(&fields[2])?;
    as_unsigned(&fields[3])?;
    ContentEncodingV1::from_numeric_tag(as_unsigned(&fields[4])?)?;
    validate_ascii(as_text(&fields[5])?, 1, 128)?;
    ResourceKindV1::from_numeric_tag(as_unsigned(&fields[6])?)?;
    let owner = parse_owner(&fields[7])?;
    let required_bundle_kind = BundleKindV1::from_numeric_tag(as_unsigned(&fields[8])?)?;
    ResourceProvenanceKindV1::from_numeric_tag(as_unsigned(&fields[9])?)?;
    as_commitment(&fields[10])?;
    parse_optional_commitment(&fields[11])?;
    let dependency_rows = any_array(&fields[12])?;
    if dependency_rows.len() > MAX_RESOURCE_DEPENDENCIES {
        return Err(ResourceReleaseError::InvalidResourceDependency);
    }
    let mut dependencies = Vec::new();
    for row in dependency_rows {
        let row = exact_array(row, 2)?;
        dependencies.push(ResourceRefV1 {
            resource_tag: as_unsigned(&row[0])?,
            resource_id: as_commitment(&row[1])?,
        });
    }
    validate_strict_resource_refs(&dependencies, false)?;
    if dependencies
        .iter()
        .any(|dependency| dependency.resource_tag >= resource_tag)
    {
        return Err(ResourceReleaseError::InvalidResourceDependency);
    }
    as_commitment(&fields[13])?;
    parse_optional_commitment(&fields[14])?;
    for field in &fields[15..=21] {
        as_commitment(field)?;
    }
    let disposition = ResourceDispositionV1::from_numeric_tag(as_unsigned(&fields[22])?)?;
    as_commitment(&fields[23])?;
    Ok(ResourceCoordinates {
        resource_tag,
        stable_resource_key: stable_resource_key.to_owned(),
        required_bundle_kind,
        disposition,
        owner,
        dependencies,
    })
}

fn validate_bundle_manifest(bundle: &BundleManifestV1) -> Result<(), ResourceReleaseError> {
    let slots = exact_array(&bundle.envelope, MANIFEST_ENVELOPE_SLOT_COUNT)?;
    exact_text(&slots[0], BUNDLE_MANIFEST_DOMAIN)?;
    exact_commitment(&slots[1], BUNDLE_MANIFEST_SCHEMA_ID)?;
    exact_commitment(&slots[2], RESOURCE_DESCRIPTOR_SCHEMA_ID)?;
    if bundle.value != CborValue::Array(vec![slots[3].clone(), slots[4].clone()]) {
        return Err(ResourceReleaseError::InvalidBundleManifest);
    }
    let header = exact_array(&slots[3], 14)?;
    let rows = any_array(&slots[4])?;
    let bundle_tag = as_unsigned(&header[1])?;
    validate_u64(bundle_tag, 1, 256)?;
    if bundle_tag != bundle.bundle_tag {
        return Err(ResourceReleaseError::InvalidBundleManifest);
    }
    validate_ascii(as_text(&header[2])?, 1, 512)?;
    let bundle_kind = BundleKindV1::from_numeric_tag(as_unsigned(&header[3])?)?;
    if bundle_kind != bundle.bundle_kind || bundle_kind == BundleKindV1::Release {
        return Err(ResourceReleaseError::InvalidBundleKind);
    }
    validate_ascii(as_text(&header[4])?, 1, 128)?;
    as_commitment(&header[5])?;
    let dependency_rows = any_array(&header[6])?;
    if dependency_rows.len() > MAX_BUNDLE_DEPENDENCIES {
        return Err(ResourceReleaseError::InvalidBundleDependency);
    }
    let mut dependency_refs = Vec::new();
    for row in dependency_rows {
        let row = exact_array(row, 2)?;
        dependency_refs.push(BundleRefV1 {
            bundle_tag: as_unsigned(&row[0])?,
            bundle_id: as_commitment(&row[1])?,
        });
    }
    validate_strict_bundle_refs(&dependency_refs, false)?;
    if dependency_refs
        .iter()
        .any(|dependency| dependency.bundle_tag >= bundle_tag)
        || dependency_refs != bundle.dependency_bundles
    {
        return Err(ResourceReleaseError::InvalidBundleDependency);
    }
    as_commitment(&header[7])?;
    parse_optional_commitment(&header[8])?;
    as_commitment(&header[9])?;
    let target_rows = any_array(&header[10])?;
    if !(1..=6).contains(&target_rows.len()) {
        return Err(ResourceReleaseError::InvalidTargetClassClosure);
    }
    let mut target_tags = Vec::new();
    for row in target_rows {
        let row = exact_array(row, 2)?;
        let row_tag = as_unsigned(&row[0])?;
        validate_u64(row_tag, 1, u64::MAX)?;
        TargetClassV1::from_numeric_tag(as_unsigned(&row[1])?)?;
        target_tags.push(row_tag);
    }
    validate_strict_tags(&target_tags, true)?;
    as_commitment(&header[11])?;
    as_commitment(&header[12])?;
    as_commitment(&header[13])?;
    if !(1..=MAX_RESOURCES_PER_BUNDLE).contains(&rows.len()) {
        return Err(ResourceReleaseError::InvalidBundleMembership);
    }
    let mut resource_refs = Vec::new();
    for row in rows {
        let row = exact_array(row, 3)?;
        let row_tag = as_unsigned(&row[0])?;
        let resource_id = as_commitment(&row[1])?;
        let coordinates = validate_resource_value(&row[2])?;
        if coordinates.resource_tag != row_tag
            || descriptor_identity(
                RESOURCE_DESCRIPTOR_DOMAIN,
                RESOURCE_DESCRIPTOR_SCHEMA_ID,
                &row[2],
            )? != resource_id
        {
            return Err(ResourceReleaseError::InvalidBundleMembership);
        }
        resource_refs.push(ResourceRefV1 {
            resource_tag: row_tag,
            resource_id,
        });
    }
    validate_strict_resource_refs(&resource_refs, true)?;
    if resource_refs
        .iter()
        .map(|resource| resource.resource_id)
        .ne(bundle.resource_ids.iter().copied())
    {
        return Err(ResourceReleaseError::InvalidBundleMembership);
    }
    let dependencies = dependency_refs
        .iter()
        .map(|dependency| (dependency.bundle_tag, dependency.bundle_id))
        .collect::<Vec<_>>();
    validate_manifest_core(
        &header[0],
        ManifestCoreExpectations {
            generated_sum_schema_id: RESOURCE_DESCRIPTOR_SCHEMA_ID,
            descriptor_schema_id: RESOURCE_DESCRIPTOR_SCHEMA_ID,
            header_schema_id: BUNDLE_MANIFEST_HEADER_SCHEMA_ID,
            manifest_schema_id: BUNDLE_MANIFEST_SCHEMA_ID,
            dependency_manifest_ids: &dependencies,
            row_count: rows.len(),
            max_row_tag: resource_refs
                .last()
                .ok_or(ResourceReleaseError::InvalidBundleMembership)?
                .resource_tag,
        },
    )?;
    let canonical = encode_manifest_identity(&bundle.envelope)?;
    if canonical != bundle.canonical_cbor || sha256_commitment(&canonical) != bundle.bundle_id {
        return Err(ResourceReleaseError::InvalidBundleManifest);
    }
    if contains_commitment(&bundle.value, bundle.bundle_id) {
        return Err(ResourceReleaseError::IdentityBackreference);
    }
    Ok(())
}

fn validate_bundle_topology(bundles: &[BundleManifestV1]) -> Result<(), ResourceReleaseError> {
    if !(1..=MAX_RELEASE_BUNDLES).contains(&bundles.len()) {
        return Err(ResourceReleaseError::InvalidBundleTopology);
    }
    let bundle_tags = bundles
        .iter()
        .map(|bundle| bundle.bundle_tag)
        .collect::<Vec<_>>();
    validate_strict_tags(&bundle_tags, true)?;
    let kinds = bundles
        .iter()
        .map(|bundle| bundle.bundle_kind)
        .collect::<Vec<_>>();
    if kinds.iter().copied().collect::<BTreeSet<_>>()
        != BundleKindV1::NON_RELEASE_TOPOLOGY.into_iter().collect()
    {
        return Err(ResourceReleaseError::InvalidBundleTopology);
    }
    let ranks = kinds
        .iter()
        .map(|kind| {
            kind.topology_rank()
                .ok_or(ResourceReleaseError::InvalidBundleTopology)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ranks.windows(2).any(|window| window[0] > window[1]) {
        return Err(ResourceReleaseError::InvalidBundleTopology);
    }
    let mut by_id = BTreeMap::new();
    for bundle in bundles {
        bundle.validate()?;
        if by_id.insert(bundle.id(), bundle).is_some() {
            return Err(ResourceReleaseError::InvalidBundleTopology);
        }
    }
    for bundle in bundles {
        for dependency in &bundle.dependency_bundles {
            let resolved = by_id
                .get(&dependency.bundle_id)
                .ok_or(ResourceReleaseError::InvalidBundleDependency)?;
            if resolved.bundle_tag != dependency.bundle_tag
                || resolved.bundle_tag >= bundle.bundle_tag
            {
                return Err(ResourceReleaseError::InvalidBundleDependency);
            }
        }
    }
    Ok(())
}

fn validate_resource_and_bundle_closure(
    resources: &[ResourceDescriptorV1],
    bundles: &[BundleManifestV1],
) -> Result<(), ResourceReleaseError> {
    if resources.is_empty() || resources.len() > MAX_RELEASE_RESOURCES {
        return Err(ResourceReleaseError::InvalidResourceClosure);
    }
    let resource_refs = resources
        .iter()
        .map(ResourceDescriptorV1::resource_ref)
        .collect::<Vec<_>>();
    validate_strict_resource_refs(&resource_refs, true)?;
    for resource in resources {
        resource.validate()?;
    }
    let known = resources
        .iter()
        .map(|resource| (resource.id(), resource))
        .collect::<BTreeMap<_, _>>();
    if known.len() != resources.len() {
        return Err(ResourceReleaseError::InvalidResourceClosure);
    }
    let mut admitted = BTreeSet::new();
    for bundle in bundles {
        bundle.validate()?;
        for resource_id in &bundle.resource_ids {
            let resource = known
                .get(resource_id)
                .ok_or(ResourceReleaseError::InvalidBundleMembership)?;
            if resource.required_bundle_kind != bundle.bundle_kind || !admitted.insert(*resource_id)
            {
                return Err(ResourceReleaseError::ResourceInWrongBundle);
            }
        }
    }
    if admitted != known.keys().copied().collect() {
        return Err(ResourceReleaseError::InvalidBundleMembership);
    }
    Ok(())
}

fn bundle_by_resource(
    bundles: &[BundleManifestV1],
) -> Result<BTreeMap<ResourceIdV1, &BundleManifestV1>, ResourceReleaseError> {
    let mut result = BTreeMap::new();
    for bundle in bundles {
        for resource_id in &bundle.resource_ids {
            if result.insert(*resource_id, bundle).is_some() {
                return Err(ResourceReleaseError::InvalidBundleMembership);
            }
        }
    }
    Ok(result)
}

fn validate_embedded_release(
    release: &EmbeddedReleaseBundleV1,
    bundles: &[BundleManifestV1],
    census: &ReleaseResourceCensusV1,
) -> Result<(), ResourceReleaseError> {
    let slots = exact_array(&release.envelope, MANIFEST_ENVELOPE_SLOT_COUNT)?;
    exact_text(&slots[0], EMBEDDED_RELEASE_MANIFEST_DOMAIN)?;
    exact_commitment(&slots[1], EMBEDDED_RELEASE_BUNDLE_SCHEMA_ID)?;
    exact_commitment(&slots[2], RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID)?;
    if release.value != CborValue::Array(vec![slots[3].clone(), slots[4].clone()]) {
        return Err(ResourceReleaseError::InvalidReleaseClosure);
    }
    let header = exact_array(&slots[3], 13)?;
    let rows = any_array(&slots[4])?;
    validate_ascii(as_text(&header[1])?, 1, 512)?;
    if BundleKindV1::from_numeric_tag(as_unsigned(&header[2])?)? != BundleKindV1::Release {
        return Err(ResourceReleaseError::InvalidReleaseClosure);
    }
    validate_ascii(as_text(&header[3])?, 1, 128)?;
    validate_ascii(as_text(&header[4])?, 1, 256)?;
    for field in &header[5..=12] {
        as_commitment(field)?;
    }
    if as_commitment(&header[8])? != census.census_id
        || release.census_id != census.census_id
        || release.bundle_ids != bundles.iter().map(BundleManifestV1::id).collect::<Vec<_>>()
    {
        return Err(ResourceReleaseError::InvalidReleaseClosure);
    }
    if rows.len() != bundles.len() || !(1..=MAX_RELEASE_BUNDLES).contains(&rows.len()) {
        return Err(ResourceReleaseError::InvalidReleaseClosure);
    }
    let by_id = bundles
        .iter()
        .map(|bundle| (bundle.id(), bundle))
        .collect::<BTreeMap<_, _>>();
    let mut row_tags = Vec::new();
    for (row, bundle) in rows.iter().zip(bundles) {
        let row = exact_array(row, 3)?;
        let row_tag = as_unsigned(&row[0])?;
        row_tags.push(row_tag);
        if row_tag != bundle.bundle_tag {
            return Err(ResourceReleaseError::InvalidReleaseClosure);
        }
        let membership = exact_array(&row[2], 4)?;
        if as_unsigned(&membership[0])? != bundle.bundle_tag
            || BundleKindV1::from_numeric_tag(as_unsigned(&membership[1])?)? != bundle.bundle_kind
            || as_commitment(&membership[2])? != bundle.bundle_id
        {
            return Err(ResourceReleaseError::InvalidReleaseClosure);
        }
        let dependency_rows = any_array(&membership[3])?;
        if dependency_rows.len() > MAX_BUNDLE_DEPENDENCIES {
            return Err(ResourceReleaseError::InvalidBundleDependency);
        }
        let mut dependency_tags = Vec::new();
        for dependency_row in dependency_rows {
            let dependency_row = exact_array(dependency_row, 1)?;
            dependency_tags.push(as_unsigned(&dependency_row[0])?);
        }
        validate_strict_tags(&dependency_tags, false)?;
        let expected_dependency_tags = bundle
            .dependency_bundles
            .iter()
            .map(|dependency| {
                let resolved = by_id
                    .get(&dependency.bundle_id)
                    .ok_or(ResourceReleaseError::InvalidBundleDependency)?;
                if resolved.bundle_tag != dependency.bundle_tag {
                    return Err(ResourceReleaseError::InvalidBundleDependency);
                }
                Ok(dependency.bundle_tag)
            })
            .collect::<Result<Vec<_>, ResourceReleaseError>>()?;
        if dependency_tags != expected_dependency_tags
            || as_commitment(&row[1])?
                != descriptor_identity(
                    RELEASE_MEMBERSHIP_DESCRIPTOR_DOMAIN,
                    RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
                    &row[2],
                )?
        {
            return Err(ResourceReleaseError::InvalidReleaseClosure);
        }
    }
    validate_strict_tags(&row_tags, true)?;
    let mut dependencies = bundles
        .iter()
        .map(|bundle| (bundle.bundle_tag, bundle.bundle_id))
        .collect::<Vec<_>>();
    dependencies.push((
        row_tags
            .last()
            .copied()
            .ok_or(ResourceReleaseError::InvalidReleaseClosure)?
            + 1,
        census.census_id,
    ));
    validate_manifest_core(
        &header[0],
        ManifestCoreExpectations {
            generated_sum_schema_id: RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
            descriptor_schema_id: RELEASE_BUNDLE_MEMBERSHIP_SCHEMA_ID,
            header_schema_id: EMBEDDED_RELEASE_HEADER_SCHEMA_ID,
            manifest_schema_id: EMBEDDED_RELEASE_BUNDLE_SCHEMA_ID,
            dependency_manifest_ids: &dependencies,
            row_count: rows.len(),
            max_row_tag: *row_tags
                .last()
                .ok_or(ResourceReleaseError::InvalidReleaseClosure)?,
        },
    )?;
    let canonical = encode_manifest_identity(&release.envelope)?;
    if canonical != release.canonical_cbor
        || sha256_commitment(&canonical) != release.release_id
        || contains_commitment(&release.value, release.release_id)
    {
        return Err(ResourceReleaseError::InvalidReleaseClosure);
    }
    Ok(())
}

fn make_manifest_core(
    generated_sum_schema_id: &str,
    descriptor_schema_id: &str,
    header_schema_id: &str,
    manifest_schema_id: &str,
    dependency_manifest_ids: &[(u64, CommitmentV1)],
    row_count: usize,
    max_row_tag: u64,
) -> Result<CborValue, ResourceReleaseError> {
    let dependency_tags = dependency_manifest_ids
        .iter()
        .map(|(tag, _)| *tag)
        .collect::<Vec<_>>();
    validate_strict_tags(&dependency_tags, false)?;
    if dependency_manifest_ids.len() > MAX_RESOURCES_PER_BUNDLE {
        return Err(ResourceReleaseError::InvalidManifestCore);
    }
    let row_count = usize_to_u64(row_count)?;
    if row_count != 0 && max_row_tag < row_count {
        return Err(ResourceReleaseError::InvalidManifestCore);
    }
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(1),
        CborValue::Unsigned(1),
        CommitmentV1::from_hex(generated_sum_schema_id)?.canonical_value(),
        CommitmentV1::from_hex(descriptor_schema_id)?.canonical_value(),
        CommitmentV1::from_hex(header_schema_id)?.canonical_value(),
        CommitmentV1::from_hex(manifest_schema_id)?.canonical_value(),
        CborValue::Array(
            dependency_manifest_ids
                .iter()
                .map(|(tag, identity)| {
                    CborValue::Array(vec![CborValue::Unsigned(*tag), identity.canonical_value()])
                })
                .collect(),
        ),
        CommitmentV1::from_hex(MANIFEST_IDENTITY_PROTOCOL_SHA256)?.canonical_value(),
        CommitmentV1::from_hex(OWNER_PROTOCOL_ID)?.canonical_value(),
        CborValue::Unsigned(row_count),
        CborValue::Unsigned(max_row_tag),
        CommitmentV1::from_hex(DEFAULT_MIGRATION_PROFILE_ID)?.canonical_value(),
        CommitmentV1::from_hex(DEFAULT_PARITY_PROFILE_ID)?.canonical_value(),
        CommitmentV1::from_hex(DEFAULT_REMOVAL_PROFILE_ID)?.canonical_value(),
        CommitmentV1::from_hex(DEFAULT_PROOF_PROFILE_ID)?.canonical_value(),
    ]))
}

struct ManifestCoreExpectations<'a> {
    generated_sum_schema_id: &'a str,
    descriptor_schema_id: &'a str,
    header_schema_id: &'a str,
    manifest_schema_id: &'a str,
    dependency_manifest_ids: &'a [(u64, CommitmentV1)],
    row_count: usize,
    max_row_tag: u64,
}

fn validate_manifest_core(
    core: &CborValue,
    expected: ManifestCoreExpectations<'_>,
) -> Result<(), ResourceReleaseError> {
    let fields = exact_array(core, 16)?;
    if fields[..3]
        != [
            CborValue::Unsigned(1),
            CborValue::Unsigned(1),
            CborValue::Unsigned(1),
        ]
    {
        return Err(ResourceReleaseError::InvalidManifestCore);
    }
    for (field, expected) in fields[3..7].iter().zip([
        expected.generated_sum_schema_id,
        expected.descriptor_schema_id,
        expected.header_schema_id,
        expected.manifest_schema_id,
    ]) {
        exact_commitment(field, expected)?;
    }
    let dependency_rows = any_array(&fields[7])?;
    if dependency_rows.len() > MAX_RESOURCES_PER_BUNDLE {
        return Err(ResourceReleaseError::InvalidManifestCore);
    }
    let mut actual_dependencies = Vec::new();
    for row in dependency_rows {
        let row = exact_array(row, 2)?;
        actual_dependencies.push((as_unsigned(&row[0])?, as_commitment(&row[1])?));
    }
    validate_strict_tags(
        &actual_dependencies
            .iter()
            .map(|(tag, _)| *tag)
            .collect::<Vec<_>>(),
        false,
    )?;
    if actual_dependencies != expected.dependency_manifest_ids {
        return Err(ResourceReleaseError::InvalidManifestCore);
    }
    exact_commitment(&fields[8], MANIFEST_IDENTITY_PROTOCOL_SHA256)?;
    exact_commitment(&fields[9], OWNER_PROTOCOL_ID)?;
    if as_unsigned(&fields[10])? != usize_to_u64(expected.row_count)?
        || as_unsigned(&fields[11])? != expected.max_row_tag
    {
        return Err(ResourceReleaseError::InvalidManifestCore);
    }
    for (field, expected) in fields[12..16].iter().zip([
        DEFAULT_MIGRATION_PROFILE_ID,
        DEFAULT_PARITY_PROFILE_ID,
        DEFAULT_REMOVAL_PROFILE_ID,
        DEFAULT_PROOF_PROFILE_ID,
    ]) {
        exact_commitment(field, expected)?;
    }
    Ok(())
}

fn descriptor_envelope(
    domain: &str,
    schema_id: &str,
    value: CborValue,
) -> Result<CborValue, ResourceReleaseError> {
    Ok(CborValue::Array(vec![
        CborValue::Text(domain.to_owned()),
        CommitmentV1::from_hex(schema_id)?.canonical_value(),
        value,
    ]))
}

fn manifest_envelope(
    domain: &str,
    manifest_schema_id: &str,
    descriptor_schema_id: &str,
    header: CborValue,
    rows: CborValue,
) -> Result<CborValue, ResourceReleaseError> {
    Ok(CborValue::Array(vec![
        CborValue::Text(domain.to_owned()),
        CommitmentV1::from_hex(manifest_schema_id)?.canonical_value(),
        CommitmentV1::from_hex(descriptor_schema_id)?.canonical_value(),
        header,
        rows,
    ]))
}

fn descriptor_identity(
    domain: &str,
    schema_id: &str,
    value: &CborValue,
) -> Result<CommitmentV1, ResourceReleaseError> {
    identity_of_value(&descriptor_envelope(domain, schema_id, value.clone())?)
}

fn identity_of_value(value: &CborValue) -> Result<CommitmentV1, ResourceReleaseError> {
    Ok(sha256_commitment(&encode_manifest_identity(value)?))
}

fn sha256_commitment(bytes: &[u8]) -> CommitmentV1 {
    CommitmentV1::from_bytes(Sha256::digest(bytes).into())
}

fn optional_commitment(value: Option<CommitmentV1>) -> CborValue {
    match value {
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), value.canonical_value()]),
    }
}

fn parse_optional(value: &CborValue) -> Result<Option<&CborValue>, ResourceReleaseError> {
    let fields = any_array(value)?;
    match fields {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), value] => Ok(Some(value)),
        _ => Err(ResourceReleaseError::InvalidOptional),
    }
}

fn parse_optional_commitment(
    value: &CborValue,
) -> Result<Option<CommitmentV1>, ResourceReleaseError> {
    parse_optional(value)?.map(as_commitment).transpose()
}

fn parse_owner(value: &CborValue) -> Result<OwnerRefV1, ResourceReleaseError> {
    let fields = exact_array(value, 2)?;
    OwnerRefV1::new(as_unsigned(&fields[0])?, as_commitment(&fields[1])?)
}

fn exact_array(
    value: &CborValue,
    expected_len: usize,
) -> Result<&[CborValue], ResourceReleaseError> {
    let values = any_array(value)?;
    if values.len() != expected_len {
        return Err(ResourceReleaseError::InvalidShape);
    }
    Ok(values)
}

fn any_array(value: &CborValue) -> Result<&[CborValue], ResourceReleaseError> {
    match value {
        CborValue::Array(values) => Ok(values),
        _ => Err(ResourceReleaseError::InvalidShape),
    }
}

fn as_unsigned(value: &CborValue) -> Result<u64, ResourceReleaseError> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(ResourceReleaseError::InvalidShape),
    }
}

fn as_text(value: &CborValue) -> Result<&str, ResourceReleaseError> {
    match value {
        CborValue::Text(value) => Ok(value),
        _ => Err(ResourceReleaseError::InvalidShape),
    }
}

fn as_commitment(value: &CborValue) -> Result<CommitmentV1, ResourceReleaseError> {
    match value {
        CborValue::Bytes(bytes) if bytes.len() == 32 => {
            let mut value = [0_u8; 32];
            value.copy_from_slice(bytes);
            Ok(CommitmentV1::from_bytes(value))
        }
        _ => Err(ResourceReleaseError::InvalidCommitment),
    }
}

fn exact_text(value: &CborValue, expected: &str) -> Result<(), ResourceReleaseError> {
    if as_text(value)? != expected {
        return Err(ResourceReleaseError::InvalidIdentityEnvelope);
    }
    Ok(())
}

fn exact_commitment(value: &CborValue, expected: &str) -> Result<(), ResourceReleaseError> {
    if as_commitment(value)? != CommitmentV1::from_hex(expected)? {
        return Err(ResourceReleaseError::InvalidIdentityEnvelope);
    }
    Ok(())
}

fn validate_ascii(value: &str, minimum: usize, maximum: usize) -> Result<(), ResourceReleaseError> {
    if !value.is_ascii() || !(minimum..=maximum).contains(&value.len()) {
        return Err(ResourceReleaseError::InvalidAscii);
    }
    Ok(())
}

fn validate_u64(value: u64, minimum: u64, maximum: u64) -> Result<(), ResourceReleaseError> {
    if !(minimum..=maximum).contains(&value) {
        return Err(ResourceReleaseError::InvalidRange);
    }
    Ok(())
}

fn validate_strict_tags(tags: &[u64], require_nonempty: bool) -> Result<(), ResourceReleaseError> {
    if (require_nonempty && tags.is_empty())
        || tags.contains(&0)
        || tags.windows(2).any(|window| window[0] >= window[1])
    {
        return Err(ResourceReleaseError::InvalidStrictTags);
    }
    Ok(())
}

fn validate_strict_resource_refs(
    resources: &[ResourceRefV1],
    require_nonempty: bool,
) -> Result<(), ResourceReleaseError> {
    validate_strict_tags(
        &resources
            .iter()
            .map(|resource| resource.resource_tag)
            .collect::<Vec<_>>(),
        require_nonempty,
    )?;
    if resources
        .iter()
        .map(|resource| resource.resource_id)
        .collect::<BTreeSet<_>>()
        .len()
        != resources.len()
    {
        return Err(ResourceReleaseError::InvalidResourceDependency);
    }
    Ok(())
}

fn validate_strict_bundle_refs(
    bundles: &[BundleRefV1],
    require_nonempty: bool,
) -> Result<(), ResourceReleaseError> {
    validate_strict_tags(
        &bundles
            .iter()
            .map(|bundle| bundle.bundle_tag)
            .collect::<Vec<_>>(),
        require_nonempty,
    )?;
    if bundles
        .iter()
        .map(|bundle| bundle.bundle_id)
        .collect::<BTreeSet<_>>()
        .len()
        != bundles.len()
    {
        return Err(ResourceReleaseError::InvalidBundleDependency);
    }
    Ok(())
}

fn usize_to_u64(value: usize) -> Result<u64, ResourceReleaseError> {
    u64::try_from(value).map_err(|_| ResourceReleaseError::InvalidRange)
}

fn resource_value_array(
    resource: &ResourceDescriptorV1,
) -> Result<&[CborValue], ResourceReleaseError> {
    exact_array(resource.value(), RESOURCE_DESCRIPTOR_FIELD_COUNT)
}

fn resource_value_without_dependencies(
    resource: &ResourceDescriptorV1,
) -> Result<CborValue, ResourceReleaseError> {
    let fields = resource_value_array(resource)?;
    Ok(CborValue::Array(
        fields[..12]
            .iter()
            .chain(fields[13..].iter())
            .cloned()
            .collect(),
    ))
}

fn contains_commitment(value: &CborValue, commitment: CommitmentV1) -> bool {
    match value {
        CborValue::Bytes(bytes) => bytes.as_slice() == commitment.as_bytes(),
        CborValue::Array(values) => values
            .iter()
            .any(|value| contains_commitment(value, commitment)),
        _ => false,
    }
}

fn reject_commitments(
    value: &CborValue,
    commitments: &[CommitmentV1],
) -> Result<(), ResourceReleaseError> {
    if commitments
        .iter()
        .any(|commitment| contains_commitment(value, *commitment))
    {
        return Err(ResourceReleaseError::IdentityBackreference);
    }
    Ok(())
}

pub fn encode_manifest_identity(value: &CborValue) -> Result<Vec<u8>, ResourceReleaseError> {
    let mut output = Vec::new();
    encode_cbor_value(value, 0, &mut output)?;
    Ok(output)
}

fn encode_cbor_value(
    value: &CborValue,
    depth: usize,
    output: &mut Vec<u8>,
) -> Result<(), ResourceReleaseError> {
    if depth > 128 {
        return Err(ResourceReleaseError::InvalidCanonicalCbor);
    }
    match value {
        CborValue::Unsigned(value) => append_cbor_head(0, *value, output),
        CborValue::Bool(value) => output.push(if *value { 0xf5 } else { 0xf4 }),
        CborValue::Bytes(value) => {
            if value.len() != 32 {
                return Err(ResourceReleaseError::InvalidCanonicalCbor);
            }
            append_cbor_head(2, 32, output);
            output.extend_from_slice(value);
        }
        CborValue::Text(value) => {
            if !value.is_ascii() {
                return Err(ResourceReleaseError::InvalidCanonicalCbor);
            }
            append_cbor_head(3, usize_to_u64(value.len())?, output);
            output.extend_from_slice(value.as_bytes());
        }
        CborValue::Array(values) => {
            append_cbor_head(4, usize_to_u64(values.len())?, output);
            for value in values {
                encode_cbor_value(value, depth + 1, output)?;
            }
        }
    }
    Ok(())
}

fn append_cbor_head(major: u8, value: u64, output: &mut Vec<u8>) {
    if value < 24 {
        output.push((major << 5) | value as u8);
    } else if value <= u8::MAX.into() {
        output.extend_from_slice(&[(major << 5) | 24, value as u8]);
    } else if value <= u16::MAX.into() {
        output.push((major << 5) | 25);
        output.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= u32::MAX.into() {
        output.push((major << 5) | 26);
        output.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        output.push((major << 5) | 27);
        output.extend_from_slice(&value.to_be_bytes());
    }
}

pub fn validate_manifest_identity_cbor(bytes: &[u8]) -> Result<(), ResourceReleaseError> {
    let mut cursor = 0_usize;
    validate_cbor_item(bytes, &mut cursor, 0)?;
    if cursor != bytes.len() {
        return Err(ResourceReleaseError::InvalidCanonicalCbor);
    }
    Ok(())
}

fn validate_cbor_item(
    bytes: &[u8],
    cursor: &mut usize,
    depth: usize,
) -> Result<(), ResourceReleaseError> {
    if depth > 128 {
        return Err(ResourceReleaseError::InvalidCanonicalCbor);
    }
    let initial = take_cbor_byte(bytes, cursor)?;
    let major = initial >> 5;
    let additional = initial & 0x1f;
    match major {
        0 => {
            read_cbor_argument(bytes, cursor, additional)?;
        }
        2 | 3 => {
            let length = read_cbor_argument(bytes, cursor, additional)?;
            let length =
                usize::try_from(length).map_err(|_| ResourceReleaseError::InvalidCanonicalCbor)?;
            if major == 2 && length != 32 {
                return Err(ResourceReleaseError::InvalidCanonicalCbor);
            }
            let end = cursor
                .checked_add(length)
                .ok_or(ResourceReleaseError::InvalidCanonicalCbor)?;
            let value = bytes
                .get(*cursor..end)
                .ok_or(ResourceReleaseError::InvalidCanonicalCbor)?;
            if major == 3 && !value.is_ascii() {
                return Err(ResourceReleaseError::InvalidCanonicalCbor);
            }
            *cursor = end;
        }
        4 => {
            let length = read_cbor_argument(bytes, cursor, additional)?;
            for _ in 0..length {
                validate_cbor_item(bytes, cursor, depth + 1)?;
            }
        }
        7 if initial == 0xf4 || initial == 0xf5 => {}
        _ => return Err(ResourceReleaseError::InvalidCanonicalCbor),
    }
    Ok(())
}

fn read_cbor_argument(
    bytes: &[u8],
    cursor: &mut usize,
    additional: u8,
) -> Result<u64, ResourceReleaseError> {
    if additional < 24 {
        return Ok(u64::from(additional));
    }
    let width = match additional {
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => return Err(ResourceReleaseError::InvalidCanonicalCbor),
    };
    let end = cursor
        .checked_add(width)
        .ok_or(ResourceReleaseError::InvalidCanonicalCbor)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ResourceReleaseError::InvalidCanonicalCbor)?;
    *cursor = end;
    let value = raw
        .iter()
        .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
    let minimum = match width {
        1 => 24,
        2 => 0x100,
        4 => 0x1_0000,
        8 => 0x1_0000_0000,
        _ => unreachable!("invariant: CBOR argument width is closed"),
    };
    if value < minimum {
        return Err(ResourceReleaseError::InvalidCanonicalCbor);
    }
    Ok(value)
}

fn take_cbor_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, ResourceReleaseError> {
    let byte = bytes
        .get(*cursor)
        .copied()
        .ok_or(ResourceReleaseError::InvalidCanonicalCbor)?;
    *cursor += 1;
    Ok(byte)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceReleaseError {
    #[error("commitment must be canonical lowercase bytes32")]
    InvalidCommitment,
    #[error("value must be non-empty bounded ASCII")]
    InvalidAscii,
    #[error("unsigned coordinate is outside its frozen range")]
    InvalidRange,
    #[error("value has the wrong exact field or envelope shape")]
    InvalidShape,
    #[error("closed enum tag is outside the frozen C868 set")]
    InvalidClosedTag,
    #[error("tags must be positive, strictly increasing, and unique")]
    InvalidStrictTags,
    #[error("typed optional must be exactly [0] or [1,value]")]
    InvalidOptional,
    #[error("OwnerRefV1 must bind owner_tag 1..20 and bytes32 profile")]
    InvalidOwner,
    #[error("ManifestIdentityV1 envelope domain or SchemaId differs from C868")]
    InvalidIdentityEnvelope,
    #[error("ManifestIdentityV1 CBOR is outside the strict deterministic subset")]
    InvalidCanonicalCbor,
    #[error("ResourceDescriptorV1 differs from the exact 24-field contract")]
    InvalidResourceDescriptor,
    #[error("Resource dependency is not an exact strictly-backward tag/ResourceId pair")]
    InvalidResourceDependency,
    #[error("Release Resource tags or identities are not globally strict and unique")]
    InvalidResourceClosure,
    #[error("Bundle kind is Release or outside the seven non-Release kinds")]
    InvalidBundleKind,
    #[error("BundleManifestV1 differs from its exact five-slot identity")]
    InvalidBundleManifest,
    #[error("Bundle membership is empty, duplicated, wrong-kind, or outside its bound")]
    InvalidBundleMembership,
    #[error("Resource required_bundle_kind differs from its one owning Bundle")]
    ResourceInWrongBundle,
    #[error("Bundle dependency is not a <=64 strict-backward subset")]
    InvalidBundleDependency,
    #[error("Release Bundles do not form the ordered exact seven-kind topology")]
    InvalidBundleTopology,
    #[error("supported target classes are not 1..6 strict closed tags")]
    InvalidTargetClassClosure,
    #[error("ManifestHeaderCoreV1 coordinates or pinned commitments differ")]
    InvalidManifestCore,
    #[error("census Resource locator keys do not equal the ResourceId closure")]
    InvalidResourceLocatorClosure,
    #[error("direct consumer tag/ResourceId pair targets a different Resource")]
    InvalidConsumerEdge,
    #[error("census descriptor row or exact optional partition is invalid")]
    InvalidCensusDescriptor,
    #[error("census Resource coordinates differ from ResourceId -> owning BundleId")]
    InvalidCensusCoordinates,
    #[error("census rows/counts/identities do not equal the concrete closure")]
    InvalidCensusClosure,
    #[error("Remove requires zero direct consumers")]
    RemoveHasDirectConsumers,
    #[error("embedded Release is not the sole exact five-slot Release-kind root")]
    InvalidReleaseClosure,
    #[error("identity contains an undeclared future, upward, self, or installed-state ID")]
    IdentityBackreference,
    #[error("identity delta entry transition is invalid")]
    InvalidIdentityDelta,
    #[error("identity delta does not close exactly Schema through Release")]
    IncompleteIdentityDelta,
    #[error("downstream obligations must be exactly candidate-root/finalization/handoff")]
    InvalidDownstreamIdentityObligations,
}
