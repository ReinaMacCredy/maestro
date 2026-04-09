import { describe, expect, it } from "bun:test";
import { compressUki, parseUki, validateUki } from "@/lib/uki-format.js";
import type { ExecuteUkiHandoffContent } from "@/domain/uki-types.js";
import {
  EXECUTE_UKI_FIXTURE,
  LEGACY_V52_UKI,
  LEGACY_V53_UKI,
  PLAN_UKI_FIXTURE,
} from "../../helpers/uki-fixtures.js";

  describe("UKI v5.4 format", () => {
  it("compresses a plan payload in deterministic order", () => {
    expect(compressUki(PLAN_UKI_FIXTURE)).toBe(
      "MODE-plan"
      + "|CURRENT_STATE-plan_ready"
      + "|SESSION_CORE-handoff_rebuild_plan"
      + "|CAUSAL_DRIVERS-save_first_durability"
      + "|DIVERGENCES-NONE"
      + "|MAESTRO_REFS-mission_2026_04_09_001-plan_plan_md"
      + "|PLAN_PATHS-plan_md"
      + "|MAESTRO_SYNC-mission_created-mission_approved"
        + "|DECISIONS-structured_json_source_of_truth-pickup_defaults_to_uki"
        + "|SIGNAL_DELTA-mission_0_1-milestones_0_2"
        + "|ARTIFACTS-mission_2026_04_09_001-file_plan_md"
        + "|READ_MORE-plan_md"
        + "|NEXT_ACTION-implement_canonical_model"
        + "|CS-work_0.95~summary_0.9"
        + "|SUMMARY-Handoff_plan_saved-mission_setup_complete-ready_to_execute",
    );
  });

  it("compresses an execute payload with optional blind spot and metaphor", () => {
    expect(compressUki(EXECUTE_UKI_FIXTURE)).toBe(
      "MODE-execute"
      + "|CURRENT_STATE-execute_in_progress"
      + "|SESSION_CORE-refactor_bug_hunt_fix"
      + "|CAUSAL_DRIVERS-user_requested_swarm"
      + "|DIVERGENCES-legacy_shape_drift"
      + "|MAESTRO_REFS-mission_2026_04_09_001-feature_handoff_renderer-milestone_execute"
      + "|DECISIONS-rank_then_implement-atomic_latest_claim-legacy_read_compat"
      + "|SIGNAL_DELTA-tests_903_909-status_pending_visible-claim_path_atomic"
      + "|TOUCHED_FILES-file_src_lib_uki_format_ts-file_src_handoff_store_adapter_ts"
        + "|COMPLETED_WORK-structured_model_written-renderer_migrated"
        + "|VALIDATION-build_green-unit_green-real_handoff_created"
        + "|ARTIFACTS-commit_6fa0a8cc-branch_feat_handoff_rebuild-file_src_lib_uki_format_ts"
      + "|READ_MORE-file_src_lib_uki_format_ts-file_tests_uki_handoff_roundtrip_ts"
        + "|BOUNDARY_STATE-snapshot_local_edits_untouched"
        + "|RISKS-green_tests_masked_contract_drift"
        + "|BLIND_SPOT-green_tests_masked_contract_drift"
      + "|METAPHOR-iceberg_below_green_suite"
        + "|NEXT_ACTION-wire_cli_auto_collection"
        + "|CS-work_0.96~summary_0.93"
        + "|SUMMARY-Latent_refactors_fixed-verified_and_migrated-neighbor_edits_untouched",
    );
  });

  it("round-trips plan and execute payloads", () => {
    expect(parseUki(compressUki(PLAN_UKI_FIXTURE))).toEqual(PLAN_UKI_FIXTURE);
    expect(parseUki(compressUki(EXECUTE_UKI_FIXTURE))).toEqual(EXECUTE_UKI_FIXTURE);
  });

  it("accepts six-word underscore tokens in v5.4 output", () => {
    const compressed = compressUki(EXECUTE_UKI_FIXTURE);
    expect(compressed).toContain("file_src_lib_uki_format_ts");
    expect(validateUki(compressed)).toEqual([]);
  });

  it("flags forbidden characters and overlong summary values", () => {
    const withColon = compressUki(PLAN_UKI_FIXTURE).replace(
        "SESSION_CORE-handoff_rebuild_plan",
        "SESSION_CORE-handoff:rebuild_plan",
      );
      expect(validateUki(withColon).length).toBeGreaterThan(0);

    const longSummary = `SUMMARY-${"x".repeat(141)}`;
    const withLongSummary = compressUki(PLAN_UKI_FIXTURE).replace(
        "SUMMARY-Handoff_plan_saved-mission_setup_complete-ready_to_execute",
        longSummary,
      );
      expect(validateUki(withLongSummary).length).toBeGreaterThan(0);
  });

  it("normalizes legacy v5.3 execute strings into the new content model", () => {
    expect(parseUki(LEGACY_V53_UKI)).toEqual({
        mode: "execute",
        currentState: "build_test_typecheck_green",
        sessionCore: "refactor_bug_hunt_fix",
      decisions: [
        "rank_then_implement",
        "atomic_latest_claim",
        "legacy_read_compat",
        "severity_first_hidden_bugs",
        "claim_fix_due_race_risk",
      ],
      artifacts: ["commit_6fa0a8cc", "file_src_lib_uki_format_ts"],
      readMore: ["file_src_lib_uki_format_ts"],
      nextAction: "wire_cli_auto_collection",
      summary: "latent_refactors_fixed-verified_and_committed-neighbor_edits_unmerged",
      maestroRefs: {},
      cs: { work: 0.96, summary: 0.93 },
      signalDelta: ["tests_903_to_909", "status_pending_visible", "claim_path_atomic"],
      boundaryState: ["snapshot_local_edits_untouched"],
      risks: ["green_tests_masked_contract_drift"],
      blindSpot: "green_tests_masked_contract_drift",
      metaphor: "iceberg_below_green_suite",
      causalDrivers: [
        "user_requested_swarm",
        "latent_regressions_proven",
        "fix_all_followup",
      ],
      divergences: [
        "green_suite_hidden_bugs",
        "status_showed_empty",
        "claim_returned_pending",
        "overlay_lost_query",
      ],
        touchedFiles: ["file_src_lib_uki_format_ts"],
        completedWork: ["tests_903_to_909", "status_pending_visible", "claim_path_atomic"],
        validation: ["build_green", "test_909_green", "real_handoff_created"],
    } satisfies ExecuteUkiHandoffContent);
  });

  it("normalizes legacy v5.2 execute strings into the new content model", () => {
    expect(parseUki(LEGACY_V52_UKI)).toEqual({
      mode: "execute",
      currentState: "legacy_tmpdir",
      sessionCore: "legacy_record",
      decisions: ["keep_pickup_safe"],
      artifacts: ["branch_main", "file_src_lib_uki_format_ts"],
      readMore: ["file_src_lib_uki_format_ts"],
      nextAction: "review_upgrade",
      summary: "Legacy_record-normalized-low_risk",
      maestroRefs: {},
      cs: { work: 0.8 },
      signalDelta: ["handoffs_1_2"],
      boundaryState: [],
      risks: [],
      causalDrivers: ["upgrade_path"],
      divergences: [],
      touchedFiles: ["file_src_lib_uki_format_ts"],
      completedWork: ["handoffs_1_2"],
      validation: [],
    } satisfies ExecuteUkiHandoffContent);
  });
});
