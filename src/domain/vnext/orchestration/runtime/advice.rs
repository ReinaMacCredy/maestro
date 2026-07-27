use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::integration::public_literals::{
    PacketRecipeAdviceOutcomeV1, PacketRecipeAdviceProvenanceV1, PacketRecipeBindingV1,
    PacketRecipeComponentProvenanceV1, PacketRecipeComponentSlotV1, PublicLiteralError,
};
use crate::domain::vnext::orchestration::literals::{
    ExactRecipeSelectionV1, RecipeApplicationV1, RecipeComponentEvaluationV1,
    RecipeComponentOutcomeTagV1, RecipeComponentSlotV1, RecipeLiteralError,
    RecipeReturnComponentV1, RecipeReturnOccurrenceV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::catalog::{FrozenRecipeCatalogErrorV1, FrozenRecipeCatalogV1};
use super::continuation::BoundedContinuationLimitV1;

const MAX_FRONTIER_MEMBERS_V1: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionFrontierViewV1 {
    pub(crate) resolution_basis_ref: String,
    pub(crate) frontier_ref: String,
    pub(crate) eligible_action_refs: Vec<String>,
    pub(crate) complete_wave_refs: Vec<String>,
    pub(crate) material_dependency_hash: [u8; 32],
}

impl ActionFrontierViewV1 {
    pub(crate) fn validate(&self) -> Result<(), RecipeRuntimeErrorV1> {
        if self.resolution_basis_ref.is_empty()
            || self.frontier_ref.is_empty()
            || self.material_dependency_hash == [0; 32]
            || self.eligible_action_refs.len() > MAX_FRONTIER_MEMBERS_V1
            || self.complete_wave_refs.len() > MAX_FRONTIER_MEMBERS_V1
            || !strictly_ordered_unique(&self.eligible_action_refs)
            || !strictly_ordered_unique(&self.complete_wave_refs)
        {
            return Err(RecipeRuntimeErrorV1::InvalidFrontier);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecipeComponentAdviceV1 {
    pub(crate) evaluation: RecipeComponentEvaluationV1,
    pub(crate) allowed_action_refs: Vec<String>,
    pub(crate) ordered_action_refs: Vec<String>,
    pub(crate) preferred_complete_wave_ref: Option<String>,
    pub(crate) continuation_limits: Vec<BoundedContinuationLimitV1>,
}

impl RecipeComponentAdviceV1 {
    pub(crate) fn validate(&self) -> Result<(), RecipeRuntimeErrorV1> {
        self.evaluation.validate()?;
        if !strictly_ordered_unique(&self.allowed_action_refs)
            || !strictly_ordered_unique(&self.ordered_action_refs)
            || !self
                .ordered_action_refs
                .iter()
                .all(|value| self.allowed_action_refs.contains(value))
            || self
                .continuation_limits
                .windows(2)
                .any(|pair| pair[0].kind() >= pair[1].kind())
        {
            return Err(RecipeRuntimeErrorV1::InvalidComponentAdvice);
        }
        match &self.evaluation {
            RecipeComponentEvaluationV1::NotApplicable(_) => {
                if !self.allowed_action_refs.is_empty()
                    || !self.ordered_action_refs.is_empty()
                    || self.preferred_complete_wave_ref.is_some()
                    || !self.continuation_limits.is_empty()
                {
                    return Err(RecipeRuntimeErrorV1::InvalidComponentAdvice);
                }
            }
            RecipeComponentEvaluationV1::HardStop { .. } => {
                if !self.allowed_action_refs.is_empty()
                    || !self.ordered_action_refs.is_empty()
                    || self.preferred_complete_wave_ref.is_some()
                    || !self.continuation_limits.is_empty()
                {
                    return Err(RecipeRuntimeErrorV1::InvalidComponentAdvice);
                }
            }
            RecipeComponentEvaluationV1::RestrictiveAdvice { occurrence, .. } => {
                match occurrence.component.slot() {
                    RecipeComponentSlotV1::Primary => {
                        if self.allowed_action_refs.is_empty()
                            || !self.continuation_limits.is_empty()
                        {
                            return Err(RecipeRuntimeErrorV1::InvalidComponentAdvice);
                        }
                    }
                    RecipeComponentSlotV1::Continuation => {
                        if !self.allowed_action_refs.is_empty()
                            || !self.ordered_action_refs.is_empty()
                            || self.preferred_complete_wave_ref.is_some()
                            || self.continuation_limits.is_empty()
                        {
                            return Err(RecipeRuntimeErrorV1::InvalidComponentAdvice);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn occurrence(&self) -> &RecipeReturnOccurrenceV1 {
        match &self.evaluation {
            RecipeComponentEvaluationV1::NotApplicable(occurrence)
            | RecipeComponentEvaluationV1::RestrictiveAdvice { occurrence, .. }
            | RecipeComponentEvaluationV1::HardStop { occurrence, .. } => occurrence,
        }
    }

    fn output_hash(&self) -> Result<[u8; 32], RecipeRuntimeErrorV1> {
        let occurrence = self.occurrence();
        let evaluation_ref = match &self.evaluation {
            RecipeComponentEvaluationV1::NotApplicable(_) => {
                CborValue::Array(vec![CborValue::Unsigned(1)])
            }
            RecipeComponentEvaluationV1::RestrictiveAdvice {
                recipe_advice_ref, ..
            } => CborValue::Array(vec![CborValue::Unsigned(2), text(recipe_advice_ref)]),
            RecipeComponentEvaluationV1::HardStop {
                recipe_hard_stop_ref,
                ..
            } => CborValue::Array(vec![CborValue::Unsigned(3), text(recipe_hard_stop_ref)]),
        };
        Ok(domain_hash(
            "maestro.vnext.recipe-component-advice-output.v1",
            &CborValue::Array(vec![
                occurrence_value(occurrence),
                evaluation_ref,
                text_array(&self.allowed_action_refs),
                text_array(&self.ordered_action_refs),
                optional_text(self.preferred_complete_wave_ref.as_deref()),
                CborValue::Array(
                    self.continuation_limits
                        .iter()
                        .map(BoundedContinuationLimitV1::canonical_value)
                        .collect(),
                ),
            ]),
        )?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecipeEvaluationInputV1 {
    pub(crate) application: RecipeApplicationV1,
    pub(crate) components: Vec<RecipeComponentAdviceV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComposedRecipeAdviceV1 {
    pub(crate) recipe_binding: PacketRecipeBindingV1,
    pub(crate) composition_outcome: PacketRecipeAdviceOutcomeV1,
    pub(crate) admissible_action_refs: Vec<String>,
    pub(crate) ordered_action_refs: Vec<String>,
    pub(crate) preferred_complete_wave_ref: Option<String>,
    pub(crate) continuation_limits: Vec<BoundedContinuationLimitV1>,
}

impl ComposedRecipeAdviceV1 {
    pub(crate) fn is_actionable(&self) -> bool {
        self.recipe_binding.is_actionable()
    }
}

pub(crate) fn evaluate_recipe_application(
    catalog: &FrozenRecipeCatalogV1,
    frontier: &ActionFrontierViewV1,
    input: RecipeEvaluationInputV1,
) -> Result<ComposedRecipeAdviceV1, RecipeRuntimeErrorV1> {
    frontier.validate()?;
    input.application.validate()?;
    if input.application.resolution_basis_ref != frontier.resolution_basis_ref
        || input.application.frontier_ref != frontier.frontier_ref
    {
        return Err(RecipeRuntimeErrorV1::MixedFrontier);
    }
    validate_exact_catalog_application(catalog, &input.application)?;
    let expected_slots = expected_slots(&input.application);
    if input.components.len() != expected_slots.len() || input.components.len() > 2 {
        return Err(RecipeRuntimeErrorV1::ComponentSetMismatch);
    }
    let mut component_provenance = Vec::with_capacity(input.components.len());
    for (component, expected_slot) in input.components.iter().zip(expected_slots) {
        component.validate()?;
        let occurrence = component.occurrence();
        if occurrence.component.slot() != expected_slot
            || occurrence.resolution_basis_ref != input.application.resolution_basis_ref
            || occurrence.frontier_ref != input.application.frontier_ref
            || !occurrence
                .component
                .matches_selection(selection_for_slot(&input.application, expected_slot))
        {
            return Err(RecipeRuntimeErrorV1::ComponentSetMismatch);
        }
        component_provenance.push(PacketRecipeComponentProvenanceV1 {
            component_slot: match expected_slot {
                RecipeComponentSlotV1::Primary => PacketRecipeComponentSlotV1::Primary,
                RecipeComponentSlotV1::Continuation => PacketRecipeComponentSlotV1::Continuation,
            },
            recipe_return_occurrence: occurrence.clone(),
            component_output_hash: component.output_hash()?,
        });
    }
    let outcomes = input
        .components
        .iter()
        .map(|component| component.occurrence().outcome_tag)
        .collect::<Vec<_>>();
    let composition_outcome = if outcomes.is_empty() {
        PacketRecipeAdviceOutcomeV1::CoreOnly
    } else if outcomes.contains(&RecipeComponentOutcomeTagV1::HardStop) {
        PacketRecipeAdviceOutcomeV1::HardStop
    } else if outcomes.contains(&RecipeComponentOutcomeTagV1::NotApplicable) {
        PacketRecipeAdviceOutcomeV1::NotApplicable
    } else {
        PacketRecipeAdviceOutcomeV1::RestrictiveAdvice
    };
    let primary = compose_primary(frontier, &input.components, composition_outcome)?;
    let admissible_action_refs = primary.admissible_action_refs;
    let ordered_action_refs = primary.ordered_action_refs;
    let preferred_complete_wave_ref = primary.preferred_complete_wave_ref;
    let continuation_limits = input
        .components
        .iter()
        .flat_map(|component| component.continuation_limits.clone())
        .collect::<Vec<_>>();
    let component_hashes = component_provenance
        .iter()
        .map(|row| row.component_output_hash)
        .collect::<Vec<_>>();
    let composed_output_hash = domain_hash(
        "maestro.vnext.composed-recipe-advice.v1",
        &CborValue::Array(vec![
            CborValue::Unsigned(composition_tag(composition_outcome)),
            digest_array(&component_hashes),
            text_array(&admissible_action_refs),
            text_array(&ordered_action_refs),
            optional_text(preferred_complete_wave_ref.as_deref()),
            CborValue::Array(
                continuation_limits
                    .iter()
                    .map(BoundedContinuationLimitV1::canonical_value)
                    .collect(),
            ),
        ]),
    )?;
    let recipe_binding = PacketRecipeBindingV1 {
        schema_version: 1,
        selection_request_hash: input.application.selection_request().semantic_hash()?,
        recipe_application_hash: input.application.semantic_hash()?,
        recipe_application: input.application,
        component_provenance,
        advice_provenance: PacketRecipeAdviceProvenanceV1 {
            composition_outcome,
            ordered_component_output_hashes: component_hashes,
            composed_output_hash,
        },
    };
    recipe_binding.validate()?;
    Ok(ComposedRecipeAdviceV1 {
        recipe_binding,
        composition_outcome,
        admissible_action_refs,
        ordered_action_refs,
        preferred_complete_wave_ref,
        continuation_limits,
    })
}

fn validate_exact_catalog_application(
    catalog: &FrozenRecipeCatalogV1,
    application: &RecipeApplicationV1,
) -> Result<(), RecipeRuntimeErrorV1> {
    match &application.primary {
        ExactRecipeSelectionV1::Absent => {}
        ExactRecipeSelectionV1::Primary {
            recipe_resource_ref,
            manifest_content_ref,
        } if catalog.validates_primary(recipe_resource_ref, manifest_content_ref) => {}
        _ => return Err(RecipeRuntimeErrorV1::StaleOrForeignSelection),
    }
    match &application.continuation {
        ExactRecipeSelectionV1::Absent => {}
        ExactRecipeSelectionV1::Continuation {
            recipe_resource_ref,
            manifest_content_ref,
            profile_resource_ref,
        } if catalog.validates_continuation(
            recipe_resource_ref,
            manifest_content_ref,
            profile_resource_ref,
        ) => {}
        _ => return Err(RecipeRuntimeErrorV1::StaleOrForeignSelection),
    }
    Ok(())
}

fn expected_slots(application: &RecipeApplicationV1) -> Vec<RecipeComponentSlotV1> {
    let mut slots = Vec::new();
    if !application.primary.is_absent() {
        slots.push(RecipeComponentSlotV1::Primary);
    }
    if !application.continuation.is_absent() {
        slots.push(RecipeComponentSlotV1::Continuation);
    }
    slots
}

fn selection_for_slot(
    application: &RecipeApplicationV1,
    slot: RecipeComponentSlotV1,
) -> &ExactRecipeSelectionV1 {
    match slot {
        RecipeComponentSlotV1::Primary => &application.primary,
        RecipeComponentSlotV1::Continuation => &application.continuation,
    }
}

fn compose_primary(
    frontier: &ActionFrontierViewV1,
    components: &[RecipeComponentAdviceV1],
    outcome: PacketRecipeAdviceOutcomeV1,
) -> Result<PrimaryCompositionV1, RecipeRuntimeErrorV1> {
    if matches!(
        outcome,
        PacketRecipeAdviceOutcomeV1::NotApplicable | PacketRecipeAdviceOutcomeV1::HardStop
    ) {
        return Ok(PrimaryCompositionV1::empty());
    }
    let Some(primary) = components.iter().find(|component| {
        component.occurrence().component.slot() == RecipeComponentSlotV1::Primary
    }) else {
        return Ok(PrimaryCompositionV1 {
            admissible_action_refs: frontier.eligible_action_refs.clone(),
            ordered_action_refs: Vec::new(),
            preferred_complete_wave_ref: None,
        });
    };
    let allowed = primary
        .allowed_action_refs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !allowed
        .iter()
        .all(|action| frontier.eligible_action_refs.contains(action))
        || !primary
            .ordered_action_refs
            .iter()
            .all(|action| allowed.contains(action))
        || primary
            .preferred_complete_wave_ref
            .as_ref()
            .is_some_and(|wave| !frontier.complete_wave_refs.contains(wave))
    {
        return Err(RecipeRuntimeErrorV1::AdviceEscapesFrontier);
    }
    let admissible = frontier
        .eligible_action_refs
        .iter()
        .filter(|action| allowed.contains(*action))
        .cloned()
        .collect::<Vec<_>>();
    if admissible.is_empty() {
        return Err(RecipeRuntimeErrorV1::AdviceRemovesEveryOpportunity);
    }
    Ok(PrimaryCompositionV1 {
        admissible_action_refs: admissible,
        ordered_action_refs: primary.ordered_action_refs.clone(),
        preferred_complete_wave_ref: primary.preferred_complete_wave_ref.clone(),
    })
}

struct PrimaryCompositionV1 {
    admissible_action_refs: Vec<String>,
    ordered_action_refs: Vec<String>,
    preferred_complete_wave_ref: Option<String>,
}

impl PrimaryCompositionV1 {
    fn empty() -> Self {
        Self {
            admissible_action_refs: Vec::new(),
            ordered_action_refs: Vec::new(),
            preferred_complete_wave_ref: None,
        }
    }
}

fn occurrence_value(occurrence: &RecipeReturnOccurrenceV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(occurrence.schema_version),
        text(&occurrence.resolution_basis_ref),
        text(&occurrence.frontier_ref),
        component_value(&occurrence.component),
        CborValue::Unsigned(occurrence.outcome_tag as u64),
        CborValue::Array(vec![
            text(
                &occurrence
                    .return_reason_ref
                    .recipe_return_reason_resource_ref,
            ),
            CborValue::Unsigned(occurrence.return_reason_ref.reason as u64),
        ]),
    ])
}

fn component_value(component: &RecipeReturnComponentV1) -> CborValue {
    match component {
        RecipeReturnComponentV1::Primary {
            recipe_resource_ref,
            manifest_content_ref,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            text(recipe_resource_ref),
            text(manifest_content_ref),
        ]),
        RecipeReturnComponentV1::Continuation {
            recipe_resource_ref,
            manifest_content_ref,
            profile_resource_ref,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            text(recipe_resource_ref),
            text(manifest_content_ref),
            text(profile_resource_ref),
        ]),
    }
}

fn composition_tag(outcome: PacketRecipeAdviceOutcomeV1) -> u64 {
    match outcome {
        PacketRecipeAdviceOutcomeV1::CoreOnly => 1,
        PacketRecipeAdviceOutcomeV1::NotApplicable => 2,
        PacketRecipeAdviceOutcomeV1::RestrictiveAdvice => 3,
        PacketRecipeAdviceOutcomeV1::HardStop => 4,
    }
}

fn strictly_ordered_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn text(value: &str) -> CborValue {
    CborValue::Text(value.to_owned())
}

fn text_array(values: &[String]) -> CborValue {
    CborValue::Array(values.iter().map(|value| text(value)).collect())
}

fn optional_text(value: Option<&str>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), text(value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

fn digest_array(values: &[[u8; 32]]) -> CborValue {
    CborValue::Array(
        values
            .iter()
            .map(|value| CborValue::Bytes(value.to_vec()))
            .collect(),
    )
}

fn domain_hash(domain: &str, value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            text(domain),
            value.clone(),
        ]))?)
        .into(),
    )
}

#[derive(Debug, Error)]
pub(crate) enum RecipeRuntimeErrorV1 {
    #[error("Action Frontier is incomplete, unbounded, unordered, or uncommitted")]
    InvalidFrontier,
    #[error("Recipe application and evaluation do not bind one exact Frontier and basis")]
    MixedFrontier,
    #[error("selected Recipe or profile does not match the exact frozen catalog bytes")]
    StaleOrForeignSelection,
    #[error("Recipe component count, slot, selection, or occurrence does not match application")]
    ComponentSetMismatch,
    #[error("Recipe component Advice is not restrictive for its exact component role")]
    InvalidComponentAdvice,
    #[error("Recipe Advice references an Action or Wave outside the exact Frontier")]
    AdviceEscapesFrontier,
    #[error("restrictive Recipe Advice cannot remove every eligible opportunity")]
    AdviceRemovesEveryOpportunity,
    #[error(transparent)]
    Catalog(#[from] FrozenRecipeCatalogErrorV1),
    #[error(transparent)]
    Literal(#[from] RecipeLiteralError),
    #[error(transparent)]
    PublicLiteral(#[from] PublicLiteralError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
