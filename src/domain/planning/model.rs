use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const MAX_REF_BYTES_V1: usize = 2_048;
const MAX_OPPORTUNITIES_V1: usize = 65_536;
const MAX_PROPOSAL_UNITS_V1: usize = 4_096;
const MAX_COUNTERFACTUALS_V1: usize = 4_096;

macro_rules! planning_identity {
    ($name:ident, $domain:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub(crate) struct $name([u8; 32]);

        impl $name {
            pub(crate) fn derive(seed: &str) -> Result<Self, PlanningErrorV1> {
                require_ref(seed)?;
                Ok(Self(domain_hash($domain, &text(seed))?))
            }

            pub(crate) fn from_digest(digest: [u8; 32]) -> Result<Self, PlanningErrorV1> {
                require_digest(digest)?;
                Ok(Self(digest))
            }

            pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

planning_identity!(
    PlanningProposalIdV1,
    "maestro.vnext.planning-proposal-id.v1"
);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SchedulingOpportunityRefV1 {
    Action {
        action_ref: String,
        material_dependency_stamp: [u8; 32],
    },
    Wave {
        wave_ref: String,
        ordered_member_action_refs: Vec<String>,
        material_dependency_stamp: [u8; 32],
    },
}

impl SchedulingOpportunityRefV1 {
    pub(crate) fn validate(&self) -> Result<(), PlanningErrorV1> {
        match self {
            Self::Action {
                action_ref,
                material_dependency_stamp,
            } => {
                require_ref(action_ref)?;
                require_digest(*material_dependency_stamp)
            }
            Self::Wave {
                wave_ref,
                ordered_member_action_refs,
                material_dependency_stamp,
            } => {
                require_ref(wave_ref)?;
                require_digest(*material_dependency_stamp)?;
                if ordered_member_action_refs.is_empty()
                    || ordered_member_action_refs.len() > MAX_OPPORTUNITIES_V1
                    || !strictly_ordered_unique(ordered_member_action_refs)
                {
                    return Err(PlanningErrorV1::InvalidOpportunity);
                }
                ordered_member_action_refs
                    .iter()
                    .try_for_each(|value| require_ref(value))
            }
        }
    }

    pub(crate) fn stable_ref(&self) -> &str {
        match self {
            Self::Action { action_ref, .. } => action_ref,
            Self::Wave { wave_ref, .. } => wave_ref,
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::Action {
                action_ref,
                material_dependency_stamp,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                text(action_ref),
                bytes(material_dependency_stamp),
            ]),
            Self::Wave {
                wave_ref,
                ordered_member_action_refs,
                material_dependency_stamp,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                text(wave_ref),
                text_array(ordered_member_action_refs),
                bytes(material_dependency_stamp),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingOpportunitySetV1 {
    pub(crate) frontier_hash: [u8; 32],
    pub(crate) opportunities: Vec<SchedulingOpportunityRefV1>,
    pub(crate) participant_census_hash: [u8; 32],
    pub(crate) bounded_complete: bool,
    pub(crate) semantic_hash: [u8; 32],
}

impl SchedulingOpportunitySetV1 {
    pub(crate) fn new(
        frontier_hash: [u8; 32],
        opportunities: Vec<SchedulingOpportunityRefV1>,
        participant_census_hash: [u8; 32],
        bounded_complete: bool,
    ) -> Result<Self, PlanningErrorV1> {
        require_digest(frontier_hash)?;
        require_digest(participant_census_hash)?;
        if !bounded_complete
            || opportunities.is_empty()
            || opportunities.len() > MAX_OPPORTUNITIES_V1
            || opportunities
                .windows(2)
                .any(|pair| pair[0].stable_ref() >= pair[1].stable_ref())
        {
            return Err(PlanningErrorV1::IncompleteOpportunitySet);
        }
        opportunities
            .iter()
            .try_for_each(SchedulingOpportunityRefV1::validate)?;
        let value = CborValue::Array(vec![
            bytes(&frontier_hash),
            CborValue::Array(
                opportunities
                    .iter()
                    .map(SchedulingOpportunityRefV1::canonical_value)
                    .collect(),
            ),
            bytes(&participant_census_hash),
            CborValue::Bool(bounded_complete),
        ]);
        let semantic_hash = domain_hash("maestro.vnext.scheduling-opportunity-set.v1", &value)?;
        Ok(Self {
            frontier_hash,
            opportunities,
            participant_census_hash,
            bounded_complete,
            semantic_hash,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(&self.frontier_hash),
            CborValue::Array(
                self.opportunities
                    .iter()
                    .map(SchedulingOpportunityRefV1::canonical_value)
                    .collect(),
            ),
            bytes(&self.participant_census_hash),
            CborValue::Bool(self.bounded_complete),
            bytes(&self.semantic_hash),
        ])
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ProposalAdviceUnitV1 {
    pub(crate) semantic_claim_hash: [u8; 32],
    pub(crate) covered_opportunity_refs: Vec<String>,
    pub(crate) rationale_ref: String,
}

impl ProposalAdviceUnitV1 {
    pub(crate) fn validate(&self) -> Result<(), PlanningErrorV1> {
        require_digest(self.semantic_claim_hash)?;
        require_ref(&self.rationale_ref)?;
        if self.covered_opportunity_refs.is_empty()
            || self.covered_opportunity_refs.len() > MAX_OPPORTUNITIES_V1
            || !strictly_ordered_unique(&self.covered_opportunity_refs)
        {
            return Err(PlanningErrorV1::InvalidProposal);
        }
        self.covered_opportunity_refs
            .iter()
            .try_for_each(|value| require_ref(value))
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(&self.semantic_claim_hash),
            text_array(&self.covered_opportunity_refs),
            text(&self.rationale_ref),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlanningProposalV1 {
    pub(crate) proposal_id: PlanningProposalIdV1,
    pub(crate) repository_installation_ref: String,
    pub(crate) store_generation_ref: String,
    pub(crate) frontier_hash: [u8; 32],
    pub(crate) work_and_contract_root_refs: Vec<String>,
    pub(crate) opportunity_set_hash: [u8; 32],
    pub(crate) advice_units: Vec<ProposalAdviceUnitV1>,
    pub(crate) assumptions: Vec<String>,
    pub(crate) observation_refs: Vec<String>,
    pub(crate) producer_provenance_ref: String,
    pub(crate) acquisition_provenance_ref: String,
    pub(crate) privacy_basis_ref: String,
    pub(crate) redaction_provenance_ref: String,
    pub(crate) issued_at: u64,
    pub(crate) valid_until: u64,
    pub(crate) semantic_hash: [u8; 32],
}

impl PlanningProposalV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "PlanningProposalV1 closes all advisory provenance and applicability inputs"
    )]
    pub(crate) fn new(
        proposal_id: PlanningProposalIdV1,
        repository_installation_ref: String,
        store_generation_ref: String,
        frontier_hash: [u8; 32],
        work_and_contract_root_refs: Vec<String>,
        opportunity_set_hash: [u8; 32],
        advice_units: Vec<ProposalAdviceUnitV1>,
        assumptions: Vec<String>,
        observation_refs: Vec<String>,
        producer_provenance_ref: String,
        acquisition_provenance_ref: String,
        privacy_basis_ref: String,
        redaction_provenance_ref: String,
        issued_at: u64,
        valid_until: u64,
    ) -> Result<Self, PlanningErrorV1> {
        for value in [
            &repository_installation_ref,
            &store_generation_ref,
            &producer_provenance_ref,
            &acquisition_provenance_ref,
            &privacy_basis_ref,
            &redaction_provenance_ref,
        ] {
            require_ref(value)?;
        }
        require_digest(frontier_hash)?;
        require_digest(opportunity_set_hash)?;
        if issued_at >= valid_until
            || work_and_contract_root_refs.is_empty()
            || !strictly_ordered_unique(&work_and_contract_root_refs)
            || !strictly_ordered_unique(&assumptions)
            || !strictly_ordered_unique(&observation_refs)
            || advice_units.is_empty()
            || advice_units.len() > MAX_PROPOSAL_UNITS_V1
            || advice_units.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(PlanningErrorV1::InvalidProposal);
        }
        work_and_contract_root_refs
            .iter()
            .chain(assumptions.iter())
            .chain(observation_refs.iter())
            .try_for_each(|value| require_ref(value))?;
        advice_units
            .iter()
            .try_for_each(ProposalAdviceUnitV1::validate)?;
        let mut proposal = Self {
            proposal_id,
            repository_installation_ref,
            store_generation_ref,
            frontier_hash,
            work_and_contract_root_refs,
            opportunity_set_hash,
            advice_units,
            assumptions,
            observation_refs,
            producer_provenance_ref,
            acquisition_provenance_ref,
            privacy_basis_ref,
            redaction_provenance_ref,
            issued_at,
            valid_until,
            semantic_hash: [0; 32],
        };
        proposal.semantic_hash = domain_hash(
            "maestro.vnext.planning-proposal.v1",
            &proposal.canonical_value_without_hash(),
        )?;
        Ok(proposal)
    }

    fn canonical_value_without_hash(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.proposal_id.as_bytes()),
            text(&self.repository_installation_ref),
            text(&self.store_generation_ref),
            bytes(&self.frontier_hash),
            text_array(&self.work_and_contract_root_refs),
            bytes(&self.opportunity_set_hash),
            CborValue::Array(
                self.advice_units
                    .iter()
                    .map(ProposalAdviceUnitV1::canonical_value)
                    .collect(),
            ),
            text_array(&self.assumptions),
            text_array(&self.observation_refs),
            text(&self.producer_provenance_ref),
            text(&self.acquisition_provenance_ref),
            text(&self.privacy_basis_ref),
            text(&self.redaction_provenance_ref),
            CborValue::Unsigned(self.issued_at),
            CborValue::Unsigned(self.valid_until),
        ])
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let CborValue::Array(mut fields) = self.canonical_value_without_hash() else {
            unreachable!("Planning Proposal value is an array")
        };
        fields.push(bytes(&self.semantic_hash));
        CborValue::Array(fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlanningProposalDispositionKindV1 {
    SupersededBy(PlanningProposalIdV1),
    Retracted,
    Invalidated { evidence_refs: Vec<String> },
    SecurityErased { tombstone_ref: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlanningProposalDispositionV1 {
    pub(crate) proposal_id: PlanningProposalIdV1,
    pub(crate) expected_proposal_hash: [u8; 32],
    pub(crate) kind: PlanningProposalDispositionKindV1,
    pub(crate) disposed_at: u64,
    pub(crate) reason_ref: String,
    pub(crate) semantic_hash: [u8; 32],
}

impl PlanningProposalDispositionV1 {
    pub(crate) fn new(
        proposal_id: PlanningProposalIdV1,
        expected_proposal_hash: [u8; 32],
        kind: PlanningProposalDispositionKindV1,
        disposed_at: u64,
        reason_ref: String,
    ) -> Result<Self, PlanningErrorV1> {
        require_digest(expected_proposal_hash)?;
        require_ref(&reason_ref)?;
        validate_disposition_kind(&kind)?;
        if disposed_at == 0 {
            return Err(PlanningErrorV1::InvalidDisposition);
        }
        let value = disposition_value(
            proposal_id,
            expected_proposal_hash,
            &kind,
            disposed_at,
            &reason_ref,
        );
        let semantic_hash = domain_hash("maestro.vnext.planning-proposal-disposition.v1", &value)?;
        Ok(Self {
            proposal_id,
            expected_proposal_hash,
            kind,
            disposed_at,
            reason_ref,
            semantic_hash,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let mut fields = match disposition_value(
            self.proposal_id,
            self.expected_proposal_hash,
            &self.kind,
            self.disposed_at,
            &self.reason_ref,
        ) {
            CborValue::Array(fields) => fields,
            _ => unreachable!("Planning disposition value is an array"),
        };
        fields.push(bytes(&self.semantic_hash));
        CborValue::Array(fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingPolicySnapshotV1 {
    pub(crate) policy_ref: String,
    pub(crate) evaluator_ref: String,
    pub(crate) evaluator_revision: u64,
    pub(crate) core_compatibility_ref: String,
    pub(crate) containment_precedence: bool,
    pub(crate) require_deadline_safety: bool,
    pub(crate) foundation_maximum_total_time: u64,
    pub(crate) fairness_maximum_deferral: u64,
    pub(crate) hysteresis_window: u64,
    pub(crate) overload_opportunity_limit: u64,
    pub(crate) semantic_hash: [u8; 32],
}

impl SchedulingPolicySnapshotV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Scheduling Policy snapshot carries the exact evaluator and four bounded policy dimensions"
    )]
    pub(crate) fn new(
        policy_ref: String,
        evaluator_ref: String,
        evaluator_revision: u64,
        core_compatibility_ref: String,
        foundation_maximum_total_time: u64,
        fairness_maximum_deferral: u64,
        hysteresis_window: u64,
        overload_opportunity_limit: u64,
    ) -> Result<Self, PlanningErrorV1> {
        require_ref(&policy_ref)?;
        require_ref(&evaluator_ref)?;
        require_ref(&core_compatibility_ref)?;
        if evaluator_revision == 0
            || foundation_maximum_total_time == 0
            || fairness_maximum_deferral == 0
            || overload_opportunity_limit == 0
        {
            return Err(PlanningErrorV1::InvalidPolicy);
        }
        let mut policy = Self {
            policy_ref,
            evaluator_ref,
            evaluator_revision,
            core_compatibility_ref,
            containment_precedence: true,
            require_deadline_safety: true,
            foundation_maximum_total_time,
            fairness_maximum_deferral,
            hysteresis_window,
            overload_opportunity_limit,
            semantic_hash: [0; 32],
        };
        policy.semantic_hash = domain_hash(
            "maestro.vnext.scheduling-policy-snapshot.v1",
            &policy.canonical_value_without_hash(),
        )?;
        Ok(policy)
    }

    fn canonical_value_without_hash(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.policy_ref),
            text(&self.evaluator_ref),
            CborValue::Unsigned(self.evaluator_revision),
            text(&self.core_compatibility_ref),
            CborValue::Bool(self.containment_precedence),
            CborValue::Bool(self.require_deadline_safety),
            CborValue::Unsigned(self.foundation_maximum_total_time),
            CborValue::Unsigned(self.fairness_maximum_deferral),
            CborValue::Unsigned(self.hysteresis_window),
            CborValue::Unsigned(self.overload_opportunity_limit),
        ])
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let CborValue::Array(mut fields) = self.canonical_value_without_hash() else {
            unreachable!("Scheduling Policy value is an array")
        };
        fields.push(bytes(&self.semantic_hash));
        CborValue::Array(fields)
    }

    pub(crate) const fn strength(&self) -> [u64; 4] {
        [
            u64::MAX - self.foundation_maximum_total_time,
            u64::MAX - self.fairness_maximum_deferral,
            u64::MAX - self.hysteresis_window,
            u64::MAX - self.overload_opportunity_limit,
        ]
    }

    pub(crate) const fn semantic_hash(&self) -> [u8; 32] {
        self.semantic_hash
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticPolicyDiffKindV1 {
    Equivalent,
    Strengthening,
    Weakening,
    Incomparable,
    Invalid,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticPolicyDiffV1 {
    pub(crate) old_policy_hash: Option<[u8; 32]>,
    pub(crate) candidate_policy_hash: [u8; 32],
    pub(crate) classifier_hash: [u8; 32],
    pub(crate) kind: SemanticPolicyDiffKindV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingPolicyBindingV1 {
    pub(crate) repository_installation_ref: String,
    pub(crate) store_generation_ref: String,
    pub(crate) revision: u64,
    pub(crate) expected_old_binding_hash: Option<[u8; 32]>,
    pub(crate) policy: SchedulingPolicySnapshotV1,
    pub(crate) diff: SemanticPolicyDiffV1,
    pub(crate) semantic_hash: [u8; 32],
}

impl SchedulingPolicyBindingV1 {
    pub(crate) fn new(
        repository_installation_ref: String,
        store_generation_ref: String,
        revision: u64,
        expected_old_binding_hash: Option<[u8; 32]>,
        policy: SchedulingPolicySnapshotV1,
        diff: SemanticPolicyDiffV1,
    ) -> Result<Self, PlanningErrorV1> {
        require_ref(&repository_installation_ref)?;
        require_ref(&store_generation_ref)?;
        require_digest(policy.semantic_hash)?;
        require_digest(diff.candidate_policy_hash)?;
        require_digest(diff.classifier_hash)?;
        if revision == 0
            || diff.candidate_policy_hash != policy.semantic_hash
            || diff.old_policy_hash.is_some() != expected_old_binding_hash.is_some()
            || matches!(
                diff.kind,
                SemanticPolicyDiffKindV1::Invalid | SemanticPolicyDiffKindV1::Unknown
            )
        {
            return Err(PlanningErrorV1::InvalidPolicyBinding);
        }
        let mut binding = Self {
            repository_installation_ref,
            store_generation_ref,
            revision,
            expected_old_binding_hash,
            policy,
            diff,
            semantic_hash: [0; 32],
        };
        binding.semantic_hash = domain_hash(
            "maestro.vnext.scheduling-policy-binding.v1",
            &binding.canonical_value_without_hash(),
        )?;
        Ok(binding)
    }

    fn canonical_value_without_hash(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.repository_installation_ref),
            text(&self.store_generation_ref),
            CborValue::Unsigned(self.revision),
            optional_digest(self.expected_old_binding_hash),
            self.policy.canonical_value(),
            diff_value(&self.diff),
        ])
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let CborValue::Array(mut fields) = self.canonical_value_without_hash() else {
            unreachable!("Scheduling Policy Binding value is an array")
        };
        fields.push(bytes(&self.semantic_hash));
        CborValue::Array(fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActiveHarmDispositionV1 {
    ConfirmedUncontained,
    ConfirmedContained,
    ConfirmedAbsent,
    Conflicted,
    Unknown,
    Stale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpportunityFactsV1 {
    pub(crate) opportunity: SchedulingOpportunityRefV1,
    pub(crate) facts_hash: [u8; 32],
    pub(crate) feasible: bool,
    pub(crate) containment: bool,
    pub(crate) hard_deadline_safe: bool,
    pub(crate) foundation: bool,
    pub(crate) foundation_total_time: Option<u64>,
    pub(crate) fairness_deferral: u64,
    pub(crate) feasible_load: bool,
    pub(crate) switching_cost: u64,
    pub(crate) currently_active_or_uncertain: bool,
}

impl OpportunityFactsV1 {
    pub(crate) fn validate(&self) -> Result<(), PlanningErrorV1> {
        self.opportunity.validate()?;
        require_digest(self.facts_hash)?;
        if self.foundation != self.foundation_total_time.is_some() {
            return Err(PlanningErrorV1::InvalidOpportunityFacts);
        }
        Ok(())
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.opportunity.canonical_value(),
            bytes(&self.facts_hash),
            CborValue::Bool(self.feasible),
            CborValue::Bool(self.containment),
            CborValue::Bool(self.hard_deadline_safe),
            CborValue::Bool(self.foundation),
            optional_u64(self.foundation_total_time),
            CborValue::Unsigned(self.fairness_deferral),
            CborValue::Bool(self.feasible_load),
            CborValue::Unsigned(self.switching_cost),
            CborValue::Bool(self.currently_active_or_uncertain),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingAssessmentInputKeyV1 {
    pub(crate) repository_installation_ref: String,
    pub(crate) store_generation_ref: String,
    pub(crate) projection_scope_ref: String,
    pub(crate) frontier_hash: [u8; 32],
    pub(crate) opportunity_set_hash: [u8; 32],
    pub(crate) policy_binding_hash: [u8; 32],
    pub(crate) policy_hash: [u8; 32],
    pub(crate) evaluator_hash: [u8; 32],
    pub(crate) classifier_hash: [u8; 32],
    pub(crate) safety_floor_hash: [u8; 32],
    pub(crate) proposal_closure_hash: [u8; 32],
    pub(crate) owner_fact_closure_hash: [u8; 32],
    pub(crate) observation_closure_hash: [u8; 32],
    pub(crate) trusted_as_of: u64,
    pub(crate) valid_until: u64,
    pub(crate) bounds_hash: [u8; 32],
    pub(crate) semantic_hash: [u8; 32],
}

impl SchedulingAssessmentInputKeyV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Scheduling Assessment key is the exact complete positive and negative fact closure"
    )]
    pub(crate) fn new(
        repository_installation_ref: String,
        store_generation_ref: String,
        projection_scope_ref: String,
        frontier_hash: [u8; 32],
        opportunity_set_hash: [u8; 32],
        policy_binding_hash: [u8; 32],
        policy_hash: [u8; 32],
        evaluator_hash: [u8; 32],
        classifier_hash: [u8; 32],
        safety_floor_hash: [u8; 32],
        proposal_closure_hash: [u8; 32],
        owner_fact_closure_hash: [u8; 32],
        observation_closure_hash: [u8; 32],
        trusted_as_of: u64,
        valid_until: u64,
        bounds_hash: [u8; 32],
    ) -> Result<Self, PlanningErrorV1> {
        require_ref(&repository_installation_ref)?;
        require_ref(&store_generation_ref)?;
        require_ref(&projection_scope_ref)?;
        for digest in [
            frontier_hash,
            opportunity_set_hash,
            policy_binding_hash,
            policy_hash,
            evaluator_hash,
            classifier_hash,
            safety_floor_hash,
            proposal_closure_hash,
            owner_fact_closure_hash,
            observation_closure_hash,
            bounds_hash,
        ] {
            require_digest(digest)?;
        }
        if trusted_as_of >= valid_until {
            return Err(PlanningErrorV1::InvalidAssessmentKey);
        }
        let mut key = Self {
            repository_installation_ref,
            store_generation_ref,
            projection_scope_ref,
            frontier_hash,
            opportunity_set_hash,
            policy_binding_hash,
            policy_hash,
            evaluator_hash,
            classifier_hash,
            safety_floor_hash,
            proposal_closure_hash,
            owner_fact_closure_hash,
            observation_closure_hash,
            trusted_as_of,
            valid_until,
            bounds_hash,
            semantic_hash: [0; 32],
        };
        key.semantic_hash = domain_hash(
            "maestro.vnext.scheduling-assessment-input-key.v1",
            &key.canonical_value_without_hash(),
        )?;
        Ok(key)
    }

    fn canonical_value_without_hash(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.repository_installation_ref),
            text(&self.store_generation_ref),
            text(&self.projection_scope_ref),
            bytes(&self.frontier_hash),
            bytes(&self.opportunity_set_hash),
            bytes(&self.policy_binding_hash),
            bytes(&self.policy_hash),
            bytes(&self.evaluator_hash),
            bytes(&self.classifier_hash),
            bytes(&self.safety_floor_hash),
            bytes(&self.proposal_closure_hash),
            bytes(&self.owner_fact_closure_hash),
            bytes(&self.observation_closure_hash),
            CborValue::Unsigned(self.trusted_as_of),
            CborValue::Unsigned(self.valid_until),
            bytes(&self.bounds_hash),
        ])
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let CborValue::Array(mut fields) = self.canonical_value_without_hash() else {
            unreachable!("Scheduling key value is an array")
        };
        fields.push(bytes(&self.semantic_hash));
        CborValue::Array(fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SchedulingReasonV1 {
    ActiveHarmContainment,
    HardDeadlineSafety,
    FoundationFeasible,
    FairnessDeferral,
    Hysteresis,
    StableCoreOrder,
    Overload,
    ProposalAdvice,
}

impl SchedulingReasonV1 {
    pub(crate) const fn tag(&self) -> u64 {
        match self {
            Self::ActiveHarmContainment => 1,
            Self::HardDeadlineSafety => 2,
            Self::FoundationFeasible => 3,
            Self::FairnessDeferral => 4,
            Self::Hysteresis => 5,
            Self::StableCoreOrder => 6,
            Self::Overload => 7,
            Self::ProposalAdvice => 8,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CounterfactualV1 {
    pub(crate) opportunity_ref: String,
    pub(crate) condition_ref: String,
    pub(crate) resulting_class_ordinal: u64,
}

impl CounterfactualV1 {
    pub(crate) fn validate(&self) -> Result<(), PlanningErrorV1> {
        require_ref(&self.opportunity_ref)?;
        require_ref(&self.condition_ref)?;
        if self.resulting_class_ordinal == 0 {
            return Err(PlanningErrorV1::InvalidAssessment);
        }
        Ok(())
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.opportunity_ref),
            text(&self.condition_ref),
            CborValue::Unsigned(self.resulting_class_ordinal),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingEquivalenceClassV1 {
    pub(crate) ordered_opportunity_refs: Vec<String>,
    pub(crate) reasons: Vec<SchedulingReasonV1>,
    pub(crate) uncertainty_refs: Vec<String>,
    pub(crate) counterfactuals: Vec<CounterfactualV1>,
}

impl SchedulingEquivalenceClassV1 {
    pub(crate) fn validate(&self) -> Result<(), PlanningErrorV1> {
        if self.ordered_opportunity_refs.is_empty()
            || !strictly_ordered_unique(&self.ordered_opportunity_refs)
            || self.reasons.is_empty()
            || self
                .reasons
                .windows(2)
                .any(|pair| pair[0].tag() >= pair[1].tag())
            || !strictly_ordered_unique(&self.uncertainty_refs)
            || self.counterfactuals.len() > MAX_COUNTERFACTUALS_V1
        {
            return Err(PlanningErrorV1::InvalidAssessment);
        }
        self.ordered_opportunity_refs
            .iter()
            .chain(self.uncertainty_refs.iter())
            .try_for_each(|value| require_ref(value))?;
        self.counterfactuals
            .iter()
            .try_for_each(CounterfactualV1::validate)
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            text_array(&self.ordered_opportunity_refs),
            CborValue::Array(
                self.reasons
                    .iter()
                    .map(|reason| CborValue::Unsigned(reason.tag()))
                    .collect(),
            ),
            text_array(&self.uncertainty_refs),
            CborValue::Array(
                self.counterfactuals
                    .iter()
                    .map(CounterfactualV1::canonical_value)
                    .collect(),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SchedulingAssessmentResultV1 {
    OrderedEquivalenceClasses(Vec<SchedulingEquivalenceClassV1>),
    Indeterminate { reason_refs: Vec<String> },
    Error { error_ref: String },
}

impl SchedulingAssessmentResultV1 {
    pub(crate) fn validate(&self) -> Result<(), PlanningErrorV1> {
        match self {
            Self::OrderedEquivalenceClasses(classes) => {
                if classes.is_empty() || classes.len() > MAX_OPPORTUNITIES_V1 {
                    return Err(PlanningErrorV1::InvalidAssessment);
                }
                let mut seen = Vec::new();
                for class in classes {
                    class.validate()?;
                    seen.extend(class.ordered_opportunity_refs.iter());
                }
                if seen.iter().collect::<std::collections::BTreeSet<_>>().len() != seen.len() {
                    return Err(PlanningErrorV1::InvalidAssessment);
                }
                Ok(())
            }
            Self::Indeterminate { reason_refs } => {
                if reason_refs.is_empty() || !strictly_ordered_unique(reason_refs) {
                    return Err(PlanningErrorV1::InvalidAssessment);
                }
                reason_refs.iter().try_for_each(|value| require_ref(value))
            }
            Self::Error { error_ref } => require_ref(error_ref),
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::OrderedEquivalenceClasses(classes) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Array(
                    classes
                        .iter()
                        .map(SchedulingEquivalenceClassV1::canonical_value)
                        .collect(),
                ),
            ]),
            Self::Indeterminate { reason_refs } => {
                CborValue::Array(vec![CborValue::Unsigned(2), text_array(reason_refs)])
            }
            Self::Error { error_ref } => {
                CborValue::Array(vec![CborValue::Unsigned(3), text(error_ref)])
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingAssessmentV1 {
    pub(crate) input_key: SchedulingAssessmentInputKeyV1,
    pub(crate) result: SchedulingAssessmentResultV1,
    pub(crate) semantic_hash: [u8; 32],
}

impl SchedulingAssessmentV1 {
    pub(crate) fn new(
        input_key: SchedulingAssessmentInputKeyV1,
        result: SchedulingAssessmentResultV1,
    ) -> Result<Self, PlanningErrorV1> {
        result.validate()?;
        let semantic_hash = domain_hash(
            "maestro.vnext.scheduling-assessment.v1",
            &CborValue::Array(vec![input_key.canonical_value(), result.canonical_value()]),
        )?;
        Ok(Self {
            input_key,
            result,
            semantic_hash,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.input_key.canonical_value(),
            self.result.canonical_value(),
            bytes(&self.semantic_hash),
        ])
    }
}

fn validate_disposition_kind(
    kind: &PlanningProposalDispositionKindV1,
) -> Result<(), PlanningErrorV1> {
    match kind {
        PlanningProposalDispositionKindV1::SupersededBy(_)
        | PlanningProposalDispositionKindV1::Retracted => Ok(()),
        PlanningProposalDispositionKindV1::Invalidated { evidence_refs } => {
            if evidence_refs.is_empty() || !strictly_ordered_unique(evidence_refs) {
                return Err(PlanningErrorV1::InvalidDisposition);
            }
            evidence_refs
                .iter()
                .try_for_each(|value| require_ref(value))
        }
        PlanningProposalDispositionKindV1::SecurityErased { tombstone_ref } => {
            require_ref(tombstone_ref)
        }
    }
}

fn disposition_value(
    proposal_id: PlanningProposalIdV1,
    expected_proposal_hash: [u8; 32],
    kind: &PlanningProposalDispositionKindV1,
    disposed_at: u64,
    reason_ref: &str,
) -> CborValue {
    let kind_value = match kind {
        PlanningProposalDispositionKindV1::SupersededBy(successor) => {
            CborValue::Array(vec![CborValue::Unsigned(1), bytes(successor.as_bytes())])
        }
        PlanningProposalDispositionKindV1::Retracted => {
            CborValue::Array(vec![CborValue::Unsigned(2)])
        }
        PlanningProposalDispositionKindV1::Invalidated { evidence_refs } => {
            CborValue::Array(vec![CborValue::Unsigned(3), text_array(evidence_refs)])
        }
        PlanningProposalDispositionKindV1::SecurityErased { tombstone_ref } => {
            CborValue::Array(vec![CborValue::Unsigned(4), text(tombstone_ref)])
        }
    };
    CborValue::Array(vec![
        bytes(proposal_id.as_bytes()),
        bytes(&expected_proposal_hash),
        kind_value,
        CborValue::Unsigned(disposed_at),
        text(reason_ref),
    ])
}

fn diff_value(diff: &SemanticPolicyDiffV1) -> CborValue {
    let tag = match diff.kind {
        SemanticPolicyDiffKindV1::Equivalent => 1,
        SemanticPolicyDiffKindV1::Strengthening => 2,
        SemanticPolicyDiffKindV1::Weakening => 3,
        SemanticPolicyDiffKindV1::Incomparable => 4,
        SemanticPolicyDiffKindV1::Invalid => 5,
        SemanticPolicyDiffKindV1::Unknown => 6,
    };
    CborValue::Array(vec![
        optional_digest(diff.old_policy_hash),
        bytes(&diff.candidate_policy_hash),
        bytes(&diff.classifier_hash),
        CborValue::Unsigned(tag),
    ])
}

pub(crate) fn bytes(value: &[u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

pub(crate) fn text(value: &str) -> CborValue {
    CborValue::Text(value.to_owned())
}

pub(crate) fn text_array(values: &[String]) -> CborValue {
    CborValue::Array(values.iter().map(|value| text(value)).collect())
}

pub(crate) fn optional_digest(value: Option<[u8; 32]>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), bytes(&value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

pub(crate) fn optional_text(value: Option<&str>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), text(value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

pub(crate) fn optional_u64(value: Option<u64>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

pub(crate) fn hash_value(domain: &str, value: &CborValue) -> Result<[u8; 32], PlanningErrorV1> {
    domain_hash(domain, value)
}

fn require_ref(value: &str) -> Result<(), PlanningErrorV1> {
    if value.is_empty()
        || value.len() > MAX_REF_BYTES_V1
        || value.trim() != value
        || value.contains('\0')
    {
        return Err(PlanningErrorV1::InvalidReference);
    }
    Ok(())
}

fn require_digest(value: [u8; 32]) -> Result<(), PlanningErrorV1> {
    if value == [0; 32] {
        return Err(PlanningErrorV1::InvalidDigest);
    }
    Ok(())
}

fn strictly_ordered_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn domain_hash(domain: &str, value: &CborValue) -> Result<[u8; 32], PlanningErrorV1> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            text(domain),
            value.clone(),
        ]))?)
        .into(),
    )
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum PlanningErrorV1 {
    #[error("Planning reference is empty, oversized, untrimmed, or NUL-bearing")]
    InvalidReference,
    #[error("Planning semantic digest must be nonzero")]
    InvalidDigest,
    #[error("Scheduling Opportunity is outside the ActionRef or complete WaveRef grammar")]
    InvalidOpportunity,
    #[error("Scheduling Opportunity Set is incomplete, unbounded, duplicated, or unordered")]
    IncompleteOpportunitySet,
    #[error("Planning Proposal violates immutable advisory or provenance requirements")]
    InvalidProposal,
    #[error("Planning Proposal Disposition is not one exact terminal append")]
    InvalidDisposition,
    #[error("Scheduling Policy violates the non-authorizing core floor")]
    InvalidPolicy,
    #[error("Scheduling Policy Binding is stale, malformed, or uses an invalid diff")]
    InvalidPolicyBinding,
    #[error("Scheduling opportunity facts are internally inconsistent")]
    InvalidOpportunityFacts,
    #[error("Scheduling Assessment input key is incomplete or has invalid trusted time")]
    InvalidAssessmentKey,
    #[error("Scheduling Assessment result is malformed, overlapping, or empty")]
    InvalidAssessment,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
