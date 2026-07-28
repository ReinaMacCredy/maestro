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
        owner_currentness: [u8; 32],
        owner_attestation: [u8; 32],
    ) -> Result<Self, FoundationLegacyQuarantineErrorV1> {
        if roots.is_empty()
            || owner_currentness == [0; 32]
            || owner_attestation == [0; 32]
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
        Ok(Self {
            owner,
            roots,
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
    lease: RetainedDescriptorCensusLeaseV3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FoundationSourceCaseV1 {
    source_token: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    display_locator: Vec<u8>,
    root_binding: [u8; 32],
    relative_locator: Vec<u8>,
    row: InventoryRowV1,
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

    pub(crate) const fn row(&self) -> &InventoryRowV1 {
        &self.row
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
        {
            return Err(FoundationLegacyQuarantineErrorV1::InvalidOwnerAdmission);
        }
        validate_physical_separation(&repository, &installation, &protected_primary, &custody)?;
        protected_primary.retain_source_census(limits)?;
        let mut roots = Vec::new();
        for admission in [repository, installation] {
            for admitted in admission.roots {
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
                roots.push(RetainedRootV1 {
                    owner: admission.owner,
                    display_locator: admitted.display_locator,
                    root_binding: admitted.root_binding,
                    mount_identity: physical.mount_identity(),
                    provider_identity: physical.provider_identity(),
                    owner_currentness: admission.owner_currentness,
                    owner_attestation: admission.owner_attestation,
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
        let admitted_set = admitted_set_identity(invocation, &roots);
        let mut source_cases = Vec::new();
        for (root_index, root) in roots.iter().enumerate() {
            for row in root.lease.census().rows() {
                let source_token = commitment(
                    SOURCE_TOKEN_DOMAIN_V1,
                    &[
                        &invocation,
                        &admitted_set,
                        &(root_index as u64).to_be_bytes(),
                        row.relative_name(),
                        &row.object_identity(),
                        &row.content_identity(),
                    ],
                );
                source_cases.push(FoundationSourceCaseV1 {
                    source_token,
                    owner: root.owner,
                    display_locator: root.display_locator.clone(),
                    root_binding: root.root_binding,
                    relative_locator: row.relative_name().to_vec(),
                    row: row.clone(),
                    mount_identity: root.mount_identity,
                    provider_identity: root.provider_identity,
                    owner_currentness: root.owner_currentness,
                    owner_attestation: root.owner_attestation,
                });
            }
        }
        let primary_root_binding = protected_primary.resolved_locator_commitment();
        let primary_currentness = protected_primary.currentness();
        let primary_attestation = protected_primary.identity();
        for row in protected_primary.source_rows()? {
            let source_token = commitment(
                SOURCE_TOKEN_DOMAIN_V1,
                &[
                    &invocation,
                    &admitted_set,
                    &[LegacyQuarantineOwnerDomainV3::ProtectedPrimary.tag()],
                    row.relative_name(),
                    &row.object_identity(),
                    &row.content_identity(),
                ],
            );
            source_cases.push(FoundationSourceCaseV1 {
                source_token,
                owner: LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
                display_locator: protected_primary.display_locator().to_vec(),
                root_binding: primary_root_binding,
                relative_locator: row.relative_name().to_vec(),
                row: row.clone(),
                mount_identity: protected_primary.mount_identity(),
                provider_identity: protected_primary.provider_identity(),
                owner_currentness: primary_currentness,
                owner_attestation: primary_attestation,
            });
        }
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
        let primary_by_locator = source_cases
            .iter()
            .filter(|source| source.owner == LegacyQuarantineOwnerDomainV3::ProtectedPrimary)
            .map(|source| (source.relative_locator.as_slice(), source.source_token))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut overlap_pairs = source_cases
            .iter()
            .filter(|source| source.owner != LegacyQuarantineOwnerDomainV3::ProtectedPrimary)
            .filter_map(|source| {
                primary_by_locator
                    .get(source.relative_locator.as_slice())
                    .map(
                        |primary_source_token| FoundationProtectedPrimaryOverlapPairV1 {
                            owner_source_token: source.source_token,
                            primary_source_token: *primary_source_token,
                        },
                    )
            })
            .collect::<Vec<_>>();
        overlap_pairs.sort_by_key(|pair| (pair.owner_source_token, pair.primary_source_token));
        if overlap_pairs.windows(2).any(|pair| {
            pair[0].owner_source_token == pair[1].owner_source_token
                || pair[0].primary_source_token == pair[1].primary_source_token
        }) {
            return Err(FoundationLegacyQuarantineErrorV1::RootAlias);
        }
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
        let bytes = if source.owner == LegacyQuarantineOwnerDomainV3::ProtectedPrimary {
            self.lease
                .protected_primary
                .read_source(&source.relative_locator, source.row.kind())?
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
                source.row.kind(),
            )?
        };
        if bytes.len() as u64 != source.row.logical_byte_length()
            || <[u8; 32]>::from(Sha256::digest(&bytes)) != source.row.content_identity()
        {
            return Err(FoundationLegacyQuarantineErrorV1::SourceChanged);
        }
        let receipt = self.lease.custody.persist_source(
            source.source_token,
            source.row.object_identity(),
            source.row.kind(),
            &bytes,
        )?;
        if receipt.copied_length() != source.row.logical_byte_length()
            || receipt.copied_sha256() != source.row.content_identity()
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
        if candidate_manifest == [0; 32] || self.copied.len() != self.lease.source_cases.len() {
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

fn admitted_set_identity(invocation: [u8; 32], roots: &[RetainedRootV1]) -> [u8; 32] {
    let mut parts = Vec::new();
    parts.push(invocation.as_slice());
    for root in roots {
        parts.push(root.display_locator.as_slice());
        parts.push(root.root_binding.as_slice());
        parts.push(root.owner_currentness.as_slice());
        parts.push(root.owner_attestation.as_slice());
    }
    commitment(ADMITTED_SET_DOMAIN_V3, &parts)
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
                [currentness; 32],
                [attestation; 32],
            )
            .expect("owner admission"),
        )
    }

    struct TestPrimary {
        root: PathBuf,
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
            [23; 32]
        }

        fn provider_identity(&self) -> [u8; 32] {
            [24; 32]
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
        (
            admission(
                LegacyQuarantineOwnerDomainV3::Repository,
                &repository.0,
                1,
                2,
            ),
            admission(
                LegacyQuarantineOwnerDomainV3::Installation,
                &installation.0,
                3,
                4,
            ),
            TestPrimary {
                root: primary.0.clone(),
                retained: None,
                limits: None,
            },
            TestCustody {
                records: Rc::new(RefCell::new(Vec::new())),
                rolled_back: Rc::new(RefCell::new(false)),
            },
        )
    }

    #[test]
    fn owner_and_protected_primary_memberships_with_the_same_locator_form_one_bijective_pair() {
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

        assert_eq!(lease.overlap_pairs().len(), 1);
        let pair = &lease.overlap_pairs()[0];
        let owner = lease
            .source_cases()
            .iter()
            .find(|source| source.source_token() == pair.owner_source_token())
            .expect("owner source");
        let primary = lease
            .source_cases()
            .iter()
            .find(|source| source.source_token() == pair.primary_source_token())
            .expect("primary source");
        assert_ne!(
            owner.owner(),
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary
        );
        assert_eq!(
            primary.owner(),
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary
        );
        assert_ne!(
            owner.row().content_identity(),
            primary.row().content_identity()
        );
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
