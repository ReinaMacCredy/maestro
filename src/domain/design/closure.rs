use std::collections::{BTreeMap, BTreeSet};

use crate::domain::identity::{
    ContractRootIdV1, DecisionClosureIdV1, DecisionResolutionIdV1, SchemaClosureV1,
    decision_closure_identity,
};
use crate::domain::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborValue};

use super::{
    AlternativeConsequenceV1, DecisionIdV1, DecisionLineageV1,
    DecisionMaterializationDispositionV1, DecisionMaterializationV1, DecisionStateV1, DecisionV1,
    DecisionV1Error,
};

const DECISION_CLOSURE_MANIFEST_VERSION_V1: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum DecisionClosureEffectStatusV1 {
    NoContractEffect = 1,
    AppliedExactRoot = 2,
    Historical = 3,
    SupersededButEffectLive = 4,
    Withdrawn = 5,
    AppliedEquivalentRoot = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum DecisionClosureTerminalStatusV1 {
    Resolved = 1,
    Withdrawn = 2,
    Superseded = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionClosureRecordV1 {
    decision_id: DecisionIdV1,
    revision_ids: Vec<super::DecisionRevisionIdV1>,
    terminal_status: DecisionClosureTerminalStatusV1,
    resolution_id: Option<DecisionResolutionIdV1>,
    supersession_id: Option<super::DecisionSupersessionIdV1>,
    materialization_ids: Vec<crate::domain::identity::DecisionMaterializationIdV1>,
    effect_status: DecisionClosureEffectStatusV1,
}

impl DecisionClosureRecordV1 {
    pub fn decision_id(&self) -> &DecisionIdV1 {
        &self.decision_id
    }

    pub const fn effect_status(&self) -> DecisionClosureEffectStatusV1 {
        self.effect_status
    }

    pub fn revision_ids(&self) -> &[super::DecisionRevisionIdV1] {
        &self.revision_ids
    }

    pub const fn terminal_status(&self) -> DecisionClosureTerminalStatusV1 {
        self.terminal_status
    }

    pub const fn resolution_id(&self) -> Option<&DecisionResolutionIdV1> {
        self.resolution_id.as_ref()
    }

    pub const fn supersession_id(&self) -> Option<&super::DecisionSupersessionIdV1> {
        self.supersession_id.as_ref()
    }

    pub fn materialization_ids(&self) -> &[crate::domain::identity::DecisionMaterializationIdV1] {
        &self.materialization_ids
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.decision_id.canonical_value(),
            CborValue::Array(
                self.revision_ids
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
            CborValue::Unsigned(self.terminal_status as u64),
            CborValue::optional(
                self.resolution_id
                    .as_ref()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec())),
            ),
            CborValue::optional(
                self.supersession_id
                    .as_ref()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec())),
            ),
            CborValue::Array(
                self.materialization_ids
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
            CborValue::Unsigned(self.effect_status as u64),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionClosureManifestV1 {
    work_id: WorkIdV1,
    decisions: Vec<DecisionClosureRecordV1>,
    lineage_edges: Vec<super::DecisionSupersessionV1>,
    materializations: Vec<DecisionMaterializationV1>,
    candidate_contract_root_id: ContractRootIdV1,
    closure_id: DecisionClosureIdV1,
}

impl DecisionClosureManifestV1 {
    pub fn close(
        work_id: WorkIdV1,
        mut expected_decision_ids: Vec<DecisionIdV1>,
        mut decisions: Vec<DecisionV1>,
        lineage: &DecisionLineageV1,
        mut materializations: Vec<DecisionMaterializationV1>,
        schemas: &SchemaClosureV1,
        initial_contract_root: &crate::domain::contract::root::CandidateContractRootV1,
    ) -> Result<Self, DecisionV1Error> {
        expected_decision_ids.sort();
        if expected_decision_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(DecisionV1Error::IncompleteDecisionClosure);
        }
        decisions.sort_by(|left, right| left.decision_id().cmp(right.decision_id()));
        let actual_ids: Vec<_> = decisions
            .iter()
            .map(|decision| decision.decision_id().clone())
            .collect();
        if expected_decision_ids != actual_ids
            || decisions
                .iter()
                .any(|decision| decision.work_id() != work_id)
            || lineage.work_id() != work_id
        {
            return Err(DecisionV1Error::IncompleteDecisionClosure);
        }
        validate_lineage(&decisions, lineage)?;

        materializations.sort_by_key(|value| *value.materialization_id());
        if materializations
            .windows(2)
            .any(|pair| pair[0].materialization_id() == pair[1].materialization_id())
        {
            return Err(DecisionV1Error::UnexpectedDecisionMaterialization);
        }
        let by_resolution = materializations_by_resolution(&materializations);
        let known_resolutions: BTreeSet<_> = decisions
            .iter()
            .filter_map(DecisionV1::resolution)
            .map(|resolution| *resolution.resolution_id())
            .collect();
        if by_resolution
            .keys()
            .any(|resolution_id| !known_resolutions.contains(resolution_id))
        {
            return Err(DecisionV1Error::UnexpectedDecisionMaterialization);
        }
        let candidate_contract_root_id = derive_candidate_root(
            initial_contract_root,
            schemas,
            &decisions,
            &materializations,
        )?;

        let mut records = Vec::with_capacity(decisions.len());
        for decision in &decisions {
            let revision_ids = decision
                .revisions()
                .iter()
                .map(|revision| *revision.revision_id())
                .collect();
            let (terminal_status, resolution_id, supersession_id, effect_status) =
                match decision.state() {
                    DecisionStateV1::Open => {
                        return Err(DecisionV1Error::DecisionClosureContainsOpenDecision);
                    }
                    DecisionStateV1::Withdrawn(_) => (
                        DecisionClosureTerminalStatusV1::Withdrawn,
                        None,
                        None,
                        DecisionClosureEffectStatusV1::Withdrawn,
                    ),
                    DecisionStateV1::Resolved(resolution) => {
                        let materialized = by_resolution
                            .get(resolution.resolution_id())
                            .cloned()
                            .unwrap_or_default();
                        let effect = resolved_effect_status(
                            resolution.selected_consequence(),
                            &materialized,
                            &candidate_contract_root_id,
                            false,
                        )?;
                        (
                            DecisionClosureTerminalStatusV1::Resolved,
                            Some(*resolution.resolution_id()),
                            None,
                            effect,
                        )
                    }
                    DecisionStateV1::Superseded {
                        resolution,
                        supersession,
                    } => {
                        let materialized = by_resolution
                            .get(resolution.resolution_id())
                            .cloned()
                            .unwrap_or_default();
                        let effect = resolved_effect_status(
                            resolution.selected_consequence(),
                            &materialized,
                            &candidate_contract_root_id,
                            true,
                        )?;
                        (
                            DecisionClosureTerminalStatusV1::Superseded,
                            Some(*resolution.resolution_id()),
                            Some(*supersession.supersession_id()),
                            effect,
                        )
                    }
                };
            let mut materialization_ids = resolution_id
                .as_ref()
                .and_then(|id| by_resolution.get(id))
                .map(|rows| {
                    rows.iter()
                        .map(|row| *row.materialization_id())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            materialization_ids.sort_unstable();
            records.push(DecisionClosureRecordV1 {
                decision_id: decision.decision_id().clone(),
                revision_ids,
                terminal_status,
                resolution_id,
                supersession_id,
                materialization_ids,
                effect_status,
            });
        }
        let lineage_edges = lineage.edges().to_vec();
        let value = closure_value(
            work_id,
            &records,
            &lineage_edges,
            &materializations,
            &candidate_contract_root_id,
        );
        let closure_id = decision_closure_identity(&value)?;
        Ok(Self {
            work_id,
            decisions: records,
            lineage_edges,
            materializations,
            candidate_contract_root_id,
            closure_id,
        })
    }

    pub fn decisions(&self) -> &[DecisionClosureRecordV1] {
        &self.decisions
    }

    pub fn materializations(&self) -> &[DecisionMaterializationV1] {
        &self.materializations
    }

    pub fn effect_status(
        &self,
        decision_id: &DecisionIdV1,
    ) -> Option<DecisionClosureEffectStatusV1> {
        self.decisions
            .iter()
            .find(|record| record.decision_id() == decision_id)
            .map(DecisionClosureRecordV1::effect_status)
    }

    pub fn closure_id(&self) -> &DecisionClosureIdV1 {
        &self.closure_id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn lineage_edges(&self) -> &[super::DecisionSupersessionV1] {
        &self.lineage_edges
    }

    pub const fn candidate_contract_root_id(&self) -> &ContractRootIdV1 {
        &self.candidate_contract_root_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DecisionV1Error> {
        Ok(deterministic_cbor::encode(&closure_value(
            self.work_id,
            &self.decisions,
            &self.lineage_edges,
            &self.materializations,
            &self.candidate_contract_root_id,
        ))?)
    }
}

fn derive_candidate_root(
    initial_root: &crate::domain::contract::root::CandidateContractRootV1,
    schemas: &SchemaClosureV1,
    decisions: &[DecisionV1],
    materializations: &[DecisionMaterializationV1],
) -> Result<ContractRootIdV1, DecisionV1Error> {
    let resolutions = decisions
        .iter()
        .filter_map(DecisionV1::resolution)
        .map(|resolution| (*resolution.resolution_id(), resolution))
        .collect::<BTreeMap<_, _>>();
    for materialization in materializations {
        let resolution = resolutions
            .get(materialization.resolution_id())
            .ok_or(DecisionV1Error::UnexpectedDecisionMaterialization)?;
        materialization
            .verify_exact_transformation(resolution, schemas)
            .map_err(|_| DecisionV1Error::DecisionMaterializationRootJoinMismatch)?;
    }

    let mut current = initial_root.clone();
    let mut remaining = materializations.iter().collect::<Vec<_>>();
    loop {
        let mut connected = Vec::new();
        let mut disconnected = Vec::new();
        for materialization in remaining {
            if roots_equal_exact(materialization.base_root(), &current)? {
                connected.push(materialization);
            } else {
                disconnected.push(materialization);
            }
        }
        let mut advancing = connected
            .iter()
            .copied()
            .filter(|materialization| {
                matches!(
                    materialization.disposition(),
                    DecisionMaterializationDispositionV1::Changed { .. }
                )
            })
            .collect::<Vec<_>>();
        if advancing.len() > 1 {
            return Err(DecisionV1Error::DecisionMaterializationRootJoinMismatch);
        }
        if let Some(materialization) = advancing.pop() {
            current = materialization.candidate_root().clone();
            remaining = disconnected;
            continue;
        }
        if disconnected.is_empty() {
            break;
        }
        return Err(DecisionV1Error::DecisionMaterializationRootJoinMismatch);
    }
    Ok(*current.root_id())
}

fn roots_equal_exact(
    left: &crate::domain::contract::root::CandidateContractRootV1,
    right: &crate::domain::contract::root::CandidateContractRootV1,
) -> Result<bool, DecisionV1Error> {
    let left = left
        .canonical_bytes()
        .map_err(|_| DecisionV1Error::DecisionMaterializationRootJoinMismatch)?;
    let right = right
        .canonical_bytes()
        .map_err(|_| DecisionV1Error::DecisionMaterializationRootJoinMismatch)?;
    Ok(left == right)
}

fn validate_lineage(
    decisions: &[DecisionV1],
    lineage: &DecisionLineageV1,
) -> Result<(), DecisionV1Error> {
    let by_id: BTreeMap<_, _> = decisions
        .iter()
        .map(|decision| (decision.decision_id(), decision))
        .collect();
    for edge in lineage.edges() {
        let Some(predecessor) = by_id.get(edge.predecessor_id()) else {
            return Err(DecisionV1Error::DecisionLineageMismatch);
        };
        let Some(successor) = by_id.get(edge.successor_id()) else {
            return Err(DecisionV1Error::DecisionLineageMismatch);
        };
        if !matches!(
            predecessor.state(),
            DecisionStateV1::Superseded { supersession, .. }
                if supersession.supersession_id() == edge.supersession_id()
        ) || !matches!(successor.state(), DecisionStateV1::Resolved(_))
        {
            return Err(DecisionV1Error::DecisionLineageMismatch);
        }
    }
    let superseded_count = decisions
        .iter()
        .filter(|decision| matches!(decision.state(), DecisionStateV1::Superseded { .. }))
        .count();
    if superseded_count != lineage.edges().len() {
        return Err(DecisionV1Error::DecisionLineageMismatch);
    }
    Ok(())
}

fn materializations_by_resolution(
    materializations: &[DecisionMaterializationV1],
) -> BTreeMap<DecisionResolutionIdV1, Vec<&DecisionMaterializationV1>> {
    let mut by_resolution = BTreeMap::new();
    for materialization in materializations {
        by_resolution
            .entry(*materialization.resolution_id())
            .or_insert_with(Vec::new)
            .push(materialization);
    }
    by_resolution
}

fn resolved_effect_status(
    consequence: &AlternativeConsequenceV1,
    materializations: &[&DecisionMaterializationV1],
    candidate_root_id: &ContractRootIdV1,
    superseded: bool,
) -> Result<DecisionClosureEffectStatusV1, DecisionV1Error> {
    match consequence {
        AlternativeConsequenceV1::NoContractEffect => {
            if !materializations.is_empty() {
                return Err(DecisionV1Error::UnexpectedDecisionMaterialization);
            }
            Ok(DecisionClosureEffectStatusV1::NoContractEffect)
        }
        AlternativeConsequenceV1::TypedConsequencePlan { .. } => {
            if materializations.is_empty() {
                return Err(DecisionV1Error::UnappliedNormativeDecision);
            }
            let exact_effect_live = materializations
                .iter()
                .any(|row| row.candidate_root_id() == candidate_root_id);
            let equivalent_effect_live = materializations.iter().any(|row| {
                matches!(
                    row.disposition(),
                    DecisionMaterializationDispositionV1::NoOpEquivalent { .. }
                ) && row.base_root_id() == candidate_root_id
            });
            Ok(
                if (exact_effect_live || equivalent_effect_live) && superseded {
                    DecisionClosureEffectStatusV1::SupersededButEffectLive
                } else if exact_effect_live {
                    DecisionClosureEffectStatusV1::AppliedExactRoot
                } else if equivalent_effect_live {
                    DecisionClosureEffectStatusV1::AppliedEquivalentRoot
                } else {
                    DecisionClosureEffectStatusV1::Historical
                },
            )
        }
    }
}

fn closure_value(
    work_id: WorkIdV1,
    records: &[DecisionClosureRecordV1],
    lineage: &[super::DecisionSupersessionV1],
    materializations: &[DecisionMaterializationV1],
    candidate_root_id: &ContractRootIdV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DECISION_CLOSURE_MANIFEST_VERSION_V1),
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::Array(
            records
                .iter()
                .map(DecisionClosureRecordV1::canonical_value)
                .collect(),
        ),
        CborValue::Array(
            lineage
                .iter()
                .map(|edge| CborValue::Bytes(edge.supersession_id().as_bytes().to_vec()))
                .collect(),
        ),
        CborValue::Array(
            materializations
                .iter()
                .map(|row| CborValue::Bytes(row.materialization_id().as_bytes().to_vec()))
                .collect(),
        ),
        CborValue::Bytes(candidate_root_id.as_bytes().to_vec()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applied_equivalent_root_has_additive_canonical_tag_six() {
        let record = DecisionClosureRecordV1 {
            decision_id: DecisionIdV1::new("eq").expect("Decision id"),
            revision_ids: vec![],
            terminal_status: DecisionClosureTerminalStatusV1::Resolved,
            resolution_id: None,
            supersession_id: None,
            materialization_ids: vec![],
            effect_status: DecisionClosureEffectStatusV1::AppliedEquivalentRoot,
        };

        assert_eq!(
            DecisionClosureEffectStatusV1::AppliedEquivalentRoot as u64,
            6
        );
        assert_eq!(
            deterministic_cbor::encode(&record.canonical_value())
                .expect("Decision closure record CBOR"),
            vec![
                0x87, 0x62, b'e', b'q', 0x80, 0x01, 0x81, 0x00, 0x81, 0x00, 0x80, 0x06,
            ]
        );
    }
}
