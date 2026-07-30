use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::bootstrap_catalog::{BootstrapMandateTargetV1, BootstrapTargetDispositionV1};
use super::grant::HalfOpenValidityV1;
use super::identity::{
    ActionRequestIdV1, AuthorityBasisCommitmentIdV1, AuthorityContextIdV1,
    BootstrapMandateIssuanceBindingIdV1, ConsentProtocolCommitmentIdV1, ConsentSlotCommitmentIdV1,
    IdempotencyKeyIdV1, InteractionClosureIdV1, MandateIdV1, ObservationIdV1, PrincipalBindingIdV1,
    SessionIdV1, StateTokenIdV1, TargetActionCommitmentIdV1,
};

const MAX_TARGET_SUBJECT_BYTES: usize = 256;
const MAX_EFFECT_COMMITMENT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ConsentRoleV1 {
    AffirmativeHumanConsent = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NaturalMemberSubjectV1 {
    WholeTarget = 1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentRequirementMemberV1 {
    target: BootstrapMandateTargetV1,
    target_revision: u64,
    target_protocol: TargetActionProtocolV1,
    role: ConsentRoleV1,
    natural_subject: NaturalMemberSubjectV1,
    horizon: HalfOpenValidityV1,
}

impl ConsentRequirementMemberV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.consent-requirement-member-value.v1";

    pub fn derive_for_target(
        target: &TargetActionProjectionV1,
    ) -> Result<Self, ConsentSlotDerivationErrorV1> {
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
            _ => return Err(ConsentSlotDerivationErrorV1::Unavailable),
        };
        if target.owner() != TargetActionOwnerV1::Authority
            || target.protocol() != TargetActionProtocolV1::RecoveryCommitmentSelection
            || target.effect_kind() != expected_effect
        {
            return Err(ConsentSlotDerivationErrorV1::Unavailable);
        }
        Ok(Self {
            target: target.target(),
            target_revision: target.target_revision(),
            target_protocol: target.protocol(),
            role: ConsentRoleV1::AffirmativeHumanConsent,
            natural_subject: NaturalMemberSubjectV1::WholeTarget,
            horizon: target.validity(),
        })
    }

    pub const fn target(&self) -> BootstrapMandateTargetV1 {
        self.target
    }

    pub const fn target_revision(&self) -> u64 {
        self.target_revision
    }

    pub const fn target_protocol(&self) -> TargetActionProtocolV1 {
        self.target_protocol
    }

    pub const fn role(&self) -> ConsentRoleV1 {
        self.role
    }

    pub const fn natural_subject(&self) -> NaturalMemberSubjectV1 {
        self.natural_subject
    }

    pub const fn horizon(&self) -> HalfOpenValidityV1 {
        self.horizon
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Unsigned(self.target as u64),
            CborValue::Unsigned(self.target_revision),
            CborValue::Unsigned(self.target_protocol as u64),
            CborValue::Unsigned(self.role as u64),
            CborValue::Unsigned(self.natural_subject as u64),
            CborValue::Unsigned(1),
            CborValue::Unsigned(1),
            CborValue::Unsigned(1),
            CborValue::Unsigned(self.horizon.not_before()),
            CborValue::Unsigned(self.horizon.expires_at()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub(crate) fn from_schema_value(
        value: CborValue,
    ) -> Result<Self, ConsentSlotDerivationErrorV1> {
        let fields = match value {
            CborValue::Array(fields) if fields.len() == 11 => fields,
            _ => return Err(ConsentSlotDerivationErrorV1::Unavailable),
        };
        require_consent_domain(&fields[0], Self::SCHEMA_DOMAIN)?;
        let target = BootstrapMandateTargetV1::try_from(consent_u8(&fields[1])?)
            .map_err(|_| ConsentSlotDerivationErrorV1::Unavailable)?;
        let target_revision = consent_unsigned(&fields[2])?;
        let target_protocol = TargetActionProtocolV1::try_from(consent_u8(&fields[3])?)
            .map_err(|_| ConsentSlotDerivationErrorV1::Unavailable)?;
        if consent_u8(&fields[4])? != ConsentRoleV1::AffirmativeHumanConsent as u8
            || consent_u8(&fields[5])? != NaturalMemberSubjectV1::WholeTarget as u8
            || consent_unsigned(&fields[6])? != 1
            || consent_unsigned(&fields[7])? != 1
            || consent_unsigned(&fields[8])? != 1
            || target_revision == 0
        {
            return Err(ConsentSlotDerivationErrorV1::Unavailable);
        }
        let horizon = HalfOpenValidityV1::new(
            consent_unsigned(&fields[9])?,
            consent_unsigned(&fields[10])?,
        )
        .map_err(|_| ConsentSlotDerivationErrorV1::Unavailable)?;
        Ok(Self {
            target,
            target_revision,
            target_protocol,
            role: ConsentRoleV1::AffirmativeHumanConsent,
            natural_subject: NaturalMemberSubjectV1::WholeTarget,
            horizon,
        })
    }

    fn commitment(&self) -> Result<[u8; 32], ConsentSlotDerivationErrorV1> {
        Ok(Sha256::digest(self.canonical_bytes()?).into())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ConsentSlotDerivationErrorV1 {
    #[error("consent slot is unavailable")]
    Unavailable,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentSlotBindingParameterV1 {
    protocol_commitment: ConsentProtocolCommitmentIdV1,
    target_action_commitment: TargetActionCommitmentIdV1,
    slot_commitment: ConsentSlotCommitmentIdV1,
}

impl ConsentSlotBindingParameterV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.consent-slot-binding-parameter.v1";
    pub const IDENTITY_PROTOCOL_DOMAIN: &'static str =
        "maestro.vnext.consent-slot-identity-protocol.v1";
    pub const NORMAL_FORM_DOMAIN: &'static str = "maestro.vnext.consent-slot-normal-form.v1";

    pub fn derive(
        target: &TargetActionProjectionV1,
        member: &ConsentRequirementMemberV1,
    ) -> Result<Self, ConsentSlotDerivationErrorV1> {
        if ConsentRequirementMemberV1::derive_for_target(target)? != *member {
            return Err(ConsentSlotDerivationErrorV1::Unavailable);
        }
        let protocol_value = CborValue::Array(vec![
            CborValue::text(Self::IDENTITY_PROTOCOL_DOMAIN)?,
            CborValue::Unsigned(1),
        ]);
        let protocol_commitment = ConsentProtocolCommitmentIdV1::from_digest(
            Sha256::digest(deterministic_cbor::encode(&protocol_value)?).into(),
        );
        let target_action_commitment = target
            .target_action_commitment()
            .map_err(|_| ConsentSlotDerivationErrorV1::Unavailable)?;
        let normal_form = CborValue::Array(vec![
            CborValue::text(Self::NORMAL_FORM_DOMAIN)?,
            CborValue::Bytes(protocol_commitment.as_bytes().to_vec()),
            CborValue::Bytes(target.expected_heads().context_id().as_bytes().to_vec()),
            CborValue::Bytes(target_action_commitment.as_bytes().to_vec()),
            CborValue::Bytes(member.commitment()?.to_vec()),
        ]);
        let slot_commitment = ConsentSlotCommitmentIdV1::from_digest(
            Sha256::digest(deterministic_cbor::encode(&normal_form)?).into(),
        );
        Ok(Self {
            protocol_commitment,
            target_action_commitment,
            slot_commitment,
        })
    }

    pub(crate) const fn from_commitments(
        protocol_commitment: ConsentProtocolCommitmentIdV1,
        target_action_commitment: TargetActionCommitmentIdV1,
        slot_commitment: ConsentSlotCommitmentIdV1,
    ) -> Self {
        Self {
            protocol_commitment,
            target_action_commitment,
            slot_commitment,
        }
    }

    pub const fn protocol_commitment(&self) -> ConsentProtocolCommitmentIdV1 {
        self.protocol_commitment
    }

    pub const fn target_action_commitment(&self) -> TargetActionCommitmentIdV1 {
        self.target_action_commitment
    }

    pub const fn slot_commitment(&self) -> ConsentSlotCommitmentIdV1 {
        self.slot_commitment
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.protocol_commitment.as_bytes().to_vec()),
            CborValue::Bytes(self.target_action_commitment.as_bytes().to_vec()),
            CborValue::Bytes(self.slot_commitment.as_bytes().to_vec()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TargetActionOwnerV1 {
    Authority = 1,
    Execution = 2,
    Evidence = 3,
}

impl TryFrom<u8> for TargetActionOwnerV1 {
    type Error = TargetActionProjectionErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Authority),
            2 => Ok(Self::Execution),
            3 => Ok(Self::Evidence),
            _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TargetActionProtocolV1 {
    RecoveryCommitmentSelection = 1,
    BootstrapMandateInteraction = 2,
}

impl TryFrom<u8> for TargetActionProtocolV1 {
    type Error = TargetActionProjectionErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::RecoveryCommitmentSelection),
            2 => Ok(Self::BootstrapMandateInteraction),
            _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TargetActionEffectKindV1 {
    Enroll = 1,
    Rotate = 2,
    Revoke = 3,
    IssueMandate = 4,
}

impl TryFrom<u8> for TargetActionEffectKindV1 {
    type Error = TargetActionProjectionErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Enroll),
            2 => Ok(Self::Rotate),
            3 => Ok(Self::Revoke),
            4 => Ok(Self::IssueMandate),
            _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetExpectedHeadsV1 {
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    authority_epoch: u64,
    trust_root_revision: u64,
    subject_revision: u64,
    target_head: StateTokenIdV1,
}

impl TargetExpectedHeadsV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.target-expected-heads.v1";

    pub fn new(
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        trust_root_revision: u64,
        subject_revision: u64,
        target_head: StateTokenIdV1,
    ) -> Result<Self, TargetActionProjectionErrorV1> {
        if trust_root_revision == 0 || subject_revision == 0 {
            return Err(TargetActionProjectionErrorV1::ZeroRevision);
        }
        Ok(Self {
            context_id,
            store_generation,
            authority_epoch,
            trust_root_revision,
            subject_revision,
            target_head,
        })
    }

    pub const fn context_id(self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn store_generation(self) -> u64 {
        self.store_generation
    }

    pub const fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub const fn trust_root_revision(self) -> u64 {
        self.trust_root_revision
    }

    pub const fn subject_revision(self) -> u64 {
        self.subject_revision
    }

    pub const fn target_head(self) -> StateTokenIdV1 {
        self.target_head
    }

    pub fn schema_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.context_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.store_generation),
            CborValue::Unsigned(self.authority_epoch),
            CborValue::Unsigned(self.trust_root_revision),
            CborValue::Unsigned(self.subject_revision),
            CborValue::Bytes(self.target_head.as_bytes().to_vec()),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetActionProjectionV1 {
    target: BootstrapMandateTargetV1,
    target_subject: String,
    target_revision: u64,
    owner: TargetActionOwnerV1,
    protocol: TargetActionProtocolV1,
    effect_kind: TargetActionEffectKindV1,
    effect_commitment: String,
    expected_heads: TargetExpectedHeadsV1,
    validity: HalfOpenValidityV1,
}

impl TargetActionProjectionV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.authorization-free-target-action-projection.v1";

    #[expect(
        clippy::too_many_arguments,
        reason = "the authorization-free projection exposes every committed semantic dimension"
    )]
    pub fn new(
        target: BootstrapMandateTargetV1,
        target_subject: &str,
        target_revision: u64,
        owner: TargetActionOwnerV1,
        protocol: TargetActionProtocolV1,
        effect_kind: TargetActionEffectKindV1,
        effect_commitment: &str,
        expected_heads: TargetExpectedHeadsV1,
        validity: HalfOpenValidityV1,
    ) -> Result<Self, TargetActionProjectionErrorV1> {
        if target_subject.is_empty()
            || target_subject.len() > MAX_TARGET_SUBJECT_BYTES
            || !target_subject.is_ascii()
        {
            return Err(TargetActionProjectionErrorV1::InvalidTargetSubject);
        }
        if effect_commitment.is_empty()
            || effect_commitment.len() > MAX_EFFECT_COMMITMENT_BYTES
            || !effect_commitment.is_ascii()
        {
            return Err(TargetActionProjectionErrorV1::InvalidEffectCommitment);
        }
        if target_revision == 0 {
            return Err(TargetActionProjectionErrorV1::ZeroRevision);
        }
        Ok(Self {
            target,
            target_subject: target_subject.to_owned(),
            target_revision,
            owner,
            protocol,
            effect_kind,
            effect_commitment: effect_commitment.to_owned(),
            expected_heads,
            validity,
        })
    }

    pub const fn target(&self) -> BootstrapMandateTargetV1 {
        self.target
    }

    pub fn target_subject(&self) -> &str {
        &self.target_subject
    }

    pub const fn target_revision(&self) -> u64 {
        self.target_revision
    }

    pub const fn owner(&self) -> TargetActionOwnerV1 {
        self.owner
    }

    pub const fn protocol(&self) -> TargetActionProtocolV1 {
        self.protocol
    }

    pub const fn effect_kind(&self) -> TargetActionEffectKindV1 {
        self.effect_kind
    }

    pub fn effect_commitment(&self) -> &str {
        &self.effect_commitment
    }

    pub const fn expected_heads(&self) -> TargetExpectedHeadsV1 {
        self.expected_heads
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Unsigned(self.target as u64),
            CborValue::text(&self.target_subject)?,
            CborValue::Unsigned(self.target_revision),
            CborValue::Unsigned(self.owner as u64),
            CborValue::Unsigned(self.protocol as u64),
            CborValue::Unsigned(self.effect_kind as u64),
            CborValue::text(&self.effect_commitment)?,
            self.expected_heads.schema_value()?,
            CborValue::Unsigned(self.validity.not_before()),
            CborValue::Unsigned(self.validity.expires_at()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn target_action_commitment(
        &self,
    ) -> Result<TargetActionCommitmentIdV1, TargetActionProjectionErrorV1> {
        Ok(TargetActionCommitmentIdV1::from_digest(
            Sha256::digest(self.canonical_bytes()?).into(),
        ))
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, TargetActionProjectionErrorV1> {
        let projection = Self::from_schema_value(deterministic_cbor::decode(bytes)?)?;
        if projection.canonical_bytes()? != bytes {
            return Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection);
        }
        Ok(projection)
    }

    pub(crate) fn from_schema_value(
        value: CborValue,
    ) -> Result<Self, TargetActionProjectionErrorV1> {
        let fields = exact_array(value, 11)?;
        require_domain(&fields[0], Self::SCHEMA_DOMAIN)?;
        let target = BootstrapMandateTargetV1::try_from(as_u8(&fields[1])?)
            .map_err(|_| TargetActionProjectionErrorV1::InvalidCanonicalProjection)?;
        let target_subject = as_text(&fields[2])?;
        let target_revision = as_unsigned(&fields[3])?;
        let owner = TargetActionOwnerV1::try_from(as_u8(&fields[4])?)?;
        let protocol = TargetActionProtocolV1::try_from(as_u8(&fields[5])?)?;
        let effect_kind = TargetActionEffectKindV1::try_from(as_u8(&fields[6])?)?;
        let effect_commitment = as_text(&fields[7])?;
        let head_fields = exact_array(fields[8].clone(), 7)?;
        require_domain(&head_fields[0], TargetExpectedHeadsV1::SCHEMA_DOMAIN)?;
        let expected_heads = TargetExpectedHeadsV1::new(
            AuthorityContextIdV1::from_digest(as_digest(&head_fields[1])?),
            as_unsigned(&head_fields[2])?,
            as_unsigned(&head_fields[3])?,
            as_unsigned(&head_fields[4])?,
            as_unsigned(&head_fields[5])?,
            StateTokenIdV1::from_digest(as_digest(&head_fields[6])?),
        )?;
        let validity = HalfOpenValidityV1::new(as_unsigned(&fields[9])?, as_unsigned(&fields[10])?)
            .map_err(|_| TargetActionProjectionErrorV1::InvalidCanonicalProjection)?;
        Self::new(
            target,
            target_subject,
            target_revision,
            owner,
            protocol,
            effect_kind,
            effect_commitment,
            expected_heads,
            validity,
        )
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TargetActionProjectionErrorV1 {
    #[error("target subject must contain between 1 and 256 ASCII bytes")]
    InvalidTargetSubject,
    #[error("target effect commitment must contain between 1 and 256 ASCII bytes")]
    InvalidEffectCommitment,
    #[error("target projection revision must be nonzero")]
    ZeroRevision,
    #[error("invalid canonical target Action projection")]
    InvalidCanonicalProjection,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueBootstrapMandateInputV1 {
    pub request_id: ActionRequestIdV1,
    pub idempotency_key: IdempotencyKeyIdV1,
    pub context_id: AuthorityContextIdV1,
    pub actor_binding_id: PrincipalBindingIdV1,
    pub actor_session_id: SessionIdV1,
    pub responder_binding_id: PrincipalBindingIdV1,
    pub presentation_observation_id: ObservationIdV1,
    pub response_observation_id: ObservationIdV1,
    pub target: BootstrapMandateTargetV1,
    pub target_subject: String,
    pub target_revision: u64,
    pub consent_slot: ConsentSlotBindingParameterV1,
    pub supplied_mandates: Vec<MandateIdV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueBootstrapMandateRequestV1 {
    input: IssueBootstrapMandateInputV1,
}

impl TryFrom<IssueBootstrapMandateInputV1> for IssueBootstrapMandateRequestV1 {
    type Error = IssueBootstrapMandateError;

    fn try_from(input: IssueBootstrapMandateInputV1) -> Result<Self, Self::Error> {
        if !input.supplied_mandates.is_empty() {
            return Err(IssueBootstrapMandateError::SuppliedMandatesForbidden);
        }
        if input.target.disposition() != BootstrapTargetDispositionV1::Admitted {
            return Err(IssueBootstrapMandateError::TargetExcluded);
        }
        if input.target_subject.is_empty()
            || input.target_subject.len() > MAX_TARGET_SUBJECT_BYTES
            || !input.target_subject.is_ascii()
        {
            return Err(IssueBootstrapMandateError::InvalidTargetSubject);
        }
        if input.target_revision == 0 {
            return Err(IssueBootstrapMandateError::ZeroRevision);
        }
        Ok(Self { input })
    }
}

impl IssueBootstrapMandateRequestV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.issue-bootstrap-mandate-request.v1";

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.input.request_id
    }

    pub const fn idempotency_key(&self) -> IdempotencyKeyIdV1 {
        self.input.idempotency_key
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.input.context_id
    }

    pub const fn actor_binding_id(&self) -> PrincipalBindingIdV1 {
        self.input.actor_binding_id
    }

    pub const fn actor_session_id(&self) -> SessionIdV1 {
        self.input.actor_session_id
    }

    pub const fn responder_binding_id(&self) -> PrincipalBindingIdV1 {
        self.input.responder_binding_id
    }

    pub const fn presentation_observation_id(&self) -> ObservationIdV1 {
        self.input.presentation_observation_id
    }

    pub const fn response_observation_id(&self) -> ObservationIdV1 {
        self.input.response_observation_id
    }

    pub const fn target(&self) -> BootstrapMandateTargetV1 {
        self.input.target
    }

    pub fn target_subject(&self) -> &str {
        &self.input.target_subject
    }

    pub const fn target_revision(&self) -> u64 {
        self.input.target_revision
    }

    pub fn consent_slot(&self) -> &ConsentSlotBindingParameterV1 {
        &self.input.consent_slot
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.request_id().as_bytes().to_vec()),
            CborValue::Bytes(self.context_id().as_bytes().to_vec()),
            CborValue::Bytes(self.actor_binding_id().as_bytes().to_vec()),
            CborValue::Bytes(self.actor_session_id().as_bytes().to_vec()),
            CborValue::Bytes(self.responder_binding_id().as_bytes().to_vec()),
            CborValue::Bytes(self.presentation_observation_id().as_bytes().to_vec()),
            CborValue::Bytes(self.response_observation_id().as_bytes().to_vec()),
            CborValue::Unsigned(self.target() as u64),
            CborValue::text(self.target_subject())?,
            CborValue::Unsigned(self.target_revision()),
            self.consent_slot().schema_value()?,
            CborValue::Bytes(self.idempotency_key().as_bytes().to_vec()),
            CborValue::Array(Vec::new()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapMandateEvaluationV1 {
    request: IssueBootstrapMandateRequestV1,
    responder_assurance_revision: u64,
    interaction_closure_id: InteractionClosureIdV1,
    authority_basis_commitment_id: AuthorityBasisCommitmentIdV1,
    validity: HalfOpenValidityV1,
}

impl BootstrapMandateEvaluationV1 {
    pub(crate) fn seal(
        request: IssueBootstrapMandateRequestV1,
        responder_assurance_revision: u64,
        interaction_closure_id: InteractionClosureIdV1,
        authority_basis_commitment_id: AuthorityBasisCommitmentIdV1,
        validity: HalfOpenValidityV1,
    ) -> Self {
        Self {
            request,
            responder_assurance_revision,
            interaction_closure_id,
            authority_basis_commitment_id,
            validity,
        }
    }

    pub fn request(&self) -> &IssueBootstrapMandateRequestV1 {
        &self.request
    }

    pub const fn responder_assurance_revision(&self) -> u64 {
        self.responder_assurance_revision
    }

    pub const fn interaction_closure_id(&self) -> InteractionClosureIdV1 {
        self.interaction_closure_id
    }

    pub const fn authority_basis_commitment_id(&self) -> AuthorityBasisCommitmentIdV1 {
        self.authority_basis_commitment_id
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityMandateV1 {
    id: MandateIdV1,
    context_id: AuthorityContextIdV1,
    action: &'static str,
    subject: String,
    action_revision: u64,
    consent_slot: ConsentSlotBindingParameterV1,
    responder_binding_id: PrincipalBindingIdV1,
    responder_assurance_revision: u64,
    interaction_closure_id: InteractionClosureIdV1,
    authority_basis_commitment_id: AuthorityBasisCommitmentIdV1,
    validity: HalfOpenValidityV1,
}

impl AuthorityMandateV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.authority-mandate-value.v1";

    pub const fn id(&self) -> MandateIdV1 {
        self.id
    }

    pub const fn maximum_uses(&self) -> u8 {
        1
    }

    pub const fn delegation_depth(&self) -> u8 {
        0
    }

    pub const fn action(&self) -> &'static str {
        self.action
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub const fn action_revision(&self) -> u64 {
        self.action_revision
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub fn consent_slot(&self) -> &ConsentSlotBindingParameterV1 {
        &self.consent_slot
    }

    pub const fn responder_binding_id(&self) -> PrincipalBindingIdV1 {
        self.responder_binding_id
    }

    pub const fn responder_assurance_revision(&self) -> u64 {
        self.responder_assurance_revision
    }

    pub const fn interaction_closure_id(&self) -> InteractionClosureIdV1 {
        self.interaction_closure_id
    }

    pub const fn authority_basis_commitment_id(&self) -> AuthorityBasisCommitmentIdV1 {
        self.authority_basis_commitment_id
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }

    pub const fn schema_domain(&self) -> &'static str {
        Self::SCHEMA_DOMAIN
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        mandate_semantic_value(MandateSemanticFields {
            context_id: self.context_id,
            action: self.action,
            subject: &self.subject,
            action_revision: self.action_revision,
            consent_slot: &self.consent_slot,
            responder_binding_id: self.responder_binding_id,
            responder_assurance_revision: self.responder_assurance_revision,
            interaction_closure_id: self.interaction_closure_id,
            authority_basis_commitment_id: self.authority_basis_commitment_id,
            validity: self.validity,
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapMandateIssuanceBindingV1 {
    id: BootstrapMandateIssuanceBindingIdV1,
    mandate_id: MandateIdV1,
    target_action_commitment: TargetActionCommitmentIdV1,
    slot_commitment: ConsentSlotCommitmentIdV1,
    interaction_closure_id: InteractionClosureIdV1,
}

impl BootstrapMandateIssuanceBindingV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.bootstrap-mandate-issuance-binding-value.v1";

    pub const fn id(&self) -> BootstrapMandateIssuanceBindingIdV1 {
        self.id
    }

    pub const fn mandate_id(&self) -> MandateIdV1 {
        self.mandate_id
    }

    pub const fn target_action_commitment(&self) -> TargetActionCommitmentIdV1 {
        self.target_action_commitment
    }

    pub const fn slot_commitment(&self) -> ConsentSlotCommitmentIdV1 {
        self.slot_commitment
    }

    pub const fn interaction_closure_id(&self) -> InteractionClosureIdV1 {
        self.interaction_closure_id
    }

    pub const fn schema_domain(&self) -> &'static str {
        Self::SCHEMA_DOMAIN
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.mandate_id.as_bytes().to_vec()),
            CborValue::Bytes(self.target_action_commitment.as_bytes().to_vec()),
            CborValue::Bytes(self.slot_commitment.as_bytes().to_vec()),
            CborValue::Bytes(self.interaction_closure_id.as_bytes().to_vec()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapMandateIssuanceV1 {
    pub mandate: AuthorityMandateV1,
    pub issuance_binding: BootstrapMandateIssuanceBindingV1,
}

pub fn issue_bootstrap_mandate(
    evaluation: BootstrapMandateEvaluationV1,
) -> Result<BootstrapMandateIssuanceV1, IssueBootstrapMandateError> {
    let request = &evaluation.request.input;
    let semantic_value = mandate_semantic_value(MandateSemanticFields {
        context_id: request.context_id,
        action: request.target.action_name(),
        subject: &request.target_subject,
        action_revision: request.target_revision,
        consent_slot: &request.consent_slot,
        responder_binding_id: request.responder_binding_id,
        responder_assurance_revision: evaluation.responder_assurance_revision,
        interaction_closure_id: evaluation.interaction_closure_id,
        authority_basis_commitment_id: evaluation.authority_basis_commitment_id,
        validity: evaluation.validity,
    })?;
    let mandate_id = MandateIdV1::from_digest(hash_schema_value(&semantic_value)?);
    let mandate = AuthorityMandateV1 {
        id: mandate_id,
        context_id: request.context_id,
        action: request.target.action_name(),
        subject: request.target_subject.clone(),
        action_revision: request.target_revision,
        consent_slot: request.consent_slot.clone(),
        responder_binding_id: request.responder_binding_id,
        responder_assurance_revision: evaluation.responder_assurance_revision,
        interaction_closure_id: evaluation.interaction_closure_id,
        authority_basis_commitment_id: evaluation.authority_basis_commitment_id,
        validity: evaluation.validity,
    };
    let binding_value = CborValue::Array(vec![
        CborValue::text(BootstrapMandateIssuanceBindingV1::SCHEMA_DOMAIN)?,
        CborValue::Bytes(mandate_id.as_bytes().to_vec()),
        CborValue::Bytes(
            request
                .consent_slot
                .target_action_commitment
                .as_bytes()
                .to_vec(),
        ),
        CborValue::Bytes(request.consent_slot.slot_commitment.as_bytes().to_vec()),
        CborValue::Bytes(evaluation.interaction_closure_id.as_bytes().to_vec()),
    ]);
    let issuance_binding = BootstrapMandateIssuanceBindingV1 {
        id: BootstrapMandateIssuanceBindingIdV1::from_digest(hash_schema_value(&binding_value)?),
        mandate_id,
        target_action_commitment: request.consent_slot.target_action_commitment,
        slot_commitment: request.consent_slot.slot_commitment,
        interaction_closure_id: evaluation.interaction_closure_id,
    };
    Ok(BootstrapMandateIssuanceV1 {
        mandate,
        issuance_binding,
    })
}

struct MandateSemanticFields<'a> {
    context_id: AuthorityContextIdV1,
    action: &'a str,
    subject: &'a str,
    action_revision: u64,
    consent_slot: &'a ConsentSlotBindingParameterV1,
    responder_binding_id: PrincipalBindingIdV1,
    responder_assurance_revision: u64,
    interaction_closure_id: InteractionClosureIdV1,
    authority_basis_commitment_id: AuthorityBasisCommitmentIdV1,
    validity: HalfOpenValidityV1,
}

fn mandate_semantic_value(fields: MandateSemanticFields<'_>) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(AuthorityMandateV1::SCHEMA_DOMAIN)?,
        CborValue::Bytes(fields.context_id.as_bytes().to_vec()),
        CborValue::text(fields.action)?,
        CborValue::text(fields.subject)?,
        CborValue::Unsigned(fields.action_revision),
        fields.consent_slot.schema_value()?,
        CborValue::Bytes(fields.responder_binding_id.as_bytes().to_vec()),
        CborValue::Unsigned(fields.responder_assurance_revision),
        CborValue::Bytes(fields.interaction_closure_id.as_bytes().to_vec()),
        CborValue::Bytes(fields.authority_basis_commitment_id.as_bytes().to_vec()),
        CborValue::Unsigned(fields.validity.not_before()),
        CborValue::Unsigned(fields.validity.expires_at()),
        CborValue::Unsigned(1),
        CborValue::Unsigned(0),
    ]))
}

fn hash_schema_value(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn require_consent_domain(
    value: &CborValue,
    expected: &str,
) -> Result<(), ConsentSlotDerivationErrorV1> {
    match value {
        CborValue::Text(actual) if actual == expected => Ok(()),
        _ => Err(ConsentSlotDerivationErrorV1::Unavailable),
    }
}

fn consent_unsigned(value: &CborValue) -> Result<u64, ConsentSlotDerivationErrorV1> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(ConsentSlotDerivationErrorV1::Unavailable),
    }
}

fn consent_u8(value: &CborValue) -> Result<u8, ConsentSlotDerivationErrorV1> {
    u8::try_from(consent_unsigned(value)?).map_err(|_| ConsentSlotDerivationErrorV1::Unavailable)
}

fn exact_array(
    value: CborValue,
    expected: usize,
) -> Result<Vec<CborValue>, TargetActionProjectionErrorV1> {
    match value {
        CborValue::Array(values) if values.len() == expected => Ok(values),
        _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
    }
}

fn require_domain(value: &CborValue, expected: &str) -> Result<(), TargetActionProjectionErrorV1> {
    match value {
        CborValue::Text(actual) if actual == expected => Ok(()),
        _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
    }
}

fn as_unsigned(value: &CborValue) -> Result<u64, TargetActionProjectionErrorV1> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
    }
}

fn as_u8(value: &CborValue) -> Result<u8, TargetActionProjectionErrorV1> {
    u8::try_from(as_unsigned(value)?)
        .map_err(|_| TargetActionProjectionErrorV1::InvalidCanonicalProjection)
}

fn as_text(value: &CborValue) -> Result<&str, TargetActionProjectionErrorV1> {
    match value {
        CborValue::Text(value) => Ok(value),
        _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
    }
}

fn as_digest(value: &CborValue) -> Result<[u8; 32], TargetActionProjectionErrorV1> {
    match value {
        CborValue::Bytes(value) => value
            .as_slice()
            .try_into()
            .map_err(|_| TargetActionProjectionErrorV1::InvalidCanonicalProjection),
        _ => Err(TargetActionProjectionErrorV1::InvalidCanonicalProjection),
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IssueBootstrapMandateError {
    #[error("IssueBootstrapMandate accepts exactly zero supplied Mandates")]
    SuppliedMandatesForbidden,
    #[error("Bootstrap Mandate target is explicitly excluded")]
    TargetExcluded,
    #[error("Bootstrap Mandate target subject must contain between 1 and 256 ASCII bytes")]
    InvalidTargetSubject,
    #[error("Bootstrap Mandate revision must be nonzero")]
    ZeroRevision,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
