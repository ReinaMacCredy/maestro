use std::collections::BTreeSet;

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::{StepIdentityError, StepRevisionIdV1, domain_hash, require_nonzero};

const STEP_REVISION_DOMAIN_V1: &str = "maestro.vnext.step-revision.v1";
const STEP_REVISION_VERSION_V1: u64 = 1;
const MAX_ADDITIONAL_MATERIAL_CONSTRAINTS_V1: usize = 1_024;
const MAX_MATERIAL_CONSTRAINT_NAME_BYTES_V1: usize = 128;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NamedMaterialConstraintV1 {
    name: String,
    commitment: [u8; 32],
}

impl NamedMaterialConstraintV1 {
    pub fn new(name: impl Into<String>, commitment: [u8; 32]) -> Result<Self, StepRevisionError> {
        let name = name.into();
        if name.is_empty() || !name.is_ascii() {
            return Err(StepRevisionError::InvalidMaterialConstraintName);
        }
        if name.len() > MAX_MATERIAL_CONSTRAINT_NAME_BYTES_V1 {
            return Err(StepRevisionError::MaterialConstraintNameTooLong);
        }
        require_nonzero(commitment, "additional Step material constraint")?;
        Ok(Self { name, commitment })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn commitment(&self) -> &[u8; 32] {
        &self.commitment
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Text(self.name.clone()),
            CborValue::Bytes(self.commitment.to_vec()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepRevisionMaterialV1 {
    outcome: [u8; 32],
    claim_schema: [u8; 32],
    inputs: [u8; 32],
    outputs: [u8; 32],
    action_scope: [u8; 32],
    effect_scope: [u8; 32],
    resource_requirements: [u8; 32],
    budget_requirements: [u8; 32],
    gate_requirements: [u8; 32],
    authority_requirements: [u8; 32],
    environment_constraints: [u8; 32],
    target_constraints: [u8; 32],
    execution_constraints: [u8; 32],
    completion_constraints: [u8; 32],
    additional_material_constraints: Vec<NamedMaterialConstraintV1>,
}

impl StepRevisionMaterialV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the frozen Step Revision schema keeps every material execution and completion commitment explicit"
    )]
    pub fn new(
        outcome: [u8; 32],
        claim_schema: [u8; 32],
        inputs: [u8; 32],
        outputs: [u8; 32],
        action_scope: [u8; 32],
        effect_scope: [u8; 32],
        resource_requirements: [u8; 32],
        budget_requirements: [u8; 32],
        gate_requirements: [u8; 32],
        authority_requirements: [u8; 32],
        environment_constraints: [u8; 32],
        target_constraints: [u8; 32],
        execution_constraints: [u8; 32],
        completion_constraints: [u8; 32],
        mut additional_material_constraints: Vec<NamedMaterialConstraintV1>,
    ) -> Result<Self, StepRevisionError> {
        if additional_material_constraints.len() > MAX_ADDITIONAL_MATERIAL_CONSTRAINTS_V1 {
            return Err(StepRevisionError::TooManyAdditionalMaterialConstraints);
        }
        for (label, commitment) in [
            ("Step outcome", outcome),
            ("Step Claim schema", claim_schema),
            ("Step inputs", inputs),
            ("Step outputs", outputs),
            ("Step Action scope", action_scope),
            ("Step effect scope", effect_scope),
            ("Step resource requirements", resource_requirements),
            ("Step budget requirements", budget_requirements),
            ("Step Gate requirements", gate_requirements),
            ("Step authority requirements", authority_requirements),
            ("Step environment constraints", environment_constraints),
            ("Step target constraints", target_constraints),
            ("Step execution constraints", execution_constraints),
            ("Step completion constraints", completion_constraints),
        ] {
            require_nonzero(commitment, label)?;
        }

        additional_material_constraints.sort();
        let mut names = BTreeSet::new();
        for constraint in &additional_material_constraints {
            if !names.insert(constraint.name()) {
                return Err(StepRevisionError::DuplicateMaterialConstraintName);
            }
        }

        Ok(Self {
            outcome,
            claim_schema,
            inputs,
            outputs,
            action_scope,
            effect_scope,
            resource_requirements,
            budget_requirements,
            gate_requirements,
            authority_requirements,
            environment_constraints,
            target_constraints,
            execution_constraints,
            completion_constraints,
            additional_material_constraints,
        })
    }

    pub fn additional_material_constraints(&self) -> &[NamedMaterialConstraintV1] {
        &self.additional_material_constraints
    }

    fn canonical_value(&self) -> CborValue {
        let mut values = vec![CborValue::Unsigned(STEP_REVISION_VERSION_V1)];
        values.extend(
            [
                self.outcome,
                self.claim_schema,
                self.inputs,
                self.outputs,
                self.action_scope,
                self.effect_scope,
                self.resource_requirements,
                self.budget_requirements,
                self.gate_requirements,
                self.authority_requirements,
                self.environment_constraints,
                self.target_constraints,
                self.execution_constraints,
                self.completion_constraints,
            ]
            .into_iter()
            .map(|commitment| CborValue::Bytes(commitment.to_vec())),
        );
        values.push(CborValue::Array(
            self.additional_material_constraints
                .iter()
                .map(NamedMaterialConstraintV1::canonical_value)
                .collect(),
        ));
        CborValue::Array(values)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepRevisionV1 {
    id: StepRevisionIdV1,
    material: StepRevisionMaterialV1,
}

impl StepRevisionV1 {
    pub fn new(material: StepRevisionMaterialV1) -> Result<Self, StepRevisionError> {
        let id = StepRevisionIdV1::from_bytes(domain_hash(
            STEP_REVISION_DOMAIN_V1,
            &material.canonical_value(),
        )?)?;
        Ok(Self { id, material })
    }

    pub fn id(&self) -> StepRevisionIdV1 {
        self.id
    }

    pub fn material(&self) -> &StepRevisionMaterialV1 {
        &self.material
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StepRevisionError> {
        Ok(deterministic_cbor::encode(
            &self.material.canonical_value(),
        )?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepRevisionError {
    #[error(transparent)]
    Identity(#[from] StepIdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Step material constraint name must be non-empty canonical ASCII")]
    InvalidMaterialConstraintName,
    #[error(
        "Step material constraint name exceeds the finite v1 limit of {MAX_MATERIAL_CONSTRAINT_NAME_BYTES_V1} bytes"
    )]
    MaterialConstraintNameTooLong,
    #[error(
        "Step Revision exceeds the finite v1 limit of {MAX_ADDITIONAL_MATERIAL_CONSTRAINTS_V1} additional material constraints"
    )]
    TooManyAdditionalMaterialConstraints,
    #[error("Step Revision contains a duplicate additional material constraint name")]
    DuplicateMaterialConstraintName,
}
