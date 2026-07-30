use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::distribution::{
    BundleIdV1, BundleManifestV1, CommitmentV1, EmbeddedReleaseBundleV1, ReleaseIdV1,
    ReleaseResourceCensusV1, ResourceDescriptorV1, ResourceIdV1,
};
use crate::domain::identity::StoreObjectIdV1;
use crate::domain::persistence::{StoreObjectError, StoreObjectV1};
use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

use super::{
    DistributionDomainKindV1, DistributionDomainRefV1, DistributionModelErrorV1,
    DistributionMutationKindV1, DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
    ManagedTargetCustodyClassV1, OrdinarySnapshotCatalogStateV1,
};

const MAX_CLAIMS_V1: usize = 65_535;
const MAX_SNAPSHOT_TARGETS_V1: usize = 65_535;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseMaterializationClosureV1 {
    release_id: ReleaseIdV1,
    resource_bindings: BTreeMap<ResourceIdV1, (BundleIdV1, CommitmentV1)>,
}

impl ReleaseMaterializationClosureV1 {
    pub fn new(
        release: &EmbeddedReleaseBundleV1,
        census: &ReleaseResourceCensusV1,
        resources: &[ResourceDescriptorV1],
        bundles: &[BundleManifestV1],
    ) -> Result<Self, DistributionRecordErrorV1> {
        let resource_ids = resources
            .iter()
            .map(ResourceDescriptorV1::id)
            .collect::<BTreeSet<_>>();
        let bundle_ids = bundles.iter().map(BundleManifestV1::id).collect::<Vec<_>>();
        if release.census_id() != census.id()
            || release.bundle_ids() != bundle_ids
            || census
                .resource_ids()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != resource_ids
            || census.bundle_ids() != bundle_ids
        {
            return Err(DistributionRecordErrorV1::InvalidReleaseClosure);
        }
        let by_resource = resources
            .iter()
            .map(|resource| (resource.id(), resource))
            .collect::<BTreeMap<_, _>>();
        if by_resource.len() != resources.len() {
            return Err(DistributionRecordErrorV1::InvalidReleaseClosure);
        }
        let mut resource_bindings = BTreeMap::new();
        for bundle in bundles {
            for resource_id in bundle.resource_ids() {
                let resource = by_resource
                    .get(resource_id)
                    .ok_or(DistributionRecordErrorV1::InvalidReleaseClosure)?;
                if resource.required_bundle_kind() != bundle.kind()
                    || resource_bindings
                        .insert(
                            *resource_id,
                            (bundle.id(), resource_content_sha256(resource)?),
                        )
                        .is_some()
                {
                    return Err(DistributionRecordErrorV1::InvalidReleaseClosure);
                }
            }
        }
        if resource_bindings.len() != resources.len() {
            return Err(DistributionRecordErrorV1::InvalidReleaseClosure);
        }
        Ok(Self {
            release_id: release.release_id(),
            resource_bindings,
        })
    }

    pub const fn release_id(&self) -> ReleaseIdV1 {
        self.release_id
    }

    pub fn contains(
        &self,
        resource_id: ResourceIdV1,
        bundle_id: BundleIdV1,
        content_sha256: CommitmentV1,
    ) -> bool {
        self.resource_bindings.get(&resource_id) == Some(&(bundle_id, content_sha256))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseMaterializationProofV1 {
    claim_set_object_id: StoreObjectIdV1,
    release_id: ReleaseIdV1,
}

impl ReleaseMaterializationProofV1 {
    pub(crate) const fn claim_set_object_id(&self) -> StoreObjectIdV1 {
        self.claim_set_object_id
    }

    pub(crate) const fn release_id(&self) -> ReleaseIdV1 {
        self.release_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledResourceClaimV1 {
    pub claim_tag: u64,
    pub domain: DistributionDomainRefV1,
    pub canonical_target_identity_ref: DistributionScopedObjectRefV1,
    pub display_locator: String,
    pub resolved_locator: String,
    pub custody_class: ManagedTargetCustodyClassV1,
    pub resource_id: ResourceIdV1,
    pub bundle_id: BundleIdV1,
    pub release_id: ReleaseIdV1,
    pub content_sha256: CommitmentV1,
    pub managed_block_ref: Option<DistributionScopedObjectRefV1>,
    pub alias_closure_ref: DistributionScopedObjectRefV1,
    pub verification_profile_id: CommitmentV1,
}

impl InstalledResourceClaimV1 {
    pub fn validate(&self) -> Result<(), DistributionRecordErrorV1> {
        if self.claim_tag == 0 || self.claim_tag > 65_535 {
            return Err(DistributionRecordErrorV1::InvalidRowTag);
        }
        validate_locator(&self.display_locator)?;
        validate_locator(&self.resolved_locator)?;
        self.canonical_target_identity_ref
            .require_same_domain(&self.domain)?;
        self.canonical_target_identity_ref
            .require_kind(DistributionRuntimeObjectKindV1::CanonicalTargetIdentity)?;
        self.alias_closure_ref.require_same_domain(&self.domain)?;
        self.alias_closure_ref
            .require_kind(DistributionRuntimeObjectKindV1::AliasClosure)?;
        match (self.custody_class, &self.managed_block_ref) {
            (ManagedTargetCustodyClassV1::MaestroOwnedTarget, None) => {}
            (ManagedTargetCustodyClassV1::SharedManagedBlock, Some(block_ref)) => {
                block_ref.require_same_domain(&self.domain)?;
                block_ref.require_kind(DistributionRuntimeObjectKindV1::ManagedBlock)?;
            }
            _ => return Err(DistributionRecordErrorV1::ManagedBlockCustodyMismatch),
        }
        validate_nonzero(&[
            self.resource_id,
            self.bundle_id,
            self.release_id,
            self.content_sha256,
            self.verification_profile_id,
        ])
    }

    pub(super) fn canonical_value(&self) -> Result<CborValue, DistributionRecordErrorV1> {
        self.validate()?;
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.claim_tag),
            self.domain.canonical_value(),
            self.canonical_target_identity_ref.canonical_value(),
            CborValue::text(self.display_locator.clone())?,
            CborValue::text(self.resolved_locator.clone())?,
            CborValue::Unsigned(self.custody_class.numeric_tag()),
            bytes(self.resource_id),
            bytes(self.bundle_id),
            bytes(self.release_id),
            bytes(self.content_sha256),
            optional_ref(self.managed_block_ref.as_ref()),
            self.alias_closure_ref.canonical_value(),
            bytes(self.verification_profile_id),
            CborValue::Unsigned(1),
        ]))
    }

    fn add_references(&self, references: &mut BTreeSet<StoreObjectIdV1>) {
        references.insert(self.canonical_target_identity_ref.object_id());
        references.insert(self.alias_closure_ref.object_id());
        if let Some(block_ref) = &self.managed_block_ref {
            references.insert(block_ref.object_id());
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledResourceClaimSetV1 {
    pub domain: DistributionDomainRefV1,
    pub release_id: ReleaseIdV1,
    pub prior_claim_set_ref: Option<DistributionScopedObjectRefV1>,
    pub rows: Vec<(u64, CommitmentV1, InstalledResourceClaimV1)>,
}

impl InstalledResourceClaimSetV1 {
    pub fn validate(&self) -> Result<(), DistributionRecordErrorV1> {
        if self.rows.is_empty() || self.rows.len() > MAX_CLAIMS_V1 {
            return Err(DistributionRecordErrorV1::InvalidRowCount);
        }
        validate_nonzero(&[self.release_id])?;
        if let Some(prior) = &self.prior_claim_set_ref {
            prior.require_same_domain(&self.domain)?;
            prior.require_kind(DistributionRuntimeObjectKindV1::InstalledResourceClaimSet)?;
        }
        let mut prior_key = None;
        for (tag, row_id, claim) in &self.rows {
            claim.validate()?;
            if claim.domain != self.domain
                || claim.release_id != self.release_id
                || claim.claim_tag != *tag
                || row_id.as_bytes() == &[0; 32]
            {
                return Err(DistributionRecordErrorV1::RowBindingMismatch);
            }
            let key = (*tag, *row_id);
            if prior_key.is_some_and(|prior| prior >= key) {
                return Err(DistributionRecordErrorV1::RowsNotStrictlySorted);
            }
            prior_key = Some(key);
        }
        Ok(())
    }

    pub fn materialize_against_release(
        &self,
        release: &ReleaseMaterializationClosureV1,
    ) -> Result<(StoreObjectV1, ReleaseMaterializationProofV1), DistributionRecordErrorV1> {
        let mut claimed_resources = BTreeSet::new();
        if self.release_id != release.release_id
            || self.rows.len() != release.resource_bindings.len()
            || self.rows.iter().any(|(_, _, claim)| {
                !claimed_resources.insert(claim.resource_id)
                    || !release.contains(claim.resource_id, claim.bundle_id, claim.content_sha256)
            })
            || claimed_resources.len() != release.resource_bindings.len()
        {
            return Err(DistributionRecordErrorV1::ClaimOutsideReleaseClosure);
        }
        let object = self.to_store_object()?;
        let proof = ReleaseMaterializationProofV1 {
            claim_set_object_id: object.id(),
            release_id: release.release_id,
        };
        Ok((object, proof))
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, DistributionRecordErrorV1> {
        self.validate()?;
        let header = CborValue::Array(vec![
            self.domain.canonical_value(),
            bytes(self.release_id),
            optional_ref(self.prior_claim_set_ref.as_ref()),
            CborValue::Unsigned(self.rows.len() as u64),
            CborValue::Unsigned(1),
        ]);
        let rows = CborValue::Array(
            self.rows
                .iter()
                .map(|(tag, row_id, claim)| {
                    Ok(CborValue::Array(vec![
                        CborValue::Unsigned(*tag),
                        bytes(*row_id),
                        claim.canonical_value()?,
                    ]))
                })
                .collect::<Result<Vec<_>, DistributionRecordErrorV1>>()?,
        );
        let mut references = BTreeSet::new();
        if let Some(prior) = &self.prior_claim_set_ref {
            references.insert(prior.object_id());
        }
        for (_, _, claim) in &self.rows {
            claim.add_references(&mut references);
        }
        store_object(
            DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
            CborValue::Array(vec![header, rows]),
            references,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionSnapshotTargetV1 {
    pub target_tag: u64,
    pub domain: DistributionDomainRefV1,
    pub canonical_target_identity_ref: DistributionScopedObjectRefV1,
    pub prior_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub content_object_ref: Option<DistributionScopedObjectRefV1>,
    pub content_sha256: Option<CommitmentV1>,
    pub prior_absence: bool,
    pub permissions_commitment_id: CommitmentV1,
    pub owner_metadata_commitment_id: CommitmentV1,
    pub managed_block_ref: Option<DistributionScopedObjectRefV1>,
    pub restore_profile_id: CommitmentV1,
}

impl DistributionSnapshotTargetV1 {
    pub fn validate(&self) -> Result<(), DistributionRecordErrorV1> {
        if self.target_tag == 0 {
            return Err(DistributionRecordErrorV1::InvalidRowTag);
        }
        self.canonical_target_identity_ref
            .require_same_domain(&self.domain)?;
        self.canonical_target_identity_ref
            .require_kind(DistributionRuntimeObjectKindV1::CanonicalTargetIdentity)?;
        require_optional_ref(
            self.prior_claim_ref.as_ref(),
            &self.domain,
            DistributionRuntimeObjectKindV1::InstalledResourceClaim,
        )?;
        require_optional_ref(
            self.content_object_ref.as_ref(),
            &self.domain,
            DistributionRuntimeObjectKindV1::ContentObject,
        )?;
        require_optional_ref(
            self.managed_block_ref.as_ref(),
            &self.domain,
            DistributionRuntimeObjectKindV1::ManagedBlock,
        )?;
        validate_nonzero(&[
            self.permissions_commitment_id,
            self.owner_metadata_commitment_id,
            self.restore_profile_id,
        ])?;
        if self.prior_absence {
            if self.prior_claim_ref.is_some()
                || self.content_object_ref.is_some()
                || self.content_sha256.is_some()
                || self.managed_block_ref.is_some()
            {
                return Err(DistributionRecordErrorV1::InvalidAbsentPreimage);
            }
        } else if self.content_object_ref.is_none() || self.content_sha256.is_none() {
            return Err(DistributionRecordErrorV1::IncompletePresentPreimage);
        }
        if self
            .content_sha256
            .is_some_and(|commitment| commitment.as_bytes() == &[0; 32])
        {
            return Err(DistributionRecordErrorV1::ZeroCommitment);
        }
        Ok(())
    }

    pub(super) fn canonical_value(&self) -> Result<CborValue, DistributionRecordErrorV1> {
        self.validate()?;
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.target_tag),
            self.domain.canonical_value(),
            self.canonical_target_identity_ref.canonical_value(),
            optional_ref(self.prior_claim_ref.as_ref()),
            optional_ref(self.content_object_ref.as_ref()),
            CborValue::optional(self.content_sha256.map(bytes)),
            CborValue::Bool(self.prior_absence),
            bytes(self.permissions_commitment_id),
            bytes(self.owner_metadata_commitment_id),
            optional_ref(self.managed_block_ref.as_ref()),
            bytes(self.restore_profile_id),
            CborValue::Unsigned(1),
        ]))
    }

    fn add_references(&self, references: &mut BTreeSet<StoreObjectIdV1>) {
        references.insert(self.canonical_target_identity_ref.object_id());
        for reference in [
            self.prior_claim_ref.as_ref(),
            self.content_object_ref.as_ref(),
            self.managed_block_ref.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            references.insert(reference.object_id());
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionSnapshotV1 {
    pub domain: DistributionDomainRefV1,
    pub captured_commit_ref: DistributionScopedObjectRefV1,
    pub effect_intent_ref: DistributionScopedObjectRefV1,
    pub release_id: Option<ReleaseIdV1>,
    pub claim_set_ref: DistributionScopedObjectRefV1,
    pub compatibility_closure_id: CommitmentV1,
    pub rows: Vec<(u64, CommitmentV1, DistributionSnapshotTargetV1)>,
}

impl DistributionSnapshotV1 {
    pub fn validate(&self) -> Result<(), DistributionRecordErrorV1> {
        if self.rows.is_empty() || self.rows.len() > MAX_SNAPSHOT_TARGETS_V1 {
            return Err(DistributionRecordErrorV1::InvalidRowCount);
        }
        require_ref(
            &self.captured_commit_ref,
            &self.domain,
            DistributionRuntimeObjectKindV1::DistributionCommitRecord,
        )?;
        require_ref(
            &self.effect_intent_ref,
            &self.domain,
            DistributionRuntimeObjectKindV1::EffectIntent,
        )?;
        require_ref(
            &self.claim_set_ref,
            &self.domain,
            DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
        )?;
        validate_nonzero(&[self.compatibility_closure_id])?;
        validate_release_binding(&self.domain, self.release_id)?;
        let mut prior_key = None;
        for (tag, row_id, target) in &self.rows {
            target.validate()?;
            let key = (*tag, *row_id);
            if target.target_tag != *tag
                || target.domain != self.domain
                || row_id.as_bytes() == &[0; 32]
                || prior_key.is_some_and(|prior| prior >= key)
            {
                return Err(DistributionRecordErrorV1::RowBindingMismatch);
            }
            prior_key = Some(key);
        }
        Ok(())
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, DistributionRecordErrorV1> {
        self.validate()?;
        let header = CborValue::Array(vec![
            self.domain.canonical_value(),
            self.captured_commit_ref.canonical_value(),
            self.effect_intent_ref.canonical_value(),
            CborValue::optional(self.release_id.map(bytes)),
            self.claim_set_ref.canonical_value(),
            bytes(self.compatibility_closure_id),
            CborValue::Unsigned(self.rows.len() as u64),
            CborValue::Unsigned(1),
            CborValue::Unsigned(1),
        ]);
        let rows = CborValue::Array(
            self.rows
                .iter()
                .map(|(tag, row_id, target)| {
                    Ok(CborValue::Array(vec![
                        CborValue::Unsigned(*tag),
                        bytes(*row_id),
                        target.canonical_value()?,
                    ]))
                })
                .collect::<Result<Vec<_>, DistributionRecordErrorV1>>()?,
        );
        let mut references = BTreeSet::from([
            self.captured_commit_ref.object_id(),
            self.effect_intent_ref.object_id(),
            self.claim_set_ref.object_id(),
        ]);
        for (_, _, target) in &self.rows {
            target.add_references(&mut references);
        }
        store_object(
            DistributionRuntimeObjectKindV1::DistributionSnapshot,
            CborValue::Array(vec![header, rows]),
            references,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrdinarySnapshotCatalogV1 {
    pub state: OrdinarySnapshotCatalogStateV1,
    pub retention_pin_set_ref: DistributionScopedObjectRefV1,
    pub cleanup_debt_set_ref: DistributionScopedObjectRefV1,
}

impl OrdinarySnapshotCatalogV1 {
    pub fn to_store_object(&self) -> Result<StoreObjectV1, DistributionRecordErrorV1> {
        let domain = self.state.domain();
        require_ref(
            &self.retention_pin_set_ref,
            domain,
            DistributionRuntimeObjectKindV1::RetentionPinSet,
        )?;
        require_ref(
            &self.cleanup_debt_set_ref,
            domain,
            DistributionRuntimeObjectKindV1::CleanupDebtSet,
        )?;
        let header = CborValue::Array(vec![
            domain.canonical_value(),
            self.state.excluded_current_state_ref().canonical_value(),
            CborValue::Unsigned(self.state.eligible().len() as u64),
            CborValue::Unsigned(self.state.eligible().len() as u64),
            self.retention_pin_set_ref.canonical_value(),
            self.cleanup_debt_set_ref.canonical_value(),
            CborValue::Unsigned(1),
        ]);
        let rows = CborValue::Array(
            self.state
                .eligible()
                .iter()
                .enumerate()
                .map(|(index, (snapshot_ref, source_commit_ref, sequence))| {
                    CborValue::Array(vec![
                        CborValue::Unsigned((index + 1) as u64),
                        domain.canonical_value(),
                        snapshot_ref.canonical_value(),
                        source_commit_ref.canonical_value(),
                        CborValue::Unsigned(*sequence),
                        CborValue::Unsigned(1),
                        CborValue::Unsigned(1),
                    ])
                })
                .collect(),
        );
        let mut references = BTreeSet::from([
            self.state.excluded_current_state_ref().object_id(),
            self.retention_pin_set_ref.object_id(),
            self.cleanup_debt_set_ref.object_id(),
        ]);
        for (snapshot_ref, source_commit_ref, _) in self.state.eligible() {
            references.insert(snapshot_ref.object_id());
            references.insert(source_commit_ref.object_id());
        }
        store_object(
            DistributionRuntimeObjectKindV1::OrdinarySnapshotCatalog,
            CborValue::Array(vec![header, rows]),
            references,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionReceiptV1 {
    pub domain: DistributionDomainRefV1,
    pub mutation_kind: DistributionMutationKindV1,
    pub request_or_ceremony_ref: DistributionScopedObjectRefV1,
    pub plan_ref: DistributionScopedObjectRefV1,
    pub effect_intent_set_ref: DistributionScopedObjectRefV1,
    pub prior_receipt_ref: Option<DistributionScopedObjectRefV1>,
    pub release_id: Option<ReleaseIdV1>,
    pub claim_set_ref: DistributionScopedObjectRefV1,
    pub snapshot_catalog_ref: DistributionScopedObjectRefV1,
    pub verification_result_ref: DistributionScopedObjectRefV1,
    pub authorization_receipt_set_ref: DistributionScopedObjectRefV1,
    pub committed_operation_result_ref: DistributionScopedObjectRefV1,
}

impl DistributionReceiptV1 {
    pub fn validate(&self) -> Result<(), DistributionRecordErrorV1> {
        let required = [
            (
                &self.request_or_ceremony_ref,
                DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
            ),
            (
                &self.plan_ref,
                DistributionRuntimeObjectKindV1::DistributionPlan,
            ),
            (
                &self.effect_intent_set_ref,
                DistributionRuntimeObjectKindV1::EffectIntentSet,
            ),
            (
                &self.claim_set_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
            ),
            (
                &self.snapshot_catalog_ref,
                DistributionRuntimeObjectKindV1::OrdinarySnapshotCatalog,
            ),
            (
                &self.verification_result_ref,
                DistributionRuntimeObjectKindV1::VerificationResult,
            ),
            (
                &self.authorization_receipt_set_ref,
                DistributionRuntimeObjectKindV1::AuthorizationReceiptSet,
            ),
            (
                &self.committed_operation_result_ref,
                DistributionRuntimeObjectKindV1::OperationResult,
            ),
        ];
        for (reference, kind) in required {
            require_ref(reference, &self.domain, kind)?;
        }
        require_optional_ref(
            self.prior_receipt_ref.as_ref(),
            &self.domain,
            DistributionRuntimeObjectKindV1::DistributionReceipt,
        )?;
        validate_release_binding(&self.domain, self.release_id)
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, DistributionRecordErrorV1> {
        self.validate()?;
        let refs = [
            Some(&self.request_or_ceremony_ref),
            Some(&self.plan_ref),
            Some(&self.effect_intent_set_ref),
            self.prior_receipt_ref.as_ref(),
            Some(&self.claim_set_ref),
            Some(&self.snapshot_catalog_ref),
            Some(&self.verification_result_ref),
            Some(&self.authorization_receipt_set_ref),
            Some(&self.committed_operation_result_ref),
        ];
        let references = refs
            .into_iter()
            .flatten()
            .map(DistributionScopedObjectRefV1::object_id)
            .collect();
        store_object(
            DistributionRuntimeObjectKindV1::DistributionReceipt,
            CborValue::Array(vec![
                self.domain.canonical_value(),
                CborValue::Unsigned(self.mutation_kind.numeric_tag()),
                self.request_or_ceremony_ref.canonical_value(),
                self.plan_ref.canonical_value(),
                self.effect_intent_set_ref.canonical_value(),
                optional_ref(self.prior_receipt_ref.as_ref()),
                CborValue::optional(self.release_id.map(bytes)),
                self.claim_set_ref.canonical_value(),
                self.snapshot_catalog_ref.canonical_value(),
                self.verification_result_ref.canonical_value(),
                self.authorization_receipt_set_ref.canonical_value(),
                self.committed_operation_result_ref.canonical_value(),
                CborValue::Unsigned(1),
                CborValue::Unsigned(1),
            ]),
            references,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionCommitRecordV1 {
    pub domain: DistributionDomainRefV1,
    pub prior_commit_ref: Option<DistributionScopedObjectRefV1>,
    pub receipt_ref: DistributionScopedObjectRefV1,
    pub claim_set_ref: DistributionScopedObjectRefV1,
    pub snapshot_catalog_ref: DistributionScopedObjectRefV1,
    pub current_release_id: Option<ReleaseIdV1>,
    pub operation_result_ref: DistributionScopedObjectRefV1,
    pub idempotency_key_ref: DistributionScopedObjectRefV1,
}

impl DistributionCommitRecordV1 {
    pub fn validate(&self) -> Result<(), DistributionRecordErrorV1> {
        require_optional_ref(
            self.prior_commit_ref.as_ref(),
            &self.domain,
            DistributionRuntimeObjectKindV1::DistributionCommitRecord,
        )?;
        for (reference, kind) in [
            (
                &self.receipt_ref,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
            ),
            (
                &self.claim_set_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
            ),
            (
                &self.snapshot_catalog_ref,
                DistributionRuntimeObjectKindV1::OrdinarySnapshotCatalog,
            ),
            (
                &self.operation_result_ref,
                DistributionRuntimeObjectKindV1::OperationResult,
            ),
            (
                &self.idempotency_key_ref,
                DistributionRuntimeObjectKindV1::IdempotencyKey,
            ),
        ] {
            require_ref(reference, &self.domain, kind)?;
        }
        validate_release_binding(&self.domain, self.current_release_id)
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, DistributionRecordErrorV1> {
        self.validate()?;
        let references = [
            self.prior_commit_ref.as_ref(),
            Some(&self.receipt_ref),
            Some(&self.claim_set_ref),
            Some(&self.snapshot_catalog_ref),
            Some(&self.operation_result_ref),
            Some(&self.idempotency_key_ref),
        ]
        .into_iter()
        .flatten()
        .map(DistributionScopedObjectRefV1::object_id)
        .collect();
        store_object(
            DistributionRuntimeObjectKindV1::DistributionCommitRecord,
            CborValue::Array(vec![
                self.domain.canonical_value(),
                optional_ref(self.prior_commit_ref.as_ref()),
                self.receipt_ref.canonical_value(),
                self.claim_set_ref.canonical_value(),
                self.snapshot_catalog_ref.canonical_value(),
                CborValue::optional(self.current_release_id.map(bytes)),
                self.operation_result_ref.canonical_value(),
                self.idempotency_key_ref.canonical_value(),
                CborValue::Unsigned(1),
                CborValue::Unsigned(1),
            ]),
            references,
        )
    }
}

#[derive(Debug, Error)]
pub enum DistributionRecordErrorV1 {
    #[error("distribution row tag is outside its frozen non-zero range")]
    InvalidRowTag,
    #[error("distribution row count is empty or exceeds its frozen finite limit")]
    InvalidRowCount,
    #[error("distribution rows must be strictly ordered by tag and identity")]
    RowsNotStrictlySorted,
    #[error("distribution row identity, tag, domain, or Release binding is inconsistent")]
    RowBindingMismatch,
    #[error("distribution record commitment must be non-zero")]
    ZeroCommitment,
    #[error("installed Resource claim custody and managed-block presence disagree")]
    ManagedBlockCustodyMismatch,
    #[error("absent target preimage unexpectedly carries prior bytes or ownership")]
    InvalidAbsentPreimage,
    #[error("present target preimage requires an exact content object and digest")]
    IncompletePresentPreimage,
    #[error("Installation-domain publication requires one exact Release")]
    InstallationReleaseMissing,
    #[error("Repository-domain publication must not claim a current Release")]
    RepositoryReleasePresent,
    #[error("Resource, Bundle, census, and Release do not form one exact frozen closure")]
    InvalidReleaseClosure,
    #[error("installed Resource claim is not an exact member of its bound Release closure")]
    ClaimOutsideReleaseClosure,
    #[error("distribution locator must be non-empty bounded ASCII")]
    InvalidLocator,
    #[error(transparent)]
    Model(#[from] DistributionModelErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
}

fn validate_locator(value: &str) -> Result<(), DistributionRecordErrorV1> {
    if value.is_empty() || value.len() > 4_096 || !value.is_ascii() {
        return Err(DistributionRecordErrorV1::InvalidLocator);
    }
    Ok(())
}

fn resource_content_sha256(
    resource: &ResourceDescriptorV1,
) -> Result<CommitmentV1, DistributionRecordErrorV1> {
    let CborValue::Array(fields) = resource.value() else {
        return Err(DistributionRecordErrorV1::InvalidReleaseClosure);
    };
    let Some(CborValue::Bytes(content_sha256)) = fields.get(2) else {
        return Err(DistributionRecordErrorV1::InvalidReleaseClosure);
    };
    let content_sha256: [u8; 32] = content_sha256
        .as_slice()
        .try_into()
        .map_err(|_| DistributionRecordErrorV1::InvalidReleaseClosure)?;
    Ok(CommitmentV1::from_bytes(content_sha256))
}

fn validate_nonzero(values: &[CommitmentV1]) -> Result<(), DistributionRecordErrorV1> {
    if values.iter().any(|value| value.as_bytes() == &[0; 32]) {
        return Err(DistributionRecordErrorV1::ZeroCommitment);
    }
    Ok(())
}

fn validate_release_binding(
    domain: &DistributionDomainRefV1,
    release_id: Option<ReleaseIdV1>,
) -> Result<(), DistributionRecordErrorV1> {
    if release_id.is_some_and(|release| release.as_bytes() == &[0; 32]) {
        return Err(DistributionRecordErrorV1::ZeroCommitment);
    }
    match (domain.kind(), release_id) {
        (DistributionDomainKindV1::RepositoryDomain, None)
        | (DistributionDomainKindV1::InstallationDomain, Some(_)) => Ok(()),
        (DistributionDomainKindV1::RepositoryDomain, Some(_)) => {
            Err(DistributionRecordErrorV1::RepositoryReleasePresent)
        }
        (DistributionDomainKindV1::InstallationDomain, None) => {
            Err(DistributionRecordErrorV1::InstallationReleaseMissing)
        }
    }
}

fn require_optional_ref(
    reference: Option<&DistributionScopedObjectRefV1>,
    domain: &DistributionDomainRefV1,
    kind: DistributionRuntimeObjectKindV1,
) -> Result<(), DistributionRecordErrorV1> {
    if let Some(reference) = reference {
        require_ref(reference, domain, kind)?;
    }
    Ok(())
}

fn require_ref(
    reference: &DistributionScopedObjectRefV1,
    domain: &DistributionDomainRefV1,
    kind: DistributionRuntimeObjectKindV1,
) -> Result<(), DistributionRecordErrorV1> {
    reference.require_same_domain(domain)?;
    reference.require_kind(kind)?;
    Ok(())
}

fn optional_ref(reference: Option<&DistributionScopedObjectRefV1>) -> CborValue {
    CborValue::optional(reference.map(DistributionScopedObjectRefV1::canonical_value))
}

fn bytes(value: CommitmentV1) -> CborValue {
    CborValue::Bytes(value.as_bytes().to_vec())
}

fn store_object(
    kind: DistributionRuntimeObjectKindV1,
    value: CborValue,
    references: BTreeSet<StoreObjectIdV1>,
) -> Result<StoreObjectV1, DistributionRecordErrorV1> {
    let schema_id = kind
        .schema_id()
        .expect("invariant: C868 Store record kind has a frozen SchemaId");
    Ok(StoreObjectV1::new(
        schema_id,
        value,
        references.into_iter().collect(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commitment(byte: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([byte; 32])
    }

    fn object_id(byte: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
    }

    fn domain() -> DistributionDomainRefV1 {
        DistributionDomainRefV1::new(
            DistributionDomainKindV1::InstallationDomain,
            commitment(1),
            commitment(2),
            commitment(3),
        )
        .unwrap()
    }

    fn scoped(
        domain: &DistributionDomainRefV1,
        kind: DistributionRuntimeObjectKindV1,
        byte: u8,
    ) -> DistributionScopedObjectRefV1 {
        DistributionScopedObjectRefV1::new(domain.clone(), kind, object_id(byte)).unwrap()
    }

    fn claim(
        domain: &DistributionDomainRefV1,
        tag: u64,
        resource: u8,
        bundle: u8,
        target: u8,
    ) -> InstalledResourceClaimV1 {
        InstalledResourceClaimV1 {
            claim_tag: tag,
            domain: domain.clone(),
            canonical_target_identity_ref: scoped(
                domain,
                DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                target,
            ),
            display_locator: format!("target-{target}"),
            resolved_locator: format!("/private/tmp/stage9/target-{target}"),
            custody_class: ManagedTargetCustodyClassV1::MaestroOwnedTarget,
            resource_id: commitment(resource),
            bundle_id: commitment(bundle),
            release_id: commitment(4),
            content_sha256: commitment(resource.saturating_add(20)),
            managed_block_ref: None,
            alias_closure_ref: scoped(
                domain,
                DistributionRuntimeObjectKindV1::AliasClosure,
                target.saturating_add(40),
            ),
            verification_profile_id: commitment(5),
        }
    }

    fn release() -> ReleaseMaterializationClosureV1 {
        ReleaseMaterializationClosureV1 {
            release_id: commitment(4),
            resource_bindings: BTreeMap::from([
                (commitment(10), (commitment(30), commitment(30))),
                (commitment(11), (commitment(31), commitment(31))),
            ]),
        }
    }

    fn snapshot(
        domain: DistributionDomainRefV1,
        release_id: Option<ReleaseIdV1>,
    ) -> DistributionSnapshotV1 {
        DistributionSnapshotV1 {
            captured_commit_ref: scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionCommitRecord,
                70,
            ),
            effect_intent_ref: scoped(&domain, DistributionRuntimeObjectKindV1::EffectIntent, 71),
            claim_set_ref: scoped(
                &domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
                72,
            ),
            compatibility_closure_id: commitment(73),
            rows: vec![(
                1,
                commitment(74),
                DistributionSnapshotTargetV1 {
                    target_tag: 1,
                    domain: domain.clone(),
                    canonical_target_identity_ref: scoped(
                        &domain,
                        DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                        75,
                    ),
                    prior_claim_ref: None,
                    content_object_ref: None,
                    content_sha256: None,
                    prior_absence: true,
                    permissions_commitment_id: commitment(76),
                    owner_metadata_commitment_id: commitment(77),
                    managed_block_ref: None,
                    restore_profile_id: commitment(78),
                },
            )],
            domain,
            release_id,
        }
    }

    #[test]
    fn release_materialization_requires_the_complete_unique_resource_set() {
        let domain = domain();
        let exact = InstalledResourceClaimSetV1 {
            domain: domain.clone(),
            release_id: commitment(4),
            prior_claim_set_ref: None,
            rows: vec![
                (1, commitment(50), claim(&domain, 1, 10, 30, 60)),
                (2, commitment(51), claim(&domain, 2, 11, 31, 61)),
            ],
        };
        assert!(exact.materialize_against_release(&release()).is_ok());

        let subset = InstalledResourceClaimSetV1 {
            rows: exact.rows[..1].to_vec(),
            ..exact.clone()
        };
        assert!(matches!(
            subset.materialize_against_release(&release()),
            Err(DistributionRecordErrorV1::ClaimOutsideReleaseClosure)
        ));

        let duplicate = InstalledResourceClaimSetV1 {
            rows: vec![
                (1, commitment(50), claim(&domain, 1, 10, 30, 60)),
                (2, commitment(51), claim(&domain, 2, 10, 30, 61)),
            ],
            ..exact
        };
        assert!(matches!(
            duplicate.materialize_against_release(&release()),
            Err(DistributionRecordErrorV1::ClaimOutsideReleaseClosure)
        ));
    }

    #[test]
    fn snapshot_release_binding_is_exact_for_its_domain() {
        let repository = DistributionDomainRefV1::new(
            DistributionDomainKindV1::RepositoryDomain,
            commitment(1),
            commitment(2),
            commitment(3),
        )
        .unwrap();
        assert!(matches!(
            snapshot(repository, Some(commitment(4))).validate(),
            Err(DistributionRecordErrorV1::RepositoryReleasePresent)
        ));
        assert!(matches!(
            snapshot(domain(), Some(CommitmentV1::from_bytes([0; 32]))).validate(),
            Err(DistributionRecordErrorV1::ZeroCommitment)
        ));
        assert!(matches!(
            snapshot(domain(), None).validate(),
            Err(DistributionRecordErrorV1::InstallationReleaseMissing)
        ));
    }
}
