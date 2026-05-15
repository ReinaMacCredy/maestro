import { describe, expect, it } from "bun:test";
import { launchHandoff, type HandoffLaunchPort, type HandoffRecord, type HandoffStorePort } from "@/features/handoff";
import type { Feature, Mission } from "@/shared/domain/legacy-mission";
import type { GitPort } from "@/infra/ports/git.port.js";
import { mockMissions } from "../../../../helpers/mocks.js";

function makeHandoffStore(): HandoffStorePort & { readonly updates: HandoffRecord[] } {
  let current: HandoffRecord | undefined;
  const updates: HandoffRecord[] = [];
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
        promptPath: ".maestro/handoff/2026-04-20-001/prompt.md",
        outputPath: ".maestro/handoff/2026-04-20-001/output.log",
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
    async listOpenForTask() {
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
    const handoffStore = makeHandoffStore();
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
      missions: mockMissions(),
      git: makeGit(),
      handoffStore,
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
    expect(launchCalls[0]?.prompt).toContain("## Handoff Startup");
    expect(launchCalls[0]?.prompt).toContain("maestro handoff pickup --id 2026-04-20-001 --json");
    expect(result.prompt).toContain("## Task");
    expect(result.prompt).toContain("maestro handoff pickup --id 2026-04-20-001 --json");
  });

  it("launches Hermes without passing the default model and injects Maestro session env", async () => {
    const handoffStore = makeHandoffStore();
    const launchCalls: Array<Parameters<HandoffLaunchPort["launch"]>[0]> = [];
    const hermesLauncher: HandoffLaunchPort = {
      agent: "hermes",
      async launch(request) {
        launchCalls.push(request);
        return {
          command: ["hermes", "chat", "--quiet", "-q", request.prompt],
          pid: 6789,
        };
      },
    };

    const result = await launchHandoff({
      missions: mockMissions(),
      git: makeGit(),
      handoffStore,
      launchers: {
        hermes: hermesLauncher,
      },
    }, {
      cwd: "/tmp/project",
      task: "Run Hermes handoff",
      agent: "hermes",
      wait: false,
    });

    expect(result.record.model).toBe("default");
    expect(result.record.status).toBe("launched");
    expect(launchCalls[0]?.modelProvided).toBe(false);
    expect(launchCalls[0]?.env).toEqual({
      MAESTRO_AGENT: "hermes",
      MAESTRO_SESSION_ID: "2026-04-20-001",
    });
  });

  it("marks an explicitly supplied Hermes model for the launcher", async () => {
    const launchCalls: Array<Parameters<HandoffLaunchPort["launch"]>[0]> = [];
    await launchHandoff({
      missions: mockMissions(),
      git: makeGit(),
      handoffStore: makeHandoffStore(),
      launchers: {
        hermes: {
          agent: "hermes",
          async launch(request) {
            launchCalls.push(request);
            return { command: ["hermes"], pid: 1 };
          },
        },
      },
    }, {
      cwd: "/tmp/project",
      task: "Run Hermes with model",
      agent: "hermes",
      model: "nous/hermes",
      wait: false,
    });

    expect(launchCalls[0]?.model).toBe("nous/hermes");
    expect(launchCalls[0]?.modelProvided).toBe(true);
  });

  it("rejects --base without --worktree", async () => {
    await expect(
      launchHandoff({
        missions: mockMissions(),
        git: makeGit(),
        handoffStore: makeHandoffStore(),
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
    const handoffStore = makeHandoffStore();
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
      missions: mockMissions(),
      git: makeGit(),
      handoffStore,
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
    const handoffStore = makeHandoffStore();

    await expect(
      launchHandoff({
        missions: mockMissions(),
        git: makeGit(),
        handoffStore,
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

    expect(handoffStore.updates.at(-1)?.status).toBe("failed");
    expect(handoffStore.updates.at(-1)?.exitCode).toBe(7);
  });

  it("throws when a waited launch does not report an exit code", async () => {
    const handoffStore = makeHandoffStore();

    await expect(
      launchHandoff({
        missions: mockMissions(),
        git: makeGit(),
        handoffStore,
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

    expect(handoffStore.updates.at(-1)?.status).toBe("failed");
    expect(handoffStore.updates.at(-1)?.exitCode).toBeUndefined();
  });

  describe("--prompt-file bypass", () => {
    it("uses the file contents verbatim as the prompt and skips auto-generation", async () => {
      const briefPath = `/tmp/maestro-prompt-file-test-${Date.now()}.md`;
      const briefContent = "## Task\n\nHand-written brief body.\n\n## Constraints\n- Do not edit.\n";
      await Bun.write(briefPath, briefContent);

      const handoffStore = makeHandoffStore();
      const codexLauncher: HandoffLaunchPort = {
        agent: "codex",
        async launch(request) {
          expect(request.prompt).toContain("maestro handoff pickup --id 2026-04-20-001 --json");
          expect(request.prompt).toContain(briefContent);
          return {
            command: ["codex", "exec", "--model", request.model, request.prompt],
            pid: 1234,
          };
        },
      };

      const result = await launchHandoff({
        missions: mockMissions(),
        git: makeGit(),
        handoffStore,
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

      expect(result.prompt).toContain("maestro handoff pickup --id 2026-04-20-001 --json");
      expect(result.prompt).toContain(briefContent);
      expect(handoffStore.updates[0]?.task).toBe("Ignored when promptFile is set");
      // Note: the supplied brief remains embedded in the launched prompt after
      // the startup pickup preamble is injected.
    });

    it("preserves mission refs on the launch record when --prompt-file is set", async () => {
      const briefPath = `/tmp/maestro-prompt-file-refs-${Date.now()}.md`;
      await Bun.write(briefPath, "## Task\n\nUse the supplied prompt.\n");

      const mission: Mission = {
        id: "2026-04-20-refs",
        status: "executing",
        title: "Prompt file launch",
        description: "Keep mission linkage metadata on prompt-file launches.",
        milestones: [
          {
            id: "m1",
            title: "Launch",
            description: "Ship the prompt-file path without losing refs.",
            order: 0,
            featureIds: ["f1"],
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
        title: "Keep launch refs",
        description: "Preserve mission linkage when using prompt files.",
        agentType: "codex-cli",
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        createdAt: "2026-04-20T00:00:00.000Z",
        updatedAt: "2026-04-20T00:00:00.000Z",
      };

      const handoffStore = makeHandoffStore();
      const result = await launchHandoff({
        missions: mockMissions({
          missions: [mission],
          features: [feature],
        }),
        git: makeGit(),
        handoffStore,
        launchers: {
          codex: {
            agent: "codex",
            async launch(request) {
              return {
                command: ["codex", "exec", "--model", request.model, request.prompt],
                pid: 1234,
              };
            },
          },
          claude: { agent: "claude", async launch() { throw new Error("not used"); } },
        },
      }, {
        cwd: "/tmp/project",
        task: "Ignored when promptFile is set",
        agent: "codex",
        wait: false,
        promptFile: briefPath,
      });

      expect(result.record.refs).toMatchObject({
        missionId: mission.id,
        featureId: feature.id,
        milestoneId: "m1",
      });
      expect(handoffStore.updates[0]?.refs).toMatchObject({
        missionId: mission.id,
        featureId: feature.id,
        milestoneId: "m1",
      });
    });

    it("throws a MaestroError when --prompt-file does not exist", async () => {
      await expect(
        launchHandoff({
          missions: mockMissions(),
          git: makeGit(),
          handoffStore: makeHandoffStore(),
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
          missions: mockMissions(),
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
          handoffStore: makeHandoffStore(),
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

    it("throws a MaestroError when --prompt-file points at a directory", async () => {
      const dirPath = `/tmp/maestro-prompt-file-dir-${Date.now()}`;
      await (await import("node:fs/promises")).mkdir(dirPath, { recursive: true });

      try {
        await expect(
          launchHandoff({
            missions: mockMissions(),
            git: makeGit(),
            handoffStore: makeHandoffStore(),
            launchers: {
              codex: { agent: "codex", async launch() { throw new Error("not used"); } },
              claude: { agent: "claude", async launch() { throw new Error("not used"); } },
            },
          }, {
            cwd: "/tmp/project",
            task: "probe",
            agent: "codex",
            wait: false,
            promptFile: dirPath,
          }),
        ).rejects.toThrow("--prompt-file is a directory");
      } finally {
        await (await import("node:fs/promises")).rm(dirPath, { recursive: true, force: true });
      }
    });

    it("throws a MaestroError when --prompt-file points at a FIFO", async () => {
      // `mkfifo` is POSIX-only; skip on Windows runners where the syscall is
      // unavailable. The guard itself (stats.isFile() === false) still runs
      // on Windows for device paths, but we can't build a synthetic special
      // file there from the test.
      if (process.platform === "win32") return;

      const fifoPath = `/tmp/maestro-prompt-file-fifo-${Date.now()}`;
      const { spawnSync } = await import("node:child_process");
      const mk = spawnSync("mkfifo", [fifoPath]);
      if (mk.status !== 0) return;

      try {
        await expect(
          launchHandoff({
            missions: mockMissions(),
            git: makeGit(),
            handoffStore: makeHandoffStore(),
            launchers: {
              codex: { agent: "codex", async launch() { throw new Error("not used"); } },
              claude: { agent: "claude", async launch() { throw new Error("not used"); } },
            },
          }, {
            cwd: "/tmp/project",
            task: "probe",
            agent: "codex",
            wait: false,
            promptFile: fifoPath,
          }),
        ).rejects.toThrow("--prompt-file is not a regular file");
      } finally {
        await (await import("node:fs/promises")).rm(fifoPath, { force: true });
      }
    });

    it("throws a MaestroError when --prompt-file is empty", async () => {
      const briefPath = `/tmp/maestro-prompt-file-empty-${Date.now()}.md`;
      await Bun.write(briefPath, "   \n\n");

      await expect(
        launchHandoff({
          missions: mockMissions(),
          git: makeGit(),
          handoffStore: makeHandoffStore(),
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
          missions: mockMissions(),
          git: makeGit(),
          handoffStore: makeHandoffStore(),
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

  it("refuses to create a duplicate task-linked handoff", async () => {
    const handoffStore = makeHandoffStore();
    const codexLauncher: HandoffLaunchPort = {
      agent: "codex",
      async launch(request) {
        return {
          command: ["codex", "exec", request.prompt],
          pid: 1234,
        };
      },
    };
    const launchers = {
      codex: codexLauncher,
      claude: { agent: "claude" as const, async launch() { throw new Error("not used"); } },
    };

    await launchHandoff({
      missions: mockMissions(),
      git: makeGit(),
      handoffStore,
      launchers,
    }, {
      cwd: "/tmp/project",
      task: "First packet",
      agent: "codex",
      wait: false,
      refs: { taskId: "tsk-aaa111" },
    });

    await expect(
      launchHandoff({
        missions: mockMissions(),
        git: makeGit(),
        handoffStore,
        launchers,
      }, {
        cwd: "/tmp/project",
        task: "Second packet",
        agent: "codex",
        wait: false,
        refs: { taskId: "tsk-aaa111" },
      }),
    ).rejects.toThrow(/already has an open handoff packet/);
  });
});
