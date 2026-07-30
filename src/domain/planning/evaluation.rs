use std::collections::BTreeMap;

use crate::foundation::core::deterministic_cbor::CborValue;

use super::model::{
    ActiveHarmDispositionV1, OpportunityFactsV1, PlanningErrorV1, PlanningProposalV1,
    SchedulingAssessmentInputKeyV1, SchedulingAssessmentResultV1, SchedulingAssessmentV1,
    SchedulingEquivalenceClassV1, SchedulingOpportunitySetV1, SchedulingPolicyBindingV1,
    SchedulingPolicySnapshotV1, SchedulingReasonV1, SemanticPolicyDiffKindV1, SemanticPolicyDiffV1,
    bytes, hash_value, text,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingSafetyFloorV1 {
    pub(crate) floor_ref: String,
    pub(crate) evaluator_compatibility_ref: String,
    pub(crate) classifier_hash: [u8; 32],
    pub(crate) classifier_revision: u64,
    pub(crate) require_containment_precedence: bool,
    pub(crate) require_deadline_safety: bool,
    pub(crate) maximum_foundation_total_time: u64,
    pub(crate) maximum_fairness_deferral: u64,
    pub(crate) maximum_hysteresis_window: u64,
    pub(crate) maximum_overload_opportunity_limit: u64,
    pub(crate) semantic_hash: [u8; 32],
}

impl SchedulingSafetyFloorV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the safety floor binds every scheduling ceiling in one refusal gate"
    )]
    pub(crate) fn new(
        floor_ref: String,
        evaluator_compatibility_ref: String,
        classifier_hash: [u8; 32],
        classifier_revision: u64,
        maximum_foundation_total_time: u64,
        maximum_fairness_deferral: u64,
        maximum_hysteresis_window: u64,
        maximum_overload_opportunity_limit: u64,
    ) -> Result<Self, PlanningErrorV1> {
        // A ceiling of u64::MAX would collapse that lane's floor strength to
        // zero, which the Authority seam refuses as an absent floor.
        if floor_ref.is_empty()
            || evaluator_compatibility_ref.is_empty()
            || classifier_hash == [0; 32]
            || classifier_revision == 0
            || [
                maximum_foundation_total_time,
                maximum_fairness_deferral,
                maximum_hysteresis_window,
                maximum_overload_opportunity_limit,
            ]
            .contains(&u64::MAX)
        {
            return Err(PlanningErrorV1::InvalidPolicy);
        }
        let mut floor = Self {
            floor_ref,
            evaluator_compatibility_ref,
            classifier_hash,
            classifier_revision,
            require_containment_precedence: true,
            require_deadline_safety: true,
            maximum_foundation_total_time,
            maximum_fairness_deferral,
            maximum_hysteresis_window,
            maximum_overload_opportunity_limit,
            semantic_hash: [0; 32],
        };
        floor.semantic_hash = hash_value(
            "maestro.vnext.scheduling-safety-floor.v1",
            &floor.canonical_value_without_hash(),
        )?;
        Ok(floor)
    }

    fn canonical_value_without_hash(&self) -> CborValue {
        CborValue::Array(vec![
            text(&self.floor_ref),
            text(&self.evaluator_compatibility_ref),
            bytes(&self.classifier_hash),
            CborValue::Unsigned(self.classifier_revision),
            CborValue::Bool(self.require_containment_precedence),
            CborValue::Bool(self.require_deadline_safety),
            CborValue::Unsigned(self.maximum_foundation_total_time),
            CborValue::Unsigned(self.maximum_fairness_deferral),
            CborValue::Unsigned(self.maximum_hysteresis_window),
            CborValue::Unsigned(self.maximum_overload_opportunity_limit),
        ])
    }

    pub(crate) fn admits(
        &self,
        policy: &SchedulingPolicySnapshotV1,
    ) -> Result<(), PlanningErrorV1> {
        if (self.require_containment_precedence && !policy.containment_precedence)
            || (self.require_deadline_safety && !policy.require_deadline_safety)
            || policy.core_compatibility_ref != self.evaluator_compatibility_ref
            || policy.foundation_maximum_total_time > self.maximum_foundation_total_time
            || policy.fairness_maximum_deferral > self.maximum_fairness_deferral
            || policy.hysteresis_window > self.maximum_hysteresis_window
            || policy.overload_opportunity_limit > self.maximum_overload_opportunity_limit
        {
            return Err(PlanningErrorV1::InvalidPolicy);
        }
        Ok(())
    }

    /// Floor strengths in the same orientation as `policy_strength`: a policy
    /// is numerically admitted exactly when its strength vector sits at or
    /// above this floor lane by lane.
    pub(crate) const fn strength(&self) -> [u64; 4] {
        [
            u64::MAX - self.maximum_foundation_total_time,
            u64::MAX - self.maximum_fairness_deferral,
            u64::MAX - self.maximum_hysteresis_window,
            u64::MAX - self.maximum_overload_opportunity_limit,
        ]
    }

    pub(crate) const fn semantic_hash(&self) -> [u8; 32] {
        self.semantic_hash
    }
}

pub(crate) fn classify_policy_diff(
    old: Option<&SchedulingPolicySnapshotV1>,
    candidate: &SchedulingPolicySnapshotV1,
    floor: &SchedulingSafetyFloorV1,
) -> Result<SemanticPolicyDiffV1, PlanningErrorV1> {
    floor.admits(candidate)?;
    let kind = match old {
        None => SemanticPolicyDiffKindV1::Strengthening,
        Some(old)
            if old.evaluator_ref != candidate.evaluator_ref
                || old.core_compatibility_ref != candidate.core_compatibility_ref =>
        {
            SemanticPolicyDiffKindV1::Incomparable
        }
        Some(old) if old == candidate => SemanticPolicyDiffKindV1::Equivalent,
        Some(old) => {
            let candidate_is_no_weaker = candidate.foundation_maximum_total_time
                <= old.foundation_maximum_total_time
                && candidate.fairness_maximum_deferral <= old.fairness_maximum_deferral
                && candidate.hysteresis_window <= old.hysteresis_window
                && candidate.overload_opportunity_limit <= old.overload_opportunity_limit;
            let candidate_is_no_stronger = candidate.foundation_maximum_total_time
                >= old.foundation_maximum_total_time
                && candidate.fairness_maximum_deferral >= old.fairness_maximum_deferral
                && candidate.hysteresis_window >= old.hysteresis_window
                && candidate.overload_opportunity_limit >= old.overload_opportunity_limit;
            match (candidate_is_no_weaker, candidate_is_no_stronger) {
                (true, false) => SemanticPolicyDiffKindV1::Strengthening,
                (false, true) => SemanticPolicyDiffKindV1::Weakening,
                (true, true) => SemanticPolicyDiffKindV1::Equivalent,
                (false, false) => SemanticPolicyDiffKindV1::Incomparable,
            }
        }
    };
    Ok(SemanticPolicyDiffV1 {
        old_policy_hash: old.map(|policy| policy.semantic_hash),
        candidate_policy_hash: candidate.semantic_hash,
        classifier_hash: floor.classifier_hash,
        kind,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingEvaluationInputV1 {
    pub(crate) key: SchedulingAssessmentInputKeyV1,
    pub(crate) opportunity_set: SchedulingOpportunitySetV1,
    pub(crate) policy_binding: SchedulingPolicyBindingV1,
    pub(crate) safety_floor: SchedulingSafetyFloorV1,
    pub(crate) active_harm: ActiveHarmDispositionV1,
    pub(crate) opportunity_facts: Vec<OpportunityFactsV1>,
    pub(crate) applicable_proposals: Vec<PlanningProposalV1>,
    pub(crate) observation_closure_member_hashes: Vec<[u8; 32]>,
    pub(crate) complete_owner_fact_closure: bool,
    pub(crate) complete_proposal_closure: bool,
}

impl SchedulingEvaluationInputV1 {
    fn validate(&self) -> Result<(), PlanningErrorV1> {
        self.safety_floor.admits(&self.policy_binding.policy)?;
        let exact_owner_fact_closure = owner_fact_closure_hash(&self.opportunity_facts)?;
        let exact_proposal_closure = proposal_closure_hash(&self.applicable_proposals)?;
        let exact_observation_closure =
            observation_closure_hash(&self.observation_closure_member_hashes)?;
        if !self.complete_owner_fact_closure
            || !self.complete_proposal_closure
            || self.key.frontier_hash != self.opportunity_set.frontier_hash
            || self.key.opportunity_set_hash != self.opportunity_set.semantic_hash
            || self.key.policy_binding_hash != self.policy_binding.semantic_hash
            || self.key.policy_hash != self.policy_binding.policy.semantic_hash
            || self.key.classifier_hash != self.safety_floor.classifier_hash
            || self.key.safety_floor_hash != self.safety_floor.semantic_hash
            || self.key.owner_fact_closure_hash != exact_owner_fact_closure
            || self.key.proposal_closure_hash != exact_proposal_closure
            || self.key.observation_closure_hash != exact_observation_closure
            || self.opportunity_facts.len() != self.opportunity_set.opportunities.len()
        {
            return Err(PlanningErrorV1::InvalidAssessmentKey);
        }
        for (facts, opportunity) in self
            .opportunity_facts
            .iter()
            .zip(&self.opportunity_set.opportunities)
        {
            facts.validate()?;
            if &facts.opportunity != opportunity {
                return Err(PlanningErrorV1::InvalidOpportunityFacts);
            }
        }
        if self.applicable_proposals.windows(2).any(|pair| {
            (pair[0].semantic_hash, pair[0].proposal_id)
                >= (pair[1].semantic_hash, pair[1].proposal_id)
        }) {
            return Err(PlanningErrorV1::InvalidProposal);
        }
        if self.applicable_proposals.iter().any(|proposal| {
            proposal.frontier_hash != self.key.frontier_hash
                || proposal.opportunity_set_hash != self.key.opportunity_set_hash
                || proposal.valid_until <= self.key.trusted_as_of
                || proposal.issued_at > self.key.trusted_as_of
        }) {
            return Err(PlanningErrorV1::InvalidProposal);
        }
        Ok(())
    }
}

pub(crate) fn evaluate_scheduling(
    input: &SchedulingEvaluationInputV1,
) -> Result<SchedulingAssessmentV1, PlanningErrorV1> {
    input.validate()?;
    let result = match input.active_harm {
        ActiveHarmDispositionV1::Conflicted => indeterminate("planning:active-harm:conflicted"),
        ActiveHarmDispositionV1::Unknown => indeterminate("planning:active-harm:unknown"),
        ActiveHarmDispositionV1::Stale => indeterminate("planning:active-harm:stale"),
        ActiveHarmDispositionV1::ConfirmedUncontained => evaluate_ranked(input, true),
        ActiveHarmDispositionV1::ConfirmedContained | ActiveHarmDispositionV1::ConfirmedAbsent => {
            evaluate_ranked(input, false)
        }
    };
    SchedulingAssessmentV1::new(input.key.clone(), result)
}

fn evaluate_ranked(
    input: &SchedulingEvaluationInputV1,
    containment_only: bool,
) -> SchedulingAssessmentResultV1 {
    let mut candidates = input
        .opportunity_facts
        .iter()
        .filter(|facts| facts.feasible)
        .filter(|facts| !containment_only || facts.containment)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return indeterminate(if containment_only {
            "planning:active-harm:no-eligible-containment"
        } else {
            "planning:no-feasible-opportunity"
        });
    }
    if candidates
        .iter()
        .any(|facts| facts.currently_active_or_uncertain)
    {
        let active = candidates
            .iter()
            .filter(|facts| facts.currently_active_or_uncertain)
            .count();
        if active != candidates.len() {
            return indeterminate("planning:active-or-uncertain:non-preemption");
        }
    }
    candidates.sort_by_key(|facts| facts.opportunity.stable_ref());
    let overload = candidates.len() as u64 > input.policy_binding.policy.overload_opportunity_limit
        || candidates.iter().any(|facts| !facts.feasible_load);
    let mut classes: BTreeMap<RankKeyV1, Vec<String>> = BTreeMap::new();
    for facts in candidates {
        let foundation_admissible = facts.foundation
            && facts.foundation_total_time.is_some_and(|total| {
                total <= input.policy_binding.policy.foundation_maximum_total_time
            });
        let fairness_due = !overload
            && facts.fairness_deferral >= input.policy_binding.policy.fairness_maximum_deferral;
        let key = RankKeyV1 {
            containment: u8::from(!facts.containment),
            deadline: u8::from(!facts.hard_deadline_safe),
            foundation: u8::from(!foundation_admissible),
            fairness: u8::from(!fairness_due),
            hysteresis: u64::from(
                facts.switching_cost > input.policy_binding.policy.hysteresis_window,
            ),
            proposal_preference: u64::MAX - proposal_support(input, facts.opportunity.stable_ref()),
        };
        classes
            .entry(key)
            .or_default()
            .push(facts.opportunity.stable_ref().to_owned());
    }
    SchedulingAssessmentResultV1::OrderedEquivalenceClasses(
        classes
            .into_iter()
            .map(|(key, opportunities)| SchedulingEquivalenceClassV1 {
                ordered_opportunity_refs: opportunities,
                reasons: reasons_for_key(key, containment_only, overload),
                uncertainty_refs: Vec::new(),
                counterfactuals: Vec::new(),
            })
            .collect(),
    )
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RankKeyV1 {
    containment: u8,
    deadline: u8,
    foundation: u8,
    fairness: u8,
    hysteresis: u64,
    proposal_preference: u64,
}

fn reasons_for_key(
    key: RankKeyV1,
    containment_only: bool,
    overload: bool,
) -> Vec<SchedulingReasonV1> {
    let mut reasons = Vec::new();
    if containment_only || key.containment == 0 {
        reasons.push(SchedulingReasonV1::ActiveHarmContainment);
    }
    if key.deadline == 0 {
        reasons.push(SchedulingReasonV1::HardDeadlineSafety);
    }
    if key.foundation == 0 {
        reasons.push(SchedulingReasonV1::FoundationFeasible);
    }
    if key.fairness == 0 && !overload {
        reasons.push(SchedulingReasonV1::FairnessDeferral);
    }
    if key.hysteresis == 0 {
        reasons.push(SchedulingReasonV1::Hysteresis);
    }
    if key.proposal_preference != u64::MAX {
        reasons.push(SchedulingReasonV1::ProposalAdvice);
    }
    if overload {
        reasons.push(SchedulingReasonV1::Overload);
    }
    reasons.push(SchedulingReasonV1::StableCoreOrder);
    reasons.sort_by_key(SchedulingReasonV1::tag);
    reasons.dedup();
    reasons
}

fn indeterminate(reason: &str) -> SchedulingAssessmentResultV1 {
    SchedulingAssessmentResultV1::Indeterminate {
        reason_refs: vec![reason.to_owned()],
    }
}

fn proposal_support(input: &SchedulingEvaluationInputV1, opportunity_ref: &str) -> u64 {
    input
        .applicable_proposals
        .iter()
        .flat_map(|proposal| &proposal.advice_units)
        .filter(|unit| {
            unit.covered_opportunity_refs
                .binary_search_by(|value| value.as_str().cmp(opportunity_ref))
                .is_ok()
        })
        .count() as u64
}

pub(crate) fn owner_fact_closure_hash(
    facts: &[OpportunityFactsV1],
) -> Result<[u8; 32], PlanningErrorV1> {
    hash_value(
        "maestro.vnext.scheduling-owner-fact-closure.v1",
        &CborValue::Array(
            facts
                .iter()
                .map(OpportunityFactsV1::canonical_value)
                .collect(),
        ),
    )
}

pub(crate) fn proposal_closure_hash(
    proposals: &[PlanningProposalV1],
) -> Result<[u8; 32], PlanningErrorV1> {
    hash_value(
        "maestro.vnext.scheduling-proposal-closure.v1",
        &CborValue::Array(
            proposals
                .iter()
                .map(PlanningProposalV1::canonical_value)
                .collect(),
        ),
    )
}

pub(crate) fn observation_closure_hash(
    member_hashes: &[[u8; 32]],
) -> Result<[u8; 32], PlanningErrorV1> {
    if member_hashes.contains(&[0; 32]) || member_hashes.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PlanningErrorV1::InvalidAssessmentKey);
    }
    hash_value(
        "maestro.vnext.scheduling-observation-closure.v1",
        &CborValue::Array(member_hashes.iter().map(bytes).collect()),
    )
}
