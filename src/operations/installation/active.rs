use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::distribution::runtime::{
    AuthorizedDistributionPlanV1, DistributionCommitRecordV1, DistributionMutationKindV1,
    DistributionPhaseAuthorizationV1, DistributionPlanV1, DistributionReceiptV1,
    DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1, DistributionTransactionErrorV1,
    DistributionTransactionPhaseV1, DistributionTransactionV1, ReleaseMaterializationProofV1,
    TargetPlanObservationV1,
};
use crate::domain::identity::{StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1};
use crate::domain::installation::{
    ActiveStoreCutoverCandidateV1, RepositoryInstallationClosureV1, UserAgentInstallationClosureV1,
};
use crate::domain::persistence::{
    AtomicGenerationPublicationV1, AtomicPublicationError, GenerationError,
    PreparedPublicationError, StoreError, StoreGenerationV1, StoreIdempotencyProbeV1,
    StoreIdempotencyV1, StoreObjectV1, StorePublicationOutcomeV1, StoreStateV1, StoreV1,
};

use super::DistributionEffectPortV1;

const DISTRIBUTION_COMMIT_IDEMPOTENCY_NAMESPACE_V1: &str = "maestro.vnext.distribution-commit.v1";

#[derive(Debug)]
pub struct ActiveDistributionTransactionV1 {
    expected_head_id: StoreHeadIdV1,
    expected_generation_id: StoreGenerationIdV1,
    transaction: DistributionTransactionV1,
}

impl ActiveDistributionTransactionV1 {
    pub const fn expected_head_id(&self) -> StoreHeadIdV1 {
        self.expected_head_id
    }

    pub const fn expected_generation_id(&self) -> StoreGenerationIdV1 {
        self.expected_generation_id
    }

    pub const fn transaction(&self) -> &DistributionTransactionV1 {
        &self.transaction
    }

    pub fn transaction_mut(&mut self) -> &mut DistributionTransactionV1 {
        &mut self.transaction
    }
}

#[derive(Clone, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "the frozen domain closure contract keeps both exact closure values by value"
)]
pub enum ActiveDomainInstallationClosureV1 {
    Repository(RepositoryInstallationClosureV1),
    UserAgent(UserAgentInstallationClosureV1),
}

impl ActiveDomainInstallationClosureV1 {
    fn domain(&self) -> &crate::domain::distribution::runtime::DistributionDomainRefV1 {
        match self {
            Self::Repository(closure) => &closure.domain,
            Self::UserAgent(closure) => &closure.domain,
        }
    }

    fn claim_set_ref(&self) -> &DistributionScopedObjectRefV1 {
        match self {
            Self::Repository(closure) => &closure.claim_set_ref,
            Self::UserAgent(closure) => &closure.claim_set_ref,
        }
    }

    fn receipt_ref(&self) -> &DistributionScopedObjectRefV1 {
        match self {
            Self::Repository(closure) => &closure.receipt_ref,
            Self::UserAgent(closure) => &closure.receipt_ref,
        }
    }

    fn snapshot_catalog_ref(&self) -> &DistributionScopedObjectRefV1 {
        match self {
            Self::Repository(closure) => &closure.snapshot_catalog_ref,
            Self::UserAgent(closure) => &closure.snapshot_catalog_ref,
        }
    }

    fn verification_result_ref(&self) -> &DistributionScopedObjectRefV1 {
        match self {
            Self::Repository(closure) => &closure.verification_result_ref,
            Self::UserAgent(closure) => &closure.verification_result_ref,
        }
    }

    fn release_id(&self) -> Option<crate::domain::distribution::ReleaseIdV1> {
        match self {
            Self::Repository(_) => None,
            Self::UserAgent(closure) => Some(closure.release_id),
        }
    }

    fn to_store_object(
        &self,
    ) -> Result<StoreObjectV1, crate::domain::installation::InstallationClosureErrorV1> {
        match self {
            Self::Repository(closure) => closure.to_store_object(),
            Self::UserAgent(closure) => closure.to_store_object(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActivePublicationObjectsV1 {
    objects: Vec<StoreObjectV1>,
    receipt_object_id: StoreObjectIdV1,
    commit_object_id: StoreObjectIdV1,
    closure_object_id: StoreObjectIdV1,
    domain: crate::domain::distribution::runtime::DistributionDomainRefV1,
    mutation_kind: DistributionMutationKindV1,
    release_id: Option<crate::domain::distribution::ReleaseIdV1>,
    release_proof: ReleaseMaterializationProofV1,
    migration: Option<(ActiveStoreCutoverCandidateV1, StoreObjectV1)>,
}

impl ActivePublicationObjectsV1 {
    pub fn new(
        mut supporting_objects: Vec<StoreObjectV1>,
        receipt: DistributionReceiptV1,
        commit: DistributionCommitRecordV1,
        closure: ActiveDomainInstallationClosureV1,
        release_proof: ReleaseMaterializationProofV1,
        migration: Option<(ActiveStoreCutoverCandidateV1, StoreObjectV1)>,
    ) -> Result<Self, InstallationOperationErrorV1> {
        if receipt.domain != commit.domain
            || &receipt.domain != closure.domain()
            || receipt.claim_set_ref.object_id() != release_proof.claim_set_object_id()
            || commit.claim_set_ref.object_id() != release_proof.claim_set_object_id()
            || commit.snapshot_catalog_ref != receipt.snapshot_catalog_ref
            || commit.operation_result_ref != receipt.committed_operation_result_ref
            || commit.current_release_id != receipt.release_id
            || closure.claim_set_ref().object_id() != release_proof.claim_set_object_id()
            || closure.snapshot_catalog_ref() != &receipt.snapshot_catalog_ref
            || closure.verification_result_ref() != &receipt.verification_result_ref
            || closure.release_id() != receipt.release_id
        {
            return Err(InstallationOperationErrorV1::InvalidPublicationClosure);
        }
        let receipt_object = receipt.to_store_object()?;
        let commit_object = commit.to_store_object()?;
        let closure_object = closure.to_store_object()?;
        if commit.receipt_ref.object_id() != receipt_object.id()
            || closure.receipt_ref().object_id() != receipt_object.id()
        {
            return Err(InstallationOperationErrorV1::InvalidPublicationClosure);
        }
        let claim_set = supporting_objects
            .iter()
            .find(|object| object.id() == release_proof.claim_set_object_id())
            .ok_or(InstallationOperationErrorV1::MissingReleaseMaterialization)?;
        if claim_set.schema_id()
            != DistributionRuntimeObjectKindV1::InstalledResourceClaimSet
                .schema_id()
                .expect("invariant: frozen claim-set schema exists")
        {
            return Err(InstallationOperationErrorV1::MissingReleaseMaterialization);
        }
        let receipt_object_id = receipt_object.id();
        let commit_object_id = commit_object.id();
        let closure_object_id = closure_object.id();
        supporting_objects.extend([receipt_object, commit_object, closure_object]);
        Ok(Self {
            objects: supporting_objects,
            receipt_object_id,
            commit_object_id,
            closure_object_id,
            domain: receipt.domain,
            mutation_kind: receipt.mutation_kind,
            release_id: receipt.release_id,
            release_proof,
            migration,
        })
    }
}

pub struct ActiveInstallationFacadeV1<'store> {
    store: &'store mut StoreV1,
}

impl<'store> ActiveInstallationFacadeV1<'store> {
    pub fn new(store: &'store mut StoreV1) -> Self {
        Self { store }
    }

    pub fn begin(
        &mut self,
        plan: DistributionPlanV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<ActiveDistributionTransactionV1, InstallationOperationErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active
            || !plan
                .domain()
                .matches_store(self.store.domain(), *generation.id().as_bytes())
        {
            return Err(InstallationOperationErrorV1::StaleDomain);
        }
        let authorized = AuthorizedDistributionPlanV1::from_current_authority(
            plan,
            phase_authorizations,
            &objects,
        )?;
        let transaction = DistributionTransactionV1::begin(authorized, observations)?;
        Ok(ActiveDistributionTransactionV1 {
            expected_head_id: head.id(),
            expected_generation_id: generation.id(),
            transaction,
        })
    }

    pub fn drive_to_verification(
        &mut self,
        active: &mut ActiveDistributionTransactionV1,
        effects: &mut impl DistributionEffectPortV1,
    ) -> Result<(), InstallationOperationErrorV1> {
        self.revalidate_effect_authority(active)?;
        let captures = active
            .transaction
            .plan()
            .targets()
            .iter()
            .map(|target| effects.compare_and_capture(target))
            .collect::<Result<Vec<_>, _>>()?;
        active.transaction.record_atomic_captures(captures)?;
        effects.persist_checkpoint(&active.transaction)?;

        let staged_digest = effects.stage_candidate(active.transaction.plan())?;
        active.transaction.record_candidate_staged(staged_digest)?;
        effects.persist_checkpoint(&active.transaction)?;

        self.revalidate_effect_authority(active)?;
        let reservations = effects.reserve_all_effects_atomically(
            active.transaction.plan(),
            active.transaction.captures(),
        )?;
        active
            .transaction
            .record_effect_reservations(reservations.into_reservations())?;
        effects.persist_checkpoint(&active.transaction)?;

        self.revalidate_effect_authority(active)?;
        let crossings = active
            .transaction
            .plan()
            .targets()
            .iter()
            .zip(active.transaction.effect_intents())
            .map(|(target, (_, effect_intent_id))| {
                effects.reconcile_and_apply(target, *effect_intent_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        active.transaction.record_effect_crossings(crossings)?;
        effects.persist_checkpoint(&active.transaction)?;
        if active.transaction.phase() != DistributionTransactionPhaseV1::EffectsCrossed {
            return Err(InstallationOperationErrorV1::RecoveryRequired);
        }

        let verification = active
            .transaction
            .plan()
            .targets()
            .iter()
            .map(|target| {
                effects
                    .verify_target(target)
                    .map(|disposition| (target.target_tag, disposition))
            })
            .collect::<Result<Vec<_>, _>>()?;
        active.transaction.record_verification(verification)?;
        effects.persist_checkpoint(&active.transaction)
    }

    pub fn restore_from_captures(
        &mut self,
        active: &mut ActiveDistributionTransactionV1,
        effects: &mut impl DistributionEffectPortV1,
    ) -> Result<(), InstallationOperationErrorV1> {
        self.revalidate_effect_authority(active)?;
        active.transaction.begin_rollback()?;
        effects.persist_checkpoint(&active.transaction)?;
        let captures = active.transaction.captures().to_vec();
        for (target, capture) in active.transaction.plan().targets().iter().zip(&captures) {
            effects.restore_exact_preimage(target, capture)?;
        }
        let restored = captures
            .iter()
            .map(|capture| capture.target_tag)
            .collect::<Vec<_>>();
        active.transaction.record_rollback_restored(&restored)?;
        effects.persist_checkpoint(&active.transaction)
    }

    pub fn publish(
        &mut self,
        active: &mut ActiveDistributionTransactionV1,
        publication_objects: ActivePublicationObjectsV1,
    ) -> Result<StorePublicationOutcomeV1, InstallationOperationErrorV1> {
        validate_publication_objects(&active.transaction, &publication_objects)?;
        let key_digest = *active
            .transaction
            .plan()
            .idempotency_key_ref()
            .object_id()
            .as_bytes();
        let meaning_digest = *active.transaction.plan().meaning_digest().as_bytes();
        let probe = StoreIdempotencyProbeV1::new(
            DISTRIBUTION_COMMIT_IDEMPOTENCY_NAMESPACE_V1,
            key_digest,
            meaning_digest,
        )?;
        let expected_head_id = active.expected_head_id;
        let expected_generation_id = active.expected_generation_id;
        let plan_domain = active.transaction.plan().domain().clone();
        let authority_receipts = active
            .transaction
            .authority_receipts()
            .cloned()
            .collect::<Vec<_>>();
        let commit_object_id = publication_objects.commit_object_id;
        let closure_object_id = publication_objects.closure_object_id;
        let association_object_id = publication_objects
            .migration
            .as_ref()
            .map(|(_, object)| object.id());
        let new_objects = normalized_new_objects(publication_objects)?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(InstallationOperationErrorV1::StaleDomain)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(InstallationOperationErrorV1::StaleDomain)?;
                if current_head.id() != expected_head_id
                    || current_generation.id() != expected_generation_id
                    || !plan_domain
                        .matches_store(view.domain(), *current_generation.id().as_bytes())
                {
                    return Err(InstallationOperationErrorV1::StaleDomain);
                }
                let active_objects = view.active_generation_objects()?;
                if !authority_receipts.iter().all(|receipt| {
                    crate::domain::authority::current_authorization_receipt_is_persisted(
                        &active_objects,
                        receipt,
                    )
                    .unwrap_or(false)
                }) {
                    return Err(InstallationOperationErrorV1::AuthorityUnavailable);
                }
                let mut all_objects = active_objects;
                all_objects.extend(new_objects.clone());
                let mut roots = current_generation.roots().to_vec();
                roots.push(commit_object_id);
                roots.push(closure_object_id);
                if let Some(association_object_id) = association_object_id {
                    roots.push(association_object_id);
                }
                roots.sort_unstable();
                roots.dedup();
                let generation = StoreGenerationV1::new(
                    view.domain().clone(),
                    current_generation
                        .ordinal()
                        .checked_add(1)
                        .ok_or(InstallationOperationErrorV1::GenerationExhausted)?,
                    Some(current_generation.id()),
                    current_generation.contract_root_id(),
                    current_generation.compatibility().clone(),
                    roots,
                )?;
                let idempotency = StoreIdempotencyV1::new(
                    DISTRIBUTION_COMMIT_IDEMPOTENCY_NAMESPACE_V1,
                    key_digest,
                    meaning_digest,
                    commit_object_id,
                )?;
                Ok(AtomicGenerationPublicationV1::new_from_object_superset(
                    generation,
                    Some(expected_head_id),
                    all_objects,
                    idempotency,
                )?)
            });
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(PreparedPublicationError::Store(error)) => return Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        let committed_ref = DistributionScopedObjectRefV1::new(
            active.transaction.plan().domain().clone(),
            DistributionRuntimeObjectKindV1::DistributionCommitRecord,
            commit_object_id,
        )?;
        active.transaction.mark_committed(&committed_ref)?;
        Ok(outcome)
    }

    fn revalidate_effect_authority(
        &mut self,
        active: &ActiveDistributionTransactionV1,
    ) -> Result<(), InstallationOperationErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active
            || head.id() != active.expected_head_id
            || generation.id() != active.expected_generation_id
            || !active
                .transaction
                .plan()
                .domain()
                .matches_store(self.store.domain(), *generation.id().as_bytes())
        {
            return Err(InstallationOperationErrorV1::StaleDomain);
        }
        active.transaction.revalidate_current_authority(&objects)?;
        Ok(())
    }
}

fn validate_publication_objects(
    transaction: &DistributionTransactionV1,
    objects: &ActivePublicationObjectsV1,
) -> Result<(), InstallationOperationErrorV1> {
    if transaction.phase() != DistributionTransactionPhaseV1::CommitPrepared {
        return Err(InstallationOperationErrorV1::TransactionNotPrepared);
    }
    if transaction.plan().domain() != &objects.domain
        || transaction.plan().mutation_kind() != objects.mutation_kind
        || transaction.plan().release_id() != objects.release_id
    {
        return Err(InstallationOperationErrorV1::InvalidPublicationClosure);
    }
    if transaction
        .prepared_receipt_ref()
        .is_none_or(|reference| reference.object_id() != objects.receipt_object_id)
        || transaction
            .prepared_commit_ref()
            .is_none_or(|reference| reference.object_id() != objects.commit_object_id)
    {
        return Err(InstallationOperationErrorV1::InvalidPublicationClosure);
    }
    let by_id = objects
        .objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<BTreeMap<_, _>>();
    if by_id.len() != objects.objects.len() {
        return Err(InstallationOperationErrorV1::DuplicatePublicationObject);
    }
    let receipt = by_id
        .get(&objects.receipt_object_id)
        .ok_or(InstallationOperationErrorV1::MissingPublicationObject)?;
    let commit = by_id
        .get(&objects.commit_object_id)
        .ok_or(InstallationOperationErrorV1::MissingPublicationObject)?;
    let closure = by_id
        .get(&objects.closure_object_id)
        .ok_or(InstallationOperationErrorV1::MissingPublicationObject)?;
    if receipt.schema_id()
        != DistributionRuntimeObjectKindV1::DistributionReceipt
            .schema_id()
            .expect("invariant: frozen Receipt schema exists")
        || commit.schema_id()
            != DistributionRuntimeObjectKindV1::DistributionCommitRecord
                .schema_id()
                .expect("invariant: frozen commit schema exists")
        || !commit.references().contains(&receipt.id())
        || !receipt
            .references()
            .contains(&objects.release_proof.claim_set_object_id())
        || !commit
            .references()
            .contains(&objects.release_proof.claim_set_object_id())
    {
        return Err(InstallationOperationErrorV1::InvalidPublicationClosure);
    }
    let expected_closure_kind = match transaction.plan().domain().kind() {
        crate::domain::distribution::runtime::DistributionDomainKindV1::RepositoryDomain => {
            DistributionRuntimeObjectKindV1::RepositoryInstallationClosure
        }
        crate::domain::distribution::runtime::DistributionDomainKindV1::InstallationDomain => {
            DistributionRuntimeObjectKindV1::UserAgentInstallationClosure
        }
    };
    if closure.schema_id()
        != expected_closure_kind
            .schema_id()
            .expect("invariant: frozen Installation closure schema exists")
        || !closure.references().contains(&receipt.id())
        || !closure
            .references()
            .contains(&objects.release_proof.claim_set_object_id())
    {
        return Err(InstallationOperationErrorV1::InvalidPublicationClosure);
    }
    if transaction.plan().domain().kind()
        == crate::domain::distribution::runtime::DistributionDomainKindV1::InstallationDomain
        && transaction.plan().release_id() != Some(objects.release_proof.release_id())
    {
        return Err(InstallationOperationErrorV1::MissingReleaseMaterialization);
    }
    match (transaction.plan().mutation_kind(), &objects.migration) {
        (DistributionMutationKindV1::Migrate, Some((candidate, association))) => {
            let release_matches = match (
                transaction.plan().release_id(),
                candidate.finality().parts().association.release(),
            ) {
                (
                    Some(release_id),
                    crate::domain::migration::ReleaseBindingV1::InstallationExact(
                        migration_release_id,
                    ),
                ) => release_id.as_bytes() == migration_release_id.as_bytes(),
                (None, crate::domain::migration::ReleaseBindingV1::RepositoryAbsent) => true,
                _ => false,
            };
            if candidate.commit_record_ref().object_id() != commit.id()
                || candidate.association_object_id() != association.id()
                || association.schema_id()
                    != crate::domain::persistence::StoreCompatibilityV1::stage0_successor()?
                        .association_schema_id()
                || association.value()
                    != &candidate.finality().parts().association.canonical_value()
                || candidate
                    .finality()
                    .parts()
                    .association
                    .material()
                    .distribution_receipt_id
                    .as_bytes()
                    != receipt.id().as_bytes()
                || !release_matches
                || !association.references().contains(&commit.id())
                || !association.references().contains(&receipt.id())
            {
                return Err(InstallationOperationErrorV1::InvalidMigrationAssociation);
            }
        }
        (DistributionMutationKindV1::Migrate, None) => {
            return Err(InstallationOperationErrorV1::MissingMigrationAssociation);
        }
        (_, Some(_)) => return Err(InstallationOperationErrorV1::UnexpectedMigrationAssociation),
        (_, None) => {}
    }
    Ok(())
}

fn normalized_new_objects(
    publication: ActivePublicationObjectsV1,
) -> Result<Vec<StoreObjectV1>, InstallationOperationErrorV1> {
    let mut objects = publication.objects;
    if let Some((_, association)) = publication.migration {
        objects.push(association);
    }
    let mut by_id = BTreeMap::new();
    for object in objects {
        match by_id.entry(object.id()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(object);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &object {
                    return Err(InstallationOperationErrorV1::DuplicatePublicationObject);
                }
            }
        }
    }
    Ok(by_id.into_values().collect())
}

#[derive(Debug, Error)]
pub enum InstallationOperationErrorV1 {
    #[error("Installation Store or Distribution domain changed since planning")]
    StaleDomain,
    #[error("current persisted Authority no longer authorizes the exact Distribution request")]
    AuthorityUnavailable,
    #[error("target effect provider failed: {0}")]
    EffectProvider(String),
    #[error("Stage-4 effect reservation batch does not cover every target exactly once")]
    IncompleteEffectReservationBatch,
    #[error("effect reconciliation requires recovery before verification or commit")]
    RecoveryRequired,
    #[error("Distribution transaction is not in commit-prepared phase")]
    TransactionNotPrepared,
    #[error("same-domain publication is missing one required object")]
    MissingPublicationObject,
    #[error("same-domain publication repeats one object identity inconsistently")]
    DuplicatePublicationObject,
    #[error("Receipt, commit, and Installation closure do not form one exact object closure")]
    InvalidPublicationClosure,
    #[error("same-domain publication lacks an exact frozen Resource/Bundle/Release claim proof")]
    MissingReleaseMaterialization,
    #[error("Migrate requires one typed Migration association in the same Store transaction")]
    MissingMigrationAssociation,
    #[error("a non-Migrate mutation must not publish a Migration association")]
    UnexpectedMigrationAssociation,
    #[error("Migration association does not bind the exact Receipt and Distribution commit")]
    InvalidMigrationAssociation,
    #[error("Store Generation ordinal is exhausted")]
    GenerationExhausted,
    #[error(transparent)]
    Transaction(#[from] DistributionTransactionErrorV1),
    #[error(transparent)]
    DistributionModel(#[from] crate::domain::distribution::runtime::DistributionModelErrorV1),
    #[error(transparent)]
    DistributionRecord(#[from] crate::domain::distribution::runtime::DistributionRecordErrorV1),
    #[error(transparent)]
    InstallationClosure(#[from] crate::domain::installation::InstallationClosureErrorV1),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    AtomicPublication(#[from] AtomicPublicationError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
}
