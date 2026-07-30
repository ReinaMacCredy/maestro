use thiserror::Error;

use crate::domain::distribution::CommitmentV1;
use crate::domain::distribution::runtime::{
    DistributionDomainKindV1, DistributionDomainRefV1, DistributionRuntimeObjectKindV1,
    DistributionScopedObjectRefV1,
};
use crate::domain::identity::{StoreDomainIdV1, StoreObjectIdV1};
use crate::domain::migration::{
    ActiveStoreAtomicParticipantV1, ActiveStoreFinalityV1, CutoverCommitmentV1, CutoverDomainRefV1,
    CutoverDomainV1, MigrationCutoverContextV1, MigrationCutoverError, PreStoreAtomicParticipantV1,
    PreStoreFinalityV1, ReleaseBindingV1,
};

/// Non-persisted owner fact that derives the cutover reference and typed Store
/// destination from one exact Distribution domain. It grants neither authority
/// nor currentness and introduces no new schema identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverDomainBindingV1 {
    distribution_domain: DistributionDomainRefV1,
    cutover_domain_ref: CutoverDomainRefV1,
    destination_domain_id: StoreDomainIdV1,
}

impl CutoverDomainBindingV1 {
    pub fn new(
        distribution_domain: DistributionDomainRefV1,
        cutover_generation: u64,
        cutover_epoch: u64,
    ) -> Result<Self, InstallationCutoverErrorV1> {
        let domain_id = *distribution_domain.domain_id().as_bytes();
        let cutover_domain = match distribution_domain.kind() {
            DistributionDomainKindV1::RepositoryDomain => CutoverDomainV1::Repository,
            DistributionDomainKindV1::InstallationDomain => CutoverDomainV1::Installation,
        };
        let cutover_domain_ref = CutoverDomainRefV1::new(
            cutover_domain,
            CutoverCommitmentV1::new(domain_id)?,
            cutover_generation,
            cutover_epoch,
        )?;
        Ok(Self {
            distribution_domain,
            cutover_domain_ref,
            destination_domain_id: StoreDomainIdV1::from_digest(domain_id),
        })
    }

    pub const fn distribution_domain(&self) -> &DistributionDomainRefV1 {
        &self.distribution_domain
    }

    pub const fn cutover_domain_ref(&self) -> &CutoverDomainRefV1 {
        &self.cutover_domain_ref
    }

    pub const fn destination_domain_id(&self) -> StoreDomainIdV1 {
        self.destination_domain_id
    }

    pub fn require_same_cutover_domain_ref(
        &self,
        observed: &CutoverDomainRefV1,
    ) -> Result<(), InstallationCutoverErrorV1> {
        if observed != &self.cutover_domain_ref {
            return Err(InstallationCutoverErrorV1::CutoverDomainIdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationLocatorCandidateV1 {
    pub domain: DistributionDomainRefV1,
    pub expected_old_root_id: CommitmentV1,
    pub candidate_store_root_id: CommitmentV1,
    pub candidate_head_ref: DistributionScopedObjectRefV1,
    pub locator_cas_commitment: CommitmentV1,
}

impl InstallationLocatorCandidateV1 {
    pub fn validate(&self) -> Result<(), InstallationCutoverErrorV1> {
        if self.domain.kind() != DistributionDomainKindV1::InstallationDomain
            || [
                self.expected_old_root_id,
                self.candidate_store_root_id,
                self.locator_cas_commitment,
            ]
            .iter()
            .any(|commitment| commitment.as_bytes() == &[0; 32])
            || self.expected_old_root_id == self.candidate_store_root_id
        {
            return Err(InstallationCutoverErrorV1::InvalidLocatorCandidate);
        }
        self.candidate_head_ref.require_same_domain(&self.domain)?;
        self.candidate_head_ref
            .require_kind(DistributionRuntimeObjectKindV1::DistributionCommitRecord)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreCutoverCandidateV1 {
    finality: ActiveStoreFinalityV1,
    commit_record_ref: DistributionScopedObjectRefV1,
    association_object_id: StoreObjectIdV1,
}

impl ActiveStoreCutoverCandidateV1 {
    pub fn new(
        finality: ActiveStoreFinalityV1,
        commit_record_ref: DistributionScopedObjectRefV1,
        association_object_id: StoreObjectIdV1,
    ) -> Result<Self, InstallationCutoverErrorV1> {
        validate_active_finality(&finality, &commit_record_ref, association_object_id)?;
        Ok(Self {
            finality,
            commit_record_ref,
            association_object_id,
        })
    }

    pub const fn finality(&self) -> &ActiveStoreFinalityV1 {
        &self.finality
    }

    pub const fn commit_record_ref(&self) -> &DistributionScopedObjectRefV1 {
        &self.commit_record_ref
    }

    pub const fn association_object_id(&self) -> StoreObjectIdV1 {
        self.association_object_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreStoreCutoverCandidateV1 {
    finality: PreStoreFinalityV1,
    locator: InstallationLocatorCandidateV1,
}

impl PreStoreCutoverCandidateV1 {
    pub fn new(
        finality: PreStoreFinalityV1,
        locator: InstallationLocatorCandidateV1,
    ) -> Result<Self, InstallationCutoverErrorV1> {
        locator.validate()?;
        validate_prestore_finality(&finality, &locator)?;
        Ok(Self { finality, locator })
    }

    pub const fn finality(&self) -> &PreStoreFinalityV1 {
        &self.finality
    }

    pub const fn locator(&self) -> &InstallationLocatorCandidateV1 {
        &self.locator
    }
}

#[derive(Debug, Error)]
pub enum InstallationCutoverErrorV1 {
    #[error("cutover domain reference does not equal the Stage-9 owner domain binding")]
    CutoverDomainIdentityMismatch,
    #[error("Installation locator candidate is not a non-zero expected-old CAS in one domain")]
    InvalidLocatorCandidate,
    #[error("active-store Migration association is not joined to the exact Distribution commit")]
    ActiveAssociationMismatch,
    #[error("pre-store Migration association, candidate seal, and protected locator CAS disagree")]
    PreStoreAssociationMismatch,
    #[error(transparent)]
    DistributionModel(#[from] crate::domain::distribution::runtime::DistributionModelErrorV1),
    #[error(transparent)]
    MigrationCutover(#[from] MigrationCutoverError),
}

fn validate_active_finality(
    finality: &ActiveStoreFinalityV1,
    commit_ref: &DistributionScopedObjectRefV1,
    association_object_id: StoreObjectIdV1,
) -> Result<(), InstallationCutoverErrorV1> {
    commit_ref.require_kind(DistributionRuntimeObjectKindV1::DistributionCommitRecord)?;
    let parts = finality.parts();
    if parts.association.domain_ref().domain()
        != match commit_ref.domain().kind() {
            DistributionDomainKindV1::RepositoryDomain => CutoverDomainV1::Repository,
            DistributionDomainKindV1::InstallationDomain => CutoverDomainV1::Installation,
        }
    {
        return Err(InstallationCutoverErrorV1::ActiveAssociationMismatch);
    }
    let MigrationCutoverContextV1::ActiveStore {
        distribution_commit_record_id,
    } = parts.association.context()
    else {
        return Err(InstallationCutoverErrorV1::ActiveAssociationMismatch);
    };
    if association_object_id.as_bytes() == &[0; 32]
        || distribution_commit_record_id.as_bytes() != commit_ref.object_id().as_bytes()
        || !parts.atomic_participants.iter().any(|participant| {
            matches!(participant, ActiveStoreAtomicParticipantV1::Association(id)
                if id == &parts.association.material().association_id)
        })
        || !parts.atomic_participants.iter().any(|participant| {
            matches!(participant, ActiveStoreAtomicParticipantV1::OwningHead(head)
                if head.distribution_commit_record_id.as_bytes()
                    == commit_ref.object_id().as_bytes())
        })
    {
        return Err(InstallationCutoverErrorV1::ActiveAssociationMismatch);
    }
    Ok(())
}

fn validate_prestore_finality(
    finality: &PreStoreFinalityV1,
    locator: &InstallationLocatorCandidateV1,
) -> Result<(), InstallationCutoverErrorV1> {
    let parts = finality.parts();
    if parts.association.domain_ref().domain() != CutoverDomainV1::Installation
        || !matches!(
            parts.association.release(),
            ReleaseBindingV1::InstallationExact(_)
        )
    {
        return Err(InstallationCutoverErrorV1::PreStoreAssociationMismatch);
    }
    let MigrationCutoverContextV1::PreStore {
        candidate_seal_id,
        expected_old_root_id,
        ..
    } = parts.association.context()
    else {
        return Err(InstallationCutoverErrorV1::PreStoreAssociationMismatch);
    };
    let material = parts.association.material();
    if material.candidate_store_root_id.as_bytes() != locator.candidate_store_root_id.as_bytes()
        || expected_old_root_id.as_bytes() != locator.expected_old_root_id.as_bytes()
        || !parts.atomic_participants.iter().any(|participant| {
            matches!(participant, PreStoreAtomicParticipantV1::CandidateSeal(seal)
                if seal.candidate_seal_id == *candidate_seal_id
                    && seal.candidate_store_root_id.as_bytes()
                        == locator.candidate_store_root_id.as_bytes())
        })
        || !parts.atomic_participants.iter().any(|participant| {
            matches!(participant, PreStoreAtomicParticipantV1::ProtectedExpectedOldCas(cas)
                if cas.expected_old_root_id.as_bytes()
                    == locator.expected_old_root_id.as_bytes()
                    && cas.candidate_store_root_id.as_bytes()
                        == locator.candidate_store_root_id.as_bytes())
        })
    {
        return Err(InstallationCutoverErrorV1::PreStoreAssociationMismatch);
    }
    Ok(())
}
