/**
 * Unit tests for mission lifecycle usecases
 */
import { describe, expect, it, beforeEach } from "bun:test";
import {
  createMission,
  expandWorkflowTemplate,
  listMissions,
  showMission,
  approveMission,
  rejectMission,
  updateMission,
} from "@/usecases/mission-lifecycle.usecase.js";
import { FsMissionStoreAdapter } from "@/adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "@/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "@/adapters/assertion-store.adapter.js";
import { MaestroError } from "@/domain/errors.js";
import type { MilestoneInput } from "@/domain/mission-types.js";
import type { WorkflowTemplate } from "@/domain/types.js";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp } from "node:fs/promises";

describe("mission lifecycle usecases", () => {
  let tmpDir: string;
  let missionStore: FsMissionStoreAdapter;
  let featureStore: FsFeatureStoreAdapter;
  let assertionStore: FsAssertionStoreAdapter;

      const sampleMilestones: MilestoneInput[] = [
        { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
        { id: "m2", title: "Milestone 2", description: "Second milestone", order: 1 },
      ];

  const samplePlan = {
    title: "Test Mission",
    description: "A test mission",
    milestones: sampleMilestones,
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "First feature",
        workerType: "test-skill",
        verificationSteps: ["step1", "step2"] as const,
        dependsOn: [],
        fulfills: ["assertion1", "assertion2"],
      },
      {
        id: "f2",
        milestoneId: "m2",
        title: "Feature 2",
        description: "Second feature",
        workerType: "test-skill",
        verificationSteps: ["step3"] as const,
        dependsOn: ["f1"],
      },
    ],
  };

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "mission-test-"));
    missionStore = new FsMissionStoreAdapter(tmpDir);
    featureStore = new FsFeatureStoreAdapter(tmpDir);
    assertionStore = new FsAssertionStoreAdapter(tmpDir);
  });

  describe("createMission", () => {
    it("creates a mission with generated ID from plan file", async () => {
      const result = await createMission(
        missionStore,
        featureStore,
        assertionStore,
        samplePlan,
      );

      expect(result.mission).toBeDefined();
      expect(result.mission.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
      expect(result.mission.title).toBe("Test Mission");
      expect(result.mission.status).toBe("draft");
      expect(result.mission.milestones).toHaveLength(2);
      expect(result.features).toHaveLength(2);
    });

    it("creates features with correct data", async () => {
      const result = await createMission(
        missionStore,
        featureStore,
        assertionStore,
        samplePlan,
      );

      const f1 = result.features.find((f) => f.id === "f1");
      expect(f1).toBeDefined();
      expect(f1?.missionId).toBe(result.mission.id);
      expect(f1?.milestoneId).toBe("m1");
      expect(f1?.workerType).toBe("test-skill");
      expect(f1?.verificationSteps).toEqual(["step1", "step2"]);
    });

    it("creates assertions for features with fulfills", async () => {
      const result = await createMission(
        missionStore,
        featureStore,
        assertionStore,
        samplePlan,
      );

      const assertions = await assertionStore.list(result.mission.id);
      expect(assertions).toHaveLength(2);
      expect(assertions[0]?.featureId).toBe("f1");
      expect(assertions[0]?.result).toBe("pending");
    });

    it("handles features without fulfills", async () => {
      const plan = {
        ...samplePlan,
        features: [
          {
            id: "f3",
            milestoneId: "m1",
            title: "Feature 3",
            description: "No assertions",
            workerType: "test-skill",
            verificationSteps: ["step"],
          },
        ],
      };

      const result = await createMission(
        missionStore,
        featureStore,
        assertionStore,
        plan,
      );

      const assertions = await assertionStore.list(result.mission.id);
      expect(assertions).toHaveLength(0);
    });

    it("rejects duplicate milestone IDs", async () => {
      const plan = {
        ...samplePlan,
        milestones: [
          { id: "dup", title: "One", description: "First", order: 0 },
          { id: "dup", title: "Two", description: "Second", order: 1 },
        ],
      };

      expect(
        createMission(missionStore, featureStore, assertionStore, plan),
      ).rejects.toThrow("Milestone IDs must be unique");
    });

    it("rejects dangling milestone references in features", async () => {
      const plan = {
        ...samplePlan,
        features: [
          {
            id: "bad",
            milestoneId: "nonexistent",
            title: "Bad Feature",
            description: "References invalid milestone",
            workerType: "test-skill",
            verificationSteps: ["step"],
          },
        ],
      };

      expect(
        createMission(missionStore, featureStore, assertionStore, plan),
      ).rejects.toThrow("references non-existent milestone");
    });

    it("rejects duplicate feature IDs", async () => {
      const f1 = samplePlan.features[0]!;
      const plan = {
        ...samplePlan,
        features: [f1, f1], // duplicate
      };

      expect(
        createMission(missionStore, featureStore, assertionStore, plan),
      ).rejects.toThrow("Duplicate feature ID");
    });

    it("rejects cyclic dependencies", async () => {
      const plan = {
        ...samplePlan,
        features: [
          {
            id: "a",
            milestoneId: "m1",
            title: "A",
            description: "Depends on B",
            workerType: "test-skill",
            verificationSteps: ["step"] as const,
            dependsOn: ["b"] as const,
          },
          {
            id: "b",
            milestoneId: "m1",
            title: "B",
            description: "Depends on A",
            workerType: "test-skill",
            verificationSteps: ["step"] as const,
            dependsOn: ["a"] as const,
          },
        ],
      };

      expect(
        createMission(missionStore, featureStore, assertionStore, plan),
      ).rejects.toThrow("Cyclic dependency");
    });

    it("stores mission in .maestro/missions/{id}", async () => {
      const result = await createMission(
        missionStore,
        featureStore,
        assertionStore,
        samplePlan,
      );

      const retrieved = await missionStore.get(result.mission.id);
      expect(retrieved).toBeDefined();
      expect(retrieved?.id).toBe(result.mission.id);
    });
  });

  describe("listMissions", () => {
    it("returns empty array when no missions", async () => {
      const missions = await listMissions(missionStore);
      expect(missions).toHaveLength(0);
    });

    it("lists all missions sorted by date", async () => {
      await createMission(missionStore, featureStore, assertionStore, samplePlan);
      await createMission(missionStore, featureStore, assertionStore, {
        ...samplePlan,
        title: "Second Mission",
      });

      const missions = await listMissions(missionStore);
      expect(missions).toHaveLength(2);
      // Newest first
      expect(missions[0]?.title).toBe("Second Mission");
      expect(missions[1]?.title).toBe("Test Mission");
    });

    it("filters by status", async () => {
      const result1 = await createMission(missionStore, featureStore, assertionStore, samplePlan);
      const result2 = await createMission(missionStore, featureStore, assertionStore, {
        ...samplePlan,
        title: "Second Mission",
      });

      // Approve the second mission
      await approveMission(missionStore, result2.mission.id);

      const drafts = await listMissions(missionStore, { status: "draft" });
      expect(drafts).toHaveLength(1);
      expect(drafts[0]?.id).toBe(result1.mission.id);

      const approved = await listMissions(missionStore, { status: "approved" });
      expect(approved).toHaveLength(1);
      expect(approved[0]?.id).toBe(result2.mission.id);
    });
  });

  describe("showMission", () => {
    it("returns mission by ID", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);

      const mission = await showMission(missionStore, created.mission.id);
      expect(mission).toBeDefined();
      expect(mission?.title).toBe("Test Mission");
    });

    it("returns undefined for non-existent mission", async () => {
      const mission = await showMission(missionStore, "2026-03-28-001");
      expect(mission).toBeUndefined();
    });
  });

  describe("approveMission", () => {
    it("transitions draft mission to approved", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);
      expect(created.mission.status).toBe("draft");

      const approved = await approveMission(missionStore, created.mission.id);
      expect(approved.status).toBe("approved");
      expect(approved.approvedAt).toBeDefined();
    });

    it("throws for non-existent mission", async () => {
      expect(
        approveMission(missionStore, "2026-03-28-001"),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

    it("throws with helpful hints for invalid transitions", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);
      await approveMission(missionStore, created.mission.id);

      // Try to approve again - should fail
      let threw = false;
      try {
        await approveMission(missionStore, created.mission.id);
      } catch (err) {
        threw = true;
        expect(err).toBeInstanceOf(MaestroError);
        const me = err as MaestroError;
        expect(me.message).toContain("Invalid mission transition");
        expect(me.hints.length).toBeGreaterThan(0);
        expect(me.hints.some((h) => h.includes("approved"))).toBe(true);
      }
      expect(threw).toBe(true);
    });
  });

  describe("rejectMission", () => {
    it("transitions draft mission to rejected", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);

      const rejected = await rejectMission(missionStore, created.mission.id);
      expect(rejected.status).toBe("rejected");
      expect(rejected.rejectedAt).toBeDefined();
    });

    it("throws for non-existent mission", async () => {
      expect(
        rejectMission(missionStore, "2026-03-28-001"),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

    it("throws with helpful hints for invalid transitions", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);
      await approveMission(missionStore, created.mission.id);

      // Try to reject an approved mission - should fail
      let threw = false;
      try {
        await rejectMission(missionStore, created.mission.id);
      } catch (err) {
        threw = true;
        expect(err).toBeInstanceOf(MaestroError);
        const me = err as MaestroError;
        expect(me.message).toContain("Invalid mission transition");
        expect(me.hints.length).toBeGreaterThan(0);
      }
      expect(threw).toBe(true);
    });
  });

    describe("updateMission", () => {
    it("updates mission status", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);
      await approveMission(missionStore, created.mission.id);

      const updated = await updateMission(missionStore, created.mission.id, {
        status: "executing",
      });
      expect(updated.status).toBe("executing");
    });

    it("updates mission title", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);

      const updated = await updateMission(missionStore, created.mission.id, {
        title: "Updated Title",
      });
      expect(updated.title).toBe("Updated Title");
      expect(updated.status).toBe("draft"); // unchanged
    });

    it("updates mission description", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);

      const updated = await updateMission(missionStore, created.mission.id, {
        description: "Updated description",
      });
      expect(updated.description).toBe("Updated description");
    });

    it("validates legal transitions on status update", async () => {
      const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);

      // Cannot go directly from draft to executing (must go through approved)
      expect(
        updateMission(missionStore, created.mission.id, { status: "executing" }),
      ).rejects.toThrow("Invalid mission transition");
    });

    it("throws for non-existent mission", async () => {
      expect(
        updateMission(missionStore, "2026-03-28-001", { title: "New Title" }),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

      it("allows same status update (no-op)", async () => {
        const created = await createMission(missionStore, featureStore, assertionStore, samplePlan);

      const updated = await updateMission(missionStore, created.mission.id, {
        status: "draft",
      });
      expect(updated.status).toBe("draft");
        expect(updated.updatedAt).not.toBe(created.mission.updatedAt);
      });
    });

    describe("expandWorkflowTemplate", () => {
      it("rejects magic prototype keys as unknown templates", () => {
        expect(() => expandWorkflowTemplate("__proto__", { workflowTemplates: {} })).toThrow(
          "Unknown workflow template: __proto__",
        );
      });

      it("rejects malformed custom workflow templates with a structured error", () => {
        expect(() => expandWorkflowTemplate("broken", {
          workflowTemplates: {
              broken: {
                description: "Broken",
                phases: [{ kind: "work" }],
              } as unknown as WorkflowTemplate,
            },
          })).toThrow("Invalid workflow template 'broken'");
        });

      it("normalizes legacy custom workflow templates with omitted kind and extra metadata", () => {
        const milestones = expandWorkflowTemplate("legacy", {
          workflowTemplates: {
            legacy: {
              phases: [
                { label: "Planning", profile: "planning" },
                { label: "Implementation", profile: "implementation", notes: "ignored" },
              ],
              notes: "ignored",
            } as unknown as WorkflowTemplate,
          },
        });

        expect(milestones).toEqual([
          {
            id: "planning",
            title: "Planning",
            description: "Planning",
            order: 0,
            kind: "work",
            profile: "planning",
          },
          {
            id: "implementation",
            title: "Implementation",
            description: "Implementation",
            order: 1,
            kind: "work",
            profile: "implementation",
          },
        ]);
      });
    });
  });
