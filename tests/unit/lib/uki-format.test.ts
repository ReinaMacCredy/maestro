/**
 * UKI v5.2 format: golden fixtures, syntax invariants, and round-trip.
 *
 * This is the TDD baseline for Phase 2 of the refactor. The compressor
 * and parser in `src/lib/uki-format.ts` must make all tests here pass
 * WITHOUT modifying the fixtures or invariant assertions.
 *
 * UKI v5.2 spec (12 slots in fixed order, pipe-separated, no colons, no newlines):
 *   1. SESSION_CORE       (single string)
 *   2. CAUSAL_DRIVERS     (list)
 *   3. DIVERGENCES        (list)
 *   4. KEY_DECISIONS      (list)
 *   5. SIGNAL_DELTA       (list, tokens may contain ~ for before->after)
 *   6. ARTIFACTS          (list, must contain at least one of commit_/branch_/version_/file_)
 *   7. EXECUTION_STATE    (single string)
 *   8. BOUNDARY_STATE     (list)
 *   9. STANCE_COLLAPSE    (single, default NONE_DETECTED_LOW_FRICTION)
 *  10. NEXT_ACTION        (single string)
 *  11. CS                 (mini-format: CS-work_X, CS-summary_Y, or CS-work_X~summary_Y)
 *  12. SUMMARY            (single string, under 140 chars)
 */
import { describe, expect, it } from "bun:test";
import {
  compressUki,
  parseUki,
  validateUki,
  type UkiSlots,
} from "../../../src/lib/uki-format.js";

interface Fixture {
  readonly name: string;
  readonly slots: UkiSlots;
  readonly expected: string;
}

const MINIMAL: Fixture = {
  name: "minimal",
  slots: {
    sessionCore: "bootstrap_handoff",
    causalDrivers: ["user_requested_kickoff"],
    divergences: [],
    keyDecisions: ["proceed_with_default_plan"],
    signalDelta: ["tests_0~1_green"],
    artifacts: ["branch_feat_bootstrap"],
    executionState: "working_tree_clean",
    boundaryState: [],
    stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
    nextAction: "pick_up_first_feature",
    cs: { work: 0.9 },
    summary: "Bootstrap_handoff-ready-low_risk",
  },
  expected:
    "SESSION_CORE-bootstrap_handoff"
    + "|CAUSAL_DRIVERS-user_requested_kickoff"
    + "|DIVERGENCES-NONE"
    + "|KEY_DECISIONS-proceed_with_default_plan"
    + "|SIGNAL_DELTA-tests_0~1_green"
    + "|ARTIFACTS-branch_feat_bootstrap"
    + "|EXECUTION_STATE-working_tree_clean"
    + "|BOUNDARY_STATE-NONE"
    + "|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION"
    + "|NEXT_ACTION-pick_up_first_feature"
    + "|CS-work_0.9"
    + "|SUMMARY-Bootstrap_handoff-ready-low_risk",
};

const FULL: Fixture = {
  name: "full",
  slots: {
    sessionCore: "memory_wire_shipped",
    causalDrivers: ["user_asked", "gap_diagnosed"],
    divergences: ["stash_polluted_workspace"],
    keyDecisions: ["inject_at_prompt_seam", "best_effort_try_catch"],
    signalDelta: ["recallMemory_callers_1~3", "tests_27~41_green"],
    artifacts: [
      "commit_79dfc053~12ed055d",
      "branch_feat_missionControl",
      "version_0_16_0~0_16_1",
    ],
    executionState: "binary_verified_working_tree_clean",
    boundaryState: ["no_caching", "no_refactor"],
    stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
    nextAction: "push_and_open_PR",
    cs: { work: 0.95, summary: 0.92 },
    summary: "Memory_auto_injects-shipped-low_risk",
  },
  expected:
    "SESSION_CORE-memory_wire_shipped"
    + "|CAUSAL_DRIVERS-user_asked-gap_diagnosed"
    + "|DIVERGENCES-stash_polluted_workspace"
    + "|KEY_DECISIONS-inject_at_prompt_seam-best_effort_try_catch"
    + "|SIGNAL_DELTA-recallMemory_callers_1~3-tests_27~41_green"
    + "|ARTIFACTS-commit_79dfc053~12ed055d-branch_feat_missionControl-version_0_16_0~0_16_1"
    + "|EXECUTION_STATE-binary_verified_working_tree_clean"
    + "|BOUNDARY_STATE-no_caching-no_refactor"
    + "|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION"
    + "|NEXT_ACTION-push_and_open_PR"
    + "|CS-work_0.95~summary_0.92"
    + "|SUMMARY-Memory_auto_injects-shipped-low_risk",
};

// Edge case: long slot values pushing against limits, non-empty BOUNDARY_STATE.
// SUMMARY is 131 chars (<140). Multiple boundary tokens.
const EDGE: Fixture = {
  name: "edge",
  slots: {
    sessionCore: "phase2_handoff_suite_complete",
    causalDrivers: ["plan_delivered", "risk_surface_audited", "tests_failing_first"],
    divergences: ["spec_ambiguity_on_empty_list"],
    keyDecisions: [
      "use_NONE_for_empty_list",
      "reject_summary_over_140",
      "normalize_dash_to_underscore",
    ],
    signalDelta: ["test_count_832~890_green", "handoff_commands_0~3"],
    artifacts: [
      "file_src_lib_uki_format_ts",
      "file_tests_unit_lib_uki_format_test_ts",
      "branch_feat_missionControl",
    ],
    executionState: "build_green_tests_green",
    boundaryState: [
      "no_migration_of_old_records",
      "no_tui_modal_changes",
      "no_worker_types_touch",
    ],
    stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
    nextAction: "run_release_local_bump",
    cs: { summary: 0.88 },
    summary:
      "Phase2_UKI_format_plus_store_plus_CLI-shipped-no_migration_path_existing_handoffs_orphaned_by_design",
  },
  expected:
    "SESSION_CORE-phase2_handoff_suite_complete"
    + "|CAUSAL_DRIVERS-plan_delivered-risk_surface_audited-tests_failing_first"
    + "|DIVERGENCES-spec_ambiguity_on_empty_list"
    + "|KEY_DECISIONS-use_NONE_for_empty_list-reject_summary_over_140-normalize_dash_to_underscore"
    + "|SIGNAL_DELTA-test_count_832~890_green-handoff_commands_0~3"
    + "|ARTIFACTS-file_src_lib_uki_format_ts-file_tests_unit_lib_uki_format_test_ts-branch_feat_missionControl"
    + "|EXECUTION_STATE-build_green_tests_green"
    + "|BOUNDARY_STATE-no_migration_of_old_records-no_tui_modal_changes-no_worker_types_touch"
    + "|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION"
    + "|NEXT_ACTION-run_release_local_bump"
    + "|CS-summary_0.88"
    + "|SUMMARY-Phase2_UKI_format_plus_store_plus_CLI-shipped-no_migration_path_existing_handoffs_orphaned_by_design",
};

const ALL_FIXTURES: readonly Fixture[] = [MINIMAL, FULL, EDGE];

const EXPECTED_SLOT_NAMES = [
  "SESSION_CORE",
  "CAUSAL_DRIVERS",
  "DIVERGENCES",
  "KEY_DECISIONS",
  "SIGNAL_DELTA",
  "ARTIFACTS",
  "EXECUTION_STATE",
  "BOUNDARY_STATE",
  "STANCE_COLLAPSE",
  "NEXT_ACTION",
  "CS",
  "SUMMARY",
] as const;

function countChar(s: string, ch: string): number {
  let count = 0;
  for (const c of s) {
    if (c === ch) count++;
  }
  return count;
}

describe("UKI v5.2 format -- golden fixtures", () => {
  for (const fixture of ALL_FIXTURES) {
    it(`compresses the ${fixture.name} fixture byte-exactly`, () => {
      const compressed = compressUki(fixture.slots);
      expect(compressed).toBe(fixture.expected);
    });
  }
});

describe("UKI v5.2 format -- syntax invariants", () => {
  for (const fixture of ALL_FIXTURES) {
    describe(`fixture: ${fixture.name}`, () => {
      const compressed = fixture.expected;
      const slots = compressed.split("|");

      it("has exactly 11 pipe separators (12 slots)", () => {
        expect(countChar(compressed, "|")).toBe(11);
        expect(slots.length).toBe(12);
      });

      it("has zero colons", () => {
        expect(countChar(compressed, ":")).toBe(0);
      });

      it("has zero newlines", () => {
        expect(countChar(compressed, "\n")).toBe(0);
        expect(countChar(compressed, "\r")).toBe(0);
      });

      it("has no leading or trailing whitespace", () => {
        expect(compressed).toBe(compressed.trim());
      });

      it("has each slot starting with its expected name in fixed order", () => {
        for (let i = 0; i < EXPECTED_SLOT_NAMES.length; i++) {
          const slot = slots[i];
          const name = EXPECTED_SLOT_NAMES[i];
          expect(slot).toBeDefined();
          expect(slot!.startsWith(`${name}-`)).toBe(true);
        }
      });

      it("has STANCE_COLLAPSE always present", () => {
        const stance = slots[8];
        expect(stance).toBeDefined();
        expect(stance!.startsWith("STANCE_COLLAPSE-")).toBe(true);
        expect(stance!.length).toBeGreaterThan("STANCE_COLLAPSE-".length);
      });

      it("has CS matching the scoped confidence pattern", () => {
        const cs = slots[10];
        expect(cs).toBeDefined();
        const pattern = /^CS-(work_\d+(\.\d+)?(~summary_\d+(\.\d+)?)?|summary_\d+(\.\d+)?)$/;
        expect(pattern.test(cs!)).toBe(true);
      });

      it("has ARTIFACTS containing at least one of commit_/branch_/version_/file_", () => {
        const artifacts = slots[5];
        expect(artifacts).toBeDefined();
        const pattern = /(commit_|branch_|version_|file_)/;
        expect(pattern.test(artifacts!)).toBe(true);
      });

      it("has SUMMARY under 140 characters", () => {
        const summary = slots[11];
        expect(summary).toBeDefined();
        // Slot value = the part after "SUMMARY-"
        const value = summary!.slice("SUMMARY-".length);
        expect(value.length).toBeLessThan(140);
      });

      it("has every _-joined token with at most 4 words", () => {
        // For each slot, strip the slot name + "-", then split on "-" for list slots
        // (each token is a _-joined string, possibly containing ~). Count _-segments.
        for (let i = 0; i < EXPECTED_SLOT_NAMES.length; i++) {
          const name = EXPECTED_SLOT_NAMES[i]!;
          // CS has its own mini-format and is not word-counted here (it is
          // separately validated by the pattern assertion above).
          if (name === "CS") continue;
          const slot = slots[i]!;
          const value = slot.slice(name.length + 1); // after "NAME-"
          if (value === "NONE") continue;
          // Split by "-" to get individual tokens. For single-string slots,
          // this is still a single element array (no embedded "-"), so the
          // word-count check still applies.
          const tokens = value.split("-");
          for (const token of tokens) {
            // A token may contain ~ for before->after series. Split on ~
            // into halves and count words per half. Each half is _-joined.
            const halves = token.split("~");
            for (const half of halves) {
              if (half.length === 0) continue;
              const words = half.split("_").filter((w) => w.length > 0);
              expect(words.length).toBeLessThanOrEqual(4);
            }
          }
        }
      });
    });
  }
});

describe("UKI v5.2 format -- round-trip", () => {
  for (const fixture of ALL_FIXTURES) {
    it(`parseUki(compressUki(x)) deep-equals x for ${fixture.name}`, () => {
      const compressed = compressUki(fixture.slots);
      const parsed = parseUki(compressed);
      expect(parsed).toEqual(fixture.slots);
    });

    it(`compressUki(parseUki(expected)) equals expected for ${fixture.name}`, () => {
      const parsed = parseUki(fixture.expected);
      const recompressed = compressUki(parsed);
      expect(recompressed).toBe(fixture.expected);
    });
  }
});

describe("UKI v5.2 format -- validator", () => {
  for (const fixture of ALL_FIXTURES) {
    it(`validateUki returns empty violations for ${fixture.name}`, () => {
      const violations = validateUki(fixture.expected);
      expect(violations).toEqual([]);
    });
  }

  it("flags bare CS-0.92 as invalid (R5)", () => {
    const bad = MINIMAL.expected.replace("CS-work_0.9", "CS-0.92");
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
    expect(violations.some((v) => v.toLowerCase().includes("cs"))).toBe(true);
  });

  it("flags ARTIFACTS missing the commit/branch/version/file prefix (R7)", () => {
    const bad = MINIMAL.expected.replace(
      "ARTIFACTS-branch_feat_bootstrap",
      "ARTIFACTS-some_other_token",
    );
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
    expect(violations.some((v) => v.toLowerCase().includes("artifact"))).toBe(true);
  });

  it("flags missing STANCE_COLLAPSE slot as invalid (R6)", () => {
    // Produce a string with only 11 slots by dropping STANCE_COLLAPSE
    const parts = MINIMAL.expected.split("|");
    parts.splice(8, 1); // remove STANCE_COLLAPSE at index 8
    const bad = parts.join("|");
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
  });

  it("flags SUMMARY over 140 chars as invalid (R3)", () => {
    const longSummary = "A".repeat(150);
    const bad = MINIMAL.expected.replace(
      "SUMMARY-Bootstrap_handoff-ready-low_risk",
      `SUMMARY-${longSummary}`,
    );
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
    expect(violations.some((v) => v.toLowerCase().includes("summary"))).toBe(true);
  });

  it("flags wrong slot order as invalid (R4)", () => {
    // Swap SESSION_CORE and CAUSAL_DRIVERS at the front
    const parts = MINIMAL.expected.split("|");
    [parts[0], parts[1]] = [parts[1]!, parts[0]!];
    const bad = parts.join("|");
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
  });

  it("flags strings containing colons as invalid", () => {
    const bad = MINIMAL.expected.replace(
      "SESSION_CORE-bootstrap_handoff",
      "SESSION_CORE-bootstrap:handoff",
    );
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
  });

  it("flags strings containing newlines as invalid", () => {
    const bad = MINIMAL.expected.replace(
      "SESSION_CORE-bootstrap_handoff",
      "SESSION_CORE-bootstrap\nhandoff",
    );
    const violations = validateUki(bad);
    expect(violations.length).toBeGreaterThan(0);
  });
});

describe("UKI v5.2 format -- compressor determinism", () => {
  it("produces byte-identical output for the same input across calls", () => {
    const first = compressUki(FULL.slots);
    const second = compressUki(FULL.slots);
    const third = compressUki(FULL.slots);
    expect(first).toBe(second);
    expect(second).toBe(third);
  });

  it("produces the same output regardless of object literal shape", () => {
    // Two structurally equal inputs must produce equal output. This exercises
    // the "no object-key iteration order dependence" rule.
    const slotsA: UkiSlots = {
      sessionCore: FULL.slots.sessionCore,
      causalDrivers: [...FULL.slots.causalDrivers],
      divergences: [...FULL.slots.divergences],
      keyDecisions: [...FULL.slots.keyDecisions],
      signalDelta: [...FULL.slots.signalDelta],
      artifacts: [...FULL.slots.artifacts],
      executionState: FULL.slots.executionState,
      boundaryState: [...FULL.slots.boundaryState],
      stanceCollapse: FULL.slots.stanceCollapse,
      nextAction: FULL.slots.nextAction,
      cs: { ...FULL.slots.cs },
      summary: FULL.slots.summary,
    };
    const slotsB: UkiSlots = {
      // Deliberately declared in a scrambled order, but the interface field
      // values are the same.
      summary: FULL.slots.summary,
      cs: { ...FULL.slots.cs },
      nextAction: FULL.slots.nextAction,
      stanceCollapse: FULL.slots.stanceCollapse,
      boundaryState: [...FULL.slots.boundaryState],
      executionState: FULL.slots.executionState,
      artifacts: [...FULL.slots.artifacts],
      signalDelta: [...FULL.slots.signalDelta],
      keyDecisions: [...FULL.slots.keyDecisions],
      divergences: [...FULL.slots.divergences],
      causalDrivers: [...FULL.slots.causalDrivers],
      sessionCore: FULL.slots.sessionCore,
    };
    expect(compressUki(slotsA)).toBe(compressUki(slotsB));
  });
});
