use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::distribution::CommitmentV1;
use crate::domain::vnext::distribution::runtime::{
    CanonicalTargetIdentityV1, DistributionDomainKindV1, DistributionRuntimeObjectKindV1,
    DistributionTransactionPhaseV1, DistributionTransactionV1, InstalledResourceClaimSetV1,
    OrdinarySnapshotCatalogV1,
};
use crate::domain::vnext::identity::StoreObjectIdV1;
use crate::domain::vnext::persistence::CollectionPlanV1;

use super::{ActiveStoreCutoverCandidateV1, InstallationCensusV1, UserAgentInstallationClosureV1};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Stage9ActiveConsumerOwnerV1 {
    PreCurrentness {
        association: [u8; 32],
        migration_consumer_set: [u8; 32],
    },
    ProtectedRetention {
        retention_pin_set: [u8; 32],
        cleanup_debt_set: [u8; 32],
    },
    PhysicalPruning {
        collection_plan: [u8; 32],
        collection_head: [u8; 32],
        retention_revision: u64,
    },
}

impl Stage9ActiveConsumerOwnerV1 {
    const fn stage_tag(&self) -> u8 {
        match self {
            Self::PreCurrentness { .. } => 1,
            Self::ProtectedRetention { .. } => 2,
            Self::PhysicalPruning { .. } => 3,
        }
    }

    const fn commitment_domain(&self) -> &'static [u8] {
        match self {
            Self::PreCurrentness { .. } => {
                b"maestro.vnext.stage9-active-pre-currentness-owner-operation.v1"
            }
            Self::ProtectedRetention { .. } => {
                b"maestro.vnext.stage9-active-protected-retention-owner-operation.v1"
            }
            Self::PhysicalPruning { .. } => {
                b"maestro.vnext.stage9-active-physical-pruning-owner-operation.v1"
            }
        }
    }

    fn append_owner_commitments(&self, commitments: &mut Vec<[u8; 32]>) {
        match self {
            Self::PreCurrentness {
                association,
                migration_consumer_set,
            } => commitments.extend([*association, *migration_consumer_set]),
            Self::ProtectedRetention {
                retention_pin_set,
                cleanup_debt_set,
            } => commitments.extend([*retention_pin_set, *cleanup_debt_set]),
            Self::PhysicalPruning {
                collection_plan,
                collection_head,
                retention_revision,
            } => {
                commitments.extend([*collection_plan, *collection_head]);
                commitments.push(scalar_commitment(
                    b"maestro.vnext.stage9-retention-revision.v1",
                    *retention_revision,
                ));
            }
        }
    }

    fn expected_head(&self) -> Option<[u8; 32]> {
        match self {
            Self::PhysicalPruning {
                collection_head, ..
            } => Some(*collection_head),
            _ => None,
        }
    }
}

/// Owner-sealed, process-local facts derived only from the Stage-9
/// Distribution transaction and its exact Installation materialization.
///
/// Persistence consumes this value to construct the frozen current-view facts.
/// Callers cannot supply a stage tag, epoch, root commitment, count, or digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct Stage9ActiveConsumerMaterializationV1 {
    owner_operation: [u8; 32],
    owner_stage_tag: u8,
    installation_id: [u8; 32],
    realm: [u8; 32],
    domain: [u8; 32],
    census_identity: [u8; 32],
    census_rows: Vec<[u8; 32]>,
    release_identity: [u8; 32],
    declared_consumer_root_manifest: [u8; 32],
    public_resource_closure: [u8; 32],
    public_bundle_closure: [u8; 32],
    public_release_closure: [u8; 32],
    alias_roots: [u8; 32],
    manager_roots: [u8; 32],
    target_roots: [u8; 32],
    claims_catalog_descriptors: [u8; 32],
    activation_carrier_identity: [u8; 32],
    activation_carrier_revision: u64,
    activation_attempt_identity: [u8; 32],
    activation_destination_seal: [u8; 32],
    expected_head: Option<[u8; 32]>,
    required_current_objects: Vec<StoreObjectIdV1>,
}

impl Stage9ActiveConsumerMaterializationV1 {
    pub(in crate::domain::vnext) fn mint_pre_currentness(
        transaction: &DistributionTransactionV1,
        census: &InstallationCensusV1,
        claims: &InstalledResourceClaimSetV1,
        catalog: &OrdinarySnapshotCatalogV1,
        closure: &UserAgentInstallationClosureV1,
        targets: &[CanonicalTargetIdentityV1],
        cutover: &ActiveStoreCutoverCandidateV1,
    ) -> Result<Self, InstallationConsumerMaterializationErrorV1> {
        let prepared_commit = transaction
            .prepared_commit_ref()
            .ok_or(InstallationConsumerMaterializationErrorV1::IncompleteOwnerTransaction)?;
        if cutover.commit_record_ref() != prepared_commit {
            return Err(InstallationConsumerMaterializationErrorV1::CrossOwnerSubstitution);
        }
        Self::mint(
            Stage9ActiveConsumerOwnerV1::PreCurrentness {
                association: *cutover.association_object_id().as_bytes(),
                migration_consumer_set: *cutover
                    .finality()
                    .parts()
                    .association
                    .material()
                    .consumer_set_id
                    .as_bytes(),
            },
            transaction,
            census,
            claims,
            catalog,
            closure,
            targets,
        )
    }

    pub(in crate::domain::vnext) fn mint_protected_retention(
        transaction: &DistributionTransactionV1,
        census: &InstallationCensusV1,
        claims: &InstalledResourceClaimSetV1,
        catalog: &OrdinarySnapshotCatalogV1,
        closure: &UserAgentInstallationClosureV1,
        targets: &[CanonicalTargetIdentityV1],
    ) -> Result<Self, InstallationConsumerMaterializationErrorV1> {
        Self::mint(
            Stage9ActiveConsumerOwnerV1::ProtectedRetention {
                retention_pin_set: *catalog.retention_pin_set_ref.object_id().as_bytes(),
                cleanup_debt_set: *catalog.cleanup_debt_set_ref.object_id().as_bytes(),
            },
            transaction,
            census,
            claims,
            catalog,
            closure,
            targets,
        )
    }

    pub(in crate::domain::vnext) fn mint_physical_pruning(
        transaction: &DistributionTransactionV1,
        census: &InstallationCensusV1,
        claims: &InstalledResourceClaimSetV1,
        catalog: &OrdinarySnapshotCatalogV1,
        closure: &UserAgentInstallationClosureV1,
        targets: &[CanonicalTargetIdentityV1],
        collection_plan: &CollectionPlanV1,
    ) -> Result<Self, InstallationConsumerMaterializationErrorV1> {
        Self::mint(
            Stage9ActiveConsumerOwnerV1::PhysicalPruning {
                collection_plan: *collection_plan.id().as_bytes(),
                collection_head: *collection_plan.head_id().as_bytes(),
                retention_revision: collection_plan.retention_revision(),
            },
            transaction,
            census,
            claims,
            catalog,
            closure,
            targets,
        )
    }

    fn mint(
        owner: Stage9ActiveConsumerOwnerV1,
        transaction: &DistributionTransactionV1,
        census: &InstallationCensusV1,
        claims: &InstalledResourceClaimSetV1,
        catalog: &OrdinarySnapshotCatalogV1,
        closure: &UserAgentInstallationClosureV1,
        targets: &[CanonicalTargetIdentityV1],
    ) -> Result<Self, InstallationConsumerMaterializationErrorV1> {
        let plan = transaction.plan();
        if plan.domain().kind() != DistributionDomainKindV1::InstallationDomain
            || !matches!(
                transaction.phase(),
                DistributionTransactionPhaseV1::CommitPrepared
                    | DistributionTransactionPhaseV1::Committed
            )
            || census.rows.is_empty()
            || targets.is_empty()
        {
            return Err(InstallationConsumerMaterializationErrorV1::IncompleteOwnerTransaction);
        }
        census.validate()?;
        claims.validate()?;
        closure.validate()?;
        let census_object = census.to_store_object()?;
        let claims_object = claims.to_store_object()?;
        let catalog_object = catalog.to_store_object()?;
        let closure_object = closure.to_store_object()?;
        let domain = plan.domain();
        let release = plan
            .release_id()
            .ok_or(InstallationConsumerMaterializationErrorV1::IncompleteOwnerTransaction)?;
        if &census.header.domain != domain
            || &claims.domain != domain
            || catalog.state.domain() != domain
            || &closure.domain != domain
            || claims.release_id != release
            || closure.release_id != release
            || closure.claim_set_ref.object_id() != claims_object.id()
            || closure.snapshot_catalog_ref.object_id() != catalog_object.id()
            || transaction
                .prepared_receipt_ref()
                .is_none_or(|receipt| receipt != &closure.receipt_ref)
        {
            return Err(InstallationConsumerMaterializationErrorV1::CrossOwnerSubstitution);
        }

        let mut target_identities = BTreeSet::new();
        let mut manager_roots = BTreeSet::new();
        let mut realms = BTreeSet::new();
        for target in targets {
            if target.domain() != domain || !target_identities.insert(target.identity()) {
                return Err(InstallationConsumerMaterializationErrorV1::TargetClosureMismatch);
            }
            manager_roots.insert(target.parts().manager_realm_id);
            realms.insert(target.parts().security_realm_id);
        }
        let planned_targets = plan
            .targets()
            .iter()
            .map(|target| target.target_identity)
            .collect::<BTreeSet<_>>();
        let claimed_targets = claims
            .rows
            .iter()
            .map(|(_, _, claim)| claim.canonical_target_identity_ref.object_id())
            .collect::<BTreeSet<_>>();
        let planned_target_refs = plan
            .targets()
            .iter()
            .map(|target| target.target_identity_ref.object_id())
            .collect::<BTreeSet<_>>();
        if target_identities != planned_targets
            || claimed_targets != planned_target_refs
            || realms.len() != 1
        {
            return Err(InstallationConsumerMaterializationErrorV1::TargetClosureMismatch);
        }

        let checkpoint = transaction.checkpoint_commitment()?;
        let mut owner_commitments = vec![
            *checkpoint.as_bytes(),
            *census_object.id().as_bytes(),
            *claims_object.id().as_bytes(),
            *catalog_object.id().as_bytes(),
            *closure_object.id().as_bytes(),
        ];
        owner.append_owner_commitments(&mut owner_commitments);
        let owner_operation = tuple_commitment(
            owner.commitment_domain(),
            &owner_commitments
                .iter()
                .map(<[u8; 32]>::as_slice)
                .collect::<Vec<_>>(),
        );
        let public_resource_closure = set_commitment(
            b"maestro.vnext.stage9-public-resource-consumer-closure.v1",
            claims.rows.iter().map(|(_, _, claim)| claim.resource_id),
        );
        let public_bundle_closure = set_commitment(
            b"maestro.vnext.stage9-public-bundle-consumer-closure.v1",
            claims.rows.iter().map(|(_, _, claim)| claim.bundle_id),
        );
        let public_release_closure = tuple_commitment(
            b"maestro.vnext.stage9-public-release-consumer-closure.v1",
            &[
                release.as_bytes(),
                claims_object.id().as_bytes(),
                closure_object.id().as_bytes(),
            ],
        );
        let alias_roots = set_commitment(
            b"maestro.vnext.stage9-alias-root-closure.v1",
            claims.rows.iter().map(|(_, _, claim)| {
                CommitmentV1::from_bytes(*claim.alias_closure_ref.object_id().as_bytes())
            }),
        );
        let manager_roots = set_commitment(
            b"maestro.vnext.stage9-manager-root-closure.v1",
            manager_roots,
        );
        let target_roots = set_commitment(
            b"maestro.vnext.stage9-target-root-closure.v1",
            target_identities,
        );
        let claims_catalog_descriptors = tuple_commitment(
            b"maestro.vnext.stage9-claims-catalog-descriptor-closure.v1",
            &[
                claims_object.id().as_bytes(),
                catalog_object.id().as_bytes(),
                closure_object.id().as_bytes(),
                closure.recovery_root_set_ref.object_id().as_bytes(),
                closure.verification_result_ref.object_id().as_bytes(),
                census.header.host_adapter_set_ref.object_id().as_bytes(),
                census.header.legacy_locator_set_ref.object_id().as_bytes(),
                census.header.observed_state_ref.object_id().as_bytes(),
                census.header.proof_profile_id.as_bytes(),
            ],
        );
        let domain_commitment = canonical_domain_commitment(domain)?;
        let activation_carrier_identity = transaction
            .prepared_commit_ref()
            .ok_or(InstallationConsumerMaterializationErrorV1::IncompleteOwnerTransaction)?
            .object_id();
        let activation_destination_seal = *checkpoint.as_bytes();
        let required_current_objects = vec![
            census_object.id(),
            claims_object.id(),
            catalog_object.id(),
            closure_object.id(),
            closure.receipt_ref.object_id(),
            activation_carrier_identity,
        ];
        let realm = realms
            .into_iter()
            .next()
            .ok_or(InstallationConsumerMaterializationErrorV1::TargetClosureMismatch)?;
        Ok(Self {
            owner_operation,
            owner_stage_tag: owner.stage_tag(),
            installation_id: *domain.domain_id().as_bytes(),
            realm: *realm.as_bytes(),
            domain: domain_commitment,
            census_identity: *census_object.id().as_bytes(),
            census_rows: census
                .rows
                .iter()
                .map(|(_, row_id, _)| *row_id.as_bytes())
                .collect(),
            release_identity: *release.as_bytes(),
            declared_consumer_root_manifest: *census
                .header
                .declared_root_set_ref
                .object_id()
                .as_bytes(),
            public_resource_closure,
            public_bundle_closure,
            public_release_closure,
            alias_roots,
            manager_roots,
            target_roots,
            claims_catalog_descriptors,
            activation_carrier_identity: *activation_carrier_identity.as_bytes(),
            activation_carrier_revision: transaction.journal_sequence(),
            activation_attempt_identity: *plan.request_id().as_bytes(),
            activation_destination_seal,
            expected_head: owner.expected_head(),
            required_current_objects,
        })
    }

    pub(in crate::domain::vnext) const fn owner_operation(&self) -> [u8; 32] {
        self.owner_operation
    }

    pub(in crate::domain::vnext) const fn owner_stage_tag(&self) -> u8 {
        self.owner_stage_tag
    }

    pub(in crate::domain::vnext) const fn installation_id(&self) -> [u8; 32] {
        self.installation_id
    }

    pub(in crate::domain::vnext) const fn realm(&self) -> [u8; 32] {
        self.realm
    }

    pub(in crate::domain::vnext) const fn domain(&self) -> [u8; 32] {
        self.domain
    }

    pub(in crate::domain::vnext) const fn census_identity(&self) -> [u8; 32] {
        self.census_identity
    }

    pub(in crate::domain::vnext) fn census_rows(&self) -> &[[u8; 32]] {
        &self.census_rows
    }

    pub(in crate::domain::vnext) const fn release_identity(&self) -> [u8; 32] {
        self.release_identity
    }

    pub(in crate::domain::vnext) const fn declared_consumer_root_manifest(&self) -> [u8; 32] {
        self.declared_consumer_root_manifest
    }

    pub(in crate::domain::vnext) const fn public_resource_closure(&self) -> [u8; 32] {
        self.public_resource_closure
    }

    pub(in crate::domain::vnext) const fn public_bundle_closure(&self) -> [u8; 32] {
        self.public_bundle_closure
    }

    pub(in crate::domain::vnext) const fn public_release_closure(&self) -> [u8; 32] {
        self.public_release_closure
    }

    pub(in crate::domain::vnext) const fn alias_roots(&self) -> [u8; 32] {
        self.alias_roots
    }

    pub(in crate::domain::vnext) const fn manager_roots(&self) -> [u8; 32] {
        self.manager_roots
    }

    pub(in crate::domain::vnext) const fn target_roots(&self) -> [u8; 32] {
        self.target_roots
    }

    pub(in crate::domain::vnext) const fn claims_catalog_descriptors(&self) -> [u8; 32] {
        self.claims_catalog_descriptors
    }

    pub(in crate::domain::vnext) const fn activation_carrier_identity(&self) -> [u8; 32] {
        self.activation_carrier_identity
    }

    pub(in crate::domain::vnext) const fn activation_carrier_revision(&self) -> u64 {
        self.activation_carrier_revision
    }

    pub(in crate::domain::vnext) const fn activation_attempt_identity(&self) -> [u8; 32] {
        self.activation_attempt_identity
    }

    pub(in crate::domain::vnext) const fn activation_destination_seal(&self) -> [u8; 32] {
        self.activation_destination_seal
    }

    pub(in crate::domain::vnext) const fn expected_head(&self) -> Option<[u8; 32]> {
        self.expected_head
    }

    pub(in crate::domain::vnext) fn required_current_objects(&self) -> &[StoreObjectIdV1] {
        &self.required_current_objects
    }
}

#[derive(Debug, Error)]
pub(in crate::domain::vnext) enum InstallationConsumerMaterializationErrorV1 {
    #[error("consumer materialization requires one commit-prepared or committed owner transaction")]
    IncompleteOwnerTransaction,
    #[error("consumer materialization combines facts from different owner transactions")]
    CrossOwnerSubstitution,
    #[error("consumer materialization target identities, claims, plan, or Realm disagree")]
    TargetClosureMismatch,
    #[error(transparent)]
    Transaction(
        #[from] crate::domain::vnext::distribution::runtime::DistributionTransactionErrorV1,
    ),
    #[error(transparent)]
    Census(#[from] super::InstallationCensusErrorV1),
    #[error(transparent)]
    Records(#[from] crate::domain::vnext::distribution::runtime::DistributionRecordErrorV1),
    #[error(transparent)]
    Closure(#[from] super::InstallationClosureErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] crate::foundation::core::deterministic_cbor::CborError),
}

fn canonical_domain_commitment(
    domain: &crate::domain::vnext::distribution::runtime::DistributionDomainRefV1,
) -> Result<[u8; 32], crate::foundation::core::deterministic_cbor::CborError> {
    Ok(
        Sha256::digest(crate::foundation::core::deterministic_cbor::encode(
            &domain.canonical_value(),
        )?)
        .into(),
    )
}

fn set_commitment(domain: &[u8], values: impl IntoIterator<Item = CommitmentV1>) -> [u8; 32] {
    let values = values.into_iter().collect::<BTreeSet<_>>();
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    digest.update((values.len() as u64).to_be_bytes());
    for value in values {
        digest.update(value.as_bytes());
    }
    digest.finalize().into()
}

fn tuple_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn scalar_commitment(domain: &[u8], value: u64) -> [u8; 32] {
    tuple_commitment(domain, &[&value.to_be_bytes()])
}
