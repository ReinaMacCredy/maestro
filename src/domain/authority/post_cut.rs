use std::collections::BTreeSet;

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::continuity::{AuthorityContinuityClosureIdV1, ContinuityReferenceV1};
use super::identity::StateTokenIdV1;

const MAX_AUTHORITY_CONSUMPTION_REFS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LinearizationFenceCarrierV1 {
    SameStoreCommit = 1,
    ProtectedLocatorCas = 2,
    ProtectedRepositoryGenerationCas = 3,
    ProtectedSnapshot = 4,
}

impl TryFrom<u8> for LinearizationFenceCarrierV1 {
    type Error = AuthorityPostCutErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::SameStoreCommit),
            2 => Ok(Self::ProtectedLocatorCas),
            3 => Ok(Self::ProtectedRepositoryGenerationCas),
            4 => Ok(Self::ProtectedSnapshot),
            _ => Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinearizationCoverageWitnessV1 {
    fence_subject_ref: ContinuityReferenceV1,
    fence_carrier: LinearizationFenceCarrierV1,
    fence_carrier_ref: ContinuityReferenceV1,
    attempt_ref: ContinuityReferenceV1,
    semantic_point_ref: ContinuityReferenceV1,
    covered_closure_ref: ContinuityReferenceV1,
    conservative_point_envelope_ref: ContinuityReferenceV1,
    carrier_revision_ref: ContinuityReferenceV1,
}

impl LinearizationCoverageWitnessV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.linearization-coverage-witness.v1";

    #[expect(
        clippy::too_many_arguments,
        reason = "the frozen witness schema binds every linearization commitment explicitly"
    )]
    pub fn new(
        fence_subject_ref: ContinuityReferenceV1,
        fence_carrier: LinearizationFenceCarrierV1,
        fence_carrier_ref: ContinuityReferenceV1,
        attempt_ref: ContinuityReferenceV1,
        semantic_point_ref: ContinuityReferenceV1,
        covered_closure_ref: ContinuityReferenceV1,
        conservative_point_envelope_ref: ContinuityReferenceV1,
        carrier_revision_ref: ContinuityReferenceV1,
    ) -> Result<Self, AuthorityPostCutErrorV1> {
        let references = [
            fence_subject_ref,
            fence_carrier_ref,
            attempt_ref,
            semantic_point_ref,
            covered_closure_ref,
            conservative_point_envelope_ref,
            carrier_revision_ref,
        ];
        require_nonzero(&references)?;
        Ok(Self {
            fence_subject_ref,
            fence_carrier,
            fence_carrier_ref,
            attempt_ref,
            semantic_point_ref,
            covered_closure_ref,
            conservative_point_envelope_ref,
            carrier_revision_ref,
        })
    }

    pub const fn fence_subject_ref(&self) -> ContinuityReferenceV1 {
        self.fence_subject_ref
    }

    pub const fn fence_carrier(&self) -> LinearizationFenceCarrierV1 {
        self.fence_carrier
    }

    pub const fn fence_carrier_ref(&self) -> ContinuityReferenceV1 {
        self.fence_carrier_ref
    }

    pub const fn attempt_ref(&self) -> ContinuityReferenceV1 {
        self.attempt_ref
    }

    pub const fn semantic_point_ref(&self) -> ContinuityReferenceV1 {
        self.semantic_point_ref
    }

    pub const fn covered_closure_ref(&self) -> ContinuityReferenceV1 {
        self.covered_closure_ref
    }

    pub const fn conservative_point_envelope_ref(&self) -> ContinuityReferenceV1 {
        self.conservative_point_envelope_ref
    }

    pub const fn carrier_revision_ref(&self) -> ContinuityReferenceV1 {
        self.carrier_revision_ref
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            reference(self.fence_subject_ref),
            CborValue::Array(vec![
                CborValue::Unsigned(self.fence_carrier as u64),
                reference(self.fence_carrier_ref),
            ]),
            reference(self.attempt_ref),
            reference(self.semantic_point_ref),
            reference(self.covered_closure_ref),
            reference(self.conservative_point_envelope_ref),
            reference(self.carrier_revision_ref),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, AuthorityPostCutErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let fields = exact_array(&value, 8)?;
        require_domain(&fields[0], Self::SCHEMA_DOMAIN)?;
        let carrier = exact_array(&fields[2], 2)?;
        let witness = Self::new(
            parse_reference(&fields[1])?,
            LinearizationFenceCarrierV1::try_from(exact_u8(&carrier[0])?)?,
            parse_reference(&carrier[1])?,
            parse_reference(&fields[3])?,
            parse_reference(&fields[4])?,
            parse_reference(&fields[5])?,
            parse_reference(&fields[6])?,
            parse_reference(&fields[7])?,
        )?;
        if witness.canonical_bytes()? != bytes {
            return Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier);
        }
        Ok(witness)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityPostCutConsequenceSetV1 {
    authority_continuity_closure_ref: ContinuityReferenceV1,
    closure_id: AuthorityContinuityClosureIdV1,
    successor_state_token: StateTokenIdV1,
    action_request_commitment: ContinuityReferenceV1,
    success_visible_continuity_state_ref: ContinuityReferenceV1,
    selected_authority_consumption_refs: Vec<ContinuityReferenceV1>,
    phase_owned_semantic_mutation_ref: ContinuityReferenceV1,
    primary_authorization_receipt_ref: ContinuityReferenceV1,
    action_result_ref: ContinuityReferenceV1,
    active_idempotency_mapping_ref: ContinuityReferenceV1,
    linearization_coverage_witness_ref: ContinuityReferenceV1,
    context_current_continuity_relation_ref: ContinuityReferenceV1,
}

impl AuthorityContinuityPostCutConsequenceSetV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.authority-continuity-post-cut-consequence-set.v1";

    #[expect(
        clippy::too_many_arguments,
        reason = "the row-12J contract is an exact indivisible consequence set"
    )]
    pub fn new(
        authority_continuity_closure_ref: ContinuityReferenceV1,
        closure_id: AuthorityContinuityClosureIdV1,
        successor_state_token: StateTokenIdV1,
        action_request_commitment: ContinuityReferenceV1,
        success_visible_continuity_state_ref: ContinuityReferenceV1,
        mut selected_authority_consumption_refs: Vec<ContinuityReferenceV1>,
        phase_owned_semantic_mutation_ref: ContinuityReferenceV1,
        primary_authorization_receipt_ref: ContinuityReferenceV1,
        action_result_ref: ContinuityReferenceV1,
        active_idempotency_mapping_ref: ContinuityReferenceV1,
        linearization_coverage_witness_ref: ContinuityReferenceV1,
        context_current_continuity_relation_ref: ContinuityReferenceV1,
    ) -> Result<Self, AuthorityPostCutErrorV1> {
        if selected_authority_consumption_refs.len() > MAX_AUTHORITY_CONSUMPTION_REFS {
            return Err(AuthorityPostCutErrorV1::AuthorityConsumptionBoundExceeded);
        }
        selected_authority_consumption_refs.sort_unstable();
        if selected_authority_consumption_refs
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != selected_authority_consumption_refs.len()
        {
            return Err(AuthorityPostCutErrorV1::DuplicateAuthorityConsumption);
        }
        require_nonzero(&selected_authority_consumption_refs)?;
        require_nonzero(&[
            authority_continuity_closure_ref,
            action_request_commitment,
            success_visible_continuity_state_ref,
            phase_owned_semantic_mutation_ref,
            primary_authorization_receipt_ref,
            action_result_ref,
            active_idempotency_mapping_ref,
            linearization_coverage_witness_ref,
            context_current_continuity_relation_ref,
        ])?;
        if closure_id.as_bytes() == &[0; 32] || successor_state_token.as_bytes() == &[0; 32] {
            return Err(AuthorityPostCutErrorV1::ZeroCommitment);
        }
        Ok(Self {
            authority_continuity_closure_ref,
            closure_id,
            successor_state_token,
            action_request_commitment,
            success_visible_continuity_state_ref,
            selected_authority_consumption_refs,
            phase_owned_semantic_mutation_ref,
            primary_authorization_receipt_ref,
            action_result_ref,
            active_idempotency_mapping_ref,
            linearization_coverage_witness_ref,
            context_current_continuity_relation_ref,
        })
    }

    pub const fn closure_id(&self) -> AuthorityContinuityClosureIdV1 {
        self.closure_id
    }

    pub const fn authority_continuity_closure_ref(&self) -> ContinuityReferenceV1 {
        self.authority_continuity_closure_ref
    }

    pub const fn successor_state_token(&self) -> StateTokenIdV1 {
        self.successor_state_token
    }

    pub const fn action_request_commitment(&self) -> ContinuityReferenceV1 {
        self.action_request_commitment
    }

    pub const fn success_visible_continuity_state_ref(&self) -> ContinuityReferenceV1 {
        self.success_visible_continuity_state_ref
    }

    pub fn selected_authority_consumption_refs(&self) -> &[ContinuityReferenceV1] {
        &self.selected_authority_consumption_refs
    }

    pub const fn action_result_ref(&self) -> ContinuityReferenceV1 {
        self.action_result_ref
    }

    pub const fn phase_owned_semantic_mutation_ref(&self) -> ContinuityReferenceV1 {
        self.phase_owned_semantic_mutation_ref
    }

    pub const fn primary_authorization_receipt_ref(&self) -> ContinuityReferenceV1 {
        self.primary_authorization_receipt_ref
    }

    pub const fn active_idempotency_mapping_ref(&self) -> ContinuityReferenceV1 {
        self.active_idempotency_mapping_ref
    }

    pub const fn linearization_coverage_witness_ref(&self) -> ContinuityReferenceV1 {
        self.linearization_coverage_witness_ref
    }

    pub const fn context_current_continuity_relation_ref(&self) -> ContinuityReferenceV1 {
        self.context_current_continuity_relation_ref
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            reference(self.authority_continuity_closure_ref),
            CborValue::Bytes(self.closure_id.as_bytes().to_vec()),
            CborValue::Bytes(self.successor_state_token.as_bytes().to_vec()),
            reference(self.action_request_commitment),
            reference(self.success_visible_continuity_state_ref),
            CborValue::Array(
                self.selected_authority_consumption_refs
                    .iter()
                    .copied()
                    .map(reference)
                    .collect(),
            ),
            reference(self.phase_owned_semantic_mutation_ref),
            reference(self.primary_authorization_receipt_ref),
            reference(self.action_result_ref),
            reference(self.active_idempotency_mapping_ref),
            reference(self.linearization_coverage_witness_ref),
            reference(self.context_current_continuity_relation_ref),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, AuthorityPostCutErrorV1> {
        let value = deterministic_cbor::decode(bytes)?;
        let fields = exact_array(&value, 13)?;
        require_domain(&fields[0], Self::SCHEMA_DOMAIN)?;
        let CborValue::Array(consumptions) = &fields[6] else {
            return Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier);
        };
        let consequence = Self::new(
            parse_reference(&fields[1])?,
            AuthorityContinuityClosureIdV1::from_digest(exact_digest(&fields[2])?),
            StateTokenIdV1::from_digest(exact_digest(&fields[3])?),
            parse_reference(&fields[4])?,
            parse_reference(&fields[5])?,
            consumptions
                .iter()
                .map(parse_reference)
                .collect::<Result<Vec<_>, _>>()?,
            parse_reference(&fields[7])?,
            parse_reference(&fields[8])?,
            parse_reference(&fields[9])?,
            parse_reference(&fields[10])?,
            parse_reference(&fields[11])?,
            parse_reference(&fields[12])?,
        )?;
        if consequence.canonical_bytes()? != bytes {
            return Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier);
        }
        Ok(consequence)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityPostCutErrorV1 {
    #[error("Authority post-cut commitment must be nonzero")]
    ZeroCommitment,
    #[error("Authority consumption set exceeds its finite bound")]
    AuthorityConsumptionBoundExceeded,
    #[error("Authority consumption set contains a duplicate")]
    DuplicateAuthorityConsumption,
    #[error("Authority post-cut carrier is malformed or noncanonical")]
    InvalidCanonicalCarrier,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

fn require_nonzero(values: &[ContinuityReferenceV1]) -> Result<(), AuthorityPostCutErrorV1> {
    if values.iter().any(|value| value.as_bytes() == &[0; 32]) {
        Err(AuthorityPostCutErrorV1::ZeroCommitment)
    } else {
        Ok(())
    }
}

fn reference(value: ContinuityReferenceV1) -> CborValue {
    CborValue::Bytes(value.as_bytes().to_vec())
}

fn parse_reference(value: &CborValue) -> Result<ContinuityReferenceV1, AuthorityPostCutErrorV1> {
    Ok(ContinuityReferenceV1::from_digest(exact_digest(value)?))
}

fn exact_array(
    value: &CborValue,
    expected: usize,
) -> Result<&[CborValue], AuthorityPostCutErrorV1> {
    match value {
        CborValue::Array(values) if values.len() == expected => Ok(values),
        _ => Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
    }
}

fn require_domain(value: &CborValue, expected: &str) -> Result<(), AuthorityPostCutErrorV1> {
    match value {
        CborValue::Text(actual) if actual == expected => Ok(()),
        _ => Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
    }
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], AuthorityPostCutErrorV1> {
    match value {
        CborValue::Bytes(bytes) => bytes
            .as_slice()
            .try_into()
            .map_err(|_| AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
        _ => Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
    }
}

fn exact_u8(value: &CborValue) -> Result<u8, AuthorityPostCutErrorV1> {
    match value {
        CborValue::Unsigned(value) => (*value)
            .try_into()
            .map_err(|_| AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
        _ => Err(AuthorityPostCutErrorV1::InvalidCanonicalCarrier),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_id(seed: u8) -> ContinuityReferenceV1 {
        ContinuityReferenceV1::from_digest([seed; 32])
    }

    #[test]
    fn witness_round_trips_and_unknown_carrier_is_refused() {
        let witness = LinearizationCoverageWitnessV1::new(
            reference_id(1),
            LinearizationFenceCarrierV1::SameStoreCommit,
            reference_id(2),
            reference_id(3),
            reference_id(4),
            reference_id(5),
            reference_id(6),
            reference_id(7),
        )
        .unwrap();
        let bytes = witness.canonical_bytes().unwrap();
        assert_eq!(
            LinearizationCoverageWitnessV1::decode(&bytes).unwrap(),
            witness
        );

        let mut value = deterministic_cbor::decode(&bytes).unwrap();
        let CborValue::Array(fields) = &mut value else {
            unreachable!();
        };
        let CborValue::Array(carrier) = &mut fields[2] else {
            unreachable!();
        };
        carrier[0] = CborValue::Unsigned(5);
        let mutant = deterministic_cbor::encode(&value).unwrap();
        assert!(LinearizationCoverageWitnessV1::decode(&mutant).is_err());
    }

    #[test]
    fn post_cut_set_round_trips_and_rejects_duplicate_consumption() {
        let consequence = AuthorityContinuityPostCutConsequenceSetV1::new(
            reference_id(1),
            AuthorityContinuityClosureIdV1::from_digest([2; 32]),
            StateTokenIdV1::from_digest([3; 32]),
            reference_id(4),
            reference_id(5),
            vec![],
            reference_id(6),
            reference_id(7),
            reference_id(8),
            reference_id(9),
            reference_id(10),
            reference_id(11),
        )
        .unwrap();
        let bytes = consequence.canonical_bytes().unwrap();
        assert_eq!(
            AuthorityContinuityPostCutConsequenceSetV1::decode(&bytes).unwrap(),
            consequence
        );
        assert!(matches!(
            AuthorityContinuityPostCutConsequenceSetV1::new(
                reference_id(1),
                AuthorityContinuityClosureIdV1::from_digest([2; 32]),
                StateTokenIdV1::from_digest([3; 32]),
                reference_id(4),
                reference_id(5),
                vec![reference_id(12), reference_id(12)],
                reference_id(6),
                reference_id(7),
                reference_id(8),
                reference_id(9),
                reference_id(10),
                reference_id(11),
            ),
            Err(AuthorityPostCutErrorV1::DuplicateAuthorityConsumption)
        ));
    }
}
