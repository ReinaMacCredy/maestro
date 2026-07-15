use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::bootstrap_catalog::{BootstrapMandateTargetV1, BootstrapTargetDispositionV1};
use super::closed::AuthorityContextKindV1;
use super::context::AuthorityContextV1;
use super::continuity::GuardAdmissionKindV1;
use super::grant::{
    AuthorityUseConstraintV1, BootstrapG0PathV1, BootstrapGenesisGrantV1, GrantDefinitionV1,
    GrantScopeV1, HalfOpenValidityV1, ScopeAtomV1,
};
use super::identity::{
    ActionRequestIdV1, AuthorityBasisCommitmentIdV1, AuthorityContextIdV1,
    AuthorityContinuityManifestIdV1, AuthorityIdV1, AuthorityIdentityKindV1, CapacityRootIdV1,
    GenesisGrantIdV1, InteractionClosureIdV1, ObservationIdV1, PrincipalBindingIdV1, SessionIdV1,
    StateTokenIdV1, TargetActionCommitmentIdV1,
};
use super::mandate::{
    BootstrapMandateEvaluationV1, ConsentRequirementMemberV1, ConsentSlotBindingParameterV1,
    ConsentSlotDerivationErrorV1, IssueBootstrapMandateRequestV1, TargetActionEffectKindV1,
    TargetActionOwnerV1, TargetActionProjectionV1, TargetActionProtocolV1,
};
use super::principal::{
    AuthoritySnapshotV1, PrincipalBindingV1, RevocationSetV1, RevocationTargetV1, SessionV1,
    TrustedTimeV1,
};

const MAX_G0_CANDIDATE_PATHS: usize = 64;
const MAX_BOOTSTRAP_MANDATE_LIFETIME: u64 = 300;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapInteractionSubjectV1 {
    context_id: AuthorityContextIdV1,
    interaction_plan: StateTokenIdV1,
    interaction_attempt: ActionRequestIdV1,
    responder_binding_id: PrincipalBindingIdV1,
    responder_assurance_revision: u64,
    target_action_commitment: TargetActionCommitmentIdV1,
    consent_slot: ConsentSlotBindingParameterV1,
    option_mapping_commitment: StateTokenIdV1,
    affirmative_option_commitment: StateTokenIdV1,
}

impl BootstrapInteractionSubjectV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.bootstrap-mandate-interaction-subject.v1";

    #[expect(
        clippy::too_many_arguments,
        reason = "the canonical interaction join exposes every frozen identity explicitly"
    )]
    pub const fn new(
        context_id: AuthorityContextIdV1,
        interaction_plan: StateTokenIdV1,
        interaction_attempt: ActionRequestIdV1,
        responder_binding_id: PrincipalBindingIdV1,
        responder_assurance_revision: u64,
        target_action_commitment: TargetActionCommitmentIdV1,
        consent_slot: ConsentSlotBindingParameterV1,
        option_mapping_commitment: StateTokenIdV1,
        affirmative_option_commitment: StateTokenIdV1,
    ) -> Self {
        Self {
            context_id,
            interaction_plan,
            interaction_attempt,
            responder_binding_id,
            responder_assurance_revision,
            target_action_commitment,
            consent_slot,
            option_mapping_commitment,
            affirmative_option_commitment,
        }
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn interaction_plan(&self) -> StateTokenIdV1 {
        self.interaction_plan
    }

    pub const fn interaction_attempt(&self) -> ActionRequestIdV1 {
        self.interaction_attempt
    }

    pub const fn responder_binding_id(&self) -> PrincipalBindingIdV1 {
        self.responder_binding_id
    }

    pub const fn responder_assurance_revision(&self) -> u64 {
        self.responder_assurance_revision
    }

    pub const fn target_action_commitment(&self) -> TargetActionCommitmentIdV1 {
        self.target_action_commitment
    }

    pub const fn consent_slot(&self) -> &ConsentSlotBindingParameterV1 {
        &self.consent_slot
    }

    pub const fn option_mapping_commitment(&self) -> StateTokenIdV1 {
        self.option_mapping_commitment
    }

    pub const fn affirmative_option_commitment(&self) -> StateTokenIdV1 {
        self.affirmative_option_commitment
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            bytes(self.context_id),
            bytes(self.interaction_plan),
            bytes(self.interaction_attempt),
            bytes(self.responder_binding_id),
            CborValue::Unsigned(self.responder_assurance_revision),
            bytes(self.target_action_commitment),
            self.consent_slot.schema_value()?,
            bytes(self.option_mapping_commitment),
            bytes(self.affirmative_option_commitment),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapMandatePresentationObservationV1 {
    id: ObservationIdV1,
    subject: BootstrapInteractionSubjectV1,
    carrier_commitment: StateTokenIdV1,
    procedure_commitment: StateTokenIdV1,
}

impl BootstrapMandatePresentationObservationV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.bootstrap-mandate-presentation-observation.v1";

    pub fn new(
        subject: BootstrapInteractionSubjectV1,
        carrier_commitment: StateTokenIdV1,
        procedure_commitment: StateTokenIdV1,
    ) -> Result<Self, CborError> {
        let value = presentation_value(&subject, carrier_commitment, procedure_commitment)?;
        Ok(Self {
            id: ObservationIdV1::from_digest(hash_value(&value)?),
            subject,
            carrier_commitment,
            procedure_commitment,
        })
    }

    pub const fn id(&self) -> ObservationIdV1 {
        self.id
    }

    pub const fn subject(&self) -> &BootstrapInteractionSubjectV1 {
        &self.subject
    }

    pub const fn carrier_commitment(&self) -> StateTokenIdV1 {
        self.carrier_commitment
    }

    pub const fn procedure_commitment(&self) -> StateTokenIdV1 {
        self.procedure_commitment
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        presentation_value(
            &self.subject,
            self.carrier_commitment,
            self.procedure_commitment,
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let observation = parse_presentation(&value)?;
        if observation.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(observation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BootstrapResponseDispositionV1 {
    Affirmative = 1,
    Declined = 2,
    Ambiguous = 3,
}

impl TryFrom<u8> for BootstrapResponseDispositionV1 {
    type Error = BootstrapAuthoritySnapshotErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Affirmative),
            2 => Ok(Self::Declined),
            3 => Ok(Self::Ambiguous),
            _ => Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapMandateResponseObservationV1 {
    id: ObservationIdV1,
    subject: BootstrapInteractionSubjectV1,
    presentation_observation_id: ObservationIdV1,
    disposition: BootstrapResponseDispositionV1,
    selected_option_commitment: StateTokenIdV1,
}

impl BootstrapMandateResponseObservationV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.bootstrap-mandate-response-observation.v1";

    pub fn new(
        subject: BootstrapInteractionSubjectV1,
        presentation_observation_id: ObservationIdV1,
        disposition: BootstrapResponseDispositionV1,
        selected_option_commitment: StateTokenIdV1,
    ) -> Result<Self, CborError> {
        let value = response_value(
            &subject,
            presentation_observation_id,
            disposition,
            selected_option_commitment,
        )?;
        Ok(Self {
            id: ObservationIdV1::from_digest(hash_value(&value)?),
            subject,
            presentation_observation_id,
            disposition,
            selected_option_commitment,
        })
    }

    pub const fn id(&self) -> ObservationIdV1 {
        self.id
    }

    pub const fn subject(&self) -> &BootstrapInteractionSubjectV1 {
        &self.subject
    }

    pub const fn presentation_observation_id(&self) -> ObservationIdV1 {
        self.presentation_observation_id
    }

    pub const fn disposition(&self) -> BootstrapResponseDispositionV1 {
        self.disposition
    }

    pub const fn selected_option_commitment(&self) -> StateTokenIdV1 {
        self.selected_option_commitment
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        response_value(
            &self.subject,
            self.presentation_observation_id,
            self.disposition,
            self.selected_option_commitment,
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let observation = parse_response(&value)?;
        if observation.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(observation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapMandateInteractionObservationJoinV1 {
    interaction_closure_id: InteractionClosureIdV1,
    context_id: AuthorityContextIdV1,
    responder_binding_id: PrincipalBindingIdV1,
    responder_current_authentication_ref: SessionIdV1,
    presentation_observation_id: ObservationIdV1,
    affirmative_response_observation_id: ObservationIdV1,
    carrier_procedure_ref: StateTokenIdV1,
    target_action_commitment: TargetActionCommitmentIdV1,
}

impl BootstrapMandateInteractionObservationJoinV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.bootstrap-mandate-interaction-observation-join-value.v1";

    pub fn new(
        presentation: &BootstrapMandatePresentationObservationV1,
        response: &BootstrapMandateResponseObservationV1,
        responder_current_authentication_ref: SessionIdV1,
        carrier_procedure_ref: StateTokenIdV1,
    ) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        if response.presentation_observation_id() != presentation.id()
            || response.subject() != presentation.subject()
            || response.disposition() != BootstrapResponseDispositionV1::Affirmative
            || response.selected_option_commitment()
                != presentation.subject().affirmative_option_commitment()
            || presentation.procedure_commitment() != carrier_procedure_ref
        {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(Self {
            interaction_closure_id: interaction_closure(presentation, response)?,
            context_id: presentation.subject().context_id(),
            responder_binding_id: presentation.subject().responder_binding_id(),
            responder_current_authentication_ref,
            presentation_observation_id: presentation.id(),
            affirmative_response_observation_id: response.id(),
            carrier_procedure_ref,
            target_action_commitment: presentation.subject().target_action_commitment(),
        })
    }

    pub const fn interaction_closure_id(&self) -> InteractionClosureIdV1 {
        self.interaction_closure_id
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn responder_binding_id(&self) -> PrincipalBindingIdV1 {
        self.responder_binding_id
    }

    pub const fn responder_current_authentication_ref(&self) -> SessionIdV1 {
        self.responder_current_authentication_ref
    }

    pub const fn presentation_observation_id(&self) -> ObservationIdV1 {
        self.presentation_observation_id
    }

    pub const fn affirmative_response_observation_id(&self) -> ObservationIdV1 {
        self.affirmative_response_observation_id
    }

    pub const fn carrier_procedure_ref(&self) -> StateTokenIdV1 {
        self.carrier_procedure_ref
    }

    pub const fn target_action_commitment(&self) -> TargetActionCommitmentIdV1 {
        self.target_action_commitment
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            bytes(self.interaction_closure_id),
            bytes(self.context_id),
            bytes(self.responder_binding_id),
            bytes(self.responder_current_authentication_ref),
            bytes(self.presentation_observation_id),
            bytes(self.affirmative_response_observation_id),
            bytes(self.carrier_procedure_ref),
            bytes(self.target_action_commitment),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let join = parse_interaction_join(&value)?;
        if join.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(join)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentSlotEvaluationFactsV1 {
    binding: ConsentSlotBindingParameterV1,
    requirement_members: Vec<ConsentRequirementMemberV1>,
    validity: HalfOpenValidityV1,
}

impl ConsentSlotEvaluationFactsV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.consent-slot-evaluation-facts.v1";

    pub fn derive_for_target(
        target: &TargetActionProjectionV1,
        validity: HalfOpenValidityV1,
    ) -> Result<Self, ConsentSlotDerivationErrorV1> {
        let member = ConsentRequirementMemberV1::derive_for_target(target)?;
        let binding = ConsentSlotBindingParameterV1::derive(target, &member)?;
        Ok(Self {
            binding,
            requirement_members: vec![member],
            validity,
        })
    }

    pub(crate) fn from_parts(
        binding: ConsentSlotBindingParameterV1,
        requirement_members: Vec<ConsentRequirementMemberV1>,
        validity: HalfOpenValidityV1,
    ) -> Self {
        Self {
            binding,
            requirement_members,
            validity,
        }
    }

    pub const fn binding(&self) -> &ConsentSlotBindingParameterV1 {
        &self.binding
    }

    pub fn requirement_members(&self) -> &[ConsentRequirementMemberV1] {
        &self.requirement_members
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            self.binding.schema_value()?,
            CborValue::Array(
                self.requirement_members
                    .iter()
                    .map(ConsentRequirementMemberV1::schema_value)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            validity_value(self.validity),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityRevocationSetV1 {
    context_id: AuthorityContextIdV1,
    revocations: RevocationSetV1,
}

impl AuthorityRevocationSetV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.revocation-set-value.v1";

    pub const fn new(context_id: AuthorityContextIdV1, revocations: RevocationSetV1) -> Self {
        Self {
            context_id,
            revocations,
        }
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn revocations(&self) -> &RevocationSetV1 {
        &self.revocations
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        revocations_value(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let revocations = parse_revocations(&value)?;
        if revocations.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(revocations)
    }
}

impl PrincipalBindingV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.principal-binding-value.v1";

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        binding_value(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let binding = parse_binding(&value)?;
        if binding.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(binding)
    }
}

impl SessionV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.session-value.v1";

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        session_value(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let session = parse_session(&value)?;
        if session.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(session)
    }
}

impl AuthoritySnapshotV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.authority-snapshot-value.v1";

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        snapshot_value(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let snapshot = parse_snapshot(&value)?;
        if snapshot.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(snapshot)
    }
}

impl BootstrapGenesisGrantV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.bootstrap-genesis-grant-value.v1";

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        bootstrap_genesis_grant_value(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let grant = parse_bootstrap_genesis_grant(&value)?;
        if grant.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(grant)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapContinuityTransitionProofV1 {
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    authority_epoch: u64,
    trust_root_revision: u64,
    manifest_id: AuthorityContinuityManifestIdV1,
    guard_kind: GuardAdmissionKindV1,
    state_token: StateTokenIdV1,
    validity: HalfOpenValidityV1,
}

impl BootstrapContinuityTransitionProofV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.bootstrap-continuity-transition-proof.v1";

    #[expect(
        clippy::too_many_arguments,
        reason = "the Store-derived proof binds every current head and both proof dispositions"
    )]
    pub const fn new(
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        trust_root_revision: u64,
        manifest_id: AuthorityContinuityManifestIdV1,
        guard_kind: GuardAdmissionKindV1,
        state_token: StateTokenIdV1,
        validity: HalfOpenValidityV1,
    ) -> Self {
        Self {
            context_id,
            store_generation,
            authority_epoch,
            trust_root_revision,
            manifest_id,
            guard_kind,
            state_token,
            validity,
        }
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn store_generation(&self) -> u64 {
        self.store_generation
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub const fn trust_root_revision(&self) -> u64 {
        self.trust_root_revision
    }

    pub const fn manifest_id(&self) -> AuthorityContinuityManifestIdV1 {
        self.manifest_id
    }

    pub const fn guard_kind(&self) -> GuardAdmissionKindV1 {
        self.guard_kind
    }

    pub const fn state_token(&self) -> StateTokenIdV1 {
        self.state_token
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            bytes(self.context_id),
            CborValue::Unsigned(self.store_generation),
            CborValue::Unsigned(self.authority_epoch),
            CborValue::Unsigned(self.trust_root_revision),
            bytes(self.manifest_id),
            CborValue::Unsigned(match self.guard_kind {
                GuardAdmissionKindV1::ExternallyRootedContextGenesis => 0,
                GuardAdmissionKindV1::Established(kind) => kind as u64,
            }),
            bytes(self.state_token),
            validity_value(self.validity),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapAuthoritySnapshotV1 {
    context: AuthorityContextV1,
    snapshot: AuthoritySnapshotV1,
    actor_binding: PrincipalBindingV1,
    actor_session: SessionV1,
    responder_binding: PrincipalBindingV1,
    responder_session: SessionV1,
    g0_candidate_paths: Vec<BootstrapG0PathV1>,
    revocations: AuthorityRevocationSetV1,
    interaction_join: Option<BootstrapMandateInteractionObservationJoinV1>,
    current_carrier_procedure_ref: StateTokenIdV1,
    target: TargetActionProjectionV1,
    current_target_head: StateTokenIdV1,
    consent_slot: ConsentSlotEvaluationFactsV1,
    continuity: BootstrapContinuityTransitionProofV1,
}

impl BootstrapAuthoritySnapshotV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.bootstrap-authority-snapshot.v1";

    #[expect(
        clippy::too_many_arguments,
        reason = "the canonical snapshot is a complete Store-independent evaluator fact carrier"
    )]
    pub fn new(
        context: AuthorityContextV1,
        snapshot: AuthoritySnapshotV1,
        actor_binding: PrincipalBindingV1,
        actor_session: SessionV1,
        responder_binding: PrincipalBindingV1,
        responder_session: SessionV1,
        g0_candidate_paths: Vec<BootstrapG0PathV1>,
        revocations: AuthorityRevocationSetV1,
        interaction_join: Option<BootstrapMandateInteractionObservationJoinV1>,
        current_carrier_procedure_ref: StateTokenIdV1,
        target: TargetActionProjectionV1,
        current_target_head: StateTokenIdV1,
        consent_slot: ConsentSlotEvaluationFactsV1,
        continuity: BootstrapContinuityTransitionProofV1,
    ) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        if g0_candidate_paths.len() > MAX_G0_CANDIDATE_PATHS {
            return Err(BootstrapAuthoritySnapshotErrorV1::CandidatePathBoundExceeded);
        }
        Ok(Self {
            context,
            snapshot,
            actor_binding,
            actor_session,
            responder_binding,
            responder_session,
            g0_candidate_paths,
            revocations,
            interaction_join,
            current_carrier_procedure_ref,
            target,
            current_target_head,
            consent_slot,
            continuity,
        })
    }

    pub const fn context(&self) -> &AuthorityContextV1 {
        &self.context
    }

    pub const fn snapshot(&self) -> &AuthoritySnapshotV1 {
        &self.snapshot
    }

    pub const fn actor_binding(&self) -> &PrincipalBindingV1 {
        &self.actor_binding
    }

    pub const fn actor_session(&self) -> &SessionV1 {
        &self.actor_session
    }

    pub const fn responder_binding(&self) -> &PrincipalBindingV1 {
        &self.responder_binding
    }

    pub const fn responder_session(&self) -> &SessionV1 {
        &self.responder_session
    }

    pub fn g0_candidate_paths(&self) -> &[BootstrapG0PathV1] {
        &self.g0_candidate_paths
    }

    pub const fn revocations(&self) -> &AuthorityRevocationSetV1 {
        &self.revocations
    }

    pub const fn interaction_join(&self) -> Option<&BootstrapMandateInteractionObservationJoinV1> {
        self.interaction_join.as_ref()
    }

    pub const fn current_carrier_procedure_ref(&self) -> StateTokenIdV1 {
        self.current_carrier_procedure_ref
    }

    pub const fn target(&self) -> &TargetActionProjectionV1 {
        &self.target
    }

    pub const fn current_target_head(&self) -> StateTokenIdV1 {
        self.current_target_head
    }

    pub const fn consent_slot(&self) -> &ConsentSlotEvaluationFactsV1 {
        &self.consent_slot
    }

    pub const fn continuity(&self) -> &BootstrapContinuityTransitionProofV1 {
        &self.continuity
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            self.context.schema_value()?,
            snapshot_value(&self.snapshot)?,
            binding_value(&self.actor_binding)?,
            session_value(&self.actor_session)?,
            binding_value(&self.responder_binding)?,
            session_value(&self.responder_session)?,
            CborValue::Array(
                self.g0_candidate_paths
                    .iter()
                    .map(g0_path_value)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            revocations_value(&self.revocations)?,
            CborValue::optional(
                self.interaction_join
                    .as_ref()
                    .map(BootstrapMandateInteractionObservationJoinV1::schema_value)
                    .transpose()?,
            ),
            bytes(self.current_carrier_procedure_ref),
            self.target.schema_value()?,
            bytes(self.current_target_head),
            self.consent_slot.schema_value()?,
            self.continuity.schema_value()?,
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub(crate) fn continue_at_store_generation(
        &self,
        store_generation: u64,
        manifest_id: AuthorityContinuityManifestIdV1,
        guard_kind: GuardAdmissionKindV1,
        state_token: StateTokenIdV1,
    ) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        if self.snapshot.store_generation.checked_add(1) != Some(store_generation) {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        let context = self.context.continue_at_store_generation(store_generation);
        let snapshot = AuthoritySnapshotV1::new(
            self.snapshot.context_id,
            store_generation,
            self.snapshot.authority_epoch,
            self.snapshot.trust_root_revision,
            self.snapshot.subject_revision,
            self.snapshot.trusted_time,
        );
        let actor_session = self
            .actor_session
            .continue_at_store_generation(store_generation);
        let responder_session = self
            .responder_session
            .continue_at_store_generation(store_generation);
        let g0_candidate_paths = self
            .g0_candidate_paths
            .iter()
            .map(|path| path.continue_at_store_generation(store_generation))
            .collect();
        let continuity = BootstrapContinuityTransitionProofV1::new(
            self.snapshot.context_id,
            store_generation,
            self.snapshot.authority_epoch,
            self.snapshot.trust_root_revision,
            manifest_id,
            guard_kind,
            state_token,
            self.continuity.validity(),
        );
        Self::new(
            context,
            snapshot,
            self.actor_binding.clone(),
            actor_session,
            self.responder_binding.clone(),
            responder_session,
            g0_candidate_paths,
            self.revocations.clone(),
            self.interaction_join.clone(),
            self.current_carrier_procedure_ref,
            self.target.clone(),
            self.current_target_head,
            self.consent_slot.clone(),
            continuity,
        )
    }

    pub(crate) fn continue_at_store_generation_with_revoked_grant(
        &self,
        store_generation: u64,
        manifest_id: AuthorityContinuityManifestIdV1,
        guard_kind: GuardAdmissionKindV1,
        state_token: StateTokenIdV1,
        grant_id: super::GrantIdV1,
    ) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let mut successor = self.continue_at_store_generation(
            store_generation,
            manifest_id,
            guard_kind,
            state_token,
        )?;
        let mut targets = successor
            .revocations
            .revocations
            .targets()
            .collect::<Vec<_>>();
        targets.push(RevocationTargetV1::Grant(grant_id));
        successor.revocations = AuthorityRevocationSetV1::new(
            successor.context.context_id(),
            RevocationSetV1::new(targets)
                .map_err(|_| BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot)?,
        );
        Ok(successor)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BootstrapAuthoritySnapshotErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let fields = exact_array(&value, 15)?;
        require_domain(&fields[0], Self::SCHEMA_DOMAIN)?;
        let context = parse_context(&fields[1])?;
        let snapshot = parse_snapshot(&fields[2])?;
        let actor_binding = parse_binding(&fields[3])?;
        let actor_session = parse_session(&fields[4])?;
        let responder_binding = parse_binding(&fields[5])?;
        let responder_session = parse_session(&fields[6])?;
        let paths = as_array(&fields[7])?
            .iter()
            .map(parse_g0_path)
            .collect::<Result<Vec<_>, _>>()?;
        let revocations = parse_revocations(&fields[8])?;
        let interaction_join = parse_optional(&fields[9], parse_interaction_join)?;
        let current_carrier_procedure_ref = parse_id(&fields[10])?;
        let target = TargetActionProjectionV1::from_schema_value(fields[11].clone())
            .map_err(|_| BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot)?;
        let current_target_head = parse_id(&fields[12])?;
        let consent_slot = parse_consent_slot_facts(&fields[13])?;
        let continuity = parse_continuity(&fields[14])?;
        let snapshot = Self::new(
            context,
            snapshot,
            actor_binding,
            actor_session,
            responder_binding,
            responder_session,
            paths,
            revocations,
            interaction_join,
            current_carrier_procedure_ref,
            target,
            current_target_head,
            consent_slot,
            continuity,
        )?;
        if snapshot.canonical_bytes()? != bytes {
            return Err(BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot);
        }
        Ok(snapshot)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthorityEvaluatorV1;

impl AuthorityEvaluatorV1 {
    pub const ISSUE_BOOTSTRAP_MANDATE_PROTOCOL_REVISION: u64 = 1;

    pub fn evaluate_bootstrap_mandate(
        request: IssueBootstrapMandateRequestV1,
        facts: &BootstrapAuthoritySnapshotV1,
    ) -> Result<BootstrapMandateEvaluationV1, AuthorityEvaluationErrorV1> {
        validate_context(facts)?;
        let target_commitment = validate_target(&request, facts)?;
        validate_principal(
            facts,
            &facts.actor_binding,
            &facts.actor_session,
            request.actor_binding_id(),
            request.actor_session_id(),
            false,
            &target_commitment.render(),
        )?;
        validate_principal(
            facts,
            &facts.responder_binding,
            &facts.responder_session,
            request.responder_binding_id(),
            facts.responder_session.id(),
            true,
            &target_commitment.render(),
        )?;
        let g0 = validate_g0(&request, facts, target_commitment)?;
        let interaction_join = validate_interaction(&request, facts, target_commitment)?;
        validate_continuity(facts)?;
        let validity = evaluation_validity(facts, g0)?;
        let authority_basis_commitment_id = AuthorityBasisCommitmentIdV1::from_digest(hash_value(
            &semantic_authority_basis_value(facts, target_commitment, g0, interaction_join)?,
        )?);
        Ok(BootstrapMandateEvaluationV1::seal(
            request,
            facts.responder_binding.assurance_revision(),
            interaction_join.interaction_closure_id(),
            authority_basis_commitment_id,
            validity,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AuthorityEvaluationErrorV1 {
    #[error("bootstrap authority unavailable")]
    Unavailable,
}

impl From<CborError> for AuthorityEvaluationErrorV1 {
    fn from(_: CborError) -> Self {
        Self::Unavailable
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BootstrapAuthoritySnapshotErrorV1 {
    #[error("bootstrap Authority snapshot exceeds the finite candidate-path bound")]
    CandidatePathBoundExceeded,
    #[error("invalid canonical bootstrap Authority snapshot")]
    InvalidCanonicalSnapshot,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

fn validate_context(
    facts: &BootstrapAuthoritySnapshotV1,
) -> Result<(), AuthorityEvaluationErrorV1> {
    let context = &facts.context;
    let snapshot = &facts.snapshot;
    require(
        context.context_id() == snapshot.context_id
            && context.store_generation() == snapshot.store_generation
            && context.authority_epoch() == snapshot.authority_epoch
            && context.trust_root_revision() == snapshot.trust_root_revision
            && facts.revocations.context_id() == snapshot.context_id,
    )
}

fn semantic_authority_basis_value(
    facts: &BootstrapAuthoritySnapshotV1,
    target_commitment: TargetActionCommitmentIdV1,
    g0: &BootstrapG0PathV1,
    interaction: &BootstrapMandateInteractionObservationJoinV1,
) -> Result<CborValue, CborError> {
    let session_semantics = |session: &SessionV1| -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            bytes(session.id()),
            bytes(session.binding_id()),
            bytes(session.context_id()),
            CborValue::Unsigned(session.authority_epoch()),
            CborValue::text(session.request_commitment())?,
            validity_value(session.validity()),
        ]))
    };
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.bootstrap-authority-basis-commitment.v1")?,
        bytes(facts.snapshot.context_id),
        CborValue::Unsigned(facts.snapshot.authority_epoch),
        CborValue::Unsigned(facts.snapshot.trust_root_revision),
        CborValue::Unsigned(facts.snapshot.subject_revision),
        trusted_time_value(facts.snapshot.trusted_time),
        binding_value(&facts.actor_binding)?,
        session_semantics(&facts.actor_session)?,
        binding_value(&facts.responder_binding)?,
        session_semantics(&facts.responder_session)?,
        g0.genesis_grant().schema_value()?,
        CborValue::Bool(g0.complete()),
        CborValue::Array(g0.root_contributions().iter().copied().map(bytes).collect()),
        revocations_value(&facts.revocations)?,
        interaction.schema_value()?,
        bytes(facts.current_carrier_procedure_ref),
        bytes(target_commitment),
        bytes(facts.current_target_head),
        facts.consent_slot.schema_value()?,
        bytes(facts.continuity.manifest_id()),
        CborValue::Unsigned(match facts.continuity.guard_kind() {
            GuardAdmissionKindV1::ExternallyRootedContextGenesis => 0,
            GuardAdmissionKindV1::Established(kind) => kind as u64,
        }),
    ]))
}

fn validate_principal(
    facts: &BootstrapAuthoritySnapshotV1,
    binding: &PrincipalBindingV1,
    session: &SessionV1,
    expected_binding_id: PrincipalBindingIdV1,
    expected_session_id: SessionIdV1,
    require_human: bool,
    request_commitment: &str,
) -> Result<(), AuthorityEvaluationErrorV1> {
    let snapshot = &facts.snapshot;
    require(
        binding.id() == expected_binding_id
            && session.id() == expected_session_id
            && binding.context_id() == snapshot.context_id
            && session.context_id() == snapshot.context_id
            && binding.trust_root_revision() == snapshot.trust_root_revision
            && session.binding_id() == binding.id()
            && session.store_generation() == snapshot.store_generation
            && session.authority_epoch() == snapshot.authority_epoch
            && session.request_commitment() == request_commitment
            && (!require_human || binding.human_capable())
            && !facts
                .revocations
                .revocations()
                .contains(RevocationTargetV1::TrustRoot(snapshot.trust_root_revision))
            && !facts
                .revocations
                .revocations()
                .contains(RevocationTargetV1::PrincipalBinding(binding.id()))
            && !facts
                .revocations
                .revocations()
                .contains(RevocationTargetV1::Session(session.id()))
            && snapshot
                .trusted_time
                .is_within(binding.validity())
                .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?
            && snapshot
                .trusted_time
                .is_within(session.validity())
                .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?,
    )
}

fn validate_target(
    request: &IssueBootstrapMandateRequestV1,
    facts: &BootstrapAuthoritySnapshotV1,
) -> Result<TargetActionCommitmentIdV1, AuthorityEvaluationErrorV1> {
    let target = &facts.target;
    let expected_heads = target.expected_heads();
    let expected_effect = match target.target() {
        BootstrapMandateTargetV1::EnrollRecoveryCommitmentSelection => {
            TargetActionEffectKindV1::Enroll
        }
        BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection => {
            TargetActionEffectKindV1::Rotate
        }
        BootstrapMandateTargetV1::RevokeRecoveryCommitmentSelection => {
            TargetActionEffectKindV1::Revoke
        }
        _ => return Err(AuthorityEvaluationErrorV1::Unavailable),
    };
    let commitment = target
        .target_action_commitment()
        .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?;
    let derived_slot =
        ConsentSlotEvaluationFactsV1::derive_for_target(target, facts.consent_slot.validity())
            .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?;
    require(
        target.target().disposition() == BootstrapTargetDispositionV1::Admitted
            && target.target() == request.target()
            && target.target_subject() == request.target_subject()
            && target.target_revision() == request.target_revision()
            && target.owner() == TargetActionOwnerV1::Authority
            && target.protocol() == TargetActionProtocolV1::RecoveryCommitmentSelection
            && target.effect_kind() == expected_effect
            && expected_heads.context_id() == facts.snapshot.context_id
            && expected_heads.store_generation() <= facts.snapshot.store_generation
            && expected_heads.authority_epoch() == facts.snapshot.authority_epoch
            && expected_heads.trust_root_revision() == facts.snapshot.trust_root_revision
            && expected_heads.subject_revision() == facts.snapshot.subject_revision
            && expected_heads.subject_revision() == target.target_revision()
            && expected_heads.target_head() == facts.current_target_head
            && request.consent_slot().target_action_commitment() == commitment
            && facts.consent_slot.binding() == request.consent_slot()
            && facts.consent_slot == derived_slot
            && facts
                .snapshot
                .trusted_time
                .is_within(target.validity())
                .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?
            && facts
                .snapshot
                .trusted_time
                .is_within(facts.consent_slot.validity())
                .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?,
    )?;
    Ok(commitment)
}

fn validate_g0<'a>(
    request: &IssueBootstrapMandateRequestV1,
    facts: &'a BootstrapAuthoritySnapshotV1,
    target_commitment: TargetActionCommitmentIdV1,
) -> Result<&'a BootstrapG0PathV1, AuthorityEvaluationErrorV1> {
    let [path] = facts.g0_candidate_paths.as_slice() else {
        return Err(AuthorityEvaluationErrorV1::Unavailable);
    };
    let grant = path.grant();
    let required_atom = ScopeAtomV1::new(
        "IssueBootstrapMandate",
        &target_commitment.render(),
        AuthorityEvaluatorV1::ISSUE_BOOTSTRAP_MANDATE_PROTOCOL_REVISION,
    )
    .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?;
    require(
        path.complete()
            && path.root_contributions().is_empty()
            && path.store_generation() == facts.snapshot.store_generation
            && path.authority_epoch() == facts.snapshot.authority_epoch
            && path.trust_root_revision() == facts.snapshot.trust_root_revision
            && grant.context_id() == request.context_id()
            && grant.grantee_principal_id() == facts.actor_binding.principal_id()
            && grant.parent_grant_id().is_none()
            && grant.delegation_id().is_none()
            && grant.authority_use_constraint() == AuthorityUseConstraintV1::NoLocalBoundedRoot
            && grant.terminal_scope().contains(&required_atom)
            && !facts
                .revocations
                .revocations()
                .contains(RevocationTargetV1::Grant(grant.id()))
            && facts
                .snapshot
                .trusted_time
                .is_within(grant.validity())
                .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?,
    )?;
    Ok(path)
}

fn validate_interaction<'a>(
    request: &IssueBootstrapMandateRequestV1,
    facts: &'a BootstrapAuthoritySnapshotV1,
    target_commitment: TargetActionCommitmentIdV1,
) -> Result<&'a BootstrapMandateInteractionObservationJoinV1, AuthorityEvaluationErrorV1> {
    let join = facts
        .interaction_join
        .as_ref()
        .ok_or(AuthorityEvaluationErrorV1::Unavailable)?;
    require(
        join.presentation_observation_id() == request.presentation_observation_id()
            && join.affirmative_response_observation_id() == request.response_observation_id()
            && join.context_id() == facts.snapshot.context_id
            && join.responder_binding_id() == facts.responder_binding.id()
            && join.responder_current_authentication_ref() == facts.responder_session.id()
            && join.carrier_procedure_ref() == facts.current_carrier_procedure_ref
            && join.target_action_commitment() == target_commitment,
    )?;
    Ok(join)
}

fn validate_continuity(
    facts: &BootstrapAuthoritySnapshotV1,
) -> Result<(), AuthorityEvaluationErrorV1> {
    let proof = &facts.continuity;
    require(
        proof.context_id() == facts.snapshot.context_id
            && proof.store_generation() == facts.snapshot.store_generation
            && proof.authority_epoch() == facts.snapshot.authority_epoch
            && proof.trust_root_revision() == facts.snapshot.trust_root_revision
            && facts
                .snapshot
                .trusted_time
                .is_within(proof.validity())
                .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)?,
    )
}

fn evaluation_validity(
    facts: &BootstrapAuthoritySnapshotV1,
    g0: &BootstrapG0PathV1,
) -> Result<HalfOpenValidityV1, AuthorityEvaluationErrorV1> {
    let TrustedTimeV1::Verified {
        lower_bound,
        upper_bound,
    } = facts.snapshot.trusted_time
    else {
        return Err(AuthorityEvaluationErrorV1::Unavailable);
    };
    let not_before = lower_bound;
    let short_lived = upper_bound
        .checked_add(MAX_BOOTSTRAP_MANDATE_LIFETIME)
        .ok_or(AuthorityEvaluationErrorV1::Unavailable)?;
    let expires_at = [
        g0.grant().validity().expires_at(),
        facts.actor_binding.validity().expires_at(),
        facts.actor_session.validity().expires_at(),
        facts.responder_binding.validity().expires_at(),
        facts.responder_session.validity().expires_at(),
        facts.target.validity().expires_at(),
        facts.consent_slot.validity().expires_at(),
        facts.continuity.validity().expires_at(),
        short_lived,
    ]
    .into_iter()
    .min()
    .ok_or(AuthorityEvaluationErrorV1::Unavailable)?;
    HalfOpenValidityV1::new(not_before, expires_at)
        .map_err(|_| AuthorityEvaluationErrorV1::Unavailable)
}

fn interaction_closure(
    presentation: &BootstrapMandatePresentationObservationV1,
    response: &BootstrapMandateResponseObservationV1,
) -> Result<InteractionClosureIdV1, CborError> {
    Ok(InteractionClosureIdV1::from_digest(hash_value(
        &CborValue::Array(vec![
            CborValue::text("maestro.vnext.bootstrap-mandate-interaction-closure.v1")?,
            presentation.schema_value()?,
            response.schema_value()?,
        ]),
    )?))
}

fn require(value: bool) -> Result<(), AuthorityEvaluationErrorV1> {
    if value {
        Ok(())
    } else {
        Err(AuthorityEvaluationErrorV1::Unavailable)
    }
}

fn presentation_value(
    subject: &BootstrapInteractionSubjectV1,
    carrier_commitment: StateTokenIdV1,
    procedure_commitment: StateTokenIdV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(BootstrapMandatePresentationObservationV1::SCHEMA_DOMAIN)?,
        subject.schema_value()?,
        bytes(carrier_commitment),
        bytes(procedure_commitment),
    ]))
}

fn response_value(
    subject: &BootstrapInteractionSubjectV1,
    presentation_observation_id: ObservationIdV1,
    disposition: BootstrapResponseDispositionV1,
    selected_option_commitment: StateTokenIdV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(BootstrapMandateResponseObservationV1::SCHEMA_DOMAIN)?,
        subject.schema_value()?,
        bytes(presentation_observation_id),
        CborValue::Unsigned(disposition as u64),
        bytes(selected_option_commitment),
    ]))
}

fn snapshot_value(snapshot: &AuthoritySnapshotV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(AuthoritySnapshotV1::SCHEMA_DOMAIN)?,
        bytes(snapshot.context_id),
        CborValue::Unsigned(snapshot.store_generation),
        CborValue::Unsigned(snapshot.authority_epoch),
        CborValue::Unsigned(snapshot.trust_root_revision),
        CborValue::Unsigned(snapshot.subject_revision),
        trusted_time_value(snapshot.trusted_time),
    ]))
}

fn trusted_time_value(time: TrustedTimeV1) -> CborValue {
    match time {
        TrustedTimeV1::Verified {
            lower_bound,
            upper_bound,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Unsigned(lower_bound),
            CborValue::Unsigned(upper_bound),
        ]),
        TrustedTimeV1::Unavailable => CborValue::Array(vec![CborValue::Unsigned(2)]),
    }
}

fn binding_value(binding: &PrincipalBindingV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(PrincipalBindingV1::SCHEMA_DOMAIN)?,
        bytes(binding.id()),
        bytes(binding.principal_id()),
        bytes(binding.context_id()),
        CborValue::Unsigned(binding.trust_root_revision()),
        CborValue::Unsigned(binding.assurance_revision()),
        validity_value(binding.validity()),
        CborValue::Bool(binding.human_capable()),
    ]))
}

fn session_value(session: &SessionV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(SessionV1::SCHEMA_DOMAIN)?,
        bytes(session.id()),
        bytes(session.binding_id()),
        bytes(session.context_id()),
        CborValue::Unsigned(session.store_generation()),
        CborValue::Unsigned(session.authority_epoch()),
        CborValue::text(session.request_commitment())?,
        validity_value(session.validity()),
    ]))
}

fn g0_path_value(path: &BootstrapG0PathV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.bootstrap-g0-path-evaluator-fact.v1")?,
        path.genesis_grant().schema_value()?,
        CborValue::Unsigned(path.store_generation()),
        CborValue::Bool(path.complete()),
        CborValue::Array(
            path.root_contributions()
                .iter()
                .copied()
                .map(bytes)
                .collect(),
        ),
    ]))
}

fn bootstrap_genesis_grant_value(
    genesis: &BootstrapGenesisGrantV1,
) -> Result<CborValue, CborError> {
    let definition = genesis.grant().definition();
    Ok(CborValue::Array(vec![
        CborValue::text(BootstrapGenesisGrantV1::SCHEMA_DOMAIN)?,
        bytes(definition.id),
        bytes(definition.context_id),
        bytes(definition.grantee_principal_id),
        CborValue::Unsigned(genesis.authority_epoch()),
        CborValue::Unsigned(genesis.trust_root_revision()),
        CborValue::Array(vec![CborValue::Unsigned(1)]),
        scope_value(&definition.terminal_scope)?,
        scope_value(&definition.delegable_scope)?,
        CborValue::Unsigned(definition.validity.not_before()),
        CborValue::Unsigned(definition.validity.expires_at()),
    ]))
}

fn scope_value(scope: &GrantScopeV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(
        scope
            .atoms()
            .map(|atom| {
                Ok(CborValue::Array(vec![
                    CborValue::text(atom.action())?,
                    CborValue::text(atom.subject())?,
                    CborValue::Unsigned(atom.protocol_revision()),
                ]))
            })
            .collect::<Result<Vec<_>, CborError>>()?,
    ))
}

fn revocations_value(revocations: &AuthorityRevocationSetV1) -> Result<CborValue, CborError> {
    let rows = revocations
        .revocations()
        .targets()
        .map(|target| match target {
            RevocationTargetV1::TrustRoot(revision) => Ok(CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(revision),
            ])),
            RevocationTargetV1::PrincipalBinding(id) => {
                Ok(CborValue::Array(vec![CborValue::Unsigned(2), bytes(id)]))
            }
            RevocationTargetV1::Session(id) => {
                Ok(CborValue::Array(vec![CborValue::Unsigned(3), bytes(id)]))
            }
            RevocationTargetV1::Grant(id) => {
                Ok(CborValue::Array(vec![CborValue::Unsigned(4), bytes(id)]))
            }
            RevocationTargetV1::Mandate(id) => {
                Ok(CborValue::Array(vec![CborValue::Unsigned(5), bytes(id)]))
            }
        })
        .collect::<Result<Vec<_>, CborError>>()?;
    Ok(CborValue::Array(vec![
        CborValue::text(AuthorityRevocationSetV1::SCHEMA_DOMAIN)?,
        bytes(revocations.context_id()),
        CborValue::Array(rows),
    ]))
}

fn validity_value(validity: HalfOpenValidityV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(validity.not_before()),
        CborValue::Unsigned(validity.expires_at()),
    ])
}

fn bytes<K: AuthorityIdentityKindV1>(id: AuthorityIdV1<K>) -> CborValue {
    CborValue::Bytes(id.as_bytes().to_vec())
}

fn parse_context(
    value: &CborValue,
) -> Result<AuthorityContextV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = as_array(value)?;
    if fields.len() < 2 {
        return invalid();
    }
    require_domain(&fields[0], AuthorityContextV1::SCHEMA_DOMAIN)?;
    let kind = AuthorityContextKindV1::try_from(as_u8(&fields[1])?).map_err(|_| invalid_error())?;
    match kind {
        AuthorityContextKindV1::RepositoryAuthorityContext => {
            let fields = exact_array(value, 7)?;
            AuthorityContextV1::repository(
                parse_id(&fields[2])?,
                as_text(&fields[3])?,
                as_unsigned(&fields[4])?,
                as_unsigned(&fields[5])?,
                as_unsigned(&fields[6])?,
            )
            .map_err(|_| invalid_error())
        }
        AuthorityContextKindV1::InstallationAuthorityContext => {
            let fields = exact_array(value, 9)?;
            AuthorityContextV1::installation(
                parse_id(&fields[2])?,
                as_text(&fields[3])?,
                as_text(&fields[4])?,
                as_unsigned(&fields[5])?,
                as_unsigned(&fields[6])?,
                as_unsigned(&fields[7])?,
                as_unsigned(&fields[8])?,
            )
            .map_err(|_| invalid_error())
        }
    }
}

fn parse_snapshot(
    value: &CborValue,
) -> Result<AuthoritySnapshotV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 7)?;
    require_domain(&fields[0], AuthoritySnapshotV1::SCHEMA_DOMAIN)?;
    Ok(AuthoritySnapshotV1::new(
        parse_id(&fields[1])?,
        as_unsigned(&fields[2])?,
        as_unsigned(&fields[3])?,
        as_unsigned(&fields[4])?,
        as_unsigned(&fields[5])?,
        parse_trusted_time(&fields[6])?,
    ))
}

fn parse_trusted_time(
    value: &CborValue,
) -> Result<TrustedTimeV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = as_array(value)?;
    match fields {
        [CborValue::Unsigned(1), lower, upper] => {
            TrustedTimeV1::verified(as_unsigned(lower)?, as_unsigned(upper)?)
                .map_err(|_| invalid_error())
        }
        [CborValue::Unsigned(2)] => Ok(TrustedTimeV1::Unavailable),
        _ => invalid(),
    }
}

fn parse_binding(
    value: &CborValue,
) -> Result<PrincipalBindingV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 8)?;
    require_domain(&fields[0], PrincipalBindingV1::SCHEMA_DOMAIN)?;
    PrincipalBindingV1::new(
        parse_id(&fields[1])?,
        parse_id(&fields[2])?,
        parse_id(&fields[3])?,
        as_unsigned(&fields[4])?,
        as_unsigned(&fields[5])?,
        parse_validity(&fields[6])?,
        as_bool(&fields[7])?,
    )
    .map_err(|_| invalid_error())
}

fn parse_session(value: &CborValue) -> Result<SessionV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 8)?;
    require_domain(&fields[0], SessionV1::SCHEMA_DOMAIN)?;
    SessionV1::new(
        parse_id(&fields[1])?,
        parse_id(&fields[2])?,
        parse_id(&fields[3])?,
        as_unsigned(&fields[4])?,
        as_unsigned(&fields[5])?,
        as_text(&fields[6])?,
        parse_validity(&fields[7])?,
    )
    .map_err(|_| invalid_error())
}

fn parse_g0_path(
    value: &CborValue,
) -> Result<BootstrapG0PathV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 5)?;
    require_domain(
        &fields[0],
        "maestro.vnext.bootstrap-g0-path-evaluator-fact.v1",
    )?;
    let root_contributions = as_array(&fields[4])?
        .iter()
        .map(parse_id::<super::identity::CapacityRootIdentityKindV1>)
        .collect::<Result<Vec<CapacityRootIdV1>, _>>()?;
    BootstrapG0PathV1::from_genesis_grant(
        parse_bootstrap_genesis_grant(&fields[1])?,
        as_unsigned(&fields[2])?,
        as_bool(&fields[3])?,
        root_contributions,
    )
    .map_err(|_| invalid_error())
}

fn parse_bootstrap_genesis_grant(
    value: &CborValue,
) -> Result<BootstrapGenesisGrantV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 11)?;
    require_domain(&fields[0], BootstrapGenesisGrantV1::SCHEMA_DOMAIN)?;
    let authority_use_constraint = match as_array(&fields[6])? {
        [CborValue::Unsigned(1)] => AuthorityUseConstraintV1::NoLocalBoundedRoot,
        _ => return invalid(),
    };
    let grant = GrantDefinitionV1 {
        id: parse_id::<super::identity::GrantIdentityKindV1>(&fields[1])?,
        context_id: parse_id(&fields[2])?,
        grantee_principal_id: parse_id(&fields[3])?,
        parent_grant_id: None,
        delegation_id: None,
        terminal_scope: parse_scope(&fields[7])?,
        delegable_scope: parse_scope(&fields[8])?,
        validity: HalfOpenValidityV1::new(as_unsigned(&fields[9])?, as_unsigned(&fields[10])?)
            .map_err(|_| invalid_error())?,
        delegation_depth_remaining: 0,
        authority_use_constraint,
    }
    .validate()
    .map_err(|_| invalid_error())?;
    BootstrapGenesisGrantV1::new(
        GenesisGrantIdV1::derive(&grant.id().render()).map_err(|_| invalid_error())?,
        grant,
        as_unsigned(&fields[4])?,
        as_unsigned(&fields[5])?,
    )
    .map_err(|_| invalid_error())
}

fn parse_scope(value: &CborValue) -> Result<GrantScopeV1, BootstrapAuthoritySnapshotErrorV1> {
    let atoms = as_array(value)?
        .iter()
        .map(|value| {
            let fields = exact_array(value, 3)?;
            ScopeAtomV1::new(
                as_text(&fields[0])?,
                as_text(&fields[1])?,
                as_unsigned(&fields[2])?,
            )
            .map_err(|_| invalid_error())
        })
        .collect::<Result<Vec<_>, _>>()?;
    GrantScopeV1::new(atoms).map_err(|_| invalid_error())
}

fn parse_revocations(
    value: &CborValue,
) -> Result<AuthorityRevocationSetV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 3)?;
    require_domain(&fields[0], AuthorityRevocationSetV1::SCHEMA_DOMAIN)?;
    let targets = as_array(&fields[2])?
        .iter()
        .map(|row| {
            let row = exact_array(row, 2)?;
            match as_u8(&row[0])? {
                1 => Ok(RevocationTargetV1::TrustRoot(as_unsigned(&row[1])?)),
                2 => Ok(RevocationTargetV1::PrincipalBinding(parse_id(&row[1])?)),
                3 => Ok(RevocationTargetV1::Session(parse_id(&row[1])?)),
                4 => Ok(RevocationTargetV1::Grant(parse_id(&row[1])?)),
                5 => Ok(RevocationTargetV1::Mandate(parse_id(&row[1])?)),
                _ => invalid(),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AuthorityRevocationSetV1::new(
        parse_id(&fields[1])?,
        RevocationSetV1::new(targets).map_err(|_| invalid_error())?,
    ))
}

fn parse_presentation(
    value: &CborValue,
) -> Result<BootstrapMandatePresentationObservationV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 4)?;
    require_domain(
        &fields[0],
        BootstrapMandatePresentationObservationV1::SCHEMA_DOMAIN,
    )?;
    BootstrapMandatePresentationObservationV1::new(
        parse_interaction_subject(&fields[1])?,
        parse_id(&fields[2])?,
        parse_id(&fields[3])?,
    )
    .map_err(BootstrapAuthoritySnapshotErrorV1::CanonicalCbor)
}

fn parse_response(
    value: &CborValue,
) -> Result<BootstrapMandateResponseObservationV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 5)?;
    require_domain(
        &fields[0],
        BootstrapMandateResponseObservationV1::SCHEMA_DOMAIN,
    )?;
    BootstrapMandateResponseObservationV1::new(
        parse_interaction_subject(&fields[1])?,
        parse_id(&fields[2])?,
        BootstrapResponseDispositionV1::try_from(as_u8(&fields[3])?)?,
        parse_id(&fields[4])?,
    )
    .map_err(BootstrapAuthoritySnapshotErrorV1::CanonicalCbor)
}

fn parse_interaction_join(
    value: &CborValue,
) -> Result<BootstrapMandateInteractionObservationJoinV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 9)?;
    require_domain(
        &fields[0],
        BootstrapMandateInteractionObservationJoinV1::SCHEMA_DOMAIN,
    )?;
    Ok(BootstrapMandateInteractionObservationJoinV1 {
        interaction_closure_id: parse_id(&fields[1])?,
        context_id: parse_id(&fields[2])?,
        responder_binding_id: parse_id(&fields[3])?,
        responder_current_authentication_ref: parse_id(&fields[4])?,
        presentation_observation_id: parse_id(&fields[5])?,
        affirmative_response_observation_id: parse_id(&fields[6])?,
        carrier_procedure_ref: parse_id(&fields[7])?,
        target_action_commitment: parse_id(&fields[8])?,
    })
}

fn parse_interaction_subject(
    value: &CborValue,
) -> Result<BootstrapInteractionSubjectV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 10)?;
    require_domain(&fields[0], BootstrapInteractionSubjectV1::SCHEMA_DOMAIN)?;
    let assurance = as_unsigned(&fields[5])?;
    if assurance == 0 {
        return invalid();
    }
    Ok(BootstrapInteractionSubjectV1::new(
        parse_id(&fields[1])?,
        parse_id(&fields[2])?,
        parse_id(&fields[3])?,
        parse_id(&fields[4])?,
        assurance,
        parse_id(&fields[6])?,
        parse_consent_binding(&fields[7])?,
        parse_id(&fields[8])?,
        parse_id(&fields[9])?,
    ))
}

fn parse_consent_slot_facts(
    value: &CborValue,
) -> Result<ConsentSlotEvaluationFactsV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 4)?;
    require_domain(&fields[0], ConsentSlotEvaluationFactsV1::SCHEMA_DOMAIN)?;
    let requirement_members = as_array(&fields[2])?
        .iter()
        .cloned()
        .map(ConsentRequirementMemberV1::from_schema_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid_error())?;
    Ok(ConsentSlotEvaluationFactsV1::from_parts(
        parse_consent_binding(&fields[1])?,
        requirement_members,
        parse_validity(&fields[3])?,
    ))
}

fn parse_consent_binding(
    value: &CborValue,
) -> Result<ConsentSlotBindingParameterV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 4)?;
    require_domain(&fields[0], ConsentSlotBindingParameterV1::SCHEMA_DOMAIN)?;
    Ok(ConsentSlotBindingParameterV1::from_commitments(
        parse_id::<super::identity::ConsentProtocolCommitmentIdentityKindV1>(&fields[1])?,
        parse_id::<super::identity::TargetActionCommitmentIdentityKindV1>(&fields[2])?,
        parse_id::<super::identity::ConsentSlotCommitmentIdentityKindV1>(&fields[3])?,
    ))
}

fn parse_continuity(
    value: &CborValue,
) -> Result<BootstrapContinuityTransitionProofV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 9)?;
    require_domain(
        &fields[0],
        BootstrapContinuityTransitionProofV1::SCHEMA_DOMAIN,
    )?;
    let guard_tag = as_u8(&fields[6])?;
    let guard_kind = if guard_tag == 0 {
        GuardAdmissionKindV1::ExternallyRootedContextGenesis
    } else {
        GuardAdmissionKindV1::Established(
            super::closed::TransitionGuardKindV1::try_from(guard_tag)
                .map_err(|_| invalid_error())?,
        )
    };
    Ok(BootstrapContinuityTransitionProofV1::new(
        parse_id(&fields[1])?,
        as_unsigned(&fields[2])?,
        as_unsigned(&fields[3])?,
        as_unsigned(&fields[4])?,
        parse_id(&fields[5])?,
        guard_kind,
        parse_id(&fields[7])?,
        parse_validity(&fields[8])?,
    ))
}

fn parse_validity(
    value: &CborValue,
) -> Result<HalfOpenValidityV1, BootstrapAuthoritySnapshotErrorV1> {
    let fields = exact_array(value, 2)?;
    HalfOpenValidityV1::new(as_unsigned(&fields[0])?, as_unsigned(&fields[1])?)
        .map_err(|_| invalid_error())
}

fn parse_optional<T>(
    value: &CborValue,
    parse: fn(&CborValue) -> Result<T, BootstrapAuthoritySnapshotErrorV1>,
) -> Result<Option<T>, BootstrapAuthoritySnapshotErrorV1> {
    match as_array(value)? {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), value] => Ok(Some(parse(value)?)),
        _ => invalid(),
    }
}

fn parse_id<K: AuthorityIdentityKindV1>(
    value: &CborValue,
) -> Result<AuthorityIdV1<K>, BootstrapAuthoritySnapshotErrorV1> {
    Ok(AuthorityIdV1::from_digest(as_digest(value)?))
}

fn exact_array(
    value: &CborValue,
    expected: usize,
) -> Result<&[CborValue], BootstrapAuthoritySnapshotErrorV1> {
    let values = as_array(value)?;
    if values.len() == expected {
        Ok(values)
    } else {
        invalid()
    }
}

fn as_array(value: &CborValue) -> Result<&[CborValue], BootstrapAuthoritySnapshotErrorV1> {
    match value {
        CborValue::Array(values) => Ok(values),
        _ => invalid(),
    }
}

fn require_domain(
    value: &CborValue,
    expected: &str,
) -> Result<(), BootstrapAuthoritySnapshotErrorV1> {
    if as_text(value)? == expected {
        Ok(())
    } else {
        invalid()
    }
}

fn as_unsigned(value: &CborValue) -> Result<u64, BootstrapAuthoritySnapshotErrorV1> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => invalid(),
    }
}

fn as_u8(value: &CborValue) -> Result<u8, BootstrapAuthoritySnapshotErrorV1> {
    u8::try_from(as_unsigned(value)?).map_err(|_| invalid_error())
}

fn as_text(value: &CborValue) -> Result<&str, BootstrapAuthoritySnapshotErrorV1> {
    match value {
        CborValue::Text(value) => Ok(value),
        _ => invalid(),
    }
}

fn as_bool(value: &CborValue) -> Result<bool, BootstrapAuthoritySnapshotErrorV1> {
    match value {
        CborValue::Bool(value) => Ok(*value),
        _ => invalid(),
    }
}

fn as_digest(value: &CborValue) -> Result<[u8; 32], BootstrapAuthoritySnapshotErrorV1> {
    match value {
        CborValue::Bytes(value) => value.as_slice().try_into().map_err(|_| invalid_error()),
        _ => invalid(),
    }
}

fn invalid<T>() -> Result<T, BootstrapAuthoritySnapshotErrorV1> {
    Err(invalid_error())
}

const fn invalid_error() -> BootstrapAuthoritySnapshotErrorV1 {
    BootstrapAuthoritySnapshotErrorV1::InvalidCanonicalSnapshot
}

fn hash_value(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}
