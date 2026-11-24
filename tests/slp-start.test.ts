import { createHash } from "node:crypto";
import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { realpathSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { buildSlpTeamPlan } from "../src/plugins/slp/runtime-plan.ts";
import { HerdrSlpRuntime } from "../src/plugins/slp/runtime.ts";
import {
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./helpers-herdr.ts";
import {
  addLinkedWorktree,
  initializeGitRepository,
  runCliAt,
  runCliBehindStrippedParentAt,
  withFixture,
} from "./helpers.ts";
import { envelope, phaseFree, failureEnvelope, testRoleContract } from "./helpers-slp.ts";

test("SLP v2 starts one ready generation with a pinned pack and initial Lead work", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Implement the approved change", "--json"],
      fake.env,
    );

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: {
        generation: number;
        packDigest: string;
        projectPath: string;
        roles: Array<{
          briefDigest: string;
          instanceId: string;
          name: string;
          packDigest: string;
          paneId: string;
          readyChallenge: string;
          role: string;
        }>;
        state: string;
        teamId: string;
      };
      work: { assignedTo: string; id: string; state: string };
    }>(started.stdout);
    const hubPack = await readFile(join(room, "SLP.md"));
    const projectPack = await readFile(join(fixture.repo, ".maestro", "SLP.md"));
    const expectedDigest = createHash("sha256").update(hubPack).digest("hex");
    const archivedPack = join(room, ".maestro", "packs", `${expectedDigest}.md`);

    expect(projectPack).toEqual(hubPack);
    expect(await readFile(archivedPack)).toEqual(hubPack);
    expect(startData.team).toMatchObject({
      generation: 1,
      packDigest: expectedDigest,
      projectPath: realpathSync.native(fixture.repo),
      state: "RUNNING",
    });
    expect(startData.team.roles).toEqual([
      expect.objectContaining({
        briefDigest: expect.stringMatching(/^[a-f0-9]{64}$/),
        instanceId: expect.stringMatching(/^[a-f0-9-]{36}$/),
        packDigest: expectedDigest,
        readyChallenge: expect.stringMatching(/^[a-f0-9]{32}$/),
        role: "lead",
      }),
    ]);
    expect(startData.work).toMatchObject({
      assignedTo: expect.stringMatching(/^lead-/),
      state: "OPEN",
    });

    const lead = startData.team.roles.find((role) => role.role === "lead");
    expect(
      (
        await runCliAt(fixture, fixture.repo, ["status", "--json"], {
          ...fake.env,
          HERDR_PANE_ID: lead?.paneId,
        })
      ).exitCode,
    ).toBe(0);

    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectDatabase
        .query<{ present: number }, []>(
          "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = 'slp_local_teams'",
        )
        .get()?.present,
    ).toBe(1);
    expect(
      projectDatabase
        .query<{ present: number }, []>(
          "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = 'slp_teams'",
        )
        .get(),
    ).toBeNull();
    projectDatabase.close();

    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents.every((agent: { agent_status: string }) => agent.agent_status === "idle"))
      .toBe(true);
    expect(runtime.agents.map((agent: { name: string }) => agent.name)).toEqual([
      startData.team.roles.find((role) => role.role === "lead")?.name,
    ]);
    const commands = await fakeHerdrCommands(fake);
    const allPrompts = commands.filter(
      (command) => command[0] === "agent" && command[1] === "prompt",
    );
    const prompts = allPrompts.filter((command) => (command[3] ?? "").startsWith("slp team "));
    expect(prompts).toHaveLength(1);
    expect(prompts.every((command) => command.includes("--wait") && command.includes("--timeout")))
      .toBe(true);
    // d90: the post-open prompt is one line naming team, generation, instance
    // and the challenge; the mandate lives in the seat file.
    for (const command of prompts) {
      expect(command[3]).toMatch(
        new RegExp(`^slp team ${startData.team.teamId} generation 1 instance [0-9a-f-]{36}; reply [0-9a-f]{32}$`),
      );
    }
    // w696: and the Lead's first item is pushed to it like any other item.
    expect(allPrompts.filter((command) => !prompts.includes(command))).toEqual([
      [
        "agent",
        "prompt",
        lead!.name,
        `[from hub-supervisor][${startData.work.id} OPEN] Implement the approved change; read: maestro status ${startData.work.id}`,
      ],
    ]);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "read"),
    ).toHaveLength(1);
  });
});

test("SLP v2 accepts presentation-only markers before exact role acknowledgements", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, {
      acknowledgementPrefixes: {
        lead: "• ",
        peer: "› ",
      },
      wrapAcknowledgements: true,
    });

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Accept terminal presentation markers", "--json"],
      fake.env,
    );

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    expect(lead).toBeDefined();

    const delegated = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Check Peer acknowledgement marker", "--to", "peer-marker", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );

    expect(phaseFree(delegated.stderr)).toBe("");
    expect(delegated.exitCode).toBe(0);
    const delegatedData = envelope<{ role: { name: string } }>(delegated.stdout);
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.outputs[lead?.name ?? ""]).toMatch(/^• SLP_ROLE_READY /);
    expect(runtime.outputs[delegatedData.role.name]).toMatch(/^› SLP_ROLE_READY /);
  });
}, 15_000);

test("SLP v2 reads the pane identity from an ancestor process when the shell dropped HERDR_PANE_ID", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Resolve the pane from a parent process", "--json"],
      fake.env,
    );
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const lead = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout).team.roles.find((role) => role.role === "lead");
    expect(lead).toBeDefined();

    const delegated = await runCliBehindStrippedParentAt(
      fixture,
      fixture.repo,
      ["work", "add", "Claim through the parent process", "--to", "peer-parent", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(delegated.stderr)).toBe("");
    expect(delegated.exitCode).toBe(0);
    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      expect(
        projectDatabase
          .query<{ created_by: string }, [string]>(
            "SELECT created_by FROM slp_work WHERE objective = ?",
          )
          .get("Claim through the parent process")?.created_by,
      ).toBe(lead?.name ?? "");
    } finally {
      projectDatabase.close();
    }
  });
}, 15_000);

test("SLP v2 rejects content before an otherwise exact role acknowledgement", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, {
      acknowledgementPrefixes: { lead: "answer: " },
      wrapAcknowledgements: true,
    });

    const failed = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Reject content before the acknowledgement", "--json"],
      fake.env,
    );

    expect(failed.exitCode).toBe(1);
    expect(failed.stderr).toContain("ROLE_ACKNOWLEDGEMENT_MISMATCH");
    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);
  });
}, 20_000);

test("SLP v2 rejects stale generation or challenge acknowledgements before granting authority", async () => {
  for (const invalidAcknowledgementField of ["generation", "challenge"] as const) {
    await withFixture(async (fixture) => {
      const room = await scaffoldRoom(fixture.home);
      expect(
        (
          await runCliAt(fixture, room, ["room", "mark"], {
            MAESTRO_ROOM_SCAFFOLD: "1",
            MAESTRO_SESSION_NONE: "1",
          })
        ).exitCode,
      ).toBe(0);
      const fake = await installFakeHerdr(fixture, {
        invalidAcknowledgementField,
        wrapAcknowledgements: true,
      });

      const failed = await runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Reject the stale role acknowledgement", "--json"],
        fake.env,
      );

      expect(failed.exitCode).toBe(1);
      expect(failed.stderr).toContain("ROLE_ACKNOWLEDGEMENT_MISMATCH");
      expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);
      const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
        readonly: true,
      });
      expect(
        projectDatabase.query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_local_roles",
        ).get()?.count,
      ).toBe(0);
      projectDatabase.close();
    });
  }
}, 30_000);

test("SLP v2 accepts one space between the challenge halves and nothing else (d768)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { spacedChallenge: true, wrapAcknowledgements: true });
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Accept the spaced challenge", "--json"],
      fake.env,
    );
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    expect(
      envelope<{ team: { roles: Array<{ role: string }> } }>(started.stdout).team.roles,
    ).toHaveLength(1);
  });
}, 30_000);

test("SLP v2 accepts an acknowledgement a narrow pane hard-wrapped inside a token", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { wrapAcknowledgementsAt: 39 });
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Accept the narrow-pane acknowledgement", "--json"],
      fake.env,
    );
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
  });
}, 30_000);

test("SLP v2 waits for newly created panes to become available shells", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { agentBusyAttempts: 2 });

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Wait for pane shells", "--json"],
      fake.env,
    );

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    expect(
      (await fakeHerdrCommands(fake)).filter(
        (command) => command[0] === "agent" && command[1] === "start",
      ),
    ).toHaveLength(3);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(1);
  });
});

test("SLP v2 retries a transient contract prompt stall without restarting roles", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { promptStalledAttempts: 2 });

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Retry contract delivery", "--json"],
      fake.env,
    );

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const commands = await fakeHerdrCommands(fake);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "start"),
    ).toHaveLength(1);
    expect(
      commands.filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("slp team "),
      ),
    ).toHaveLength(3);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(1);
  });
});

test("SLP v2 waits for an agent_not_ready launch without restarting the agent", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { codexNotReadyAttempts: 1 });

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Wait for the launched Codex role", "--json"],
      fake.env,
    );

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const commands = await fakeHerdrCommands(fake);
    const startIndex = commands.findIndex(
      (command) =>
        command[0] === "agent" && command[1] === "start" && command.includes("codex"),
    );
    const promptIndex = commands.findIndex(
      (command) =>
        command[0] === "agent" && command[1] === "prompt" && command[2]?.startsWith("lead-"),
    );
    expect(
      commands.filter(
        (command) =>
          command[0] === "agent" && command[1] === "start" && command.includes("codex"),
      ),
    ).toHaveLength(1);
    expect(
      commands.slice(startIndex + 1, promptIndex).some(
        (command) => command[0] === "agent" && command[1] === "list",
      ),
    ).toBe(true);
  });
});

test("SLP v2 gives Herdr agent startup its own bounded readiness window", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { agentStartDelayMs: 1_500 });
    const plan = buildSlpTeamPlan({
      generation: 1,
      lead: { harness: "codex" },
      projectPath: fixture.repo,
      teamId: "startup-window",
    });
    // The fixture's HOME holds the seat launch files, not the machine's.
    const runtime = new HerdrSlpRuntime(1_000, { ...process.env, ...fake.env, HOME: fixture.home });

    const started = await runtime.start(
      plan,
      new Map([["lead", testRoleContract(plan.teamId, plan.generation)]]),
    );

    expect(started.roles).toHaveLength(1);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(1);
  });
}, 10_000);

test("SLP v2 waits for workspace-close visibility before declaring runtime absence", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture);
    const plan = buildSlpTeamPlan({
      generation: 1,
      lead: { harness: "codex" },
      projectPath: fixture.repo,
      teamId: "close-visibility",
    });
    const runtime = new HerdrSlpRuntime(15_000, { ...process.env, ...fake.env, HOME: fixture.home });
    const started = await runtime.start(
      plan,
      new Map([["lead", testRoleContract(plan.teamId, plan.generation)]]),
    );
    await setFakeHerdrBehavior(fake, { workspaceCloseListLag: 2 });

    await runtime.stop(plan, started.roles);

    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);
  });
});

test("SLP v2 canonicalizes a project subdirectory to its checkout root", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const nested = join(fixture.repo, "src", "nested");
    await mkdir(nested, { recursive: true });
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const objective = "Canonicalize the checkout root";

    const nestedStart = await runCliAt(
      fixture,
      room,
      ["team", "start", nested, objective, "--json"],
      fake.env,
    );
    const rootStart = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, objective, "--json"],
      fake.env,
    );

    expect(phaseFree(nestedStart.stderr)).toBe("");
    expect(phaseFree(rootStart.stderr)).toBe("");
    const nestedData = envelope<{
      team: { generation: number; projectPath: string; teamId: string };
      work: { id: string };
    }>(nestedStart.stdout);
    expect(envelope(rootStart.stdout)).toMatchObject({
      team: {
        generation: nestedData.team.generation,
        projectPath: nestedData.team.projectPath,
        teamId: nestedData.team.teamId,
      },
      work: { id: nestedData.work.id },
    });
    expect(nestedData.team.projectPath).toBe(realpathSync.native(fixture.repo));
    expect(await readFakeHerdrState(fake)).toMatchObject({
      agents: [expect.any(Object)],
      workspaces: [expect.any(Object)],
    });
  });
}, 20_000);

test("SLP v2 derives cooperative authority only from the current Herdr pane binding", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const first = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
    }>((
      await runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Prove current pane authority", "--json"],
        fake.env,
      )
    ).stdout);
    const firstLead = first.team.roles.find(
      (role) => role.role === "lead",
    )!;
    const forgedSession = {
      ...fake.env,
      MAESTRO_SESSION_ID: firstLead.name,
      MAESTRO_SESSION_PID: String(process.pid),
    };

    for (const environment of [
      { ...forgedSession, HERDR_PANE_ID: undefined },
      { ...forgedSession, HERDR_PANE_ID: "pane-not-in-this-generation" },
    ]) {
      const rejected = await runCliAt(
        fixture,
        fixture.repo,
        ["work", "add", "Forged role must not write", "--json"],
        environment,
      );
      expect(rejected.exitCode).toBe(1);
      expect(JSON.parse(rejected.stderr).error.code).toBe("ROLE_UNPROVEN");
    }

    expect(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "stop", first.team.teamId, "--emergency", "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);
    const second = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
      };
    }>((
      await runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Prove stale pane rejection", "--json"],
        fake.env,
      )
    ).stdout);
    expect(second.team.generation).toBe(first.team.generation + 1);

    const stale = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Stale role must not write", "--json"],
      { ...forgedSession, HERDR_PANE_ID: firstLead.paneId },
    );
    expect(stale.exitCode).toBe(1);
    expect(JSON.parse(stale.stderr).error.code).toBe("ROLE_UNPROVEN");

    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(databasePath, { readonly: true });
    expect(
      database
        .query<{ count: number }, [number]>(
          "SELECT COUNT(*) AS count FROM slp_work WHERE generation = ?",
        )
        .get(second.team.generation)?.count,
    ).toBe(1);
    database.close();

    const secondLead = second.team.roles.find(
      (role) => role.role === "lead",
    )!;
    const accepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Current pane may write", "--json"],
      {
        ...forgedSession,
        MAESTRO_SESSION_ID: "irrelevant-forged-session",
        HERDR_PANE_ID: secondLead.paneId,
      },
    );
    expect(phaseFree(accepted.stderr)).toBe("");
    expect(accepted.exitCode).toBe(0);

    database = new Database(databasePath, { readonly: true });
    expect(
      database
        .query<{ count: number }, [number]>(
          "SELECT COUNT(*) AS count FROM slp_work WHERE generation = ?",
        )
        .get(second.team.generation)?.count,
    ).toBe(2);
    database.close();
  });
}, 25_000);

test("SLP v2 concurrently isolates linked-worktree teams and stopping one leaves the other running", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const linked = join(fixture.root, "linked");
    await addLinkedWorktree(fixture.repo, linked);
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const [firstResult, secondResult] = await Promise.all([
      runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Main checkout team", "--json"],
        fake.env,
      ),
      runCliAt(
        fixture,
        room,
        ["team", "start", linked, "Linked checkout team", "--json"],
        fake.env,
      ),
    ]);
    expect([firstResult, secondResult].map((result) => ({
      exitCode: result.exitCode,
      stderr: phaseFree(result.stderr),
    }))).toEqual([
      { exitCode: 0, stderr: "" },
      { exitCode: 0, stderr: "" },
    ]);
    const first = envelope<{
      team: {
        packDigest: string;
        projectPath: string;
        roles: Array<{ paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(firstResult.stdout);
    const second = envelope<{
      team: {
        packDigest: string;
        projectPath: string;
        roles: Array<{ paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(secondResult.stdout);
    expect(first.team.teamId).not.toBe(second.team.teamId);
    expect(first.work.id).not.toBe(second.work.id);
    expect(first.team.projectPath).toBe(realpathSync.native(fixture.repo));
    expect(second.team.projectPath).toBe(realpathSync.native(linked));
    expect(first.team.packDigest).toBe(second.team.packDigest);
    expect(await readFile(join(fixture.repo, ".maestro", "SLP.md"))).toEqual(
      await readFile(join(linked, ".maestro", "SLP.md")),
    );
    const firstLead = first.team.roles.find((role) => role.role === "lead")!;
    const secondLead = second.team.roles.find((role) => role.role === "lead")!;

    expect((await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      { ...fake.env, HERDR_PANE_ID: firstLead.paneId },
    )).exitCode).toBe(0);
    expect((await runCliAt(
      fixture,
      linked,
      ["status", "--json"],
      { ...fake.env, HERDR_PANE_ID: secondLead.paneId },
    )).exitCode).toBe(0);
    const crossed = await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      { ...fake.env, HERDR_PANE_ID: secondLead.paneId },
    );
    expect(crossed.exitCode).toBe(1);
    expect(JSON.parse(crossed.stderr).error.code).toBe("ROLE_UNPROVEN");
    const [firstDecision, secondDecision] = await Promise.all([
      runCliAt(
        fixture,
        fixture.repo,
        ["decide", "Main checkout choice", "--why", "main checkout evidence", "--json"],
        { ...fake.env, HERDR_PANE_ID: firstLead.paneId },
      ),
      runCliAt(
        fixture,
        linked,
        ["decide", "Linked checkout choice", "--why", "linked checkout evidence", "--json"],
        { ...fake.env, HERDR_PANE_ID: secondLead.paneId },
      ),
    ]);
    expect([firstDecision, secondDecision].map((result) => result.exitCode)).toEqual([0, 0]);
    expect(
      new Set(
        [firstDecision, secondDecision].map(
          (result) => envelope<{ decision: { id: string } }>(result.stdout).decision.id,
        ),
      ).size,
    ).toBe(2);

    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      roomDatabase
        .query<{ project_path: string; state: string; team_id: string }, []>(
          "SELECT team_id, project_path, state FROM slp_teams ORDER BY project_path",
        )
        .all(),
    ).toEqual([
      { project_path: first.team.projectPath, state: "RUNNING", team_id: first.team.teamId },
      { project_path: second.team.projectPath, state: "RUNNING", team_id: second.team.teamId },
    ].toSorted((left, right) => left.project_path.localeCompare(right.project_path)));
    roomDatabase.close();
    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectDatabase
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_local_teams")
        .get()?.count,
    ).toBe(2);
    expect(
      projectDatabase
        .query<{ id: string; team_id: string }, []>(
          "SELECT id, team_id FROM slp_work ORDER BY id",
        )
        .all(),
    ).toEqual([
      { id: first.work.id, team_id: first.team.teamId },
      { id: second.work.id, team_id: second.team.teamId },
    ].toSorted((left, right) => left.id.localeCompare(right.id, undefined, { numeric: true })));
    expect(
      projectDatabase
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_local_roles")
        .get()?.count,
    ).toBe(2);
    expect(
      projectDatabase
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_decisions")
        .get()?.count,
    ).toBe(2);
    expect(
      projectDatabase
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_lifecycle_operations WHERE operation = 'START'",
        )
        .get()?.count,
    ).toBe(2);
    projectDatabase.close();
    const runtimeBeforeStop = await readFakeHerdrState(fake);
    expect(runtimeBeforeStop.workspaces).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ cwd: first.team.projectPath }),
        expect.objectContaining({ cwd: second.team.projectPath }),
      ]),
    );
    expect(runtimeBeforeStop.agents).toHaveLength(2);

    const stoppedFirst = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        first.team.teamId,
        "--emergency",
        "--reason",
        "prove linked-worktree stop isolation",
        "--json",
      ],
      fake.env,
    );
    expect(stoppedFirst.exitCode).toBe(0);
    const secondStillRunning = await runCliAt(
      fixture,
      linked,
      ["status", "--json"],
      { ...fake.env, HERDR_PANE_ID: secondLead.paneId },
    );
    expect(secondStillRunning.exitCode).toBe(0);
    expect(envelope<{ runtime: string; teamId: string }>(secondStillRunning.stdout))
      .toMatchObject({ runtime: "available", teamId: second.team.teamId });
    expect(await readFakeHerdrState(fake)).toMatchObject({
      agents: [expect.any(Object)],
      workspaces: [expect.objectContaining({ cwd: second.team.projectPath })],
    });

    expect(
      (
        await runCliAt(
          fixture,
          room,
          [
            "team",
            "stop",
            second.team.teamId,
            "--emergency",
            "--reason",
            "finish linked-worktree proof",
            "--json",
          ],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);
  });
}, 25_000);

test("SLP v2 rejects a foreground role that never reaches a settled ready state", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { settleAgents: false });

    const failed = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Require settled roles", "--json"],
      fake.env,
    );

    expect(failed.exitCode).toBe(1);
    expect(failed.stderr).toContain("not ready");
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toEqual([]);
    expect(runtime.agents).toEqual([]);
    expect(await Bun.file(join(fixture.repo, ".maestro", "SLP.md")).exists()).toBe(false);
  });
});

test("SLP v2 gives same-basename projects distinct Herdr-safe team and role identities", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const firstProject = join(fixture.root, "north", "a-very-long-identical-project-name");
    const secondProject = join(fixture.root, "south", "a-very-long-identical-project-name");
    await mkdir(firstProject, { recursive: true });
    await mkdir(secondProject, { recursive: true });

    const first = await runCliAt(
      fixture,
      room,
      ["team", "start", firstProject, "First same-basename team", "--json"],
      fake.env,
    );
    const second = await runCliAt(
      fixture,
      room,
      ["team", "start", secondProject, "Second same-basename team", "--json"],
      fake.env,
    );

    expect(phaseFree(first.stderr)).toBe("");
    expect(phaseFree(second.stderr)).toBe("");
    const firstTeam = envelope<{
      team: { roles: Array<{ name: string }>; teamId: string };
    }>(first.stdout).team;
    const secondTeam = envelope<{
      team: { roles: Array<{ name: string }>; teamId: string };
    }>(second.stdout).team;
    expect(firstTeam.teamId).not.toBe(secondTeam.teamId);
    expect(new Set([...firstTeam.roles, ...secondTeam.roles].map((role) => role.name)).size).toBe(2);
    for (const name of [...firstTeam.roles, ...secondTeam.roles].map((role) => role.name)) {
      expect(name).toMatch(/^[a-z][a-z0-9_-]{0,31}$/);
    }
  });
}, 20_000);

test("SLP v2 polls for a late acknowledgement instead of failing on the first read", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { acknowledgementDelayReads: 3 });

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Wait for a slow acknowledgement", "--json"],
      fake.env,
    );

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const reads = (await fakeHerdrCommands(fake)).filter(
      (command) => command[0] === "agent" && command[1] === "read",
    );
    expect(reads.length).toBeGreaterThanOrEqual(4);
    expect(reads.every((command) => command.includes("visible"))).toBe(true);
    expect(started.stderr).toContain("waiting for acknowledgement (up to 30s)");
    expect(started.stderr).toMatch(/^lead-\S+: starting codex pane in slp-/m);
    expect(started.stderr).toMatch(/^lead-\S+: ready in \d+s$/m);
  });
}, 30_000);

test("SLP v2 reports an expired acknowledgement window with the pane tail", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { invalidAcknowledgementField: "challenge" });

    const failed = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Expire the acknowledgement window", "--json"],
      fake.env,
    );

    expect(failed.exitCode).toBe(1);
    const { error } = failureEnvelope(failed.stderr);
    expect(error.code).toBe("ROLE_ACKNOWLEDGEMENT_MISMATCH");
    expect(error.command).toEqual(["agent", "read", expect.stringMatching(/^lead-/)]);
    expect((error.paneTail as string[]).join("\n")).toContain("SLP_ROLE_READY");
    // project lesson l6 (w734 F2): the message itself carries the last
    // non-empty pane line, so a pane that died on a harness error (e.g. an
    // unsupported model) does not read as a merely slow seat. paneTail in the
    // data is unchanged.
    const tail = error.paneTail as string[];
    expect(error.message).toContain(`; last pane line: ${tail.at(-1)}`);
    expect(error.message).toContain("within 30s");
    const reads = (await fakeHerdrCommands(fake)).filter(
      (command) => command[0] === "agent" && command[1] === "read",
    );
    expect(reads.length).toBeGreaterThanOrEqual(4);
    expect(reads.at(-1)).toContain("recent_unwrapped");
    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);
  });
}, 30_000);

for (const harness of ["claude", "codex"] as const) {
  test(`SLP v2 names the ${harness} directory trust dialog instead of a generic not-ready failure`, async () => {
    await withFixture(async (fixture) => {
      const room = await scaffoldRoom(fixture.home);
      expect(
        (
          await runCliAt(fixture, room, ["room", "mark"], {
            MAESTRO_ROOM_SCAFFOLD: "1",
            MAESTRO_SESSION_NONE: "1",
          })
        ).exitCode,
      ).toBe(0);
      const fake = await installFakeHerdr(fixture, { trustDialog: harness });
      if (harness === "claude") {
        await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
        await writeFile(join(fixture.home, "maestro", "profiles", "lead.md"), "---\nharness: claude\n---\n");
      }

      const failed = await runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Hit the directory trust dialog", "--json"],
        fake.env,
      );

      expect(failed.exitCode).toBe(1);
      const { error } = failureEnvelope(failed.stderr);
      expect(error.code).toBe("TRUST_DIALOG");
      expect(error.harness).toBe(harness);
      expect(error.message).toContain(`${harness} is waiting on its directory trust dialog in `);
      expect(error.message).toContain(String(error.directory));
      expect((error.paneTail as string[]).join("\n")).toMatch(/trust/);
      expect(error.command).toEqual(expect.arrayContaining(["agent", "start", "--kind", harness]));
      const runtime = await readFakeHerdrState(fake);
      expect(runtime.workspaces).toEqual([]);
      expect(runtime.agents).toEqual([]);
    });
  }, 30_000);
}
