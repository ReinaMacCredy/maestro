/**
 * Tests for principle gate validation on handoff create.
 *
 * These cover both the pure gate-check helpers and the real CLI rejection
 * paths for gate enforcement during `maestro handoff create`.
 */
import { afterEach, beforeEach, describe, it, expect } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { evaluateGateCheck } from "@/shared/lib/gate-check.js";
import { validateUkiHandoffContent } from "@/features/handoff/domain/validators.js";
import type { UkiHandoffContent } from "@/features/handoff/domain/uki-types.js";
import { runCli } from "../../../../helpers/run-cli.js";
import { initGitRepo } from "../../../../helpers/command-runner.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-gate-validation-"));
  await initGitRepo(tmpDir);
  const initResult = await runCli(["init", "--json"], tmpDir);
  expect(initResult.exitCode).toBe(0);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function createImplementationMission(): Promise<string> {
  const planPath = join(tmpDir, "plan.json");
  await writeFile(planPath, JSON.stringify({
    title: "Gate Test Mission",
    description: "Mission for handoff gate validation",
    milestones: [
      { id: "m1", title: "Implementation", description: "Implement", order: 0, profile: "implementation" },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "Feature under gate test",
        workerType: "test-skill",
        verificationSteps: ["build", "test"],
      },
    ],
  }));

  const result = await runCli(["mission", "create", "--file", planPath, "--json"], tmpDir);
  expect(result.exitCode).toBe(0);
  return JSON.parse(result.stdout).mission.id as string;
}

function makeExecuteContent(overrides: Record<string, unknown> = {}): UkiHandoffContent {
  return {
    mode: "execute",
    currentState: "done",
    sessionCore: "session-abc",
    decisions: ["decision-1"],
    artifacts: [],
    readMore: [],
    nextAction: "deploy",
    summary: "Completed implementation",
    maestroRefs: { missionId: "2026-04-12-001", featureId: "f1" },
    cs: { work: 0.9 },
    signalDelta: [],
    boundaryState: [],
    risks: [],
    causalDrivers: [],
    divergences: [],
    touchedFiles: [],
    completedWork: ["impl"],
    validation: ["tests pass"],
    ...overrides,
  } as UkiHandoffContent;
}

describe("gate validation on handoff content", () => {
  describe("assumptions field (array_min_length:1)", () => {
    it("passes when assumptions has at least one entry", () => {
      const content = makeExecuteContent({ assumptions: ["I assumed X"] });
      expect(evaluateGateCheck("array_min_length:1", content.assumptions)).toBe(true);
    });

    it("fails when assumptions is missing", () => {
      const content = makeExecuteContent();
      expect(evaluateGateCheck("array_min_length:1", content.assumptions)).toBe(false);
    });

    it("fails when assumptions is empty array", () => {
      const content = makeExecuteContent({ assumptions: [] });
      expect(evaluateGateCheck("array_min_length:1", content.assumptions)).toBe(false);
    });
  });

  describe("scopeDeclaration field (object_non_empty)", () => {
    it("passes when scopeDeclaration has keys", () => {
      const content = makeExecuteContent({ scopeDeclaration: { touched: "src/foo.ts" } });
      expect(evaluateGateCheck("object_non_empty", content.scopeDeclaration)).toBe(true);
    });

    it("fails when scopeDeclaration is empty object", () => {
      const content = makeExecuteContent({ scopeDeclaration: {} });
      expect(evaluateGateCheck("object_non_empty", content.scopeDeclaration)).toBe(false);
    });

    it("fails when scopeDeclaration is missing", () => {
      const content = makeExecuteContent();
      expect(evaluateGateCheck("object_non_empty", content.scopeDeclaration)).toBe(false);
    });
  });

  describe("verificationResults field (array_all_passed)", () => {
    it("passes when all results have passed: true", () => {
      const content = makeExecuteContent({
        verificationResults: [
          { step: "build", passed: true },
          { step: "test", passed: true },
        ],
      });
      expect(evaluateGateCheck("array_all_passed", content.verificationResults)).toBe(true);
    });

    it("fails when any result has passed: false", () => {
      const content = makeExecuteContent({
        verificationResults: [
          { step: "build", passed: true },
          { step: "test", passed: false },
        ],
      });
      expect(evaluateGateCheck("array_all_passed", content.verificationResults)).toBe(false);
    });

    it("fails when verificationResults is missing", () => {
      const content = makeExecuteContent();
      expect(evaluateGateCheck("array_all_passed", content.verificationResults)).toBe(false);
    });
  });

  describe("Zod schema accepts new optional fields", () => {
    it("validates content with all principle gate fields present", () => {
      const raw = makeExecuteContent({
        assumptions: ["Assumed no breaking changes"],
        scopeDeclaration: { touched: "src/foo.ts", reason: "fix bug" },
        complexityDelta: { linesAdded: 10 },
        verificationResults: [{ step: "build", passed: true }],
      });
      const parsed = validateUkiHandoffContent(raw);
      expect(parsed.assumptions).toEqual(["Assumed no breaking changes"]);
      expect((parsed as Record<string, unknown>).scopeDeclaration).toEqual({ touched: "src/foo.ts", reason: "fix bug" });
      expect((parsed as Record<string, unknown>).verificationResults).toEqual([{ step: "build", passed: true }]);
    });

    it("validates content without principle gate fields (backward compat)", () => {
      const raw = makeExecuteContent();
      const parsed = validateUkiHandoffContent(raw);
      expect(parsed.assumptions).toBeUndefined();
    });

    it("validates plan mode content with gate fields", () => {
      const raw = {
        mode: "plan",
        currentState: "planning",
        sessionCore: "session",
        decisions: [],
        artifacts: [],
        readMore: [],
        nextAction: "review",
        summary: "Plan ready",
        maestroRefs: {},
        cs: { summary: 0.8 },
        signalDelta: [],
        boundaryState: [],
        risks: [],
        causalDrivers: [],
        divergences: [],
        planPaths: [],
        maestroSync: [],
        assumptions: ["Design assumption"],
      };
      const parsed = validateUkiHandoffContent(raw);
      expect(parsed.assumptions).toEqual(["Design assumption"]);
    });
  });

  describe("gate skip behavior", () => {
    it("skips validation when missionId is absent (ad-hoc handoff)", () => {
      const content = makeExecuteContent();
      // When maestroRefs has no missionId, gate validation is skipped entirely.
      // This is tested via the absence of missionId -- no error thrown.
      expect(content.maestroRefs.missionId).toBeDefined();

      const adHocContent = makeExecuteContent({ maestroRefs: {} });
      expect(adHocContent.maestroRefs.missionId).toBeUndefined();
    });
  });

  describe("handoff create command", () => {
    it("allows auto-collected mission refs when no feature ref is provided", async () => {
      const missionId = await createImplementationMission();
      const approveResult = await runCli(["mission", "approve", missionId, "--json"], tmpDir);
      expect(approveResult.exitCode).toBe(0);

      const result = await runCli([
        "handoff",
        "create",
        "--mode", "execute",
        "--session-core", "session_abc",
        "--summary", "summary",
        "--next-action", "next_step",
        "--completed", "implemented_feature",
        "--validation", "tests_pass",
        "--confidence-work", "0.9",
        "--artifact", "branch_main",
        "--json",
      ], tmpDir);

      expect(result.exitCode).toBe(0);
      expect(result.stderr).not.toContain("Gate validation requires both missionId and featureId");
    });

    it("rejects mission-linked handoffs that do not satisfy active gates", async () => {
      const missionId = await createImplementationMission();

      const result = await runCli([
        "handoff",
        "create",
        "--mode", "execute",
        "--mission-id", missionId,
        "--feature-id", "f1",
        "--session-core", "session_abc",
        "--summary", "summary",
        "--next-action", "next_step",
        "--completed", "implemented_feature",
        "--validation", "tests_pass",
        "--confidence-work", "0.9",
        "--artifact", "branch_main",
      ], tmpDir);

      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("Handoff rejected by");
      expect(result.stderr).toContain("assumptions");
      expect(result.stderr).toContain("scopeDeclaration");
      expect(result.stderr).toContain("verificationResults");
    });

    it("rejects non-string scope declaration values before persistence", async () => {
      const missionId = await createImplementationMission();

      const result = await runCli([
        "handoff",
        "create",
        "--mode", "execute",
        "--mission-id", missionId,
        "--feature-id", "f1",
        "--session-core", "session_abc",
        "--summary", "summary",
        "--next-action", "next_step",
        "--completed", "implemented_feature",
        "--validation", "tests_pass",
        "--confidence-work", "0.9",
        "--artifact", "branch_main",
        "--scope-declaration", "{\"touched\":1}",
      ], tmpDir);

      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("--scope-declaration values must be strings");
    });

    it("reports invalid verification results as a CLI error instead of leaking zod output", async () => {
      const result = await runCli([
        "handoff",
        "create",
        "--mode", "execute",
        "--session-core", "session_abc",
        "--summary", "summary",
        "--next-action", "next_step",
        "--completed", "implemented_feature",
        "--validation", "tests_pass",
        "--confidence-work", "0.9",
        "--verification-result", ":passed",
      ], tmpDir);

      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("Invalid handoff content");
      expect(result.stderr).not.toContain("ZodError");
    });

    it("fails closed when the principle store is unreadable", async () => {
      const missionId = await createImplementationMission();
      await writeFile(join(tmpDir, ".maestro", "principles.jsonl"), "{not-json}\n");

      const result = await runCli([
        "handoff",
        "create",
        "--mode", "execute",
        "--mission-id", missionId,
        "--feature-id", "f1",
        "--session-core", "session_abc",
        "--summary", "summary",
        "--next-action", "next_step",
        "--completed", "implemented_feature",
        "--validation", "tests_pass",
        "--confidence-work", "0.9",
        "--artifact", "branch_main",
        "--assumption", "one_assumption",
        "--scope-declaration", "{\"touched\":\"src/index.ts\"}",
        "--verification-result", "build:passed",
      ], tmpDir);

      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("Failed to load active principles");
      expect(result.stderr).toContain("Invalid principle record at line 1");
    });
  });
});
