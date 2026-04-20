import { describe, expect, it } from "bun:test";
import { launchHandoff, type HandoffLaunchPort, type HandoffLaunchRecord, type LaunchStorePort } from "@/features/handoff";
import type { GitPort } from "@/infra/ports/git.port.js";
import { mockAssertionStore, mockFeatureStore, mockMissionStore } from "../../../../helpers/mocks.js";

function makeLaunchStore(): LaunchStorePort & { readonly updates: HandoffLaunchRecord[] } {
  let current: HandoffLaunchRecord | undefined;
  const updates: HandoffLaunchRecord[] = [];
  return {
    updates,
    async create(input) {
      current = {
        id: "2026-04-20-001",
        createdAt: "2026-04-20T00:00:00.000Z",
        task: input.task,
        name: input.name,
        provider: input.provider,
        model: input.model,
        status: "launching",
        wait: input.wait,
        sourceDir: input.sourceDir,
        targetDir: input.targetDir,
        promptPath: ".maestro/launches/2026-04-20-001/prompt.md",
        outputPath: ".maestro/launches/2026-04-20-001/output.log",
        command: [],
        refs: input.refs,
        ...(input.worktree ? { worktree: input.worktree } : {}),
      };
      updates.push(current);
      return current;
    },
    async update(record) {
      current = record;
      updates.push(record);
      return record;
    },
    async get() {
      return current;
    },
    async list() {
      return current ? [current] : [];
    },
    resolveArtifactPath(relativePath: string) {
      return `/tmp/project/${relativePath}`;
    },
  };
}

function makeGit(): GitPort {
  return {
    async isRepo() { return true; },
    async getState() {
      return {
        branch: "main",
        recentCommits: ["abc1234 feat: seed"],
        changedFiles: ["src/features/handoff/commands/handoff.command.ts"],
        workingTreeClean: false,
        diffStat: "+12 -3",
      };
    },
    async getCurrentBranch() { return "main"; },
    async createWorktree(_cwd, input) {
      return {
        slug: input.slug,
        baseBranch: input.baseBranch,
        branch: `${input.branchPrefix}/${input.slug}`,
        path: `/tmp/${input.slug}`,
      };
    },
  };
}

describe("launchHandoff", () => {
  it("uses the provider default model and records a detached launch", async () => {
    const launchStore = makeLaunchStore();
    const launchCalls: Array<Parameters<HandoffLaunchPort["launch"]>[0]> = [];
    const codexLauncher: HandoffLaunchPort = {
      provider: "codex",
      async launch(request) {
        launchCalls.push(request);
        return {
          command: ["codex", "exec", "--cd", request.targetDir, "--full-auto", "--model", request.model, request.prompt],
          pid: 4321,
        };
      },
    };

    const result = await launchHandoff({
      missionStore: mockMissionStore([]),
      featureStore: mockFeatureStore("2026-04-20-001", []),
      assertionStore: mockAssertionStore("2026-04-20-001", []),
      git: makeGit(),
      launchStore,
      launchers: {
        codex: codexLauncher,
        claude: { provider: "claude", async launch() { throw new Error("not used"); } },
      },
    }, {
      cwd: "/tmp/project",
      task: "Investigate the failing bundle export",
      provider: "codex",
      wait: false,
    });

    expect(result.record.model).toBe("gpt-5.4");
    expect(result.record.status).toBe("launched");
    expect(result.record.pid).toBe(4321);
    expect(launchCalls[0]?.model).toBe("gpt-5.4");
    expect(launchCalls[0]?.name).toContain("[Handoff]");
    expect(result.prompt).toContain("## Task");
  });

  it("rejects --base without --worktree", async () => {
    await expect(
      launchHandoff({
        missionStore: mockMissionStore([]),
        featureStore: mockFeatureStore("2026-04-20-001", []),
        assertionStore: mockAssertionStore("2026-04-20-001", []),
        git: makeGit(),
        launchStore: makeLaunchStore(),
        launchers: {
          codex: { provider: "codex", async launch() { throw new Error("not used"); } },
          claude: { provider: "claude", async launch() { throw new Error("not used"); } },
        },
      }, {
        cwd: "/tmp/project",
        task: "Fail fast",
        provider: "codex",
        wait: false,
        baseBranch: "main",
      }),
    ).rejects.toThrow("--base can only be used with --worktree");
  });

  it("creates a worktree and waits for a claude launch to finish", async () => {
    const launchStore = makeLaunchStore();
    const claudeLauncher: HandoffLaunchPort = {
      provider: "claude",
      async launch(request) {
        expect(request.targetDir).toBe("/tmp/fix-handoff");
        expect(request.model).toBe("opus");
        return {
          command: ["claude", "--print", "--permission-mode", "bypassPermissions", "--model", request.model, "--name", request.name, request.prompt],
          exitCode: 0,
        };
      },
    };

    const result = await launchHandoff({
      missionStore: mockMissionStore([]),
      featureStore: mockFeatureStore("2026-04-20-001", []),
      assertionStore: mockAssertionStore("2026-04-20-001", []),
      git: makeGit(),
      launchStore,
      launchers: {
        codex: { provider: "codex", async launch() { throw new Error("not used"); } },
        claude: claudeLauncher,
      },
    }, {
      cwd: "/tmp/project",
      task: "Fix handoff worktree behavior",
      provider: "claude",
      wait: true,
      worktree: "fix-handoff",
    });

    expect(result.record.status).toBe("completed");
    expect(result.record.exitCode).toBe(0);
    expect(result.record.worktree).toMatchObject({
      path: "/tmp/fix-handoff",
      branch: "codex/fix-handoff",
      baseBranch: "main",
    });
    expect(result.prompt).toContain("fresh worktree");
  });
});
