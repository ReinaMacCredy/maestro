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
        agent: input.agent,
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
    async consume() {
      throw new Error("not used");
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
  it("uses the agent default model and records a detached launch", async () => {
    const launchStore = makeLaunchStore();
    const launchCalls: Array<Parameters<HandoffLaunchPort["launch"]>[0]> = [];
    const codexLauncher: HandoffLaunchPort = {
      agent: "codex",
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
        claude: { agent: "claude", async launch() { throw new Error("not used"); } },
      },
    }, {
      cwd: "/tmp/project",
      task: "Investigate the failing bundle export",
      agent: "codex",
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
          codex: { agent: "codex", async launch() { throw new Error("not used"); } },
          claude: { agent: "claude", async launch() { throw new Error("not used"); } },
        },
      }, {
        cwd: "/tmp/project",
        task: "Fail fast",
        agent: "codex",
        wait: false,
        baseBranch: "main",
      }),
    ).rejects.toThrow("--base can only be used with --worktree");
  });

  it("creates a worktree and waits for a claude launch to finish", async () => {
    const launchStore = makeLaunchStore();
    const claudeLauncher: HandoffLaunchPort = {
      agent: "claude",
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
        codex: { agent: "codex", async launch() { throw new Error("not used"); } },
        claude: claudeLauncher,
      },
    }, {
      cwd: "/tmp/project",
      task: "Fix handoff worktree behavior",
      agent: "claude",
      wait: true,
      worktree: "fix-handoff",
    });

    expect(result.record.status).toBe("completed");
    expect(result.record.exitCode).toBe(0);
    expect(result.record.worktree).toMatchObject({
      path: "/tmp/fix-handoff",
      branch: "claude/fix-handoff",
      baseBranch: "main",
    });
    expect(result.prompt).toContain("fresh worktree");
  });

  it("throws when a waited launch exits non-zero after recording the failure", async () => {
    const launchStore = makeLaunchStore();

    await expect(
      launchHandoff({
        missionStore: mockMissionStore([]),
        featureStore: mockFeatureStore("2026-04-20-001", []),
        assertionStore: mockAssertionStore("2026-04-20-001", []),
        git: makeGit(),
        launchStore,
        launchers: {
          codex: {
            agent: "codex",
            async launch() {
              return {
                command: ["codex", "exec"],
                exitCode: 7,
              };
            },
          },
          claude: { agent: "claude", async launch() { throw new Error("not used"); } },
        },
      }, {
        cwd: "/tmp/project",
        task: "Fail loudly",
        agent: "codex",
        wait: true,
      }),
    ).rejects.toThrow("codex handoff exited with code 7");

    expect(launchStore.updates.at(-1)?.status).toBe("failed");
    expect(launchStore.updates.at(-1)?.exitCode).toBe(7);
  });

  it("throws when a waited launch does not report an exit code", async () => {
    const launchStore = makeLaunchStore();

    await expect(
      launchHandoff({
        missionStore: mockMissionStore([]),
        featureStore: mockFeatureStore("2026-04-20-001", []),
        assertionStore: mockAssertionStore("2026-04-20-001", []),
        git: makeGit(),
        launchStore,
        launchers: {
          codex: {
            agent: "codex",
            async launch() {
              return {
                command: ["codex", "exec"],
              };
            },
          },
          claude: { agent: "claude", async launch() { throw new Error("not used"); } },
        },
      }, {
        cwd: "/tmp/project",
        task: "Missing exit code",
        agent: "codex",
        wait: true,
      }),
    ).rejects.toThrow("codex handoff did not report an exit code");

    expect(launchStore.updates.at(-1)?.status).toBe("failed");
    expect(launchStore.updates.at(-1)?.exitCode).toBeUndefined();
  });

  describe("--prompt-file bypass", () => {
    it("uses the file contents verbatim as the prompt and skips auto-generation", async () => {
      const briefPath = `/tmp/maestro-prompt-file-test-${Date.now()}.md`;
      const briefContent = "## Task\n\nHand-written brief body.\n\n## Constraints\n- Do not edit.\n";
      await Bun.write(briefPath, briefContent);

      const launchStore = makeLaunchStore();
      const codexLauncher: HandoffLaunchPort = {
        agent: "codex",
        async launch(request) {
          // launchStore persistence uses `prompt` from the create call, which
          // must be the raw file content (not the auto-generated brief).
          expect(request.prompt).toBe(briefContent);
          return {
            command: ["codex", "exec", "--model", request.model, request.prompt],
            pid: 1234,
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
          claude: { agent: "claude", async launch() { throw new Error("not used"); } },
        },
      }, {
        cwd: "/tmp/project",
        task: "Ignored when promptFile is set",
        agent: "codex",
        wait: false,
        promptFile: briefPath,
      });

      expect(result.prompt).toBe(briefContent);
      expect(launchStore.updates[0]?.task).toBe("Ignored when promptFile is set");
      // Note: the supplied brief is what launchStore persists to prompt.md.
    });

    it("throws a MaestroError when --prompt-file does not exist", async () => {
      await expect(
        launchHandoff({
          missionStore: mockMissionStore([]),
          featureStore: mockFeatureStore("2026-04-20-001", []),
          assertionStore: mockAssertionStore("2026-04-20-001", []),
          git: makeGit(),
          launchStore: makeLaunchStore(),
          launchers: {
            codex: { agent: "codex", async launch() { throw new Error("not used"); } },
            claude: { agent: "claude", async launch() { throw new Error("not used"); } },
          },
        }, {
          cwd: "/tmp/project",
          task: "probe",
          agent: "codex",
          wait: false,
          promptFile: "/tmp/maestro-prompt-file-definitely-not-here-xyz.md",
        }),
      ).rejects.toThrow("--prompt-file not found");
    });

    it("does not create a worktree before validating --prompt-file", async () => {
      let createWorktreeCalls = 0;
      await expect(
        launchHandoff({
          missionStore: mockMissionStore([]),
          featureStore: mockFeatureStore("2026-04-20-001", []),
          assertionStore: mockAssertionStore("2026-04-20-001", []),
          git: {
            ...makeGit(),
            async createWorktree(_cwd, input) {
              createWorktreeCalls += 1;
              return {
                slug: input.slug,
                baseBranch: input.baseBranch,
                branch: `${input.branchPrefix}/${input.slug}`,
                path: `/tmp/${input.slug}`,
              };
            },
          },
          launchStore: makeLaunchStore(),
          launchers: {
            codex: { agent: "codex", async launch() { throw new Error("not used"); } },
            claude: { agent: "claude", async launch() { throw new Error("not used"); } },
          },
        }, {
          cwd: "/tmp/project",
          task: "probe",
          agent: "codex",
          wait: false,
          worktree: "bad-prompt-worktree",
          promptFile: "/tmp/maestro-prompt-file-definitely-not-here-xyz.md",
        }),
      ).rejects.toThrow("--prompt-file not found");

      expect(createWorktreeCalls).toBe(0);
    });

    it("throws a MaestroError when --prompt-file is empty", async () => {
      const briefPath = `/tmp/maestro-prompt-file-empty-${Date.now()}.md`;
      await Bun.write(briefPath, "   \n\n");

      await expect(
        launchHandoff({
          missionStore: mockMissionStore([]),
          featureStore: mockFeatureStore("2026-04-20-001", []),
          assertionStore: mockAssertionStore("2026-04-20-001", []),
          git: makeGit(),
          launchStore: makeLaunchStore(),
          launchers: {
            codex: { agent: "codex", async launch() { throw new Error("not used"); } },
            claude: { agent: "claude", async launch() { throw new Error("not used"); } },
          },
        }, {
          cwd: "/tmp/project",
          task: "probe",
          agent: "codex",
          wait: false,
          promptFile: briefPath,
        }),
      ).rejects.toThrow("--prompt-file is empty");
    });

    it("resolves a relative --prompt-file path against cwd", async () => {
      // Write the brief to the test cwd and pass a relative path. This verifies
      // the resolve() behavior without requiring a specific project directory
      // layout.
      const briefName = `maestro-prompt-file-rel-${Date.now()}.md`;
      const cwd = process.cwd();
      await Bun.write(`${cwd}/${briefName}`, "## Task\nRelative probe.\n");

      try {
        const result = await launchHandoff({
          missionStore: mockMissionStore([]),
          featureStore: mockFeatureStore("2026-04-20-001", []),
          assertionStore: mockAssertionStore("2026-04-20-001", []),
          git: makeGit(),
          launchStore: makeLaunchStore(),
          launchers: {
            codex: {
              agent: "codex",
              async launch(req) {
                return { command: ["codex", req.prompt], pid: 1 };
              },
            },
            claude: { agent: "claude", async launch() { throw new Error("not used"); } },
          },
        }, {
          cwd,
          task: "probe",
          agent: "codex",
          wait: false,
          promptFile: briefName,
        });

        expect(result.prompt).toContain("Relative probe");
      } finally {
        await Bun.file(`${cwd}/${briefName}`).delete?.();
      }
    });
  });
});
