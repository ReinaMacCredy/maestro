use serde_json::Value;

use crate::domain::vnext::integration::public_literals::PacketRecipeAdviceOutcomeV1;
use crate::domain::vnext::orchestration::literals::{
    BoundedContinuationProfileV1, ExactRecipeSelectionV1, RecipeComponentEvaluationV1,
    RecipeComponentOutcomeTagV1, RecipeIdV1, RecipeReturnComponentV1, RecipeReturnOccurrenceV1,
    RecipeReturnReasonRefV1, RecipeReturnReasonV1, RecipeSelectionRequestV1,
};

use super::*;

type ContinuationStopCaseV1 = (
    fn(&mut BoundedContinuationInputV1),
    BoundedContinuationStopV1,
);

fn frontier(seed: usize) -> ActionFrontierViewV1 {
    ActionFrontierViewV1 {
        resolution_basis_ref: format!("basis:{seed}"),
        frontier_ref: format!("frontier:{seed}"),
        eligible_action_refs: vec!["action:a".into(), "action:b".into()],
        complete_wave_refs: vec!["wave:a-b".into()],
        material_dependency_hash: [1; 32],
    }
}

fn not_applicable(
    application: &crate::domain::vnext::orchestration::literals::RecipeApplicationV1,
    selection: &ExactRecipeSelectionV1,
) -> RecipeComponentAdviceV1 {
    let component = match selection {
        ExactRecipeSelectionV1::Primary {
            recipe_resource_ref,
            manifest_content_ref,
        } => RecipeReturnComponentV1::Primary {
            recipe_resource_ref: recipe_resource_ref.clone(),
            manifest_content_ref: manifest_content_ref.clone(),
        },
        ExactRecipeSelectionV1::Continuation {
            recipe_resource_ref,
            manifest_content_ref,
            profile_resource_ref,
        } => RecipeReturnComponentV1::Continuation {
            recipe_resource_ref: recipe_resource_ref.clone(),
            manifest_content_ref: manifest_content_ref.clone(),
            profile_resource_ref: profile_resource_ref.clone(),
        },
        ExactRecipeSelectionV1::Absent => panic!("absent selection has no component"),
    };
    let recipe = component.recipe().expect("stage7 runtime test invariant");
    let reason = RecipeReturnReasonV1::for_pair(recipe, RecipeComponentOutcomeTagV1::NotApplicable);
    RecipeComponentAdviceV1 {
        evaluation: RecipeComponentEvaluationV1::NotApplicable(RecipeReturnOccurrenceV1 {
            schema_version: 1,
            resolution_basis_ref: application.resolution_basis_ref.clone(),
            frontier_ref: application.frontier_ref.clone(),
            component,
            outcome_tag: RecipeComponentOutcomeTagV1::NotApplicable,
            return_reason_ref: RecipeReturnReasonRefV1 {
                recipe_return_reason_resource_ref: reason.resource_ref(),
                reason,
            },
        }),
        allowed_action_refs: Vec::new(),
        ordered_action_refs: Vec::new(),
        preferred_complete_wave_ref: None,
        continuation_limits: Vec::new(),
    }
}

#[test]
fn exact_ten_recipe_two_profile_catalog_covers_all_thirty_application_shapes() {
    let catalog = FrozenRecipeCatalogV1::load().expect("stage7 runtime test invariant");
    assert_eq!(catalog.recipes.len(), 10);
    assert_eq!(catalog.profiles.len(), 2);
    assert_eq!(super::catalog::frozen_source_count(), 13);

    let primary_axis = std::iter::once(ExactRecipeSelectionV1::Absent)
        .chain(
            catalog
                .recipes
                .iter()
                .filter(|row| row.recipe != RecipeIdV1::BoundedContinuation)
                .map(|row| ExactRecipeSelectionV1::Primary {
                    recipe_resource_ref: row.recipe_resource_ref.clone(),
                    manifest_content_ref: row.manifest_content_ref.clone(),
                }),
        )
        .collect::<Vec<_>>();
    let continuation_recipe = catalog
        .recipe(RecipeIdV1::BoundedContinuation)
        .expect("stage7 runtime test invariant");
    let continuation_axis = std::iter::once(ExactRecipeSelectionV1::Absent)
        .chain(
            catalog
                .profiles
                .iter()
                .map(|profile| ExactRecipeSelectionV1::Continuation {
                    recipe_resource_ref: continuation_recipe.recipe_resource_ref.clone(),
                    manifest_content_ref: continuation_recipe.manifest_content_ref.clone(),
                    profile_resource_ref: profile.profile_resource_ref.clone(),
                }),
        )
        .collect::<Vec<_>>();

    let mut evaluated = 0;
    for primary in &primary_axis {
        for continuation in &continuation_axis {
            let current_frontier = frontier(evaluated);
            let application = RecipeSelectionRequestV1 {
                schema_version: 1,
                resolution_basis_ref: current_frontier.resolution_basis_ref.clone(),
                primary_selection: primary.clone(),
                continuation_selection: continuation.clone(),
            }
            .seal(current_frontier.frontier_ref.clone())
            .expect("stage7 runtime test invariant");
            let components = [primary, continuation]
                .into_iter()
                .filter(|selection| !selection.is_absent())
                .map(|selection| not_applicable(&application, selection))
                .collect();
            let result = evaluate_recipe_application(
                &catalog,
                &current_frontier,
                RecipeEvaluationInputV1 {
                    application,
                    components,
                },
            )
            .expect("stage7 runtime test invariant");
            if primary.is_absent() && continuation.is_absent() {
                assert_eq!(
                    result.composition_outcome,
                    PacketRecipeAdviceOutcomeV1::CoreOnly
                );
                assert!(result.is_actionable());
            } else {
                assert_eq!(
                    result.composition_outcome,
                    PacketRecipeAdviceOutcomeV1::NotApplicable
                );
                assert!(!result.is_actionable());
                assert!(result.admissible_action_refs.is_empty());
            }
            evaluated += 1;
        }
    }
    assert_eq!(evaluated, 30);
}

#[test]
fn frozen_public_vectors_retain_exact_stage_zero_counts() {
    let application: Value = serde_json::from_str(include_str!(
        "../../../../../contracts/vnext/public/recipe_selection_application_vectors.v1.json"
    ))
    .expect("stage7 runtime test invariant");
    let returns: Value = serde_json::from_str(include_str!(
        "../../../../../contracts/vnext/public/recipe_return_reasons.v1.json"
    ))
    .expect("stage7 runtime test invariant");
    let eligibility: Value = serde_json::from_str(include_str!(
        "../../../../../contracts/vnext/public/job_recipe_eligibility_vectors.v1.json"
    ))
    .expect("stage7 runtime test invariant");
    assert_eq!(application["vector_count"], 30);
    assert_eq!(returns["application_outcome_vector_count"], 196);
    assert_eq!(eligibility["application_vector_count"], 210);
    assert_eq!(eligibility["positive_edges"], 22);
    assert_eq!(eligibility["negative_edges"], 48);
}

fn continuation_fixture(
    catalog: &FrozenRecipeCatalogV1,
    profile: BoundedContinuationProfileV1,
) -> (ComposedRecipeAdviceV1, BoundedContinuationInputV1) {
    let continuation_recipe = catalog
        .recipe(RecipeIdV1::BoundedContinuation)
        .expect("stage7 runtime test invariant");
    let profile_resource_ref = catalog
        .profile(profile)
        .expect("stage7 runtime test invariant")
        .profile_resource_ref
        .clone();
    let current_frontier = frontier(100 + profile as usize);
    let selection = ExactRecipeSelectionV1::Continuation {
        recipe_resource_ref: continuation_recipe.recipe_resource_ref.clone(),
        manifest_content_ref: continuation_recipe.manifest_content_ref.clone(),
        profile_resource_ref: profile_resource_ref.clone(),
    };
    let application = RecipeSelectionRequestV1 {
        schema_version: 1,
        resolution_basis_ref: current_frontier.resolution_basis_ref.clone(),
        primary_selection: ExactRecipeSelectionV1::Absent,
        continuation_selection: selection.clone(),
    }
    .seal(current_frontier.frontier_ref.clone())
    .expect("stage7 runtime test invariant");
    let limit = BoundedContinuationLimitV1 {
        kind: BoundedContinuationLimitKindV1::Attempts,
        ceiling: 3,
        consumed: 1,
        ceiling_source_ref: "limit:attempts".into(),
        reset_boundary_ref: "boundary:application".into(),
    };
    let reason = RecipeReturnReasonV1::for_pair(
        RecipeIdV1::BoundedContinuation,
        RecipeComponentOutcomeTagV1::RestrictiveAdvice,
    );
    let advice = evaluate_recipe_application(
        catalog,
        &current_frontier,
        RecipeEvaluationInputV1 {
            application: application.clone(),
            components: vec![RecipeComponentAdviceV1 {
                evaluation: RecipeComponentEvaluationV1::RestrictiveAdvice {
                    recipe_advice_ref: "advice:bounded-continuation".into(),
                    occurrence: RecipeReturnOccurrenceV1 {
                        schema_version: 1,
                        resolution_basis_ref: application.resolution_basis_ref.clone(),
                        frontier_ref: application.frontier_ref.clone(),
                        component: RecipeReturnComponentV1::Continuation {
                            recipe_resource_ref: continuation_recipe.recipe_resource_ref.clone(),
                            manifest_content_ref: continuation_recipe.manifest_content_ref.clone(),
                            profile_resource_ref: profile_resource_ref.clone(),
                        },
                        outcome_tag: RecipeComponentOutcomeTagV1::RestrictiveAdvice,
                        return_reason_ref: RecipeReturnReasonRefV1 {
                            recipe_return_reason_resource_ref: reason.resource_ref(),
                            reason,
                        },
                    },
                },
                allowed_action_refs: Vec::new(),
                ordered_action_refs: Vec::new(),
                preferred_complete_wave_ref: None,
                continuation_limits: vec![limit.clone()],
            }],
        },
    )
    .expect("stage7 runtime test invariant");
    let input = BoundedContinuationInputV1 {
        profile,
        profile_resource_ref,
        limits: vec![limit],
        progress: ContinuationProgressV1 {
            exact_frontier_ref: application.frontier_ref,
            predecessor_result_hash: [2; 32],
            current_result_hash: [3; 32],
            current_wave_ref: "wave:a-b".into(),
            wave_boundary: false,
            authority_available: true,
            evidence_complete: true,
            mutation_boundary: false,
            external_effect_boundary: false,
            material_ambiguity: false,
            conflict_present: false,
            terminal: false,
            unknown_state: false,
        },
        recommendation: Some(ContinuationRecommendationV1 {
            recommendation_ref: "recommendation:current".into(),
            action_ref: "action:a".into(),
            wave_ref: "wave:a-b".into(),
            is_current: true,
            is_auto_safe: true,
        }),
    };
    (advice, input)
}

#[test]
fn bounded_continuation_submits_at_most_one_current_auto_safe_recommendation() {
    let catalog = FrozenRecipeCatalogV1::load().expect("stage7 runtime test invariant");
    let (advice, input) = continuation_fixture(&catalog, BoundedContinuationProfileV1::Attended);
    assert_eq!(
        evaluate_bounded_continuation(&catalog, &advice, &input)
            .expect("stage7 runtime test invariant"),
        BoundedContinuationOutcomeV1::SubmitOneCurrentAutoSafeRecommendation {
            recommendation_ref: "recommendation:current".into(),
            action_ref: "action:a".into(),
        }
    );
    let mut unsafe_input = input;
    unsafe_input
        .recommendation
        .as_mut()
        .expect("stage7 runtime test invariant")
        .is_auto_safe = false;
    assert_eq!(
        evaluate_bounded_continuation(&catalog, &advice, &unsafe_input)
            .expect("stage7 runtime test invariant"),
        BoundedContinuationOutcomeV1::HardStop {
            stop: BoundedContinuationStopV1::MissingCurrentAutoSafeRecommendation,
        }
    );
    let (advice, mut rollback) =
        continuation_fixture(&catalog, BoundedContinuationProfileV1::Attended);
    rollback.limits[0].consumed = 0;
    assert!(matches!(
        evaluate_bounded_continuation(&catalog, &advice, &rollback),
        Err(BoundedContinuationErrorV1::InvalidAdviceBinding)
    ));
}

#[test]
fn attended_returns_and_unattended_hard_stops_on_material_ambiguity_or_missing_authority() {
    let catalog = FrozenRecipeCatalogV1::load().expect("stage7 runtime test invariant");
    let ambiguity_mutators: [fn(&mut BoundedContinuationInputV1); 4] = [
        |input: &mut BoundedContinuationInputV1| {
            input.progress.material_ambiguity = true;
        },
        |input: &mut BoundedContinuationInputV1| {
            input.progress.authority_available = false;
        },
        |input: &mut BoundedContinuationInputV1| {
            input.progress.conflict_present = true;
        },
        |input: &mut BoundedContinuationInputV1| {
            input.progress.unknown_state = true;
        },
    ];
    for mutate in ambiguity_mutators {
        let (attended_advice, mut attended) =
            continuation_fixture(&catalog, BoundedContinuationProfileV1::Attended);
        mutate(&mut attended);
        assert!(matches!(
            evaluate_bounded_continuation(&catalog, &attended_advice, &attended)
                .expect("stage7 runtime test invariant"),
            BoundedContinuationOutcomeV1::ReturnForMaterialChoice { .. }
        ));
        let (unattended_advice, mut unattended) =
            continuation_fixture(&catalog, BoundedContinuationProfileV1::Unattended);
        mutate(&mut unattended);
        assert!(matches!(
            evaluate_bounded_continuation(&catalog, &unattended_advice, &unattended)
                .expect("stage7 runtime test invariant"),
            BoundedContinuationOutcomeV1::HardStop { .. }
        ));
    }
}

#[test]
fn continuation_stops_at_evidence_mutation_effect_limit_no_progress_terminal_and_wave_boundaries() {
    let catalog = FrozenRecipeCatalogV1::load().expect("stage7 runtime test invariant");
    let profile = BoundedContinuationProfileV1::Attended;
    let cases: [ContinuationStopCaseV1; 7] = [
        (
            |input: &mut BoundedContinuationInputV1| input.progress.evidence_complete = false,
            BoundedContinuationStopV1::EvidenceBoundary,
        ),
        (
            |input: &mut BoundedContinuationInputV1| input.progress.mutation_boundary = true,
            BoundedContinuationStopV1::MutationBoundary,
        ),
        (
            |input: &mut BoundedContinuationInputV1| {
                input.progress.external_effect_boundary = true;
            },
            BoundedContinuationStopV1::ExternalEffectBoundary,
        ),
        (
            |input: &mut BoundedContinuationInputV1| input.limits[0].consumed = 3,
            BoundedContinuationStopV1::OperatingLimit,
        ),
        (
            |input: &mut BoundedContinuationInputV1| {
                input.progress.current_result_hash = input.progress.predecessor_result_hash;
            },
            BoundedContinuationStopV1::NoProgress,
        ),
        (
            |input: &mut BoundedContinuationInputV1| input.progress.terminal = true,
            BoundedContinuationStopV1::Terminal,
        ),
        (
            |input: &mut BoundedContinuationInputV1| {
                input.progress.wave_boundary = true;
            },
            BoundedContinuationStopV1::WaveBoundary,
        ),
    ];
    for (mutate, expected) in cases {
        let (advice, mut input) = continuation_fixture(&catalog, profile);
        mutate(&mut input);
        assert_eq!(
            evaluate_bounded_continuation(&catalog, &advice, &input)
                .expect("stage7 runtime test invariant"),
            BoundedContinuationOutcomeV1::HardStop { stop: expected }
        );
    }
}
