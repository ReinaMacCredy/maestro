use std::marker::PhantomData;
use std::rc::Rc;

#[cfg(test)]
use std::cell::Cell;
use thiserror::Error;

use super::{EffectIntentHomeKindV1, WithdrawalCatalogCellV1, WithdrawalRouteBindingV1};
use crate::domain::vnext::migration::{
    ActiveStoreFinalityV1, MigrationCutoverAssociationV1, MigrationCutoverContextV1,
    PreStoreFinalityV1, ReleaseBindingV1,
};

#[expect(
    clippy::enum_variant_names,
    reason = "the locked H3 home spellings are exactly ActiveStore, PreStore, and NoStore"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext::execution) enum H3WithdrawalPublicationSourceV1 {
    ActiveStore {
        store_identity: [u8; 32],
        domain_identity: [u8; 32],
        current_snapshot: [u8; 32],
        predecessor_head: [u8; 32],
        cancelled_head: [u8; 32],
        writer_term: [u8; 32],
        winning_expected_old_transition: [u8; 32],
        withdrawal_publication: [u8; 32],
        withdrawal_root: [u8; 32],
        association_input_context: [u8; 32],
        finality_context: [u8; 32],
    },
    PreStore {
        pre_store_home: [u8; 32],
        protected_realm: [u8; 32],
        subject: [u8; 32],
        source_candidate_carrier: [u8; 32],
        ceremony_spec: [u8; 32],
        attempt: [u8; 32],
        expected_old_source: [u8; 32],
        withdrawal_carrier: [u8; 32],
        withdrawal_cas: [u8; 32],
        withdrawal_publication_root: [u8; 32],
    },
    NoStore {
        no_store_home: [u8; 32],
        ceremony_spec: [u8; 32],
        attempt: [u8; 32],
        occurrence: [u8; 32],
        carrier: [u8; 32],
    },
}

impl H3WithdrawalPublicationSourceV1 {
    const fn home(self) -> EffectIntentHomeKindV1 {
        match self {
            Self::ActiveStore { .. } => EffectIntentHomeKindV1::ActiveStore,
            Self::PreStore { .. } => EffectIntentHomeKindV1::PreStoreCeremony,
            Self::NoStore { .. } => EffectIntentHomeKindV1::NoStoreCeremony,
        }
    }

    fn commitments(self) -> Vec<[u8; 32]> {
        match self {
            Self::ActiveStore {
                store_identity,
                domain_identity,
                current_snapshot,
                predecessor_head,
                cancelled_head,
                writer_term,
                winning_expected_old_transition,
                withdrawal_publication,
                withdrawal_root,
                association_input_context,
                finality_context,
            } => vec![
                store_identity,
                domain_identity,
                current_snapshot,
                predecessor_head,
                cancelled_head,
                writer_term,
                winning_expected_old_transition,
                withdrawal_publication,
                withdrawal_root,
                association_input_context,
                finality_context,
            ],
            Self::PreStore {
                pre_store_home,
                protected_realm,
                subject,
                source_candidate_carrier,
                ceremony_spec,
                attempt,
                expected_old_source,
                withdrawal_carrier,
                withdrawal_cas,
                withdrawal_publication_root,
            } => vec![
                pre_store_home,
                protected_realm,
                subject,
                source_candidate_carrier,
                ceremony_spec,
                attempt,
                expected_old_source,
                withdrawal_carrier,
                withdrawal_cas,
                withdrawal_publication_root,
            ],
            Self::NoStore {
                no_store_home,
                ceremony_spec,
                attempt,
                occurrence,
                carrier,
            } => vec![no_store_home, ceremony_spec, attempt, occurrence, carrier],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext::execution) enum H3WithdrawalPublicationOriginV1 {
    Action {
        leaf_tag: u64,
        action_spec: [u8; 32],
        request: [u8; 32],
        route: [u8; 32],
    },
    Ceremony {
        ceremony_tag: u8,
        ceremony_spec: [u8; 32],
        attempt: [u8; 32],
        withdraw_mode: [u8; 32],
        route: [u8; 32],
    },
}

impl H3WithdrawalPublicationOriginV1 {
    const fn route(self) -> WithdrawalRouteBindingV1 {
        match self {
            Self::Action { leaf_tag, .. } => WithdrawalRouteBindingV1::Action {
                action_tag: leaf_tag,
            },
            Self::Ceremony { ceremony_tag, .. } => {
                WithdrawalRouteBindingV1::Ceremony { ceremony_tag }
            }
        }
    }

    fn commitments(self) -> Vec<[u8; 32]> {
        match self {
            Self::Action {
                action_spec,
                request,
                route,
                ..
            } => vec![action_spec, request, route],
            Self::Ceremony {
                ceremony_spec,
                attempt,
                withdraw_mode,
                route,
                ..
            } => vec![ceremony_spec, attempt, withdraw_mode, route],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext::execution) struct H3WithdrawalPublicationFactsV1 {
    catalog_cell: WithdrawalCatalogCellV1,
    source: H3WithdrawalPublicationSourceV1,
    origin: H3WithdrawalPublicationOriginV1,
    intent_identity: [u8; 32],
    intent_revision: [u8; 32],
    intent_origin: [u8; 32],
    intent_home: [u8; 32],
    originating_result: [u8; 32],
    withdrawal_result: [u8; 32],
    withdrawal_request: [u8; 32],
    withdrawal_publication: [u8; 32],
    authority_basis: [u8; 32],
    authority_snapshot: [u8; 32],
    authority_fence: [u8; 32],
    authority_state_token: [u8; 32],
    authority_currentness: [u8; 32],
    revocation: [u8; 32],
    trusted_time: [u8; 32],
    normalized_witness: [u8; 32],
    debit_map: [u8; 32],
    committed_withdrawal_debit: [u8; 32],
    authorization_receipt: [u8; 32],
    occurrence: [u8; 32],
    carrier: [u8; 32],
    incarnation: [u8; 32],
    no_live_attempt: [u8; 32],
    no_live_fence: [u8; 32],
    no_live_seal: [u8; 32],
    no_live_release: [u8; 32],
    closed_runs: [u8; 32],
    effect_slot_accounting: [u8; 32],
    dispatch_budget_accounting: [u8; 32],
    material_currentness: [u8; 32],
    credential_currentness: [u8; 32],
    use_fence_currentness: [u8; 32],
    source_inventory: [u8; 32],
    source_inventory_rows: [u8; 32],
    declared_target_closure: [u8; 32],
    cancelled_target: [u8; 32],
    display_identity: [u8; 32],
    resolved_identity: [u8; 32],
    authorizing_branch: [u8; 32],
    effect_lineage: [u8; 32],
    custody_transition: [u8; 32],
    quarantine_roots: [u8; 32],
    protected_roots: [u8; 32],
    consumer_gate: [u8; 32],
    claims_catalog_verification: [u8; 32],
    optional_release: Option<[u8; 32]>,
    result: [u8; 32],
    idempotency_meaning: [u8; 32],
}

pub(in crate::domain::vnext::execution) mod native_publication_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext::execution) trait NativeH3WithdrawalPublicationPortV1:
    native_publication_sealed::Sealed
{
    fn publication_identity(&self) -> [u8; 32];
    fn catalog_cell(&self) -> WithdrawalCatalogCellV1;
    fn source(&self) -> H3WithdrawalPublicationSourceV1;
    fn is_unused(&self) -> bool;
    fn consume_once(&self) -> Result<(), H3WithdrawalPublicationErrorV1>;
}

pub(in crate::domain::vnext) struct VerifiedH3WithdrawalPublicationUseV1<'tx> {
    facts: H3WithdrawalPublicationFactsV1,
    native_publication: &'tx dyn NativeH3WithdrawalPublicationPortV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct H3WithdrawalPublicationCommitV1 {
    facts: H3WithdrawalPublicationFactsV1,
}

pub(in crate::domain::vnext) struct ConsumedH3WithdrawalPublicationV1<'tx> {
    facts: H3WithdrawalPublicationFactsV1,
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx> VerifiedH3WithdrawalPublicationUseV1<'tx> {
    fn consume(
        self,
        commit: H3WithdrawalPublicationCommitV1,
    ) -> Result<ConsumedH3WithdrawalPublicationV1<'tx>, H3WithdrawalPublicationErrorV1> {
        if !self.native_publication.is_unused() || self.facts != commit.facts {
            return Err(H3WithdrawalPublicationErrorV1::BindingMismatch);
        }
        self.native_publication.consume_once()?;
        let consumed = ConsumedH3WithdrawalPublicationV1 {
            facts: self.facts,
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        };
        if consumed.facts != commit.facts {
            return Err(H3WithdrawalPublicationErrorV1::BindingMismatch);
        }
        Ok(consumed)
    }

    pub(in crate::domain::vnext) fn consume_for_migration(
        self,
        association: &MigrationCutoverAssociationV1,
        finality: H3MigrationFinalityV1<'_>,
    ) -> Result<ConsumedH3WithdrawalPublicationV1<'tx>, H3WithdrawalPublicationErrorV1> {
        validate_migration_association(self.facts, association, finality)?;
        let commit = H3WithdrawalPublicationCommitV1 { facts: self.facts };
        self.consume(commit)
    }
}

pub(in crate::domain::vnext) enum H3MigrationFinalityV1<'a> {
    ActiveStore(&'a ActiveStoreFinalityV1),
    PreStore(&'a PreStoreFinalityV1),
}

pub(in crate::domain::vnext::execution) fn verify_h3_withdrawal_publication_use<'tx>(
    facts: H3WithdrawalPublicationFactsV1,
    native_publication: &'tx dyn NativeH3WithdrawalPublicationPortV1,
) -> Result<VerifiedH3WithdrawalPublicationUseV1<'tx>, H3WithdrawalPublicationErrorV1> {
    let mut required = facts.source.commitments();
    required.extend(facts.origin.commitments());
    required.extend([
        facts.intent_identity,
        facts.intent_revision,
        facts.intent_origin,
        facts.intent_home,
        facts.originating_result,
        facts.withdrawal_result,
        facts.withdrawal_request,
        facts.withdrawal_publication,
        facts.authority_basis,
        facts.authority_snapshot,
        facts.authority_fence,
        facts.authority_state_token,
        facts.authority_currentness,
        facts.revocation,
        facts.trusted_time,
        facts.normalized_witness,
        facts.debit_map,
        facts.committed_withdrawal_debit,
        facts.authorization_receipt,
        facts.occurrence,
        facts.carrier,
        facts.incarnation,
        facts.no_live_attempt,
        facts.no_live_fence,
        facts.no_live_seal,
        facts.no_live_release,
        facts.closed_runs,
        facts.effect_slot_accounting,
        facts.dispatch_budget_accounting,
        facts.material_currentness,
        facts.credential_currentness,
        facts.use_fence_currentness,
        facts.source_inventory,
        facts.source_inventory_rows,
        facts.declared_target_closure,
        facts.cancelled_target,
        facts.display_identity,
        facts.resolved_identity,
        facts.authorizing_branch,
        facts.effect_lineage,
        facts.custody_transition,
        facts.quarantine_roots,
        facts.protected_roots,
        facts.consumer_gate,
        facts.claims_catalog_verification,
        facts.result,
        facts.idempotency_meaning,
    ]);
    if required.contains(&[0; 32])
        || facts.optional_release == Some([0; 32])
        || !native_publication.is_unused()
        || facts.catalog_cell != native_publication.catalog_cell()
        || facts.withdrawal_publication != native_publication.publication_identity()
        || facts.source != native_publication.source()
        || facts.catalog_cell.home() != facts.source.home()
        || facts.catalog_cell.route() != facts.origin.route()
    {
        return Err(H3WithdrawalPublicationErrorV1::InvalidCarrier);
    }
    validate_source_origin_equality(facts)?;
    match (facts.source.home(), facts.origin.route()) {
        (EffectIntentHomeKindV1::ActiveStore, WithdrawalRouteBindingV1::Ceremony { .. })
        | (
            EffectIntentHomeKindV1::PreStoreCeremony | EffectIntentHomeKindV1::NoStoreCeremony,
            WithdrawalRouteBindingV1::Action { .. },
        ) => return Err(H3WithdrawalPublicationErrorV1::CausalBranchMismatch),
        _ => {}
    }
    Ok(VerifiedH3WithdrawalPublicationUseV1 {
        facts,
        native_publication,
        _not_send_or_sync: PhantomData,
    })
}

fn validate_source_origin_equality(
    facts: H3WithdrawalPublicationFactsV1,
) -> Result<(), H3WithdrawalPublicationErrorV1> {
    if facts.intent_home != canonical_home_commitment(facts.source.home()) {
        return Err(H3WithdrawalPublicationErrorV1::CausalBranchMismatch);
    }
    match (facts.source, facts.origin) {
        (
            H3WithdrawalPublicationSourceV1::ActiveStore {
                withdrawal_publication,
                ..
            },
            H3WithdrawalPublicationOriginV1::Action { .. },
        ) if withdrawal_publication == facts.withdrawal_publication => Ok(()),
        (
            H3WithdrawalPublicationSourceV1::PreStore {
                ceremony_spec,
                attempt,
                withdrawal_carrier,
                ..
            },
            H3WithdrawalPublicationOriginV1::Ceremony {
                ceremony_spec: origin_spec,
                attempt: origin_attempt,
                ..
            },
        ) if ceremony_spec == origin_spec
            && attempt == origin_attempt
            && withdrawal_carrier == facts.carrier =>
        {
            Ok(())
        }
        (
            H3WithdrawalPublicationSourceV1::NoStore {
                ceremony_spec,
                attempt,
                occurrence,
                carrier,
                ..
            },
            H3WithdrawalPublicationOriginV1::Ceremony {
                ceremony_spec: origin_spec,
                attempt: origin_attempt,
                ..
            },
        ) if ceremony_spec == origin_spec
            && attempt == origin_attempt
            && occurrence == facts.occurrence
            && carrier == facts.carrier =>
        {
            Ok(())
        }
        _ => Err(H3WithdrawalPublicationErrorV1::CausalBranchMismatch),
    }
}

fn validate_migration_association(
    facts: H3WithdrawalPublicationFactsV1,
    association: &MigrationCutoverAssociationV1,
    finality: H3MigrationFinalityV1<'_>,
) -> Result<(), H3WithdrawalPublicationErrorV1> {
    let material = association.material();
    let exact_material = material.inventory_id.as_bytes() == &facts.source_inventory
        && material.target_set_id.as_bytes() == &facts.declared_target_closure
        && material.quarantine_set_id.as_bytes() == &facts.quarantine_roots
        && material.consumer_set_id.as_bytes() == &facts.consumer_gate
        && material.distribution_receipt_id.as_bytes() == &facts.authorization_receipt
        && material.schema_read_write_set_id.as_bytes() == &facts.claims_catalog_verification;
    let exact_release = match (association.release(), facts.optional_release) {
        (ReleaseBindingV1::RepositoryAbsent, None) => true,
        (ReleaseBindingV1::InstallationExact(release), Some(expected)) => {
            release.as_bytes() == &expected
        }
        _ => false,
    };
    let exact_context = match (facts.source, association.context(), finality) {
        (
            H3WithdrawalPublicationSourceV1::ActiveStore { .. },
            MigrationCutoverContextV1::ActiveStore { .. },
            H3MigrationFinalityV1::ActiveStore(finality),
        ) => &finality.parts().association == association,
        (
            H3WithdrawalPublicationSourceV1::PreStore {
                attempt,
                expected_old_source,
                ..
            },
            MigrationCutoverContextV1::PreStore {
                sealed_ceremony_attempt_id,
                expected_old_root_id,
                ..
            },
            H3MigrationFinalityV1::PreStore(finality),
        ) => {
            &finality.parts().association == association
                && sealed_ceremony_attempt_id.as_bytes() == &attempt
                && expected_old_root_id.as_bytes() == &expected_old_source
        }
        (H3WithdrawalPublicationSourceV1::NoStore { .. }, _, _) => {
            return Err(H3WithdrawalPublicationErrorV1::NonPromoting);
        }
        _ => false,
    };
    if !exact_material || !exact_release || !exact_context {
        return Err(H3WithdrawalPublicationErrorV1::BindingMismatch);
    }
    Ok(())
}

fn canonical_home_commitment(home: EffectIntentHomeKindV1) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let literal = match home {
        EffectIntentHomeKindV1::ActiveStore => b"ActiveStore".as_slice(),
        EffectIntentHomeKindV1::PreStoreCeremony => b"PreStore".as_slice(),
        EffectIntentHomeKindV1::NoStoreCeremony => b"NoStore".as_slice(),
    };
    Sha256::digest(literal).into()
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum H3WithdrawalPublicationErrorV1 {
    #[error("H3 withdrawal publication carrier is invalid")]
    InvalidCarrier,
    #[error("H3 source and Action/Ceremony causal branch disagree")]
    CausalBranchMismatch,
    #[error("H3 withdrawal publication does not match the exact finality use")]
    BindingMismatch,
    #[error("H3 withdrawal publication is non-promoting in the selected migration context")]
    NonPromoting,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::migration::{
        ActiveStoreAtomicParticipantV1, ActiveStoreFinalityPartsV1, ActiveStoreOwningHeadV1,
        ActiveStorePreconditionV1, CutoverCommitmentV1, CutoverDomainRefV1, CutoverDomainV1,
        MigrationCutoverMaterialV1, PreStoreAtomicParticipantV1, PreStoreCandidateSealV1,
        PreStoreFinalityPartsV1, PreStorePreconditionV1, ProtectedExpectedOldCasV1,
    };

    struct TestNativePublicationV1 {
        consumed: Cell<bool>,
        publication_identity: [u8; 32],
        catalog_cell: WithdrawalCatalogCellV1,
        source: H3WithdrawalPublicationSourceV1,
    }

    impl native_publication_sealed::Sealed for TestNativePublicationV1 {}

    impl NativeH3WithdrawalPublicationPortV1 for TestNativePublicationV1 {
        fn publication_identity(&self) -> [u8; 32] {
            self.publication_identity
        }

        fn catalog_cell(&self) -> WithdrawalCatalogCellV1 {
            self.catalog_cell
        }

        fn source(&self) -> H3WithdrawalPublicationSourceV1 {
            self.source
        }

        fn is_unused(&self) -> bool {
            !self.consumed.get()
        }

        fn consume_once(&self) -> Result<(), H3WithdrawalPublicationErrorV1> {
            if self.consumed.replace(true) {
                return Err(H3WithdrawalPublicationErrorV1::BindingMismatch);
            }
            Ok(())
        }
    }

    fn test_native(facts: H3WithdrawalPublicationFactsV1) -> TestNativePublicationV1 {
        TestNativePublicationV1 {
            consumed: Cell::new(false),
            publication_identity: facts.withdrawal_publication,
            catalog_cell: facts.catalog_cell,
            source: facts.source,
        }
    }

    fn commitment(bytes: [u8; 32]) -> CutoverCommitmentV1 {
        CutoverCommitmentV1::new(bytes).unwrap()
    }

    #[expect(
        clippy::enum_variant_names,
        reason = "the conformance fixture mirrors the locked ActiveStore, PreStore, and NoStore homes"
    )]
    enum TestMigrationFinalityV1 {
        ActiveStore(ActiveStoreFinalityV1),
        PreStore(PreStoreFinalityV1),
        NoStore,
    }

    fn migration_finality(
        facts: H3WithdrawalPublicationFactsV1,
    ) -> (MigrationCutoverAssociationV1, TestMigrationFinalityV1) {
        let domain_ref =
            CutoverDomainRefV1::new(CutoverDomainV1::Installation, commitment([70; 32]), 1, 1)
                .unwrap();
        let release =
            ReleaseBindingV1::InstallationExact(commitment(facts.optional_release.unwrap()));
        let material = MigrationCutoverMaterialV1 {
            association_id: commitment([71; 32]),
            inventory_id: commitment(facts.source_inventory),
            target_set_id: commitment(facts.declared_target_closure),
            quarantine_set_id: commitment(facts.quarantine_roots),
            consumer_set_id: commitment(facts.consumer_gate),
            distribution_receipt_id: commitment(facts.authorization_receipt),
            candidate_store_root_id: commitment([72; 32]),
            schema_read_write_set_id: commitment(facts.claims_catalog_verification),
            writer_protocol_epoch_id: commitment([73; 32]),
            migration_epoch_id: commitment([74; 32]),
        };
        match facts.source {
            H3WithdrawalPublicationSourceV1::ActiveStore { .. } => {
                let commit_record = commitment([75; 32]);
                let association = MigrationCutoverAssociationV1::new(
                    domain_ref.clone(),
                    release.clone(),
                    MigrationCutoverContextV1::ActiveStore {
                        distribution_commit_record_id: commit_record,
                    },
                    material.clone(),
                )
                .unwrap();
                let finality = ActiveStoreFinalityV1::new(ActiveStoreFinalityPartsV1 {
                    association: association.clone(),
                    ordered_preconditions: vec![
                        ActiveStorePreconditionV1::DistributionReceipt(
                            material.distribution_receipt_id,
                        ),
                        ActiveStorePreconditionV1::DistributionCommitRecord {
                            commit_record_id: commit_record,
                            receipt_id: material.distribution_receipt_id,
                        },
                    ],
                    atomic_participants: vec![
                        ActiveStoreAtomicParticipantV1::Association(material.association_id),
                        ActiveStoreAtomicParticipantV1::OwningHead(ActiveStoreOwningHeadV1 {
                            association_id: material.association_id,
                            distribution_commit_record_id: commit_record,
                            distribution_receipt_id: material.distribution_receipt_id,
                            domain_ref,
                            release,
                            candidate_store_root_id: material.candidate_store_root_id,
                        }),
                    ],
                })
                .unwrap();
                (association, TestMigrationFinalityV1::ActiveStore(finality))
            }
            H3WithdrawalPublicationSourceV1::PreStore {
                attempt,
                expected_old_source,
                ..
            } => {
                let attempt = commitment(attempt);
                let candidate_seal = commitment([75; 32]);
                let expected_old = commitment(expected_old_source);
                let association = MigrationCutoverAssociationV1::new(
                    domain_ref.clone(),
                    release.clone(),
                    MigrationCutoverContextV1::PreStore {
                        sealed_ceremony_attempt_id: attempt,
                        candidate_seal_id: candidate_seal,
                        expected_old_root_id: expected_old,
                    },
                    material.clone(),
                )
                .unwrap();
                let finality = PreStoreFinalityV1::new(PreStoreFinalityPartsV1 {
                    association: association.clone(),
                    ordered_preconditions: vec![PreStorePreconditionV1::SealedCeremonyAttempt(
                        attempt,
                    )],
                    atomic_participants: vec![
                        PreStoreAtomicParticipantV1::Association(material.association_id),
                        PreStoreAtomicParticipantV1::CandidateSeal(PreStoreCandidateSealV1 {
                            association_id: material.association_id,
                            candidate_seal_id: candidate_seal,
                            sealed_ceremony_attempt_id: attempt,
                            domain_ref: domain_ref.clone(),
                            release: release.clone(),
                            candidate_store_root_id: material.candidate_store_root_id,
                        }),
                        PreStoreAtomicParticipantV1::ProtectedExpectedOldCas(
                            ProtectedExpectedOldCasV1 {
                                association_id: material.association_id,
                                expected_old_root_id: expected_old,
                                candidate_store_root_id: material.candidate_store_root_id,
                            },
                        ),
                    ],
                })
                .unwrap();
                (association, TestMigrationFinalityV1::PreStore(finality))
            }
            H3WithdrawalPublicationSourceV1::NoStore { .. } => {
                let association = MigrationCutoverAssociationV1::new(
                    domain_ref,
                    release,
                    MigrationCutoverContextV1::ActiveStore {
                        distribution_commit_record_id: commitment([75; 32]),
                    },
                    material,
                )
                .unwrap();
                (association, TestMigrationFinalityV1::NoStore)
            }
        }
    }

    fn action_origin() -> H3WithdrawalPublicationOriginV1 {
        H3WithdrawalPublicationOriginV1::Action {
            leaf_tag: 36,
            action_spec: [61; 32],
            request: [62; 32],
            route: [63; 32],
        }
    }

    fn ceremony_origin() -> H3WithdrawalPublicationOriginV1 {
        H3WithdrawalPublicationOriginV1::Ceremony {
            ceremony_tag: 9,
            ceremony_spec: [64; 32],
            attempt: [65; 32],
            withdraw_mode: [66; 32],
            route: [67; 32],
        }
    }

    fn facts(source: H3WithdrawalPublicationSourceV1) -> H3WithdrawalPublicationFactsV1 {
        let origin = match source {
            H3WithdrawalPublicationSourceV1::ActiveStore { .. } => action_origin(),
            H3WithdrawalPublicationSourceV1::PreStore {
                ceremony_spec,
                attempt,
                ..
            } => H3WithdrawalPublicationOriginV1::Ceremony {
                ceremony_tag: 9,
                ceremony_spec,
                attempt,
                withdraw_mode: [66; 32],
                route: [67; 32],
            },
            H3WithdrawalPublicationSourceV1::NoStore {
                ceremony_spec,
                attempt,
                ..
            } => H3WithdrawalPublicationOriginV1::Ceremony {
                ceremony_tag: 1,
                ceremony_spec,
                attempt,
                withdraw_mode: [66; 32],
                route: [67; 32],
            },
        };
        let catalog_cell = crate::domain::vnext::execution::withdrawal_catalog_cells_v1()
            .into_iter()
            .find(|cell| cell.home() == source.home() && cell.route() == origin.route())
            .unwrap();
        H3WithdrawalPublicationFactsV1 {
            catalog_cell,
            source,
            origin,
            intent_identity: [1; 32],
            intent_revision: [2; 32],
            intent_origin: [3; 32],
            intent_home: canonical_home_commitment(source.home()),
            originating_result: [5; 32],
            withdrawal_result: [6; 32],
            withdrawal_request: [7; 32],
            withdrawal_publication: match source {
                H3WithdrawalPublicationSourceV1::ActiveStore {
                    withdrawal_publication,
                    ..
                } => withdrawal_publication,
                _ => [8; 32],
            },
            authority_basis: [9; 32],
            authority_snapshot: [10; 32],
            authority_fence: [11; 32],
            authority_state_token: [12; 32],
            authority_currentness: [13; 32],
            revocation: [14; 32],
            trusted_time: [15; 32],
            normalized_witness: [16; 32],
            debit_map: [17; 32],
            committed_withdrawal_debit: [18; 32],
            authorization_receipt: [19; 32],
            occurrence: match source {
                H3WithdrawalPublicationSourceV1::NoStore { occurrence, .. } => occurrence,
                _ => [20; 32],
            },
            carrier: match source {
                H3WithdrawalPublicationSourceV1::PreStore {
                    withdrawal_carrier, ..
                } => withdrawal_carrier,
                H3WithdrawalPublicationSourceV1::NoStore { carrier, .. } => carrier,
                _ => [21; 32],
            },
            incarnation: [22; 32],
            no_live_attempt: [23; 32],
            no_live_fence: [24; 32],
            no_live_seal: [25; 32],
            no_live_release: [26; 32],
            closed_runs: [27; 32],
            effect_slot_accounting: [28; 32],
            dispatch_budget_accounting: [29; 32],
            material_currentness: [30; 32],
            credential_currentness: [31; 32],
            use_fence_currentness: [32; 32],
            source_inventory: [33; 32],
            source_inventory_rows: [34; 32],
            declared_target_closure: [35; 32],
            cancelled_target: [36; 32],
            display_identity: [37; 32],
            resolved_identity: [38; 32],
            authorizing_branch: [39; 32],
            effect_lineage: [40; 32],
            custody_transition: [41; 32],
            quarantine_roots: [42; 32],
            protected_roots: [43; 32],
            consumer_gate: [44; 32],
            claims_catalog_verification: [45; 32],
            optional_release: Some([46; 32]),
            result: [47; 32],
            idempotency_meaning: [48; 32],
        }
    }

    fn sources() -> [H3WithdrawalPublicationSourceV1; 3] {
        [
            H3WithdrawalPublicationSourceV1::ActiveStore {
                store_identity: [50; 32],
                domain_identity: [51; 32],
                current_snapshot: [52; 32],
                predecessor_head: [53; 32],
                cancelled_head: [54; 32],
                writer_term: [55; 32],
                winning_expected_old_transition: [56; 32],
                withdrawal_publication: [57; 32],
                withdrawal_root: [58; 32],
                association_input_context: [59; 32],
                finality_context: [60; 32],
            },
            H3WithdrawalPublicationSourceV1::PreStore {
                pre_store_home: [50; 32],
                protected_realm: [51; 32],
                subject: [52; 32],
                source_candidate_carrier: [53; 32],
                ceremony_spec: [54; 32],
                attempt: [55; 32],
                expected_old_source: [56; 32],
                withdrawal_carrier: [57; 32],
                withdrawal_cas: [58; 32],
                withdrawal_publication_root: [59; 32],
            },
            H3WithdrawalPublicationSourceV1::NoStore {
                no_store_home: [50; 32],
                ceremony_spec: [51; 32],
                attempt: [52; 32],
                occurrence: [53; 32],
                carrier: [54; 32],
            },
        ]
    }

    #[test]
    fn all_three_homes_require_the_exact_causal_branch_and_one_use_finality() {
        for source in sources() {
            let bound = facts(source);
            let native = test_native(bound);
            let verified = verify_h3_withdrawal_publication_use(bound, &native).unwrap();
            let (association, finality) = migration_finality(bound);
            let outcome = match finality {
                TestMigrationFinalityV1::ActiveStore(finality) => verified
                    .consume_for_migration(
                        &association,
                        H3MigrationFinalityV1::ActiveStore(&finality),
                    )
                    .map(|_| ()),
                TestMigrationFinalityV1::PreStore(finality) => verified
                    .consume_for_migration(&association, H3MigrationFinalityV1::PreStore(&finality))
                    .map(|_| ()),
                TestMigrationFinalityV1::NoStore => {
                    let (_, fallback) = migration_finality(facts(sources()[0]));
                    let TestMigrationFinalityV1::ActiveStore(fallback) = fallback else {
                        unreachable!()
                    };
                    verified
                        .consume_for_migration(
                            &association,
                            H3MigrationFinalityV1::ActiveStore(&fallback),
                        )
                        .map(|_| ())
                }
            };
            if matches!(source, H3WithdrawalPublicationSourceV1::NoStore { .. }) {
                assert!(matches!(
                    outcome,
                    Err(H3WithdrawalPublicationErrorV1::NonPromoting)
                ));
                assert!(verify_h3_withdrawal_publication_use(bound, &native).is_ok());
            } else {
                outcome.unwrap();
                assert!(matches!(
                    verify_h3_withdrawal_publication_use(bound, &native),
                    Err(H3WithdrawalPublicationErrorV1::InvalidCarrier)
                ));
            }
        }
    }

    #[test]
    fn cross_branch_and_complete_meaning_substitution_refuse_without_consumption() {
        let mut active = facts(sources()[0]);
        active.origin = ceremony_origin();
        let native = test_native(active);
        assert!(matches!(
            verify_h3_withdrawal_publication_use(active, &native),
            Err(H3WithdrawalPublicationErrorV1::CausalBranchMismatch)
                | Err(H3WithdrawalPublicationErrorV1::InvalidCarrier)
        ));

        let facts = facts(sources()[1]);
        let native = test_native(facts);
        let verified = verify_h3_withdrawal_publication_use(facts, &native).unwrap();
        let mut substituted = facts;
        substituted.consumer_gate = [99; 32];
        assert!(matches!(
            verified.consume(H3WithdrawalPublicationCommitV1 { facts: substituted }),
            Err(H3WithdrawalPublicationErrorV1::BindingMismatch)
        ));
        assert!(verify_h3_withdrawal_publication_use(facts, &native).is_ok());
    }

    #[test]
    fn native_publication_source_catalog_and_release_substitution_refuse() {
        let facts = facts(sources()[0]);
        let mut wrong_source_facts = facts;
        wrong_source_facts.source = sources()[1];
        let native = test_native(facts);
        assert!(matches!(
            verify_h3_withdrawal_publication_use(wrong_source_facts, &native),
            Err(H3WithdrawalPublicationErrorV1::InvalidCarrier)
        ));

        let mut wrong_catalog_facts = facts;
        wrong_catalog_facts.catalog_cell =
            crate::domain::vnext::execution::withdrawal_catalog_cells_v1()[1];
        assert!(matches!(
            verify_h3_withdrawal_publication_use(wrong_catalog_facts, &native),
            Err(H3WithdrawalPublicationErrorV1::InvalidCarrier)
        ));

        let mut zero_release = facts;
        zero_release.optional_release = Some([0; 32]);
        assert!(matches!(
            verify_h3_withdrawal_publication_use(zero_release, &native),
            Err(H3WithdrawalPublicationErrorV1::InvalidCarrier)
        ));
    }

    #[test]
    fn pre_store_has_no_destination_root_or_candidate_seal_field() {
        let source = sources()[1];
        let source_text = include_str!("h3_withdrawal_publication.rs");
        assert!(matches!(
            source,
            H3WithdrawalPublicationSourceV1::PreStore { .. }
        ));
        assert!(!source_text.contains(concat!("candidate_root_", "commitment")));
        assert!(!source_text.contains(concat!("candidate_seal_", "commitment")));
        assert!(!source_text.contains(concat!("association_", "commitment")));
    }
}
