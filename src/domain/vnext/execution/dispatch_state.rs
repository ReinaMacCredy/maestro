//! Closed, candidate-only representation of the DispatchAttempt crossing state.
//!
//! This module deliberately contains no transport, persistence, CAS, retry, or
//! release-capability implementation. It only makes the frozen state algebra
//! and its legal transitions representable and independently checkable.

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

pub const DISPATCH_ATTEMPT_STATE_SCHEMA: &str = "maestro.vnext.dispatch-attempt-state.v1";
pub const DISPATCH_CROSSING_SEAL_SCHEMA: &str = "maestro.vnext.dispatch-crossing-seal.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DispatchCommitmentV1([u8; 32]);

impl DispatchCommitmentV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, DispatchStateError> {
        if bytes == [0; 32] {
            return Err(DispatchStateError::ZeroCommitment);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Bytes(self.0.to_vec())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchBindingV1 {
    attempt_id: DispatchCommitmentV1,
    attempt_revision: u64,
    effect_intent_home_id: DispatchCommitmentV1,
    effect_intent_use_fence_id: DispatchCommitmentV1,
    application_envelope_id: DispatchCommitmentV1,
    provider_operation_contract_id: DispatchCommitmentV1,
    provider_scope_id: DispatchCommitmentV1,
    provider_key_id: DispatchCommitmentV1,
    credential_id: DispatchCommitmentV1,
    authority_basis_id: DispatchCommitmentV1,
    dispatch_fence_id: DispatchCommitmentV1,
    material_stamp_id: DispatchCommitmentV1,
    run_set_revision_id: DispatchCommitmentV1,
    accounting_basis_id: DispatchCommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchBindingPartsV1 {
    pub attempt_id: DispatchCommitmentV1,
    pub attempt_revision: u64,
    pub effect_intent_home_id: DispatchCommitmentV1,
    pub effect_intent_use_fence_id: DispatchCommitmentV1,
    pub application_envelope_id: DispatchCommitmentV1,
    pub provider_operation_contract_id: DispatchCommitmentV1,
    pub provider_scope_id: DispatchCommitmentV1,
    pub provider_key_id: DispatchCommitmentV1,
    pub credential_id: DispatchCommitmentV1,
    pub authority_basis_id: DispatchCommitmentV1,
    pub dispatch_fence_id: DispatchCommitmentV1,
    pub material_stamp_id: DispatchCommitmentV1,
    pub run_set_revision_id: DispatchCommitmentV1,
    pub accounting_basis_id: DispatchCommitmentV1,
}

impl DispatchBindingV1 {
    pub fn new(parts: DispatchBindingPartsV1) -> Result<Self, DispatchStateError> {
        if parts.attempt_revision == 0 {
            return Err(DispatchStateError::ZeroAttemptRevision);
        }
        Ok(Self {
            attempt_id: parts.attempt_id,
            attempt_revision: parts.attempt_revision,
            effect_intent_home_id: parts.effect_intent_home_id,
            effect_intent_use_fence_id: parts.effect_intent_use_fence_id,
            application_envelope_id: parts.application_envelope_id,
            provider_operation_contract_id: parts.provider_operation_contract_id,
            provider_scope_id: parts.provider_scope_id,
            provider_key_id: parts.provider_key_id,
            credential_id: parts.credential_id,
            authority_basis_id: parts.authority_basis_id,
            dispatch_fence_id: parts.dispatch_fence_id,
            material_stamp_id: parts.material_stamp_id,
            run_set_revision_id: parts.run_set_revision_id,
            accounting_basis_id: parts.accounting_basis_id,
        })
    }

    pub const fn attempt_revision(&self) -> u64 {
        self.attempt_revision
    }

    pub const fn attempt_id(&self) -> DispatchCommitmentV1 {
        self.attempt_id
    }

    pub const fn application_envelope_id(&self) -> DispatchCommitmentV1 {
        self.application_envelope_id
    }

    pub const fn provider_operation_contract_id(&self) -> DispatchCommitmentV1 {
        self.provider_operation_contract_id
    }

    pub const fn provider_scope_id(&self) -> DispatchCommitmentV1 {
        self.provider_scope_id
    }

    pub const fn provider_key_id(&self) -> DispatchCommitmentV1 {
        self.provider_key_id
    }

    pub const fn credential_id(&self) -> DispatchCommitmentV1 {
        self.credential_id
    }

    pub const fn material_stamp_id(&self) -> DispatchCommitmentV1 {
        self.material_stamp_id
    }

    pub const fn run_set_revision_id(&self) -> DispatchCommitmentV1 {
        self.run_set_revision_id
    }

    pub const fn accounting_basis_id(&self) -> DispatchCommitmentV1 {
        self.accounting_basis_id
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            self.attempt_id.canonical_value(),
            CborValue::Unsigned(self.attempt_revision),
            self.effect_intent_home_id.canonical_value(),
            self.effect_intent_use_fence_id.canonical_value(),
            self.application_envelope_id.canonical_value(),
            self.provider_operation_contract_id.canonical_value(),
            self.provider_scope_id.canonical_value(),
            self.provider_key_id.canonical_value(),
            self.credential_id.canonical_value(),
            self.authority_basis_id.canonical_value(),
            self.dispatch_fence_id.canonical_value(),
            self.material_stamp_id.canonical_value(),
            self.run_set_revision_id.canonical_value(),
            self.accounting_basis_id.canonical_value(),
        ])
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, DispatchStateError> {
        let CborValue::Array(fields) = value else {
            return Err(DispatchStateError::InvalidCanonicalState);
        };
        let [
            CborValue::Unsigned(1),
            attempt_id,
            CborValue::Unsigned(attempt_revision),
            home,
            use_fence,
            envelope,
            operation,
            scope,
            provider_key,
            credential,
            authority_basis,
            dispatch_fence,
            material_stamp,
            run_set_revision,
            accounting_basis,
        ] = fields.as_slice()
        else {
            return Err(DispatchStateError::InvalidCanonicalState);
        };
        Self::new(DispatchBindingPartsV1 {
            attempt_id: parse_commitment(attempt_id)?,
            attempt_revision: *attempt_revision,
            effect_intent_home_id: parse_commitment(home)?,
            effect_intent_use_fence_id: parse_commitment(use_fence)?,
            application_envelope_id: parse_commitment(envelope)?,
            provider_operation_contract_id: parse_commitment(operation)?,
            provider_scope_id: parse_commitment(scope)?,
            provider_key_id: parse_commitment(provider_key)?,
            credential_id: parse_commitment(credential)?,
            authority_basis_id: parse_commitment(authority_basis)?,
            dispatch_fence_id: parse_commitment(dispatch_fence)?,
            material_stamp_id: parse_commitment(material_stamp)?,
            run_set_revision_id: parse_commitment(run_set_revision)?,
            accounting_basis_id: parse_commitment(accounting_basis)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchCrossingSealV1 {
    seal_id: DispatchCommitmentV1,
    binding: DispatchBindingV1,
}

impl DispatchCrossingSealV1 {
    pub fn new(seal_id: DispatchCommitmentV1, binding: DispatchBindingV1) -> Self {
        Self { seal_id, binding }
    }

    pub const fn seal_id(&self) -> DispatchCommitmentV1 {
        self.seal_id
    }

    pub const fn binding(&self) -> &DispatchBindingV1 {
        &self.binding
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            self.seal_id.canonical_value(),
            self.binding.canonical_value(),
        ])
    }

    fn from_canonical_value(value: &CborValue) -> Result<Self, DispatchStateError> {
        let CborValue::Array(fields) = value else {
            return Err(DispatchStateError::InvalidCanonicalState);
        };
        let [CborValue::Unsigned(1), seal, binding] = fields.as_slice() else {
            return Err(DispatchStateError::InvalidCanonicalState);
        };
        Ok(Self::new(
            parse_commitment(seal)?,
            DispatchBindingV1::from_canonical_value(binding)?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DispatchAttemptOutcomeV1 {
    LocallyRejected,
    DefinitelyNotSent,
    ResponseReceived,
    AmbiguousTransport,
}

impl DispatchAttemptOutcomeV1 {
    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::LocallyRejected => 1,
            Self::DefinitelyNotSent => 2,
            Self::ResponseReceived => 3,
            Self::AmbiguousTransport => 4,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocallyRejected => "locally_rejected",
            Self::DefinitelyNotSent => "definitely_not_sent",
            Self::ResponseReceived => "response_received",
            Self::AmbiguousTransport => "ambiguous_transport",
        }
    }

    pub fn from_numeric_tag(tag: u64) -> Result<Self, DispatchStateError> {
        match tag {
            1 => Ok(Self::LocallyRejected),
            2 => Ok(Self::DefinitelyNotSent),
            3 => Ok(Self::ResponseReceived),
            4 => Ok(Self::AmbiguousTransport),
            _ => Err(DispatchStateError::UnknownOutcomeTag(tag)),
        }
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![CborValue::Unsigned(self.numeric_tag())])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReservedUnsealedV1 {
    binding: DispatchBindingV1,
}

impl ReservedUnsealedV1 {
    pub fn new(binding: DispatchBindingV1) -> Self {
        Self { binding }
    }

    pub const fn binding(&self) -> &DispatchBindingV1 {
        &self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedInFlightV1 {
    binding: DispatchBindingV1,
    seal: DispatchCrossingSealV1,
}

impl SealedInFlightV1 {
    pub fn new(
        binding: DispatchBindingV1,
        seal: DispatchCrossingSealV1,
    ) -> Result<Self, DispatchStateError> {
        ensure_identical_seal(&binding, &seal)?;
        Ok(Self { binding, seal })
    }

    pub const fn binding(&self) -> &DispatchBindingV1 {
        &self.binding
    }

    pub const fn seal(&self) -> &DispatchCrossingSealV1 {
        &self.seal
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreSealLocallyRejectedV1 {
    binding: DispatchBindingV1,
    rejection_evidence_id: DispatchCommitmentV1,
}

impl PreSealLocallyRejectedV1 {
    pub fn new(binding: DispatchBindingV1, rejection_evidence_id: DispatchCommitmentV1) -> Self {
        Self {
            binding,
            rejection_evidence_id,
        }
    }

    pub const fn binding(&self) -> &DispatchBindingV1 {
        &self.binding
    }

    pub const fn rejection_evidence_id(&self) -> DispatchCommitmentV1 {
        self.rejection_evidence_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedDispatchTerminalV1 {
    binding: DispatchBindingV1,
    seal: DispatchCrossingSealV1,
    outcome: SealedDispatchOutcomeV1,
    terminal_evidence_id: DispatchCommitmentV1,
}

impl SealedDispatchTerminalV1 {
    pub fn new(
        binding: DispatchBindingV1,
        seal: DispatchCrossingSealV1,
        outcome: SealedDispatchOutcomeV1,
        terminal_evidence_id: DispatchCommitmentV1,
    ) -> Result<Self, DispatchStateError> {
        ensure_identical_seal(&binding, &seal)?;
        Ok(Self {
            binding,
            seal,
            outcome,
            terminal_evidence_id,
        })
    }

    pub const fn binding(&self) -> &DispatchBindingV1 {
        &self.binding
    }

    pub const fn seal(&self) -> &DispatchCrossingSealV1 {
        &self.seal
    }

    pub const fn outcome(&self) -> SealedDispatchOutcomeV1 {
        self.outcome
    }

    pub const fn terminal_evidence_id(&self) -> DispatchCommitmentV1 {
        self.terminal_evidence_id
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SealedDispatchOutcomeV1 {
    DefinitelyNotSent,
    ResponseReceived,
    AmbiguousTransport,
}

impl SealedDispatchOutcomeV1 {
    pub const fn as_dispatch_outcome(self) -> DispatchAttemptOutcomeV1 {
        match self {
            Self::DefinitelyNotSent => DispatchAttemptOutcomeV1::DefinitelyNotSent,
            Self::ResponseReceived => DispatchAttemptOutcomeV1::ResponseReceived,
            Self::AmbiguousTransport => DispatchAttemptOutcomeV1::AmbiguousTransport,
        }
    }

    pub fn from_dispatch_outcome(
        outcome: DispatchAttemptOutcomeV1,
    ) -> Result<Self, DispatchStateError> {
        match outcome {
            DispatchAttemptOutcomeV1::LocallyRejected => {
                Err(DispatchStateError::SealedLocalRejection)
            }
            DispatchAttemptOutcomeV1::DefinitelyNotSent => Ok(Self::DefinitelyNotSent),
            DispatchAttemptOutcomeV1::ResponseReceived => Ok(Self::ResponseReceived),
            DispatchAttemptOutcomeV1::AmbiguousTransport => Ok(Self::AmbiguousTransport),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchAttemptTerminalV1 {
    PreSealLocallyRejected(Box<PreSealLocallyRejectedV1>),
    SealedDispatchTerminal(Box<SealedDispatchTerminalV1>),
}

impl DispatchAttemptTerminalV1 {
    pub const fn binding(&self) -> &DispatchBindingV1 {
        match self {
            Self::PreSealLocallyRejected(terminal) => terminal.binding(),
            Self::SealedDispatchTerminal(terminal) => terminal.binding(),
        }
    }

    pub const fn outcome(&self) -> DispatchAttemptOutcomeV1 {
        match self {
            Self::PreSealLocallyRejected(_) => DispatchAttemptOutcomeV1::LocallyRejected,
            Self::SealedDispatchTerminal(terminal) => terminal.outcome().as_dispatch_outcome(),
        }
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::PreSealLocallyRejected(terminal) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                terminal.binding.canonical_value(),
                terminal.rejection_evidence_id.canonical_value(),
                DispatchAttemptOutcomeV1::LocallyRejected.canonical_value(),
            ]),
            Self::SealedDispatchTerminal(terminal) => CborValue::Array(vec![
                CborValue::Unsigned(2),
                terminal.binding.canonical_value(),
                terminal.seal.canonical_value(),
                terminal.outcome.as_dispatch_outcome().canonical_value(),
                terminal.terminal_evidence_id.canonical_value(),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchAttemptStateV1 {
    ReservedUnsealed(Box<ReservedUnsealedV1>),
    SealedInFlight(Box<SealedInFlightV1>),
    Terminal(DispatchAttemptTerminalV1),
}

impl DispatchAttemptStateV1 {
    pub const fn binding(&self) -> &DispatchBindingV1 {
        match self {
            Self::ReservedUnsealed(state) => state.binding(),
            Self::SealedInFlight(state) => state.binding(),
            Self::Terminal(terminal) => terminal.binding(),
        }
    }

    pub const fn numeric_tag(&self) -> u64 {
        match self {
            Self::ReservedUnsealed(_) => 1,
            Self::SealedInFlight(_) => 2,
            Self::Terminal(_) => 3,
        }
    }

    pub fn terminal_outcome(&self) -> Result<DispatchAttemptOutcomeV1, DispatchStateError> {
        match self {
            Self::Terminal(terminal) => Ok(terminal.outcome()),
            Self::ReservedUnsealed(_) | Self::SealedInFlight(_) => {
                Err(DispatchStateError::NonTerminalOutcome)
            }
        }
    }

    pub const fn can_reconstruct_live_release_capability(&self) -> bool {
        false
    }

    pub fn validate_transition_to(&self, next: &Self) -> Result<(), DispatchStateError> {
        match (self, next) {
            (Self::ReservedUnsealed(previous), Self::SealedInFlight(next)) => {
                ensure_identical_binding(previous.binding(), next.binding())?;
                ensure_identical_seal(next.binding(), next.seal())
            }
            (
                Self::ReservedUnsealed(previous),
                Self::Terminal(DispatchAttemptTerminalV1::PreSealLocallyRejected(next)),
            ) => ensure_identical_binding(previous.binding(), next.binding()),
            (
                Self::SealedInFlight(previous),
                Self::Terminal(DispatchAttemptTerminalV1::SealedDispatchTerminal(next)),
            ) => {
                ensure_identical_binding(previous.binding(), next.binding())?;
                if previous.seal() != next.seal() {
                    return Err(DispatchStateError::SealReplacement);
                }
                ensure_identical_seal(next.binding(), next.seal())
            }
            (Self::Terminal(_), _) => Err(DispatchStateError::TerminalEscape),
            (
                Self::ReservedUnsealed(_),
                Self::Terminal(DispatchAttemptTerminalV1::SealedDispatchTerminal(_)),
            ) => Err(DispatchStateError::DirectReservedToSealedTerminal),
            (
                Self::SealedInFlight(_),
                Self::Terminal(DispatchAttemptTerminalV1::PreSealLocallyRejected(_)),
            ) => Err(DispatchStateError::SealedLocalRejection),
            _ => Err(DispatchStateError::IllegalTransition),
        }
    }

    pub fn canonical_value(&self) -> CborValue {
        match self {
            Self::ReservedUnsealed(state) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(self.numeric_tag()),
                state.binding.canonical_value(),
            ]),
            Self::SealedInFlight(state) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(self.numeric_tag()),
                state.binding.canonical_value(),
                state.seal.canonical_value(),
            ]),
            Self::Terminal(terminal) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(self.numeric_tag()),
                terminal.canonical_value(),
            ]),
        }
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, DispatchStateError> {
        let CborValue::Array(fields) = value else {
            return Err(DispatchStateError::InvalidCanonicalState);
        };
        let [
            CborValue::Unsigned(1),
            CborValue::Unsigned(tag),
            remainder @ ..,
        ] = fields.as_slice()
        else {
            return Err(DispatchStateError::InvalidCanonicalState);
        };
        let state = match (*tag, remainder) {
            (1, [binding]) => Self::ReservedUnsealed(Box::new(ReservedUnsealedV1::new(
                DispatchBindingV1::from_canonical_value(binding)?,
            ))),
            (2, [binding, seal]) => {
                let binding = DispatchBindingV1::from_canonical_value(binding)?;
                let seal = DispatchCrossingSealV1::from_canonical_value(seal)?;
                Self::SealedInFlight(Box::new(SealedInFlightV1::new(binding, seal)?))
            }
            (3, [terminal]) => {
                let CborValue::Array(terminal) = terminal else {
                    return Err(DispatchStateError::InvalidCanonicalState);
                };
                let terminal = match terminal.as_slice() {
                    [CborValue::Unsigned(1), binding, evidence, outcome]
                        if parse_outcome(outcome)? == DispatchAttemptOutcomeV1::LocallyRejected =>
                    {
                        DispatchAttemptTerminalV1::PreSealLocallyRejected(Box::new(
                            PreSealLocallyRejectedV1::new(
                                DispatchBindingV1::from_canonical_value(binding)?,
                                parse_commitment(evidence)?,
                            ),
                        ))
                    }
                    [CborValue::Unsigned(2), binding, seal, outcome, evidence] => {
                        let outcome = SealedDispatchOutcomeV1::from_dispatch_outcome(
                            parse_outcome(outcome)?,
                        )?;
                        DispatchAttemptTerminalV1::SealedDispatchTerminal(Box::new(
                            SealedDispatchTerminalV1::new(
                                DispatchBindingV1::from_canonical_value(binding)?,
                                DispatchCrossingSealV1::from_canonical_value(seal)?,
                                outcome,
                                parse_commitment(evidence)?,
                            )?,
                        ))
                    }
                    _ => return Err(DispatchStateError::InvalidCanonicalState),
                };
                Self::Terminal(terminal)
            }
            _ => return Err(DispatchStateError::InvalidCanonicalState),
        };
        if state.canonical_value() != *value {
            return Err(DispatchStateError::InvalidCanonicalState);
        }
        Ok(state)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchRaceDescriptorV1;

impl DispatchRaceDescriptorV1 {
    pub const fn persisted_winner_count(self) -> u64 {
        1
    }

    pub const fn release_scope(self) -> &'static str {
        "successful_live_seal_cas_caller_only"
    }

    pub const fn losing_writer_may_dispatch(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchRecoveryDescriptorV1;

impl DispatchRecoveryDescriptorV1 {
    pub const fn reconstruction_io_operations(self) -> u64 {
        0
    }

    pub const fn reconstructs_release_capability(self) -> bool {
        false
    }

    pub const fn permits_synthetic_truth(self) -> bool {
        false
    }

    pub const fn permits_synthetic_refund(self) -> bool {
        false
    }

    pub const fn permits_synthetic_retry(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DispatchStateError {
    #[error("dispatch commitments cannot be all-zero")]
    ZeroCommitment,
    #[error("DispatchAttempt revision must be non-zero")]
    ZeroAttemptRevision,
    #[error("unknown DispatchAttempt outcome tag {0}")]
    UnknownOutcomeTag(u64),
    #[error("nonterminal DispatchAttempt state has no outcome")]
    NonTerminalOutcome,
    #[error("sealed dispatch cannot project locally_rejected")]
    SealedLocalRejection,
    #[error("DispatchAttempt binding changed across a transition")]
    BindingReplacement,
    #[error("crossing seal does not exactly bind the DispatchAttempt basis")]
    SealBindingMismatch,
    #[error("crossing seal was replaced after sealing")]
    SealReplacement,
    #[error("ReservedUnsealed cannot transition directly to a sealed terminal")]
    DirectReservedToSealedTerminal,
    #[error("terminal DispatchAttempt state cannot transition")]
    TerminalEscape,
    #[error("illegal DispatchAttempt state transition")]
    IllegalTransition,
    #[error("stored DispatchAttempt state is not the exact canonical carrier")]
    InvalidCanonicalState,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

fn parse_commitment(value: &CborValue) -> Result<DispatchCommitmentV1, DispatchStateError> {
    let CborValue::Bytes(bytes) = value else {
        return Err(DispatchStateError::InvalidCanonicalState);
    };
    let bytes = bytes
        .as_slice()
        .try_into()
        .map_err(|_| DispatchStateError::InvalidCanonicalState)?;
    DispatchCommitmentV1::new(bytes)
}

fn parse_outcome(value: &CborValue) -> Result<DispatchAttemptOutcomeV1, DispatchStateError> {
    let CborValue::Array(fields) = value else {
        return Err(DispatchStateError::InvalidCanonicalState);
    };
    let [CborValue::Unsigned(tag)] = fields.as_slice() else {
        return Err(DispatchStateError::InvalidCanonicalState);
    };
    DispatchAttemptOutcomeV1::from_numeric_tag(*tag)
}

fn ensure_identical_binding(
    previous: &DispatchBindingV1,
    next: &DispatchBindingV1,
) -> Result<(), DispatchStateError> {
    if previous == next {
        Ok(())
    } else {
        Err(DispatchStateError::BindingReplacement)
    }
}

fn ensure_identical_seal(
    binding: &DispatchBindingV1,
    seal: &DispatchCrossingSealV1,
) -> Result<(), DispatchStateError> {
    if binding == seal.binding() {
        Ok(())
    } else {
        Err(DispatchStateError::SealBindingMismatch)
    }
}
