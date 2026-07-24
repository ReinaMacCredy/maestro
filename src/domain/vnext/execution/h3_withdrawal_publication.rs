use std::marker::PhantomData;
use std::rc::Rc;

#[cfg(test)]
use std::cell::Cell;
use thiserror::Error;

use super::{EffectIntentHomeKindV1, WithdrawalCatalogCellV1, WithdrawalRouteBindingV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum H3WithdrawalPublicationSourceV1 {
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
pub enum H3WithdrawalPublicationOriginV1 {
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
pub struct H3WithdrawalPublicationFactsV1 {
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

pub trait NativeH3WithdrawalPublicationPortV1: native_publication_sealed::Sealed {
    fn publication_identity(&self) -> [u8; 32];
    fn catalog_cell(&self) -> WithdrawalCatalogCellV1;
    fn source(&self) -> H3WithdrawalPublicationSourceV1;
    fn is_unused(&self) -> bool;
    fn consume_once(&self) -> Result<(), H3WithdrawalPublicationErrorV1>;
}

pub struct VerifiedH3WithdrawalPublicationUseV1<'tx> {
    facts: H3WithdrawalPublicationFactsV1,
    native_publication: &'tx dyn NativeH3WithdrawalPublicationPortV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct H3WithdrawalPublicationCommitV1 {
    facts: H3WithdrawalPublicationFactsV1,
}

pub struct ConsumedH3WithdrawalPublicationV1<'tx> {
    source: H3WithdrawalPublicationSourceV1,
    origin: H3WithdrawalPublicationOriginV1,
    publication_identity: [u8; 32],
    optional_release: Option<[u8; 32]>,
    association_input_context: Option<[u8; 32]>,
    finality_context: [u8; 32],
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx> VerifiedH3WithdrawalPublicationUseV1<'tx> {
    pub fn consume(
        self,
        commit: H3WithdrawalPublicationCommitV1,
    ) -> Result<ConsumedH3WithdrawalPublicationV1<'tx>, H3WithdrawalPublicationErrorV1> {
        if !self.native_publication.is_unused() || self.facts != commit.facts {
            return Err(H3WithdrawalPublicationErrorV1::BindingMismatch);
        }
        self.native_publication.consume_once()?;
        let (association_input_context, finality_context) = match self.facts.source {
            H3WithdrawalPublicationSourceV1::ActiveStore {
                association_input_context,
                finality_context,
                ..
            } => (Some(association_input_context), finality_context),
            H3WithdrawalPublicationSourceV1::PreStore {
                withdrawal_cas,
                withdrawal_publication_root,
                ..
            } => (
                None,
                canonical_pair(withdrawal_cas, withdrawal_publication_root),
            ),
            H3WithdrawalPublicationSourceV1::NoStore {
                occurrence,
                carrier,
                ..
            } => (None, canonical_pair(occurrence, carrier)),
        };
        Ok(ConsumedH3WithdrawalPublicationV1 {
            source: self.facts.source,
            origin: self.facts.origin,
            publication_identity: self.facts.withdrawal_publication,
            optional_release: self.facts.optional_release,
            association_input_context,
            finality_context,
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub fn verify_h3_withdrawal_publication_use<'tx>(
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

fn canonical_pair(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"maestro.vnext.h3-finality-context.v1\0");
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
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

fn canonical_home_commitment(home: EffectIntentHomeKindV1) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let literal = match home {
        EffectIntentHomeKindV1::ActiveStore => b"ActiveStore".as_slice(),
        EffectIntentHomeKindV1::PreStoreCeremony => b"PreStore".as_slice(),
        EffectIntentHomeKindV1::NoStoreCeremony => b"NoStore".as_slice(),
    };
    Sha256::digest(literal).into()
}

impl ConsumedH3WithdrawalPublicationV1<'_> {
    pub const fn source(&self) -> H3WithdrawalPublicationSourceV1 {
        self.source
    }

    pub const fn origin(&self) -> H3WithdrawalPublicationOriginV1 {
        self.origin
    }

    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    pub const fn optional_release(&self) -> Option<[u8; 32]> {
        self.optional_release
    }

    pub const fn association_input_context(&self) -> Option<[u8; 32]> {
        self.association_input_context
    }

    pub const fn finality_context(&self) -> [u8; 32] {
        self.finality_context
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum H3WithdrawalPublicationErrorV1 {
    #[error("H3 withdrawal publication carrier is invalid")]
    InvalidCarrier,
    #[error("H3 source and Action/Ceremony causal branch disagree")]
    CausalBranchMismatch,
    #[error("H3 withdrawal publication does not match the exact finality use")]
    BindingMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let facts = facts(source);
            let native = test_native(facts);
            let verified = verify_h3_withdrawal_publication_use(facts, &native).unwrap();
            verified
                .consume(H3WithdrawalPublicationCommitV1 { facts })
                .unwrap();
            assert!(matches!(
                verify_h3_withdrawal_publication_use(facts, &native),
                Err(H3WithdrawalPublicationErrorV1::InvalidCarrier)
            ));
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
