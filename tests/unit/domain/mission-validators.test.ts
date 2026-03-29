import { describe, expect, it } from "bun:test";
import { ZodError } from "zod";
import {
  validateMission,
  validateMilestone,
  validateFeature,
  validateAssertion,
  validateCheckpoint,
  validateCreateMissionInput,
  validateCreateFeatureInput,
  validateCreateAssertionInput,
  validateUpdateAssertionInput,
  assertNoDanglingReferences,
  assertNoCyclicDependencies,
} from "../../../src/domain/mission-validators.js";
import type {
  Mission,
  Milestone,
  Feature,
  Assertion,
  Checkpoint,
  CreateMissionInput,
  CreateFeatureInput,
  CreateAssertionInput,
  UpdateAssertionInput,
} from "../../../src/domain/mission-types.js";
import { MaestroError } from "../../../src/domain/errors.js";

// Test data builders
const makeMission = (overrides: Partial<Mission> = {}): Mission => ({
  id: "2026-03-28-001",
  status: "draft",
  title: "Test Mission",
  description: "A test mission",
  milestones: [
    { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
  ],
  features: ["f1"],
  createdAt: "2026-03-28T12:00:00Z",
  updatedAt: "2026-03-28T12:00:00Z",
  ...overrides,
});

const makeMilestone = (overrides: Partial<Milestone> = {}): Milestone => ({
  id: "m1",
  title: "Milestone 1",
  description: "First milestone",
  order: 0,
  ...overrides,
});

const makeFeature = (overrides: Partial<Feature> = {}): Feature => ({
  id: "f1",
  missionId: "2026-03-28-001",
  milestoneId: "m1",
  status: "pending",
  title: "Test Feature",
  description: "A test feature",
  skillName: "test-skill",
  verificationSteps: ["step 1"],
  dependsOn: [],
  createdAt: "2026-03-28T12:00:00Z",
  updatedAt: "2026-03-28T12:00:00Z",
  ...overrides,
});

const makeAssertion = (overrides: Partial<Assertion> = {}): Assertion => ({
  id: "a1",
  missionId: "2026-03-28-001",
  milestoneId: "m1",
  featureId: "f1",
  status: "pending",
  description: "An assertion",
  createdAt: "2026-03-28T12:00:00Z",
  updatedAt: "2026-03-28T12:00:00Z",
  ...overrides,
});

const makeCheckpoint = (overrides: Partial<Checkpoint> = {}): Checkpoint => ({
  id: "cp1",
  missionId: "2026-03-28-001",
  milestoneId: "m1",
  timestamp: "2026-03-28T12:00:00Z",
  featureStates: { f1: "pending" },
  assertionStates: { a1: "pending" },
  ...overrides,
});

describe("mission validators", () => {
  describe("validateMission", () => {
    it("accepts a valid mission", () => {
      const mission = makeMission();
      const result = validateMission(mission);
      expect(result.id).toBe("2026-03-28-001");
    });

    it("rejects invalid ID format", () => {
      expect(() => validateMission(makeMission({ id: "bad-id" }))).toThrow(ZodError);
    });

    it("rejects invalid status", () => {
      expect(() => validateMission(makeMission({ status: "invalid" as any }))).toThrow(ZodError);
    });

    it("rejects empty title", () => {
      expect(() => validateMission(makeMission({ title: "" }))).toThrow(ZodError);
    });

    it("accepts all valid statuses", () => {
      const validStatuses: Mission["status"][] = ["draft", "approved", "rejected", "executing", "validating", "completed", "failed"];
      for (const status of validStatuses) {
        const result = validateMission(makeMission({ status }));
        expect(result.status).toBe(status);
      }
    });
  });

  describe("validateMilestone", () => {
    it("accepts a valid milestone", () => {
      const milestone = makeMilestone();
      const result = validateMilestone(milestone);
      expect(result.id).toBe("m1");
    });

    it("rejects empty ID", () => {
      expect(() => validateMilestone(makeMilestone({ id: "" }))).toThrow(ZodError);
    });

    it("rejects negative order", () => {
      expect(() => validateMilestone(makeMilestone({ order: -1 }))).toThrow(ZodError);
    });
  });

  describe("validateFeature", () => {
    it("accepts a valid feature", () => {
      const feature = makeFeature();
      const result = validateFeature(feature);
      expect(result.id).toBe("f1");
    });

    it("rejects invalid status", () => {
      expect(() => validateFeature(makeFeature({ status: "invalid" as any }))).toThrow(ZodError);
    });

    it("accepts all valid statuses", () => {
      const validStatuses: Feature["status"][] = ["pending", "in_progress", "in_review", "completed", "blocked"];
      for (const status of validStatuses) {
        const result = validateFeature(makeFeature({ status }));
        expect(result.status).toBe(status);
      }
    });

    it("rejects empty verificationSteps array", () => {
      expect(() => validateFeature(makeFeature({ verificationSteps: [] }))).toThrow(ZodError);
    });
  });

  describe("validateAssertion", () => {
    it("accepts a valid assertion", () => {
      const assertion = makeAssertion();
      const result = validateAssertion(assertion);
      expect(result.id).toBe("a1");
    });

    it("rejects invalid status", () => {
      expect(() => validateAssertion(makeAssertion({ status: "invalid" as any }))).toThrow(ZodError);
    });

    it("accepts all valid statuses including waived", () => {
      const validStatuses: Assertion["status"][] = ["pending", "passed", "failed", "blocked", "waived"];
      for (const status of validStatuses) {
        const result = validateAssertion(makeAssertion({ status }));
        expect(result.status).toBe(status);
      }
    });

    it("accepts assertion with waivedReason when waived", () => {
      const assertion = makeAssertion({ status: "waived", waivedReason: "Not applicable" });
      const result = validateAssertion(assertion);
      expect(result.waivedReason).toBe("Not applicable");
    });

    it("rejects empty waivedReason when waived", () => {
      expect(() => validateAssertion(makeAssertion({ status: "waived", waivedReason: "" }))).toThrow(ZodError);
    });
  });

  describe("validateCheckpoint", () => {
    it("accepts a valid checkpoint", () => {
      const checkpoint = makeCheckpoint();
      const result = validateCheckpoint(checkpoint);
      expect(result.id).toBe("cp1");
    });

    it("rejects invalid timestamp", () => {
      expect(() => validateCheckpoint(makeCheckpoint({ timestamp: "not-a-date" }))).toThrow(ZodError);
    });
  });

  describe("validateCreateMissionInput", () => {
    it("accepts valid create input", () => {
      const input: CreateMissionInput = {
        title: "New Mission",
        description: "A new mission",
        milestones: [makeMilestone()],
      };
      const result = validateCreateMissionInput(input);
      expect(result.title).toBe("New Mission");
    });

    it("rejects empty title", () => {
      const input: CreateMissionInput = {
        title: "",
        description: "A new mission",
        milestones: [makeMilestone()],
      };
      expect(() => validateCreateMissionInput(input)).toThrow(ZodError);
    });

    it("rejects milestones with duplicate IDs", () => {
      const input: CreateMissionInput = {
        title: "New Mission",
        description: "A new mission",
        milestones: [makeMilestone({ id: "m1" }), makeMilestone({ id: "m1" })],
      };
      expect(() => validateCreateMissionInput(input)).toThrow(ZodError);
    });
  });

  describe("validateCreateFeatureInput", () => {
    it("accepts valid create input", () => {
      const input: CreateFeatureInput = {
        missionId: "2026-03-28-001",
        milestoneId: "m1",
        title: "New Feature",
        description: "A new feature",
        skillName: "test-skill",
        verificationSteps: ["step 1"],
      };
      const result = validateCreateFeatureInput(input);
      expect(result.title).toBe("New Feature");
    });

    it("rejects empty verificationSteps", () => {
      const input: CreateFeatureInput = {
        missionId: "2026-03-28-001",
        milestoneId: "m1",
        title: "New Feature",
        description: "A new feature",
        skillName: "test-skill",
        verificationSteps: [],
      };
      expect(() => validateCreateFeatureInput(input)).toThrow(ZodError);
    });
  });

  describe("validateCreateAssertionInput", () => {
    it("accepts valid create input", () => {
      const input: CreateAssertionInput = {
        missionId: "2026-03-28-001",
        milestoneId: "m1",
        featureId: "f1",
        description: "An assertion",
      };
      const result = validateCreateAssertionInput(input);
      expect(result.description).toBe("An assertion");
    });

    it("rejects empty description", () => {
      const input: CreateAssertionInput = {
        missionId: "2026-03-28-001",
        milestoneId: "m1",
        featureId: "f1",
        description: "",
      };
      expect(() => validateCreateAssertionInput(input)).toThrow(ZodError);
    });
  });

  describe("validateUpdateAssertionInput", () => {
    it("accepts valid update input with result", () => {
      const input: UpdateAssertionInput = {
        status: "passed",
        evidence: "Test passed",
      };
      const result = validateUpdateAssertionInput(input);
      expect(result.status).toBe("passed");
    });

    it("accepts valid update input with waive reason", () => {
      const input: UpdateAssertionInput = {
        status: "waived",
        waivedReason: "Not applicable",
      };
      const result = validateUpdateAssertionInput(input);
      expect(result.waivedReason).toBe("Not applicable");
    });

    it("rejects waived without waivedReason", () => {
      const input: UpdateAssertionInput = {
        status: "waived",
      };
      expect(() => validateUpdateAssertionInput(input)).toThrow(ZodError);
    });
  });

  describe("assertNoDanglingReferences", () => {
    it("throws MaestroError for dangling milestone reference in feature", () => {
      const mission = makeMission({ milestones: [makeMilestone({ id: "m1" })] });
      const features: Feature[] = [makeFeature({ milestoneId: "nonexistent" })];
      expect(() => assertNoDanglingReferences(mission, features, [])).toThrow(MaestroError);
    });

    it("throws MaestroError for dangling feature reference in mission", () => {
      const mission = makeMission({ features: ["nonexistent"], milestones: [makeMilestone({ id: "m1" })] });
      const features: Feature[] = [makeFeature({ id: "f1", milestoneId: "m1" })];
      expect(() => assertNoDanglingReferences(mission, features, [])).toThrow(MaestroError);
    });

    it("throws MaestroError for dangling assertion feature reference", () => {
      const mission = makeMission({ milestones: [makeMilestone({ id: "m1" })] });
      const features: Feature[] = [makeFeature({ id: "f1", milestoneId: "m1" })];
      const assertions: Assertion[] = [makeAssertion({ featureId: "nonexistent", milestoneId: "m1" })];
      expect(() => assertNoDanglingReferences(mission, features, assertions)).toThrow(MaestroError);
    });

    it("does not throw when all references are valid", () => {
      const mission = makeMission({ milestones: [makeMilestone({ id: "m1" })], features: ["f1"] });
      const features: Feature[] = [makeFeature({ id: "f1", milestoneId: "m1" })];
      const assertions: Assertion[] = [makeAssertion({ featureId: "f1", milestoneId: "m1" })];
      expect(() => assertNoDanglingReferences(mission, features, assertions)).not.toThrow();
    });
  });

  describe("assertNoCyclicDependencies", () => {
    it("throws MaestroError for self-referencing feature", () => {
      const features: Feature[] = [makeFeature({ id: "f1", milestoneId: "m1", dependsOn: ["f1"] })];
      expect(() => assertNoCyclicDependencies(features)).toThrow(MaestroError);
    });

    it("throws MaestroError for cyclic dependency chain", () => {
      const features: Feature[] = [
        makeFeature({ id: "f1", milestoneId: "m1", dependsOn: ["f2"] }),
        makeFeature({ id: "f2", milestoneId: "m1", dependsOn: ["f1"] }),
      ];
      expect(() => assertNoCyclicDependencies(features)).toThrow(MaestroError);
    });

    it("throws MaestroError for longer cyclic chain", () => {
      const features: Feature[] = [
        makeFeature({ id: "f1", milestoneId: "m1", dependsOn: ["f2"] }),
        makeFeature({ id: "f2", milestoneId: "m1", dependsOn: ["f3"] }),
        makeFeature({ id: "f3", milestoneId: "m1", dependsOn: ["f1"] }),
      ];
      expect(() => assertNoCyclicDependencies(features)).toThrow(MaestroError);
    });

    it("does not throw for acyclic dependencies", () => {
      const features: Feature[] = [
        makeFeature({ id: "f1", milestoneId: "m1", dependsOn: [] }),
        makeFeature({ id: "f2", milestoneId: "m1", dependsOn: ["f1"] }),
        makeFeature({ id: "f3", milestoneId: "m1", dependsOn: ["f1", "f2"] }),
      ];
      expect(() => assertNoCyclicDependencies(features)).not.toThrow();
    });

    it("throws MaestroError for dangling dependency reference", () => {
      const features: Feature[] = [makeFeature({ id: "f1", milestoneId: "m1", dependsOn: ["nonexistent"] })];
      expect(() => assertNoCyclicDependencies(features)).toThrow(MaestroError);
    });
  });
});
