/**
 * Unit tests for generate-worker-prompt.usecase
 */
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  generateWorkerPrompt,
  type GenerateWorkerPromptResult,
} from "../../../src/usecases/generate-worker-prompt.usecase.js";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import { MaestroError } from "../../../src/domain/errors.js";
import type { Milestone } from "../../../src/domain/mission-types.js";

let tmpDir: string;

async function setupTmpDir(): Promise<void> {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-worker-prompt-test-"));
}

async function cleanup(): Promise<void> {
  if (tmpDir) {
    await rm(tmpDir, { recursive: true, force: true });
  }
}

async function createSampleSkill(baseDir: string, skillName: string, content: string): Promise<void> {
  const skillDir = join(baseDir, ".maestro", "skills", skillName);
  await mkdir(skillDir, { recursive: true });
  await writeFile(join(skillDir, "SKILL.md"), content);
}

async function createBuiltInSkill(baseDir: string, skillName: string, content: string): Promise<void> {
  const skillDir = join(baseDir, "skills", "built-in", skillName);
  await mkdir(skillDir, { recursive: true });
  await writeFile(join(skillDir, "SKILL.md"), content);
}

async function createTestMission(
  missionStore: FsMissionStoreAdapter,
  featureStore: FsFeatureStoreAdapter,
  assertionStore: FsAssertionStoreAdapter,
  baseDir: string,
): Promise<{ missionId: string; features: string[] }> {
  const sampleMilestones: Milestone[] = [
    { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
  ];

  const samplePlan = {
    title: "Test Mission",
    description: "A test mission for worker prompt generation",
    milestones: sampleMilestones,
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Test Feature",
        description: "This feature tests worker prompt generation.",
        workerType: "test-skill",
        verificationSteps: ["Step 1: Do something", "Step 2: Verify result"],
        dependsOn: [],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature with dependencies.",
        workerType: "test-skill",
        verificationSteps: ["Step 3"],
        dependsOn: ["f1"],
      },
    ],
  };

  const { createMission } = await import("../../../src/usecases/mission-lifecycle.usecase.js");
  const result = await createMission(missionStore, featureStore, assertionStore, samplePlan);

  return {
    missionId: result.mission.id,
    features: result.features.map((f) => f.id),
  };
}

describe("generateWorkerPrompt", () => {
  beforeEach(async () => {
    await setupTmpDir();
  });

  afterEach(async () => {
    await cleanup();
  });

  it("generates a complete worker prompt with mission context", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    // Create test mission and features
    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill\n\nThis is a test skill.");

    // Generate prompt
    const result = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    // Assertions
    expect(result.prompt).toContain("Worker Assignment: Test Feature");
    expect(result.prompt).toContain("Feature ID:** f1");
    expect(result.prompt).toContain("Worker Type:** test-skill");
    expect(result.prompt).toContain(`Mission:** ${missionId}`);
    expect(result.prompt).toContain("Milestone:** m1");
    expect(result.prompt).toContain("## Mission Context");
    expect(result.prompt).toContain("A test mission for worker prompt generation");
    expect(result.prompt).toContain("## Feature Assignment");
    expect(result.prompt).toContain("Step 1: Do something");
    expect(result.prompt).toContain("Step 2: Verify result");
    expect(result.prompt).toContain("## Skill Instructions");
    expect(result.prompt).toContain("<!-- BEGIN SKILL -->");
    expect(result.prompt).toContain("<!-- END SKILL -->");
    expect(result.prompt).toContain("# Test Skill");
    expect(result.featureId).toBe("f1");
    expect(result.workerType).toBe("test-skill");
  });

  it("includes related assertions in the prompt", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill");

    // Create assertion for f1
    await assertionStore.create(missionId, {
      missionId,
      milestoneId: "m1",
      featureId: "f1",
      description: "Feature must implement X correctly",
    }, "assert-1");

    // Generate prompt
    const result = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    expect(result.prompt).toContain("Related Assertions");
    expect(result.prompt).toContain("assert-1");
    expect(result.prompt).toContain("Feature must implement X correctly");
  });

  it("writes prompt to workers/{featureId}/prompt.md", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill");

    // Generate prompt
    const result = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    expect(result.writtenTo).toBeDefined();
    expect(result.writtenTo?.length).toBe(1);
    expect(result.writtenTo?.[0]).toContain("workers/f1/prompt.md");
  });

  it("writes to --out path when provided", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill");

    // Generate prompt with --out
    const outPath = join(tmpDir, "custom-prompt.md");
    const result = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
      outPath,
    );

    expect(result.writtenTo).toBeDefined();
    expect(result.writtenTo?.length).toBe(2);
    expect(result.writtenTo?.[0]).toBe(outPath);
    expect(result.writtenTo?.[1]).toContain("workers/f1/prompt.md");
  });

  it("falls back to built-in skills when workspace skill is missing", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);
    await createBuiltInSkill(tmpDir, "test-skill", "# Built In Skill\n\nUse the packaged worker flow.");

    const result = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    expect(result.prompt).toContain("# Built In Skill");
    expect(result.prompt).toContain("Use the packaged worker flow.");
  });

  it("throws error for non-existent mission", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    let errorThrown = false;
    try {
      await generateWorkerPrompt(
        missionStore,
        featureStore,
        assertionStore,
        tmpDir,
        "non-existent",
        "f1",
      );
    } catch (err) {
      errorThrown = true;
      expect((err as Error).message).toContain("Mission non-existent not found");
    }

    expect(errorThrown).toBe(true);
  });

  it("throws error for non-existent feature", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    // Create mission without the feature
    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    let errorThrown = false;
    try {
      await generateWorkerPrompt(
        missionStore,
        featureStore,
        assertionStore,
        tmpDir,
        missionId,
        "non-existent",
      );
    } catch (err) {
      errorThrown = true;
      expect((err as Error).message).toContain("Feature non-existent not found");
    }

    expect(errorThrown).toBe(true);
  });

  it("throws actionable error for missing skill file", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    // Don't create skill file

    let errorThrown = false;
    try {
      await generateWorkerPrompt(
        missionStore,
        featureStore,
        assertionStore,
        tmpDir,
        missionId,
        "f1",
      );
    } catch (err) {
      errorThrown = true;
      expect(err).toBeInstanceOf(MaestroError);
      expect((err as Error).message).toContain("Worker skill 'test-skill' not found");
      expect((err as Error).message).toContain(".maestro/skills/test-skill/SKILL.md");
      expect((err as MaestroError).hints.join("\n")).toContain("skills/built-in/test-skill/SKILL.md");
    }

    expect(errorThrown).toBe(true);
  });

  it("sanitizes content containing markdown headers", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const sampleMilestones: Milestone[] = [
      { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
    ];

    // Create mission with markdown headers in description
    const samplePlan = {
      title: "Test Mission",
      description: "# Header\n## Subheader\nRegular text",
      milestones: sampleMilestones,
      features: [
        {
          id: "f1",
          milestoneId: "m1",
          title: "Test Feature",
          description: "# Feature Header\nSome text <!-- comment --> -->",
          workerType: "test-skill",
          verificationSteps: ["Step 1"],
          dependsOn: [],
        },
      ],
    };

    const { createMission } = await import("../../../src/usecases/mission-lifecycle.usecase.js");
    const result = await createMission(missionStore, featureStore, assertionStore, samplePlan);
    const missionId = result.mission.id;

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill");

    // Generate prompt
    const promptResult = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    // Headers should be escaped to not break structure
    expect(promptResult.prompt).toContain("\\# Header");
    expect(promptResult.prompt).toContain("\\## Subheader");
    expect(promptResult.prompt).toContain("\\# Feature Header");
  });

  it("includes dependencies section when feature has dependencies", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const sampleMilestones: Milestone[] = [
      { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
    ];

    const samplePlan = {
      title: "Test Mission",
      description: "Test mission",
      milestones: sampleMilestones,
      features: [
        {
          id: "f1",
          milestoneId: "m1",
          title: "Feature 1",
          description: "First feature",
          workerType: "test-skill",
          verificationSteps: ["Step 1"],
          dependsOn: ["f2", "f3"],
        },
        {
          id: "f2",
          milestoneId: "m1",
          title: "Feature 2",
          description: "Second feature",
          workerType: "test-skill",
          verificationSteps: ["Step 2"],
          dependsOn: [],
        },
        {
          id: "f3",
          milestoneId: "m1",
          title: "Feature 3",
          description: "Third feature",
          workerType: "test-skill",
          verificationSteps: ["Step 3"],
          dependsOn: [],
        },
      ],
    };

    const { createMission } = await import("../../../src/usecases/mission-lifecycle.usecase.js");
    const result = await createMission(missionStore, featureStore, assertionStore, samplePlan);
    const missionId = result.mission.id;

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill");

    // Generate prompt for f1 which has dependencies
    const promptResult = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    expect(promptResult.prompt).toContain("### Dependencies");
    expect(promptResult.prompt).toContain("- f2");
    expect(promptResult.prompt).toContain("- f3");
  });

  it("omits dependencies section when feature has no dependencies", async () => {
    const missionStore = new FsMissionStoreAdapter(tmpDir);
    const featureStore = new FsFeatureStoreAdapter(tmpDir);
    const assertionStore = new FsAssertionStoreAdapter(tmpDir);

    const { missionId } = await createTestMission(missionStore, featureStore, assertionStore, tmpDir);

    // Create skill file
    await createSampleSkill(tmpDir, "test-skill", "# Test Skill");

    // f1 has empty dependsOn
    const result = await generateWorkerPrompt(
      missionStore,
      featureStore,
      assertionStore,
      tmpDir,
      missionId,
      "f1",
    );

    expect(result.prompt).not.toContain("### Dependencies");
  });
});
