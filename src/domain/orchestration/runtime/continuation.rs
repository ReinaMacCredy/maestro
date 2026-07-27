use thiserror::Error;

use crate::domain::orchestration::literals::{
    BoundedContinuationProfileV1, ExactRecipeSelectionV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::advice::ComposedRecipeAdviceV1;
use super::catalog::{FrozenRecipeCatalogV1, ProfileAmbiguityDispositionV1};

const MAX_CONTINUATION_LIMITS_V1: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub(crate) enum BoundedContinuationLimitKindV1 {
    Cadence = 1,
    Attempts = 2,
    Time = 3,
    Cost = 4,
    SubagentCount = 5,
    ConnectorPermissions = 6,
    Denylist = 7,
    HardStops = 8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundedContinuationLimitV1 {
    pub(crate) kind: BoundedContinuationLimitKindV1,
    pub(crate) ceiling: u64,
    pub(crate) consumed: u64,
    pub(crate) ceiling_source_ref: String,
    pub(crate) reset_boundary_ref: String,
}

impl BoundedContinuationLimitV1 {
    pub(crate) const fn kind(&self) -> BoundedContinuationLimitKindV1 {
        self.kind
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind as u64),
            CborValue::Unsigned(self.ceiling),
            CborValue::Unsigned(self.consumed),
            CborValue::Text(self.ceiling_source_ref.clone()),
            CborValue::Text(self.reset_boundary_ref.clone()),
        ])
    }

    fn validate(&self) -> bool {
        self.ceiling > 0
            && !self.ceiling_source_ref.is_empty()
            && !self.reset_boundary_ref.is_empty()
    }

    fn exhausted(&self) -> bool {
        self.consumed >= self.ceiling
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContinuationRecommendationV1 {
    pub(crate) recommendation_ref: String,
    pub(crate) action_ref: String,
    pub(crate) wave_ref: String,
    pub(crate) is_current: bool,
    pub(crate) is_auto_safe: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContinuationProgressV1 {
    pub(crate) exact_frontier_ref: String,
    pub(crate) predecessor_result_hash: [u8; 32],
    pub(crate) current_result_hash: [u8; 32],
    pub(crate) current_wave_ref: String,
    pub(crate) wave_boundary: bool,
    pub(crate) authority_available: bool,
    pub(crate) evidence_complete: bool,
    pub(crate) mutation_boundary: bool,
    pub(crate) external_effect_boundary: bool,
    pub(crate) material_ambiguity: bool,
    pub(crate) conflict_present: bool,
    pub(crate) terminal: bool,
    pub(crate) unknown_state: bool,
}

impl ContinuationProgressV1 {
    fn validate(&self) -> bool {
        !self.exact_frontier_ref.is_empty()
            && self.predecessor_result_hash != [0; 32]
            && self.current_result_hash != [0; 32]
            && !self.current_wave_ref.is_empty()
    }

    fn made_progress(&self) -> bool {
        self.predecessor_result_hash != self.current_result_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundedContinuationInputV1 {
    pub(crate) profile: BoundedContinuationProfileV1,
    pub(crate) profile_resource_ref: String,
    pub(crate) limits: Vec<BoundedContinuationLimitV1>,
    pub(crate) progress: ContinuationProgressV1,
    pub(crate) recommendation: Option<ContinuationRecommendationV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundedContinuationStopV1 {
    WaveBoundary,
    MissingAuthority,
    EvidenceBoundary,
    MutationBoundary,
    ExternalEffectBoundary,
    MaterialAmbiguity,
    OperatingLimit,
    Conflict,
    NoProgress,
    Terminal,
    UnknownState,
    MissingCurrentAutoSafeRecommendation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BoundedContinuationOutcomeV1 {
    SubmitOneCurrentAutoSafeRecommendation {
        recommendation_ref: String,
        action_ref: String,
    },
    ReturnForMaterialChoice {
        stop: BoundedContinuationStopV1,
    },
    HardStop {
        stop: BoundedContinuationStopV1,
    },
}

pub(crate) fn evaluate_bounded_continuation(
    catalog: &FrozenRecipeCatalogV1,
    advice: &ComposedRecipeAdviceV1,
    input: &BoundedContinuationInputV1,
) -> Result<BoundedContinuationOutcomeV1, BoundedContinuationErrorV1> {
    let profile = catalog
        .profile(input.profile)
        .filter(|row| row.profile_resource_ref == input.profile_resource_ref)
        .ok_or(BoundedContinuationErrorV1::StaleOrForeignProfile)?;
    let ExactRecipeSelectionV1::Continuation {
        profile_resource_ref,
        ..
    } = &advice.recipe_binding.recipe_application.continuation
    else {
        return Err(BoundedContinuationErrorV1::InvalidAdviceBinding);
    };
    if profile_resource_ref != &input.profile_resource_ref
        || !advice.is_actionable()
        || !limits_tighten_advice(&advice.continuation_limits, &input.limits)
        || advice.recipe_binding.recipe_application.frontier_ref
            != input.progress.exact_frontier_ref
    {
        return Err(BoundedContinuationErrorV1::InvalidAdviceBinding);
    }
    validate_input(input)?;

    if input.progress.unknown_state {
        return Ok(stop(
            profile.ambiguity_or_missing_authority,
            BoundedContinuationStopV1::UnknownState,
        ));
    }
    if input.progress.wave_boundary {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::WaveBoundary,
        });
    }
    if input.progress.terminal {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::Terminal,
        });
    }
    if input.progress.conflict_present {
        return Ok(stop(
            profile.ambiguity_or_missing_authority,
            BoundedContinuationStopV1::Conflict,
        ));
    }
    if input.progress.material_ambiguity {
        return Ok(stop(
            profile.ambiguity_or_missing_authority,
            BoundedContinuationStopV1::MaterialAmbiguity,
        ));
    }
    if !input.progress.authority_available {
        return Ok(stop(
            profile.ambiguity_or_missing_authority,
            BoundedContinuationStopV1::MissingAuthority,
        ));
    }
    if !input.progress.evidence_complete {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::EvidenceBoundary,
        });
    }
    if input.progress.mutation_boundary {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::MutationBoundary,
        });
    }
    if input.progress.external_effect_boundary {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::ExternalEffectBoundary,
        });
    }
    if input
        .limits
        .iter()
        .any(BoundedContinuationLimitV1::exhausted)
    {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::OperatingLimit,
        });
    }
    if !input.progress.made_progress() {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::NoProgress,
        });
    }
    let Some(recommendation) = &input.recommendation else {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::MissingCurrentAutoSafeRecommendation,
        });
    };
    if recommendation.wave_ref != input.progress.current_wave_ref {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::WaveBoundary,
        });
    }
    if !recommendation.is_current || !recommendation.is_auto_safe {
        return Ok(BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::MissingCurrentAutoSafeRecommendation,
        });
    }
    if !advice
        .admissible_action_refs
        .contains(&recommendation.action_ref)
    {
        return Err(BoundedContinuationErrorV1::InvalidAdviceBinding);
    }
    Ok(
        BoundedContinuationOutcomeV1::SubmitOneCurrentAutoSafeRecommendation {
            recommendation_ref: recommendation.recommendation_ref.clone(),
            action_ref: recommendation.action_ref.clone(),
        },
    )
}

fn limits_tighten_advice(
    advice: &[BoundedContinuationLimitV1],
    current: &[BoundedContinuationLimitV1],
) -> bool {
    advice.len() == current.len()
        && advice.iter().zip(current).all(|(advice, current)| {
            advice.kind == current.kind
                && advice.consumed <= current.consumed
                && current.ceiling <= advice.ceiling
                && advice.ceiling_source_ref == current.ceiling_source_ref
                && advice.reset_boundary_ref == current.reset_boundary_ref
        })
}

fn validate_input(input: &BoundedContinuationInputV1) -> Result<(), BoundedContinuationErrorV1> {
    if input.limits.is_empty()
        || input.limits.len() > MAX_CONTINUATION_LIMITS_V1
        || input
            .limits
            .windows(2)
            .any(|pair| pair[0].kind >= pair[1].kind)
        || input.limits.iter().any(|limit| !limit.validate())
        || !input.progress.validate()
        || input.recommendation.as_ref().is_some_and(|row| {
            row.recommendation_ref.is_empty()
                || row.action_ref.is_empty()
                || row.wave_ref.is_empty()
        })
    {
        return Err(BoundedContinuationErrorV1::InvalidInput);
    }
    Ok(())
}

fn stop(
    disposition: ProfileAmbiguityDispositionV1,
    reason: BoundedContinuationStopV1,
) -> BoundedContinuationOutcomeV1 {
    match disposition {
        ProfileAmbiguityDispositionV1::ReturnForMaterialChoice => {
            BoundedContinuationOutcomeV1::ReturnForMaterialChoice { stop: reason }
        }
        ProfileAmbiguityDispositionV1::HardStop => {
            BoundedContinuationOutcomeV1::HardStop { stop: reason }
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum BoundedContinuationErrorV1 {
    #[error("bounded-continuation profile is stale or foreign to the frozen catalog")]
    StaleOrForeignProfile,
    #[error("bounded continuation does not bind the exact composed restrictive Advice")]
    InvalidAdviceBinding,
    #[error("bounded-continuation input is incomplete, unordered, or unbounded")]
    InvalidInput,
}
