#![allow(
    dead_code,
    reason = "Stage 11 leaf capability awaits MainIntegration owner wiring"
)]

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::ffi::OsStrExt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::secure_fs::{
    DescriptorCensusLimitsV1, DescriptorCensusObjectKindV1, InventoryRowV1,
    RetainedDescriptorCensusLeaseV3, SecureFsError, SecureRoot,
};

const ADMITTED_SET_DOMAIN_V3: &[u8] = b"maestro.foundation.legacy-quarantine.admitted-set.v3\0";
const EXPECTED_SOURCE_DOMAIN_V3: &[u8] =
    b"maestro.foundation.legacy-quarantine.expected-source.v3\0";
const EXPECTED_SOURCE_SET_DOMAIN_V3: &[u8] =
    b"maestro.foundation.legacy-quarantine.expected-source-set.v3\0";
const SOURCE_TOKEN_DOMAIN_V1: &[u8] = b"maestro.foundation.legacy-quarantine.source-token.v1\0";
const CLOSURE_DOMAIN_V1: &[u8] = b"maestro.foundation.legacy-quarantine.closure.v1\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum LegacyQuarantineOwnerDomainV3 {
    Repository,
    Installation,
    ProtectedPrimary,
}

impl LegacyQuarantineOwnerDomainV3 {
    const fn tag(self) -> u8 {
        match self {
            Self::Repository => 1,
            Self::Installation => 2,
            Self::ProtectedPrimary => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct LegacyQuarantineExpectedSourceV3 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    root_binding: [u8; 32],
    relative_locator: Vec<u8>,
    kind: DescriptorCensusObjectKindV1,
    logical_byte_length: u64,
    object_identity: [u8; 32],
    content_identity: [u8; 32],
    source_provenance_id: [u8; 32],
    loss_evidence_id: Option<[u8; 32]>,
}

impl LegacyQuarantineExpectedSourceV3 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the expected source is the packet-bound lossless physical row"
    )]
    pub(crate) fn from_packet(
        owner: LegacyQuarantineOwnerDomainV3,
        root_binding: [u8; 32],
        relative_locator: Vec<u8>,
        kind: DescriptorCensusObjectKindV1,
        logical_byte_length: u64,
        object_identity: [u8; 32],
        content_identity: [u8; 32],
        source_provenance_id: [u8; 32],
        loss_evidence_id: Option<[u8; 32]>,
    ) -> Result<Self, FoundationLegacyQuarantineErrorV1> {
        if root_binding == [0; 32]
            || !valid_relative_locator(&relative_locator)
            || object_identity == [0; 32]
            || content_identity == [0; 32]
            || source_provenance_id == [0; 32]
            || loss_evidence_id == Some([0; 32])
        {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidExpectedSource);
        }
        let kind_tag = [match kind {
            DescriptorCensusObjectKindV1::RegularFile => 1,
            DescriptorCensusObjectKindV1::SymbolicLink => 2,
        }];
        let loss_marker = [u8::from(loss_evidence_id.is_some())];
        let loss_identity = loss_evidence_id.unwrap_or([0; 32]);
        let identity = commitment(
            EXPECTED_SOURCE_DOMAIN_V3,
            &[
                &[owner.tag()],
                &root_binding,
                &relative_locator,
                &kind_tag,
                &logical_byte_length.to_be_bytes(),
                &object_identity,
                &content_identity,
                &source_provenance_id,
                &loss_marker,
                &loss_identity,
            ],
        );
        Ok(Self {
            identity,
            owner,
            root_binding,
            relative_locator,
            kind,
            logical_byte_length,
            object_identity,
            content_identity,
            source_provenance_id,
            loss_evidence_id,
        })
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LegacyQuarantineExpectedSourceSetV3 {
    identity: [u8; 32],
    packet_identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    rows: Vec<LegacyQuarantineExpectedSourceV3>,
}

impl LegacyQuarantineExpectedSourceSetV3 {
    pub(crate) fn from_packet(
        packet_identity: [u8; 32],
        owner: LegacyQuarantineOwnerDomainV3,
        mut rows: Vec<LegacyQuarantineExpectedSourceV3>,
    ) -> Result<Self, FoundationLegacyQuarantineErrorV1> {
        rows.sort_by_key(LegacyQuarantineExpectedSourceV3::identity);
        if packet_identity == [0; 32]
            || rows.is_empty()
            || rows.iter().any(|row| row.owner != owner)
            || rows.windows(2).any(|pair| {
                pair[0].identity == pair[1].identity
                    || (pair[0].root_binding, &pair[0].relative_locator)
                        == (pair[1].root_binding, &pair[1].relative_locator)
            })
        {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidExpectedSource);
        }
        let owner_tag = [owner.tag()];
        let mut parts = vec![packet_identity.as_slice(), owner_tag.as_slice()];
        parts.extend(rows.iter().map(|row| row.identity.as_slice()));
        let identity = commitment(EXPECTED_SOURCE_SET_DOMAIN_V3, &parts);
        Ok(Self {
            identity,
            packet_identity,
            owner,
            rows,
        })
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn packet_identity(&self) -> [u8; 32] {
        self.packet_identity
    }

    pub(crate) fn binds_owner_roots(
        &self,
        owner: LegacyQuarantineOwnerDomainV3,
        root_bindings: &[[u8; 32]],
    ) -> bool {
        self.owner == owner
            && !root_bindings.is_empty()
            && self
                .rows
                .iter()
                .all(|row| root_bindings.contains(&row.root_binding))
            && root_bindings
                .iter()
                .all(|binding| self.rows.iter().any(|row| row.root_binding == *binding))
    }
}

pub(crate) mod owner_admission_sealed {
    pub trait Sealed {}
}

pub(crate) trait LegacyQuarantineRootAdmissionV3: owner_admission_sealed::Sealed {
    fn into_foundation_facts(self) -> LegacyQuarantineRootAdmissionFactsV3;
}

pub(crate) fn observe_root_binding_v3(
    path: impl AsRef<std::path::Path>,
) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
    let root = SecureRoot::open(path)?;
    let facts = root.descriptor_census_admission_facts_v2()?;
    Ok(commitment(
        b"maestro.foundation.legacy-quarantine.root-binding.v3\0",
        &[&facts.resolved_identity, &facts.mount_identity],
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LegacyQuarantinePhysicalFactsV1 {
    display_locator: Vec<u8>,
    resolved_locator_commitment: [u8; 32],
    object_identity: [u8; 32],
    mount_identity: [u8; 32],
    provider_identity: [u8; 32],
    anchor_identity: [u8; 32],
    fence_identity: [u8; 32],
}

impl LegacyQuarantinePhysicalFactsV1 {
    pub(crate) fn display_locator(&self) -> &[u8] {
        &self.display_locator
    }

    pub(crate) const fn resolved_locator_commitment(&self) -> [u8; 32] {
        self.resolved_locator_commitment
    }

    pub(crate) const fn object_identity(&self) -> [u8; 32] {
        self.object_identity
    }

    pub(crate) const fn mount_identity(&self) -> [u8; 32] {
        self.mount_identity
    }

    pub(crate) const fn provider_identity(&self) -> [u8; 32] {
        self.provider_identity
    }

    pub(crate) const fn anchor_identity(&self) -> [u8; 32] {
        self.anchor_identity
    }

    pub(crate) const fn fence_identity(&self) -> [u8; 32] {
        self.fence_identity
    }
}

pub(crate) fn observe_physical_facts_v1(
    path: impl AsRef<std::path::Path>,
) -> Result<LegacyQuarantinePhysicalFactsV1, FoundationLegacyQuarantineErrorV1> {
    let root = SecureRoot::open(path)?;
    let facts = root.descriptor_census_admission_facts_v2()?;
    let display_locator = facts
        .locator_components
        .iter()
        .enumerate()
        .flat_map(|(index, component)| {
            let mut bytes = Vec::with_capacity(component.len() + 1);
            if index > 0 && component.as_slice() != b"/" {
                bytes.push(b'/');
            }
            bytes.extend_from_slice(component);
            bytes
        })
        .collect::<Vec<_>>();
    let resolved_locator_commitment = commitment(
        b"maestro.foundation.legacy-quarantine.resolved-locator.v1\0",
        &[&display_locator, &facts.resolved_identity],
    );
    Ok(LegacyQuarantinePhysicalFactsV1 {
        display_locator,
        resolved_locator_commitment,
        object_identity: facts.resolved_identity,
        mount_identity: facts.mount_identity,
        provider_identity: facts.provider_identity,
        anchor_identity: facts.anchor_identity,
        fence_identity: facts.fence_identity,
    })
}

pub(crate) struct LegacyQuarantineRootAdmissionFactsV3 {
    owner: LegacyQuarantineOwnerDomainV3,
    roots: Vec<LegacyQuarantineAdmittedRootV3>,
    expected_sources: LegacyQuarantineExpectedSourceSetV3,
    owner_currentness: [u8; 32],
    owner_attestation: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(crate) struct LegacyQuarantineAdmittedRootV3 {
    display_locator: Vec<u8>,
    path: PathBuf,
    root_binding: [u8; 32],
}

impl LegacyQuarantineRootAdmissionFactsV3 {
    pub(crate) fn from_owner(
        owner: LegacyQuarantineOwnerDomainV3,
        roots: Vec<(Vec<u8>, PathBuf, [u8; 32])>,
        expected_sources: LegacyQuarantineExpectedSourceSetV3,
        owner_currentness: [u8; 32],
        owner_attestation: [u8; 32],
    ) -> Result<Self, FoundationLegacyQuarantineErrorV1> {
        if roots.is_empty()
            || owner_currentness == [0; 32]
            || owner_attestation == [0; 32]
            || expected_sources.owner != owner
            || roots.iter().any(|(display, path, binding)| {
                display.is_empty()
                    || display.contains(&0)
                    || !path.is_absolute()
                    || *binding == [0; 32]
            })
        {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidOwnerAdmission);
        }
        let mut prior = None;
        let roots = roots
            .into_iter()
            .map(|(display_locator, path, root_binding)| {
                let key = (display_locator.clone(), root_binding);
                if prior.as_ref().is_some_and(|previous| previous >= &key) {
                    return Err(FoundationLegacyQuarantineErrorV1::InvalidOwnerAdmission);
                }
                prior = Some(key);
                Ok(LegacyQuarantineAdmittedRootV3 {
                    display_locator,
                    path,
                    root_binding,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if expected_sources.rows.iter().any(|expected| {
            !roots
                .iter()
                .any(|root| root.root_binding == expected.root_binding)
        }) {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidOwnerAdmission);
        }
        Ok(Self {
            owner,
            roots,
            expected_sources,
            owner_currentness,
            owner_attestation,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(crate) mod persistence_lease_sealed {
    pub trait Sealed {}
}

pub(crate) trait ProtectedPrimaryBoundaryPortV1: persistence_lease_sealed::Sealed {
    fn identity(&self) -> [u8; 32];
    fn display_locator(&self) -> &[u8];
    fn resolved_locator_commitment(&self) -> [u8; 32];
    fn object_identity(&self) -> [u8; 32];
    fn mount_identity(&self) -> [u8; 32];
    fn provider_identity(&self) -> [u8; 32];
    fn anchor_identity(&self) -> [u8; 32];
    fn realm_identity(&self) -> [u8; 32];
    fn currentness(&self) -> [u8; 32];
    fn fence(&self) -> [u8; 32];
    fn revocation_revision(&self) -> u64;
    fn expected_sources(&self) -> &LegacyQuarantineExpectedSourceSetV3;
    fn retain_source_census(
        &mut self,
        limits: DescriptorCensusLimitsV1,
    ) -> Result<(), FoundationLegacyQuarantineErrorV1>;
    fn source_rows(&self) -> Result<&[InventoryRowV1], FoundationLegacyQuarantineErrorV1>;
    fn read_source(
        &self,
        relative_locator: &[u8],
        kind: DescriptorCensusObjectKindV1,
    ) -> Result<Vec<u8>, FoundationLegacyQuarantineErrorV1>;
    fn final_recheck(self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1>;
}

pub(crate) trait QuarantineCustodyPortV1: persistence_lease_sealed::Sealed {
    fn identity(&self) -> [u8; 32];
    fn display_locator(&self) -> &[u8];
    fn resolved_locator_commitment(&self) -> [u8; 32];
    fn object_identity(&self) -> [u8; 32];
    fn mount_identity(&self) -> [u8; 32];
    fn provider_identity(&self) -> [u8; 32];
    fn anchor_identity(&self) -> [u8; 32];
    fn manager_realm_identity(&self) -> [u8; 32];
    fn security_realm_identity(&self) -> [u8; 32];
    fn expected_old(&self) -> [u8; 32];
    fn currentness(&self) -> [u8; 32];
    fn fence(&self) -> [u8; 32];
    fn revocation_revision(&self) -> u64;
    fn persist_source(
        &mut self,
        source_token: [u8; 32],
        object_identity: [u8; 32],
        kind: DescriptorCensusObjectKindV1,
        bytes: &[u8],
    ) -> Result<FoundationCustodyCopyReceiptV1, FoundationLegacyQuarantineErrorV1>;
    fn rollback_partial(self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1>;
    fn seal_expected_old(
        self,
        candidate_manifest: [u8; 32],
        custody_records: &[[u8; 32]],
    ) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundationCustodyCopyReceiptV1 {
    identity: [u8; 32],
    copied_length: u64,
    copied_sha256: [u8; 32],
}

impl FoundationCustodyCopyReceiptV1 {
    pub(crate) fn from_persistence(
        identity: [u8; 32],
        copied_length: u64,
        copied_sha256: [u8; 32],
    ) -> Result<Self, FoundationLegacyQuarantineErrorV1> {
        if identity == [0; 32] || copied_sha256 == [0; 32] {
            return Err(FoundationLegacyQuarantineErrorV1::CustodyWriteFailed);
        }
        Ok(Self {
            identity,
            copied_length,
            copied_sha256,
        })
    }

    pub(crate) const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn copied_length(self) -> u64 {
        self.copied_length
    }

    pub(crate) const fn copied_sha256(self) -> [u8; 32] {
        self.copied_sha256
    }
}

struct RetainedRootV1 {
    owner: LegacyQuarantineOwnerDomainV3,
    display_locator: Vec<u8>,
    root_binding: [u8; 32],
    mount_identity: [u8; 32],
    provider_identity: [u8; 32],
    owner_currentness: [u8; 32],
    owner_attestation: [u8; 32],
    expected_source_set_identity: [u8; 32],
    expected_sources: Vec<LegacyQuarantineExpectedSourceV3>,
    lease: RetainedDescriptorCensusLeaseV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FoundationLegacyPayloadStateV3 {
    Present,
    UnavailablePreexistingLoss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FoundationSourceCaseV1 {
    source_token: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    display_locator: Vec<u8>,
    root_binding: [u8; 32],
    relative_locator: Vec<u8>,
    payload_state: FoundationLegacyPayloadStateV3,
    kind: DescriptorCensusObjectKindV1,
    logical_byte_length: u64,
    object_identity: [u8; 32],
    content_identity: [u8; 32],
    source_provenance_id: [u8; 32],
    loss_evidence_id: Option<[u8; 32]>,
    mount_identity: [u8; 32],
    provider_identity: [u8; 32],
    owner_currentness: [u8; 32],
    owner_attestation: [u8; 32],
}

impl FoundationSourceCaseV1 {
    pub(crate) const fn source_token(&self) -> [u8; 32] {
        self.source_token
    }

    pub(crate) const fn owner(&self) -> LegacyQuarantineOwnerDomainV3 {
        self.owner
    }

    pub(crate) fn display_locator(&self) -> &[u8] {
        &self.display_locator
    }

    pub(crate) const fn root_binding(&self) -> [u8; 32] {
        self.root_binding
    }

    pub(crate) fn relative_locator(&self) -> &[u8] {
        &self.relative_locator
    }

    pub(crate) const fn payload_state(&self) -> FoundationLegacyPayloadStateV3 {
        self.payload_state
    }

    pub(crate) const fn kind(&self) -> DescriptorCensusObjectKindV1 {
        self.kind
    }

    pub(crate) const fn logical_byte_length(&self) -> u64 {
        self.logical_byte_length
    }

    pub(crate) const fn object_identity(&self) -> [u8; 32] {
        self.object_identity
    }

    pub(crate) const fn content_identity(&self) -> [u8; 32] {
        self.content_identity
    }

    pub(crate) const fn source_provenance_id(&self) -> [u8; 32] {
        self.source_provenance_id
    }

    pub(crate) const fn loss_evidence_id(&self) -> Option<[u8; 32]> {
        self.loss_evidence_id
    }

    pub(crate) const fn mount_identity(&self) -> [u8; 32] {
        self.mount_identity
    }

    pub(crate) const fn provider_identity(&self) -> [u8; 32] {
        self.provider_identity
    }

    pub(crate) const fn owner_currentness(&self) -> [u8; 32] {
        self.owner_currentness
    }

    pub(crate) const fn owner_attestation(&self) -> [u8; 32] {
        self.owner_attestation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FoundationProtectedPrimaryOverlapPairV1 {
    owner_source_token: [u8; 32],
    primary_source_token: [u8; 32],
}

impl FoundationProtectedPrimaryOverlapPairV1 {
    pub(crate) const fn owner_source_token(&self) -> [u8; 32] {
        self.owner_source_token
    }

    pub(crate) const fn primary_source_token(&self) -> [u8; 32] {
        self.primary_source_token
    }
}

pub(crate) struct FoundationLegacyQuarantineLeaseV1<P, Q> {
    invocation: [u8; 32],
    admitted_set: [u8; 32],
    roots: Vec<RetainedRootV1>,
    source_cases: Vec<FoundationSourceCaseV1>,
    overlap_pairs: Vec<FoundationProtectedPrimaryOverlapPairV1>,
    protected_primary: P,
    custody: Q,
    limits: DescriptorCensusLimitsV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<P, Q> FoundationLegacyQuarantineLeaseV1<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    pub(crate) fn acquire<R, I>(
        repository: R,
        installation: I,
        mut protected_primary: P,
        custody: Q,
        invocation: [u8; 32],
        limits: DescriptorCensusLimitsV1,
    ) -> Result<Self, FoundationLegacyQuarantineErrorV1>
    where
        R: LegacyQuarantineRootAdmissionV3,
        I: LegacyQuarantineRootAdmissionV3,
    {
        if invocation == [0; 32] {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidInvocation);
        }
        let repository = repository.into_foundation_facts();
        let installation = installation.into_foundation_facts();
        if repository.owner != LegacyQuarantineOwnerDomainV3::Repository
            || installation.owner != LegacyQuarantineOwnerDomainV3::Installation
            || repository.expected_sources.packet_identity()
                != installation.expected_sources.packet_identity()
            || repository.expected_sources.packet_identity()
                != protected_primary.expected_sources().packet_identity()
            || protected_primary.expected_sources().owner
                != LegacyQuarantineOwnerDomainV3::ProtectedPrimary
        {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidOwnerAdmission);
        }
        validate_physical_separation(&repository, &installation, &protected_primary, &custody)?;
        protected_primary.retain_source_census(limits)?;
        let mut roots = Vec::new();
        for admission in [repository, installation] {
            let LegacyQuarantineRootAdmissionFactsV3 {
                owner,
                roots: admitted_roots,
                expected_sources,
                owner_currentness,
                owner_attestation,
                ..
            } = admission;
            let expected_source_set_identity = expected_sources.identity();
            for admitted in admitted_roots {
                let root = SecureRoot::open(&admitted.path)?;
                let physical = observe_physical_facts_v1(&admitted.path)?;
                let observed = root.descriptor_census_admission_facts_v2()?;
                if commitment(
                    b"maestro.foundation.legacy-quarantine.root-binding.v3\0",
                    &[&observed.resolved_identity, &observed.mount_identity],
                ) != admitted.root_binding
                {
                    return Err(FoundationLegacyQuarantineErrorV1::RootBindingDrift);
                }
                let lease = root.retain_descriptor_census_root_v3(limits)?;
                let expected_sources = expected_sources
                    .rows
                    .iter()
                    .filter(|expected| expected.root_binding == admitted.root_binding)
                    .cloned()
                    .collect::<Vec<_>>();
                if expected_sources.is_empty() {
                    return Err(FoundationLegacyQuarantineErrorV1::InvalidOwnerAdmission);
                }
                roots.push(RetainedRootV1 {
                    owner,
                    display_locator: admitted.display_locator,
                    root_binding: admitted.root_binding,
                    mount_identity: physical.mount_identity(),
                    provider_identity: physical.provider_identity(),
                    owner_currentness,
                    owner_attestation,
                    expected_source_set_identity,
                    expected_sources,
                    lease,
                });
            }
        }
        roots.sort_by(|left, right| {
            (left.owner, &left.display_locator, left.root_binding).cmp(&(
                right.owner,
                &right.display_locator,
                right.root_binding,
            ))
        });
        for (index, root) in roots.iter().enumerate() {
            if roots.iter().skip(index + 1).any(|other| {
                root.root_binding == other.root_binding
                    || locator_contains(&root.display_locator, &other.display_locator)
                    || locator_contains(&other.display_locator, &root.display_locator)
            }) {
                return Err(FoundationLegacyQuarantineErrorV1::RootAlias);
            }
        }
        let admitted_set = admitted_set_identity(
            invocation,
            &roots,
            protected_primary.expected_sources().identity(),
        );
        let mut source_cases = Vec::new();
        for root in &roots {
            source_cases.extend(reconcile_expected_sources(
                invocation,
                admitted_set,
                root.owner,
                &root.display_locator,
                root.root_binding,
                root.mount_identity,
                root.provider_identity,
                root.owner_currentness,
                root.owner_attestation,
                &root.expected_sources,
                root.lease.census().rows(),
            )?);
        }
        let primary_root_binding = protected_primary.resolved_locator_commitment();
        let primary_currentness = protected_primary.currentness();
        let primary_attestation = protected_primary.identity();
        source_cases.extend(reconcile_expected_sources(
            invocation,
            admitted_set,
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
            protected_primary.display_locator(),
            primary_root_binding,
            protected_primary.mount_identity(),
            protected_primary.provider_identity(),
            primary_currentness,
            primary_attestation,
            &protected_primary.expected_sources().rows,
            protected_primary.source_rows()?,
        )?);
        source_cases.sort_by_key(|source| source.source_token);
        if source_cases
            .iter()
            .map(FoundationSourceCaseV1::source_token)
            .collect::<BTreeSet<_>>()
            .len()
            != source_cases.len()
        {
            return Err(FoundationLegacyQuarantineErrorV1::DuplicateSource);
        }
        let overlap_pairs = derive_overlap_pairs(&source_cases)?;
        Ok(Self {
            invocation,
            admitted_set,
            roots,
            source_cases,
            overlap_pairs,
            protected_primary,
            custody,
            limits,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn admitted_set(&self) -> [u8; 32] {
        self.admitted_set
    }

    pub(crate) const fn invocation(&self) -> [u8; 32] {
        self.invocation
    }

    pub(crate) fn source_cases(&self) -> &[FoundationSourceCaseV1] {
        &self.source_cases
    }

    pub(crate) fn overlap_pairs(&self) -> &[FoundationProtectedPrimaryOverlapPairV1] {
        &self.overlap_pairs
    }

    pub(crate) fn into_copy_continuation(self) -> FoundationSourceCopyContinuationV1<P, Q> {
        FoundationSourceCopyContinuationV1 {
            lease: self,
            copied: BTreeSet::new(),
            custody_records: Vec::new(),
        }
    }
}

pub(crate) struct FoundationSourceCopyContinuationV1<P, Q> {
    lease: FoundationLegacyQuarantineLeaseV1<P, Q>,
    copied: BTreeSet<[u8; 32]>,
    custody_records: Vec<[u8; 32]>,
}

impl<P, Q> FoundationSourceCopyContinuationV1<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    pub(crate) fn copy_once(
        &mut self,
        source_token: [u8; 32],
    ) -> Result<FoundationCustodyCopyReceiptV1, FoundationLegacyQuarantineErrorV1> {
        if !self.copied.insert(source_token) {
            return Err(FoundationLegacyQuarantineErrorV1::SourceTokenReused);
        }
        let source = self
            .lease
            .source_cases
            .iter()
            .find(|source| source.source_token == source_token)
            .ok_or(FoundationLegacyQuarantineErrorV1::UnknownSourceToken)?;
        if source.payload_state != FoundationLegacyPayloadStateV3::Present {
            return Err(FoundationLegacyQuarantineErrorV1::UnknownSourceToken);
        }
        let bytes = if source.owner == LegacyQuarantineOwnerDomainV3::ProtectedPrimary {
            self.lease
                .protected_primary
                .read_source(&source.relative_locator, source.kind)?
        } else {
            let root = self
                .lease
                .roots
                .iter()
                .find(|root| {
                    root.owner == source.owner
                        && root.root_binding == source.root_binding
                        && root.display_locator == source.display_locator
                })
                .ok_or(FoundationLegacyQuarantineErrorV1::UnknownSourceToken)?;
            root.lease.read_immutable(
                std::path::Path::new(std::ffi::OsStr::from_bytes(&source.relative_locator)),
                source.kind,
            )?
        };
        if bytes.len() as u64 != source.logical_byte_length
            || <[u8; 32]>::from(Sha256::digest(&bytes)) != source.content_identity
        {
            return Err(FoundationLegacyQuarantineErrorV1::SourceChanged);
        }
        let receipt = self.lease.custody.persist_source(
            source.source_token,
            source.object_identity,
            source.kind,
            &bytes,
        )?;
        if receipt.copied_length() != source.logical_byte_length
            || receipt.copied_sha256() != source.content_identity
            || self.custody_records.contains(&receipt.identity())
        {
            return Err(FoundationLegacyQuarantineErrorV1::CustodyWriteFailed);
        }
        self.custody_records.push(receipt.identity());
        Ok(receipt)
    }

    pub(crate) fn rollback(self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
        self.lease.custody.rollback_partial()
    }

    pub(crate) fn finish(
        self,
        candidate_manifest: [u8; 32],
    ) -> Result<(FoundationLegacyQuarantineClosureV1, [u8; 32]), FoundationLegacyQuarantineErrorV1>
    {
        let present_count = self
            .lease
            .source_cases
            .iter()
            .filter(|source| source.payload_state == FoundationLegacyPayloadStateV3::Present)
            .count();
        if candidate_manifest == [0; 32] || self.copied.len() != present_count {
            return Err(FoundationLegacyQuarantineErrorV1::PartialCopy);
        }
        let FoundationSourceCopyContinuationV1 {
            lease,
            custody_records,
            ..
        } = self;
        let FoundationLegacyQuarantineLeaseV1 {
            invocation,
            admitted_set,
            roots,
            protected_primary,
            custody,
            limits,
            ..
        } = lease;
        for root in roots {
            root.lease.consume_final_recheck(limits)?;
        }
        let primary_currentness = protected_primary.final_recheck()?;
        let persistence_receipt =
            custody.seal_expected_old(candidate_manifest, &custody_records)?;
        let closure_id = commitment(
            CLOSURE_DOMAIN_V1,
            &[
                &invocation,
                &admitted_set,
                &candidate_manifest,
                &primary_currentness,
                &persistence_receipt,
            ],
        );
        Ok((
            FoundationLegacyQuarantineClosureV1 {
                identity: closure_id,
                admitted_set,
                candidate_manifest,
                protected_primary_currentness: primary_currentness,
                persistence_receipt,
            },
            persistence_receipt,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundationLegacyQuarantineClosureV1 {
    identity: [u8; 32],
    admitted_set: [u8; 32],
    candidate_manifest: [u8; 32],
    protected_primary_currentness: [u8; 32],
    persistence_receipt: [u8; 32],
}

impl FoundationLegacyQuarantineClosureV1 {
    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn admitted_set(&self) -> [u8; 32] {
        self.admitted_set
    }

    pub(crate) const fn persistence_receipt(&self) -> [u8; 32] {
        self.persistence_receipt
    }
}

fn validate_physical_separation<P, Q>(
    repository: &LegacyQuarantineRootAdmissionFactsV3,
    installation: &LegacyQuarantineRootAdmissionFactsV3,
    primary: &P,
    custody: &Q,
) -> Result<(), FoundationLegacyQuarantineErrorV1>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    let mut root_bindings = BTreeSet::new();
    for root in repository.roots.iter().chain(&installation.roots) {
        if !root_bindings.insert(root.root_binding)
            || locator_contains(&root.display_locator, custody.display_locator())
            || locator_contains(custody.display_locator(), &root.display_locator)
            || locator_contains(&root.display_locator, primary.display_locator())
            || locator_contains(primary.display_locator(), &root.display_locator)
        {
            return Err(FoundationLegacyQuarantineErrorV1::PhysicalSeparationFailed);
        }
    }
    if primary.identity() == custody.identity()
        || primary.resolved_locator_commitment() == custody.resolved_locator_commitment()
        || primary.object_identity() == custody.object_identity()
        || primary.mount_identity() == custody.mount_identity()
        || primary.provider_identity() == custody.provider_identity()
        || primary.anchor_identity() == custody.anchor_identity()
        || primary.realm_identity() == custody.manager_realm_identity()
        || primary.realm_identity() == custody.security_realm_identity()
        || primary.currentness() == [0; 32]
        || primary.fence() == [0; 32]
        || custody.expected_old() == [0; 32]
        || custody.currentness() == [0; 32]
        || custody.fence() == [0; 32]
        || primary.revocation_revision() == 0
        || custody.revocation_revision() == 0
    {
        return Err(FoundationLegacyQuarantineErrorV1::PhysicalSeparationFailed);
    }
    Ok(())
}

fn admitted_set_identity(
    invocation: [u8; 32],
    roots: &[RetainedRootV1],
    protected_primary_expected_set: [u8; 32],
) -> [u8; 32] {
    let mut parts = Vec::new();
    parts.push(invocation.as_slice());
    for root in roots {
        parts.push(root.display_locator.as_slice());
        parts.push(root.root_binding.as_slice());
        parts.push(root.owner_currentness.as_slice());
        parts.push(root.owner_attestation.as_slice());
        parts.push(root.expected_source_set_identity.as_slice());
        for expected in &root.expected_sources {
            parts.push(expected.identity.as_slice());
        }
    }
    parts.push(protected_primary_expected_set.as_slice());
    commitment(ADMITTED_SET_DOMAIN_V3, &parts)
}

fn derive_overlap_pairs(
    source_cases: &[FoundationSourceCaseV1],
) -> Result<Vec<FoundationProtectedPrimaryOverlapPairV1>, FoundationLegacyQuarantineErrorV1> {
    let mut primary_by_object = std::collections::BTreeMap::new();
    for source in source_cases.iter().filter(|source| {
        source.owner == LegacyQuarantineOwnerDomainV3::ProtectedPrimary
            && source.payload_state == FoundationLegacyPayloadStateV3::Present
    }) {
        let key = (
            source.object_identity,
            source.mount_identity,
            source.provider_identity,
        );
        if primary_by_object.insert(key, source).is_some() {
            return Err(FoundationLegacyQuarantineErrorV1::RootAlias);
        }
    }
    let mut overlap_pairs = source_cases
        .iter()
        .filter(|source| {
            source.owner != LegacyQuarantineOwnerDomainV3::ProtectedPrimary
                && source.payload_state == FoundationLegacyPayloadStateV3::Present
        })
        .filter_map(|source| {
            primary_by_object
                .get(&(
                    source.object_identity,
                    source.mount_identity,
                    source.provider_identity,
                ))
                .filter(|primary| {
                    primary.kind == source.kind
                        && primary.logical_byte_length == source.logical_byte_length
                        && primary.content_identity == source.content_identity
                })
                .map(|primary| FoundationProtectedPrimaryOverlapPairV1 {
                    owner_source_token: source.source_token,
                    primary_source_token: primary.source_token,
                })
        })
        .collect::<Vec<_>>();
    overlap_pairs.sort_by_key(|pair| (pair.owner_source_token, pair.primary_source_token));
    if overlap_pairs.windows(2).any(|pair| {
        pair[0].owner_source_token == pair[1].owner_source_token
            || pair[0].primary_source_token == pair[1].primary_source_token
    }) {
        return Err(FoundationLegacyQuarantineErrorV1::RootAlias);
    }
    Ok(overlap_pairs)
}

#[expect(
    clippy::too_many_arguments,
    reason = "reconciliation binds the complete owner/root/currentness tuple"
)]
fn reconcile_expected_sources(
    invocation: [u8; 32],
    admitted_set: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    display_locator: &[u8],
    root_binding: [u8; 32],
    mount_identity: [u8; 32],
    provider_identity: [u8; 32],
    owner_currentness: [u8; 32],
    owner_attestation: [u8; 32],
    expected_sources: &[LegacyQuarantineExpectedSourceV3],
    observed_rows: &[InventoryRowV1],
) -> Result<Vec<FoundationSourceCaseV1>, FoundationLegacyQuarantineErrorV1> {
    let mut observed = observed_rows
        .iter()
        .map(|row| (row.relative_name().to_vec(), row))
        .collect::<std::collections::BTreeMap<_, _>>();
    if observed.len() != observed_rows.len() {
        return Err(FoundationLegacyQuarantineErrorV1::ExpectedSourceMismatch);
    }
    let mut sources = Vec::with_capacity(expected_sources.len());
    for expected in expected_sources {
        if expected.owner != owner || expected.root_binding != root_binding {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidExpectedSource);
        }
        let payload_state = match observed.remove(&expected.relative_locator) {
            Some(row)
                if expected.loss_evidence_id.is_none()
                    && row.kind() == expected.kind
                    && row.logical_byte_length() == expected.logical_byte_length
                    && row.object_identity() == expected.object_identity
                    && row.content_identity() == expected.content_identity =>
            {
                FoundationLegacyPayloadStateV3::Present
            }
            Some(_) => return Err(FoundationLegacyQuarantineErrorV1::ExpectedSourceMismatch),
            None if expected.loss_evidence_id.is_some() => {
                FoundationLegacyPayloadStateV3::UnavailablePreexistingLoss
            }
            None => return Err(FoundationLegacyQuarantineErrorV1::MissingLossEvidence),
        };
        let source_token = commitment(
            SOURCE_TOKEN_DOMAIN_V1,
            &[&invocation, &admitted_set, &expected.identity],
        );
        sources.push(FoundationSourceCaseV1 {
            source_token,
            owner,
            display_locator: display_locator.to_vec(),
            root_binding,
            relative_locator: expected.relative_locator.clone(),
            payload_state,
            kind: expected.kind,
            logical_byte_length: expected.logical_byte_length,
            object_identity: expected.object_identity,
            content_identity: expected.content_identity,
            source_provenance_id: expected.source_provenance_id,
            loss_evidence_id: expected.loss_evidence_id,
            mount_identity,
            provider_identity,
            owner_currentness,
            owner_attestation,
        });
    }
    if !observed.is_empty() {
        return Err(FoundationLegacyQuarantineErrorV1::ExpectedSourceMismatch);
    }
    Ok(sources)
}

fn valid_relative_locator(locator: &[u8]) -> bool {
    !locator.is_empty()
        && locator.len() <= 16 * 1024
        && locator.first().copied() != Some(b'/')
        && !locator.contains(&0)
        && locator
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

fn locator_contains(parent: &[u8], child: &[u8]) -> bool {
    parent == child
        || (child.starts_with(parent)
            && parent.last().copied() != Some(b'/')
            && child.get(parent.len()).copied() == Some(b'/'))
}

fn commitment(namespace: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[derive(Debug, Error)]
pub(crate) enum FoundationLegacyQuarantineErrorV1 {
    #[error("legacy quarantine owner admission is incomplete, unordered, or invalid")]
    InvalidOwnerAdmission,
    #[error("legacy quarantine invocation must be non-zero")]
    InvalidInvocation,
    #[error("legacy quarantine root binding changed after owner admission")]
    RootBindingDrift,
    #[error("legacy quarantine roots alias or overlap without an admitted primary pair")]
    RootAlias,
    #[error("quarantine custody is not physically separate from all active and protected roots")]
    PhysicalSeparationFailed,
    #[error("legacy quarantine source set contains a duplicate physical membership")]
    DuplicateSource,
    #[error("packet-bound expected legacy source identity is invalid")]
    InvalidExpectedSource,
    #[error("live legacy source bytes differ from the packet-bound expected universe")]
    ExpectedSourceMismatch,
    #[error("expected legacy source is absent without independent preexisting-loss evidence")]
    MissingLossEvidence,
    #[error("legacy quarantine source token is unknown")]
    UnknownSourceToken,
    #[error("legacy quarantine source token is one-use")]
    SourceTokenReused,
    #[error("legacy quarantine source changed during descriptor-serviced copy")]
    SourceChanged,
    #[error("legacy quarantine copy coverage is partial")]
    PartialCopy,
    #[error("legacy quarantine custody did not durably retain the exact copied source")]
    CustodyWriteFailed,
    #[error("legacy quarantine platform is unsupported")]
    UnsupportedPlatform,
    #[error(transparent)]
    SecureFs(#[from] SecureFsError),
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let temp_parent = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            let path = temp_parent.join(format!(
                "maestro-foundation-stage11-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test root");
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct TestAdmission(LegacyQuarantineRootAdmissionFactsV3);

    impl owner_admission_sealed::Sealed for TestAdmission {}

    impl LegacyQuarantineRootAdmissionV3 for TestAdmission {
        fn into_foundation_facts(self) -> LegacyQuarantineRootAdmissionFactsV3 {
            self.0
        }
    }

    fn admission(
        owner: LegacyQuarantineOwnerDomainV3,
        root: &Path,
        expected_sources: LegacyQuarantineExpectedSourceSetV3,
        currentness: u8,
        attestation: u8,
    ) -> TestAdmission {
        TestAdmission(
            LegacyQuarantineRootAdmissionFactsV3::from_owner(
                owner,
                vec![(
                    root.as_os_str().as_bytes().to_vec(),
                    root.to_path_buf(),
                    observe_root_binding_v3(root).expect("observe root binding"),
                )],
                expected_sources,
                [currentness; 32],
                [attestation; 32],
            )
            .expect("owner admission"),
        )
    }

    struct TestPrimary {
        root: PathBuf,
        expected_sources: LegacyQuarantineExpectedSourceSetV3,
        mount_identity: [u8; 32],
        provider_identity: [u8; 32],
        retained: Option<RetainedDescriptorCensusLeaseV3>,
        limits: Option<DescriptorCensusLimitsV1>,
    }

    impl persistence_lease_sealed::Sealed for TestPrimary {}

    impl ProtectedPrimaryBoundaryPortV1 for TestPrimary {
        fn identity(&self) -> [u8; 32] {
            [20; 32]
        }

        fn display_locator(&self) -> &[u8] {
            b"/test/protected-primary"
        }

        fn resolved_locator_commitment(&self) -> [u8; 32] {
            [21; 32]
        }

        fn object_identity(&self) -> [u8; 32] {
            [22; 32]
        }

        fn mount_identity(&self) -> [u8; 32] {
            self.mount_identity
        }

        fn provider_identity(&self) -> [u8; 32] {
            self.provider_identity
        }

        fn anchor_identity(&self) -> [u8; 32] {
            [25; 32]
        }

        fn realm_identity(&self) -> [u8; 32] {
            [26; 32]
        }

        fn currentness(&self) -> [u8; 32] {
            [27; 32]
        }

        fn fence(&self) -> [u8; 32] {
            [28; 32]
        }

        fn revocation_revision(&self) -> u64 {
            1
        }

        fn expected_sources(&self) -> &LegacyQuarantineExpectedSourceSetV3 {
            &self.expected_sources
        }

        fn retain_source_census(
            &mut self,
            limits: DescriptorCensusLimitsV1,
        ) -> Result<(), FoundationLegacyQuarantineErrorV1> {
            self.retained =
                Some(SecureRoot::open(&self.root)?.retain_descriptor_census_root_v3(limits)?);
            self.limits = Some(limits);
            Ok(())
        }

        fn source_rows(&self) -> Result<&[InventoryRowV1], FoundationLegacyQuarantineErrorV1> {
            Ok(self
                .retained
                .as_ref()
                .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?
                .census()
                .rows())
        }

        fn read_source(
            &self,
            relative_locator: &[u8],
            kind: DescriptorCensusObjectKindV1,
        ) -> Result<Vec<u8>, FoundationLegacyQuarantineErrorV1> {
            Ok(self
                .retained
                .as_ref()
                .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?
                .read_immutable(
                    Path::new(std::ffi::OsStr::from_bytes(relative_locator)),
                    kind,
                )?)
        }

        fn final_recheck(self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
            self.retained
                .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?
                .consume_final_recheck(
                    self.limits
                        .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?,
                )?;
            Ok([29; 32])
        }
    }

    struct TestCustody {
        records: Rc<RefCell<Vec<[u8; 32]>>>,
        rolled_back: Rc<RefCell<bool>>,
    }

    impl persistence_lease_sealed::Sealed for TestCustody {}

    impl QuarantineCustodyPortV1 for TestCustody {
        fn identity(&self) -> [u8; 32] {
            [30; 32]
        }

        fn display_locator(&self) -> &[u8] {
            b"/test/quarantine-custody"
        }

        fn resolved_locator_commitment(&self) -> [u8; 32] {
            [31; 32]
        }

        fn object_identity(&self) -> [u8; 32] {
            [32; 32]
        }

        fn mount_identity(&self) -> [u8; 32] {
            [33; 32]
        }

        fn provider_identity(&self) -> [u8; 32] {
            [34; 32]
        }

        fn anchor_identity(&self) -> [u8; 32] {
            [35; 32]
        }

        fn manager_realm_identity(&self) -> [u8; 32] {
            [36; 32]
        }

        fn security_realm_identity(&self) -> [u8; 32] {
            [37; 32]
        }

        fn expected_old(&self) -> [u8; 32] {
            [38; 32]
        }

        fn currentness(&self) -> [u8; 32] {
            [39; 32]
        }

        fn fence(&self) -> [u8; 32] {
            [40; 32]
        }

        fn revocation_revision(&self) -> u64 {
            1
        }

        fn persist_source(
            &mut self,
            source_token: [u8; 32],
            object_identity: [u8; 32],
            kind: DescriptorCensusObjectKindV1,
            bytes: &[u8],
        ) -> Result<FoundationCustodyCopyReceiptV1, FoundationLegacyQuarantineErrorV1> {
            let identity = commitment(
                b"test-custody-record\0",
                &[
                    &source_token,
                    &object_identity,
                    &[match kind {
                        DescriptorCensusObjectKindV1::RegularFile => 1,
                        DescriptorCensusObjectKindV1::SymbolicLink => 2,
                    }],
                    bytes,
                ],
            );
            self.records.borrow_mut().push(identity);
            FoundationCustodyCopyReceiptV1::from_persistence(
                identity,
                bytes.len() as u64,
                Sha256::digest(bytes).into(),
            )
        }

        fn rollback_partial(self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
            *self.rolled_back.borrow_mut() = true;
            Ok([41; 32])
        }

        fn seal_expected_old(
            self,
            candidate_manifest: [u8; 32],
            custody_records: &[[u8; 32]],
        ) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
            if candidate_manifest == [0; 32] || custody_records != self.records.borrow().as_slice()
            {
                return Err(FoundationLegacyQuarantineErrorV1::PartialCopy);
            }
            Ok([42; 32])
        }
    }

    fn owners_and_ports(
        repository: &TempRoot,
        installation: &TempRoot,
        primary: &TempRoot,
    ) -> (TestAdmission, TestAdmission, TestPrimary, TestCustody) {
        let packet_identity = [98; 32];
        let repository_binding =
            observe_root_binding_v3(&repository.0).expect("repository root binding");
        let repository_expected = expected_source_set(
            LegacyQuarantineOwnerDomainV3::Repository,
            &repository.0,
            repository_binding,
            packet_identity,
            None,
        );
        owners_and_ports_with_repository_expected(
            repository,
            installation,
            primary,
            repository_expected,
        )
    }

    fn owners_and_ports_with_repository_expected(
        repository: &TempRoot,
        installation: &TempRoot,
        primary: &TempRoot,
        repository_expected: LegacyQuarantineExpectedSourceSetV3,
    ) -> (TestAdmission, TestAdmission, TestPrimary, TestCustody) {
        let packet_identity = repository_expected.packet_identity();
        let installation_binding =
            observe_root_binding_v3(&installation.0).expect("installation root binding");
        let primary_facts = observe_physical_facts_v1(&primary.0).expect("primary facts");
        let installation_expected = expected_source_set(
            LegacyQuarantineOwnerDomainV3::Installation,
            &installation.0,
            installation_binding,
            packet_identity,
            None,
        );
        let primary_expected = expected_source_set(
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
            &primary.0,
            [21; 32],
            packet_identity,
            None,
        );
        (
            admission(
                LegacyQuarantineOwnerDomainV3::Repository,
                &repository.0,
                repository_expected,
                1,
                2,
            ),
            admission(
                LegacyQuarantineOwnerDomainV3::Installation,
                &installation.0,
                installation_expected,
                3,
                4,
            ),
            TestPrimary {
                root: primary.0.clone(),
                expected_sources: primary_expected,
                mount_identity: primary_facts.mount_identity(),
                provider_identity: primary_facts.provider_identity(),
                retained: None,
                limits: None,
            },
            TestCustody {
                records: Rc::new(RefCell::new(Vec::new())),
                rolled_back: Rc::new(RefCell::new(false)),
            },
        )
    }

    fn rebind_expected_source_set(
        expected: LegacyQuarantineExpectedSourceSetV3,
        root_binding: [u8; 32],
        loss_locator: Option<&[u8]>,
    ) -> LegacyQuarantineExpectedSourceSetV3 {
        let packet_identity = expected.packet_identity;
        let owner = expected.owner;
        let rows = expected
            .rows
            .into_iter()
            .map(|row| {
                let loss_evidence_id =
                    (loss_locator == Some(row.relative_locator.as_slice())).then_some([97; 32]);
                LegacyQuarantineExpectedSourceV3::from_packet(
                    owner,
                    root_binding,
                    row.relative_locator,
                    row.kind,
                    row.logical_byte_length,
                    row.object_identity,
                    row.content_identity,
                    row.source_provenance_id,
                    loss_evidence_id,
                )
                .expect("rebound expected source")
            })
            .collect();
        LegacyQuarantineExpectedSourceSetV3::from_packet(packet_identity, owner, rows)
            .expect("rebound expected source set")
    }

    fn overlap_source(
        owner: LegacyQuarantineOwnerDomainV3,
        source_token: [u8; 32],
        relative_locator: &[u8],
        object_identity: [u8; 32],
        content_identity: [u8; 32],
    ) -> FoundationSourceCaseV1 {
        FoundationSourceCaseV1 {
            source_token,
            owner,
            display_locator: vec![owner.tag()],
            root_binding: [51; 32],
            relative_locator: relative_locator.to_vec(),
            payload_state: FoundationLegacyPayloadStateV3::Present,
            kind: DescriptorCensusObjectKindV1::RegularFile,
            logical_byte_length: 17,
            object_identity,
            content_identity,
            source_provenance_id: [54; 32],
            loss_evidence_id: None,
            mount_identity: [52; 32],
            provider_identity: [53; 32],
            owner_currentness: [55; 32],
            owner_attestation: [56; 32],
        }
    }

    fn expected_source_set(
        owner: LegacyQuarantineOwnerDomainV3,
        root: &Path,
        root_binding: [u8; 32],
        packet_identity: [u8; 32],
        loss_locator: Option<&[u8]>,
    ) -> LegacyQuarantineExpectedSourceSetV3 {
        let lease = SecureRoot::open(root)
            .expect("open expected source root")
            .retain_descriptor_census_root_v3(DescriptorCensusLimitsV1::bounded_default())
            .expect("expected source census");
        let rows = lease
            .census()
            .rows()
            .iter()
            .map(|row| {
                let loss_evidence_id =
                    (loss_locator == Some(row.relative_name())).then_some([97; 32]);
                LegacyQuarantineExpectedSourceV3::from_packet(
                    owner,
                    root_binding,
                    row.relative_name().to_vec(),
                    row.kind(),
                    row.logical_byte_length(),
                    row.object_identity(),
                    row.content_identity(),
                    commitment(
                        b"maestro.test.expected-source-provenance.v3\0",
                        &[row.relative_name()],
                    ),
                    loss_evidence_id,
                )
                .expect("expected source")
            })
            .collect();
        drop(lease);
        LegacyQuarantineExpectedSourceSetV3::from_packet(packet_identity, owner, rows)
            .expect("expected source set")
    }

    #[test]
    fn equal_relative_locators_with_different_objects_do_not_authorize_overlap() {
        let repository = TempRoot::new("overlap-repository");
        let installation = TempRoot::new("overlap-installation");
        let primary = TempRoot::new("overlap-primary");
        fs::write(repository.0.join("legacy.txt"), b"successor bytes").expect("write repository");
        fs::write(
            installation.0.join("installation.txt"),
            b"installation bytes",
        )
        .expect("write installation");
        fs::write(primary.0.join("legacy.txt"), b"protected bytes").expect("write primary");
        let (repository_owner, installation_owner, primary_port, custody) =
            owners_and_ports(&repository, &installation, &primary);

        let lease = FoundationLegacyQuarantineLeaseV1::acquire(
            repository_owner,
            installation_owner,
            primary_port,
            custody,
            [43; 32],
            DescriptorCensusLimitsV1::bounded_default(),
        )
        .expect("acquire Foundation lease");

        assert!(lease.overlap_pairs().is_empty());
    }

    #[test]
    fn same_frozen_object_forms_one_bijective_protected_primary_overlap() {
        let object_identity = [57; 32];
        let content_identity = [58; 32];
        let source_cases = vec![
            overlap_source(
                LegacyQuarantineOwnerDomainV3::Repository,
                [59; 32],
                b"owner-copy.txt",
                object_identity,
                content_identity,
            ),
            overlap_source(
                LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
                [60; 32],
                b"legacy.txt",
                object_identity,
                content_identity,
            ),
        ];

        let overlap_pairs = derive_overlap_pairs(&source_cases).expect("derive overlap");
        assert_eq!(overlap_pairs.len(), 1);
        let pair = &overlap_pairs[0];
        let owner = source_cases
            .iter()
            .find(|source| source.source_token() == pair.owner_source_token())
            .expect("owner source");
        let primary = source_cases
            .iter()
            .find(|source| source.source_token() == pair.primary_source_token())
            .expect("primary source");
        assert_eq!(owner.object_identity(), primary.object_identity());
        assert_eq!(owner.content_identity(), primary.content_identity());
    }

    #[test]
    fn pre_census_missing_source_requires_independent_loss_evidence() {
        let repository = TempRoot::new("pre-census-loss-repository");
        let installation = TempRoot::new("pre-census-loss-installation");
        let primary = TempRoot::new("pre-census-loss-primary");
        let missing = repository.0.join("legacy.txt");
        fs::write(&missing, b"historically expected bytes").expect("write repository");
        fs::write(
            installation.0.join("installation.txt"),
            b"installation bytes",
        )
        .expect("write installation");
        fs::write(primary.0.join("primary.txt"), b"protected bytes").expect("write primary");
        let packet_identity = [98; 32];
        let historical_binding =
            observe_root_binding_v3(&repository.0).expect("historical repository root binding");
        let historical_expected = expected_source_set(
            LegacyQuarantineOwnerDomainV3::Repository,
            &repository.0,
            historical_binding,
            packet_identity,
            None,
        );
        fs::remove_file(&missing).expect("remove before Foundation census");
        let live_binding =
            observe_root_binding_v3(&repository.0).expect("live repository root binding");
        let repository_expected =
            rebind_expected_source_set(historical_expected, live_binding, Some(b"legacy.txt"));
        let (repository_owner, installation_owner, primary_port, custody) =
            owners_and_ports_with_repository_expected(
                &repository,
                &installation,
                &primary,
                repository_expected,
            );

        let lease = FoundationLegacyQuarantineLeaseV1::acquire(
            repository_owner,
            installation_owner,
            primary_port,
            custody,
            [46; 32],
            DescriptorCensusLimitsV1::bounded_default(),
        )
        .expect("independently evidenced loss remains in the source universe");
        let loss = lease
            .source_cases()
            .iter()
            .find(|source| source.relative_locator() == b"legacy.txt")
            .expect("missing source remains represented");
        assert_eq!(
            loss.payload_state(),
            FoundationLegacyPayloadStateV3::UnavailablePreexistingLoss
        );
        assert_eq!(loss.loss_evidence_id(), Some([97; 32]));
    }

    #[test]
    fn pre_census_missing_source_without_loss_evidence_refuses_the_epoch() {
        let repository = TempRoot::new("pre-census-unevidenced-repository");
        let installation = TempRoot::new("pre-census-unevidenced-installation");
        let primary = TempRoot::new("pre-census-unevidenced-primary");
        let missing = repository.0.join("legacy.txt");
        fs::write(&missing, b"historically expected bytes").expect("write repository");
        fs::write(
            installation.0.join("installation.txt"),
            b"installation bytes",
        )
        .expect("write installation");
        fs::write(primary.0.join("primary.txt"), b"protected bytes").expect("write primary");
        let packet_identity = [98; 32];
        let historical_binding =
            observe_root_binding_v3(&repository.0).expect("historical repository root binding");
        let historical_expected = expected_source_set(
            LegacyQuarantineOwnerDomainV3::Repository,
            &repository.0,
            historical_binding,
            packet_identity,
            None,
        );
        fs::remove_file(&missing).expect("remove before Foundation census");
        let live_binding =
            observe_root_binding_v3(&repository.0).expect("live repository root binding");
        let repository_expected =
            rebind_expected_source_set(historical_expected, live_binding, None);
        let (repository_owner, installation_owner, primary_port, custody) =
            owners_and_ports_with_repository_expected(
                &repository,
                &installation,
                &primary,
                repository_expected,
            );

        assert!(matches!(
            FoundationLegacyQuarantineLeaseV1::acquire(
                repository_owner,
                installation_owner,
                primary_port,
                custody,
                [47; 32],
                DescriptorCensusLimitsV1::bounded_default(),
            ),
            Err(FoundationLegacyQuarantineErrorV1::MissingLossEvidence)
        ));
    }

    #[test]
    fn unavailable_bytes_are_physical_failure_and_never_become_a_fabricated_loss_receipt() {
        let repository = TempRoot::new("loss-repository");
        let installation = TempRoot::new("loss-installation");
        let primary = TempRoot::new("loss-primary");
        let missing_path = repository.0.join("legacy.txt");
        fs::write(&missing_path, b"soon unavailable").expect("write repository");
        fs::write(
            installation.0.join("installation.txt"),
            b"installation bytes",
        )
        .expect("write installation");
        fs::write(primary.0.join("primary.txt"), b"protected bytes").expect("write primary");
        let (repository_owner, installation_owner, primary_port, custody) =
            owners_and_ports(&repository, &installation, &primary);
        let rolled_back = Rc::clone(&custody.rolled_back);
        let lease = FoundationLegacyQuarantineLeaseV1::acquire(
            repository_owner,
            installation_owner,
            primary_port,
            custody,
            [44; 32],
            DescriptorCensusLimitsV1::bounded_default(),
        )
        .expect("acquire Foundation lease");
        let missing_token = lease
            .source_cases()
            .iter()
            .find(|source| {
                source.owner() == LegacyQuarantineOwnerDomainV3::Repository
                    && source.relative_locator() == b"legacy.txt"
            })
            .expect("missing source")
            .source_token();
        fs::remove_file(missing_path).expect("make admitted source unavailable");
        let mut continuation = lease.into_copy_continuation();

        assert!(matches!(
            continuation.copy_once(missing_token),
            Err(FoundationLegacyQuarantineErrorV1::SecureFs(_))
                | Err(FoundationLegacyQuarantineErrorV1::SourceChanged)
        ));
        assert_ne!(continuation.rollback().expect("rollback"), [0; 32]);
        assert!(*rolled_back.borrow());
    }
}
