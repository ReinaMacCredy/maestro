//! Stage-6 Action submission and replay service.

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::authority::{AuthorityActionLeafV1, RepositoryActionLeafV1};
use crate::domain::capability::generated_catalog::{
    CatalogOwnerV1, CeremonyContextKindV1, GeneratedCapabilityCatalogV1, GeneratedCatalogErrorV1,
    OperationCatalogEntryV1,
};
use crate::domain::integration::public_literals::{
    ActionAuthorityBasisV1, ActionRequestV1, ActionResultV1, CeremonyRequestContextV1,
    CeremonyRequestModeV1, CeremonyRequestV1, CeremonyResultV1, OperationRequestV1,
    OperationResultBodyV1, OperationResultV1, OperationSemanticOutcomeV1, OperationSpecRefV1,
    OrchestrationAttributionV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerAdmissionV1 {
    pub catalog_ordinal: u16,
    pub operation_name: String,
    pub owner: CatalogOwnerV1,
    pub operation_spec_ref: String,
    pub semantic_request_hash: [u8; 32],
    pub idempotency_key: String,
    pub selected_packet_semantic_hash: Option<[u8; 32]>,
    pub material_dependency_stamp: Option<[u8; 32]>,
    pub exact_store_generation_ref: Option<String>,
    pub exact_authority_epoch_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnerSubmissionOutcomeV1 {
    Durable(Box<OwnerDurableResultV1>),
    SameKeyDifferentMeaning,
    OwnerUnavailable { inspect_ref: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerDurableResultV1 {
    admission: OwnerAdmissionV1,
    body: OperationResultBodyV1,
    replayed: bool,
}

impl OwnerDurableResultV1 {
    pub(crate) fn fresh(admission: OwnerAdmissionV1, body: OperationResultBodyV1) -> Self {
        Self {
            admission,
            body,
            replayed: false,
        }
    }

    pub(crate) fn replay(admission: OwnerAdmissionV1, body: OperationResultBodyV1) -> Self {
        Self {
            admission,
            body,
            replayed: true,
        }
    }
}

pub(crate) trait GovernedOperationPortV1 {
    /// The semantic owner must authorize and durably commit or replay this exact admission.
    fn submit(
        &self,
        request: &OperationRequestV1,
        admission: &OwnerAdmissionV1,
    ) -> Result<OwnerSubmissionOutcomeV1, ActionSubmissionErrorV1>;
}

pub(crate) trait OperationResultReadPortV1 {
    fn read_result(
        &self,
        request_id: &str,
    ) -> Result<Option<(OperationRequestV1, OperationResultV1)>, ActionSubmissionErrorV1>;
}

pub(crate) struct ActionSubmissionServiceV1 {
    catalog: GeneratedCapabilityCatalogV1,
}

impl ActionSubmissionServiceV1 {
    pub(crate) fn load() -> Result<Self, ActionSubmissionErrorV1> {
        Ok(Self {
            catalog: GeneratedCapabilityCatalogV1::load_frozen()?,
        })
    }

    pub(crate) fn prepare_named(
        &self,
        kind: OperationKindV1,
        name: &str,
    ) -> Result<PreparedOperationV1, ActionSubmissionErrorV1> {
        let entry = match kind {
            OperationKindV1::Action => self.catalog.action_named(name),
            OperationKindV1::Ceremony => self.catalog.ceremony_named(name),
        }
        .ok_or(ActionSubmissionErrorV1::UnknownOperation)?;
        Ok(PreparedOperationV1::from_entry(entry))
    }

    pub(crate) fn submit(
        &self,
        port: &dyn GovernedOperationPortV1,
        request: &OperationRequestV1,
    ) -> Result<OperationResultV1, ActionSubmissionErrorV1> {
        request
            .validate()
            .map_err(|_| ActionSubmissionErrorV1::InvalidRequest)?;
        let entry = self.validated_entry(request)?;
        if matches!(request, OperationRequestV1::Action(_))
            && !has_frozen_owner_materialization(entry)
        {
            let result = result_for(
                request,
                OperationSemanticOutcomeV1::Unavailable,
                None,
                false,
            );
            result
                .validate_for(request)
                .map_err(|_| ActionSubmissionErrorV1::InvalidOwnerResult)?;
            return Ok(result);
        }
        let admission = self.admission(entry, request);
        let result = match port.submit(request, &admission)? {
            OwnerSubmissionOutcomeV1::Durable(durable) => {
                self.validate_durable(request, &admission, *durable)?
            }
            OwnerSubmissionOutcomeV1::SameKeyDifferentMeaning => {
                result_for(request, OperationSemanticOutcomeV1::Conflict, None, false)
            }
            OwnerSubmissionOutcomeV1::OwnerUnavailable { inspect_ref } => result_for(
                request,
                OperationSemanticOutcomeV1::Unavailable,
                inspect_ref,
                false,
            ),
        };
        result
            .validate_for(request)
            .map_err(|_| ActionSubmissionErrorV1::InvalidOwnerResult)?;
        Ok(result)
    }

    pub(crate) fn read_result(
        &self,
        port: &dyn OperationResultReadPortV1,
        request_id: &str,
    ) -> Result<Option<OperationResultV1>, ActionSubmissionErrorV1> {
        if request_id.is_empty() {
            return Err(ActionSubmissionErrorV1::InvalidRequest);
        }
        let Some((request, result)) = port.read_result(request_id)? else {
            return Ok(None);
        };
        request
            .validate()
            .map_err(|_| ActionSubmissionErrorV1::InvalidOwnerResult)?;
        result
            .validate_for(&request)
            .map_err(|_| ActionSubmissionErrorV1::InvalidOwnerResult)?;
        let result_request_id = match &request {
            OperationRequestV1::Action(action) => &action.request_id,
            OperationRequestV1::Ceremony(ceremony) => &ceremony.request_id,
        };
        if result_request_id != request_id {
            return Err(ActionSubmissionErrorV1::InvalidOwnerResult);
        }
        Ok(Some(result))
    }

    fn validated_entry<'a>(
        &'a self,
        request: &OperationRequestV1,
    ) -> Result<&'a OperationCatalogEntryV1, ActionSubmissionErrorV1> {
        let (descriptor_ref, provided_hash, _) = request_identity(request);
        let entry = self
            .catalog
            .operation(descriptor_ref)
            .ok_or(ActionSubmissionErrorV1::UnknownOperation)?;
        validate_catalog_binding(entry, request)?;
        let semantic_request_hash = semantic_request_hash(request);
        if semantic_request_hash != provided_hash {
            return Err(ActionSubmissionErrorV1::SemanticHashMismatch);
        }
        Ok(entry)
    }

    fn admission(
        &self,
        entry: &OperationCatalogEntryV1,
        request: &OperationRequestV1,
    ) -> OwnerAdmissionV1 {
        let (descriptor_ref, _, idempotency_key) = request_identity(request);
        let semantic_request_hash = semantic_request_hash(request);
        let (
            selected_packet_semantic_hash,
            material_dependency_stamp,
            exact_store_generation_ref,
            exact_authority_epoch_ref,
        ) = match request {
            OperationRequestV1::Action(action) => (
                Some(action.selected_packet_semantic_hash),
                Some(action.material_dependency_stamp),
                Some(action.exact_store_generation_ref.clone()),
                Some(action.exact_authority_epoch_ref.clone()),
            ),
            OperationRequestV1::Ceremony(_) => (None, None, None, None),
        };
        OwnerAdmissionV1 {
            catalog_ordinal: entry.ordinal(),
            operation_name: entry.name().to_owned(),
            owner: entry.owner(),
            operation_spec_ref: descriptor_ref.to_owned(),
            semantic_request_hash,
            idempotency_key: idempotency_key.to_owned(),
            selected_packet_semantic_hash,
            material_dependency_stamp,
            exact_store_generation_ref,
            exact_authority_epoch_ref,
        }
    }

    fn validate_durable(
        &self,
        request: &OperationRequestV1,
        expected: &OwnerAdmissionV1,
        mut durable: OwnerDurableResultV1,
    ) -> Result<OperationResultV1, ActionSubmissionErrorV1> {
        if durable.admission != *expected {
            return Err(ActionSubmissionErrorV1::InvalidOwnerBinding);
        }
        durable.body.replayed_delivery = durable.replayed;
        if durable.replayed {
            // The idempotent meaning omits request_id, so a same-key
            // same-meaning retry legitimately carries a new request_id; the
            // owner's durable record keeps the original while delivery
            // rebinds to the caller's id.
            durable.body.request_id = match request {
                OperationRequestV1::Action(action) => action.request_id.clone(),
                OperationRequestV1::Ceremony(ceremony) => ceremony.request_id.clone(),
            };
        }
        let result = match request {
            OperationRequestV1::Action(_) => {
                OperationResultV1::Action(ActionResultV1(durable.body))
            }
            OperationRequestV1::Ceremony(_) => {
                OperationResultV1::Ceremony(CeremonyResultV1(durable.body))
            }
        };
        result
            .validate_for(request)
            .map_err(|_| ActionSubmissionErrorV1::InvalidOwnerResult)?;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationKindV1 {
    Action,
    Ceremony,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedOperationV1 {
    pub ordinal: u16,
    pub name: String,
    pub owner: CatalogOwnerV1,
    pub operation_spec: OperationSpecRefV1,
    pub material_dependency_stamp: [u8; 32],
    pub ceremony_context: Option<CeremonyContextKindV1>,
}

impl PreparedOperationV1 {
    fn from_entry(entry: &OperationCatalogEntryV1) -> Self {
        Self {
            ordinal: entry.ordinal(),
            name: entry.name().to_owned(),
            owner: entry.owner(),
            operation_spec: entry.operation_spec_ref(),
            material_dependency_stamp: entry.material_dependency_stamp(),
            ceremony_context: entry.ceremony_context(),
        }
    }
}

fn validate_catalog_binding(
    entry: &OperationCatalogEntryV1,
    request: &OperationRequestV1,
) -> Result<(), ActionSubmissionErrorV1> {
    if entry.operation_spec_ref()
        != match request {
            OperationRequestV1::Action(action) => {
                OperationSpecRefV1::Action(action.action_spec.clone())
            }
            OperationRequestV1::Ceremony(ceremony) => {
                OperationSpecRefV1::Ceremony(ceremony.ceremony_spec.clone())
            }
        }
    {
        return Err(ActionSubmissionErrorV1::CatalogBindingMismatch);
    }
    match request {
        OperationRequestV1::Action(action)
            if entry.material_dependency_stamp() != action.material_dependency_stamp =>
        {
            Err(ActionSubmissionErrorV1::MaterialDependencyMismatch)
        }
        OperationRequestV1::Ceremony(ceremony) => {
            let actual = match ceremony.context {
                CeremonyRequestContextV1::NoStore { .. } => CeremonyContextKindV1::NoStore,
                CeremonyRequestContextV1::PreStore { .. } => CeremonyContextKindV1::PreStore,
            };
            if entry.ceremony_context() == Some(actual) {
                Ok(())
            } else {
                Err(ActionSubmissionErrorV1::CeremonyContextMismatch)
            }
        }
        OperationRequestV1::Action(_) => Ok(()),
    }
}

fn has_frozen_owner_materialization(entry: &OperationCatalogEntryV1) -> bool {
    RepositoryActionLeafV1::ALL
        .iter()
        .any(|leaf| leaf.literal() == entry.name())
        || AuthorityActionLeafV1::ALL
            .iter()
            .any(|leaf| leaf.literal() == entry.name())
}

fn request_identity(request: &OperationRequestV1) -> (&str, [u8; 32], &str) {
    match request {
        OperationRequestV1::Action(action) => (
            &action.action_spec.exact_action_spec_ref,
            action.semantic_request_hash,
            &action.idempotency_key,
        ),
        OperationRequestV1::Ceremony(ceremony) => (
            &ceremony.ceremony_spec.exact_ceremony_spec_ref,
            ceremony.semantic_request_hash,
            &ceremony.idempotency_key,
        ),
    }
}

pub(crate) fn semantic_request_hash(request: &OperationRequestV1) -> [u8; 32] {
    let mut hash = SemanticHasherV1::new(b"maestro.vnext.operation-request.meaning.v1");
    match request {
        OperationRequestV1::Action(action) => hash_action(&mut hash, action),
        OperationRequestV1::Ceremony(ceremony) => hash_ceremony(&mut hash, ceremony),
    }
    hash.finish()
}

fn hash_action(hash: &mut SemanticHasherV1, action: &ActionRequestV1) {
    hash.u64(1);
    hash.u64(action.schema_version);
    hash.bytes(&action.selected_packet_semantic_hash);
    hash.text(&action.action_spec.exact_action_spec_ref);
    hash.text(&action.action_spec.exact_schema_id);
    hash.text(&action.action_spec.exact_core_catalog_ref);
    hash.text(&action.action_spec.exact_public_catalog_ref);
    hash.bytes(&action.material_dependency_stamp);
    hash.text(&action.exact_store_generation_ref);
    hash.text(&action.exact_authority_epoch_ref);
    hash.text(&action.valid_until_ref);
    hash_authority(hash, &action.authority_basis);
    hash.bytes(&action.typed_input_cbor);
    hash.strings(&action.evidence_refs);
    hash.strings(&action.prerequisite_receipt_refs);
    hash_attribution(hash, action.orchestration_attribution.as_ref());
}

fn hash_authority(hash: &mut SemanticHasherV1, authority: &ActionAuthorityBasisV1) {
    match authority {
        ActionAuthorityBasisV1::Ordinary {
            verified_principal_ref,
            current_session_ref,
            live_grant_refs,
            required_mandate_refs,
        } => {
            hash.u64(1);
            hash.text(verified_principal_ref);
            hash.text(current_session_ref);
            hash.strings(live_grant_refs);
            hash.strings(required_mandate_refs);
        }
        ActionAuthorityBasisV1::BootstrapControl {
            exact_bootstrap_scope_ref,
            current_executor_assertion_ref,
        } => {
            hash.u64(2);
            hash.text(exact_bootstrap_scope_ref);
            hash.text(current_executor_assertion_ref);
        }
        ActionAuthorityBasisV1::ContinuityMaintenance {
            exact_cma_branch_ref,
            maintenance_executor_assertion_ref,
            applicability_ref,
            phase_slot_ref,
        } => {
            hash.u64(3);
            hash.text(exact_cma_branch_ref);
            hash.text(maintenance_executor_assertion_ref);
            hash.text(applicability_ref);
            hash.text(phase_slot_ref);
        }
    }
}

fn hash_ceremony(hash: &mut SemanticHasherV1, ceremony: &CeremonyRequestV1) {
    hash.u64(2);
    hash.u64(ceremony.schema_version);
    hash.text(&ceremony.ceremony_spec.exact_ceremony_spec_ref);
    hash.text(&ceremony.ceremony_spec.exact_schema_id);
    hash.text(&ceremony.ceremony_spec.exact_core_catalog_ref);
    hash.text(&ceremony.ceremony_spec.exact_public_catalog_ref);
    hash.u64(match ceremony.request_mode {
        CeremonyRequestModeV1::Initiate => 1,
        CeremonyRequestModeV1::RecoverReserved => 2,
        CeremonyRequestModeV1::ResolveResult => 3,
        CeremonyRequestModeV1::Withdraw => 4,
    });
    match &ceremony.context {
        CeremonyRequestContextV1::NoStore {
            protected_realm_ref,
            genesis_candidate_ref,
        } => {
            hash.u64(1);
            hash.text(protected_realm_ref);
            hash.text(genesis_candidate_ref);
        }
        CeremonyRequestContextV1::PreStore {
            protected_carrier_ref,
            candidate_seal_ref,
            expected_old_token_ref,
        } => {
            hash.u64(2);
            hash.text(protected_carrier_ref);
            hash.text(candidate_seal_ref);
            hash.text(expected_old_token_ref);
        }
    }
    hash.text(&ceremony.branch_authority_ref);
    hash.text(&ceremony.expected_carrier_token_ref);
    hash.bytes(&ceremony.typed_input_cbor);
    hash.strings(&ceremony.prerequisite_receipt_refs);
    hash_attribution(hash, ceremony.orchestration_attribution.as_ref());
}

fn hash_attribution(hash: &mut SemanticHasherV1, attribution: Option<&OrchestrationAttributionV1>) {
    match attribution {
        None => hash.u64(0),
        Some(value) => {
            hash.u64(1);
            hash.text(&value.exact_packet_recipe_binding_ref);
            hash.text(&value.exact_application_ref);
            hash.u64(value.component_output_hashes.len() as u64);
            for component in &value.component_output_hashes {
                hash.bytes(component);
            }
            hash.bytes(&value.composed_advice_hash);
        }
    }
}

fn result_for(
    request: &OperationRequestV1,
    outcome: OperationSemanticOutcomeV1,
    inspect_ref: Option<String>,
    replayed_delivery: bool,
) -> OperationResultV1 {
    let (request_id, operation_spec_ref) = match request {
        OperationRequestV1::Action(action) => (
            action.request_id.clone(),
            action.action_spec.exact_action_spec_ref.clone(),
        ),
        OperationRequestV1::Ceremony(ceremony) => (
            ceremony.request_id.clone(),
            ceremony.ceremony_spec.exact_ceremony_spec_ref.clone(),
        ),
    };
    let body = OperationResultBodyV1 {
        schema_version: 1,
        request_id,
        operation_spec_ref,
        outcome,
        before_revision_refs: Vec::new(),
        after_revision_refs: Vec::new(),
        transition_receipt_refs: Vec::new(),
        produced_record_refs: Vec::new(),
        next_packet: None,
        inspect_ref,
        replayed_delivery,
    };
    match request {
        OperationRequestV1::Action(_) => OperationResultV1::Action(ActionResultV1(body)),
        OperationRequestV1::Ceremony(_) => OperationResultV1::Ceremony(CeremonyResultV1(body)),
    }
}

struct SemanticHasherV1(Sha256);

impl SemanticHasherV1 {
    fn new(domain: &[u8]) -> Self {
        let mut value = Sha256::new();
        value.update((domain.len() as u64).to_be_bytes());
        value.update(domain);
        Self(value)
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.0.update(value);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.u64(values.len() as u64);
        for value in values {
            self.text(value);
        }
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Debug, Error)]
pub(crate) enum ActionSubmissionErrorV1 {
    #[error("the Operation request is invalid")]
    InvalidRequest,
    #[error("the request semantic hash does not cover its exact meaning")]
    SemanticHashMismatch,
    #[error("the requested operation is not in the frozen generated catalog")]
    UnknownOperation,
    #[error("the request's exact schema or catalog references do not match the catalog")]
    CatalogBindingMismatch,
    #[error("the Action material dependency stamp is stale or mismatched")]
    MaterialDependencyMismatch,
    #[error("the Ceremony context is illegal for this frozen CeremonySpec")]
    CeremonyContextMismatch,
    #[error("the owner returned a durable result for a different admission")]
    InvalidOwnerBinding,
    #[error("the owner returned an invalid canonical Result")]
    InvalidOwnerResult,
    #[error("the semantic owner transport failed: {0}")]
    Owner(String),
    #[error(transparent)]
    Catalog(#[from] GeneratedCatalogErrorV1),
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    use super::*;

    struct ReplayPort {
        values: RefCell<BTreeMap<String, ([u8; 32], OperationResultBodyV1)>>,
        submissions: Cell<usize>,
        writes: RefCell<usize>,
    }

    impl GovernedOperationPortV1 for ReplayPort {
        fn submit(
            &self,
            request: &OperationRequestV1,
            admission: &OwnerAdmissionV1,
        ) -> Result<OwnerSubmissionOutcomeV1, ActionSubmissionErrorV1> {
            self.submissions.set(self.submissions.get() + 1);
            let mut values = self.values.borrow_mut();
            if let Some((meaning, body)) = values.get(&admission.idempotency_key) {
                return if meaning == &admission.semantic_request_hash {
                    Ok(OwnerSubmissionOutcomeV1::Durable(Box::new(
                        OwnerDurableResultV1::replay(admission.clone(), body.clone()),
                    )))
                } else {
                    Ok(OwnerSubmissionOutcomeV1::SameKeyDifferentMeaning)
                };
            }
            *self.writes.borrow_mut() += 1;
            let body = match result_for(request, OperationSemanticOutcomeV1::Committed, None, false)
            {
                OperationResultV1::Action(result) => result.0,
                OperationResultV1::Ceremony(result) => result.0,
            };
            values.insert(
                admission.idempotency_key.clone(),
                (admission.semantic_request_hash, body.clone()),
            );
            Ok(OwnerSubmissionOutcomeV1::Durable(Box::new(
                OwnerDurableResultV1::fresh(admission.clone(), body),
            )))
        }
    }

    struct DurableReadPort {
        rows: BTreeMap<String, (OperationRequestV1, OperationResultV1)>,
    }

    impl OperationResultReadPortV1 for DurableReadPort {
        fn read_result(
            &self,
            request_id: &str,
        ) -> Result<Option<(OperationRequestV1, OperationResultV1)>, ActionSubmissionErrorV1>
        {
            Ok(self.rows.get(request_id).cloned())
        }
    }

    fn action_request(
        entry: &OperationCatalogEntryV1,
        idempotency_key: &str,
    ) -> OperationRequestV1 {
        let OperationSpecRefV1::Action(action_spec) = entry.operation_spec_ref() else {
            panic!("action entry");
        };
        let mut request = OperationRequestV1::Action(ActionRequestV1 {
            schema_version: 1,
            request_id: format!("request-{}", entry.ordinal()),
            idempotency_key: idempotency_key.to_owned(),
            semantic_request_hash: [1; 32],
            selected_packet_semantic_hash: [2; 32],
            action_spec,
            material_dependency_stamp: entry.material_dependency_stamp(),
            exact_store_generation_ref: "candidate:store:generation:current".to_owned(),
            exact_authority_epoch_ref: "candidate:authority:epoch:current".to_owned(),
            valid_until_ref: "candidate:frontier:valid-until".to_owned(),
            authority_basis: ActionAuthorityBasisV1::Ordinary {
                verified_principal_ref: "candidate:principal:verified".to_owned(),
                current_session_ref: "candidate:session:current".to_owned(),
                live_grant_refs: Vec::new(),
                required_mandate_refs: Vec::new(),
            },
            typed_input_cbor: vec![0xa0],
            evidence_refs: Vec::new(),
            prerequisite_receipt_refs: Vec::new(),
            orchestration_attribution: None,
        });
        let meaning = semantic_request_hash(&request);
        if let OperationRequestV1::Action(action) = &mut request {
            action.semantic_request_hash = meaning;
        }
        request
    }

    fn replayed(result: &OperationResultV1) -> bool {
        match result {
            OperationResultV1::Action(result) => result.0.replayed_delivery,
            OperationResultV1::Ceremony(result) => result.0.replayed_delivery,
        }
    }

    #[test]
    fn durable_results_are_readable_by_request_id_through_the_read_port() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        let entry = service
            .catalog
            .actions()
            .iter()
            .find(|entry| has_frozen_owner_materialization(entry))
            .expect("materialized entry");
        let request = action_request(entry, "key-read");
        let result = service.submit(&port, &request).expect("submit");
        let request_id = format!("request-{}", entry.ordinal());
        let mut rows = BTreeMap::new();
        rows.insert(request_id.clone(), (request.clone(), result.clone()));
        let read_port = DurableReadPort { rows };
        let read = service
            .read_result(&read_port, &request_id)
            .expect("read")
            .expect("durable row present");
        assert_eq!(read, result);
        assert_eq!(
            service
                .read_result(&read_port, "request-absent")
                .expect("read"),
            None
        );
        let mut mismatched = BTreeMap::new();
        mismatched.insert("request-other".to_owned(), (request, result));
        let read_port = DurableReadPort { rows: mismatched };
        assert!(matches!(
            service.read_result(&read_port, "request-other"),
            Err(ActionSubmissionErrorV1::InvalidOwnerResult)
        ));
    }

    #[test]
    fn same_meaning_replay_with_a_new_request_id_is_delivered_as_replay() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        let entry = service
            .catalog
            .actions()
            .iter()
            .find(|entry| has_frozen_owner_materialization(entry))
            .expect("materialized entry");
        let first = action_request(entry, "key-replay");
        let result = service.submit(&port, &first).expect("fresh submit");
        assert!(!replayed(&result));
        let mut second = first.clone();
        let OperationRequestV1::Action(action) = &mut second else {
            panic!("action request");
        };
        action.request_id = "request-replay-retry".to_owned();
        assert_eq!(
            semantic_request_hash(&second),
            semantic_request_hash(&first)
        );
        let result = service.submit(&port, &second).expect("replayed submit");
        assert!(replayed(&result));
        let OperationResultV1::Action(result) = result else {
            panic!("action result");
        };
        assert_eq!(result.0.request_id, "request-replay-retry");
        assert_eq!(port.submissions.get(), 2);
        assert_eq!(*port.writes.borrow(), 1);
    }

    #[test]
    fn all_145_action_leaves_are_recognized_with_95_materialized_owner_delegations() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        assert_eq!(service.catalog.actions().len(), 145);
        for (index, entry) in service.catalog.actions().iter().enumerate() {
            assert_eq!(usize::from(entry.ordinal()), index + 1);
        }
        assert_eq!(
            service
                .catalog
                .actions()
                .iter()
                .filter(|entry| has_frozen_owner_materialization(entry))
                .count(),
            95
        );
    }

    #[test]
    fn downstream_tags_94_through_145_delegate_to_the_materialized_owner_port() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        let downstream = service
            .catalog
            .actions()
            .iter()
            .filter(|entry| (94..=145).contains(&entry.ordinal()))
            .collect::<Vec<_>>();
        assert_eq!(downstream.len(), 52);
        for entry in downstream {
            let request = action_request(entry, &format!("key-{}", entry.ordinal()));
            let result = service.submit(&port, &request).expect("submit");
            assert!(!replayed(&result));
        }
        assert_eq!(port.submissions.get(), 52);
        assert_eq!(*port.writes.borrow(), 52);
    }

    #[test]
    fn actions_without_a_frozen_owner_materialization_remain_unavailable() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        let unavailable = service
            .catalog
            .actions()
            .iter()
            .filter(|entry| !has_frozen_owner_materialization(entry))
            .collect::<Vec<_>>();
        assert_eq!(unavailable.len(), 50);
        for entry in unavailable {
            let request = action_request(entry, &format!("key-{}", entry.ordinal()));
            let result = service.submit(&port, &request).expect("submit");
            let OperationResultV1::Action(result) = result else {
                panic!("action result");
            };
            assert_eq!(result.0.outcome, OperationSemanticOutcomeV1::Unavailable);
            assert!(result.0.before_revision_refs.is_empty());
            assert!(result.0.after_revision_refs.is_empty());
            assert!(result.0.transition_receipt_refs.is_empty());
            assert!(result.0.produced_record_refs.is_empty());
        }
        assert_eq!(port.submissions.get(), 0);
        assert_eq!(*port.writes.borrow(), 0);
        assert!(port.values.borrow().is_empty());
    }

    #[test]
    fn all_actions_with_frozen_owner_materialization_delegate_to_the_owner_port() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        for entry in service
            .catalog
            .actions()
            .iter()
            .filter(|entry| has_frozen_owner_materialization(entry))
        {
            let request = action_request(entry, &format!("key-{}", entry.ordinal()));
            let result = service.submit(&port, &request).expect("submit");
            assert!(!replayed(&result));
        }
        assert_eq!(port.submissions.get(), 95);
        assert_eq!(*port.writes.borrow(), 95);
    }

    #[test]
    fn same_key_same_meaning_replays_without_a_second_write() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        let request = action_request(&service.catalog.actions()[0], "same-key");
        assert!(!replayed(&service.submit(&port, &request).expect("fresh")));
        assert!(replayed(&service.submit(&port, &request).expect("replay")));
        assert_eq!(*port.writes.borrow(), 1);
    }

    #[test]
    fn same_key_different_meaning_is_conflict() {
        let service = ActionSubmissionServiceV1::load().expect("service");
        let port = ReplayPort {
            values: RefCell::new(BTreeMap::new()),
            submissions: Cell::new(0),
            writes: RefCell::new(0),
        };
        let first = action_request(&service.catalog.actions()[0], "same-key");
        let second = action_request(&service.catalog.actions()[1], "same-key");
        service.submit(&port, &first).expect("fresh");
        let result = service.submit(&port, &second).expect("conflict");
        let OperationResultV1::Action(result) = result else {
            panic!("action result");
        };
        assert_eq!(result.0.outcome, OperationSemanticOutcomeV1::Conflict);
        assert_eq!(*port.writes.borrow(), 1);
    }
}
