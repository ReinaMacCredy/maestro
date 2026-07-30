use std::collections::BTreeMap;

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::graph::{StepBindingV1, StepGraphError, StepGraphSnapshotV1};
use super::identity::{StepIdentityError, require_nonzero};
use super::lifecycle::{StepLifecycleError, StepLifecycleKindV1, StepStateV1};

const STEP_AMENDMENT_PLAN_VERSION_V1: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainExactStepV1 {
    old: StepBindingV1,
    new: StepBindingV1,
}

impl RetainExactStepV1 {
    pub fn old(&self) -> StepBindingV1 {
        self.old
    }

    pub fn new_binding(&self) -> StepBindingV1 {
        self.new
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![self.old.canonical_value(), self.new.canonical_value()])
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReplacedStepV1 {
    old: StepBindingV1,
    replacement: StepBindingV1,
}

impl ReplacedStepV1 {
    pub fn old(&self) -> StepBindingV1 {
        self.old
    }

    pub fn replacement(&self) -> StepBindingV1 {
        self.replacement
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.old.canonical_value(),
            self.replacement.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepAmendmentPlanV1 {
    current_graph_id: [u8; 32],
    candidate_graph_id: [u8; 32],
    retain_exact: Vec<RetainExactStepV1>,
    replace: Vec<ReplacedStepV1>,
    remove: Vec<StepBindingV1>,
    add: Vec<StepBindingV1>,
}

impl StepAmendmentPlanV1 {
    pub fn retain_exact(&self) -> &[RetainExactStepV1] {
        &self.retain_exact
    }

    pub fn replace(&self) -> &[ReplacedStepV1] {
        &self.replace
    }

    pub fn remove(&self) -> &[StepBindingV1] {
        &self.remove
    }

    pub fn add(&self) -> &[StepBindingV1] {
        &self.add
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StepAmendmentError> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(STEP_AMENDMENT_PLAN_VERSION_V1),
            CborValue::Bytes(self.current_graph_id.to_vec()),
            CborValue::Bytes(self.candidate_graph_id.to_vec()),
            CborValue::Array(
                self.retain_exact
                    .iter()
                    .map(RetainExactStepV1::canonical_value)
                    .collect(),
            ),
            CborValue::Array(
                self.replace
                    .iter()
                    .map(ReplacedStepV1::canonical_value)
                    .collect(),
            ),
            CborValue::Array(
                self.remove
                    .iter()
                    .map(StepBindingV1::canonical_value)
                    .collect(),
            ),
            CborValue::Array(
                self.add
                    .iter()
                    .map(StepBindingV1::canonical_value)
                    .collect(),
            ),
        ]))?)
    }

    pub fn apply(
        &self,
        current: &StepGraphSnapshotV1,
        candidate: &StepGraphSnapshotV1,
        current_states: &[StepStateV1],
        amendment_receipt_hash: [u8; 32],
    ) -> Result<AppliedStepAmendmentV1, StepAmendmentError> {
        require_nonzero(amendment_receipt_hash, "Contract amendment receipt")?;
        if self != &plan_step_amendment_v1(current, candidate)? {
            return Err(StepAmendmentError::PlanGraphMismatch);
        }

        let mut states_by_binding = BTreeMap::new();
        for state in current_states {
            if states_by_binding.insert(state.binding(), *state).is_some() {
                return Err(StepAmendmentError::CurrentStateSetMismatch);
            }
        }
        if states_by_binding.len() != current.nodes().len()
            || current
                .nodes()
                .iter()
                .any(|node| !states_by_binding.contains_key(&node.binding()))
        {
            return Err(StepAmendmentError::CurrentStateSetMismatch);
        }
        let mut retain_exact = Vec::with_capacity(self.retain_exact.len());
        for retained in &self.retain_exact {
            let prior_state = states_by_binding
                .get(&retained.old)
                .copied()
                .ok_or(StepAmendmentError::CurrentStateSetMismatch)?;
            let initialization = initialize_retain_exact_v1(&prior_state, retained.new)?;
            retain_exact.push(RetainExactDispositionV1 {
                prior_state,
                historical_state: prior_state,
                next_state: StepPublicationStateV1::new(retained.new, initialization),
            });
        }

        let mut replace = Vec::with_capacity(self.replace.len());
        for replaced in &self.replace {
            let prior_state = states_by_binding
                .get(&replaced.old)
                .copied()
                .ok_or(StepAmendmentError::CurrentStateSetMismatch)?;
            let historical_state = match prior_state.lifecycle().kind() {
                StepLifecycleKindV1::Open | StepLifecycleKindV1::Submitted => {
                    prior_state.supersede(replaced.replacement, amendment_receipt_hash)?
                }
                StepLifecycleKindV1::Satisfied
                | StepLifecycleKindV1::Cancelled
                | StepLifecycleKindV1::Superseded => prior_state,
            };
            replace.push(ReplaceDispositionV1 {
                prior_state,
                historical_state,
                next_state: StepPublicationStateV1::new(
                    replaced.replacement,
                    RetainExactInitializationV1::OpenFreshV1,
                ),
            });
        }

        let mut remove = Vec::with_capacity(self.remove.len());
        for removed in &self.remove {
            let prior_state = states_by_binding
                .get(removed)
                .copied()
                .ok_or(StepAmendmentError::CurrentStateSetMismatch)?;
            let historical_state = match prior_state.lifecycle().kind() {
                StepLifecycleKindV1::Open | StepLifecycleKindV1::Submitted => {
                    prior_state.cancel(amendment_receipt_hash)?
                }
                StepLifecycleKindV1::Satisfied
                | StepLifecycleKindV1::Cancelled
                | StepLifecycleKindV1::Superseded => prior_state,
            };
            remove.push(RemoveDispositionV1 {
                prior_state,
                historical_state,
            });
        }

        let add = self
            .add
            .iter()
            .copied()
            .map(|binding| AddDispositionV1 {
                next_state: StepPublicationStateV1::new(
                    binding,
                    RetainExactInitializationV1::OpenFreshV1,
                ),
            })
            .collect();
        let obligation_conservation = StepObligationConservationV1 {
            current_obligation_count: current.nodes().len(),
            candidate_obligation_count: candidate.nodes().len(),
            retain_exact_count: self.retain_exact.len(),
            replace_count: self.replace.len(),
            remove_count: self.remove.len(),
            add_count: self.add.len(),
        };
        if obligation_conservation.current_partition_count()
            != obligation_conservation.current_obligation_count
            || obligation_conservation.candidate_partition_count()
                != obligation_conservation.candidate_obligation_count
        {
            return Err(StepAmendmentError::ObligationConservationFailure);
        }
        Ok(AppliedStepAmendmentV1 {
            retain_exact,
            replace,
            remove,
            add,
            obligation_conservation,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepPublicationStateV1 {
    binding: StepBindingV1,
    initialization: RetainExactInitializationV1,
}

impl StepPublicationStateV1 {
    fn new(binding: StepBindingV1, initialization: RetainExactInitializationV1) -> Self {
        Self {
            binding,
            initialization,
        }
    }

    pub fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub fn initialization(&self) -> RetainExactInitializationV1 {
        self.initialization
    }

    pub fn materialize(&self) -> Result<StepStateV1, StepLifecycleError> {
        match self.initialization {
            RetainExactInitializationV1::OpenFreshV1 => Ok(StepStateV1::new_open(self.binding)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainExactDispositionV1 {
    prior_state: StepStateV1,
    historical_state: StepStateV1,
    next_state: StepPublicationStateV1,
}

impl RetainExactDispositionV1 {
    pub fn prior_state(&self) -> StepStateV1 {
        self.prior_state
    }

    pub fn historical_state(&self) -> StepStateV1 {
        self.historical_state
    }

    pub fn next_state(&self) -> StepPublicationStateV1 {
        self.next_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplaceDispositionV1 {
    prior_state: StepStateV1,
    historical_state: StepStateV1,
    next_state: StepPublicationStateV1,
}

impl ReplaceDispositionV1 {
    pub fn prior_state(&self) -> StepStateV1 {
        self.prior_state
    }

    pub fn historical_state(&self) -> StepStateV1 {
        self.historical_state
    }

    pub fn next_state(&self) -> StepPublicationStateV1 {
        self.next_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveDispositionV1 {
    prior_state: StepStateV1,
    historical_state: StepStateV1,
}

impl RemoveDispositionV1 {
    pub fn prior_state(&self) -> StepStateV1 {
        self.prior_state
    }

    pub fn historical_state(&self) -> StepStateV1 {
        self.historical_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddDispositionV1 {
    next_state: StepPublicationStateV1,
}

impl AddDispositionV1 {
    pub fn next_state(&self) -> StepPublicationStateV1 {
        self.next_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepObligationConservationV1 {
    current_obligation_count: usize,
    candidate_obligation_count: usize,
    retain_exact_count: usize,
    replace_count: usize,
    remove_count: usize,
    add_count: usize,
}

impl StepObligationConservationV1 {
    pub fn current_obligation_count(&self) -> usize {
        self.current_obligation_count
    }

    pub fn candidate_obligation_count(&self) -> usize {
        self.candidate_obligation_count
    }

    pub fn retain_exact_count(&self) -> usize {
        self.retain_exact_count
    }

    pub fn replace_count(&self) -> usize {
        self.replace_count
    }

    pub fn remove_count(&self) -> usize {
        self.remove_count
    }

    pub fn add_count(&self) -> usize {
        self.add_count
    }

    pub fn current_partition_count(&self) -> usize {
        self.retain_exact_count + self.replace_count + self.remove_count
    }

    pub fn candidate_partition_count(&self) -> usize {
        self.retain_exact_count + self.replace_count + self.add_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedStepAmendmentV1 {
    retain_exact: Vec<RetainExactDispositionV1>,
    replace: Vec<ReplaceDispositionV1>,
    remove: Vec<RemoveDispositionV1>,
    add: Vec<AddDispositionV1>,
    obligation_conservation: StepObligationConservationV1,
}

impl AppliedStepAmendmentV1 {
    pub fn retain_exact(&self) -> &[RetainExactDispositionV1] {
        &self.retain_exact
    }

    pub fn replace(&self) -> &[ReplaceDispositionV1] {
        &self.replace
    }

    pub fn remove(&self) -> &[RemoveDispositionV1] {
        &self.remove
    }

    pub fn add(&self) -> &[AddDispositionV1] {
        &self.add
    }

    pub fn obligation_conservation(&self) -> StepObligationConservationV1 {
        self.obligation_conservation
    }
}

pub fn plan_step_amendment_v1(
    current: &StepGraphSnapshotV1,
    candidate: &StepGraphSnapshotV1,
) -> Result<StepAmendmentPlanV1, StepAmendmentError> {
    if current.scope() != candidate.scope() {
        return Err(StepAmendmentError::CrossWork);
    }
    if current.contract_root_id() == candidate.contract_root_id() {
        return Err(StepAmendmentError::ContractRootNotAdvanced);
    }
    if current.contract_generation_id() == candidate.contract_generation_id() {
        return Err(StepAmendmentError::ContractGenerationNotAdvanced);
    }

    let current_by_id: BTreeMap<_, _> = current
        .nodes()
        .iter()
        .map(|node| (node.binding().step_id(), node.binding()))
        .collect();
    let candidate_by_id: BTreeMap<_, _> = candidate
        .nodes()
        .iter()
        .map(|node| (node.binding().step_id(), node.binding()))
        .collect();

    let mut retain_exact = Vec::new();
    let mut replace = Vec::new();
    let mut remove = Vec::new();
    let mut add = Vec::new();

    for (step_id, old) in &current_by_id {
        match candidate_by_id.get(step_id) {
            Some(new) if old.revision_id() == new.revision_id() => {
                retain_exact.push(RetainExactStepV1 {
                    old: *old,
                    new: *new,
                });
            }
            Some(new) => replace.push(ReplacedStepV1 {
                old: *old,
                replacement: *new,
            }),
            None => remove.push(*old),
        }
    }
    for (step_id, new) in &candidate_by_id {
        if !current_by_id.contains_key(step_id) {
            add.push(*new);
        }
    }

    Ok(StepAmendmentPlanV1 {
        current_graph_id: current.id().into_bytes(),
        candidate_graph_id: candidate.id().into_bytes(),
        retain_exact,
        replace,
        remove,
        add,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainExactInitializationV1 {
    OpenFreshV1,
}

pub fn initialize_retain_exact_v1(
    old_state: &StepStateV1,
    new_binding: StepBindingV1,
) -> Result<RetainExactInitializationV1, StepAmendmentError> {
    validate_retain_exact_pair(old_state.binding(), new_binding)?;
    Ok(RetainExactInitializationV1::OpenFreshV1)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepAmendmentError {
    #[error(transparent)]
    Identity(#[from] StepIdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Lifecycle(#[from] StepLifecycleError),
    #[error(transparent)]
    Graph(#[from] StepGraphError),
    #[error("Step amendment candidate belongs to a different repository or Work")]
    CrossWork,
    #[error("Step amendment candidate must bind a distinct Contract Root")]
    ContractRootNotAdvanced,
    #[error("Step amendment candidate must bind a distinct Contract Generation")]
    ContractGenerationNotAdvanced,
    #[error("Step amendment plan does not exactly match the supplied current and candidate graphs")]
    PlanGraphMismatch,
    #[error("Step amendment requires exactly one current state for every current graph binding")]
    CurrentStateSetMismatch,
    #[error("Step amendment disposition does not conserve current and candidate obligations")]
    ObligationConservationFailure,
    #[error(
        "retain_exact requires the same stable Step and exact Step Revision in distinct Contract Generations and roots"
    )]
    NotRetainExact,
}

fn validate_retain_exact_pair(
    old: StepBindingV1,
    new: StepBindingV1,
) -> Result<(), StepAmendmentError> {
    if old == new
        || old.scope() != new.scope()
        || old.step_id() != new.step_id()
        || old.revision_id() != new.revision_id()
        || old.contract_generation_id() == new.contract_generation_id()
        || old.contract_root_id() == new.contract_root_id()
    {
        Err(StepAmendmentError::NotRetainExact)
    } else {
        Ok(())
    }
}
