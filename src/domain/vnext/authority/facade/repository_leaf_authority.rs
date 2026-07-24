use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::identity::{SchemaIdV1, StoreGenerationIdV1, StoreObjectIdV1};
use crate::domain::vnext::persistence::StoreObjectV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::{
    ActionAuthorityBasisKindV1, ActionRequestIdV1, BootstrapControlG0AuthorityBasisV1,
    CmaEffectWithdrawalSlotFamilyV1, CmaObservationPublicationPurposeV1,
    ContinuityMaintenanceAuthorityBasisV1, GrantIdV1, PrincipalBindingIdV1, PrincipalIdV1,
    RepositoryActionLeafV1, RepositoryDownstreamActionLeafV1, SessionIdV1, StateTokenIdV1,
};

const REPOSITORY_LEAF_AUTHORITY_CARRIER_DOMAIN_V1: &str =
    "maestro.vnext.repository-leaf-authority-carrier.v1";
const REPOSITORY_LEAF_AUTHORITY_CONSUMPTION_DOMAIN_V1: &str =
    "maestro.vnext.repository-leaf-authority-consumption.v1";
const REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1: u64 = 1;
const REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1: u64 = 1;
const MAX_DECISION_ID_BYTES_V1: usize = 512;
const MAX_DECISION_OPTIONS_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryAuthoritySelectionV1 {
    actor_binding_id: PrincipalBindingIdV1,
    actor_session_id: SessionIdV1,
    terminal_grant_id: GrantIdV1,
}

impl RepositoryAuthoritySelectionV1 {
    pub const fn new(
        actor_binding_id: PrincipalBindingIdV1,
        actor_session_id: SessionIdV1,
        terminal_grant_id: GrantIdV1,
    ) -> Self {
        Self {
            actor_binding_id,
            actor_session_id,
            terminal_grant_id,
        }
    }

    pub const fn actor_binding_id(self) -> PrincipalBindingIdV1 {
        self.actor_binding_id
    }

    pub const fn actor_session_id(self) -> SessionIdV1 {
        self.actor_session_id
    }

    pub const fn terminal_grant_id(self) -> GrantIdV1 {
        self.terminal_grant_id
    }

    pub const fn is_bearer_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryAuthenticatedHumanV1 {
    binding_id: PrincipalBindingIdV1,
    session_id: SessionIdV1,
    carrier_commitment: [u8; 32],
    identity: [u8; 32],
}

impl RepositoryAuthenticatedHumanV1 {
    pub fn new(
        binding_id: PrincipalBindingIdV1,
        session_id: SessionIdV1,
        authenticated_carrier: &[u8],
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if authenticated_carrier.is_empty() {
            return Err(RepositoryLeafAuthorityErrorV1::InvalidAuthenticatedCarrier);
        }
        let carrier_commitment = authenticated_human_carrier_commitment(authenticated_carrier)?;
        let identity = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-authenticated-human.v1")?,
            bytes(binding_id.as_bytes()),
            bytes(session_id.as_bytes()),
            bytes(&carrier_commitment),
        ]))?;
        Ok(Self {
            binding_id,
            session_id,
            carrier_commitment,
            identity,
        })
    }

    pub const fn binding_id(self) -> PrincipalBindingIdV1 {
        self.binding_id
    }

    pub const fn session_id(self) -> SessionIdV1 {
        self.session_id
    }

    pub const fn carrier_commitment(self) -> [u8; 32] {
        self.carrier_commitment
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn is_bearer_authority(self) -> bool {
        false
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.binding_id.as_bytes()),
            bytes(self.session_id.as_bytes()),
            bytes(&self.carrier_commitment),
            bytes(&self.identity),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryDecisionOptionMappingV1 {
    alternative_id: [u8; 32],
    presented_option_commitment: [u8; 32],
}

impl RepositoryDecisionOptionMappingV1 {
    pub fn new(
        alternative_id: [u8; 32],
        presented_option: &[u8],
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        require_nonzero(alternative_id)?;
        if presented_option.is_empty() {
            return Err(RepositoryLeafAuthorityErrorV1::InvalidDecisionOptionMapping);
        }
        let presented_option_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-presented-decision-option.v1")?,
            CborValue::Bytes(presented_option.to_vec()),
        ]))?;
        Ok(Self {
            alternative_id,
            presented_option_commitment,
        })
    }

    pub const fn alternative_id(&self) -> [u8; 32] {
        self.alternative_id
    }

    pub const fn presented_option_commitment(&self) -> [u8; 32] {
        self.presented_option_commitment
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(&self.alternative_id),
            bytes(&self.presented_option_commitment),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryDecisionPresentationV1 {
    decision_id_commitment: [u8; 32],
    decision_revision_id: [u8; 32],
    prompt_commitment: [u8; 32],
    option_mapping: Vec<RepositoryDecisionOptionMappingV1>,
    selected_alternative_id: [u8; 32],
    commitment: [u8; 32],
}

impl RepositoryDecisionPresentationV1 {
    pub fn new(
        decision_id: &str,
        decision_revision_id: [u8; 32],
        prompt: &[u8],
        option_mapping: Vec<RepositoryDecisionOptionMappingV1>,
        selected_alternative_id: [u8; 32],
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if decision_id.is_empty()
            || decision_id.len() > MAX_DECISION_ID_BYTES_V1
            || prompt.is_empty()
            || !(2..=MAX_DECISION_OPTIONS_V1).contains(&option_mapping.len())
        {
            return Err(RepositoryLeafAuthorityErrorV1::InvalidDecisionPresentation);
        }
        require_nonzero(decision_revision_id)?;
        require_nonzero(selected_alternative_id)?;
        let alternative_ids = option_mapping
            .iter()
            .map(RepositoryDecisionOptionMappingV1::alternative_id)
            .collect::<BTreeSet<_>>();
        let option_commitments = option_mapping
            .iter()
            .map(RepositoryDecisionOptionMappingV1::presented_option_commitment)
            .collect::<BTreeSet<_>>();
        if alternative_ids.len() != option_mapping.len()
            || option_commitments.len() != option_mapping.len()
        {
            return Err(RepositoryLeafAuthorityErrorV1::InvalidDecisionOptionMapping);
        }
        if !alternative_ids.contains(&selected_alternative_id) {
            return Err(RepositoryLeafAuthorityErrorV1::UnknownSelectedAlternative);
        }
        let decision_id_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-decision-identity.v1")?,
            CborValue::Text(decision_id.to_owned()),
        ]))?;
        let prompt_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-decision-prompt.v1")?,
            CborValue::Bytes(prompt.to_vec()),
        ]))?;
        let mut presentation = Self {
            decision_id_commitment,
            decision_revision_id,
            prompt_commitment,
            option_mapping,
            selected_alternative_id,
            commitment: [0; 32],
        };
        presentation.commitment = hash(&presentation.canonical_payload_value()?)?;
        Ok(presentation)
    }

    pub const fn decision_id_commitment(&self) -> [u8; 32] {
        self.decision_id_commitment
    }

    pub const fn decision_revision_id(&self) -> [u8; 32] {
        self.decision_revision_id
    }

    pub const fn prompt_commitment(&self) -> [u8; 32] {
        self.prompt_commitment
    }

    pub fn option_mapping(&self) -> &[RepositoryDecisionOptionMappingV1] {
        &self.option_mapping
    }

    pub const fn selected_alternative_id(&self) -> [u8; 32] {
        self.selected_alternative_id
    }

    pub const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RepositoryLeafAuthorityErrorV1> {
        Ok(deterministic_cbor::encode(
            &self.canonical_payload_value()?,
        )?)
    }

    fn canonical_payload_value(&self) -> Result<CborValue, RepositoryLeafAuthorityErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-decision-presentation.v1")?,
            CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1),
            bytes(&self.decision_id_commitment),
            bytes(&self.decision_revision_id),
            bytes(&self.prompt_commitment),
            CborValue::Array(
                self.option_mapping
                    .iter()
                    .map(RepositoryDecisionOptionMappingV1::canonical_value)
                    .collect(),
            ),
            bytes(&self.selected_alternative_id),
        ]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryPolicyStrengthV1 {
    minimum_human_approvals: u16,
    minimum_independent_reviews: u16,
    minimum_proof_claims: u16,
    requires_human_confirmation: bool,
    allows_unattended_publication: bool,
}

impl RepositoryPolicyStrengthV1 {
    pub const fn new(
        minimum_human_approvals: u16,
        minimum_independent_reviews: u16,
        minimum_proof_claims: u16,
        requires_human_confirmation: bool,
        allows_unattended_publication: bool,
    ) -> Self {
        Self {
            minimum_human_approvals,
            minimum_independent_reviews,
            minimum_proof_claims,
            requires_human_confirmation,
            allows_unattended_publication,
        }
    }

    pub const fn stage3_strict() -> Self {
        Self::new(1, 1, 1, true, false)
    }

    pub const fn weakens(self, current: Self) -> bool {
        self.minimum_human_approvals < current.minimum_human_approvals
            || self.minimum_independent_reviews < current.minimum_independent_reviews
            || self.minimum_proof_claims < current.minimum_proof_claims
            || (current.requires_human_confirmation && !self.requires_human_confirmation)
            || (!current.allows_unattended_publication && self.allows_unattended_publication)
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(u64::from(self.minimum_human_approvals)),
            CborValue::Unsigned(u64::from(self.minimum_independent_reviews)),
            CborValue::Unsigned(u64::from(self.minimum_proof_claims)),
            CborValue::Bool(self.requires_human_confirmation),
            CborValue::Bool(self.allows_unattended_publication),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPolicyComponentSetV1 {
    gate_snapshot: [u8; 32],
    policy_profile_provenance: [u8; 32],
    publication_authority_requirement: [u8; 32],
    completion_authority_requirement: [u8; 32],
    stage_proof_matrix: [u8; 32],
}

impl RepositoryPolicyComponentSetV1 {
    pub fn new(
        gate_snapshot: [u8; 32],
        policy_profile_provenance: [u8; 32],
        publication_authority_requirement: [u8; 32],
        completion_authority_requirement: [u8; 32],
        stage_proof_matrix: [u8; 32],
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        let ids = [
            gate_snapshot,
            policy_profile_provenance,
            publication_authority_requirement,
            completion_authority_requirement,
            stage_proof_matrix,
        ];
        if ids.contains(&[0; 32]) || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err(RepositoryLeafAuthorityErrorV1::InvalidPolicySnapshot);
        }
        Ok(Self {
            gate_snapshot,
            policy_profile_provenance,
            publication_authority_requirement,
            completion_authority_requirement,
            stage_proof_matrix,
        })
    }

    pub const fn gate_snapshot(&self) -> [u8; 32] {
        self.gate_snapshot
    }

    pub const fn policy_profile_provenance(&self) -> [u8; 32] {
        self.policy_profile_provenance
    }

    pub const fn publication_authority_requirement(&self) -> [u8; 32] {
        self.publication_authority_requirement
    }

    pub const fn completion_authority_requirement(&self) -> [u8; 32] {
        self.completion_authority_requirement
    }

    pub const fn stage_proof_matrix(&self) -> [u8; 32] {
        self.stage_proof_matrix
    }

    const fn ordered_ids(&self) -> [[u8; 32]; 5] {
        [
            self.gate_snapshot,
            self.policy_profile_provenance,
            self.publication_authority_requirement,
            self.completion_authority_requirement,
            self.stage_proof_matrix,
        ]
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(
            self.ordered_ids()
                .into_iter()
                .map(|id| bytes(&id))
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPolicySnapshotV1 {
    contract_root_id: [u8; 32],
    policy_components: RepositoryPolicyComponentSetV1,
    strength: RepositoryPolicyStrengthV1,
    commitment: [u8; 32],
}

impl RepositoryPolicySnapshotV1 {
    pub fn new(
        contract_root_id: [u8; 32],
        policy_components: RepositoryPolicyComponentSetV1,
        strength: RepositoryPolicyStrengthV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        require_nonzero(contract_root_id)?;
        let mut snapshot = Self {
            contract_root_id,
            policy_components,
            strength,
            commitment: [0; 32],
        };
        snapshot.commitment = hash(&snapshot.canonical_payload_value()?)?;
        Ok(snapshot)
    }

    pub const fn contract_root_id(&self) -> [u8; 32] {
        self.contract_root_id
    }

    pub const fn policy_components(&self) -> &RepositoryPolicyComponentSetV1 {
        &self.policy_components
    }

    pub const fn strength(&self) -> RepositoryPolicyStrengthV1 {
        self.strength
    }

    pub const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    fn canonical_payload_value(&self) -> Result<CborValue, RepositoryLeafAuthorityErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-contract-policy-snapshot.v1")?,
            CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1),
            bytes(&self.contract_root_id),
            self.policy_components.canonical_value(),
            self.strength.canonical_value(),
        ]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryPolicyTransitionKindV1 {
    Initial,
    Amendment,
}

impl RepositoryPolicyTransitionKindV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Initial => 1,
            Self::Amendment => 2,
        }
    }

    const fn action(self) -> RepositoryActionLeafV1 {
        match self {
            Self::Initial => RepositoryActionLeafV1::PublishInitialContract,
            Self::Amendment => RepositoryActionLeafV1::AmendContract,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPolicyTransitionV1 {
    kind: RepositoryPolicyTransitionKindV1,
    current: Option<RepositoryPolicySnapshotV1>,
    candidate: RepositoryPolicySnapshotV1,
    commitment: [u8; 32],
}

impl RepositoryPolicyTransitionV1 {
    pub fn initial(
        candidate: RepositoryPolicySnapshotV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        Self::construct(RepositoryPolicyTransitionKindV1::Initial, None, candidate)
    }

    pub fn amendment(
        current: RepositoryPolicySnapshotV1,
        candidate: RepositoryPolicySnapshotV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if current.commitment == candidate.commitment
            && (current.contract_root_id != candidate.contract_root_id
                || current.policy_components != candidate.policy_components
                || current.strength != candidate.strength)
        {
            return Err(RepositoryLeafAuthorityErrorV1::PolicyStrengthSubstitution);
        }
        Self::construct(
            RepositoryPolicyTransitionKindV1::Amendment,
            Some(current),
            candidate,
        )
    }

    fn construct(
        kind: RepositoryPolicyTransitionKindV1,
        current: Option<RepositoryPolicySnapshotV1>,
        candidate: RepositoryPolicySnapshotV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        let mut transition = Self {
            kind,
            current,
            candidate,
            commitment: [0; 32],
        };
        transition.commitment = hash(&transition.canonical_payload_value()?)?;
        Ok(transition)
    }

    pub const fn kind(&self) -> RepositoryPolicyTransitionKindV1 {
        self.kind
    }

    pub const fn current(&self) -> Option<&RepositoryPolicySnapshotV1> {
        self.current.as_ref()
    }

    pub const fn candidate(&self) -> &RepositoryPolicySnapshotV1 {
        &self.candidate
    }

    pub const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    pub fn is_weakening(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|current| self.candidate.strength.weakens(current.strength))
    }

    pub fn changes_policy(&self) -> bool {
        self.current.as_ref().is_none_or(|current| {
            current.policy_components != self.candidate.policy_components
                || current.strength != self.candidate.strength
                || current.contract_root_id != self.candidate.contract_root_id
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RepositoryLeafAuthorityErrorV1> {
        Ok(deterministic_cbor::encode(
            &self.canonical_payload_value()?,
        )?)
    }

    fn canonical_payload_value(&self) -> Result<CborValue, RepositoryLeafAuthorityErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-contract-policy-transition.v1")?,
            CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1),
            CborValue::Unsigned(self.kind.tag()),
            CborValue::optional(
                self.current
                    .as_ref()
                    .map(RepositoryPolicySnapshotV1::canonical_payload_value)
                    .transpose()?,
            ),
            self.candidate.canonical_payload_value()?,
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepositoryOneUseLeafCarrierV1 {
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    exact_payload: CborValue,
    authenticated_human: RepositoryAuthenticatedHumanV1,
    nonce: [u8; 32],
    expires_at: u64,
    identity: [u8; 32],
}

impl RepositoryOneUseLeafCarrierV1 {
    fn new(
        action: RepositoryActionLeafV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        exact_payload: CborValue,
        authenticated_human: RepositoryAuthenticatedHumanV1,
        nonce: [u8; 32],
        expires_at: u64,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        for commitment in [subject_commitment, subject_basis_commitment, nonce] {
            require_nonzero(commitment)?;
        }
        if expires_at == 0 {
            return Err(RepositoryLeafAuthorityErrorV1::InvalidExpiry);
        }
        let exact_payload_commitment = hash(&exact_payload)?;
        let mut carrier = Self {
            action,
            subject_commitment,
            subject_basis_commitment,
            exact_payload_commitment,
            exact_payload,
            authenticated_human,
            nonce,
            expires_at,
            identity: [0; 32],
        };
        carrier.identity = hash(&carrier.identity_value()?)?;
        Ok(carrier)
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, RepositoryLeafAuthorityErrorV1> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    fn identity_value(&self) -> Result<CborValue, RepositoryLeafAuthorityErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text(REPOSITORY_LEAF_AUTHORITY_CARRIER_DOMAIN_V1)?,
            CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1),
            CborValue::Unsigned(self.action.global_tag()),
            bytes(&self.subject_commitment),
            bytes(&self.subject_basis_commitment),
            bytes(&self.exact_payload_commitment),
            self.exact_payload.clone(),
            self.authenticated_human.canonical_value(),
            bytes(&self.nonce),
            CborValue::Unsigned(self.expires_at),
            CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1),
        ]))
    }

    fn canonical_value(&self) -> Result<CborValue, RepositoryLeafAuthorityErrorV1> {
        let CborValue::Array(mut fields) = self.identity_value()? else {
            unreachable!("Repository leaf carrier identity is an array")
        };
        fields.insert(2, bytes(&self.identity));
        Ok(CborValue::Array(fields))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryDecisionAuthorityCarrierV1(RepositoryOneUseLeafCarrierV1);

impl RepositoryDecisionAuthorityCarrierV1 {
    pub fn new(
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        presentation: &RepositoryDecisionPresentationV1,
        authenticated_human: RepositoryAuthenticatedHumanV1,
        nonce: [u8; 32],
        expires_at: u64,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        Ok(Self(RepositoryOneUseLeafCarrierV1::new(
            RepositoryActionLeafV1::ResolveDecision,
            subject_commitment,
            subject_basis_commitment,
            presentation.canonical_payload_value()?,
            authenticated_human,
            nonce,
            expires_at,
        )?))
    }

    pub const fn id(&self) -> [u8; 32] {
        self.0.identity
    }

    pub const fn nonce(&self) -> [u8; 32] {
        self.0.nonce
    }

    pub const fn expires_at(&self) -> u64 {
        self.0.expires_at
    }

    pub const fn capacity(&self) -> u64 {
        REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RepositoryLeafAuthorityErrorV1> {
        self.0.canonical_bytes()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPolicyTransitionAuthorityV1(RepositoryOneUseLeafCarrierV1);

impl RepositoryPolicyTransitionAuthorityV1 {
    pub fn new(
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        transition: &RepositoryPolicyTransitionV1,
        authenticated_human: RepositoryAuthenticatedHumanV1,
        nonce: [u8; 32],
        expires_at: u64,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        Ok(Self(RepositoryOneUseLeafCarrierV1::new(
            transition.kind.action(),
            subject_commitment,
            subject_basis_commitment,
            transition.canonical_payload_value()?,
            authenticated_human,
            nonce,
            expires_at,
        )?))
    }

    pub const fn id(&self) -> [u8; 32] {
        self.0.identity
    }

    pub const fn nonce(&self) -> [u8; 32] {
        self.0.nonce
    }

    pub const fn expires_at(&self) -> u64 {
        self.0.expires_at
    }

    pub const fn capacity(&self) -> u64 {
        REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RepositoryLeafAuthorityErrorV1> {
        self.0.canonical_bytes()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OrdinaryRepositoryLeafAuthorityV1 {
    selection: RepositoryAuthoritySelectionV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
}

impl OrdinaryRepositoryLeafAuthorityV1 {
    fn new(
        selection: RepositoryAuthoritySelectionV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        require_nonzero(subject_commitment)?;
        require_nonzero(subject_basis_commitment)?;
        Ok(Self {
            selection,
            subject_commitment,
            subject_basis_commitment,
        })
    }
}

macro_rules! ordinary_leaf_authority {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(OrdinaryRepositoryLeafAuthorityV1);

        impl $name {
            pub fn new(
                selection: RepositoryAuthoritySelectionV1,
                subject_commitment: [u8; 32],
                subject_basis_commitment: [u8; 32],
            ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
                Ok(Self(OrdinaryRepositoryLeafAuthorityV1::new(
                    selection,
                    subject_commitment,
                    subject_basis_commitment,
                )?))
            }

            pub const fn selection(&self) -> RepositoryAuthoritySelectionV1 {
                self.0.selection
            }

            pub const fn subject_commitment(&self) -> [u8; 32] {
                self.0.subject_commitment
            }

            pub const fn subject_basis_commitment(&self) -> [u8; 32] {
                self.0.subject_basis_commitment
            }
        }
    };
}

ordinary_leaf_authority!(CreateDraftWorkAuthorityV1);
ordinary_leaf_authority!(SubmitWorkCompletionAuthorityV1);
ordinary_leaf_authority!(CancelWorkAuthorityV1);
ordinary_leaf_authority!(AbsorbWorkAuthorityV1);
ordinary_leaf_authority!(AppendDesignRevisionAuthorityV1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitStepAuthorityV1 {
    selection: RepositoryAuthoritySelectionV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    executor_principal_id: PrincipalIdV1,
}

impl SubmitStepAuthorityV1 {
    pub fn new(
        selection: RepositoryAuthoritySelectionV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
        executor_principal_id: PrincipalIdV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        require_nonzero(subject_commitment)?;
        require_nonzero(subject_basis_commitment)?;
        require_nonzero(exact_payload_commitment)?;
        Ok(Self {
            selection,
            subject_commitment,
            subject_basis_commitment,
            exact_payload_commitment,
            executor_principal_id,
        })
    }

    pub const fn selection(&self) -> RepositoryAuthoritySelectionV1 {
        self.selection
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn exact_payload_commitment(&self) -> [u8; 32] {
        self.exact_payload_commitment
    }

    pub const fn executor_principal_id(&self) -> PrincipalIdV1 {
        self.executor_principal_id
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericExecutionAuthorityV1 {
    selection: RepositoryAuthoritySelectionV1,
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    executor_principal_id: PrincipalIdV1,
}

impl GenericExecutionAuthorityV1 {
    pub fn new(
        selection: RepositoryAuthoritySelectionV1,
        action: RepositoryActionLeafV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
        executor_principal_id: PrincipalIdV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if action.execution_authority_basis()
            != Some(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime)
        {
            return Err(RepositoryLeafAuthorityErrorV1::NonExecutionAction);
        }
        require_nonzero(subject_commitment)?;
        require_nonzero(subject_basis_commitment)?;
        require_nonzero(exact_payload_commitment)?;
        Ok(Self {
            selection,
            action,
            subject_commitment,
            subject_basis_commitment,
            exact_payload_commitment,
            executor_principal_id,
        })
    }

    pub const fn selection(&self) -> RepositoryAuthoritySelectionV1 {
        self.selection
    }

    pub const fn action(&self) -> RepositoryActionLeafV1 {
        self.action
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn current_state_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn exact_payload_commitment(&self) -> [u8; 32] {
        self.exact_payload_commitment
    }

    pub const fn executor_principal_id(&self) -> PrincipalIdV1 {
        self.executor_principal_id
    }

    pub const fn executor_principal_binding_id(&self) -> PrincipalBindingIdV1 {
        self.selection.actor_binding_id()
    }

    pub fn executor_session_id(&self) -> SessionIdV1 {
        self.selection.actor_session_id()
    }

    pub const fn executor_terminal_grant_id(&self) -> GrantIdV1 {
        self.selection.terminal_grant_id()
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapExecutionAuthorityV1 {
    basis: BootstrapControlG0AuthorityBasisV1,
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    executor_principal_id: PrincipalIdV1,
}

impl BootstrapExecutionAuthorityV1 {
    pub fn new(
        basis: BootstrapControlG0AuthorityBasisV1,
        action: RepositoryActionLeafV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
        executor_principal_id: PrincipalIdV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if action.execution_authority_basis()
            != Some(ActionAuthorityBasisKindV1::BootstrapControlG0)
        {
            return Err(RepositoryLeafAuthorityErrorV1::ExecutionAuthorityBasisMismatch);
        }
        require_nonzero(subject_commitment)?;
        require_nonzero(subject_basis_commitment)?;
        require_nonzero(exact_payload_commitment)?;
        Ok(Self {
            basis,
            action,
            subject_commitment,
            subject_basis_commitment,
            exact_payload_commitment,
            executor_principal_id,
        })
    }

    pub const fn basis(self) -> BootstrapControlG0AuthorityBasisV1 {
        self.basis
    }

    pub const fn action(self) -> RepositoryActionLeafV1 {
        self.action
    }

    pub const fn subject_commitment(self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn current_state_commitment(self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn exact_payload_commitment(self) -> [u8; 32] {
        self.exact_payload_commitment
    }

    pub const fn executor_principal_id(self) -> PrincipalIdV1 {
        self.executor_principal_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuityMaintenanceExecutionAuthorityV1 {
    basis: ContinuityMaintenanceAuthorityBasisV1,
    withdrawal_slot_family: Option<CmaEffectWithdrawalSlotFamilyV1>,
    purpose: CmaObservationPublicationPurposeV1,
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    executor_principal_id: PrincipalIdV1,
    continuity_state_token: StateTokenIdV1,
    continuity_state_object_id: StoreObjectIdV1,
    guard_object_id: StoreObjectIdV1,
    authority_epoch: u64,
    job_applicability_commitment: [u8; 32],
}

impl ContinuityMaintenanceExecutionAuthorityV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the owner-issued CMA permit binds the exact request and current applicability cut"
    )]
    pub(crate) fn new(
        basis: ContinuityMaintenanceAuthorityBasisV1,
        withdrawal_slot_family: Option<CmaEffectWithdrawalSlotFamilyV1>,
        purpose: CmaObservationPublicationPurposeV1,
        action: RepositoryActionLeafV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
        executor_principal_id: PrincipalIdV1,
        continuity_state_token: StateTokenIdV1,
        continuity_state_object_id: StoreObjectIdV1,
        guard_object_id: StoreObjectIdV1,
        authority_epoch: u64,
        job_applicability_commitment: [u8; 32],
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        let expected_withdrawal_slot_family = (action
            == RepositoryActionLeafV1::WithdrawContinuityMaintenanceEffect)
            .then_some(purpose.effect_withdrawal_slot_family());
        if action.execution_authority_basis()
            != Some(ActionAuthorityBasisKindV1::ContinuityMaintenance)
            || withdrawal_slot_family != expected_withdrawal_slot_family
        {
            return Err(RepositoryLeafAuthorityErrorV1::ExecutionAuthorityBasisMismatch);
        }
        require_nonzero(subject_commitment)?;
        require_nonzero(subject_basis_commitment)?;
        require_nonzero(exact_payload_commitment)?;
        require_nonzero(*continuity_state_token.as_bytes())?;
        require_nonzero(*continuity_state_object_id.as_bytes())?;
        require_nonzero(*guard_object_id.as_bytes())?;
        require_nonzero(job_applicability_commitment)?;
        if authority_epoch == 0 {
            return Err(RepositoryLeafAuthorityErrorV1::ExecutionAuthorityBasisMismatch);
        }
        Ok(Self {
            basis,
            withdrawal_slot_family,
            purpose,
            action,
            subject_commitment,
            subject_basis_commitment,
            exact_payload_commitment,
            executor_principal_id,
            continuity_state_token,
            continuity_state_object_id,
            guard_object_id,
            authority_epoch,
            job_applicability_commitment,
        })
    }

    pub const fn basis(self) -> ContinuityMaintenanceAuthorityBasisV1 {
        self.basis
    }

    pub const fn withdrawal_slot_family(self) -> Option<CmaEffectWithdrawalSlotFamilyV1> {
        self.withdrawal_slot_family
    }

    pub const fn purpose(self) -> CmaObservationPublicationPurposeV1 {
        self.purpose
    }

    pub const fn action(self) -> RepositoryActionLeafV1 {
        self.action
    }

    pub const fn subject_commitment(self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn current_state_commitment(self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn exact_payload_commitment(self) -> [u8; 32] {
        self.exact_payload_commitment
    }

    pub const fn executor_principal_id(self) -> PrincipalIdV1 {
        self.executor_principal_id
    }

    pub const fn continuity_state_token(self) -> StateTokenIdV1 {
        self.continuity_state_token
    }

    pub const fn continuity_state_object_id(self) -> StoreObjectIdV1 {
        self.continuity_state_object_id
    }

    pub const fn guard_object_id(self) -> StoreObjectIdV1 {
        self.guard_object_id
    }

    pub const fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub const fn job_applicability_commitment(self) -> [u8; 32] {
        self.job_applicability_commitment
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionAuthorityV1 {
    Ordinary(GenericExecutionAuthorityV1),
    BootstrapG0(BootstrapExecutionAuthorityV1),
    ContinuityMaintenance(ContinuityMaintenanceExecutionAuthorityV1),
}

impl From<GenericExecutionAuthorityV1> for ExecutionAuthorityV1 {
    fn from(value: GenericExecutionAuthorityV1) -> Self {
        Self::Ordinary(value)
    }
}

impl From<BootstrapExecutionAuthorityV1> for ExecutionAuthorityV1 {
    fn from(value: BootstrapExecutionAuthorityV1) -> Self {
        Self::BootstrapG0(value)
    }
}

impl From<ContinuityMaintenanceExecutionAuthorityV1> for ExecutionAuthorityV1 {
    fn from(value: ContinuityMaintenanceExecutionAuthorityV1) -> Self {
        Self::ContinuityMaintenance(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionProducerV1 {
    SessionBound {
        principal_id: PrincipalIdV1,
        session_id: SessionIdV1,
    },
    ContinuityMaintenance {
        principal_id: PrincipalIdV1,
        basis: ContinuityMaintenanceAuthorityBasisV1,
        purpose: CmaObservationPublicationPurposeV1,
        continuity_state_token: StateTokenIdV1,
        authority_epoch: u64,
    },
}

impl ExecutionProducerV1 {
    pub const fn principal_id(self) -> PrincipalIdV1 {
        match self {
            Self::SessionBound { principal_id, .. }
            | Self::ContinuityMaintenance { principal_id, .. } => principal_id,
        }
    }

    pub const fn session_id(self) -> Option<SessionIdV1> {
        match self {
            Self::SessionBound { session_id, .. } => Some(session_id),
            Self::ContinuityMaintenance { .. } => None,
        }
    }

    pub(crate) fn canonical_value(self) -> CborValue {
        match self {
            Self::SessionBound {
                principal_id,
                session_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                bytes(principal_id.as_bytes()),
                bytes(session_id.as_bytes()),
            ]),
            Self::ContinuityMaintenance {
                principal_id,
                basis,
                purpose,
                continuity_state_token,
                authority_epoch,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(principal_id.as_bytes()),
                bytes(basis.cma_branch_id.as_bytes()),
                bytes(basis.slot_id.as_bytes()),
                bytes(basis.executor_assertion_id.as_bytes()),
                CborValue::Unsigned(purpose as u64),
                bytes(continuity_state_token.as_bytes()),
                CborValue::Unsigned(authority_epoch),
            ]),
        }
    }
}

impl ExecutionAuthorityV1 {
    pub const fn action(&self) -> RepositoryActionLeafV1 {
        match self {
            Self::Ordinary(value) => value.action,
            Self::BootstrapG0(value) => value.action,
            Self::ContinuityMaintenance(value) => value.action,
        }
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        match self {
            Self::Ordinary(value) => value.subject_commitment,
            Self::BootstrapG0(value) => value.subject_commitment,
            Self::ContinuityMaintenance(value) => value.subject_commitment,
        }
    }

    pub const fn current_state_commitment(&self) -> [u8; 32] {
        match self {
            Self::Ordinary(value) => value.subject_basis_commitment,
            Self::BootstrapG0(value) => value.subject_basis_commitment,
            Self::ContinuityMaintenance(value) => value.subject_basis_commitment,
        }
    }

    pub const fn exact_payload_commitment(&self) -> [u8; 32] {
        match self {
            Self::Ordinary(value) => value.exact_payload_commitment,
            Self::BootstrapG0(value) => value.exact_payload_commitment,
            Self::ContinuityMaintenance(value) => value.exact_payload_commitment,
        }
    }

    pub const fn executor_principal_id(&self) -> PrincipalIdV1 {
        match self {
            Self::Ordinary(value) => value.executor_principal_id,
            Self::BootstrapG0(value) => value.executor_principal_id,
            Self::ContinuityMaintenance(value) => value.executor_principal_id,
        }
    }

    pub const fn producer(&self) -> ExecutionProducerV1 {
        match self {
            Self::Ordinary(value) => ExecutionProducerV1::SessionBound {
                principal_id: value.executor_principal_id,
                session_id: value.selection.actor_session_id(),
            },
            Self::BootstrapG0(value) => ExecutionProducerV1::SessionBound {
                principal_id: value.executor_principal_id,
                session_id: value.basis.session_id,
            },
            Self::ContinuityMaintenance(value) => ExecutionProducerV1::ContinuityMaintenance {
                principal_id: value.executor_principal_id,
                basis: value.basis,
                purpose: value.purpose,
                continuity_state_token: value.continuity_state_token,
                authority_epoch: value.authority_epoch,
            },
        }
    }

    pub const fn executor_session_id(&self) -> Option<SessionIdV1> {
        self.producer().session_id()
    }

    pub const fn basis_kind(&self) -> ActionAuthorityBasisKindV1 {
        match self {
            Self::Ordinary(_) => ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            Self::BootstrapG0(_) => ActionAuthorityBasisKindV1::BootstrapControlG0,
            Self::ContinuityMaintenance(_) => ActionAuthorityBasisKindV1::ContinuityMaintenance,
        }
    }

    pub const fn ordinary(&self) -> Option<&GenericExecutionAuthorityV1> {
        match self {
            Self::Ordinary(value) => Some(value),
            Self::BootstrapG0(_) | Self::ContinuityMaintenance(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepositoryOwnerFamilyAuthorityRecordV1 {
    selection: RepositoryAuthoritySelectionV1,
    action: RepositoryDownstreamActionLeafV1,
    subject_commitment: [u8; 32],
    current_semantic_owner_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
}

impl RepositoryOwnerFamilyAuthorityRecordV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the private record validates the exact nominal owner-family boundary"
    )]
    fn new(
        selection: RepositoryAuthoritySelectionV1,
        action: RepositoryDownstreamActionLeafV1,
        subject_commitment: [u8; 32],
        current_semantic_owner_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
        expected_owner_tag: u64,
        expected_family_tag: u64,
        first_global_tag: u64,
        last_global_tag: u64,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if action.owner_tag() != expected_owner_tag
            || action.family_tag() != expected_family_tag
            || !(first_global_tag..=last_global_tag).contains(&action.global_tag())
        {
            return Err(RepositoryLeafAuthorityErrorV1::DownstreamOwnerFamilyMismatch);
        }
        require_nonzero(subject_commitment)?;
        require_nonzero(current_semantic_owner_basis_commitment)?;
        require_nonzero(exact_payload_commitment)?;
        Ok(Self {
            selection,
            action,
            subject_commitment,
            current_semantic_owner_basis_commitment,
            exact_payload_commitment,
        })
    }

    const fn selection(self) -> RepositoryAuthoritySelectionV1 {
        self.selection
    }

    const fn action(self) -> RepositoryDownstreamActionLeafV1 {
        self.action
    }

    const fn subject_commitment(self) -> [u8; 32] {
        self.subject_commitment
    }

    const fn current_semantic_owner_basis_commitment(self) -> [u8; 32] {
        self.current_semantic_owner_basis_commitment
    }

    const fn exact_payload_commitment(self) -> [u8; 32] {
        self.exact_payload_commitment
    }
}

macro_rules! owner_family_authority {
    (
        $type:ident,
        owner = $owner_tag:literal,
        family = $family_tag:literal,
        global = $first_global_tag:literal..=$last_global_tag:literal,
        constructor = $constructor_cfg:meta,
        reason = $reason:literal
    ) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $type(RepositoryOwnerFamilyAuthorityRecordV1);

        impl $type {
            #[$constructor_cfg]
            pub(in crate::domain::vnext) fn new(
                selection: RepositoryAuthoritySelectionV1,
                action: RepositoryDownstreamActionLeafV1,
                subject_commitment: [u8; 32],
                current_semantic_owner_basis_commitment: [u8; 32],
                exact_payload_commitment: [u8; 32],
            ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
                Ok(Self(RepositoryOwnerFamilyAuthorityRecordV1::new(
                    selection,
                    action,
                    subject_commitment,
                    current_semantic_owner_basis_commitment,
                    exact_payload_commitment,
                    $owner_tag,
                    $family_tag,
                    $first_global_tag,
                    $last_global_tag,
                )?))
            }

            pub(in crate::domain::vnext::authority) const fn selection(
                self,
            ) -> RepositoryAuthoritySelectionV1 {
                self.0.selection()
            }

            pub(in crate::domain::vnext::authority) const fn action(
                self,
            ) -> RepositoryDownstreamActionLeafV1 {
                self.0.action()
            }

            pub(in crate::domain::vnext::authority) const fn subject_commitment(self) -> [u8; 32] {
                self.0.subject_commitment()
            }

            pub(in crate::domain::vnext::authority) const fn current_semantic_owner_basis_commitment(
                self,
            ) -> [u8; 32] {
                self.0.current_semantic_owner_basis_commitment()
            }

            pub(in crate::domain::vnext::authority) const fn exact_payload_commitment(
                self,
            ) -> [u8; 32] {
                self.0.exact_payload_commitment()
            }
        }
    };
}

owner_family_authority!(
    CoordinationRepositoryActionAuthorityV1,
    owner = 10,
    family = 9,
    global = 94..=102,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Coordination Authority input before its owner consumer"
);
owner_family_authority!(
    PlanningRepositoryActionAuthorityV1,
    owner = 12,
    family = 10,
    global = 103..=106,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Planning Authority input before its owner consumer"
);
owner_family_authority!(
    PersistenceRepositoryActionAuthorityV1,
    owner = 14,
    family = 11,
    global = 107..=116,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Persistence Authority input before its owner consumer"
);
owner_family_authority!(
    DistributionRepositoryActionAuthorityV1,
    owner = 20,
    family = 12,
    global = 117..=129,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Distribution Authority input before its owner consumer"
);
owner_family_authority!(
    SearchMaintenanceRepositoryActionAuthorityV1,
    owner = 15,
    family = 13,
    global = 130..=131,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the SearchMaintenance Authority input before its owner consumer"
);
owner_family_authority!(
    MemoryRepositoryActionAuthorityV1,
    owner = 16,
    family = 14,
    global = 132..=138,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Memory Authority input before its owner consumer"
);
owner_family_authority!(
    IntakeRepositoryActionAuthorityV1,
    owner = 17,
    family = 15,
    global = 139..=141,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Intake Authority input before its owner consumer"
);
owner_family_authority!(
    ResearchRepositoryActionAuthorityV1,
    owner = 18,
    family = 16,
    global = 142..=145,
    constructor = cfg(all()),
    reason = "Stage 7 freezes the Research Authority input before its owner consumer"
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveDecisionAuthorityV1 {
    selection: RepositoryAuthoritySelectionV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    presentation: RepositoryDecisionPresentationV1,
    carrier: RepositoryDecisionAuthorityCarrierV1,
}

impl ResolveDecisionAuthorityV1 {
    pub fn new(
        selection: RepositoryAuthoritySelectionV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        presentation: RepositoryDecisionPresentationV1,
        carrier: RepositoryDecisionAuthorityCarrierV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if carrier.0.action != RepositoryActionLeafV1::ResolveDecision
            || carrier.0.subject_commitment != subject_commitment
            || carrier.0.subject_basis_commitment != subject_basis_commitment
            || carrier.0.exact_payload_commitment != presentation.commitment
        {
            return Err(RepositoryLeafAuthorityErrorV1::DecisionCarrierMismatch);
        }
        Ok(Self {
            selection,
            subject_commitment,
            subject_basis_commitment,
            presentation,
            carrier,
        })
    }

    pub const fn presentation(&self) -> &RepositoryDecisionPresentationV1 {
        &self.presentation
    }

    pub const fn selection(&self) -> RepositoryAuthoritySelectionV1 {
        self.selection
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn carrier(&self) -> &RepositoryDecisionAuthorityCarrierV1 {
        &self.carrier
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishInitialContractAuthorityV1 {
    selection: RepositoryAuthoritySelectionV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    transition: RepositoryPolicyTransitionV1,
    transition_authority: RepositoryPolicyTransitionAuthorityV1,
}

impl PublishInitialContractAuthorityV1 {
    pub fn new(
        selection: RepositoryAuthoritySelectionV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        transition: RepositoryPolicyTransitionV1,
        transition_authority: RepositoryPolicyTransitionAuthorityV1,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if transition.kind != RepositoryPolicyTransitionKindV1::Initial
            || transition.current.is_some()
            || !carrier_matches_transition(
                &transition_authority.0,
                RepositoryActionLeafV1::PublishInitialContract,
                subject_commitment,
                subject_basis_commitment,
                &transition,
            )
        {
            return Err(RepositoryLeafAuthorityErrorV1::PolicyTransitionMismatch);
        }
        Ok(Self {
            selection,
            subject_commitment,
            subject_basis_commitment,
            transition,
            transition_authority,
        })
    }

    pub const fn transition(&self) -> &RepositoryPolicyTransitionV1 {
        &self.transition
    }

    pub const fn selection(&self) -> RepositoryAuthoritySelectionV1 {
        self.selection
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn transition_authority(&self) -> &RepositoryPolicyTransitionAuthorityV1 {
        &self.transition_authority
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmendContractAuthorityV1 {
    selection: RepositoryAuthoritySelectionV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    transition: RepositoryPolicyTransitionV1,
    transition_authority: RepositoryPolicyTransitionAuthorityV1,
}

impl AmendContractAuthorityV1 {
    pub fn new(
        selection: RepositoryAuthoritySelectionV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        transition: RepositoryPolicyTransitionV1,
        transition_authority: Option<RepositoryPolicyTransitionAuthorityV1>,
    ) -> Result<Self, RepositoryLeafAuthorityErrorV1> {
        if transition.kind != RepositoryPolicyTransitionKindV1::Amendment
            || transition.current.is_none()
        {
            return Err(RepositoryLeafAuthorityErrorV1::PolicyTransitionMismatch);
        }
        let Some(transition_authority) = transition_authority else {
            return Err(if transition.is_weakening() {
                RepositoryLeafAuthorityErrorV1::PolicyWeakeningRequiresExactTransitionAuthority
            } else {
                RepositoryLeafAuthorityErrorV1::PolicyTransitionAuthorityRequired
            });
        };
        if !carrier_matches_transition(
            &transition_authority.0,
            RepositoryActionLeafV1::AmendContract,
            subject_commitment,
            subject_basis_commitment,
            &transition,
        ) {
            return Err(RepositoryLeafAuthorityErrorV1::PolicyTransitionMismatch);
        }
        Ok(Self {
            selection,
            subject_commitment,
            subject_basis_commitment,
            transition,
            transition_authority,
        })
    }

    pub const fn transition(&self) -> &RepositoryPolicyTransitionV1 {
        &self.transition
    }

    pub const fn selection(&self) -> RepositoryAuthoritySelectionV1 {
        self.selection
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    pub const fn transition_authority(&self) -> &RepositoryPolicyTransitionAuthorityV1 {
        &self.transition_authority
    }
}

fn carrier_matches_transition(
    carrier: &RepositoryOneUseLeafCarrierV1,
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    transition: &RepositoryPolicyTransitionV1,
) -> bool {
    carrier.action == action
        && carrier.subject_commitment == subject_commitment
        && carrier.subject_basis_commitment == subject_basis_commitment
        && carrier.exact_payload_commitment == transition.commitment
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryLeafAuthorityInputV1 {
    CreateDraftWork(CreateDraftWorkAuthorityV1),
    SubmitWorkCompletion(SubmitWorkCompletionAuthorityV1),
    CancelWork(CancelWorkAuthorityV1),
    AbsorbWork(AbsorbWorkAuthorityV1),
    PublishInitialContract(PublishInitialContractAuthorityV1),
    AmendContract(AmendContractAuthorityV1),
    AppendDesignRevision(AppendDesignRevisionAuthorityV1),
    ResolveDecision(ResolveDecisionAuthorityV1),
    SubmitStep(SubmitStepAuthorityV1),
    Execution(ExecutionAuthorityV1),
    Coordination(CoordinationRepositoryActionAuthorityV1),
    Planning(PlanningRepositoryActionAuthorityV1),
    Persistence(PersistenceRepositoryActionAuthorityV1),
    Distribution(DistributionRepositoryActionAuthorityV1),
    SearchMaintenance(SearchMaintenanceRepositoryActionAuthorityV1),
    Memory(MemoryRepositoryActionAuthorityV1),
    Intake(IntakeRepositoryActionAuthorityV1),
    Research(ResearchRepositoryActionAuthorityV1),
}

macro_rules! leaf_input_from {
    ($type:ty, $variant:ident) => {
        impl From<$type> for RepositoryLeafAuthorityInputV1 {
            fn from(value: $type) -> Self {
                Self::$variant(value)
            }
        }
    };
}

leaf_input_from!(CreateDraftWorkAuthorityV1, CreateDraftWork);
leaf_input_from!(SubmitWorkCompletionAuthorityV1, SubmitWorkCompletion);
leaf_input_from!(CancelWorkAuthorityV1, CancelWork);
leaf_input_from!(AbsorbWorkAuthorityV1, AbsorbWork);
leaf_input_from!(PublishInitialContractAuthorityV1, PublishInitialContract);
leaf_input_from!(AmendContractAuthorityV1, AmendContract);
leaf_input_from!(AppendDesignRevisionAuthorityV1, AppendDesignRevision);
leaf_input_from!(ResolveDecisionAuthorityV1, ResolveDecision);
leaf_input_from!(SubmitStepAuthorityV1, SubmitStep);
leaf_input_from!(CoordinationRepositoryActionAuthorityV1, Coordination);
leaf_input_from!(PlanningRepositoryActionAuthorityV1, Planning);
leaf_input_from!(PersistenceRepositoryActionAuthorityV1, Persistence);
leaf_input_from!(DistributionRepositoryActionAuthorityV1, Distribution);
leaf_input_from!(
    SearchMaintenanceRepositoryActionAuthorityV1,
    SearchMaintenance
);
leaf_input_from!(MemoryRepositoryActionAuthorityV1, Memory);
leaf_input_from!(IntakeRepositoryActionAuthorityV1, Intake);
leaf_input_from!(ResearchRepositoryActionAuthorityV1, Research);
impl From<GenericExecutionAuthorityV1> for RepositoryLeafAuthorityInputV1 {
    fn from(value: GenericExecutionAuthorityV1) -> Self {
        Self::Execution(value.into())
    }
}

impl From<BootstrapExecutionAuthorityV1> for RepositoryLeafAuthorityInputV1 {
    fn from(value: BootstrapExecutionAuthorityV1) -> Self {
        Self::Execution(value.into())
    }
}

impl From<ContinuityMaintenanceExecutionAuthorityV1> for RepositoryLeafAuthorityInputV1 {
    fn from(value: ContinuityMaintenanceExecutionAuthorityV1) -> Self {
        Self::Execution(value.into())
    }
}

impl From<ExecutionAuthorityV1> for RepositoryLeafAuthorityInputV1 {
    fn from(value: ExecutionAuthorityV1) -> Self {
        Self::Execution(value)
    }
}

impl RepositoryLeafAuthorityInputV1 {
    pub(crate) const fn action(&self) -> RepositoryActionLeafV1 {
        match self {
            Self::CreateDraftWork(_) => RepositoryActionLeafV1::CreateDraftWork,
            Self::SubmitWorkCompletion(_) => RepositoryActionLeafV1::SubmitWorkCompletion,
            Self::CancelWork(_) => RepositoryActionLeafV1::CancelWork,
            Self::AbsorbWork(_) => RepositoryActionLeafV1::AbsorbWork,
            Self::PublishInitialContract(_) => RepositoryActionLeafV1::PublishInitialContract,
            Self::AmendContract(_) => RepositoryActionLeafV1::AmendContract,
            Self::AppendDesignRevision(_) => RepositoryActionLeafV1::AppendDesignRevision,
            Self::ResolveDecision(_) => RepositoryActionLeafV1::ResolveDecision,
            Self::SubmitStep(_) => RepositoryActionLeafV1::SubmitStep,
            Self::Execution(authority) => authority.action(),
            Self::Coordination(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
            Self::Planning(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
            Self::Persistence(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
            Self::Distribution(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
            Self::SearchMaintenance(authority) => {
                RepositoryActionLeafV1::Downstream(authority.action())
            }
            Self::Memory(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
            Self::Intake(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
            Self::Research(authority) => RepositoryActionLeafV1::Downstream(authority.action()),
        }
    }

    pub(crate) const fn selection(&self) -> Option<RepositoryAuthoritySelectionV1> {
        match self {
            Self::CreateDraftWork(authority) => Some(authority.0.selection),
            Self::SubmitWorkCompletion(authority) => Some(authority.0.selection),
            Self::CancelWork(authority) => Some(authority.0.selection),
            Self::AbsorbWork(authority) => Some(authority.0.selection),
            Self::PublishInitialContract(authority) => Some(authority.selection),
            Self::AmendContract(authority) => Some(authority.selection),
            Self::AppendDesignRevision(authority) => Some(authority.0.selection),
            Self::ResolveDecision(authority) => Some(authority.selection),
            Self::SubmitStep(authority) => Some(authority.selection),
            Self::Execution(ExecutionAuthorityV1::Ordinary(authority)) => Some(authority.selection),
            Self::Execution(
                ExecutionAuthorityV1::BootstrapG0(_)
                | ExecutionAuthorityV1::ContinuityMaintenance(_),
            ) => None,
            Self::Coordination(authority) => Some(authority.selection()),
            Self::Planning(authority) => Some(authority.selection()),
            Self::Persistence(authority) => Some(authority.selection()),
            Self::Distribution(authority) => Some(authority.selection()),
            Self::SearchMaintenance(authority) => Some(authority.selection()),
            Self::Memory(authority) => Some(authority.selection()),
            Self::Intake(authority) => Some(authority.selection()),
            Self::Research(authority) => Some(authority.selection()),
        }
    }

    pub(crate) const fn subject_commitment(&self) -> [u8; 32] {
        match self {
            Self::CreateDraftWork(authority) => authority.0.subject_commitment,
            Self::SubmitWorkCompletion(authority) => authority.0.subject_commitment,
            Self::CancelWork(authority) => authority.0.subject_commitment,
            Self::AbsorbWork(authority) => authority.0.subject_commitment,
            Self::PublishInitialContract(authority) => authority.subject_commitment,
            Self::AmendContract(authority) => authority.subject_commitment,
            Self::AppendDesignRevision(authority) => authority.0.subject_commitment,
            Self::ResolveDecision(authority) => authority.subject_commitment,
            Self::SubmitStep(authority) => authority.subject_commitment,
            Self::Execution(authority) => authority.subject_commitment(),
            Self::Coordination(authority) => authority.subject_commitment(),
            Self::Planning(authority) => authority.subject_commitment(),
            Self::Persistence(authority) => authority.subject_commitment(),
            Self::Distribution(authority) => authority.subject_commitment(),
            Self::SearchMaintenance(authority) => authority.subject_commitment(),
            Self::Memory(authority) => authority.subject_commitment(),
            Self::Intake(authority) => authority.subject_commitment(),
            Self::Research(authority) => authority.subject_commitment(),
        }
    }

    pub(crate) const fn subject_basis_commitment(&self) -> [u8; 32] {
        match self {
            Self::CreateDraftWork(authority) => authority.0.subject_basis_commitment,
            Self::SubmitWorkCompletion(authority) => authority.0.subject_basis_commitment,
            Self::CancelWork(authority) => authority.0.subject_basis_commitment,
            Self::AbsorbWork(authority) => authority.0.subject_basis_commitment,
            Self::PublishInitialContract(authority) => authority.subject_basis_commitment,
            Self::AmendContract(authority) => authority.subject_basis_commitment,
            Self::AppendDesignRevision(authority) => authority.0.subject_basis_commitment,
            Self::ResolveDecision(authority) => authority.subject_basis_commitment,
            Self::SubmitStep(authority) => authority.subject_basis_commitment,
            Self::Execution(authority) => authority.current_state_commitment(),
            Self::Coordination(authority) => authority.current_semantic_owner_basis_commitment(),
            Self::Planning(authority) => authority.current_semantic_owner_basis_commitment(),
            Self::Persistence(authority) => authority.current_semantic_owner_basis_commitment(),
            Self::Distribution(authority) => authority.current_semantic_owner_basis_commitment(),
            Self::SearchMaintenance(authority) => {
                authority.current_semantic_owner_basis_commitment()
            }
            Self::Memory(authority) => authority.current_semantic_owner_basis_commitment(),
            Self::Intake(authority) => authority.current_semantic_owner_basis_commitment(),
            Self::Research(authority) => authority.current_semantic_owner_basis_commitment(),
        }
    }

    pub(crate) const fn exact_payload_commitment(&self) -> Option<[u8; 32]> {
        match self {
            Self::Execution(authority) => Some(authority.exact_payload_commitment()),
            Self::SubmitStep(authority) => Some(authority.exact_payload_commitment),
            Self::Coordination(authority) => Some(authority.exact_payload_commitment()),
            Self::Planning(authority) => Some(authority.exact_payload_commitment()),
            Self::Persistence(authority) => Some(authority.exact_payload_commitment()),
            Self::Distribution(authority) => Some(authority.exact_payload_commitment()),
            Self::SearchMaintenance(authority) => Some(authority.exact_payload_commitment()),
            Self::Memory(authority) => Some(authority.exact_payload_commitment()),
            Self::Intake(authority) => Some(authority.exact_payload_commitment()),
            Self::Research(authority) => Some(authority.exact_payload_commitment()),
            Self::CreateDraftWork(_)
            | Self::SubmitWorkCompletion(_)
            | Self::CancelWork(_)
            | Self::AbsorbWork(_)
            | Self::PublishInitialContract(_)
            | Self::AmendContract(_)
            | Self::AppendDesignRevision(_)
            | Self::ResolveDecision(_) => None,
        }
    }

    pub(crate) const fn executor_principal_id(&self) -> Option<PrincipalIdV1> {
        match self {
            Self::Execution(authority) => Some(authority.executor_principal_id()),
            Self::SubmitStep(authority) => Some(authority.executor_principal_id),
            Self::Coordination(_)
            | Self::Planning(_)
            | Self::Persistence(_)
            | Self::Distribution(_)
            | Self::SearchMaintenance(_)
            | Self::Memory(_)
            | Self::Intake(_)
            | Self::Research(_) => None,
            Self::CreateDraftWork(_)
            | Self::SubmitWorkCompletion(_)
            | Self::CancelWork(_)
            | Self::AbsorbWork(_)
            | Self::PublishInitialContract(_)
            | Self::AmendContract(_)
            | Self::AppendDesignRevision(_)
            | Self::ResolveDecision(_) => None,
        }
    }

    fn specialized_carrier(&self) -> Option<&RepositoryOneUseLeafCarrierV1> {
        match self {
            Self::PublishInitialContract(authority) => Some(&authority.transition_authority.0),
            Self::AmendContract(authority) => Some(&authority.transition_authority.0),
            Self::ResolveDecision(authority) => Some(&authority.carrier.0),
            Self::Execution(_) => None,
            Self::SubmitStep(_) => None,
            Self::Coordination(_)
            | Self::Planning(_)
            | Self::Persistence(_)
            | Self::Distribution(_)
            | Self::SearchMaintenance(_)
            | Self::Memory(_)
            | Self::Intake(_)
            | Self::Research(_) => None,
            Self::CreateDraftWork(_)
            | Self::SubmitWorkCompletion(_)
            | Self::CancelWork(_)
            | Self::AbsorbWork(_)
            | Self::AppendDesignRevision(_) => None,
        }
    }

    pub(super) fn evaluate_specialized(
        &self,
        context: &RepositoryLeafAuthorityEvaluationContextV1,
    ) -> Result<
        Option<EvaluatedSpecializedRepositoryAuthorityV1>,
        RepositoryLeafAuthorityEvaluationErrorV1,
    > {
        let Some(carrier) = self.specialized_carrier() else {
            return Ok(None);
        };
        if carrier.action != self.action()
            || carrier.subject_commitment != self.subject_commitment()
            || carrier.subject_basis_commitment != self.subject_basis_commitment()
            || !context.human_capable
            || context.human_revoked
            || carrier.authenticated_human.binding_id != context.human_binding_id
            || carrier.authenticated_human.session_id != context.human_session_id
            || carrier.authenticated_human.carrier_commitment
                != context.authenticated_carrier_commitment
        {
            return Err(RepositoryLeafAuthorityEvaluationErrorV1::HumanAuthenticationMismatch);
        }
        let (Some(trusted_lower), Some(trusted_upper)) =
            (context.trusted_time_lower, context.trusted_time_upper)
        else {
            return Err(RepositoryLeafAuthorityEvaluationErrorV1::TrustedTimeUnavailable);
        };
        if trusted_lower > trusted_upper
            || trusted_upper >= carrier.expires_at
            || carrier.expires_at > context.human_valid_until
        {
            return Err(RepositoryLeafAuthorityEvaluationErrorV1::CarrierExpired);
        }
        if context
            .prior_consumptions
            .iter()
            .any(|consumption| consumption.carrier_id == carrier.identity)
        {
            return Err(RepositoryLeafAuthorityEvaluationErrorV1::CapacityExhausted);
        }
        if context
            .prior_consumptions
            .iter()
            .any(|consumption| consumption.nonce == carrier.nonce)
        {
            return Err(RepositoryLeafAuthorityEvaluationErrorV1::NonceReplay);
        }
        Ok(Some(EvaluatedSpecializedRepositoryAuthorityV1 {
            action: carrier.action,
            subject_commitment: carrier.subject_commitment,
            subject_basis_commitment: carrier.subject_basis_commitment,
            exact_payload_commitment: carrier.exact_payload_commitment,
            carrier_id: carrier.identity,
            nonce: carrier.nonce,
            carrier_value: carrier.canonical_value()?,
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositoryLeafAuthorityConsumptionV1 {
    carrier_id: [u8; 32],
    nonce: [u8; 32],
}

pub(super) struct RepositoryLeafAuthorityEvaluationContextV1 {
    pub(super) human_binding_id: PrincipalBindingIdV1,
    pub(super) human_session_id: SessionIdV1,
    pub(super) human_capable: bool,
    pub(super) human_revoked: bool,
    pub(super) authenticated_carrier_commitment: [u8; 32],
    pub(super) human_valid_until: u64,
    pub(super) trusted_time_lower: Option<u64>,
    pub(super) trusted_time_upper: Option<u64>,
    pub(super) prior_consumptions: Vec<RepositoryLeafAuthorityConsumptionV1>,
}

pub(super) struct EvaluatedSpecializedRepositoryAuthorityV1 {
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    carrier_id: [u8; 32],
    nonce: [u8; 32],
    carrier_value: CborValue,
}

impl EvaluatedSpecializedRepositoryAuthorityV1 {
    pub(super) const fn leaf_commitment(&self) -> [u8; 32] {
        self.carrier_id
    }

    pub(super) fn carrier_object(
        &self,
        mut references: Vec<StoreObjectIdV1>,
    ) -> Result<StoreObjectV1, RepositoryLeafAuthorityEvaluationErrorV1> {
        references.sort();
        references.dedup();
        Ok(StoreObjectV1::new(
            repository_leaf_authority_carrier_schema_id()?,
            self.carrier_value.clone(),
            references,
        )?)
    }

    pub(super) fn consumption_object(
        &self,
        request_id: ActionRequestIdV1,
        current_generation_id: StoreGenerationIdV1,
        basis_object_id: StoreObjectIdV1,
        carrier_object_id: StoreObjectIdV1,
    ) -> Result<StoreObjectV1, RepositoryLeafAuthorityEvaluationErrorV1> {
        let mut references = vec![carrier_object_id, basis_object_id];
        references.sort();
        references.dedup();
        Ok(StoreObjectV1::new(
            repository_leaf_authority_consumption_schema_id()?,
            CborValue::Array(vec![
                CborValue::text(REPOSITORY_LEAF_AUTHORITY_CONSUMPTION_DOMAIN_V1)?,
                CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1),
                bytes(&self.carrier_id),
                CborValue::Unsigned(self.action.global_tag()),
                bytes(&self.subject_commitment),
                bytes(&self.subject_basis_commitment),
                bytes(&self.exact_payload_commitment),
                bytes(&self.nonce),
                bytes(request_id.as_bytes()),
                bytes(current_generation_id.as_bytes()),
                bytes(basis_object_id.as_bytes()),
                CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1),
                CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1),
            ]),
            references,
        )?)
    }
}

pub(super) fn repository_leaf_authority_consumptions(
    active_objects: &[StoreObjectV1],
) -> Result<Vec<RepositoryLeafAuthorityConsumptionV1>, RepositoryLeafAuthorityEvaluationErrorV1> {
    let schema_id = repository_leaf_authority_consumption_schema_id()?;
    active_objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .map(|object| {
            let CborValue::Array(fields) = object.value() else {
                return Err(RepositoryLeafAuthorityEvaluationErrorV1::InvalidConsumptionRecord);
            };
            if fields.len() != 13
                || !matches!(&fields[0], CborValue::Text(domain) if domain == REPOSITORY_LEAF_AUTHORITY_CONSUMPTION_DOMAIN_V1)
                || fields[1]
                    != CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_PROTOCOL_VERSION_V1)
                || !matches!(
                    fields[3],
                    CborValue::Unsigned(1 | 2 | 4 | 12 | 13 | 15 | 20)
                )
                || fields[11]
                    != CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1)
                || fields[12]
                    != CborValue::Unsigned(REPOSITORY_LEAF_AUTHORITY_ONE_USE_CAPACITY_V1)
            {
                return Err(RepositoryLeafAuthorityEvaluationErrorV1::InvalidConsumptionRecord);
            }
            let carrier_id = exact_digest(&fields[2])?;
            for digest in &fields[4..=10] {
                exact_digest(digest)?;
            }
            Ok(RepositoryLeafAuthorityConsumptionV1 {
                carrier_id,
                nonce: exact_digest(&fields[7])?,
            })
        })
        .collect()
}

fn repository_leaf_authority_carrier_schema_id()
-> Result<SchemaIdV1, crate::domain::vnext::identity::IdentityError> {
    repository_leaf_authority_schema_id(REPOSITORY_LEAF_AUTHORITY_CARRIER_DOMAIN_V1)
}

fn repository_leaf_authority_consumption_schema_id()
-> Result<SchemaIdV1, crate::domain::vnext::identity::IdentityError> {
    repository_leaf_authority_schema_id(REPOSITORY_LEAF_AUTHORITY_CONSUMPTION_DOMAIN_V1)
}

fn repository_leaf_authority_schema_id(
    domain: &str,
) -> Result<SchemaIdV1, crate::domain::vnext::identity::IdentityError> {
    let value = CborValue::Array(vec![
        CborValue::Text("maestro.vnext.repository-runtime-schema.v1".to_owned()),
        CborValue::Text(domain.to_owned()),
    ]);
    let digest = Sha256::digest(
        deterministic_cbor::encode(&value)
            .expect("invariant: static Repository leaf schema identity encodes"),
    );
    let mut rendered = String::from("sha256:");
    for byte in digest {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    SchemaIdV1::parse(&rendered)
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], RepositoryLeafAuthorityEvaluationErrorV1> {
    let CborValue::Bytes(value) = value else {
        return Err(RepositoryLeafAuthorityEvaluationErrorV1::InvalidConsumptionRecord);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| RepositoryLeafAuthorityEvaluationErrorV1::InvalidConsumptionRecord)
}

fn require_nonzero(value: [u8; 32]) -> Result<(), RepositoryLeafAuthorityErrorV1> {
    if value == [0; 32] {
        Err(RepositoryLeafAuthorityErrorV1::ZeroCommitment)
    } else {
        Ok(())
    }
}

pub(super) fn authenticated_human_carrier_commitment(
    authenticated_carrier: &[u8],
) -> Result<[u8; 32], CborError> {
    hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-authenticated-human-carrier.v1")?,
        CborValue::Bytes(authenticated_carrier.to_vec()),
    ]))
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RepositoryLeafAuthorityErrorV1 {
    #[error("Repository leaf Authority commitments must be nonzero")]
    ZeroCommitment,
    #[error("ordinary Execution Authority requires one exact ordinary Execution action")]
    NonExecutionAction,
    #[error("Execution Authority basis does not match the exact frozen Action leaf")]
    ExecutionAuthorityBasisMismatch,
    #[error("the downstream Repository Action does not belong to the exact Authority owner family")]
    DownstreamOwnerFamilyMismatch,
    #[error("the authenticated human carrier must not be empty")]
    InvalidAuthenticatedCarrier,
    #[error("the Decision presentation is empty, ambiguous, or outside the finite v1 bounds")]
    InvalidDecisionPresentation,
    #[error("the Decision prompt-to-option mapping repeats an alternative or presented option")]
    InvalidDecisionOptionMapping,
    #[error("the selected Decision alternative is absent from the exact presented option mapping")]
    UnknownSelectedAlternative,
    #[error("the Decision Authority carrier does not bind the exact Decision presentation")]
    DecisionCarrierMismatch,
    #[error("the Repository policy snapshot is empty, duplicated, or outside the finite v1 bounds")]
    InvalidPolicySnapshot,
    #[error("the same exact Repository policy commitment was assigned conflicting strength facts")]
    PolicyStrengthSubstitution,
    #[error("the exact Repository policy transition requires typed transition Authority")]
    PolicyTransitionAuthorityRequired,
    #[error(
        "a Repository policy weakening requires Authority for that exact current/candidate transition"
    )]
    PolicyWeakeningRequiresExactTransitionAuthority,
    #[error(
        "the Repository policy transition Authority binds a different current/candidate transition"
    )]
    PolicyTransitionMismatch,
    #[error("the Repository leaf Authority expiry must be nonzero")]
    InvalidExpiry,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[derive(Debug, Error)]
pub(crate) enum RepositoryLeafAuthorityEvaluationErrorV1 {
    #[error("the typed Repository leaf Authority does not match the current authenticated human")]
    HumanAuthenticationMismatch,
    #[error("trusted Authority time is unavailable for the typed Repository leaf carrier")]
    TrustedTimeUnavailable,
    #[error("the typed Repository leaf Authority carrier has expired")]
    CarrierExpired,
    #[error("the typed Repository leaf Authority nonce has already been consumed")]
    NonceReplay,
    #[error("the typed Repository leaf Authority one-use capacity is exhausted")]
    CapacityExhausted,
    #[error("the Repository leaf Authority consumption record is malformed")]
    InvalidConsumptionRecord,
    #[error(transparent)]
    Identity(#[from] crate::domain::vnext::identity::IdentityError),
    #[error(transparent)]
    StoreObject(#[from] crate::domain::vnext::persistence::StoreObjectError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Leaf(#[from] RepositoryLeafAuthorityErrorV1),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> [u8; 32] {
        Sha256::digest(seed.as_bytes()).into()
    }

    fn rendered(value: [u8; 32]) -> String {
        let mut output = String::from("sha256:");
        for byte in value {
            use std::fmt::Write;
            write!(&mut output, "{byte:02x}").unwrap();
        }
        output
    }

    fn selection() -> RepositoryAuthoritySelectionV1 {
        RepositoryAuthoritySelectionV1::new(
            PrincipalBindingIdV1::derive("leaf-actor-binding").unwrap(),
            SessionIdV1::derive("leaf-actor-session").unwrap(),
            GrantIdV1::derive("leaf-terminal-grant").unwrap(),
        )
    }

    fn human(seed: &str) -> RepositoryAuthenticatedHumanV1 {
        RepositoryAuthenticatedHumanV1::new(
            PrincipalBindingIdV1::derive("leaf-human-binding").unwrap(),
            SessionIdV1::derive("leaf-human-session").unwrap(),
            seed.as_bytes(),
        )
        .unwrap()
    }

    fn presentation(
        revision: &str,
        prompt: &[u8],
        first_presented: &[u8],
        selected: [u8; 32],
    ) -> RepositoryDecisionPresentationV1 {
        RepositoryDecisionPresentationV1::new(
            "decision-exact",
            digest(revision),
            prompt,
            vec![
                RepositoryDecisionOptionMappingV1::new(digest("alternative-a"), first_presented)
                    .unwrap(),
                RepositoryDecisionOptionMappingV1::new(digest("alternative-b"), b"presented-b")
                    .unwrap(),
            ],
            selected,
        )
        .unwrap()
    }

    fn valid_decision_input(nonce: [u8; 32], expires_at: u64) -> RepositoryLeafAuthorityInputV1 {
        let subject = digest("decision-subject");
        let basis = digest("decision-basis");
        let presentation = presentation(
            "revision-a",
            b"exact prompt",
            b"presented-a",
            digest("alternative-a"),
        );
        let carrier = RepositoryDecisionAuthorityCarrierV1::new(
            subject,
            basis,
            &presentation,
            human("decision-carrier"),
            nonce,
            expires_at,
        )
        .unwrap();
        ResolveDecisionAuthorityV1::new(selection(), subject, basis, presentation, carrier)
            .unwrap()
            .into()
    }

    fn evaluation_context() -> RepositoryLeafAuthorityEvaluationContextV1 {
        RepositoryLeafAuthorityEvaluationContextV1 {
            human_binding_id: PrincipalBindingIdV1::derive("leaf-human-binding").unwrap(),
            human_session_id: SessionIdV1::derive("leaf-human-session").unwrap(),
            human_capable: true,
            human_revoked: false,
            authenticated_carrier_commitment: authenticated_human_carrier_commitment(
                b"decision-carrier",
            )
            .unwrap(),
            human_valid_until: 200,
            trusted_time_lower: Some(120),
            trusted_time_upper: Some(130),
            prior_consumptions: vec![],
        }
    }

    type OwnerFamilyInputConstructorV1 =
        fn(
            RepositoryAuthoritySelectionV1,
            RepositoryDownstreamActionLeafV1,
            [u8; 32],
            [u8; 32],
            [u8; 32],
        ) -> Result<RepositoryLeafAuthorityInputV1, RepositoryLeafAuthorityErrorV1>;

    #[derive(Clone, Copy)]
    struct OwnerFamilyCaseV1 {
        owner_tag: u64,
        family_tag: u64,
        first_global_tag: u64,
        last_global_tag: u64,
        constructor: OwnerFamilyInputConstructorV1,
    }

    macro_rules! owner_family_test_constructor {
        ($function:ident, $type:ident) => {
            fn $function(
                selection: RepositoryAuthoritySelectionV1,
                action: RepositoryDownstreamActionLeafV1,
                subject_commitment: [u8; 32],
                subject_basis_commitment: [u8; 32],
                exact_payload_commitment: [u8; 32],
            ) -> Result<RepositoryLeafAuthorityInputV1, RepositoryLeafAuthorityErrorV1> {
                $type::new(
                    selection,
                    action,
                    subject_commitment,
                    subject_basis_commitment,
                    exact_payload_commitment,
                )
                .map(Into::into)
            }
        };
    }

    owner_family_test_constructor!(coordination_input, CoordinationRepositoryActionAuthorityV1);
    owner_family_test_constructor!(planning_input, PlanningRepositoryActionAuthorityV1);
    owner_family_test_constructor!(persistence_input, PersistenceRepositoryActionAuthorityV1);
    owner_family_test_constructor!(distribution_input, DistributionRepositoryActionAuthorityV1);
    owner_family_test_constructor!(
        search_maintenance_input,
        SearchMaintenanceRepositoryActionAuthorityV1
    );
    owner_family_test_constructor!(memory_input, MemoryRepositoryActionAuthorityV1);
    owner_family_test_constructor!(intake_input, IntakeRepositoryActionAuthorityV1);
    owner_family_test_constructor!(research_input, ResearchRepositoryActionAuthorityV1);

    const OWNER_FAMILY_CASES_V1: [OwnerFamilyCaseV1; 8] = [
        OwnerFamilyCaseV1 {
            owner_tag: 10,
            family_tag: 9,
            first_global_tag: 94,
            last_global_tag: 102,
            constructor: coordination_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 12,
            family_tag: 10,
            first_global_tag: 103,
            last_global_tag: 106,
            constructor: planning_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 14,
            family_tag: 11,
            first_global_tag: 107,
            last_global_tag: 116,
            constructor: persistence_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 20,
            family_tag: 12,
            first_global_tag: 117,
            last_global_tag: 129,
            constructor: distribution_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 15,
            family_tag: 13,
            first_global_tag: 130,
            last_global_tag: 131,
            constructor: search_maintenance_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 16,
            family_tag: 14,
            first_global_tag: 132,
            last_global_tag: 138,
            constructor: memory_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 17,
            family_tag: 15,
            first_global_tag: 139,
            last_global_tag: 141,
            constructor: intake_input,
        },
        OwnerFamilyCaseV1 {
            owner_tag: 18,
            family_tag: 16,
            first_global_tag: 142,
            last_global_tag: 145,
            constructor: research_input,
        },
    ];

    #[test]
    fn nominal_owner_family_authorities_partition_all_fifty_two_downstream_leaves() {
        let expected_selection = selection();
        let subject = digest("downstream-subject");
        let owner_basis = digest("downstream-current-semantic-owner-basis");
        let payload = digest("downstream-exact-payload");
        let mut accepted = 0;

        for action in RepositoryDownstreamActionLeafV1::all() {
            let repository_action = RepositoryActionLeafV1::Downstream(action);
            assert!(matches!(
                repository_action.stage5_owner_dispatch(),
                super::super::super::RepositoryActionOwnerDispatchV1::OwnerUnavailable(
                    unavailable
                ) if unavailable == repository_action
            ));
            assert_eq!(repository_action.execution_authority_basis(), None);

            for owner_family in OWNER_FAMILY_CASES_V1 {
                let result = (owner_family.constructor)(
                    expected_selection,
                    action,
                    subject,
                    owner_basis,
                    payload,
                );
                if (owner_family.first_global_tag..=owner_family.last_global_tag)
                    .contains(&action.global_tag())
                {
                    let input = result.unwrap();
                    accepted += 1;
                    assert_eq!(action.owner_tag(), owner_family.owner_tag);
                    assert_eq!(action.family_tag(), owner_family.family_tag);
                    assert_eq!(input.action(), repository_action);
                    assert_eq!(input.selection(), Some(expected_selection));
                    assert_eq!(input.subject_commitment(), subject);
                    assert_eq!(input.subject_basis_commitment(), owner_basis);
                    assert_eq!(input.exact_payload_commitment(), Some(payload));
                    assert_eq!(input.executor_principal_id(), None);
                    assert!(
                        input
                            .evaluate_specialized(&evaluation_context())
                            .unwrap()
                            .is_none()
                    );
                } else {
                    assert_eq!(
                        result,
                        Err(RepositoryLeafAuthorityErrorV1::DownstreamOwnerFamilyMismatch)
                    );
                }
            }
        }

        assert_eq!(accepted, RepositoryDownstreamActionLeafV1::all().len());
    }

    #[test]
    fn every_nominal_owner_family_authority_rejects_each_zero_commitment_position() {
        let valid = (
            digest("downstream-subject"),
            digest("downstream-current-semantic-owner-basis"),
            digest("downstream-exact-payload"),
        );

        for owner_family in OWNER_FAMILY_CASES_V1 {
            let action =
                RepositoryDownstreamActionLeafV1::from_global_tag(owner_family.first_global_tag)
                    .unwrap();
            for commitments in [
                ([0; 32], valid.1, valid.2),
                (valid.0, [0; 32], valid.2),
                (valid.0, valid.1, [0; 32]),
            ] {
                assert_eq!(
                    (owner_family.constructor)(
                        selection(),
                        action,
                        commitments.0,
                        commitments.1,
                        commitments.2,
                    ),
                    Err(RepositoryLeafAuthorityErrorV1::ZeroCommitment)
                );
            }
        }
    }

    #[test]
    fn owner_family_authority_names_are_confined_to_authority_and_the_exact_semantic_owner() {
        const OWNER_ALLOWLIST: [(&str, &str); 8] = [
            (
                "CoordinationRepositoryActionAuthorityV1",
                "src/domain/vnext/coordination",
            ),
            (
                "PlanningRepositoryActionAuthorityV1",
                "src/domain/vnext/planning",
            ),
            (
                "PersistenceRepositoryActionAuthorityV1",
                "src/domain/vnext/persistence",
            ),
            (
                "DistributionRepositoryActionAuthorityV1",
                "src/domain/vnext/distribution",
            ),
            (
                "SearchMaintenanceRepositoryActionAuthorityV1",
                "src/domain/vnext/search",
            ),
            (
                "MemoryRepositoryActionAuthorityV1",
                "src/domain/vnext/memory",
            ),
            (
                "IntakeRepositoryActionAuthorityV1",
                "src/domain/vnext/intake",
            ),
            (
                "ResearchRepositoryActionAuthorityV1",
                "src/domain/vnext/research",
            ),
        ];

        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for required_forbidden_root in [
            "src/interfaces",
            "src/domain/vnext/integration",
            "src/domain/vnext/orchestration",
        ] {
            assert!(
                manifest.join(required_forbidden_root).is_dir(),
                "architecture guard did not scan required forbidden root {required_forbidden_root}"
            );
        }
        if manifest
            .join("tools/vnext_contracts/fanout/fanout-base.v1.json")
            .is_file()
        {
            for (_, owner_root) in OWNER_ALLOWLIST {
                assert!(
                    manifest.join(owner_root).is_dir(),
                    "combined fanout is missing exact semantic-owner root {owner_root}"
                );
            }
        }

        let mut sources = Vec::new();
        collect_rust_sources(&manifest.join("src"), &mut sources);
        let mut violations = Vec::new();
        for source_path in sources {
            let source = std::fs::read(&source_path).unwrap();
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_rust::LANGUAGE.into())
                .unwrap();
            let tree = parser.parse(&source, None).unwrap();
            assert!(
                !tree.root_node().has_error(),
                "architecture guard could not parse {}",
                source_path.display()
            );
            let mut identifiers = Vec::new();
            collect_rust_identifiers(tree.root_node(), &source, &mut identifiers);
            let relative = source_path
                .strip_prefix(manifest)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            for (authority_type, owner_root) in OWNER_ALLOWLIST {
                if identifiers
                    .iter()
                    .any(|identifier| identifier == authority_type)
                    && !path_is_within(&relative, "src/domain/vnext/authority")
                    && !path_is_within(&relative, owner_root)
                {
                    violations.push(format!("{authority_type} named from {relative}"));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "owner-family Authority wrappers escaped their Authority/semantic-owner allowlists: {violations:?}"
        );
    }

    fn collect_rust_sources(path: &std::path::Path, output: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                collect_rust_sources(&entry.path(), output);
            } else if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "rs")
            {
                output.push(entry.path());
            }
        }
    }

    fn collect_rust_identifiers(
        node: tree_sitter::Node<'_>,
        source: &[u8],
        output: &mut Vec<String>,
    ) {
        if matches!(node.kind(), "identifier" | "type_identifier") {
            output.push(node.utf8_text(source).unwrap().to_owned());
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_rust_identifiers(child, source, output);
        }
    }

    fn path_is_within(path: &str, root: &str) -> bool {
        path == format!("{root}.rs") || path.starts_with(&format!("{root}/"))
    }

    #[test]
    fn generic_execution_authority_binds_every_ordinary_basis_leaf_and_rejects_specialized_leaves()
    {
        let subject = digest("execution-subject");
        let current_state = digest("execution-current-state");
        let payload = digest("execution-exact-payload");
        let expected_selection = selection();
        let executor = PrincipalIdV1::derive("execution-principal").unwrap();
        let actions = RepositoryActionLeafV1::ALL
            .into_iter()
            .filter(|action| {
                action.execution_authority_basis()
                    == Some(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime)
            })
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 12);
        assert!(actions.contains(&RepositoryActionLeafV1::PublishObservation));

        for action in actions {
            let authority = GenericExecutionAuthorityV1::new(
                expected_selection,
                action,
                subject,
                current_state,
                payload,
                executor,
            )
            .unwrap();
            assert_eq!(authority.selection(), expected_selection);
            assert_eq!(authority.action(), action);
            assert_eq!(authority.subject_commitment(), subject);
            assert_eq!(authority.subject_basis_commitment(), current_state);
            assert_eq!(authority.current_state_commitment(), current_state);
            assert_eq!(authority.exact_payload_commitment(), payload);
            assert_eq!(authority.executor_principal_id(), executor);
            assert_eq!(
                authority.executor_principal_binding_id(),
                expected_selection.actor_binding_id()
            );
            assert_eq!(
                authority.executor_session_id(),
                expected_selection.actor_session_id()
            );
            assert_eq!(
                authority.executor_terminal_grant_id(),
                expected_selection.terminal_grant_id()
            );
            assert!(!authority.is_bearer_authority());

            let input = RepositoryLeafAuthorityInputV1::from(authority);
            assert_eq!(input.action(), action);
            assert_eq!(input.selection(), Some(expected_selection));
            assert_eq!(input.subject_commitment(), subject);
            assert_eq!(input.subject_basis_commitment(), current_state);
            assert_eq!(input.exact_payload_commitment(), Some(payload));
            assert_eq!(input.executor_principal_id(), Some(executor));
            assert!(
                input
                    .evaluate_specialized(&evaluation_context())
                    .unwrap()
                    .is_none()
            );
        }

        let specialized = RepositoryActionLeafV1::ALL
            .into_iter()
            .filter(|action| {
                matches!(
                    action.execution_authority_basis(),
                    Some(ActionAuthorityBasisKindV1::BootstrapControlG0)
                        | Some(ActionAuthorityBasisKindV1::ContinuityMaintenance)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(specialized.len(), 11);
        for action in specialized {
            assert_eq!(
                GenericExecutionAuthorityV1::new(
                    expected_selection,
                    action,
                    subject,
                    current_state,
                    payload,
                    executor,
                ),
                Err(RepositoryLeafAuthorityErrorV1::NonExecutionAction)
            );
        }
    }

    #[test]
    fn generic_execution_authority_rejects_non_execution_and_missing_commitments() {
        let valid = (
            digest("execution-subject"),
            digest("execution-current-state"),
            digest("execution-exact-payload"),
        );
        assert_eq!(
            GenericExecutionAuthorityV1::new(
                selection(),
                RepositoryActionLeafV1::CreateDraftWork,
                valid.0,
                valid.1,
                valid.2,
                PrincipalIdV1::derive("execution-principal").unwrap(),
            ),
            Err(RepositoryLeafAuthorityErrorV1::NonExecutionAction)
        );
        for commitments in [
            ([0; 32], valid.1, valid.2),
            (valid.0, [0; 32], valid.2),
            (valid.0, valid.1, [0; 32]),
        ] {
            assert_eq!(
                GenericExecutionAuthorityV1::new(
                    selection(),
                    RepositoryActionLeafV1::AcquireStepExecution,
                    commitments.0,
                    commitments.1,
                    commitments.2,
                    PrincipalIdV1::derive("execution-principal").unwrap(),
                ),
                Err(RepositoryLeafAuthorityErrorV1::ZeroCommitment)
            );
        }
    }

    #[test]
    fn inert_downstream_leaves_cannot_enter_the_stage_five_authority_carrier() {
        let before = evaluation_context().prior_consumptions.len();
        for downstream in RepositoryDownstreamActionLeafV1::all() {
            let action = RepositoryActionLeafV1::Downstream(downstream);
            assert!(matches!(
                action.stage5_owner_dispatch(),
                super::super::super::RepositoryActionOwnerDispatchV1::OwnerUnavailable(
                    unavailable
                ) if unavailable == action
            ));
            assert_eq!(action.execution_authority_basis(), None);
            assert_eq!(
                GenericExecutionAuthorityV1::new(
                    selection(),
                    action,
                    digest("inert-subject"),
                    digest("inert-current-state"),
                    digest("inert-payload"),
                    PrincipalIdV1::derive("inert-executor").unwrap(),
                ),
                Err(RepositoryLeafAuthorityErrorV1::NonExecutionAction)
            );
        }
        assert_eq!(evaluation_context().prior_consumptions.len(), before);
    }

    #[test]
    fn decision_carrier_rejects_wrong_option_revision_prompt_and_carrier() {
        let subject = digest("decision-subject");
        let basis = digest("decision-basis");
        let exact = presentation(
            "revision-a",
            b"exact prompt",
            b"presented-a",
            digest("alternative-a"),
        );
        let carrier = RepositoryDecisionAuthorityCarrierV1::new(
            subject,
            basis,
            &exact,
            human("decision-carrier"),
            digest("nonce"),
            140,
        )
        .unwrap();
        let hostile = [
            presentation(
                "revision-a",
                b"exact prompt",
                b"presented-a",
                digest("alternative-b"),
            ),
            presentation(
                "revision-b",
                b"exact prompt",
                b"presented-a",
                digest("alternative-a"),
            ),
            presentation(
                "revision-a",
                b"substituted prompt",
                b"presented-a",
                digest("alternative-a"),
            ),
            presentation(
                "revision-a",
                b"exact prompt",
                b"substituted presented option",
                digest("alternative-a"),
            ),
        ];
        for substituted in hostile {
            assert_eq!(
                ResolveDecisionAuthorityV1::new(
                    selection(),
                    subject,
                    basis,
                    substituted,
                    carrier.clone(),
                )
                .unwrap_err(),
                RepositoryLeafAuthorityErrorV1::DecisionCarrierMismatch
            );
        }
        let wrong_human = RepositoryAuthenticatedHumanV1::new(
            PrincipalBindingIdV1::derive("leaf-human-binding").unwrap(),
            SessionIdV1::derive("leaf-human-session").unwrap(),
            b"different-carrier",
        )
        .unwrap();
        let wrong_carrier = RepositoryDecisionAuthorityCarrierV1::new(
            subject,
            basis,
            &exact,
            wrong_human,
            digest("different-nonce"),
            140,
        )
        .unwrap();
        let input =
            ResolveDecisionAuthorityV1::new(selection(), subject, basis, exact, wrong_carrier)
                .unwrap();
        assert!(matches!(
            RepositoryLeafAuthorityInputV1::from(input).evaluate_specialized(&evaluation_context()),
            Err(RepositoryLeafAuthorityEvaluationErrorV1::HumanAuthenticationMismatch)
        ));
    }

    #[test]
    fn decision_carrier_refuses_expiry_nonce_replay_and_spent_capacity() {
        assert!(matches!(
            valid_decision_input(digest("expired-nonce"), 130)
                .evaluate_specialized(&evaluation_context()),
            Err(RepositoryLeafAuthorityEvaluationErrorV1::CarrierExpired)
        ));

        let nonce = digest("replayed-nonce");
        let first = valid_decision_input(nonce, 140)
            .evaluate_specialized(&evaluation_context())
            .unwrap()
            .unwrap();
        let mut spent = evaluation_context();
        spent
            .prior_consumptions
            .push(RepositoryLeafAuthorityConsumptionV1 {
                carrier_id: first.carrier_id,
                nonce: digest("other-nonce"),
            });
        assert!(matches!(
            valid_decision_input(nonce, 140).evaluate_specialized(&spent),
            Err(RepositoryLeafAuthorityEvaluationErrorV1::CapacityExhausted)
        ));

        let mut replayed = evaluation_context();
        replayed
            .prior_consumptions
            .push(RepositoryLeafAuthorityConsumptionV1 {
                carrier_id: digest("different-carrier-id"),
                nonce,
            });
        assert!(matches!(
            valid_decision_input(nonce, 140).evaluate_specialized(&replayed),
            Err(RepositoryLeafAuthorityEvaluationErrorV1::NonceReplay)
        ));
    }

    #[test]
    fn committed_consumption_object_exhausts_the_exact_carrier_atomically() {
        let nonce = digest("persisted-nonce");
        let input = valid_decision_input(nonce, 140);
        let evaluated = input
            .evaluate_specialized(&evaluation_context())
            .unwrap()
            .unwrap();
        let carrier = evaluated.carrier_object(vec![]).unwrap();
        let basis_id = StoreObjectIdV1::parse(&rendered(digest("admitted-basis"))).unwrap();
        let generation_id =
            StoreGenerationIdV1::parse(&rendered(digest("current-generation"))).unwrap();
        let consumption = evaluated
            .consumption_object(
                ActionRequestIdV1::derive("persisted-request").unwrap(),
                generation_id,
                basis_id,
                carrier.id(),
            )
            .unwrap();
        let mut next = evaluation_context();
        next.prior_consumptions = repository_leaf_authority_consumptions(&[consumption]).unwrap();
        assert!(matches!(
            input.evaluate_specialized(&next),
            Err(RepositoryLeafAuthorityEvaluationErrorV1::CapacityExhausted)
        ));
    }

    fn policy_snapshot(
        root: &str,
        component: &str,
        strength: RepositoryPolicyStrengthV1,
    ) -> RepositoryPolicySnapshotV1 {
        let components = RepositoryPolicyComponentSetV1::new(
            digest(&format!("{component}-gate")),
            digest(&format!("{component}-profile")),
            digest(&format!("{component}-publication")),
            digest(&format!("{component}-completion")),
            digest(&format!("{component}-proof")),
        )
        .unwrap();
        RepositoryPolicySnapshotV1::new(digest(root), components, strength).unwrap()
    }

    #[test]
    fn contract_policy_carrier_rejects_candidate_mismatch_and_unauthorized_downgrade() {
        let strict = RepositoryPolicyStrengthV1::stage3_strict();
        let weak = RepositoryPolicyStrengthV1::new(0, 0, 0, false, true);
        let current = policy_snapshot("current-root", "current-policy", strict);
        let downgraded = policy_snapshot("candidate-root", "candidate-policy", weak);
        let downgrade =
            RepositoryPolicyTransitionV1::amendment(current.clone(), downgraded).unwrap();
        assert_eq!(
            AmendContractAuthorityV1::new(
                selection(),
                digest("contract-subject"),
                digest("contract-basis"),
                downgrade.clone(),
                None,
            )
            .unwrap_err(),
            RepositoryLeafAuthorityErrorV1::PolicyWeakeningRequiresExactTransitionAuthority
        );

        let authorized_transition = RepositoryPolicyTransitionV1::amendment(
            current,
            policy_snapshot("candidate-root-a", "candidate-policy-a", strict),
        )
        .unwrap();
        let carrier = RepositoryPolicyTransitionAuthorityV1::new(
            digest("contract-subject"),
            digest("contract-basis"),
            &authorized_transition,
            human("policy-carrier"),
            digest("policy-nonce"),
            140,
        )
        .unwrap();
        assert_eq!(
            AmendContractAuthorityV1::new(
                selection(),
                digest("contract-subject"),
                digest("contract-basis"),
                downgrade,
                Some(carrier),
            )
            .unwrap_err(),
            RepositoryLeafAuthorityErrorV1::PolicyTransitionMismatch
        );
    }

    #[test]
    fn initial_contract_carrier_binds_the_exact_candidate_policy() {
        let subject = digest("initial-contract-subject");
        let basis = digest("initial-contract-basis");
        let strict = RepositoryPolicyStrengthV1::stage3_strict();
        let authorized = RepositoryPolicyTransitionV1::initial(policy_snapshot(
            "initial-root-a",
            "initial-policy-a",
            strict,
        ))
        .unwrap();
        let carrier = RepositoryPolicyTransitionAuthorityV1::new(
            subject,
            basis,
            &authorized,
            human("decision-carrier"),
            digest("initial-policy-nonce"),
            140,
        )
        .unwrap();
        let substituted = RepositoryPolicyTransitionV1::initial(policy_snapshot(
            "initial-root-b",
            "initial-policy-b",
            strict,
        ))
        .unwrap();
        assert_eq!(
            PublishInitialContractAuthorityV1::new(
                selection(),
                subject,
                basis,
                substituted,
                carrier,
            )
            .unwrap_err(),
            RepositoryLeafAuthorityErrorV1::PolicyTransitionMismatch
        );
    }

    #[test]
    fn carrier_identity_and_cbor_are_immutable_and_non_bearer() {
        let input = valid_decision_input(digest("stable-nonce"), 140);
        let RepositoryLeafAuthorityInputV1::ResolveDecision(authority) = input else {
            unreachable!("fixture is ResolveDecision Authority")
        };
        assert!(!authority.carrier().is_bearer_authority());
        assert_eq!(authority.carrier().capacity(), 1);
        assert_eq!(
            authority.carrier().canonical_bytes().unwrap(),
            authority.carrier().canonical_bytes().unwrap()
        );
        assert_ne!(authority.carrier().id(), [0; 32]);
    }
}
