import type {
  ExecuteUkiHandoffContent,
  PlanUkiHandoffContent,
} from "@/features/handoff";

export const HANDOFF_PASTE_PREAMBLE =
  "Use the following UKI as the canonical handoff packet. Interpret each block literally and continue from NEXT_ACTION.";

export const PLAN_UKI_FIXTURE: PlanUkiHandoffContent = {
  mode: "plan",
  currentState: "plan_ready",
  sessionCore: "handoff_rebuild_plan",
  maestroRefs: {
    missionId: "2026_04_09_001",
    planPath: "plan_md",
  },
  planPaths: ["plan_md"],
  maestroSync: ["mission_created", "mission_approved"],
  decisions: ["structured_json_source_of_truth", "pickup_defaults_to_uki"],
  signalDelta: ["mission_0_1", "milestones_0_2"],
  artifacts: ["mission_2026_04_09_001", "file_plan_md"],
  readMore: ["plan_md"],
  nextAction: "implement_canonical_model",
  cs: { work: 0.95, summary: 0.9 },
  summary: "Handoff_plan_saved-mission_setup_complete-ready_to_execute",
  boundaryState: [],
  risks: [],
  causalDrivers: ["save_first_durability"],
  divergences: [],
};

export const EXECUTE_UKI_FIXTURE: ExecuteUkiHandoffContent = {
  mode: "execute",
  currentState: "execute_in_progress",
  sessionCore: "refactor_bug_hunt_fix",
  maestroRefs: {
    missionId: "2026_04_09_001",
    featureId: "handoff_renderer",
    milestoneId: "execute",
  },
  decisions: ["rank_then_implement", "atomic_latest_claim", "legacy_read_compat"],
  signalDelta: ["tests_903_909", "status_pending_visible", "claim_path_atomic"],
  touchedFiles: ["file_src_lib_uki_format_ts", "file_src_handoff_store_adapter_ts"],
  completedWork: ["structured_model_written", "renderer_migrated"],
  validation: ["build_green", "unit_green", "real_handoff_created"],
  artifacts: ["commit_6fa0a8cc", "branch_feat_handoff_rebuild", "file_src_lib_uki_format_ts"],
  readMore: ["file_src_lib_uki_format_ts", "file_tests_uki_handoff_roundtrip_ts"],
  boundaryState: ["snapshot_local_edits_untouched"],
  risks: ["green_tests_masked_contract_drift"],
  blindSpot: "green_tests_masked_contract_drift",
  metaphor: "iceberg_below_green_suite",
  nextAction: "wire_cli_auto_collection",
  cs: { work: 0.96, summary: 0.93 },
  summary: "Latent_refactors_fixed-verified_and_migrated-neighbor_edits_untouched",
  causalDrivers: ["user_requested_swarm"],
  divergences: ["legacy_shape_drift"],
};

export const LEGACY_V53_UKI =
  "SESSION_CORE-refactor_bug_hunt_fix"
  + "|CAUSAL_DRIVERS-user_requested_swarm-latent_regressions_proven-fix_all_followup"
  + "|DIVERGENCES-green_suite_hidden_bugs-status_showed_empty-claim_returned_pending-overlay_lost_query"
  + "|KEY_DECISIONS-rank_then_implement-atomic_latest_claim-legacy_read_compat"
  + "|DECISION_BASIS-severity_first_hidden_bugs-claim_fix_due_race_risk"
  + "|SIGNAL_DELTA-tests_903_to_909-status_pending_visible-claim_path_atomic"
  + "|VALIDATION_STATE-build_green-test_909_green-real_handoff_created"
  + "|EXECUTION_STATE-build_test_typecheck_green"
  + "|BOUNDARY_STATE-snapshot_local_edits_untouched"
  + "|NEXT_ACTION-wire_cli_auto_collection"
  + "|ARTIFACTS-commit_6fa0a8cc-file_src_lib_uki_format_ts"
  + "|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION"
  + "|BLIND_SPOT-green_tests_masked_contract_drift"
  + "|METAPHOR-iceberg_below_green_suite"
  + "|CS-work_0.96~summary_0.93"
  + "|SUMMARY-latent_refactors_fixed-verified_and_committed-neighbor_edits_unmerged";

export const LEGACY_V52_UKI =
  "SESSION_CORE-legacy_record"
  + "|CAUSAL_DRIVERS-upgrade_path"
  + "|DIVERGENCES-NONE"
  + "|KEY_DECISIONS-keep_pickup_safe"
  + "|SIGNAL_DELTA-handoffs_1_2"
  + "|ARTIFACTS-branch_main-file_src_lib_uki_format_ts"
  + "|EXECUTION_STATE-legacy_tmpdir"
  + "|BOUNDARY_STATE-NONE"
  + "|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION"
  + "|NEXT_ACTION-review_upgrade"
  + "|CS-work_0.8"
  + "|SUMMARY-Legacy_record-normalized-low_risk";
