use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::closed::{AuthorityContextKindV1, TransitionGuardKindV1};
use super::super::identity::{
    AuthorityContextIdV1, AuthorityContinuityManifestIdV1, StateTokenIdV1,
};
use super::super::transition::TransitionGuardTermV1;
use super::catalog::ContinuityReferenceV1;
use super::closure::{
    AuthorityContinuityClosureIdV1, AuthorityContinuityClosureV1, AuthorityContinuityPredecessorV1,
    ContinuityCarrierProfileStatusV1, accepted_time_value, reference_array,
};
use super::totality::AuthorityContinuityManifestV1;
use super::trusted_time::{
    AcceptedAuthorityTimeFloorV1, HTimeAcceptanceRelationV1, HTimeCarryBasisV1,
};

const MAX_STATE_RECORDS: usize = 4_096;
const SUPPORTED_PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardAdmissionKindV1 {
    ExternallyRootedContextGenesis,
    Established(TransitionGuardKindV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuityDisclosureV1 {
    ProtectedComplete,
    Nondisclosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionGuardTermFactV1 {
    term: TransitionGuardTermV1,
    owner_fact: ContinuityReferenceV1,
    owner_revision: ContinuityReferenceV1,
}

impl TransitionGuardTermFactV1 {
    pub(crate) fn owner_confirmed(
        term: TransitionGuardTermV1,
        owner_fact: ContinuityReferenceV1,
        owner_revision: ContinuityReferenceV1,
    ) -> Result<Self, AuthorityContinuityStateError> {
        let fact = Self {
            term,
            owner_fact,
            owner_revision,
        };
        if is_zero_reference(fact.owner_fact)
            || is_zero_reference(fact.owner_revision)
            || fact.owner_fact == fact.owner_revision
        {
            return Err(AuthorityContinuityStateError::InvalidGuardOwnerFact);
        }
        Ok(fact)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionGuardOwnerCensusV1 {
    kind: GuardAdmissionKindV1,
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    authority_epoch: u64,
    source_cut_commitment: ContinuityReferenceV1,
    term_facts: Vec<TransitionGuardTermFactV1>,
    commitment: ContinuityReferenceV1,
}

impl TransitionGuardOwnerCensusV1 {
    #[cfg(test)]
    pub(crate) fn externally_rooted_genesis(
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        origin_commitment: ContinuityReferenceV1,
    ) -> Result<Self, AuthorityContinuityStateError> {
        Self::construct(
            GuardAdmissionKindV1::ExternallyRootedContextGenesis,
            context_id,
            store_generation,
            authority_epoch,
            origin_commitment,
            Vec::new(),
        )
    }

    pub(crate) fn from_owner_sources(
        kind: TransitionGuardKindV1,
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        source_cut_commitment: ContinuityReferenceV1,
        term_facts: Vec<TransitionGuardTermFactV1>,
    ) -> Result<Self, AuthorityContinuityStateError> {
        Self::construct(
            GuardAdmissionKindV1::Established(kind),
            context_id,
            store_generation,
            authority_epoch,
            source_cut_commitment,
            term_facts,
        )
    }

    fn construct(
        kind: GuardAdmissionKindV1,
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        source_cut_commitment: ContinuityReferenceV1,
        mut term_facts: Vec<TransitionGuardTermFactV1>,
    ) -> Result<Self, AuthorityContinuityStateError> {
        if store_generation == 0
            || authority_epoch == 0
            || is_zero_reference(source_cut_commitment)
            || !valid_term_facts(kind, &term_facts)
        {
            return Err(AuthorityContinuityStateError::GuardTermTotalityMismatch);
        }
        term_facts.sort_by_key(|fact| fact.term as u8);
        let commitment = ContinuityReferenceV1::from_digest(hash(&owner_census_value(
            kind,
            context_id,
            store_generation,
            authority_epoch,
            source_cut_commitment,
            &term_facts,
        )?)?);
        Ok(Self {
            kind,
            context_id,
            store_generation,
            authority_epoch,
            source_cut_commitment,
            term_facts,
            commitment,
        })
    }

    pub const fn commitment(&self) -> ContinuityReferenceV1 {
        self.commitment
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityTransitionGuardAdmissionInputV1 {
    pub kind: GuardAdmissionKindV1,
    pub context_kind: AuthorityContextKindV1,
    pub context_id: AuthorityContextIdV1,
    pub store_generation: u64,
    pub authority_epoch: u64,
    pub manifest_id: AuthorityContinuityManifestIdV1,
    pub closure_id: AuthorityContinuityClosureIdV1,
    pub predecessor_state_token: Option<StateTokenIdV1>,
    pub cut_sequence: u64,
    pub selected_trusted_time_stack: ContinuityReferenceV1,
    pub carrier_profile: ContinuityCarrierProfileStatusV1,
    pub accepted_time: AcceptedAuthorityTimeFloorV1,
    pub lane_state_closure_root: ContinuityReferenceV1,
    pub source_floor_root: ContinuityReferenceV1,
    pub gap_companions: Vec<ContinuityReferenceV1>,
    pub floor_provenance: Vec<ContinuityReferenceV1>,
    pub external_revision_cells: Vec<ContinuityReferenceV1>,
    pub cma_remaining_root: ContinuityReferenceV1,
    pub cma_spent_root: ContinuityReferenceV1,
    pub unresolved_effects: Vec<ContinuityReferenceV1>,
    pub term_facts: Vec<TransitionGuardTermFactV1>,
    pub owner_census: TransitionGuardOwnerCensusV1,
    pub disclosure: ContinuityDisclosureV1,
    pub protocol_version: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedTransitionGuardV1 {
    digest: ContinuityReferenceV1,
    kind: GuardAdmissionKindV1,
    context_kind: AuthorityContextKindV1,
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    authority_epoch: u64,
    manifest_id: AuthorityContinuityManifestIdV1,
    closure_id: AuthorityContinuityClosureIdV1,
    predecessor_state_token: Option<StateTokenIdV1>,
    cut_sequence: u64,
    selected_trusted_time_stack: ContinuityReferenceV1,
    carrier_profile: ContinuityCarrierProfileStatusV1,
    accepted_time: AcceptedAuthorityTimeFloorV1,
    lane_state_closure_root: ContinuityReferenceV1,
    source_floor_root: ContinuityReferenceV1,
    gap_companions: Vec<ContinuityReferenceV1>,
    floor_provenance: Vec<ContinuityReferenceV1>,
    external_revision_cells: Vec<ContinuityReferenceV1>,
    cma_remaining_root: ContinuityReferenceV1,
    cma_spent_root: ContinuityReferenceV1,
    unresolved_effects: Vec<ContinuityReferenceV1>,
    term_facts: Vec<TransitionGuardTermFactV1>,
    owner_census: TransitionGuardOwnerCensusV1,
    disclosure: ContinuityDisclosureV1,
    protocol_version: u16,
}

impl AdmittedTransitionGuardV1 {
    pub(crate) fn evaluate(
        mut input: AuthorityTransitionGuardAdmissionInputV1,
    ) -> Result<Self, AuthorityContinuityStateError> {
        validate_guard_admission(&input)?;
        normalize_record_families(&mut input);
        let digest = ContinuityReferenceV1::from_digest(hash(&guard_admission_value(&input)?)?);
        Ok(Self {
            digest,
            kind: input.kind,
            context_kind: input.context_kind,
            context_id: input.context_id,
            store_generation: input.store_generation,
            authority_epoch: input.authority_epoch,
            manifest_id: input.manifest_id,
            closure_id: input.closure_id,
            predecessor_state_token: input.predecessor_state_token,
            cut_sequence: input.cut_sequence,
            selected_trusted_time_stack: input.selected_trusted_time_stack,
            carrier_profile: input.carrier_profile,
            accepted_time: input.accepted_time,
            lane_state_closure_root: input.lane_state_closure_root,
            source_floor_root: input.source_floor_root,
            gap_companions: input.gap_companions,
            floor_provenance: input.floor_provenance,
            external_revision_cells: input.external_revision_cells,
            cma_remaining_root: input.cma_remaining_root,
            cma_spent_root: input.cma_spent_root,
            unresolved_effects: input.unresolved_effects,
            term_facts: input.term_facts,
            owner_census: input.owner_census,
            disclosure: input.disclosure,
            protocol_version: input.protocol_version,
        })
    }

    pub const fn digest(&self) -> ContinuityReferenceV1 {
        self.digest
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        guard_admission_value(&AuthorityTransitionGuardAdmissionInputV1 {
            kind: self.kind,
            context_kind: self.context_kind,
            context_id: self.context_id,
            store_generation: self.store_generation,
            authority_epoch: self.authority_epoch,
            manifest_id: self.manifest_id,
            closure_id: self.closure_id,
            predecessor_state_token: self.predecessor_state_token,
            cut_sequence: self.cut_sequence,
            selected_trusted_time_stack: self.selected_trusted_time_stack,
            carrier_profile: self.carrier_profile.clone(),
            accepted_time: self.accepted_time.clone(),
            lane_state_closure_root: self.lane_state_closure_root,
            source_floor_root: self.source_floor_root,
            gap_companions: self.gap_companions.clone(),
            floor_provenance: self.floor_provenance.clone(),
            external_revision_cells: self.external_revision_cells.clone(),
            cma_remaining_root: self.cma_remaining_root,
            cma_spent_root: self.cma_spent_root,
            unresolved_effects: self.unresolved_effects.clone(),
            term_facts: self.term_facts.clone(),
            owner_census: self.owner_census.clone(),
            disclosure: self.disclosure,
            protocol_version: self.protocol_version,
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessVisibleAuthorityContinuityStateV1 {
    state_token: StateTokenIdV1,
    predecessor_state_token: Option<StateTokenIdV1>,
    context_kind: AuthorityContextKindV1,
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    store_publication_clock: u64,
    authority_epoch: u64,
    manifest_id: AuthorityContinuityManifestIdV1,
    closure_id: AuthorityContinuityClosureIdV1,
    store_allocation_commitment: ContinuityReferenceV1,
    guard_kind: GuardAdmissionKindV1,
    selected_trusted_time_stack: ContinuityReferenceV1,
    carrier_profile: ContinuityReferenceV1,
    accepted_external_prefix: ContinuityReferenceV1,
    carrier_handoff_state: ContinuityReferenceV1,
    carrier_fence: ContinuityReferenceV1,
    carrier_currentness: ContinuityReferenceV1,
    accepted_time: AcceptedAuthorityTimeFloorV1,
    lane_state_closure_root: ContinuityReferenceV1,
    source_floor_root: ContinuityReferenceV1,
    gap_companions: Vec<ContinuityReferenceV1>,
    floor_provenance: Vec<ContinuityReferenceV1>,
    external_revision_cells: Vec<ContinuityReferenceV1>,
    cma_remaining_root: ContinuityReferenceV1,
    cma_spent_root: ContinuityReferenceV1,
    unresolved_effects: Vec<ContinuityReferenceV1>,
    cut_sequence: u64,
    guard_admission_digest: ContinuityReferenceV1,
}

impl SuccessVisibleAuthorityContinuityStateV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.success-visible-authority-continuity-state.v1";

    pub(crate) fn construct(
        manifest: &AuthorityContinuityManifestV1,
        closure: &AuthorityContinuityClosureV1,
        guard: &AdmittedTransitionGuardV1,
        prior_state: Option<&Self>,
    ) -> Result<Self, AuthorityContinuityStateError> {
        validate_state_basis(manifest, closure, guard, prior_state)?;
        let (profile, prefix, handoff, fence, currentness) =
            confirmed_carrier(&guard.carrier_profile)
                .ok_or(AuthorityContinuityStateError::CarrierProfileUnavailable)?;
        Ok(Self {
            state_token: closure.successor_state_token(),
            predecessor_state_token: closure.predecessor_state_token(),
            context_kind: closure.context_kind(),
            context_id: closure.context_id(),
            store_generation: closure.store_generation(),
            store_publication_clock: closure.store_publication_clock(),
            authority_epoch: closure.authority_epoch(),
            manifest_id: manifest.id(),
            closure_id: closure.id(),
            store_allocation_commitment: closure.store_allocation_commitment(),
            guard_kind: guard.kind,
            selected_trusted_time_stack: guard.selected_trusted_time_stack,
            carrier_profile: profile,
            accepted_external_prefix: prefix,
            carrier_handoff_state: handoff,
            carrier_fence: fence,
            carrier_currentness: currentness,
            accepted_time: guard.accepted_time.clone(),
            lane_state_closure_root: guard.lane_state_closure_root,
            source_floor_root: guard.source_floor_root,
            gap_companions: guard.gap_companions.clone(),
            floor_provenance: guard.floor_provenance.clone(),
            external_revision_cells: guard.external_revision_cells.clone(),
            cma_remaining_root: guard.cma_remaining_root,
            cma_spent_root: guard.cma_spent_root,
            unresolved_effects: guard.unresolved_effects.clone(),
            cut_sequence: guard.cut_sequence,
            guard_admission_digest: guard.digest,
        })
    }

    pub fn decode(
        bytes: &[u8],
        manifest: &AuthorityContinuityManifestV1,
    ) -> Result<Self, AuthorityContinuityStateError> {
        let value = deterministic_cbor::decode(bytes)?;
        let fields = exact_array(&value, 26)?;
        if exact_text(&fields[0])? != Self::SCHEMA_DOMAIN
            || exact_unsigned(&fields[1])? != u64::from(SUPPORTED_PROTOCOL_VERSION)
        {
            return Err(AuthorityContinuityStateError::UnsupportedVersion);
        }
        let state_token = StateTokenIdV1::from_digest(exact_digest(&fields[2])?);
        let predecessor_state_token =
            parse_optional_digest(&fields[3])?.map(StateTokenIdV1::from_digest);
        let context_kind = AuthorityContextKindV1::try_from(exact_u8(&fields[4])?)
            .map_err(|_| AuthorityContinuityStateError::DecodeMalformed)?;
        let context_id = AuthorityContextIdV1::from_digest(exact_digest(&fields[5])?);
        let store_generation = exact_unsigned(&fields[6])?;
        let store_publication_clock = exact_unsigned(&fields[7])?;
        let authority_epoch = exact_unsigned(&fields[8])?;
        let manifest_id = AuthorityContinuityManifestIdV1::from_digest(exact_digest(&fields[9])?);
        let closure_id = AuthorityContinuityClosureIdV1::from_digest(exact_digest(&fields[10])?);
        let store_allocation_commitment =
            ContinuityReferenceV1::from_digest(exact_digest(&fields[11])?);
        let guard_kind = parse_guard_kind(&fields[12])?;
        let carrier = exact_array(&fields[13], 5)?;
        let carrier_profile = ContinuityReferenceV1::from_digest(exact_digest(&carrier[0])?);
        let accepted_external_prefix =
            ContinuityReferenceV1::from_digest(exact_digest(&carrier[1])?);
        let carrier_handoff_state = ContinuityReferenceV1::from_digest(exact_digest(&carrier[2])?);
        let carrier_fence = ContinuityReferenceV1::from_digest(exact_digest(&carrier[3])?);
        let carrier_currentness = ContinuityReferenceV1::from_digest(exact_digest(&carrier[4])?);
        let selected_trusted_time_stack =
            ContinuityReferenceV1::from_digest(exact_digest(&fields[14])?);
        let accepted_time = parse_accepted_time(&fields[15])?;
        let lane_state_closure_root =
            ContinuityReferenceV1::from_digest(exact_digest(&fields[16])?);
        let source_floor_root = ContinuityReferenceV1::from_digest(exact_digest(&fields[17])?);
        let gap_companions = parse_references(&fields[18])?;
        let floor_provenance = parse_references(&fields[19])?;
        let external_revision_cells = parse_references(&fields[20])?;
        let cma_remaining_root = ContinuityReferenceV1::from_digest(exact_digest(&fields[21])?);
        let cma_spent_root = ContinuityReferenceV1::from_digest(exact_digest(&fields[22])?);
        let unresolved_effects = parse_references(&fields[23])?;
        let cut_sequence = exact_unsigned(&fields[24])?;
        let guard_admission_digest = ContinuityReferenceV1::from_digest(exact_digest(&fields[25])?);
        let record_families = [
            &gap_companions,
            &floor_provenance,
            &external_revision_cells,
            &unresolved_effects,
        ];
        if store_generation == 0
            || store_publication_clock == 0
            || authority_epoch == 0
            || cut_sequence == 0
            || manifest_id != manifest.id()
            || context_kind != manifest.context_kind()
            || selected_trusted_time_stack != accepted_time.policy_stack()
            || record_families.iter().any(|items| !valid_records(items))
            || predecessor_state_token == Some(state_token)
        {
            return Err(AuthorityContinuityStateError::DecodeMalformed);
        }
        let state = Self {
            state_token,
            predecessor_state_token,
            context_kind,
            context_id,
            store_generation,
            store_publication_clock,
            authority_epoch,
            manifest_id,
            closure_id,
            store_allocation_commitment,
            guard_kind,
            selected_trusted_time_stack,
            carrier_profile,
            accepted_external_prefix,
            carrier_handoff_state,
            carrier_fence,
            carrier_currentness,
            accepted_time,
            lane_state_closure_root,
            source_floor_root,
            gap_companions,
            floor_provenance,
            external_revision_cells,
            cma_remaining_root,
            cma_spent_root,
            unresolved_effects,
            cut_sequence,
            guard_admission_digest,
        };
        if state.canonical_bytes()? != bytes {
            return Err(AuthorityContinuityStateError::NonCanonicalState);
        }
        Ok(state)
    }

    pub const fn state_token(&self) -> StateTokenIdV1 {
        self.state_token
    }

    pub const fn predecessor_state_token(&self) -> Option<StateTokenIdV1> {
        self.predecessor_state_token
    }

    pub const fn context_kind(&self) -> AuthorityContextKindV1 {
        self.context_kind
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn store_generation(&self) -> u64 {
        self.store_generation
    }

    pub const fn store_publication_clock(&self) -> u64 {
        self.store_publication_clock
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub const fn manifest_id(&self) -> AuthorityContinuityManifestIdV1 {
        self.manifest_id
    }

    pub const fn closure_id(&self) -> AuthorityContinuityClosureIdV1 {
        self.closure_id
    }

    pub const fn store_allocation_commitment(&self) -> ContinuityReferenceV1 {
        self.store_allocation_commitment
    }

    pub const fn selected_trusted_time_stack(&self) -> ContinuityReferenceV1 {
        self.selected_trusted_time_stack
    }

    pub const fn carrier_profile(&self) -> ContinuityReferenceV1 {
        self.carrier_profile
    }

    pub const fn accepted_external_prefix(&self) -> ContinuityReferenceV1 {
        self.accepted_external_prefix
    }

    pub const fn carrier_handoff_state(&self) -> ContinuityReferenceV1 {
        self.carrier_handoff_state
    }

    pub const fn carrier_fence(&self) -> ContinuityReferenceV1 {
        self.carrier_fence
    }

    pub const fn carrier_currentness(&self) -> ContinuityReferenceV1 {
        self.carrier_currentness
    }

    pub fn accepted_time(&self) -> &AcceptedAuthorityTimeFloorV1 {
        &self.accepted_time
    }

    pub const fn lane_state_closure_root(&self) -> ContinuityReferenceV1 {
        self.lane_state_closure_root
    }

    pub const fn source_floor_root(&self) -> ContinuityReferenceV1 {
        self.source_floor_root
    }

    pub fn gap_companions(&self) -> &[ContinuityReferenceV1] {
        &self.gap_companions
    }

    pub fn floor_provenance(&self) -> &[ContinuityReferenceV1] {
        &self.floor_provenance
    }

    pub fn external_revision_cells(&self) -> &[ContinuityReferenceV1] {
        &self.external_revision_cells
    }

    pub const fn cma_remaining_root(&self) -> ContinuityReferenceV1 {
        self.cma_remaining_root
    }

    pub const fn cma_spent_root(&self) -> ContinuityReferenceV1 {
        self.cma_spent_root
    }

    pub fn unresolved_effects(&self) -> &[ContinuityReferenceV1] {
        &self.unresolved_effects
    }

    pub const fn cut_sequence(&self) -> u64 {
        self.cut_sequence
    }

    pub const fn guard_kind(&self) -> GuardAdmissionKindV1 {
        self.guard_kind
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Unsigned(u64::from(SUPPORTED_PROTOCOL_VERSION)),
            CborValue::Bytes(self.state_token.as_bytes().to_vec()),
            CborValue::optional(
                self.predecessor_state_token
                    .map(|token| CborValue::Bytes(token.as_bytes().to_vec())),
            ),
            CborValue::Unsigned(self.context_kind as u64),
            CborValue::Bytes(self.context_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.store_generation),
            CborValue::Unsigned(self.store_publication_clock),
            CborValue::Unsigned(self.authority_epoch),
            CborValue::Bytes(self.manifest_id.as_bytes().to_vec()),
            CborValue::Bytes(self.closure_id.as_bytes().to_vec()),
            CborValue::Bytes(self.store_allocation_commitment.as_bytes().to_vec()),
            guard_kind_value(self.guard_kind),
            CborValue::Array(vec![
                CborValue::Bytes(self.carrier_profile.as_bytes().to_vec()),
                CborValue::Bytes(self.accepted_external_prefix.as_bytes().to_vec()),
                CborValue::Bytes(self.carrier_handoff_state.as_bytes().to_vec()),
                CborValue::Bytes(self.carrier_fence.as_bytes().to_vec()),
                CborValue::Bytes(self.carrier_currentness.as_bytes().to_vec()),
            ]),
            CborValue::Bytes(self.selected_trusted_time_stack.as_bytes().to_vec()),
            accepted_time_value(&self.accepted_time),
            CborValue::Bytes(self.lane_state_closure_root.as_bytes().to_vec()),
            CborValue::Bytes(self.source_floor_root.as_bytes().to_vec()),
            reference_array(&self.gap_companions),
            reference_array(&self.floor_provenance),
            reference_array(&self.external_revision_cells),
            CborValue::Bytes(self.cma_remaining_root.as_bytes().to_vec()),
            CborValue::Bytes(self.cma_spent_root.as_bytes().to_vec()),
            reference_array(&self.unresolved_effects),
            CborValue::Unsigned(self.cut_sequence),
            CborValue::Bytes(self.guard_admission_digest.as_bytes().to_vec()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityContinuityStateError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("continuity state or guard protocol version is unsupported")]
    UnsupportedVersion,
    #[error(
        "continuity state requires nonzero Store Generation, publication clock, Authority Epoch, and cut"
    )]
    InvalidGenerationEpoch,
    #[error(
        "the transition guard term census is missing, extra, duplicate, stale, or self-asserted"
    )]
    GuardTermTotalityMismatch,
    #[error("a transition guard owner fact or revision is zero, duplicate, or substituted")]
    InvalidGuardOwnerFact,
    #[error("protected continuity material was nondisclosed")]
    ProtectedMaterialUnavailable,
    #[error("the exact carrier profile, prefix, handoff, fence, and currentness are unavailable")]
    CarrierProfileUnavailable,
    #[error("the transition kind is unavailable for this context or branch")]
    UnsupportedTransition,
    #[error("manifest, context, or guard basis does not match the proven closure")]
    BasisMismatch,
    #[error("the coherent semantic cut changed after guard evaluation")]
    StaleSemanticCut,
    #[error("the exact prior ClosureId and StateToken do not match current state")]
    PredecessorMismatch,
    #[error("Store Generation or Authority Epoch changed outside an admitted owner transition")]
    StaleGenerationEpoch,
    #[error("the selected carrier profile changed outside its admitted owner transition")]
    CarrierProfileMismatch,
    #[error("the selected trusted-time Stack changed outside its admitted owner transition")]
    TrustedTimeStackMismatch,
    #[error("accepted trusted time rolled back or changed basis without a carry proof")]
    AcceptedTimeRollback,
    #[error("an unresolved Effect Intent, Gap companion, or provenance record was lost")]
    HistoricalContinuityLost,
    #[error("continuity state bytes are malformed")]
    DecodeMalformed,
    #[error("continuity state bytes are not the exact canonical encoding")]
    NonCanonicalState,
}

fn validate_guard_admission(
    input: &AuthorityTransitionGuardAdmissionInputV1,
) -> Result<(), AuthorityContinuityStateError> {
    if input.protocol_version != SUPPORTED_PROTOCOL_VERSION {
        return Err(AuthorityContinuityStateError::UnsupportedVersion);
    }
    if input.store_generation == 0 || input.authority_epoch == 0 || input.cut_sequence == 0 {
        return Err(AuthorityContinuityStateError::InvalidGenerationEpoch);
    }
    if input.disclosure != ContinuityDisclosureV1::ProtectedComplete {
        return Err(AuthorityContinuityStateError::ProtectedMaterialUnavailable);
    }
    if confirmed_carrier(&input.carrier_profile).is_none() {
        return Err(AuthorityContinuityStateError::CarrierProfileUnavailable);
    }
    if input.accepted_time.policy_stack() != input.selected_trusted_time_stack
        || record_families(input)
            .iter()
            .any(|items| !valid_records(items))
    {
        return Err(AuthorityContinuityStateError::BasisMismatch);
    }
    if input.owner_census.kind != input.kind
        || input.owner_census.context_id != input.context_id
        || input.owner_census.store_generation != input.store_generation
        || input.owner_census.authority_epoch != input.authority_epoch
        || input.owner_census.term_facts != input.term_facts
    {
        return Err(AuthorityContinuityStateError::GuardTermTotalityMismatch);
    }
    match input.kind {
        GuardAdmissionKindV1::ExternallyRootedContextGenesis => {
            if input.predecessor_state_token.is_some()
                || !valid_term_facts(input.kind, &input.term_facts)
            {
                return Err(AuthorityContinuityStateError::PredecessorMismatch);
            }
        }
        GuardAdmissionKindV1::Established(kind) => {
            if input.predecessor_state_token.is_none() {
                return Err(AuthorityContinuityStateError::PredecessorMismatch);
            }
            if !transition_matches_context(kind, input.context_kind) {
                return Err(AuthorityContinuityStateError::UnsupportedTransition);
            }
            if !valid_term_facts(input.kind, &input.term_facts) {
                return Err(AuthorityContinuityStateError::GuardTermTotalityMismatch);
            }
        }
    }
    Ok(())
}

fn validate_state_basis(
    manifest: &AuthorityContinuityManifestV1,
    closure: &AuthorityContinuityClosureV1,
    guard: &AdmittedTransitionGuardV1,
    prior_state: Option<&SuccessVisibleAuthorityContinuityStateV1>,
) -> Result<(), AuthorityContinuityStateError> {
    if manifest.id() != closure.manifest_id()
        || manifest.id() != guard.manifest_id
        || manifest.context_kind() != closure.context_kind()
        || manifest.context_kind() != guard.context_kind
        || closure.context_id() != guard.context_id
    {
        return Err(AuthorityContinuityStateError::BasisMismatch);
    }
    if closure.id() != guard.closure_id
        || closure.cut_sequence() != guard.cut_sequence
        || closure.selected_trusted_time_stack() != guard.selected_trusted_time_stack
        || closure.carrier_profile() != &guard.carrier_profile
        || closure.accepted_time() != &guard.accepted_time
        || closure.lane_state_closure_root() != guard.lane_state_closure_root
        || closure.source_floor_root() != guard.source_floor_root
        || closure.gap_companions() != guard.gap_companions
        || closure.floor_provenance() != guard.floor_provenance
        || closure.external_revision_cells() != guard.external_revision_cells
        || closure.cma_remaining_root() != guard.cma_remaining_root
        || closure.cma_spent_root() != guard.cma_spent_root
        || closure.unresolved_effects() != guard.unresolved_effects
    {
        return Err(AuthorityContinuityStateError::StaleSemanticCut);
    }
    if closure.store_generation() != guard.store_generation
        || closure.authority_epoch() != guard.authority_epoch
    {
        return Err(AuthorityContinuityStateError::StaleGenerationEpoch);
    }
    match (closure.predecessor(), guard.kind, prior_state) {
        (
            AuthorityContinuityPredecessorV1::ContextGenesis { .. },
            GuardAdmissionKindV1::ExternallyRootedContextGenesis,
            None,
        ) if guard.predecessor_state_token.is_none() => {}
        (
            AuthorityContinuityPredecessorV1::PriorClosure {
                closure_id,
                state_token,
            },
            GuardAdmissionKindV1::Established(kind),
            Some(prior),
        ) if closure_id == prior.closure_id
            && state_token == prior.state_token
            && guard.predecessor_state_token == Some(prior.state_token)
            && prior.context_kind == closure.context_kind()
            && prior.context_id == closure.context_id() =>
        {
            if prior.store_generation.checked_add(1) != Some(closure.store_generation())
                || prior.authority_epoch != closure.authority_epoch()
                || prior.store_publication_clock >= closure.store_publication_clock()
                || prior.cut_sequence >= closure.cut_sequence()
            {
                return Err(AuthorityContinuityStateError::StaleGenerationEpoch);
            }
            if !is_subset(&prior.unresolved_effects, closure.unresolved_effects())
                || !is_subset(&prior.gap_companions, closure.gap_companions())
                || !is_subset(&prior.floor_provenance, closure.floor_provenance())
            {
                return Err(AuthorityContinuityStateError::HistoricalContinuityLost);
            }
            let (profile, _, _, _, _) = confirmed_carrier(closure.carrier_profile())
                .ok_or(AuthorityContinuityStateError::CarrierProfileUnavailable)?;
            if profile != prior.carrier_profile
                && kind != TransitionGuardKindV1::ExternalLogicalCarrierProfileRotation
                && kind != TransitionGuardKindV1::PlannedEpochTurnoverPreparation
            {
                return Err(AuthorityContinuityStateError::CarrierProfileMismatch);
            }
            if closure.selected_trusted_time_stack() != prior.selected_trusted_time_stack
                && kind != TransitionGuardKindV1::TrustedTimePolicyStackRotation
                && kind != TransitionGuardKindV1::PlannedEpochTurnoverPreparation
            {
                return Err(AuthorityContinuityStateError::TrustedTimeStackMismatch);
            }
            if closure.accepted_time().lower_bound() < prior.accepted_time.lower_bound()
                || (closure.accepted_time().relation() == HTimeAcceptanceRelationV1::Same
                    && closure.accepted_time().lower_bound() != prior.accepted_time.lower_bound())
            {
                return Err(AuthorityContinuityStateError::AcceptedTimeRollback);
            }
        }
        _ => return Err(AuthorityContinuityStateError::PredecessorMismatch),
    }
    Ok(())
}

fn transition_matches_context(
    kind: TransitionGuardKindV1,
    context: AuthorityContextKindV1,
) -> bool {
    match kind {
        TransitionGuardKindV1::RepositoryWorkAuthorityPolicyTransition
        | TransitionGuardKindV1::RepositoryFirstWorkPublication
        | TransitionGuardKindV1::RepositoryFloorOrTrustRootRotation => {
            context == AuthorityContextKindV1::RepositoryAuthorityContext
        }
        TransitionGuardKindV1::InstallationPolicyBindingReplacement
        | TransitionGuardKindV1::InstallationStructuralRootFloorReplacement => {
            context == AuthorityContextKindV1::InstallationAuthorityContext
        }
        TransitionGuardKindV1::TrustedTimePolicyStackRotation
        | TransitionGuardKindV1::ExternalLogicalCarrierProfileRotation
        | TransitionGuardKindV1::PlannedEpochTurnoverPreparation => true,
    }
}

fn confirmed_carrier(
    carrier: &ContinuityCarrierProfileStatusV1,
) -> Option<(
    ContinuityReferenceV1,
    ContinuityReferenceV1,
    ContinuityReferenceV1,
    ContinuityReferenceV1,
    ContinuityReferenceV1,
)> {
    match carrier {
        ContinuityCarrierProfileStatusV1::Confirmed {
            profile,
            accepted_prefix,
            handoff_state,
            fence,
            currentness,
        } => Some((
            *profile,
            *accepted_prefix,
            *handoff_state,
            *fence,
            *currentness,
        )),
        ContinuityCarrierProfileStatusV1::Uncertain
        | ContinuityCarrierProfileStatusV1::Unavailable => None,
    }
}

fn guard_admission_value(
    input: &AuthorityTransitionGuardAdmissionInputV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.authority-transition-guard-evaluation.v1")?,
        CborValue::Unsigned(u64::from(input.protocol_version)),
        guard_kind_value(input.kind),
        CborValue::Unsigned(input.context_kind as u64),
        CborValue::Bytes(input.context_id.as_bytes().to_vec()),
        CborValue::Unsigned(input.store_generation),
        CborValue::Unsigned(input.authority_epoch),
        CborValue::Bytes(input.manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(input.closure_id.as_bytes().to_vec()),
        CborValue::optional(
            input
                .predecessor_state_token
                .map(|token| CborValue::Bytes(token.as_bytes().to_vec())),
        ),
        CborValue::Unsigned(input.cut_sequence),
        CborValue::Bytes(input.selected_trusted_time_stack.as_bytes().to_vec()),
        carrier_status_value(&input.carrier_profile),
        accepted_time_value(&input.accepted_time),
        CborValue::Bytes(input.lane_state_closure_root.as_bytes().to_vec()),
        CborValue::Bytes(input.source_floor_root.as_bytes().to_vec()),
        reference_array(&input.gap_companions),
        reference_array(&input.floor_provenance),
        reference_array(&input.external_revision_cells),
        CborValue::Bytes(input.cma_remaining_root.as_bytes().to_vec()),
        CborValue::Bytes(input.cma_spent_root.as_bytes().to_vec()),
        reference_array(&input.unresolved_effects),
        CborValue::Array(input.term_facts.iter().map(term_fact_value).collect()),
        CborValue::Bytes(input.owner_census.commitment.as_bytes().to_vec()),
        CborValue::Bytes(input.owner_census.source_cut_commitment.as_bytes().to_vec()),
        CborValue::Unsigned(1),
    ]))
}

fn carrier_status_value(carrier: &ContinuityCarrierProfileStatusV1) -> CborValue {
    match carrier {
        ContinuityCarrierProfileStatusV1::Confirmed {
            profile,
            accepted_prefix,
            handoff_state,
            fence,
            currentness,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(profile.as_bytes().to_vec()),
            CborValue::Bytes(accepted_prefix.as_bytes().to_vec()),
            CborValue::Bytes(handoff_state.as_bytes().to_vec()),
            CborValue::Bytes(fence.as_bytes().to_vec()),
            CborValue::Bytes(currentness.as_bytes().to_vec()),
        ]),
        ContinuityCarrierProfileStatusV1::Uncertain => {
            CborValue::Array(vec![CborValue::Unsigned(2)])
        }
        ContinuityCarrierProfileStatusV1::Unavailable => {
            CborValue::Array(vec![CborValue::Unsigned(3)])
        }
    }
}

fn term_fact_value(fact: &TransitionGuardTermFactV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(fact.term as u64),
        CborValue::Bytes(fact.owner_fact.as_bytes().to_vec()),
        CborValue::Bytes(fact.owner_revision.as_bytes().to_vec()),
    ])
}

fn owner_census_value(
    kind: GuardAdmissionKindV1,
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    authority_epoch: u64,
    source_cut_commitment: ContinuityReferenceV1,
    term_facts: &[TransitionGuardTermFactV1],
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.transition-guard-owner-census.v1")?,
        guard_kind_value(kind),
        CborValue::Bytes(context_id.as_bytes().to_vec()),
        CborValue::Unsigned(store_generation),
        CborValue::Unsigned(authority_epoch),
        CborValue::Bytes(source_cut_commitment.as_bytes().to_vec()),
        CborValue::Array(term_facts.iter().map(term_fact_value).collect()),
    ]))
}

fn valid_term_facts(kind: GuardAdmissionKindV1, facts: &[TransitionGuardTermFactV1]) -> bool {
    let mut expected = match kind {
        GuardAdmissionKindV1::ExternallyRootedContextGenesis => Vec::new(),
        GuardAdmissionKindV1::Established(kind) => kind.term_bundle().terms().to_vec(),
    };
    let mut actual = facts.iter().map(|fact| fact.term).collect::<Vec<_>>();
    actual.sort_by_key(|term| *term as u8);
    expected.sort_by_key(|term| *term as u8);
    if actual != expected {
        return false;
    }
    let owner_facts = facts
        .iter()
        .map(|fact| fact.owner_fact)
        .collect::<BTreeSet<_>>();
    let owner_revisions = facts
        .iter()
        .map(|fact| fact.owner_revision)
        .collect::<BTreeSet<_>>();
    owner_facts.len() == facts.len()
        && owner_revisions.len() == facts.len()
        && owner_facts.is_disjoint(&owner_revisions)
        && !owner_facts.iter().copied().any(is_zero_reference)
        && !owner_revisions.iter().copied().any(is_zero_reference)
}

fn is_zero_reference(reference: ContinuityReferenceV1) -> bool {
    reference.as_bytes() == &[0_u8; 32]
}

fn guard_kind_value(kind: GuardAdmissionKindV1) -> CborValue {
    CborValue::Unsigned(match kind {
        GuardAdmissionKindV1::ExternallyRootedContextGenesis => 0,
        GuardAdmissionKindV1::Established(kind) => kind as u64,
    })
}

fn parse_guard_kind(
    value: &CborValue,
) -> Result<GuardAdmissionKindV1, AuthorityContinuityStateError> {
    let tag = exact_u8(value)?;
    if tag == 0 {
        Ok(GuardAdmissionKindV1::ExternallyRootedContextGenesis)
    } else {
        Ok(GuardAdmissionKindV1::Established(
            TransitionGuardKindV1::try_from(tag)
                .map_err(|_| AuthorityContinuityStateError::DecodeMalformed)?,
        ))
    }
}

fn parse_accepted_time(
    value: &CborValue,
) -> Result<AcceptedAuthorityTimeFloorV1, AuthorityContinuityStateError> {
    let values = exact_array(value, 6)?;
    let relation = match exact_u8(&values[4])? {
        1 => HTimeAcceptanceRelationV1::ContextGenesis,
        2 => HTimeAcceptanceRelationV1::Same,
        3 => HTimeAcceptanceRelationV1::Advance,
        _ => return Err(AuthorityContinuityStateError::DecodeMalformed),
    };
    let carry = match exact_array_any(&values[5])? {
        [CborValue::Unsigned(1)] => HTimeCarryBasisV1::ExactNoLineageChange,
        [CborValue::Unsigned(2), mapping] => HTimeCarryBasisV1::CompleteCarryMapping {
            mapping: ContinuityReferenceV1::from_digest(exact_digest(mapping)?),
        },
        _ => return Err(AuthorityContinuityStateError::DecodeMalformed),
    };
    if relation == HTimeAcceptanceRelationV1::ContextGenesis
        && matches!(carry, HTimeCarryBasisV1::ExactNoLineageChange)
    {
        return Err(AuthorityContinuityStateError::DecodeMalformed);
    }
    Ok(AcceptedAuthorityTimeFloorV1::from_persisted_parts(
        ContinuityReferenceV1::from_digest(exact_digest(&values[0])?),
        ContinuityReferenceV1::from_digest(exact_digest(&values[1])?),
        ContinuityReferenceV1::from_digest(exact_digest(&values[2])?),
        exact_unsigned(&values[3])?,
        relation,
        carry,
    ))
}

fn normalize_record_families(input: &mut AuthorityTransitionGuardAdmissionInputV1) {
    input.gap_companions.sort_unstable();
    input.floor_provenance.sort_unstable();
    input.external_revision_cells.sort_unstable();
    input.unresolved_effects.sort_unstable();
    input.term_facts.sort_by_key(|fact| fact.term as u8);
}

fn record_families(
    input: &AuthorityTransitionGuardAdmissionInputV1,
) -> [&Vec<ContinuityReferenceV1>; 4] {
    [
        &input.gap_companions,
        &input.floor_provenance,
        &input.external_revision_cells,
        &input.unresolved_effects,
    ]
}

fn valid_records(values: &[ContinuityReferenceV1]) -> bool {
    values.len() <= MAX_STATE_RECORDS
        && values.iter().copied().collect::<BTreeSet<_>>().len() == values.len()
}

fn is_subset(left: &[ContinuityReferenceV1], right: &[ContinuityReferenceV1]) -> bool {
    let right = right.iter().copied().collect::<BTreeSet<_>>();
    left.iter().all(|item| right.contains(item))
}

fn parse_references(
    value: &CborValue,
) -> Result<Vec<ContinuityReferenceV1>, AuthorityContinuityStateError> {
    exact_array_any(value)?
        .iter()
        .map(|value| exact_digest(value).map(ContinuityReferenceV1::from_digest))
        .collect()
}

fn parse_optional_digest(
    value: &CborValue,
) -> Result<Option<[u8; 32]>, AuthorityContinuityStateError> {
    match exact_array_any(value)? {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), value] => Ok(Some(exact_digest(value)?)),
        _ => Err(AuthorityContinuityStateError::DecodeMalformed),
    }
}

fn exact_array(
    value: &CborValue,
    length: usize,
) -> Result<&[CborValue], AuthorityContinuityStateError> {
    let values = exact_array_any(value)?;
    if values.len() == length {
        Ok(values)
    } else {
        Err(AuthorityContinuityStateError::DecodeMalformed)
    }
}

fn exact_array_any(value: &CborValue) -> Result<&[CborValue], AuthorityContinuityStateError> {
    match value {
        CborValue::Array(values) => Ok(values),
        _ => Err(AuthorityContinuityStateError::DecodeMalformed),
    }
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], AuthorityContinuityStateError> {
    match value {
        CborValue::Bytes(bytes) => bytes
            .as_slice()
            .try_into()
            .map_err(|_| AuthorityContinuityStateError::DecodeMalformed),
        _ => Err(AuthorityContinuityStateError::DecodeMalformed),
    }
}

fn exact_unsigned(value: &CborValue) -> Result<u64, AuthorityContinuityStateError> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(AuthorityContinuityStateError::DecodeMalformed),
    }
}

fn exact_u8(value: &CborValue) -> Result<u8, AuthorityContinuityStateError> {
    exact_unsigned(value)?
        .try_into()
        .map_err(|_| AuthorityContinuityStateError::DecodeMalformed)
}

fn exact_text(value: &CborValue) -> Result<&str, AuthorityContinuityStateError> {
    match value {
        CborValue::Text(value) => Ok(value),
        _ => Err(AuthorityContinuityStateError::DecodeMalformed),
    }
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}
