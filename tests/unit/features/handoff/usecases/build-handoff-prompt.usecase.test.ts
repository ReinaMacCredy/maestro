import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { buildHandoffPrompt } from "@/features/handoff";
import type { GitPort } from "@/infra/ports/git.port.js";
import { mockAssertionStore, mockFeatureStore, mockMissionStore } from "../../../../helpers/mocks.js";
import type { Mission, Feature, Assertion } from "@/features/mission";

let cwd: string;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-handoff-prompt-"));
});

afterEach(async () => {
  await rm(cwd, { recursive: true, force: true });
});

function makeGit(changedFiles: readonly string[]): GitPort {
  return {
    async isRepo() { return true; },
    async getState() {
      return {
        branch: "main",
        recentCommits: ["abc1234 feat: seed prompt context"],
        changedFiles: [...changedFiles],
        workingTreeClean: changedFiles.length === 0,
        diffStat: changedFiles.length === 0 ? "+0 -0" : "+12 -3",
      };
    },
    async getCurrentBranch() { return "main"; },
    async createWorktree() {
      throw new Error("not used");
    },
  };
}

describe("buildHandoffPrompt", () => {
  it("builds a mission-scoped prompt when exactly one actionable feature exists", async () => {
    const mission: Mission = {
      id: "2026-04-20-001",
      status: "executing",
      title: "Launch handoff",
      description: "Replace UKI with native launching.",
      milestones: [
        {
          id: "m1",
          title: "Implementation",
          description: "Ship the new handoff flow.",
          order: 0,
          featureIds: ["f1"],
          profile: "implementation",
        },
      ],
      features: ["f1"],
      createdAt: "2026-04-20T00:00:00.000Z",
      updatedAt: "2026-04-20T00:00:00.000Z",
    };
    const feature: Feature = {
      id: "f1",
      missionId: mission.id,
      milestoneId: "m1",
      status: "in-progress",
      title: "Replace the handoff queue",
      description: "Implement the new native launch command.",
      agentType: "codex-cli",
      verificationSteps: ["bun test tests/unit/features/handoff"],
      dependsOn: [],
      fulfills: ["VAL-HANDOFF-001"],
      expectedBehavior: "The command launches a fresh agent with a markdown brief.",
      preconditions: "Do not reintroduce the UKI packet format.",
      report: {
        salientSummary: "Initial command skeleton landed.",
        whatWasImplemented: "Added a launch command and prompt builder.",
        whatWasLeftUndone: "Bundle and TUI cleanup still pending.",
        verification: {
          commandsRun: [
            {
              command: "bun run build",
              exitCode: 0,
              observation: "Compiled successfully after the first pass.",
            },
          ],
          interactiveChecks: [],
        },
        tests: { added: [] },
        discoveredIssues: [
          {
            severity: "medium",
            description: "Mission Control still referenced the old queue.",
            suggestedFix: "Remove the preview and modal entry points.",
          },
        ],
      },
      createdAt: "2026-04-20T00:00:00.000Z",
      updatedAt: "2026-04-20T00:00:00.000Z",
    };
    const assertion: Assertion = {
      id: "a1",
      missionId: mission.id,
      milestoneId: "m1",
      featureId: "f1",
      result: "pending",
      description: "The handoff command persists prompt.md, output.log, and launch.json.",
      surface: "cli",
      createdAt: "2026-04-20T00:00:00.000Z",
      updatedAt: "2026-04-20T00:00:00.000Z",
    };

    const workersDir = join(cwd, ".maestro", "missions", mission.id, "workers", feature.id);
    const repliesDir = join(cwd, ".maestro", "replies", mission.id);
    await mkdir(workersDir, { recursive: true });
    await mkdir(repliesDir, { recursive: true });
    await writeFile(join(workersDir, "prompt.md"), "# existing prompt\n");
    await writeFile(join(workersDir, "report.json"), "{}\n");
    await writeFile(join(repliesDir, `${feature.id}.yaml`), "outcome: completed\n");

    const result = await buildHandoffPrompt({
      missionStore: mockMissionStore([mission]),
      featureStore: mockFeatureStore(mission.id, [feature]),
      assertionStore: mockAssertionStore(mission.id, [assertion]),
      git: makeGit(["src/features/handoff/commands/handoff.command.ts"]),
    }, {
      cwd,
      task: "Finish replacing the old handoff queue",
    });

    expect(result.context.refs).toEqual({
      missionId: mission.id,
      featureId: feature.id,
      milestoneId: "m1",
    });
    expect(result.prompt).toContain("## Task");
    expect(result.prompt).toContain("Mission 2026-04-20-001: Launch handoff");
    expect(result.prompt).toContain("`src/features/handoff/commands/handoff.command.ts`");
    expect(result.prompt).toContain("Do not reintroduce the UKI packet format.");
    expect(result.prompt).toContain("The handoff command persists prompt.md, output.log, and launch.json.");
  });

  it("falls back to repository context when multiple actionable features exist", async () => {
    const mission: Mission = {
      id: "2026-04-20-002",
      status: "executing",
      title: "Multi-feature mission",
      description: "Too broad for automatic feature context.",
      milestones: [
        { id: "m1", title: "Work", description: "desc", order: 0, featureIds: ["f1", "f2"] },
      ],
      features: ["f1", "f2"],
      createdAt: "2026-04-20T00:00:00.000Z",
      updatedAt: "2026-04-20T00:00:00.000Z",
    };
    const features: Feature[] = [
      {
        id: "f1",
        missionId: mission.id,
        milestoneId: "m1",
        status: "pending",
        title: "One",
        description: "first",
        agentType: "codex-cli",
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        createdAt: "2026-04-20T00:00:00.000Z",
        updatedAt: "2026-04-20T00:00:00.000Z",
      },
      {
        id: "f2",
        missionId: mission.id,
        milestoneId: "m1",
        status: "review",
        title: "Two",
        description: "second",
        agentType: "claude-code",
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        createdAt: "2026-04-20T00:00:00.000Z",
        updatedAt: "2026-04-20T00:00:00.000Z",
      },
    ];

    const result = await buildHandoffPrompt({
      missionStore: mockMissionStore([mission]),
      featureStore: mockFeatureStore(mission.id, features),
      assertionStore: mockAssertionStore(mission.id, []),
      git: makeGit(["README.md", "src/services.ts"]),
    }, {
      cwd,
      task: "Investigate the current repo state",
    });

    expect(result.context.refs).toEqual({});
    expect(result.prompt).toContain("without a single active mission feature");
    expect(result.prompt).toContain("`README.md`");
    expect(result.prompt).toContain("`src/services.ts`");
  });
});
