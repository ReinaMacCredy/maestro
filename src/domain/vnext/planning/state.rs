use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::vnext::authority::{PrincipalIdV1, SessionIdV1};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::evaluation::{
    SchedulingEvaluationInputV1, SchedulingSafetyFloorV1, classify_policy_diff, evaluate_scheduling,
};
use super::model::{
    PlanningErrorV1, PlanningProposalDispositionV1, PlanningProposalIdV1, PlanningProposalV1,
    SchedulingAssessmentV1, SchedulingPolicyBindingV1, bytes, hash_value,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlanningRecordV1 {
    Proposal(PlanningProposalV1),
    ProposalDisposition(PlanningProposalDispositionV1),
    PolicyBinding(SchedulingPolicyBindingV1),
    SchedulingAssessment(SchedulingAssessmentV1),
}

impl PlanningRecordV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Proposal(value) => tagged(1, value.canonical_value()),
            Self::ProposalDisposition(value) => tagged(2, value.canonical_value()),
            Self::PolicyBinding(value) => tagged(3, value.canonical_value()),
            Self::SchedulingAssessment(value) => tagged(4, value.canonical_value()),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlanningStateV1 {
    proposals: BTreeMap<PlanningProposalIdV1, PlanningProposalV1>,
    proposal_dispositions: BTreeMap<PlanningProposalIdV1, PlanningProposalDispositionV1>,
    current_policy_binding: Option<SchedulingPolicyBindingV1>,
    policy_binding_history: Vec<SchedulingPolicyBindingV1>,
    assessments: BTreeMap<[u8; 32], SchedulingAssessmentV1>,
}

impl PlanningStateV1 {
    pub(crate) fn proposal(&self, id: PlanningProposalIdV1) -> Option<&PlanningProposalV1> {
        self.proposals.get(&id)
    }

    pub(crate) fn proposal_disposition(
        &self,
        id: PlanningProposalIdV1,
    ) -> Option<&PlanningProposalDispositionV1> {
        self.proposal_dispositions.get(&id)
    }

    pub(crate) const fn current_policy_binding(&self) -> Option<&SchedulingPolicyBindingV1> {
        self.current_policy_binding.as_ref()
    }

    pub(crate) fn assessment(&self, key_hash: [u8; 32]) -> Option<&SchedulingAssessmentV1> {
        self.assessments.get(&key_hash)
    }

    pub(crate) fn applicable_proposals(
        &self,
        as_of: u64,
        frontier_hash: [u8; 32],
        opportunity_set_hash: [u8; 32],
    ) -> Vec<&PlanningProposalV1> {
        let mut proposals = self
            .proposals
            .values()
            .filter(|proposal| {
                proposal.issued_at <= as_of
                    && as_of < proposal.valid_until
                    && proposal.frontier_hash == frontier_hash
                    && proposal.opportunity_set_hash == opportunity_set_hash
                    && !self
                        .proposal_dispositions
                        .contains_key(&proposal.proposal_id)
            })
            .collect::<Vec<_>>();
        proposals.sort_by_key(|proposal| (proposal.semantic_hash, proposal.proposal_id));
        proposals
    }

    pub(crate) fn semantic_hash(&self) -> Result<[u8; 32], PlanningStateErrorV1> {
        let records = self
            .proposals
            .values()
            .cloned()
            .map(PlanningRecordV1::Proposal)
            .chain(
                self.proposal_dispositions
                    .values()
                    .cloned()
                    .map(PlanningRecordV1::ProposalDisposition),
            )
            .chain(
                self.policy_binding_history
                    .iter()
                    .cloned()
                    .map(PlanningRecordV1::PolicyBinding),
            )
            .chain(
                self.assessments
                    .values()
                    .cloned()
                    .map(PlanningRecordV1::SchedulingAssessment),
            )
            .map(|record| record.canonical_value())
            .collect();
        Ok(hash_value(
            "maestro.vnext.planning-state.v1",
            &CborValue::Array(records),
        )?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlanningMutationV1 {
    PublishPlanningProposal {
        actor_principal: PrincipalIdV1,
        actor_session: SessionIdV1,
        proposal: PlanningProposalV1,
    },
    DisposePlanningProposal {
        actor_principal: PrincipalIdV1,
        actor_session: SessionIdV1,
        disposition: PlanningProposalDispositionV1,
    },
    PublishSchedulingPolicyBinding {
        actor_principal: PrincipalIdV1,
        actor_session: SessionIdV1,
        binding: SchedulingPolicyBindingV1,
        safety_floor: SchedulingSafetyFloorV1,
    },
    PublishSchedulingAssessment {
        actor_principal: PrincipalIdV1,
        actor_session: SessionIdV1,
        evaluation: Box<SchedulingEvaluationInputV1>,
    },
}

impl PlanningMutationV1 {
    pub(crate) const fn action_literal(&self) -> &'static str {
        match self {
            Self::PublishPlanningProposal { .. } => "PublishPlanningProposal",
            Self::DisposePlanningProposal { .. } => "DisposePlanningProposal",
            Self::PublishSchedulingPolicyBinding { .. } => "PublishSchedulingPolicyBinding",
            Self::PublishSchedulingAssessment { .. } => "PublishSchedulingAssessment",
        }
    }

    pub(crate) const fn actor(&self) -> (PrincipalIdV1, SessionIdV1) {
        match self {
            Self::PublishPlanningProposal {
                actor_principal,
                actor_session,
                ..
            }
            | Self::DisposePlanningProposal {
                actor_principal,
                actor_session,
                ..
            }
            | Self::PublishSchedulingPolicyBinding {
                actor_principal,
                actor_session,
                ..
            }
            | Self::PublishSchedulingAssessment {
                actor_principal,
                actor_session,
                ..
            } => (*actor_principal, *actor_session),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PlanningTransitionV1 {
    action_literal: &'static str,
    subject_commitment: [u8; 32],
    owner_basis_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    actor_principal: PrincipalIdV1,
    actor_session: SessionIdV1,
    disposition: PlanningTransitionDispositionV1,
    records: Vec<PlanningRecordV1>,
    scheduling_safety_floor: Option<SchedulingSafetyFloorV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlanningTransitionDispositionV1 {
    Committed,
    Deduplicated,
}

impl PlanningTransitionV1 {
    pub(crate) const fn action_literal(&self) -> &'static str {
        self.action_literal
    }

    pub(crate) const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub(crate) const fn owner_basis_commitment(&self) -> [u8; 32] {
        self.owner_basis_commitment
    }

    pub(crate) const fn payload_commitment(&self) -> [u8; 32] {
        self.payload_commitment
    }

    pub(crate) const fn actor_principal(&self) -> PrincipalIdV1 {
        self.actor_principal
    }

    pub(crate) const fn actor_session(&self) -> SessionIdV1 {
        self.actor_session
    }

    pub(crate) const fn disposition(&self) -> PlanningTransitionDispositionV1 {
        self.disposition
    }

    pub(crate) fn records(&self) -> &[PlanningRecordV1] {
        &self.records
    }

    pub(crate) const fn scheduling_safety_floor(&self) -> Option<&SchedulingSafetyFloorV1> {
        self.scheduling_safety_floor.as_ref()
    }
}

pub(crate) fn apply_planning_mutation(
    state: &mut PlanningStateV1,
    mutation: PlanningMutationV1,
) -> Result<PlanningTransitionV1, PlanningStateErrorV1> {
    let action_literal = mutation.action_literal();
    let actor = mutation.actor();
    let scheduling_safety_floor = match &mutation {
        PlanningMutationV1::PublishSchedulingPolicyBinding { safety_floor, .. } => {
            Some(safety_floor.clone())
        }
        _ => None,
    };
    let (subject, owner_basis, records) = match &mutation {
        PlanningMutationV1::PublishPlanningProposal { proposal, .. } => {
            publish_proposal(state, proposal)?
        }
        PlanningMutationV1::DisposePlanningProposal { disposition, .. } => {
            dispose_proposal(state, disposition)?
        }
        PlanningMutationV1::PublishSchedulingPolicyBinding {
            binding,
            safety_floor,
            ..
        } => publish_policy_binding(state, binding, safety_floor)?,
        PlanningMutationV1::PublishSchedulingAssessment { evaluation, .. } => {
            publish_assessment(state, evaluation)?
        }
    };
    let payload = mutation_value(&mutation)?;
    let disposition = if records.is_empty() {
        PlanningTransitionDispositionV1::Deduplicated
    } else {
        PlanningTransitionDispositionV1::Committed
    };
    Ok(PlanningTransitionV1 {
        action_literal,
        subject_commitment: hash_value("maestro.vnext.planning-mutation-subject.v1", &subject)?,
        owner_basis_commitment: hash_value("maestro.vnext.planning-owner-basis.v1", &owner_basis)?,
        payload_commitment: hash_value("maestro.vnext.planning-mutation-payload.v1", &payload)?,
        actor_principal: actor.0,
        actor_session: actor.1,
        disposition,
        records,
        scheduling_safety_floor,
    })
}

fn publish_proposal(
    state: &mut PlanningStateV1,
    proposal: &PlanningProposalV1,
) -> Result<(CborValue, CborValue, Vec<PlanningRecordV1>), PlanningStateErrorV1> {
    if state.proposals.contains_key(&proposal.proposal_id) {
        return Err(PlanningStateErrorV1::DuplicateProposalIdentity);
    }
    if state
        .proposals
        .values()
        .any(|existing| existing.semantic_hash == proposal.semantic_hash)
    {
        return Err(PlanningStateErrorV1::DuplicateProposalAdvice);
    }
    state
        .proposals
        .insert(proposal.proposal_id, proposal.clone());
    Ok((
        bytes(proposal.proposal_id.as_bytes()),
        absent(),
        vec![PlanningRecordV1::Proposal(proposal.clone())],
    ))
}

fn dispose_proposal(
    state: &mut PlanningStateV1,
    disposition: &PlanningProposalDispositionV1,
) -> Result<(CborValue, CborValue, Vec<PlanningRecordV1>), PlanningStateErrorV1> {
    let proposal = state
        .proposals
        .get(&disposition.proposal_id)
        .ok_or(PlanningStateErrorV1::UnknownProposal)?;
    if proposal.semantic_hash != disposition.expected_proposal_hash {
        return Err(PlanningStateErrorV1::StaleProposal);
    }
    if state
        .proposal_dispositions
        .contains_key(&disposition.proposal_id)
    {
        return Err(PlanningStateErrorV1::ProposalAlreadyDisposed);
    }
    if let super::model::PlanningProposalDispositionKindV1::SupersededBy(successor) =
        &disposition.kind
        && (!state.proposals.contains_key(successor) || *successor == disposition.proposal_id)
    {
        return Err(PlanningStateErrorV1::InvalidDispositionSuccessor);
    }
    state
        .proposal_dispositions
        .insert(disposition.proposal_id, disposition.clone());
    Ok((
        bytes(disposition.proposal_id.as_bytes()),
        CborValue::Array(vec![bytes(&proposal.semantic_hash), absent()]),
        vec![PlanningRecordV1::ProposalDisposition(disposition.clone())],
    ))
}

fn publish_policy_binding(
    state: &mut PlanningStateV1,
    binding: &SchedulingPolicyBindingV1,
    safety_floor: &SchedulingSafetyFloorV1,
) -> Result<(CborValue, CborValue, Vec<PlanningRecordV1>), PlanningStateErrorV1> {
    let expected_old = state
        .current_policy_binding
        .as_ref()
        .map(|current| current.semantic_hash);
    if binding.expected_old_binding_hash != expected_old
        || binding.revision
            != state
                .current_policy_binding
                .as_ref()
                .map_or(1, |current| current.revision + 1)
    {
        return Err(PlanningStateErrorV1::StalePolicyBinding);
    }
    let expected_diff = classify_policy_diff(
        state
            .current_policy_binding
            .as_ref()
            .map(|current| &current.policy),
        &binding.policy,
        safety_floor,
    )?;
    if binding.diff != expected_diff {
        return Err(PlanningStateErrorV1::PolicyDiffMismatch);
    }
    let subject_hash = hash_value(
        "maestro.vnext.scheduling-policy-binding-subject.v1",
        &CborValue::Array(vec![
            super::model::text(&binding.repository_installation_ref),
            super::model::text(&binding.store_generation_ref),
        ]),
    )?;
    state.current_policy_binding = Some(binding.clone());
    state.policy_binding_history.push(binding.clone());
    Ok((
        bytes(&subject_hash),
        super::model::optional_digest(expected_old),
        vec![PlanningRecordV1::PolicyBinding(binding.clone())],
    ))
}

fn publish_assessment(
    state: &mut PlanningStateV1,
    evaluation: &SchedulingEvaluationInputV1,
) -> Result<(CborValue, CborValue, Vec<PlanningRecordV1>), PlanningStateErrorV1> {
    let assessment = evaluate_scheduling(evaluation)?;
    let key = assessment.input_key.semantic_hash;
    if let Some(existing) = state.assessments.get(&key) {
        if existing != &assessment {
            return Err(PlanningStateErrorV1::AssessmentIntegrityFault);
        }
        return Ok((bytes(&key), bytes(&existing.semantic_hash), Vec::new()));
    }
    state.assessments.insert(key, assessment.clone());
    Ok((
        bytes(&key),
        absent(),
        vec![PlanningRecordV1::SchedulingAssessment(assessment)],
    ))
}

fn mutation_value(mutation: &PlanningMutationV1) -> Result<CborValue, PlanningStateErrorV1> {
    Ok(match mutation {
        PlanningMutationV1::PublishPlanningProposal {
            actor_principal,
            actor_session,
            proposal,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            actor(*actor_principal, *actor_session),
            proposal.canonical_value(),
        ]),
        PlanningMutationV1::DisposePlanningProposal {
            actor_principal,
            actor_session,
            disposition,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            actor(*actor_principal, *actor_session),
            disposition.canonical_value(),
        ]),
        PlanningMutationV1::PublishSchedulingPolicyBinding {
            actor_principal,
            actor_session,
            binding,
            safety_floor,
        } => CborValue::Array(vec![
            CborValue::Unsigned(3),
            actor(*actor_principal, *actor_session),
            binding.canonical_value(),
            bytes(&safety_floor.semantic_hash),
        ]),
        PlanningMutationV1::PublishSchedulingAssessment {
            actor_principal,
            actor_session,
            evaluation,
        } => {
            let assessment = evaluate_scheduling(evaluation)?;
            CborValue::Array(vec![
                CborValue::Unsigned(4),
                actor(*actor_principal, *actor_session),
                assessment.canonical_value(),
            ])
        }
    })
}

fn actor(principal: PrincipalIdV1, session: SessionIdV1) -> CborValue {
    CborValue::Array(vec![bytes(principal.as_bytes()), bytes(session.as_bytes())])
}

fn tagged(tag: u64, value: CborValue) -> CborValue {
    CborValue::Array(vec![CborValue::Unsigned(tag), value])
}

fn absent() -> CborValue {
    CborValue::Array(vec![CborValue::Unsigned(0)])
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum PlanningStateErrorV1 {
    #[error(transparent)]
    Model(#[from] PlanningErrorV1),
    #[error("Planning Proposal identity already exists")]
    DuplicateProposalIdentity,
    #[error("distinct Planning Proposal identity repeats the same semantic advice unit")]
    DuplicateProposalAdvice,
    #[error("Planning Proposal does not exist")]
    UnknownProposal,
    #[error("Planning Proposal hash changed before terminal disposition")]
    StaleProposal,
    #[error("Planning Proposal already has one terminal disposition")]
    ProposalAlreadyDisposed,
    #[error("superseded_by must name one distinct existing Proposal")]
    InvalidDispositionSuccessor,
    #[error("Scheduling Policy Binding expected-old CAS or revision lost")]
    StalePolicyBinding,
    #[error("Scheduling Policy Binding diff does not match the pinned pure classifier")]
    PolicyDiffMismatch,
    #[error("same Scheduling Assessment key already has different output")]
    AssessmentIntegrityFault,
}

#[cfg(test)]
pub(crate) mod test_adapter {
    use super::*;

    pub(crate) fn apply(
        state: &PlanningStateV1,
        mutation: PlanningMutationV1,
    ) -> Result<(PlanningStateV1, PlanningTransitionV1), PlanningStateErrorV1> {
        let mut next = state.clone();
        let transition = apply_planning_mutation(&mut next, mutation)?;
        Ok((next, transition))
    }
}
