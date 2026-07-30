//! Stateless Recipe application, return composition, and bounded continuation.
#![expect(
    dead_code,
    reason = "Stage-7 candidate owner module remains inert until downstream integration"
)]

mod advice;
mod catalog;
mod continuation;

#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use advice::{
    ActionFrontierViewV1, ComposedRecipeAdviceV1, RecipeComponentAdviceV1, RecipeEvaluationInputV1,
    RecipeRuntimeErrorV1, evaluate_recipe_application,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use catalog::{
    FrozenRecipeCatalogErrorV1, FrozenRecipeCatalogV1, FrozenRecipeEntryV1, FrozenRecipeProfileV1,
    ProfileAmbiguityDispositionV1,
};
#[allow(
    unused_imports,
    reason = "Stage-7 owner facade is frozen before its downstream adapters"
)]
pub(crate) use continuation::{
    BoundedContinuationErrorV1, BoundedContinuationInputV1, BoundedContinuationLimitKindV1,
    BoundedContinuationLimitV1, BoundedContinuationOutcomeV1, BoundedContinuationStopV1,
    ContinuationProgressV1, ContinuationRecommendationV1, evaluate_bounded_continuation,
};

#[cfg(test)]
mod tests;
