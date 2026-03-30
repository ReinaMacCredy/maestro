/**
 * Unit tests for feature lifecycle usecases
 */
import { describe, expect, it, beforeEach } from "bun:test";
import {
  listFeatures,
  updateFeature,
  parseWorkerReport,
} from "../../../src/usecases/feature-lifecycle.usecase.js";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import { MaestroError } from "../../../src/domain/errors.js";
import type { Milestone } from "../../../src/domain/mission-types.js";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp, writeFile, mkdir } from "node:fs/promises";

async function createSampleMission(
  missionStore: FsMissionStoreAdapter,
  featureStore: FsFeatureStoreAdapter,
  assertionStore: FsAssertionStoreAdapter,
  tmpDir: string,
): Promise<{ missionId: string; features: string[] }> {
  const sampleMilestones: Milestone[] = [
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
        verificationSteps: ["step1", "step2"],
        dependsOn: [],
        fulfills: ["assertion1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        workerType: "test-skill",
        verificationSteps: ["step3"],
        dependsOn: ["f1"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Feature 3",
        description: "Third feature",
        workerType: "test-skill",
        verificationSteps: ["step4"],
        dependsOn: [],
      },
    ],
  };

  // Import the createMission function to set up test data
  const { createMission } = await import("../../../src/usecases/mission-lifecycle.usecase.js");
  const result = await createMission(missionStore, featureStore, assertionStore, samplePlan);

  return {
    missionId: result.mission.id,
    features: result.features.map((f) => f.id),
  };
}

describe("feature lifecycle usecases", () => {
  let tmpDir: string;
  let missionStore: FsMissionStoreAdapter;
  let featureStore: FsFeatureStoreAdapter;
  let assertionStore: FsAssertionStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "feature-test-"));
    missionStore = new FsMissionStoreAdapter(tmpDir);
    featureStore = new FsFeatureStoreAdapter(tmpDir);
    assertionStore = new FsAssertionStoreAdapter(tmpDir);
  });

  describe("listFeatures", () => {
    it("returns all features for a mission", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      const result = await listFeatures(missionStore, featureStore, missionId);

      expect(result.total).toBe(3);
      expect(result.filtered).toBe(3);
      expect(result.features).toHaveLength(3);
    });

    it("filters by milestone", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      const result = await listFeatures(missionStore, featureStore, missionId, {
        milestoneId: "m1",
      });

      expect(result.total).toBe(3);
      expect(result.filtered).toBe(2);
      expect(result.features.every((f) => f.milestoneId === "m1")).toBe(true);
    });

    it("filters by status", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      // First transition a feature to in_progress
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
      });

      const result = await listFeatures(missionStore, featureStore, missionId, {
        status: "in-progress",
      });

      expect(result.total).toBe(3);
      expect(result.filtered).toBe(1);
      expect(result.features[0]?.id).toBe("f1");
      expect(result.features[0]?.status).toBe("in-progress");
    });

    it("combines milestone and status filters", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      // Transition f1 (m1) to in_progress, f2 (m1) stays pending
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
      });

      const result = await listFeatures(missionStore, featureStore, missionId, {
        milestoneId: "m1",
        status: "in-progress",
      });

      expect(result.filtered).toBe(1);
      expect(result.features[0]?.id).toBe("f1");
    });

    it("throws for non-existent mission", async () => {
      expect(
        listFeatures(missionStore, featureStore, "2026-03-28-001"),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

    it("returns empty array when no features match filters", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      const result = await listFeatures(missionStore, featureStore, missionId, {
        status: "done",
      });

      expect(result.filtered).toBe(0);
      expect(result.features).toHaveLength(0);
    });
  });

  describe("updateFeature", () => {
    it("updates feature status with legal transition", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      const result = await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
      });

      expect(result.feature.status).toBe("in-progress");
      expect(result.feature.id).toBe("f1");
    });

    it("rejects illegal status transitions", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      // Cannot go from pending to in_review (must go through in_progress)
      expect(
        updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
          status: "review",
        }),
      ).rejects.toThrow("Invalid feature transition");
    });

    it("does not auto-start an approved mission when the feature transition is invalid", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      await missionStore.update(missionId, { status: "approved" });

      expect(
        updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
          status: "review",
        }),
      ).rejects.toThrow("Invalid feature transition");

      const mission = await missionStore.get(missionId);
      expect(mission?.status).toBe("approved");
    });

    it("allows retry from in_review to pending", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      // First move to in_progress, then in_review
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
      });
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "review",
      });

      // Retry: in_review -> pending
      const result = await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "pending",
      });

      expect(result.feature.status).toBe("pending");
    });

    it("allows retry from blocked to pending", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      // First move through the states: pending -> in_progress -> in_review -> blocked
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
      });
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "review",
      });
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "blocked",
      });

      // Retry: blocked -> pending
      const result = await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "pending",
      });

      expect(result.feature.status).toBe("pending");
    });

    it("attaches and persists worker report", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      const report = {
        content: "Feature implementation complete",
        timestamp: new Date().toISOString(),
        agent: "test-agent",
      };

      const result = await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
        report,
      });

      // parseWorkerReport converts legacy {content} to rich format
      expect(result.feature.report).toEqual({
        salientSummary: "Feature implementation complete",
        whatWasImplemented: "Feature implementation complete",
        whatWasLeftUndone: "",
        verification: { commandsRun: [], interactiveChecks: [] },
        tests: { added: [] },
        discoveredIssues: [],
      });
      expect(result.reportPersisted).toBeDefined();
      expect(result.reportPersisted).toContain("workers/f1/report.json");
    });

    it("preserves existing report when retrying without new report", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      // First attach a report (legacy format -- converted on parse)
      const report = {
        content: "Initial implementation",
        timestamp: new Date().toISOString(),
        agent: "agent-1",
      };

      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
        report,
      });

      // Move to in_review
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "review",
      });

      // Retry to pending WITHOUT providing a new report
      const result = await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "pending",
      });

      // Report should be preserved in rich format (legacy content -> salientSummary)
      expect(result.feature.status).toBe("pending");
      expect(result.feature.report).toBeDefined();
      expect(result.feature.report?.salientSummary).toBe("Initial implementation");
      expect(result.feature.report?.whatWasImplemented).toBe("Initial implementation");
    });

    it("records retry reasons only for real transitions back to pending", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "in-progress",
      });
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "review",
      });

      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "pending",
        retryReason: "First retry",
      });
      await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "pending",
        retryReason: "No-op retry",
      });

      const { readFile } = await import("node:fs/promises");
      const retryLogPath = join(tmpDir, ".maestro", "missions", missionId, "workers", "f1", "retry-log.json");
      const retryLog = JSON.parse(await readFile(retryLogPath, "utf8"));

      expect(retryLog).toHaveLength(1);
      expect(retryLog[0].reason).toBe("First retry");
      expect(retryLog[0].previousStatus).toBe("review");
    });

    it("throws for non-existent mission", async () => {
      expect(
        updateFeature(missionStore, featureStore, tmpDir, "2026-03-28-001", "f1", {
          status: "in-progress",
        }),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

    it("throws for non-existent feature", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      expect(
        updateFeature(missionStore, featureStore, tmpDir, missionId, "nonexistent", {
          status: "in-progress",
        }),
      ).rejects.toThrow("Feature nonexistent not found");
    });

    it("allows same status update (no-op)", async () => {
      const { missionId } = await createSampleMission(missionStore, featureStore, assertionStore, tmpDir);

      const before = await featureStore.get(missionId, "f1");

      const result = await updateFeature(missionStore, featureStore, tmpDir, missionId, "f1", {
        status: "pending",
      });

      expect(result.feature.status).toBe("pending");
      expect(result.feature.updatedAt).not.toBe(before?.updatedAt);
    });
  });

  describe("parseWorkerReport", () => {
    it("parses inline JSON report", async () => {
      const reportData = {
        content: "Test report content",
        timestamp: "2026-03-28T10:00:00.000Z",
        agent: "test-agent",
      };

      const result = await parseWorkerReport(JSON.stringify(reportData));

      // Legacy content is promoted to salientSummary and whatWasImplemented
      expect(result.salientSummary).toBe("Test report content");
      expect(result.whatWasImplemented).toBe("Test report content");
      expect(result.whatWasLeftUndone).toBe("");
      expect(result.verification).toEqual({ commandsRun: [], interactiveChecks: [] });
      expect(result.tests).toEqual({ added: [] });
      expect(result.discoveredIssues).toEqual([]);
    });

    it("generates timestamp if not provided", async () => {
      const reportData = {
        content: "Test report content",
      };

      const result = await parseWorkerReport(JSON.stringify(reportData));

      // Legacy content-only report is promoted to rich format
      expect(result.salientSummary).toBe("Test report content");
      expect(result.whatWasImplemented).toBe("Test report content");
      expect(result.whatWasLeftUndone).toBe("");
    });

    it("reads report from file using @ syntax", async () => {
      const reportPath = join(tmpDir, "test-report.json");
      const reportData = {
        content: "File-based report",
        timestamp: "2026-03-28T12:00:00.000Z",
      };
      await writeFile(reportPath, JSON.stringify(reportData));

      const result = await parseWorkerReport(`@${reportPath}`);

      // Legacy format from file is promoted to rich format
      expect(result.salientSummary).toBe("File-based report");
      expect(result.whatWasImplemented).toBe("File-based report");
    });

    it("throws for missing file with @ syntax", async () => {
      expect(
        parseWorkerReport("@/nonexistent/path/report.json"),
      ).rejects.toThrow("Report file not found");
    });

    it("throws for invalid JSON", async () => {
      expect(
        parseWorkerReport("not valid json"),
      ).rejects.toThrow("Invalid JSON in worker report");
    });

    it("throws for missing content field", async () => {
      const reportData = {
        timestamp: "2026-03-28T10:00:00.000Z",
      };

      expect(
        parseWorkerReport(JSON.stringify(reportData)),
      ).rejects.toThrow("Worker report must have 'salientSummary' (preferred) or 'content' (legacy) field");
    });

    it("throws for empty content field", async () => {
      const reportData = {
        content: "",
      };

      expect(
        parseWorkerReport(JSON.stringify(reportData)),
      ).rejects.toThrow("Worker report must have 'salientSummary' (preferred) or 'content' (legacy) field");
    });

    it("throws for non-object JSON", async () => {
      expect(
        parseWorkerReport("123"),
      ).rejects.toThrow("must be a JSON object");

      expect(
        parseWorkerReport("\"string\""),
      ).rejects.toThrow("must be a JSON object");
    });
  });
});
