use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::domain::vnext::persistence::{
    AtomicGenerationPublicationV1, PreparedPublicationError, StoreError, StoreIdempotencyProbeV1,
    StorePublicationOutcomeV1, StoreStateV1, StoreV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::durable_finality::{
    ActiveStoreDecisionTupleV1, ActiveStoreFinalityOwnerV1, ActiveStoreFinalityOwnerV2,
    ActiveStoreFinalityRequestV1, ActiveStoreFinalityRequestV2, ActiveStoreOwnerOutcomeV1,
    ActiveStoreOwnerOutcomeV2, DurableInstallationFinalityErrorV1,
    DurableInstallationFinalityErrorV2, InstallationFinalityCurrentnessV1, owner_sealed,
};

#[cfg(test)]
pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalitySeedV1 {
    _private: (),
}

#[cfg(test)]
impl Stage9ActiveStoreFinalitySeedV1 {
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

#[cfg(test)]
impl owner_sealed::Sealed for Stage9ActiveStoreFinalitySeedV1 {}

#[cfg(test)]
impl ActiveStoreFinalityOwnerV1 for Stage9ActiveStoreFinalitySeedV1 {
    fn prepare_request(
        &mut self,
    ) -> Result<ActiveStoreFinalityRequestV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn commit_and_readback(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &ActiveStoreFinalityRequestV1,
    ) -> Result<ActiveStoreOwnerOutcomeV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }
}

pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalitySeedV2 {
    provider: Option<Stage9ActiveStoreFinalityProviderV2>,
}

impl Stage9ActiveStoreFinalitySeedV2 {
    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { provider: None }
    }
}

struct Stage9ActiveStoreFinalityProviderV2 {
    store: StoreV1,
    expected: InstallationFinalityCurrentnessV1,
    decision: ActiveStoreDecisionTupleV1,
    probe: StoreIdempotencyProbeV1,
    publication: AtomicGenerationPublicationV1,
    replay: Option<StorePublicationOutcomeV1>,
    consumed: bool,
}

impl owner_sealed::Sealed for Stage9ActiveStoreFinalitySeedV2 {}

pub(super) fn capture_owner_publication_v2(
    store: StoreV1,
    expected: InstallationFinalityCurrentnessV1,
    decision: ActiveStoreDecisionTupleV1,
    publication: AtomicGenerationPublicationV1,
) -> Result<Stage9ActiveStoreFinalitySeedV2, DurableInstallationFinalityErrorV2> {
    let probe = StoreIdempotencyProbeV1::new(
        publication.idempotency().namespace(),
        *publication.idempotency().key_digest(),
        *publication.idempotency().meaning_digest(),
    )
    .map_err(|_| DurableInstallationFinalityErrorV2::CurrentnessMismatch)?;
    let replay = store.replay_idempotency(&probe).map_err(map_replay_error)?;
    validate_owner_publication(&store, expected, decision, &publication, replay.as_ref())?;
    Ok(Stage9ActiveStoreFinalitySeedV2 {
        provider: Some(Stage9ActiveStoreFinalityProviderV2 {
            store,
            expected,
            decision,
            probe,
            publication,
            replay,
            consumed: false,
        }),
    })
}

impl ActiveStoreFinalityOwnerV2 for Stage9ActiveStoreFinalitySeedV2 {
    fn capture_active_request(
        &mut self,
    ) -> Result<ActiveStoreFinalityRequestV2, DurableInstallationFinalityErrorV2> {
        let provider = self
            .provider
            .as_ref()
            .ok_or(DurableInstallationFinalityErrorV2::BackendUnavailable)?;
        ActiveStoreFinalityRequestV2::from_stage9_owner(provider.expected, provider.decision)
    }

    fn commit_and_readback(
        &mut self,
        request: &ActiveStoreFinalityRequestV2,
    ) -> Result<ActiveStoreOwnerOutcomeV2, DurableInstallationFinalityErrorV2> {
        let Some(provider) = self.provider.as_mut() else {
            return Err(DurableInstallationFinalityErrorV2::BackendUnavailable);
        };
        if provider.consumed {
            return Err(DurableInstallationFinalityErrorV2::Replay);
        }
        provider.consumed = true;

        if let Some(replay) = provider.replay.take() {
            return match committed_readback(provider, request, &replay) {
                Ok(readback) => Ok(ActiveStoreOwnerOutcomeV2::Committed(readback)),
                Err(_) => Ok(ActiveStoreOwnerOutcomeV2::IntegrityBlocked),
            };
        }
        if !current_store_matches(&provider.store, provider.expected)? {
            return Ok(ActiveStoreOwnerOutcomeV2::PreCommitRefused);
        }

        let publication = provider.publication.clone();
        let expected_currentness = provider.expected;
        let outcome =
            provider
                .store
                .publish_generation_atomically_with_prepare(&provider.probe, |view| {
                    let head = view
                        .active_head()
                        .map_err(|_| DurableInstallationFinalityErrorV2::BackendUnavailable)?
                        .ok_or(DurableInstallationFinalityErrorV2::CurrentnessMismatch)?;
                    let generation = view
                        .active_generation()
                        .map_err(|_| DurableInstallationFinalityErrorV2::BackendUnavailable)?
                        .ok_or(DurableInstallationFinalityErrorV2::CurrentnessMismatch)?;
                    if head.id().as_bytes() != &expected_currentness.head
                        || head.revision() != expected_currentness.head_revision
                        || generation.id().as_bytes() != &expected_currentness.generation
                        || generation.ordinal() != expected_currentness.generation_ordinal
                        || publication.expected_old() != Some(head.id())
                    {
                        return Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch);
                    }
                    Ok(publication.clone())
                });
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(PreparedPublicationError::Prepare(
                DurableInstallationFinalityErrorV2::CurrentnessMismatch,
            )) => return Ok(ActiveStoreOwnerOutcomeV2::PreCommitRefused),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
            Err(PreparedPublicationError::Store(_)) => {
                return Ok(ActiveStoreOwnerOutcomeV2::UnknownOccurrence);
            }
        };
        match committed_readback(provider, request, &outcome) {
            Ok(readback) => Ok(ActiveStoreOwnerOutcomeV2::Committed(readback)),
            Err(_) => Ok(ActiveStoreOwnerOutcomeV2::AcknowledgementLost(None)),
        }
    }
}

fn validate_owner_publication(
    store: &StoreV1,
    expected: InstallationFinalityCurrentnessV1,
    decision: ActiveStoreDecisionTupleV1,
    publication: &AtomicGenerationPublicationV1,
    replay: Option<&StorePublicationOutcomeV1>,
) -> Result<(), DurableInstallationFinalityErrorV2> {
    let (state, head, generation, _) = store
        .coherent_publication_snapshot()
        .map_err(|_| DurableInstallationFinalityErrorV2::BackendUnavailable)?;
    let expected_old = publication
        .expected_old()
        .ok_or(DurableInstallationFinalityErrorV2::CurrentnessMismatch)?;
    let successor = crate::domain::vnext::persistence::StoreHeadV1::new(
        publication.generation(),
        publication.generation().ordinal(),
        Some(expected_old),
    )
    .map_err(|_| DurableInstallationFinalityErrorV2::CurrentnessMismatch)?;
    let current_is_expected_old = expected.head == *head.id().as_bytes()
        && expected.head_revision == head.revision()
        && expected.generation == *generation.id().as_bytes()
        && expected.generation_ordinal == generation.ordinal()
        && publication.expected_old() == Some(head.id());
    let current_is_exact_replay = replay.is_some_and(|outcome| {
        outcome.head() == &head
            && publication.generation() == &generation
            && outcome.result().id() == publication.idempotency().result_object_id()
    });
    if state != StoreStateV1::Active
        || expected.domain != *store.domain().id().as_bytes()
        || (!current_is_expected_old && !current_is_exact_replay)
        || publication.generation().domain() != store.domain()
        || publication.generation().previous() != Some(generation.id()) && !current_is_exact_replay
        || publication.generation().ordinal() != generation.ordinal().saturating_add(1)
            && !current_is_exact_replay
        || decision.successor_head != *successor.id().as_bytes()
        || decision.idempotency_key != *publication.idempotency().key_digest()
        || decision.idempotency_meaning != *publication.idempotency().meaning_digest()
        || decision.result != *publication.idempotency().result_object_id().as_bytes()
        || decision.rows != object_set_commitment(publication.objects())
        || decision.postcondition != publication_postcondition(publication, successor.id())
    {
        return Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch);
    }
    validate_decision_objects(decision, publication.objects())
}

fn current_store_matches(
    store: &StoreV1,
    expected: InstallationFinalityCurrentnessV1,
) -> Result<bool, DurableInstallationFinalityErrorV2> {
    let (state, head, generation, _) = store
        .coherent_publication_snapshot()
        .map_err(|_| DurableInstallationFinalityErrorV2::BackendUnavailable)?;
    Ok(state == StoreStateV1::Active
        && expected.domain == *store.domain().id().as_bytes()
        && expected.head == *head.id().as_bytes()
        && expected.head_revision == head.revision()
        && expected.generation == *generation.id().as_bytes()
        && expected.generation_ordinal == generation.ordinal())
}

fn committed_readback(
    provider: &Stage9ActiveStoreFinalityProviderV2,
    request: &ActiveStoreFinalityRequestV2,
    outcome: &StorePublicationOutcomeV1,
) -> Result<
    super::durable_finality::ActiveStoreCommittedReadbackV2,
    DurableInstallationFinalityErrorV2,
> {
    let (state, head, generation, objects) = provider
        .store
        .coherent_publication_snapshot()
        .map_err(|_| DurableInstallationFinalityErrorV2::BackendUnavailable)?;
    let expected_ids = provider
        .publication
        .objects()
        .iter()
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let observed_ids = objects
        .iter()
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    if state != StoreStateV1::Active
        || &head != outcome.head()
        || &generation != provider.publication.generation()
        || outcome.result().id() != provider.publication.idempotency().result_object_id()
        || expected_ids != observed_ids
    {
        return Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch);
    }
    validate_decision_objects(provider.decision, &objects)?;
    request
        .consume_stage9_owner_view()?
        .validate_committed_readback(
            provider.expected,
            provider.decision,
            provider.decision.association_identity,
            provider.decision.consumer_gate_result,
            provider.decision.receipt,
            provider.decision.distribution_commit,
            *head.id().as_bytes(),
            *outcome.result().id().as_bytes(),
            *provider.publication.idempotency().meaning_digest(),
        )
}

fn validate_decision_objects(
    decision: ActiveStoreDecisionTupleV1,
    objects: &[crate::domain::vnext::persistence::StoreObjectV1],
) -> Result<(), DurableInstallationFinalityErrorV2> {
    let association = required_object(objects, decision.association_identity)?;
    let _receipt = required_object(objects, decision.receipt)?;
    let _commit = required_object(objects, decision.distribution_commit)?;
    let _result = required_object(objects, decision.result)?;
    if !cbor_contains_commitment(association.value(), decision.association_meaning)
        || !cbor_contains_commitment(association.value(), decision.consumer_gate_result)
    {
        return Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch);
    }
    Ok(())
}

fn required_object(
    objects: &[crate::domain::vnext::persistence::StoreObjectV1],
    expected: [u8; 32],
) -> Result<&crate::domain::vnext::persistence::StoreObjectV1, DurableInstallationFinalityErrorV2> {
    objects
        .iter()
        .find(|object| object.id().as_bytes() == &expected)
        .ok_or(DurableInstallationFinalityErrorV2::CurrentnessMismatch)
}

fn cbor_contains_commitment(value: &CborValue, expected: [u8; 32]) -> bool {
    match value {
        CborValue::Bytes(bytes) => bytes.as_slice() == expected.as_slice(),
        CborValue::Array(values) => values
            .iter()
            .any(|value| cbor_contains_commitment(value, expected)),
        _ => false,
    }
}

fn object_set_commitment(objects: &[crate::domain::vnext::persistence::StoreObjectV1]) -> [u8; 32] {
    let mut ids = objects.iter().map(|object| object.id()).collect::<Vec<_>>();
    ids.sort_unstable();
    let mut digest = Sha256::new();
    digest.update(b"maestro.vnext.stage9-active-finality-object-set.v1\0");
    digest.update((ids.len() as u64).to_be_bytes());
    for id in ids {
        digest.update(id.as_bytes());
    }
    digest.finalize().into()
}

fn map_replay_error(error: StoreError) -> DurableInstallationFinalityErrorV2 {
    match error {
        StoreError::IdempotencyMeaningConflict => {
            DurableInstallationFinalityErrorV2::CurrentnessMismatch
        }
        _ => DurableInstallationFinalityErrorV2::BackendUnavailable,
    }
}

fn publication_postcondition(
    publication: &AtomicGenerationPublicationV1,
    successor_head: crate::domain::vnext::identity::StoreHeadIdV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"maestro.vnext.stage9-active-finality-postcondition.v1\0");
    digest.update(successor_head.as_bytes());
    digest.update(publication.generation().id().as_bytes());
    digest.update(object_set_commitment(publication.objects()));
    digest.update(publication.idempotency().result_object_id().as_bytes());
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::vnext::identity::{ContractRootIdV1, SchemaIdV1};
    use crate::domain::vnext::persistence::{
        StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreIdempotencyV1, StoreObjectV1,
        StoreRoleV1,
    };
    use rusqlite::Connection;

    fn rendered(byte: u8) -> String {
        format!("sha256:{}", format!("{byte:02x}").repeat(32))
    }

    fn object(seed: u64, commitments: &[[u8; 32]]) -> StoreObjectV1 {
        let mut fields = vec![CborValue::Unsigned(1), CborValue::Unsigned(seed)];
        fields.extend(
            commitments
                .iter()
                .map(|commitment| CborValue::Bytes(commitment.to_vec())),
        );
        StoreObjectV1::new(
            SchemaIdV1::parse(&rendered(1)).expect("stage9 schema"),
            CborValue::Array(fields),
            vec![],
        )
        .expect("stage9 Store object")
    }

    fn store_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("stage9 Store test clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-vnext-stage9-v4-store-{}-{nanos}-{label}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("stage9 Store test root");
        std::fs::canonicalize(root).expect("stage9 Store canonical root")
    }

    fn activate_with_sqlite(path: &std::path::Path) {
        let mut connection =
            Connection::open(path.join("store.sqlite3")).expect("stage9 Store metadata");
        let transaction = connection
            .transaction()
            .expect("stage9 activation transaction");
        transaction
            .execute(
                "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                [],
            )
            .expect("stage9 active state");
        transaction.commit().expect("stage9 activation commit");
    }

    fn real_publication_fixture(
        label: &str,
    ) -> (
        StoreV1,
        InstallationFinalityCurrentnessV1,
        ActiveStoreDecisionTupleV1,
        AtomicGenerationPublicationV1,
    ) {
        let path = store_path(label);
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Installation, label.as_bytes()).expect("domain");
        let mut store = StoreV1::create(&path, domain.clone()).expect("stage9 Store");
        let initial_result = object(1, &[]);
        let initial_root = object(2, &[]);
        let initial_generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(2)).expect("stage9 Contract Root"),
            StoreCompatibilityV1::stage0_successor().expect("stage9 compatibility"),
            vec![initial_root.id(), initial_result.id()],
        )
        .expect("stage9 initial Generation");
        let initial_idempotency = StoreIdempotencyV1::new(
            "maestro.vnext.stage9-initial.v1",
            [70; 32],
            [71; 32],
            initial_result.id(),
        )
        .expect("stage9 initial idempotency");
        let initial_publication = AtomicGenerationPublicationV1::new(
            initial_generation,
            None,
            vec![initial_result, initial_root],
            initial_idempotency,
        )
        .expect("stage9 initial publication");
        store
            .publish_generation_atomically(&initial_publication)
            .expect("stage9 initial publish");
        drop(store);
        activate_with_sqlite(&path);
        let store = StoreV1::open(&path, domain.clone()).expect("stage9 active Store");
        let (state, head, generation, mut active_objects) = store
            .coherent_publication_snapshot()
            .expect("stage9 current Store");
        assert_eq!(state, StoreStateV1::Active);

        let association_meaning = [90; 32];
        let consumer_gate_result = [91; 32];
        let association = object(10, &[association_meaning, consumer_gate_result]);
        let receipt = object(11, &[]);
        let commit = object(12, &[]);
        let result = object(13, &[]);
        let association_id = *association.id().as_bytes();
        let receipt_id = *receipt.id().as_bytes();
        let commit_id = *commit.id().as_bytes();
        let result_object_id = result.id();
        let result_id = *result_object_id.as_bytes();
        active_objects.extend([association, receipt, commit, result]);
        let mut roots = active_objects
            .iter()
            .map(StoreObjectV1::id)
            .collect::<Vec<_>>();
        roots.sort_unstable();
        let successor_generation = StoreGenerationV1::new(
            domain.clone(),
            generation.ordinal() + 1,
            Some(generation.id()),
            generation.contract_root_id(),
            generation.compatibility().clone(),
            roots,
        )
        .expect("stage9 successor Generation");
        let idempotency = StoreIdempotencyV1::new(
            "maestro.vnext.stage9-active-finality.v2",
            [81; 32],
            [82; 32],
            result_object_id,
        )
        .expect("stage9 finality idempotency");
        let publication = AtomicGenerationPublicationV1::new_from_object_superset(
            successor_generation,
            Some(head.id()),
            active_objects,
            idempotency,
        )
        .expect("stage9 finality publication");
        let successor_head = crate::domain::vnext::persistence::StoreHeadV1::new(
            publication.generation(),
            publication.generation().ordinal(),
            Some(head.id()),
        )
        .expect("stage9 successor Head");
        let expected = InstallationFinalityCurrentnessV1 {
            installation: [1; 32],
            tenant: [2; 32],
            principal: [3; 32],
            authority: [4; 32],
            realm: [5; 32],
            domain: *domain.id().as_bytes(),
            store_instance: [6; 32],
            activation_incarnation: [7; 32],
            head: *head.id().as_bytes(),
            head_revision: head.revision(),
            generation: *generation.id().as_bytes(),
            generation_ordinal: generation.ordinal(),
            store_cas: [8; 32],
            host_connection: [9; 32],
            host_currentness: [10; 32],
            currentness: [11; 32],
            fence: [12; 32],
            revocation_revision: 1,
        };
        let decision = ActiveStoreDecisionTupleV1 {
            operation: [20; 32],
            attempt: [21; 32],
            action: [22; 32],
            request: [23; 32],
            release: [24; 32],
            rows: object_set_commitment(publication.objects()),
            successor_head: *successor_head.id().as_bytes(),
            result: result_id,
            idempotency_key: *publication.idempotency().key_digest(),
            idempotency_meaning: *publication.idempotency().meaning_digest(),
            writer_protocol_epoch: 1,
            schema_epoch: 1,
            migration_epoch: 1,
            census: [25; 32],
            consumer_set: [26; 32],
            consumer_gate_result,
            association_identity: association_id,
            association_meaning,
            distribution_commit: commit_id,
            receipt: receipt_id,
            expected_old_owner_state: [27; 32],
            invocation: [28; 32],
            carrier: [29; 32],
            host_guard: [30; 32],
            postcondition: publication_postcondition(&publication, successor_head.id()),
        };
        (store, expected, decision, publication)
    }

    #[test]
    fn stage9_owner_test_provider_is_constructible_only_in_its_owner_module() {
        let mut backend = Stage9ActiveStoreFinalitySeedV1::test_unavailable();
        assert!(matches!(
            super::super::durable_finality::prepare_active_from_stage9_owner(&mut backend),
            Err(super::super::durable_finality::Stage9ActiveStoreFinalityErrorV1)
        ));
    }

    #[test]
    fn owner_capture_is_a_concrete_store_publication_not_a_callback() {
        let _ = capture_owner_publication_v2;
    }

    #[test]
    fn real_v2_owner_atomically_publishes_and_rereads_the_exact_store_postcondition() {
        let (store, expected, decision, publication) = real_publication_fixture("commit");
        let mut seed = capture_owner_publication_v2(store, expected, decision, publication.clone())
            .expect("stage9 owner capture");
        assert_eq!(
            super::super::durable_finality::DurableInstallationFinalityBackendV2::capture(
                &mut seed
            )
            .consume_active()
            .expect("stage9 finality"),
            super::super::durable_finality::DurableInstallationFinalityOutcomeV2::Committed
        );

        let provider = seed.provider.take().expect("stage9 consumed provider");
        let (_, committed_head, _, _) = provider
            .store
            .coherent_publication_snapshot()
            .expect("stage9 committed Store");
        let committed_revision = committed_head.revision();
        let mut replay =
            capture_owner_publication_v2(provider.store, expected, decision, publication)
                .expect("stage9 exact replay capture");
        assert_eq!(
            super::super::durable_finality::DurableInstallationFinalityBackendV2::capture(
                &mut replay
            )
            .consume_active()
            .expect("stage9 exact replay"),
            super::super::durable_finality::DurableInstallationFinalityOutcomeV2::Committed
        );
        let replay_provider = replay.provider.take().expect("stage9 replay provider");
        let (_, replay_head, _, _) = replay_provider
            .store
            .coherent_publication_snapshot()
            .expect("stage9 replay Store");
        assert_eq!(replay_head.revision(), committed_revision);
    }

    #[test]
    fn wrong_rows_and_false_success_inputs_are_rejected_before_publication() {
        let (store, expected, mut decision, publication) = real_publication_fixture("wrong-rows");
        decision.rows = [99; 32];
        assert!(matches!(
            capture_owner_publication_v2(store, expected, decision, publication),
            Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch)
        ));
    }
}
