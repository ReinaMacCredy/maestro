use std::fs;
use std::path::Path;

#[test]
fn product_pruning_places_expected_old_effect_inside_authority_linearization() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        fs::read_to_string(workspace.join("src/domain/installation/resource_cutover.rs")).unwrap();
    let body = source
        .split("pub(in crate::domain) fn execute_stage12_product_pruning(")
        .nth(1)
        .and_then(|tail| tail.split("fn migration_release_id(").next())
        .expect("Stage12 pruning body");

    assert!(body.contains("consume_with_linearization(&closure, ||"));
    assert!(body.contains("effects.compare_expected_old_and_prune("));
    assert!(body.contains("AgentResourceCutoverErrorV1::Stage12BindingMismatch)??"));
    assert!(!body.contains(".consume_for(&closure)"));
    assert!(
        body.find("continuation.deletion_plan_id != closure.deletion_plan_id()")
            < body.find("consume_with_linearization(&closure, ||")
    );
    let callback = body
        .split("consume_with_linearization(&closure, ||")
        .nth(1)
        .and_then(|tail| tail.split(".map_err(").next())
        .expect("Authority-bracketed callback");
    assert!(callback.contains("effects.compare_expected_old_and_prune("));
}

#[test]
fn authority_predecessor_runtime_suite_covers_refusal_once_error_and_fence() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let authority =
        fs::read_to_string(workspace.join("src/domain/authority/facade_tests.rs")).unwrap();
    for runtime_test in [
        "legacy_removal_guard_v2_rechecks_every_live_authority_binding",
        "legacy_removal_guard_v2_rechecks_every_consumer_closure_binding",
        "legacy_removal_guard_v2_invokes_linearization_once_and_preserves_callback_error",
        "legacy_removal_guard_v2_holds_the_store_write_fence_through_linearization",
    ] {
        assert!(
            authority.contains(&format!("fn {runtime_test}()")),
            "missing Authority runtime proof {runtime_test}"
        );
    }
}
