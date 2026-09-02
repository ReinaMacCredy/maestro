import { createHash } from "node:crypto";
import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync, realpathSync } from "node:fs";
import { mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import {
  buildSlpTeamPlan,
  HerdrSlpRuntime,
  type SlpRoleContract,
} from "../src/plugins/slp-runtime.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./fake-herdr.ts";
import {
  addLinkedWorktree,
  initializeGitRepository,
  runCli,
  runCliAt,
  withFixture,
} from "./helpers.ts";

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

const runtimePhaseLine =
  /^\S+: (?:starting (?:claude|codex) pane in \S+|waiting for acknowledgement \(up to \d+s\)|ready in \d+s|already acknowledged in \S+; left alone)$/;

// Runtime phase lines (d757) are progress, not failures.
function phaseFree(stderr: string): string {
  return stderr
    .split("\n")
    .filter((line) => line !== "" && !runtimePhaseLine.test(line))
    .join("\n");
}

function watchRuntimeDirectory(projectPath: string, teamId: string, generation: number): string {
  const projectKey = createHash("sha256")
    .update(realpathSync.native(resolve(projectPath)))
    .digest("hex")
    .slice(0, 16);
  const user = typeof process.getuid === "function" ? String(process.getuid()) : "user";
  return join(tmpdir(), `maestro-slp-${user}`, projectKey, teamId, `g${generation}`);
}

function testRoleContract(teamId: string, generation: number): SlpRoleContract {
  const instanceId = "00000000-0000-4000-8000-000000000001";
  const packDigest = "a".repeat(64);
  const briefDigest = "b".repeat(64);
  const readyChallenge = "c".repeat(32);
  const acknowledgement = [
    "SLP_ROLE_READY",
    `team=${teamId}`,
    `generation=${generation}`,
    "role=team-supervisor",
    `instance=${instanceId}`,
    `pack=${packDigest}`,
    `brief=${briefDigest}`,
    `challenge=${readyChallenge}`,
  ].join(" ");
  return {
    acknowledgement,
    body: [
      `Team: ${teamId}`,
      `Generation: ${generation}`,
      "Role: team-supervisor",
      `Role instance: ${instanceId}`,
      `Pack SHA-256: ${packDigest}`,
      `Brief SHA-256: ${briefDigest}`,
      `Challenge left: ${readyChallenge.slice(0, 16)}`,
      `Challenge right: ${readyChallenge.slice(16)}`,
    ].join("\n"),
    briefDigest,
    instanceId,
    packDigest,
    readyChallenge,
  };
}

async function waitForText(path: string, needle: string, timeoutMs = 3_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await Bun.file(path).exists()) {
      const text = await Bun.file(path).text();
      if (text.includes(needle)) return;
    }
    await Bun.sleep(25);
  }
  throw new Error(`timed out waiting for ${needle} in ${path}`);
}

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
      };
      work: { assignedTo: string; state: string };
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
        role: "team-supervisor",
      }),
      expect.objectContaining({
        briefDigest: expect.stringMatching(/^[a-f0-9]{64}$/),
        instanceId: expect.stringMatching(/^[a-f0-9-]{36}$/),
        packDigest: expectedDigest,
        readyChallenge: expect.stringMatching(/^[a-f0-9]{32}$/),
        role: "lead",
      }),
      expect.objectContaining({
        briefDigest: expect.stringMatching(/^[a-f0-9]{64}$/),
        instanceId: expect.stringMatching(/^[a-f0-9-]{36}$/),
        packDigest: expectedDigest,
        readyChallenge: expect.stringMatching(/^[a-f0-9]{32}$/),
        role: "observer",
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
    expect(runtime.agents.map((agent: { name: string }) => agent.name).sort()).toEqual(
      [
        startData.team.roles.find((role) => role.role === "lead")?.name,
        startData.team.roles.find((role) => role.role === "observer")?.name,
        startData.team.roles.find((role) => role.role === "team-supervisor")?.name,
      ].sort(),
    );
    const commands = await fakeHerdrCommands(fake);
    const prompts = commands.filter(
      (command) => command[0] === "agent" && command[1] === "prompt",
    );
    expect(prompts).toHaveLength(3);
    expect(prompts.every((command) => command.includes("--wait") && command.includes("--timeout")))
      .toBe(true);
    expect(
      prompts.every((command) => command[3]?.includes("SLP_ROLE_READY") === true),
    ).toBe(true);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "read"),
    ).toHaveLength(3);
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
        "team-supervisor": "⏺ ",
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
    const supervisor = startData.team.roles.find((role) => role.role === "team-supervisor");
    const lead = startData.team.roles.find((role) => role.role === "lead");
    expect(supervisor).toBeDefined();
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
    expect(runtime.outputs[supervisor?.name ?? ""]).toMatch(/^⏺ SLP_ROLE_READY /);
    expect(runtime.outputs[lead?.name ?? ""]).toMatch(/^• SLP_ROLE_READY /);
    expect(runtime.outputs[delegatedData.role.name]).toMatch(/^› SLP_ROLE_READY /);
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
      acknowledgementPrefixes: { "team-supervisor": "answer: " },
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

test("SLP v2 rejects stale digest or challenge acknowledgements before granting authority", async () => {
  for (const invalidAcknowledgementField of ["pack", "challenge"] as const) {
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
    ).toHaveLength(5);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(3);
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
    ).toHaveLength(3);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "prompt"),
    ).toHaveLength(5);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(3);
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
    ).toHaveLength(2);
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
      leadModel: "default",
      observerModel: "gpt-5.6-luna",
      projectPath: fixture.repo,
      supervisorModel: "default",
      teamId: "startup-window",
    });
    plan.roles = plan.roles.slice(0, 1);
    const runtime = new HerdrSlpRuntime(1_000, { ...process.env, ...fake.env });

    const started = await runtime.start(
      plan,
      new Map([["team-supervisor", testRoleContract(plan.teamId, plan.generation)]]),
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
      leadModel: "default",
      observerModel: "gpt-5.6-luna",
      projectPath: fixture.repo,
      supervisorModel: "default",
      teamId: "close-visibility",
    });
    plan.roles = plan.roles.slice(0, 1);
    const runtime = new HerdrSlpRuntime(15_000, { ...process.env, ...fake.env });
    const started = await runtime.start(
      plan,
      new Map([["team-supervisor", testRoleContract(plan.teamId, plan.generation)]]),
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
      agents: expect.arrayContaining([expect.any(Object), expect.any(Object)]),
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
    const firstSupervisor = first.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!;
    const forgedSession = {
      ...fake.env,
      MAESTRO_SESSION_ID: firstSupervisor.name,
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
      { ...forgedSession, HERDR_PANE_ID: firstSupervisor.paneId },
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

    const secondSupervisor = second.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!;
    const accepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Current pane may write", "--json"],
      {
        ...forgedSession,
        MAESTRO_SESSION_ID: "irrelevant-forged-session",
        HERDR_PANE_ID: secondSupervisor.paneId,
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
    ).toBe(6);
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
    expect(runtimeBeforeStop.agents).toHaveLength(6);

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
      agents: [expect.any(Object), expect.any(Object), expect.any(Object)],
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

test("SLP v2 repeats an identical start without duplicates and restores a missing Lead", async () => {
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
    const args = ["team", "start", fixture.repo, "Keep the same objective", "--json"];

    const first = await runCliAt(fixture, room, args, fake.env);
    const repeated = await runCliAt(fixture, room, args, fake.env);

    expect(first.exitCode).toBe(0);
    expect(phaseFree(repeated.stderr)).toBe("");
    expect(repeated.exitCode).toBe(0);
    const firstData = envelope<{
      team: { generation: number; roles: Array<{ instanceId: string; readyChallenge: string; role: string }> };
      work: { id: string };
    }>(first.stdout);
    const repeatedData = envelope<{
      team: { generation: number; roles: Array<{ instanceId: string; readyChallenge: string; role: string }> };
      work: { id: string };
    }>(
      repeated.stdout,
    );
    expect(repeatedData).toMatchObject({
      team: { generation: firstData.team.generation },
      work: { id: firstData.work.id },
    });
    expect(repeatedData.team.roles.map((role) => [role.role, role.instanceId])).toEqual(
      firstData.team.roles.map((role) => [role.role, role.instanceId]),
    );
    expect(repeatedData.team.roles.map((role) => role.readyChallenge)).toEqual(
      firstData.team.roles.map((role) => role.readyChallenge),
    );
    const prompts = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) => command[0] === "agent" && command[1] === "prompt",
      );
    expect(await prompts()).toHaveLength(3);
    let runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(3);

    const lead = runtime.agents.find((agent: { name: string }) => agent.name.startsWith("lead-"));
    expect(lead).toBeDefined();
    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<{ pane_id: string; name: string }>;
      const tabs = state.tabs as Array<{ root_pane_id: string }>;
      const panes = state.panes as Array<{ pane_id: string }>;
      const processes = state.processes as Record<string, unknown>;
      state.agents = agents.filter((agent) => agent.name !== lead.name);
      state.tabs = tabs.filter((tab) => tab.root_pane_id !== lead.pane_id);
      state.panes = panes.filter((pane) => pane.pane_id !== lead.pane_id);
      delete processes[lead.pane_id];
    });

    const repaired = await runCliAt(fixture, room, args, fake.env);

    expect(phaseFree(repaired.stderr)).toBe("");
    expect(repaired.exitCode).toBe(0);
    expect(
      envelope<{ team: { generation: number }; work: { id: string } }>(repaired.stdout),
    ).toMatchObject({
      team: { generation: firstData.team.generation },
      work: { id: firstData.work.id },
    });
    runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(3);
    expect(runtime.agents.filter((agent: { name: string }) => agent.name.startsWith("lead-"))).toHaveLength(1);
    const repairPrompts = await prompts();
    expect(repairPrompts).toHaveLength(4);
    expect(repairPrompts[3]?.[2]).toBe(lead.name);
    const repairedData = envelope<{
      team: { roles: Array<{ paneId: string; readyChallenge: string; role: string }>; teamId: string };
    }>(repaired.stdout);
    expect(repairedData.team.roles.find((role) => role.role === "team-supervisor")?.readyChallenge).toBe(
      firstData.team.roles.find((role) => role.role === "team-supervisor")?.readyChallenge,
    );
    const newLead = runtime.agents.find((agent: { name: string }) => agent.name.startsWith("lead-"));
    for (const storePath of [join(room, ".maestro", "maestro.db"), join(fixture.repo, ".maestro", "maestro.db")]) {
      const database = new Database(storePath, { readonly: true });
      const startRow = database
        .query<{ revision: number; runtime_json: string }, [string]>(
          "SELECT revision, runtime_json FROM slp_lifecycle_operations WHERE operation = 'START' AND team_id = ?",
        )
        .get(repairedData.team.teamId);
      database.close();
      const snapshot = JSON.parse(startRow?.runtime_json ?? "[]") as Array<{ paneId: string; role: string }>;
      expect(snapshot.find((role) => role.role === "lead")?.paneId).toBe(newLead.pane_id);
    }
  });
});

test("SLP v2 serializes ten concurrent starts into one generation and rejects contract contenders", async () => {
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
    const fake = await installFakeHerdr(fixture, { workspaceListDelayMs: 150 });
    const args = ["team", "start", fixture.repo, "Serialize identical starts", "--json"];

    const firstStart = runCliAt(fixture, room, args, fake.env);
    await waitForText(fake.log, '["workspace","list"]');
    const [first, ...contenders] = await Promise.all([
      firstStart,
      ...Array.from({ length: 9 }, () => runCliAt(fixture, room, args, fake.env)),
      runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Changed concurrent objective", "--json"],
        fake.env,
      ),
      runCliAt(
        fixture,
        room,
        [...args.slice(0, -1), "--lead-model", "different-model", "--json"],
        fake.env,
      ),
    ]);

    const identical = [first, ...contenders.slice(0, 9)];
    expect(identical.map((result) => ({ exitCode: result.exitCode, stderr: phaseFree(result.stderr) })))
      .toEqual(Array.from({ length: 10 }, () => ({ exitCode: 0, stderr: "" })));
    const outputs = identical.map((result) =>
      envelope<{ team: { generation: number; teamId: string }; work: { id: string } }>(
        result.stdout,
      )
    );
    expect(new Set(outputs.map((output) => output.team.generation)).size).toBe(1);
    expect(new Set(outputs.map((output) => output.team.teamId)).size).toBe(1);
    expect(new Set(outputs.map((output) => output.work.id)).size).toBe(1);
    for (const contender of contenders.slice(9)) {
      expect(contender.exitCode).toBe(1);
      expect(["TEAM_RUNNING", "TEAM_START_PENDING"]).toContain(
        (JSON.parse(contender.stderr) as { error: { code: string } }).error.code,
      );
    }
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(3);
    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(roomDatabase.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_teams").get()?.count)
      .toBe(1);
    expect(
      roomDatabase
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.start'",
        )
        .get()?.count,
    ).toBe(1);
    roomDatabase.close();
  });
}, 45_000);

test("SLP v2 waits for a running-generation repair before emergency stop", async () => {
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
    const args = ["team", "start", fixture.repo, "Serialize repair and stop", "--json"];
    const first = await runCliAt(fixture, room, args, fake.env);
    expect(first.exitCode).toBe(0);
    const teamId = envelope<{ team: { teamId: string } }>(first.stdout).team.teamId;
    await setFakeHerdrBehavior(fake, { processInfoDelayMs: 400 });
    const commandCount = (await fakeHerdrCommands(fake)).length;
    const repairing = runCliAt(fixture, room, args, fake.env);
    for (let attempt = 0; attempt < 100; attempt += 1) {
      const commands = await fakeHerdrCommands(fake);
      if (
        commands
          .slice(commandCount)
          .some((command) => command[0] === "pane" && command[1] === "process-info")
      ) break;
      await Bun.sleep(10);
    }
    const stopping = runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        teamId,
        "--emergency",
        "--reason",
        "serialize runtime repair before stop",
        "--json",
      ],
      fake.env,
    );
    await Bun.sleep(100);
    const duringRepair = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      duringRepair
        .query<{ owner_token: string | null }, []>(
          "SELECT owner_token FROM slp_lifecycle_operations WHERE operation = 'START'",
        )
        .get()?.owner_token,
    ).not.toBeNull();
    expect(
      duringRepair
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_lifecycle_operations WHERE operation = 'STOP'",
        )
        .get()?.count,
    ).toBe(0);
    duringRepair.close();
    const [repaired, stopped] = await Promise.all([repairing, stopping]);

    expect([repaired, stopped].map((result) => ({
      exitCode: result.exitCode,
      stderr: phaseFree(result.stderr),
    }))).toEqual([
      { exitCode: 0, stderr: "" },
      { exitCode: 0, stderr: "" },
    ]);
    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);
    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      roomDatabase
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop.emergency'",
        )
        .get()?.count,
    ).toBe(1);
    roomDatabase.close();
  });
}, 20_000);

test("SLP v2 releases SQLite before Herdr and persists monotonic start phases", async () => {
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
    const fake = await installFakeHerdr(fixture, { workspaceListDelayMs: 300 });
    const starting = runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Release the database before Herdr", "--json"],
      fake.env,
    );
    await waitForText(fake.log, '["workspace","list"]');

    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    projectDatabase.exec("PRAGMA busy_timeout = 0");
    let writeError: unknown = null;
    try {
      projectDatabase.exec(`
        BEGIN IMMEDIATE;
        CREATE TABLE lifecycle_writer_probe (value TEXT NOT NULL);
        INSERT INTO lifecycle_writer_probe (value) VALUES ('writer-progressed');
        COMMIT;
      `);
    } catch (error) {
      writeError = error;
      try {
        projectDatabase.exec("ROLLBACK");
      } catch {}
    }
    const reserved = projectDatabase
      .query<{ phase: string; revision: number }, []>(
        `SELECT phase, revision FROM slp_lifecycle_operations
         WHERE operation = 'START'`,
      )
      .get();
    projectDatabase.close();

    const started = await starting;
    expect(writeError).toBeNull();
    expect(reserved).toEqual({ phase: "RESERVED", revision: 1 });
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const after = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      after
        .query<{ phase: string; revision: number }, []>(
          `SELECT phase, revision FROM slp_lifecycle_operations
           WHERE operation = 'START'`,
        )
        .get(),
    ).toEqual({ phase: "COMMITTED", revision: 3 });
    expect(
      after.query<{ value: string }, []>("SELECT value FROM lifecycle_writer_probe").get()?.value,
    ).toBe("writer-progressed");
    after.close();
  });
}, 20_000);

test("SLP v2 retries start finalization from a durable RUNTIME_READY phase", async () => {
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
    const fake = await installFakeHerdr(fixture, { processInfoDelayMs: 500 });
    const args = [
      "team",
      "start",
      fixture.repo,
      "Retry the durable runtime boundary",
      "--json",
    ];
    const starting = runCliAt(fixture, room, args, fake.env);
    await waitForText(fake.log, '["pane","process-info"');
    const roomDatabasePath = join(room, ".maestro", "maestro.db");
    const roomDatabase = new Database(roomDatabasePath);
    roomDatabase.exec(`
      CREATE TRIGGER reject_start_finalization
      BEFORE INSERT ON slp_activity
      WHEN NEW.operation = 'team.start'
      BEGIN
        SELECT RAISE(ABORT, 'injected start finalization failure');
      END;
    `);
    roomDatabase.close();

    const failed = await starting;
    expect(failed.exitCode).toBe(1);
    expect(failed.stderr).toContain("injected start finalization failure");
    const projectAfterFailure = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfterFailure
        .query<{ phase: string }, []>(
          `SELECT phase FROM slp_lifecycle_operations WHERE operation = 'START'`,
        )
        .get()?.phase,
    ).toBe("RUNTIME_READY");
    expect(
      projectAfterFailure
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_local_teams")
        .get()?.count,
    ).toBe(0);
    expect(
      projectAfterFailure.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work")
        .get()?.count,
    ).toBe(0);
    expect(
      projectAfterFailure
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
        .get()?.count,
    ).toBe(0);
    projectAfterFailure.close();
    const roomAfterFailure = new Database(roomDatabasePath, { readonly: true });
    expect(
      roomAfterFailure.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_teams")
        .get()?.count,
    ).toBe(0);
    expect(
      roomAfterFailure
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_team_roles")
        .get()?.count,
    ).toBe(0);
    expect(
      roomAfterFailure.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
        .get()?.count,
    ).toBe(0);
    roomAfterFailure.close();

    const repairDatabase = new Database(roomDatabasePath);
    repairDatabase.exec("DROP TRIGGER reject_start_finalization");
    repairDatabase.close();
    await setFakeHerdrBehavior(fake, { processInfoDelayMs: 0 });
    const commandCount = (await fakeHerdrCommands(fake)).length;
    const retried = await runCliAt(fixture, room, args, fake.env);

    expect(phaseFree(retried.stderr)).toBe("");
    expect(retried.exitCode).toBe(0);
    const retryCommands = (await fakeHerdrCommands(fake)).slice(commandCount);
    expect(retryCommands.some((command) => command[0] === "workspace" && command[1] === "create"))
      .toBe(false);
    expect(retryCommands.some((command) => command[0] === "agent" && command[1] === "start"))
      .toBe(false);
    const projectAfterRetry = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfterRetry
        .query<{ phase: string }, []>(
          `SELECT phase FROM slp_lifecycle_operations WHERE operation = 'START'`,
        )
        .get()?.phase,
    ).toBe("COMMITTED");
    expect(
      projectAfterRetry
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_local_teams")
        .get()?.count,
    ).toBe(1);
    projectAfterRetry.close();
  });
}, 20_000);

test("SLP v2 resolves symlink aliases to one canonical running project", async () => {
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
    const alias = join(fixture.root, "repo-alias");
    await symlink(fixture.repo, alias, "dir");
    const objective = "Use one canonical project identity";
    const first = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, objective, "--json"],
      fake.env,
    );
    const repeated = await runCliAt(
      fixture,
      room,
      ["team", "start", alias, objective, "--json"],
      fake.env,
    );

    expect(phaseFree(first.stderr)).toBe("");
    expect(phaseFree(repeated.stderr)).toBe("");
    const firstData = envelope<{
      team: { generation: number; projectPath: string; teamId: string };
      work: { id: string };
    }>(first.stdout);
    expect(envelope(repeated.stdout)).toMatchObject({
      team: {
        generation: firstData.team.generation,
        projectPath: firstData.team.projectPath,
        teamId: firstData.team.teamId,
      },
      work: { id: firstData.work.id },
    });
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(3);
  });
}, 20_000);

test("SLP v2 rolls back a restored Lead and both role stores when persistence fails", async () => {
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
    const args = ["team", "start", fixture.repo, "Restore atomically", "--json"];
    const started = await runCliAt(fixture, room, args, fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const siblingPaneId = `${lead.paneId}:watch-sibling`;
    await editFakeHerdrState(fake, (state) => {
      state.agents = (state.agents as Array<{ name: string }>).filter(
        (agent) => agent.name !== lead.name,
      );
      const panes = state.panes as Array<{
        cwd: string;
        pane_id: string;
        tab_id?: string;
        workspace_id: string;
      }>;
      const leadPane = panes.find((pane) => pane.pane_id === lead.paneId)!;
      panes.push({ ...leadPane, pane_id: siblingPaneId });
      const processes = state.processes as Record<string, unknown>;
      delete processes[lead.paneId];
      processes[siblingPaneId] = {
        foreground_pgid: 9001,
        foreground_processes: [{ command: "maestro-slp-watch" }],
        pane_id: siblingPaneId,
      };
    });
    const roomDatabasePath = join(room, ".maestro", "maestro.db");
    const roomDatabase = new Database(roomDatabasePath);
    roomDatabase.exec(`
      CREATE TRIGGER reject_restored_lead
      BEFORE UPDATE OF pane_id ON slp_team_roles
      WHEN OLD.role = 'lead'
      BEGIN
        SELECT RAISE(ABORT, 'injected restored Lead persistence failure');
      END;
    `);
    roomDatabase.close();

    const failed = await runCliAt(fixture, room, args, fake.env);

    expect(failed.exitCode).toBe(1);
    expect(failed.stderr).toContain("injected restored Lead persistence failure");
    const projectAfter = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfter
        .query<{ pane_id: string }, [string]>("SELECT pane_id FROM slp_local_roles WHERE name = ?")
        .get(lead.name)?.pane_id,
    ).toBe(lead.paneId);
    projectAfter.close();
    const roomAfter = new Database(roomDatabasePath, { readonly: true });
    expect(
      roomAfter
        .query<{ pane_id: string }, [string]>("SELECT pane_id FROM slp_team_roles WHERE name = ?")
        .get(lead.name)?.pane_id,
    ).toBe(lead.paneId);
    roomAfter.close();
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.agents.some((agent: { name: string }) => agent.name === lead.name)).toBe(false);
    expect(runtime.panes.some((pane: { pane_id: string }) => pane.pane_id === lead.paneId)).toBe(false);
    expect(runtime.panes.some((pane: { pane_id: string }) => pane.pane_id === siblingPaneId)).toBe(true);
    expect(runtime.tabs.some((tab: { root_pane_id: string }) => tab.root_pane_id === siblingPaneId))
      .toBe(true);
  });
}, 15_000);

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
    expect(new Set([...firstTeam.roles, ...secondTeam.roles].map((role) => role.name)).size).toBe(6);
    for (const name of [...firstTeam.roles, ...secondTeam.roles].map((role) => role.name)) {
      expect(name).toMatch(/^[a-z][a-z0-9_-]{0,31}$/);
    }
  });
}, 20_000);

test("SLP v2 rolls back every resource and snapshot when start fails after its first effect", async () => {
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
    const fake = await installFakeHerdr(fixture, { agents: false });
    const args = ["team", "start", fixture.repo, "Rollback the failed start", "--json"];

    const failed = await runCliAt(fixture, room, args, fake.env);

    expect(failed.exitCode).toBe(1);
    let runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toEqual([]);
    expect(runtime.tabs).toEqual([]);
    expect(runtime.panes).toEqual([]);
    expect(runtime.agents).toEqual([]);
    expect(await Bun.file(join(fixture.repo, ".maestro", "SLP.md")).exists()).toBe(false);
    const pendingDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      pendingDatabase
        .query<{ phase: string; revision: number }, []>(
          `SELECT phase, revision FROM slp_lifecycle_operations
           WHERE operation = 'START'`,
        )
        .get(),
    ).toEqual({ phase: "RESERVED", revision: 2 });
    pendingDatabase.close();

    await setFakeHerdrBehavior(fake, { agents: true });
    const retried = await runCliAt(fixture, room, args, fake.env);

    expect(phaseFree(retried.stderr)).toBe("");
    expect(retried.exitCode).toBe(0);
    expect(envelope<{ team: { generation: number } }>(retried.stdout).team.generation).toBe(1);
    runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(3);
  });
});

test("SLP v2 rejects a changed objective or configuration without mutating the running team", async () => {
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
    const originalArgs = ["team", "start", fixture.repo, "Original objective", "--json"];
    const first = await runCliAt(fixture, room, originalArgs, fake.env);
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{ team: { generation: number }; work: { id: string } }>(first.stdout);
    const snapshotBefore = await readFile(join(fixture.repo, ".maestro", "SLP.md"));
    const runtimeBefore = await readFakeHerdrState(fake);

    for (const changedArgs of [
      ["team", "start", fixture.repo, "Changed objective", "--json"],
      [...originalArgs.slice(0, -1), "--lead-model", "different-model", "--json"],
    ]) {
      const rejected = await runCliAt(fixture, room, changedArgs, fake.env);
      expect(rejected.exitCode).toBe(1);
      expect(rejected.stderr).toContain("maestro team stop");
    }

    expect(await readFile(join(fixture.repo, ".maestro", "SLP.md"))).toEqual(snapshotBefore);
    expect(await readFakeHerdrState(fake)).toEqual(runtimeBefore);
    const repeated = await runCliAt(fixture, room, originalArgs, fake.env);
    expect(repeated.exitCode).toBe(0);
    expect(
      envelope<{ team: { generation: number }; work: { id: string } }>(repeated.stdout),
    ).toMatchObject({
      team: { generation: firstData.team.generation },
      work: { id: firstData.work.id },
    });
  });
});

test("SLP v2 keeps work objective and acceptance contract immutable across notes", async () => {
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
    const objective = "Ship parser; acceptance: malformed input is rejected";
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string; objective: string };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, objective, "--json"],
          fake.env,
        )
      ).stdout,
    );
    const supervisorPane = started.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const environment = { ...fake.env, HERDR_PANE_ID: supervisorPane };

    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          [
            "work",
            "note",
            started.work.id,
            "requested change: accept malformed input instead",
            "--json",
          ],
          environment,
        )
      ).exitCode,
    ).toBe(0);
    const status = envelope<{
      notes: Array<{ body: string }>;
      work: { objective: string };
    }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", started.work.id, "--json"],
          environment,
        )
      ).stdout,
    );
    expect(status.work.objective).toBe(objective);
    expect(status.notes.map((note) => note.body)).toEqual([
      "requested change: accept malformed input instead",
    ]);

    const replacement = await runCliAt(
      fixture,
      fixture.repo,
      [
        "work",
        "add",
        "Ship parser; acceptance: malformed input is accepted",
        "--json",
      ],
      environment,
    );
    expect(replacement.exitCode).toBe(0);
    expect(envelope<{ work: { id: string; objective: string } }>(replacement.stdout).work)
      .toMatchObject({
        id: "w2",
        objective: "Ship parser; acceptance: malformed input is accepted",
      });
  });
});

test("SLP v2 drives one work item through OPEN ACTIVE RETURNED DONE with atomic activity", async () => {
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
      ["team", "start", fixture.repo, "Complete the normal work journey", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string; state: string };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    const supervisor = startData.team.roles.find((role) => role.role === "team-supervisor");
    expect(lead).toBeDefined();
    expect(supervisor).toBeDefined();
    expect(startData.work.state).toBe("OPEN");

    const taken = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(taken.stderr)).toBe("");
    expect(envelope<{ work: { owner: string; state: string } }>(taken.stdout).work).toMatchObject({
      owner: lead?.name,
      state: "ACTIVE",
    });

    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", startData.work.id, "result: complete; proof: focused test", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(returned.stderr)).toBe("");
    expect(envelope<{ work: { owner: null; state: string } }>(returned.stdout).work).toMatchObject({
      owner: null,
      state: "RETURNED",
    });

    const accepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", startData.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisor?.paneId },
    );
    expect(phaseFree(accepted.stderr)).toBe("");
    expect(envelope<{ work: { state: string } }>(accepted.stdout).work.state).toBe("DONE");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const activity = database
      .query<{ operation: string }, []>(
        "SELECT operation FROM slp_activity ORDER BY id",
      )
      .all()
      .map((row) => row.operation);
    database.close();
    expect(activity).toEqual(["work.add", "work.take", "work.return", "work.accept"]);
  });
}, 15_000);

test("SLP v2 rejects a stale work transition without recording false activity", async () => {
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
      ["team", "start", fixture.repo, "Reject stale transition", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(databasePath);
    database.exec(`
      CREATE TRIGGER ignore_work_take
      BEFORE UPDATE OF state ON slp_work
      WHEN OLD.id = '${data.work.id}' AND NEW.state = 'ACTIVE'
      BEGIN
        SELECT RAISE(IGNORE);
      END;
    `);
    const activityBefore = database
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
      .get()?.count;
    database.close();

    const rejected = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", data.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: leadPane },
    );

    expect(rejected.exitCode).toBe(1);
    expect(rejected.stderr).toContain("INVALID_STATE");
    const after = new Database(databasePath, { readonly: true });
    expect(
      after.query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?")
        .get(data.work.id)?.state,
    ).toBe("OPEN");
    expect(
      after.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity").get()?.count,
    ).toBe(activityBefore);
    after.close();
  });
}, 15_000);

test("SLP v2 rolls back every project mutation when its activity record cannot be inserted", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Prove atomic project mutations", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find((role) => role.role === "lead")!.paneId;
    const supervisorPane = started.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisorPane };
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const rejectActivity = async (
      args: string[],
      environment: Record<string, string | undefined>,
    ) => {
      const before = new Database(databasePath);
      before.exec(`
        CREATE TRIGGER reject_activity_insert
        BEFORE INSERT ON slp_activity
        BEGIN
          SELECT RAISE(ABORT, 'injected activity insertion failure');
        END;
      `);
      before.close();
      const result = await runCliAt(fixture, fixture.repo, args, environment);
      const after = new Database(databasePath);
      after.exec("DROP TRIGGER reject_activity_insert");
      after.close();
      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("injected activity insertion failure");
    };
    const readDatabase = () => new Database(databasePath, { readonly: true });

    await rejectActivity(
      ["work", "take", started.work.id, "--json"],
      leadEnvironment,
    );
    let database = readDatabase();
    expect(
      database.query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?")
        .get(started.work.id)?.state,
    ).toBe("OPEN");
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", started.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "note", started.work.id, "must roll back", "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'NOTE'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();

    await rejectActivity(
      ["work", "return", started.work.id, "result: rejected audit", "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ current_return: string | null; return_revision: number; state: string }, [string]>(
          "SELECT state, current_return, return_revision FROM slp_work WHERE id = ?",
        )
        .get(started.work.id),
    ).toEqual({ current_return: null, return_revision: 0, state: "ACTIVE" });
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'RETURN'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", started.work.id, "result: first return", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "note", started.work.id, "grant must roll back", "--rework", "--json"],
      supervisorEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_rework_grants WHERE work_id = ?",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'NOTE'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "note", started.work.id, "reviewer grants rework", "--rework", "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "take", started.work.id, "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?")
        .get(started.work.id)?.state,
    ).toBe("RETURNED");
    expect(
      database
        .query<{ consumed_at: string | null }, [string]>(
          "SELECT consumed_at FROM slp_rework_grants WHERE work_id = ?",
        )
        .get(started.work.id)?.consumed_at,
    ).toBeNull();
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", started.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", started.work.id, "result: second return", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "accept", started.work.id, "--json"],
      supervisorEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ acceptance_outcome: string | null; state: string }, [string]>(
          "SELECT state, acceptance_outcome FROM slp_work WHERE id = ?",
        )
        .get(started.work.id),
    ).toEqual({ acceptance_outcome: null, state: "RETURNED" });
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'ACCEPTANCE'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", started.work.id, "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "add", "audit rejected work", "--json"],
      supervisorEnvironment,
    );
    database = readDatabase();
    expect(database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    database.close();
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "audit accepted work", "--json"],
      supervisorEnvironment,
    );
    expect(added.exitCode).toBe(0);
    expect(envelope<{ work: { id: string } }>(added.stdout).work.id).toBe("w2");

    await rejectActivity(
      ["decide", "Rejected decision", "--why", "audit unavailable", "--json"],
      supervisorEnvironment,
    );
    database = readDatabase();
    expect(
      database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_decisions").get()?.count,
    ).toBe(0);
    database.close();
    const decided = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "Accepted decision", "--why", "audit available", "--json"],
      supervisorEnvironment,
    );
    expect(decided.exitCode).toBe(0);
    expect(envelope<{ decision: { id: string } }>(decided.stdout).decision.id).toBe("d1");
  });
}, 30_000);

test("SLP v2 serializes ten concurrent work additions without duplicate ids or lost activity", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Serialize concurrent work writes", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const supervisorPane = started.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const environment = { ...fake.env, HERDR_PANE_ID: supervisorPane };

    const results = await Promise.all(
      Array.from({ length: 10 }, (_, index) =>
        runCliAt(
          fixture,
          fixture.repo,
          ["work", "add", `Concurrent work ${index}`, "--json"],
          environment,
        )
      ),
    );

    expect(results.map((result) => ({ exitCode: result.exitCode, stderr: result.stderr })))
      .toEqual(Array.from({ length: 10 }, () => ({ exitCode: 0, stderr: "" })));
    const ids = results.map(
      (result) => envelope<{ work: { id: string } }>(result.stdout).work.id,
    );
    expect(new Set(ids).size).toBe(10);
    expect(ids.toSorted()).toEqual(
      ["w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11"].toSorted(),
    );
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(11);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'work.add'",
        )
        .get()?.count,
    ).toBe(11);
    database.close();
  });
}, 20_000);

test("SLP v2 reports a structured store-busy failure without partial mutation", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Report busy stores", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const supervisorPane = started.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", "--json"],
          { ...fake.env, HERDR_PANE_ID: supervisorPane },
        )
      ).exitCode,
    ).toBe(0);
    const blocker = new Database(databasePath);
    blocker.exec("BEGIN IMMEDIATE");
    const rejected = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "must not partially commit", "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    blocker.exec("ROLLBACK");
    blocker.close();

    expect(rejected.exitCode).toBe(1);
    expect(JSON.parse(rejected.stderr)).toMatchObject({
      error: { code: "STORE_BUSY", sqliteCode: expect.stringMatching(/^SQLITE_BUSY/) },
      ok: false,
    });
    const database = new Database(databasePath, { readonly: true });
    expect(database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'work.add'",
        )
        .get()?.count,
    ).toBe(1);
    database.close();
  });
}, 45_000);

test("SLP v2 Lead adds OPEN work to one lazily created and reusable Peer", async () => {
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
      ["team", "start", fixture.repo, "Delegate one Peer task", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    const supervisor = startData.team.roles.find((role) => role.role === "team-supervisor");
    expect(lead).toBeDefined();
    const environment = { ...fake.env, HERDR_PANE_ID: lead?.paneId };

    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Design independently", "--to", "peer-design", "--json"],
      environment,
    );
    expect(phaseFree(first.stderr)).toBe("");
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; id: string; state: string };
    }>(first.stdout);
    const peerName = firstData.role.name;
    expect(firstData.role).toMatchObject({ name: expect.stringMatching(/^peer-design-/), role: "peer" });
    expect(firstData.work).toMatchObject({ assignedTo: firstData.role.name, state: "OPEN" });

    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Implement independently", "--to", "peer-design", "--json"],
      environment,
    );
    expect(phaseFree(second.stderr)).toBe("");
    expect(second.exitCode).toBe(0);
    const secondData = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; id: string; state: string };
    }>(second.stdout);
    expect(secondData.role).toEqual(firstData.role);
    expect(secondData.work).toMatchObject({ assignedTo: firstData.role.name, state: "OPEN" });
    expect(secondData.work.id).not.toBe(firstData.work.id);
    const peerPrompts = (await fakeHerdrCommands(fake)).filter(
      (command) =>
        command[0] === "agent" && command[1] === "prompt" && command[2] === peerName,
    );
    expect(peerPrompts).toHaveLength(1);

    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(
      runtime.agents
        .filter((agent: { name: string }) => agent.name.startsWith("peer-"))
        .map((agent: { name: string }) => agent.name),
    ).toEqual([firstData.role.name]);
  });
}, 15_000);

test("SLP v2 rolls back a newly provisioned Peer when its first work insert fails", async () => {
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
      ["team", "start", fixture.repo, "Rollback failed Peer work", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const peerName = `peer-rollback-${createHash("sha256")
      .update(`${data.team.teamId}\0rollback`)
      .digest("hex")
      .slice(0, 6)}`;
    const existingPeerPane = "existing-peer-root";
    const peerSiblingPane = "existing-peer-sibling";
    await editFakeHerdrState(fake, (state) => {
      const workspace = (state.workspaces as Array<{ workspace_id: string }>)[0]!;
      const tabId = "existing-peer-tab";
      (state.tabs as Array<Record<string, unknown>>).push({
        label: `slp:${data.team.teamId}:g1:peer:${peerName}`,
        root_pane_id: existingPeerPane,
        tab_id: tabId,
        workspace_id: workspace.workspace_id,
      });
      (state.panes as Array<Record<string, unknown>>).push(
        {
          cwd: fixture.repo,
          pane_id: existingPeerPane,
          tab_id: tabId,
          workspace_id: workspace.workspace_id,
        },
        {
          cwd: fixture.repo,
          pane_id: peerSiblingPane,
          tab_id: tabId,
          workspace_id: workspace.workspace_id,
        },
      );
      (state.processes as Record<string, unknown>)[peerSiblingPane] = {
        foreground_pgid: 9002,
        foreground_processes: [{ command: "sibling" }],
        pane_id: peerSiblingPane,
      };
    });
    const projectPath = join(fixture.repo, ".maestro", "maestro.db");
    const project = new Database(projectPath);
    project.exec(`
      CREATE TRIGGER reject_peer_work
      BEFORE INSERT ON slp_work
      WHEN NEW.assigned_to = '${peerName}'
      BEGIN
        SELECT RAISE(ABORT, 'injected peer work failure');
      END;
    `);
    project.close();

    const failed = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Must roll back", "--to", "peer-rollback", "--json"],
      { ...fake.env, HERDR_PANE_ID: leadPane },
    );

    expect(failed.exitCode).toBe(1);
    const projectAfter = new Database(projectPath, { readonly: true });
    expect(
      projectAfter
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_local_roles WHERE name = ?",
        )
        .get(peerName)?.count,
    ).toBe(0);
    projectAfter.close();
    const roomAfter = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      roomAfter
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_team_roles WHERE name = ?",
        )
        .get(peerName)?.count,
    ).toBe(0);
    roomAfter.close();
    const runtime = await readFakeHerdrState(fake);
    expect(
      runtime.agents.some((agent: { name: string }) => agent.name === peerName),
    ).toBe(false);
    expect(runtime.panes.some((pane: { pane_id: string }) => pane.pane_id === existingPeerPane))
      .toBe(false);
    expect(runtime.panes.some((pane: { pane_id: string }) => pane.pane_id === peerSiblingPane))
      .toBe(true);
    expect(runtime.tabs.some((tab: { root_pane_id: string }) => tab.root_pane_id === peerSiblingPane))
      .toBe(true);
  });
}, 15_000);

test("SLP v2 rejects Peer self-acceptance and lets the Lead accept the return", async () => {
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
      ["team", "start", fixture.repo, "Review Peer work", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    const supervisor = startData.team.roles.find((role) => role.role === "team-supervisor");
    expect(lead).toBeDefined();
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer-owned result", "--to", "peer-review", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    const addedData = envelope<{
      role: { name: string; paneId: string };
      work: { id: string };
    }>(added.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: addedData.role.paneId };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", addedData.work.id, "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", addedData.work.id, "result: peer result; proof: peer proof", "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const selfAccepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", addedData.work.id, "--json"],
      peerEnvironment,
    );
    expect(selfAccepted.exitCode).toBe(1);
    expect(selfAccepted.stderr).toContain("ROLE_FORBIDDEN");

    const wrongReviewer = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", addedData.work.id, "Supervisor cannot grant Peer rework", "--rework", "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisor?.paneId },
    );
    expect(wrongReviewer.exitCode).toBe(1);
    expect(JSON.parse(wrongReviewer.stderr).error.code).toBe("ROLE_FORBIDDEN");
    const leadGrant = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", addedData.work.id, "Lead grants Peer rework", "--rework", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(leadGrant.stderr)).toBe("");
    expect(leadGrant.exitCode).toBe(0);

    const leadAccepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", addedData.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(leadAccepted.stderr)).toBe("");
    expect(leadAccepted.exitCode).toBe(0);
    expect(envelope<{ work: { acceptedBy: string; state: string } }>(leadAccepted.stdout).work)
      .toMatchObject({ acceptedBy: lead?.name, state: "DONE" });
  });
}, 15_000);

test("SLP v2 rework requires one correct reviewer grant for the current return revision", async () => {
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
      ["team", "start", fixture.repo, "Exercise rework", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const supervisorPane = startData.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", startData.work.id, "result: first attempt", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const directRetake = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(directRetake.exitCode).toBe(1);
    expect(JSON.parse(directRetake.stderr).error.code).toBe("REWORK_REQUIRED");

    const wrongReviewer = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", startData.work.id, "self-granted rework", "--rework", "--json"],
      leadEnvironment,
    );
    expect(wrongReviewer.exitCode).toBe(1);
    expect(JSON.parse(wrongReviewer.stderr).error.code).toBe("ROLE_FORBIDDEN");

    const ordinaryNote = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", startData.work.id, "context: the missing case is Windows", "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    expect(ordinaryNote.exitCode).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(1);

    const granted = await runCliAt(
      fixture,
      fixture.repo,
      [
        "work",
        "note",
        startData.work.id,
        "add the missing Windows negative case",
        "--rework",
        "--json",
      ],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    expect(phaseFree(granted.stderr)).toBe("");
    expect(granted.exitCode).toBe(0);
    expect(envelope<{ work: { state: string } }>(granted.stdout).work.state).toBe("RETURNED");
    const duplicateGrant = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", startData.work.id, "duplicate grant", "--rework", "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    expect(duplicateGrant.exitCode).toBe(1);
    expect(JSON.parse(duplicateGrant.stderr).error.code).toBe("REWORK_ALREADY_GRANTED");

    const retaken = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(phaseFree(retaken.stderr)).toBe("");
    expect(envelope<{ work: { currentReturn: string | null; state: string } }>(retaken.stdout).work)
      .toMatchObject({ currentReturn: null, state: "ACTIVE" });

    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", startData.work.id, "result: second attempt", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const staleGrant = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(staleGrant.exitCode).toBe(1);
    expect(JSON.parse(staleGrant.stderr).error.code).toBe("REWORK_REQUIRED");
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "note", startData.work.id, "review the second return", "--rework", "--json"],
          { ...fake.env, HERDR_PANE_ID: supervisorPane },
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      database
        .query<{ count: number }, []>(
          `SELECT COUNT(*) AS count FROM slp_rework_grants
           WHERE consumed_at IS NOT NULL`,
        )
        .get()?.count,
    ).toBe(2);
    database.close();
  });
}, 15_000);

test("SLP v2 keeps a blocker in the return body and resumes without a BLOCKED state", async () => {
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
      ["team", "start", fixture.repo, "Resolve one blocker", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const supervisorPane = startData.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const blocker = "blocker: owner must provide the missing API contract";
    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", startData.work.id, blocker, "--json"],
      leadEnvironment,
    );
    expect(envelope<{ work: { currentReturn: string; state: string } }>(returned.stdout).work)
      .toMatchObject({ currentReturn: blocker, state: "RETURNED" });

    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          [
            "work",
            "note",
            startData.work.id,
            "resolved: contract supplied",
            "--rework",
            "--json",
          ],
          { ...fake.env, HERDR_PANE_ID: supervisorPane },
        )
      ).exitCode,
    ).toBe(0);
    const resumed = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(envelope<{ work: { state: string } }>(resumed.stdout).work.state).toBe("ACTIVE");
  });
}, 15_000);

test("SLP v2 cancels OPEN work through acceptance and requires ACTIVE work to return first", async () => {
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
      ["team", "start", fixture.repo, "Exercise cancellation", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const supervisorPane = startData.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisorPane };
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };

    const openCancelled = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", startData.work.id, "--outcome", "cancelled", "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(openCancelled.stderr)).toBe("");
    expect(
      envelope<{ work: { acceptanceOutcome: string; state: string } }>(openCancelled.stdout).work,
    ).toMatchObject({ acceptanceOutcome: "cancelled", state: "DONE" });

    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Cancel after start", "--json"],
      supervisorEnvironment,
    );
    const activeId = envelope<{ work: { id: string } }>(added.stdout).work.id;
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", activeId, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const rejected = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", activeId, "--outcome", "cancelled", "--json"],
      supervisorEnvironment,
    );
    expect(rejected.exitCode).toBe(1);
    expect(rejected.stderr).toContain("work return");
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", activeId, "result: cancellation requested", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const returnedCancelled = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", activeId, "--outcome", "cancelled", "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(returnedCancelled.stderr)).toBe("");
    expect(
      envelope<{ work: { acceptanceOutcome: string; state: string } }>(returnedCancelled.stdout).work,
    ).toMatchObject({ acceptanceOutcome: "cancelled", state: "DONE" });
  });
}, 15_000);

test("SLP v2 records one-step immutable decisions and preserves the replaced record", async () => {
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
      ["team", "start", fixture.repo, "Decide the technical boundary", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const environment = { ...fake.env, HERDR_PANE_ID: leadPane };

    const original = await runCliAt(
      fixture,
      fixture.repo,
      [
        "decide",
        "Use the direct adapter",
        "--why",
        "It is the narrowest stable seam",
        "--work",
        startData.work.id,
        "--json",
      ],
      environment,
    );
    expect(phaseFree(original.stderr)).toBe("");
    expect(original.exitCode).toBe(0);
    const originalDecision = envelope<{
      decision: { choice: string; id: string; scope: string };
    }>(original.stdout).decision;
    expect(originalDecision).toMatchObject({
      choice: "Use the direct adapter",
      scope: "technical",
    });

    const replacement = await runCliAt(
      fixture,
      fixture.repo,
      [
        "decide",
        "Use the bounded runtime adapter",
        "--why",
        "The runtime boundary needs explicit failure handling",
        "--work",
        startData.work.id,
        "--replaces",
        originalDecision.id,
        "--json",
      ],
      environment,
    );
    expect(phaseFree(replacement.stderr)).toBe("");
    const replacementDecision = envelope<{
      decision: { id: string; replaces: string };
    }>(replacement.stdout).decision;
    expect(replacementDecision.replaces).toBe(originalDecision.id);
    expect(replacementDecision.id).not.toBe(originalDecision.id);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const rows = database
      .query<{ choice: string; id: string; replaces_id: string | null }, []>(
        "SELECT id, choice, replaces_id FROM slp_decisions ORDER BY id",
      )
      .all();
    database.close();
    expect(rows).toEqual([
      { choice: "Use the direct adapter", id: originalDecision.id, replaces_id: null },
      {
        choice: "Use the bounded runtime adapter",
        id: replacementDecision.id,
        replaces_id: originalDecision.id,
      },
    ]);
  });
}, 15_000);

test("SLP v2 serializes ten concurrent decision ids without dropping a decision", async () => {
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
    expect((await runCliAt(fixture, room, ["status", "--json"])).exitCode).toBe(0);

    const results = await Promise.all(
      Array.from({ length: 10 }, (_, index) =>
        runCliAt(fixture, room, [
          "decide",
          `Concurrent choice ${index}`,
          "--why",
          `Concurrent reason ${index}`,
          "--json",
        ])
      ),
    );

    expect(results.map((result) => result.exitCode)).toEqual(Array(10).fill(0));
    const ids = results.map(
      (result) => envelope<{ decision: { id: string } }>(result.stdout).decision.id,
    );
    expect(new Set(ids).size).toBe(10);
  });
}, 15_000);

test("SLP v2 status gives Hub, team, and work-scoped truth and reports a missing pane", async () => {
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
      ["team", "start", fixture.repo, "Expose scoped status", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    const supervisor = startData.team.roles.find((role) => role.role === "team-supervisor");
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead?.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor?.paneId };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "note", startData.work.id, "note: implementation detail", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const decision = await runCliAt(
      fixture,
      fixture.repo,
      [
        "decide",
        "Keep the public seam small",
        "--why",
        "The team needs one status reader",
        "--work",
        startData.work.id,
        "--json",
      ],
      leadEnvironment,
    );
    const decisionId = envelope<{ decision: { id: string } }>(decision.stdout).decision.id;
    const returned = "result: status ready; proof: scoped CLI test";
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", startData.work.id, returned, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const hubStatus = await runCliAt(fixture, room, ["status", "--json"], fake.env);
    expect(phaseFree(hubStatus.stderr)).toBe("");
    expect(envelope<{ teams: Array<{ packDigest: string; state: string }> }>(hubStatus.stdout).teams)
      .toEqual([expect.objectContaining({ packDigest: expect.any(String), state: "RUNNING" })]);

    const teamStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(teamStatus.stderr)).toBe("");
    const teamData = envelope<{
      missingPanes: string[];
      runtime: string;
      roles: Array<{ role: string }>;
      watch: string;
      work: Array<{ id: string; state: string }>;
    }>(teamStatus.stdout);
    expect(teamData.roles.map((role) => role.role).sort()).toEqual(["lead", "observer", "team-supervisor"]);
    expect(teamData.work).toContainEqual(
      expect.objectContaining({ id: startData.work.id, state: "RETURNED" }),
    );
    expect(teamData).toMatchObject({ missingPanes: [], runtime: "available", watch: "off" });
    await editFakeHerdrState(fake, (state) => {
      for (const agent of state.agents as Array<{ agent_status: string }>) {
        agent.agent_status = "working";
      }
    });
    const workingStatus = envelope<{ missingPanes: string[]; runtime: string }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", "--json"],
          supervisorEnvironment,
        )
      ).stdout,
    );
    expect(workingStatus).toMatchObject({ missingPanes: [], runtime: "available" });
    await setFakeHerdrBehavior(fake, { processInfo: false });
    const unavailableStatus = envelope<{ missingPanes: string[]; runtime: string }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", "--json"],
          supervisorEnvironment,
        )
      ).stdout,
    );
    expect(unavailableStatus).toMatchObject({ missingPanes: [], runtime: "unavailable" });
    await setFakeHerdrBehavior(fake, { processInfo: true });

    const workStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", startData.work.id, "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(workStatus.stderr)).toBe("");
    expect(envelope<{
      decisions: Array<{ id: string }>;
      notes: Array<{ body: string }>;
      work: { currentReturn: string; id: string; state: string };
    }>(workStatus.stdout)).toMatchObject({
      decisions: [{ id: decisionId }],
      notes: [{ body: "note: implementation detail" }],
      work: { currentReturn: returned, id: startData.work.id, state: "RETURNED" },
    });

    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<{ name: string; pane_id: string }>;
      const panes = state.panes as Array<{ pane_id: string }>;
      const processes = state.processes as Record<string, unknown>;
      state.agents = agents.filter((agent) => agent.name !== lead?.name);
      state.panes = panes.filter((pane) => pane.pane_id !== lead?.paneId);
      if (lead?.paneId) delete processes[lead.paneId];
    });
    const degraded = await runCliAt(fixture, room, ["status", "--json"], fake.env);
    expect(
      envelope<{ teams: Array<{ missingPanes: string[] }> }>(degraded.stdout).teams[0]?.missingPanes,
    ).toEqual([lead!.name]);
    const degradedTeam = await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      supervisorEnvironment,
    );
    expect(
      envelope<{ missingPanes: string[] }>(degradedTeam.stdout).missingPanes,
    ).toEqual([lead!.name]);
  });
}, 20_000);

test("SLP v2 runtime reads fail within their configured deadline", async () => {
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
    const started = envelope<{
      team: {
        generation: number;
        projectPath: string;
        roles: Array<{
          briefDigest: string;
          instanceId: string;
          name: string;
          packDigest: string;
          paneId: string;
          readyChallenge: string;
          role: "lead" | "team-supervisor";
        }>;
        teamId: string;
      };
    }>((await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Bound runtime reads", "--json"],
      fake.env,
    )).stdout);
    await setFakeHerdrBehavior(fake, { processInfoDelayMs: 500 });
    const plan = buildSlpTeamPlan({
      generation: started.team.generation,
      leadModel: "default",
      observerModel: "gpt-5.6-luna",
      projectPath: started.team.projectPath,
      supervisorModel: "default",
      teamId: started.team.teamId,
    });
    const runtime = new HerdrSlpRuntime(50, { ...process.env, ...fake.env });
    const beganAt = Date.now();

    await expect(runtime.inspect(
      plan,
      started.team.roles.map((role) => ({
        briefDigest: role.briefDigest,
        instanceId: role.instanceId,
        name: role.name,
        packDigest: role.packDigest,
        paneId: role.paneId,
        readyChallenge: role.readyChallenge,
        role: role.role,
        workspaceId: "unused-by-inspect",
      })),
    )).rejects.toThrow("timed out after 50ms");
    expect(Date.now() - beganAt).toBeLessThan(1_000);
  });
}, 15_000);

test("SLP v2 pins Hub pack changes until the next stopped generation", async () => {
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
    const first = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Run generation one", "--json"],
      fake.env,
    );
    const firstData = envelope<{
      team: {
        generation: number;
        packDigest: string;
        roles: Array<{ paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(first.stdout);
    const pinned = await readFile(join(fixture.repo, ".maestro", "SLP.md"));
    const hubPackPath = join(room, "SLP.md");
    await writeFile(hubPackPath, `${await readFile(hubPackPath, "utf8")}\n<!-- generation-two -->\n`);
    expect(await readFile(join(fixture.repo, ".maestro", "SLP.md"))).toEqual(pinned);
    const runningStatus = envelope<{ teams: Array<{ packDigest: string }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout,
    );
    expect(runningStatus.teams[0]?.packDigest).toBe(firstData.team.packDigest);

    const leadPane = firstData.team.roles.find((role) => role.role === "lead")?.paneId;
    const supervisorPane = firstData.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", firstData.work.id, "--json"],
          { ...fake.env, HERDR_PANE_ID: leadPane },
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", firstData.work.id, "result: generation one complete", "--json"],
          { ...fake.env, HERDR_PANE_ID: leadPane },
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", firstData.work.id, "--json"],
          { ...fake.env, HERDR_PANE_ID: supervisorPane },
        )
      ).exitCode,
    ).toBe(0);
    const stopped = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", firstData.team.teamId, "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);

    const second = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Run generation two", "--json"],
      fake.env,
    );
    expect(phaseFree(second.stderr)).toBe("");
    const secondTeam = envelope<{ team: { generation: number; packDigest: string } }>(
      second.stdout,
    ).team;
    expect(secondTeam.generation).toBe(2);
    expect(secondTeam.packDigest).not.toBe(firstData.team.packDigest);
    expect(await readFile(join(fixture.repo, ".maestro", "SLP.md"))).toEqual(
      await readFile(hubPackPath),
    );
    await rm(join(fixture.repo, ".maestro", "SLP.md"));
    expect(
      await readFile(join(room, ".maestro", "packs", `${firstData.team.packDigest}.md`)),
    ).toEqual(pinned);
    expect(
      await readFile(join(room, ".maestro", "packs", `${secondTeam.packDigest}.md`)),
    ).toEqual(await readFile(hubPackPath));
  });
}, 20_000);

test("SLP v2 normal stop rejects every unfinished work item without mutation", async () => {
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
      ["team", "start", fixture.repo, "Keep unfinished work running", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(started.stdout);
    const supervisorPane = data.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    const runtimeBefore = await readFakeHerdrState(fake);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const activityBefore = database
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
      .get()?.count;
    database.close();

    const rejected = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", data.team.teamId, "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    expect(rejected.exitCode).toBe(1);
    expect(rejected.stderr).toContain(data.work.id);
    expect(await readFakeHerdrState(fake)).toEqual(runtimeBefore);
    const after = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(after.query<{ state: string }, []>("SELECT state FROM slp_local_teams").get()?.state)
      .toBe("RUNNING");
    expect(
      after.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity").get()?.count,
    ).toBe(activityBefore);
    after.close();
  });
}, 20_000);

test("SLP v2 normal stop closes Peer Lead Watch transcript Supervisor then workspace", async () => {
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
      ["team", "start", fixture.repo, "Close a complete team", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
        workspaceId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
    const peerWork = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Finish Peer work", "--to", "peer-stop", "--json"],
      leadEnvironment,
    );
    const peerData = envelope<{ role: { paneId: string }; work: { id: string } }>(peerWork.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peerData.role.paneId };
    for (const [workId, actorEnvironment, body] of [
      [data.work.id, leadEnvironment, "result: Lead done"],
      [peerData.work.id, peerEnvironment, "result: Peer done"],
    ] as const) {
      expect(
        (
          await runCliAt(
            fixture,
            fixture.repo,
            ["work", "take", workId, "--json"],
            actorEnvironment,
          )
        ).exitCode,
      ).toBe(0);
      expect(
        (
          await runCliAt(
            fixture,
            fixture.repo,
            ["work", "return", workId, body, "--json"],
            actorEnvironment,
          )
        ).exitCode,
      ).toBe(0);
    }
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", data.work.id, "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", peerData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const runtimeBeforeWatch = await readFakeHerdrState(fake);
    const supervisorTabId = (runtimeBeforeWatch.tabs as Array<{
      root_pane_id: string;
      tab_id: string;
    }>).find((tab) => tab.root_pane_id === supervisor.paneId)!.tab_id;
    const watchPaneId = `${supervisorTabId}:watch-pane`;
    await editFakeHerdrState(fake, (state) => {
      (state.panes as Array<Record<string, string>>).push({
        pane_id: watchPaneId,
        tab_id: supervisorTabId,
        workspace_id: data.team.workspaceId,
      });
      (state.processes as Record<string, unknown>)[watchPaneId] = {
        foreground_pgid: 9001,
        foreground_processes: [{
          args: ["maestro-slp-watch", "--team", data.team.teamId, "--generation", String(data.team.generation)],
          command: "maestro-slp-watch",
          pid: 9001,
        }],
      };
    });
    const runtimeDirectory = watchRuntimeDirectory(
      fixture.repo,
      data.team.teamId,
      data.team.generation,
    );
    const transcript = join(runtimeDirectory, "transcript.txt");
    await mkdir(runtimeDirectory, { recursive: true });
    await writeFile(transcript, "raw runtime transcript\n");
    await setFakeHerdrBehavior(fake, {
      closeWorkspaceWithLastTab: true,
      paneRunEmptyOutput: true,
      processInfo: false,
    });
    const unrelatedHubId = "unrelated-owner-hub";
    const unrelatedHubPath = join(fixture.root, "unrelated-owner-hub");
    await editFakeHerdrState(fake, (state) => {
      (state.workspaces as Array<Record<string, string>>).push({
        cwd: unrelatedHubPath,
        label: "maestro",
        workspace_id: unrelatedHubId,
      });
      (state.panes as Array<Record<string, string>>).push({
        cwd: unrelatedHubPath,
        pane_id: `${unrelatedHubId}:root`,
        workspace_id: unrelatedHubId,
      });
    });
    const before = await readFakeHerdrState(fake);
    const tabs = new Map(
      (before.tabs as Array<{ root_pane_id: string; tab_id: string }>).map((tab) => [
        tab.root_pane_id,
        tab.tab_id,
      ]),
    );
    const commandCountBeforeStop = (await fakeHerdrCommands(fake)).length;

    const stopped = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", data.team.teamId, "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);
    expect(envelope<{ team: { state: string } }>(stopped.stdout).team.state).toBe("STOPPED");
    const after = await readFakeHerdrState(fake);
    expect(after.workspaces).toEqual([{
      cwd: unrelatedHubPath,
      label: "maestro",
      workspace_id: unrelatedHubId,
    }]);
    expect(after.panes).toEqual([{
      cwd: unrelatedHubPath,
      pane_id: `${unrelatedHubId}:root`,
      workspace_id: unrelatedHubId,
    }]);
    expect(await Bun.file(transcript).exists()).toBe(false);

    const commands = await fakeHerdrCommands(fake);
    const stopCommands = commands.slice(commandCountBeforeStop);
    expect(
      stopCommands.some(
        (command) =>
          command[0] === "pane" &&
          command[1] === "run" &&
          command.some((argument) => argument.startsWith("MAESTRO_SLP_STOP_GRANT=")),
      ),
    ).toBe(true);
    expect(stopCommands.some((command) => command[0] === "agent" && command[1] === "start"))
      .toBe(false);
    expect(
      stopCommands.some(
        (command) =>
          command[0] === "tab" &&
          command[1] === "create" &&
          command.includes(unrelatedHubId),
      ),
    ).toBe(false);
    const closeIndex = (kind: "tab" | "workspace", id: string) =>
      commands.findIndex((command) => command[0] === kind && command[1] === "close" && command[2] === id);
    const peerTab = tabs.get(peerData.role.paneId)!;
    const leadTab = tabs.get(lead.paneId)!;
    const supervisorTab = tabs.get(supervisor.paneId)!;
    const peerClosed = closeIndex("tab", peerTab);
    const leadClosed = closeIndex("tab", leadTab);
    const watchClosed = commands.findIndex(
      (command) => command[0] === "pane" && command[1] === "close" && command[2] === watchPaneId,
    );
    const supervisorClosed = closeIndex("tab", supervisorTab);
    const workspaceClosed = closeIndex("workspace", data.team.workspaceId);
    expect(peerClosed).toBeGreaterThanOrEqual(0);
    expect(leadClosed).toBeGreaterThan(peerClosed);
    expect(watchClosed).toBeGreaterThan(leadClosed);
    expect(supervisorClosed).toBeGreaterThan(watchClosed);
    expect(workspaceClosed).toBe(-1);
  });
}, 20_000);

test("SLP v2 interrupted stop stays RUNNING and retry continues without duplicate closes", async () => {
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
      ["team", "start", fixture.repo, "Retry an interrupted stop", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const supervisorPane = data.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisorPane };
    expect(
      (
        await runCliAt(fixture, fixture.repo, ["work", "take", data.work.id, "--json"], leadEnvironment)
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", data.work.id, "result: ready to stop", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", data.work.id, "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await setFakeHerdrBehavior(fake, { closeResources: false });
    const interrupted = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", data.team.teamId, "--json"],
      supervisorEnvironment,
    );
    expect(interrupted.exitCode).toBe(1);
    const afterFirst = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(afterFirst.query<{ state: string }, []>("SELECT state FROM slp_local_teams").get()?.state)
      .toBe("RUNNING");
    expect(
      afterFirst
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop'",
        )
        .get()?.count,
    ).toBe(0);
    expect(
      afterFirst
        .query<{ phase: string; revision: number }, []>(
          `SELECT phase, revision FROM slp_lifecycle_operations
           WHERE operation = 'STOP'`,
        )
        .get(),
    ).toEqual({ phase: "RESERVED", revision: 2 });
    afterFirst.close();
    const firstCommands = await fakeHerdrCommands(fake);

    await setFakeHerdrBehavior(fake, { closeResources: true });
    const retried = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", data.team.teamId, "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(retried.stderr)).toBe("");
    expect(retried.exitCode).toBe(0);
    const retryCommands = (await fakeHerdrCommands(fake)).slice(firstCommands.length);
    expect(retryCommands.some((command) => command[0] === "tab" && command[1] === "close"))
      .toBe(true);
    const afterRetry = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(afterRetry.query<{ state: string }, []>("SELECT state FROM slp_local_teams").get()?.state)
      .toBe("STOPPED");
    expect(
      afterRetry
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop'",
        )
        .get()?.count,
    ).toBe(1);
    afterRetry.close();
  });
}, 20_000);

test("SLP v2 final workspace close failure leaves RUNNING and a restored Supervisor can retry", async () => {
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
    const started = envelope<{
      team: {
        roles: Array<{ paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Commit only after final close", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find((role) => role.role === "lead")!.paneId;
    const supervisorPane = started.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisorPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", started.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", started.work.id, "result: complete", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", started.work.id, "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const runtimeBeforeStop = await readFakeHerdrState(fake);
    const targetWorkspaceId = (runtimeBeforeStop.workspaces as Array<{
      label: string;
      workspace_id: string;
    }>).find((workspace) => workspace.label.startsWith("slp-"))!.workspace_id;
    await setFakeHerdrBehavior(fake, { failWorkspaceId: targetWorkspaceId });
    const interrupted = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", started.team.teamId, "--json"],
      supervisorEnvironment,
    );
    expect(interrupted.exitCode).toBe(1);
    const projectAfterFailure = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfterFailure.query<{ state: string }, []>("SELECT state FROM slp_local_teams").get()?.state,
    ).toBe("RUNNING");
    expect(
      projectAfterFailure
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop'",
        )
        .get()?.count,
    ).toBe(0);
    projectAfterFailure.close();
    const residual = envelope<{
      teams: Array<{ missingPanes: string[]; runtime: string; state: string; teamId: string }>;
    }>((await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout).teams[0]!;
    expect(residual).toMatchObject({ runtime: "available", state: "RUNNING", teamId: started.team.teamId });
    expect(residual.missingPanes.length).toBeGreaterThan(0);

    await setFakeHerdrBehavior(fake, { failWorkspaceId: "" });
    const restored = envelope<{
      team: { generation: number; roles: Array<{ paneId: string; role: string }> };
    }>((await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Commit only after final close", "--json"],
      fake.env,
    )).stdout);
    expect(restored.team.generation).toBe(1);
    const restoredSupervisor = restored.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const recovered = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", started.team.teamId, "--json"],
      { ...fake.env, HERDR_PANE_ID: restoredSupervisor },
    );
    expect(phaseFree(recovered.stderr)).toBe("");
    expect(recovered.exitCode).toBe(0);
    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);
    const projectAfterRecovery = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfterRecovery
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop'",
        )
        .get()?.count,
    ).toBe(1);
    projectAfterRecovery.close();
  });
}, 20_000);

test("SLP v2 releases SQLite before Herdr and persists monotonic stop phases", async () => {
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
    const started = envelope<{ team: { teamId: string } }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Release the database during stop", "--json"],
          fake.env,
        )
      ).stdout,
    );
    await setFakeHerdrBehavior(fake, { workspaceListDelayMs: 300 });
    const commandCount = (await fakeHerdrCommands(fake)).length;
    const stopping = runCliAt(
      fixture,
      room,
      ["team", "stop", started.team.teamId, "--emergency", "--json"],
      fake.env,
    );
    for (let attempt = 0; attempt < 100; attempt += 1) {
      const commands = await fakeHerdrCommands(fake);
      if (
        commands
          .slice(commandCount)
          .some((command) => command[0] === "workspace" && command[1] === "list")
      ) break;
      await Bun.sleep(10);
    }

    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    projectDatabase.exec("PRAGMA busy_timeout = 0");
    let writeError: unknown = null;
    try {
      projectDatabase.exec(`
        BEGIN IMMEDIATE;
        CREATE TABLE stop_writer_probe (value TEXT NOT NULL);
        INSERT INTO stop_writer_probe (value) VALUES ('writer-progressed');
        COMMIT;
      `);
    } catch (error) {
      writeError = error;
      try {
        projectDatabase.exec("ROLLBACK");
      } catch {}
    }
    const reserved = projectDatabase
      .query<{ phase: string; revision: number }, []>(
        `SELECT phase, revision FROM slp_lifecycle_operations
         WHERE operation = 'STOP'`,
      )
      .get();
    projectDatabase.close();

    const stopped = await stopping;
    expect(writeError).toBeNull();
    expect(reserved).toEqual({ phase: "RESERVED", revision: 1 });
    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);
    const after = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      after
        .query<{ phase: string; revision: number }, []>(
          `SELECT phase, revision FROM slp_lifecycle_operations
           WHERE operation = 'STOP'`,
        )
        .get(),
    ).toEqual({ phase: "COMMITTED", revision: 3 });
    expect(
      after.query<{ value: string }, []>("SELECT value FROM stop_writer_probe").get()?.value,
    ).toBe("writer-progressed");
    after.close();
  });
}, 20_000);

test("SLP v2 retries stop finalization from a durable RUNTIME_READY phase", async () => {
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
    const started = envelope<{ team: { teamId: string } }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Retry stop finalization", "--json"],
          fake.env,
        )
      ).stdout,
    );
    await setFakeHerdrBehavior(fake, { workspaceListDelayMs: 300 });
    const commandCount = (await fakeHerdrCommands(fake)).length;
    const stopping = runCliAt(
      fixture,
      room,
      ["team", "stop", started.team.teamId, "--emergency", "--json"],
      fake.env,
    );
    for (let attempt = 0; attempt < 100; attempt += 1) {
      const commands = await fakeHerdrCommands(fake);
      if (
        commands
          .slice(commandCount)
          .some((command) => command[0] === "workspace" && command[1] === "list")
      ) break;
      await Bun.sleep(10);
    }
    const roomDatabasePath = join(room, ".maestro", "maestro.db");
    const roomDatabase = new Database(roomDatabasePath);
    roomDatabase.exec(`
      CREATE TRIGGER reject_stop_finalization
      BEFORE INSERT ON slp_activity
      WHEN NEW.operation = 'team.stop.emergency'
      BEGIN
        SELECT RAISE(ABORT, 'injected stop finalization failure');
      END;
    `);
    roomDatabase.close();

    const failed = await stopping;
    expect(failed.exitCode).toBe(1);
    expect(failed.stderr).toContain("injected stop finalization failure");
    const projectAfterFailure = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfterFailure
        .query<{ phase: string }, []>(
          `SELECT phase FROM slp_lifecycle_operations WHERE operation = 'STOP'`,
        )
        .get()?.phase,
    ).toBe("RUNTIME_READY");
    expect(
      projectAfterFailure.query<{ state: string }, []>("SELECT state FROM slp_local_teams").get()?.state,
    ).toBe("RUNNING");
    expect(
      projectAfterFailure
        .query<{ abandoned_at: string | null }, []>("SELECT abandoned_at FROM slp_work")
        .get()?.abandoned_at,
    ).toBeNull();
    expect(
      projectAfterFailure
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop.emergency'",
        )
        .get()?.count,
    ).toBe(0);
    projectAfterFailure.close();
    const roomAfterFailure = new Database(roomDatabasePath, { readonly: true });
    expect(
      roomAfterFailure.query<{ state: string }, []>("SELECT state FROM slp_teams").get()?.state,
    ).toBe("RUNNING");
    expect(
      roomAfterFailure
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop.emergency'",
        )
        .get()?.count,
    ).toBe(0);
    roomAfterFailure.close();
    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);

    const changedReason = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        started.team.teamId,
        "--emergency",
        "--reason",
        "replace the already pinned reason",
        "--json",
      ],
      fake.env,
    );
    expect(changedReason.exitCode).toBe(1);
    expect(JSON.parse(changedReason.stderr).error.code).toBe("EMERGENCY_REASON_CHANGED");

    const repairDatabase = new Database(roomDatabasePath);
    repairDatabase.exec("DROP TRIGGER reject_stop_finalization");
    repairDatabase.close();
    await setFakeHerdrBehavior(fake, { workspaceListDelayMs: 0 });
    const retryCommandCount = (await fakeHerdrCommands(fake)).length;
    const retried = await runCliAt(
      fixture,
      room,
      ["team", "stop", started.team.teamId, "--emergency", "--json"],
      fake.env,
    );

    expect(phaseFree(retried.stderr)).toBe("");
    expect(retried.exitCode).toBe(0);
    expect(
      (await fakeHerdrCommands(fake))
        .slice(retryCommandCount)
        .some((command) => command[0] === "tab" && command[1] === "close"),
    ).toBe(false);
    const projectAfterRetry = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfterRetry
        .query<{ phase: string }, []>(
          `SELECT phase FROM slp_lifecycle_operations WHERE operation = 'STOP'`,
        )
        .get()?.phase,
    ).toBe("COMMITTED");
    expect(
      projectAfterRetry.query<{ state: string }, []>("SELECT state FROM slp_local_teams").get()?.state,
    ).toBe("STOPPED");
    expect(
      projectAfterRetry
        .query<{ count: number }, []>(
          `SELECT COUNT(*) AS count FROM slp_activity
           WHERE operation = 'team.stop.emergency'`,
        )
        .get()?.count,
    ).toBe(1);
    projectAfterRetry.close();
  });
}, 20_000);

test("SLP v2 stop fences every competing mutation and repeated stop before committing once", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Serialize stop and work", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find((role) => role.role === "lead")!.paneId;
    const supervisorPane = started.team.roles.find(
      (role) => role.role === "team-supervisor",
    )!.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisorPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", started.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", started.work.id, "result: complete", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", started.work.id, "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const commandCount = (await fakeHerdrCommands(fake)).length;
    await setFakeHerdrBehavior(fake, { workspaceListDelayMs: 400 });
    const stopping = runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", started.team.teamId, "--json"],
      supervisorEnvironment,
    );
    for (let attempt = 0; attempt < 100; attempt += 1) {
      const commands = await fakeHerdrCommands(fake);
      if (commands.slice(commandCount).some((command) => command.join(" ") === "workspace list")) break;
      await Bun.sleep(10);
    }
    const statusDuringStop = runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      supervisorEnvironment,
    );
    const competing = await Promise.all([
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "add", "late work", "--json"],
        supervisorEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "note", started.work.id, "late note", "--json"],
        supervisorEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "take", started.work.id, "--json"],
        leadEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "return", started.work.id, "late return", "--json"],
        leadEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "accept", started.work.id, "--json"],
        supervisorEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["decide", "Late decision", "--why", "stop already fenced writes", "--json"],
        supervisorEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["team", "stop", started.team.teamId, "--json"],
        supervisorEnvironment,
      ),
      runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Serialize stop and work", "--json"],
        fake.env,
      ),
    ]);
    const duringStop = await statusDuringStop;
    const stopped = await stopping;

    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);
    expect(duringStop.exitCode).toBe(0);
    expect(envelope<{ teamId: string }>(duringStop.stdout).teamId).toBe(started.team.teamId);
    expect(
      competing.map((result) => ({
        code: (JSON.parse(result.stderr) as { error: { code: string } }).error.code,
        exitCode: result.exitCode,
      })),
    ).toEqual(
      Array.from({ length: 8 }, () => ({ code: "TEAM_STOP_IN_PROGRESS", exitCode: 1 })),
    );
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      project
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE kind = 'NOTE'",
        )
        .get()?.count,
    ).toBe(0);
    expect(project.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    expect(
      project
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop'",
        )
        .get()?.count,
    ).toBe(1);
    expect(
      project.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_decisions").get()?.count,
    ).toBe(0);
    project.close();
  });
}, 20_000);

test("SLP v2 Hub emergency stop abandons every unfinished item in its original generation", async () => {
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
      ["team", "start", fixture.repo, "Emergency stop active work", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { generation: number; roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const supervisorPane = data.team.roles.find(
      (role) => role.role === "team-supervisor",
    )?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisorPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", data.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const openWork = envelope<{ work: { id: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "add", "Remain open during emergency", "--json"],
          supervisorEnvironment,
        )
      ).stdout,
    ).work;
    const returnedWork = envelope<{ work: { id: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "add", "Return before emergency", "--json"],
          supervisorEnvironment,
        )
      ).stdout,
    ).work;
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", returnedWork.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", returnedWork.id, "blocker: owner ended the generation", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const reason = "owner cancelled the generation after requirements changed";
    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--emergency", "--reason", reason, "--json"],
      fake.env,
    );
    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);
    expect(envelope<{ emergency: boolean; team: { state: string } }>(stopped.stdout)).toMatchObject({
      emergency: true,
      team: { state: "STOPPED" },
    });
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const abandoned = project
      .query<{
        abandoned_at: string;
        abandoned_by: string;
        abandonment_reason: string;
        generation: number;
        id: string;
        state: string;
      }, []>(
        `SELECT id, generation, state, abandoned_at, abandoned_by, abandonment_reason
         FROM slp_work ORDER BY id`,
      )
      .all();
    expect(abandoned.map((work) => [work.id, work.state])).toEqual([
      [data.work.id, "ACTIVE"],
      [openWork.id, "OPEN"],
      [returnedWork.id, "RETURNED"],
    ]);
    expect(
      abandoned.every(
        (work) =>
          work.generation === data.team.generation &&
          work.abandoned_by === "hub-supervisor" &&
          work.abandonment_reason === reason &&
          work.abandoned_at.length > 0,
      ),
    ).toBe(true);
    expect(
      project
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'team.stop.emergency'",
        )
        .get()?.count,
    ).toBe(1);
    project.close();
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toEqual([]);
    expect(runtime.panes).toEqual([]);
    const stoppedStatus = envelope<{
      teams: Array<{ abandonedWorkCount: number; generation: number }>;
    }>((await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout);
    expect(
      stoppedStatus.teams.find((team) => team.generation === data.team.generation)
        ?.abandonedWorkCount,
    ).toBe(3);

    const restarted = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Start a clean generation", "--json"],
      fake.env,
    );
    expect(phaseFree(restarted.stderr)).toBe("");
    const restartedData = envelope<{
      team: { generation: number; roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(restarted.stdout);
    expect(restartedData.team.generation).toBe(data.team.generation + 1);
    expect([data.work.id, openWork.id, returnedWork.id]).not.toContain(restartedData.work.id);
    const newLeadPane = restartedData.team.roles.find((role) => role.role === "lead")?.paneId;
    const oldMutation = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", data.work.id, "mutate abandoned work", "--json"],
      { ...fake.env, HERDR_PANE_ID: newLeadPane },
    );
    expect(oldMutation.exitCode).toBe(1);
    expect(JSON.parse(oldMutation.stderr).error.code).toBe("NOT_FOUND");
    const afterRestart = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      afterRestart
        .query<{ count: number }, [number]>(
          `SELECT COUNT(*) AS count FROM slp_work
           WHERE generation = ? AND abandoned_at IS NOT NULL`,
        )
        .get(data.team.generation)?.count,
    ).toBe(3);
    afterRestart.close();
  });
}, 20_000);

test("SLP v2 enforces the locked operation authority matrix without forbidden mutation", async () => {
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
      ["team", "start", fixture.repo, "Exercise every role boundary", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: {
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
    const peerWork = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer matrix work", "--to", "peer-matrix", "--json"],
      leadEnvironment,
    );
    const peerData = envelope<{ role: { paneId: string }; work: { id: string } }>(peerWork.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peerData.role.paneId };

    const allowed: Array<{
      args: string[];
      cwd: string;
      env: Record<string, string | undefined>;
    }> = [
      { args: ["status", "--json"], cwd: room, env: fake.env },
      { args: ["decide", "Owner matrix choice", "--why", "owner authority", "--json"], cwd: room, env: fake.env },
      { args: ["status", "--json"], cwd: fixture.repo, env: supervisorEnvironment },
      { args: ["work", "note", data.work.id, "supervisor note", "--json"], cwd: fixture.repo, env: supervisorEnvironment },
      { args: ["decide", "Team matrix choice", "--why", "team authority", "--json"], cwd: fixture.repo, env: supervisorEnvironment },
      { args: ["status", "--json"], cwd: fixture.repo, env: leadEnvironment },
      { args: ["work", "note", data.work.id, "lead note", "--json"], cwd: fixture.repo, env: leadEnvironment },
      { args: ["decide", "Technical matrix choice", "--why", "lead authority", "--json"], cwd: fixture.repo, env: leadEnvironment },
      { args: ["status", peerData.work.id, "--json"], cwd: fixture.repo, env: peerEnvironment },
      { args: ["work", "note", peerData.work.id, "peer note", "--json"], cwd: fixture.repo, env: peerEnvironment },
    ];
    for (const command of allowed) {
      const result = await runCliAt(fixture, command.cwd, command.args, command.env);
      expect(phaseFree(result.stderr)).toBe("");
      expect(result.exitCode).toBe(0);
    }

    const projectBefore = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const slpBefore = projectBefore
      .query<{ id: string; state: string }, []>("SELECT id, state FROM slp_work ORDER BY id")
      .all();
    const activityBefore = projectBefore
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
      .get()?.count;
    projectBefore.close();

    const forbidden: Array<{
      args: string[];
      cwd: string;
      env: Record<string, string | undefined>;
      role: string;
    }> = [
      { args: ["work", "add", "Hub team work", "--to", lead.name, "--json"], cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["work", "take", data.work.id, "--json"], cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["work", "note", data.work.id, "hub note", "--json"], cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["work", "return", data.work.id, "hub return", "--json"], cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["work", "accept", data.work.id, "--json"], cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["team", "stop", data.team.teamId, "--json"], cwd: room, env: fake.env, role: "Hub Supervisor" },
      {
        args: ["team", "stop", data.team.teamId, "--json"],
        cwd: room,
        env: {
          ...fake.env,
          MAESTRO_SLP_STOP_CLOSE_WORKSPACE: "0",
          MAESTRO_SLP_STOP_GRANT: "forged-token",
          MAESTRO_SLP_STOP_HELPER_TAB: "forged-tab",
          MAESTRO_SLP_STOP_HELPER_WORKSPACE: "forged-workspace",
          MAESTRO_SLP_STOP_PROJECT: fixture.repo,
        },
        role: "Forged stop helper",
      },
      { args: ["team", "start", fixture.repo, "Supervisor cannot start", "--json"], cwd: fixture.repo, env: supervisorEnvironment, role: "Team Supervisor" },
      { args: ["work", "take", data.work.id, "--json"], cwd: fixture.repo, env: supervisorEnvironment, role: "Team Supervisor" },
      { args: ["work", "return", data.work.id, "bad", "--json"], cwd: fixture.repo, env: supervisorEnvironment, role: "Team Supervisor" },
      { args: ["team", "start", fixture.repo, "Lead cannot start", "--json"], cwd: fixture.repo, env: leadEnvironment, role: "Lead" },
      { args: ["team", "stop", data.team.teamId, "--json"], cwd: fixture.repo, env: leadEnvironment, role: "Lead" },
      { args: ["team", "start", fixture.repo, "Peer cannot start", "--json"], cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["team", "stop", data.team.teamId, "--json"], cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["work", "add", "Peer cannot add", "--to", "peer-other", "--json"], cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["work", "accept", peerData.work.id, "--json"], cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["decide", "Peer cannot decide", "--why", "no authority", "--json"], cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
    ];
    for (const attempt of forbidden) {
      const result = await runCliAt(fixture, attempt.cwd, attempt.args, attempt.env);
      expect(result.exitCode, `${attempt.role}: ${attempt.args.join(" ")}`).toBe(1);
    }

    const projectAfter = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfter.query<{ id: string; state: string }, []>("SELECT id, state FROM slp_work ORDER BY id").all(),
    ).toEqual(slpBefore);
    expect(
      projectAfter.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity").get()?.count,
    ).toBe(activityBefore);
    projectAfter.close();
  });
}, 25_000);

test("SLP v2 hard cut rejects old verbs and leaves legacy records read-only", async () => {
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
    const roomDatabasePath = join(room, ".maestro", "maestro.db");
    const legacy = new Database(roomDatabasePath);
    legacy.exec(`
      CREATE TABLE legacy_team_history (
        id TEXT PRIMARY KEY,
        payload TEXT NOT NULL
      );
      INSERT INTO legacy_team_history (id, payload)
      VALUES ('legacy-g1', 'opaque old lifecycle bytes');
    `);
    legacy.close();
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Use only the v2 surface", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    for (const attempt of [
      { args: ["team", "open", data.team.teamId, "--json"], cwd: room, env: fake.env, replacement: "team start" },
      { args: ["team", "health", data.team.teamId, "--json"], cwd: room, env: fake.env, replacement: "status" },
      { args: ["dispatch", "open", data.work.id, "--json"], cwd: fixture.repo, env: leadEnvironment, replacement: "work add" },
      { args: ["handback", "file", "x1", "--json"], cwd: fixture.repo, env: leadEnvironment, replacement: "work return" },
      { args: ["decision", "draft", "old decision", "--json"], cwd: fixture.repo, env: leadEnvironment, replacement: "decide" },
      { args: ["work", "start", data.work.id, "--json"], cwd: fixture.repo, env: leadEnvironment, replacement: "work take" },
    ]) {
      const result = await runCliAt(fixture, attempt.cwd, attempt.args, attempt.env);
      expect(result.exitCode).toBe(1);
      const error = JSON.parse(result.stderr) as { error: { code: string; message: string } };
      expect(error.error.code).toBe("SLP_V2_CUTOVER");
      expect(error.error.message).toContain(attempt.replacement);
    }
    const roomAfter = new Database(roomDatabasePath, { readonly: true });
    expect(
      roomAfter
        .query<{ payload: string }, []>("SELECT payload FROM legacy_team_history WHERE id = 'legacy-g1'")
        .get()?.payload,
    ).toBe("opaque old lifecycle bytes");
    roomAfter.close();
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(project.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM work").get()?.count)
      .toBe(0);
    expect(project.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    project.close();

    const emergencyStopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--emergency", "--json"],
      fake.env,
    );
    expect(emergencyStopped.exitCode).toBe(0);
    for (const administrative of [
      ["ready", "--json"],
      ["decision", "list", "--json"],
    ]) {
      const result = await runCliAt(
        fixture,
        fixture.repo,
        administrative,
        leadEnvironment,
      );
      expect(result.exitCode, administrative.join(" ")).toBe(0);
    }
    for (const args of [
      ["work", "add", "must not reach legacy work", "--json"],
      ["work", "note", data.work.id, "must not reach legacy notes", "--json"],
    ]) {
      const retiredSharedVerb = await runCliAt(
        fixture,
        fixture.repo,
        args,
        leadEnvironment,
      );
      expect(retiredSharedVerb.exitCode).toBe(1);
      expect(JSON.parse(retiredSharedVerb.stderr).error.code).toBe("NO_ACTIVE_TEAM");
    }
    const afterSharedVerbs = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(afterSharedVerbs.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM work").get()?.count)
      .toBe(0);
    expect(
      afterSharedVerbs
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE kind = 'NOTE'",
        )
        .get()?.count,
    ).toBe(0);
    afterSharedVerbs.close();
  });
}, 20_000);

test("SLP v2 Hub decisions link unique work and require team qualification when ambiguous", async () => {
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
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>((
      await runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "First Hub-linked work", "--json"],
        fake.env,
      )
    ).stdout);
    const ownerDecision = envelope<{ decision: { id: string } }>((await runCliAt(
      fixture,
      room,
      ["decide", "Owner baseline", "--why", "Owner authority", "--json"],
      fake.env,
    )).stdout).decision;
    const linked = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "Hub choice",
        "--why",
        "Owner authority",
        "--work",
        first.work.id,
        "--replaces",
        ownerDecision.id,
        "--json",
      ],
      fake.env,
    );
    expect(phaseFree(linked.stderr)).toBe("");
    expect(linked.exitCode).toBe(0);
    const linkedDecision = envelope<{
      decision: { id: string; replaces: string; workId: string };
    }>(linked.stdout).decision;
    expect(linkedDecision.workId).toBe(first.work.id);
    expect(linkedDecision.replaces).toBe(ownerDecision.id);
    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      roomDatabase
        .query<{ team_id: string; work_id: string }, [string]>(
          "SELECT team_id, work_id FROM slp_decisions WHERE id = ?",
        )
        .get(linkedDecision.id),
    ).toEqual({ team_id: first.team.teamId, work_id: first.work.id });
    roomDatabase.close();
    const firstProject = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(
      firstProject
        .query<{ actor: string; work_id: string }, [string]>(
          "SELECT actor, work_id FROM slp_decisions WHERE id = ?",
        )
        .get(linkedDecision.id),
    ).toBeNull();
    firstProject.close();
    const firstSupervisor = first.team.roles.find((role) => role.role === "team-supervisor")!;
    const linkedStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", first.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: firstSupervisor.paneId },
    );
    expect(
      envelope<{ decisions: Array<Record<string, unknown>> }>(linkedStatus.stdout).decisions,
    ).toContainEqual({
      actor: "hub-supervisor",
      choice: "Hub choice",
      createdAt: expect.any(String),
      id: linkedDecision.id,
      replaces: ownerDecision.id,
      scope: "owner",
      why: "Owner authority",
    });

    const secondRepo = join(fixture.root, "repo-two");
    await mkdir(join(secondRepo, ".maestro", "plugins"), { recursive: true });
    const second = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>((
      await runCliAt(
        fixture,
        room,
        ["team", "start", secondRepo, "Second Hub-linked work", "--json"],
        fake.env,
      )
    ).stdout);
    expect(second.work.id).toBe(first.work.id);
    const ambiguous = await runCliAt(
      fixture,
      room,
      ["decide", "Ambiguous choice", "--why", "Must identify the team", "--work", "w1", "--json"],
      fake.env,
    );
    expect(ambiguous.exitCode).toBe(1);
    expect((JSON.parse(ambiguous.stderr) as { error: { code: string } }).error.code)
      .toBe("AMBIGUOUS_WORK");
    const qualified = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "Second team choice",
        "--why",
        "Qualified owner authority",
        "--work",
        `${second.team.teamId}:${second.work.id}`,
        "--json",
      ],
      fake.env,
    );
    expect(phaseFree(qualified.stderr)).toBe("");
    expect(qualified.exitCode).toBe(0);
    const secondProject = new Database(join(secondRepo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      secondProject
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_decisions WHERE actor = 'hub-supervisor' AND work_id = 'w1'",
        )
        .get()?.count,
    ).toBe(0);
    secondProject.close();
    const secondDecision = envelope<{ decision: { id: string } }>(qualified.stdout).decision;
    const secondSupervisor = second.team.roles.find((role) => role.role === "team-supervisor")!;
    const secondStatus = await runCliAt(
      fixture,
      secondRepo,
      ["status", second.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: secondSupervisor.paneId },
    );
    expect(envelope<{ decisions: Array<{ id: string }> }>(secondStatus.stdout).decisions)
      .toContainEqual(expect.objectContaining({ id: secondDecision.id }));
    expect((await runCliAt(
      fixture,
      room,
      ["team", "stop", first.team.teamId, "--emergency", "--json"],
      fake.env,
    )).exitCode).toBe(0);
    const stoppedTarget = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "Must not link stopped work",
        "--why",
        "Only running generations are eligible",
        "--work",
        `${first.team.teamId}:${first.work.id}`,
        "--json",
      ],
      fake.env,
    );
    expect(stoppedTarget.exitCode).toBe(1);
    expect(JSON.parse(stoppedTarget.stderr).error.code).toBe("NOT_FOUND");
  });
}, 25_000);

test("SLP v2 Hub scaffold retires every old role file in favor of one canonical pack", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(await Bun.file(join(room, "SLP.md")).exists()).toBe(true);
    const ownerPack = "# Owner-edited SLP pack\nkeep these exact bytes\n";
    await writeFile(join(room, "SLP.md"), ownerPack);
    await scaffoldRoom(fixture.home);
    expect(await readFile(join(room, "SLP.md"), "utf8")).toBe(ownerPack);
    for (const retired of ["lane.md", "lead.md", "observer.md", "supervisor.md", "observer-watch.sh"]) {
      expect(existsSync(join(room, retired)), retired).toBe(false);
    }
    for (const entry of ["AGENTS.md", "CLAUDE.md", "IDENTITY.md"]) {
      expect(await readFile(join(room, entry), "utf8")).toContain("SLP.md");
    }
  });
});

test("SLP v2 install removes only the old managed block and leaves clean file endings", async () => {
  await withFixture(async (fixture) => {
    const managed = "<!-- maestro:begin -->\nold managed SLP\n<!-- maestro:end -->";
    const agentsPrefix = "alpha\r\nuser spacing\r\n";
    const agentsSuffix = "\r\nomega\r\n";
    const claudePrefix = "custom-one\n\n";
    await writeFile(join(fixture.repo, "AGENTS.md"), `${agentsPrefix}${managed}${agentsSuffix}`);
    await writeFile(join(fixture.repo, "CLAUDE.md"), `${claudePrefix}${managed}\n`);
    const cleanPath = [join(import.meta.dir, "..", "node_modules", ".bin"), process.execPath, "/usr/bin", "/bin"]
      .map((entry) => entry.endsWith("/bun") ? join(entry, "..") : entry)
      .join(":");

    const installed = await runCli(fixture, ["install"], {
      PATH: cleanPath,
      SHELL: "/bin/zsh",
    });
    expect(phaseFree(installed.stderr)).toBe("");
    expect(installed.exitCode).toBe(0);
    expect(await readFile(join(fixture.repo, "AGENTS.md"), "utf8"))
      .toBe(`${agentsPrefix}${agentsSuffix}`);
    expect(await readFile(join(fixture.repo, "CLAUDE.md"), "utf8"))
      .toBe("custom-one\n");
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-slp-watch")).exists())
      .toBe(true);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-team-sensor")).exists())
      .toBe(false);
  });
}, 20_000);

test("SLP v2 Watch is one foreground non-agent reader whose death never blocks work", async () => {
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
      ["team", "start", fixture.repo, "Observe without authority", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
        workspaceId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const runtimeDirectory = watchRuntimeDirectory(
      fixture.repo,
      data.team.teamId,
      data.team.generation,
    );
    const transcript = join(runtimeDirectory, "transcript.txt");
    const watchCommand = [
      process.execPath,
      join(import.meta.dir, "..", "bin", "maestro-slp-watch.ts"),
      "--team",
      data.team.teamId,
      "--generation",
      String(data.team.generation),
      "--interval-ms",
      "50",
    ];
    const watchEnvironment = {
      ...process.env,
      ...fake.env,
      HERDR_WORKSPACE_ID: data.team.workspaceId,
    };
    const before = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const activityBefore = before
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
      .get()?.count;
    before.close();
    const firstWatch = Bun.spawn(watchCommand, {
      cwd: fixture.repo,
      env: watchEnvironment,
      stderr: "pipe",
      stdout: "ignore",
    });
    await waitForText(transcript, data.team.roles[0]!.name);

    const duplicate = Bun.spawn(watchCommand, {
      cwd: fixture.repo,
      env: watchEnvironment,
      stderr: "pipe",
      stdout: "ignore",
    });
    const duplicateError = await new Response(duplicate.stderr).text();
    expect(await duplicate.exited).toBe(1);
    expect(duplicateError).toContain("already running");

    const watchTabId = `${data.team.workspaceId}:watch-live`;
    const watchPaneId = `${watchTabId}:pane`;
    await editFakeHerdrState(fake, (state) => {
      (state.tabs as Array<Record<string, string>>).push({
        label: `slp:${data.team.teamId}:g${data.team.generation}:watch`,
        root_pane_id: watchPaneId,
        tab_id: watchTabId,
        workspace_id: data.team.workspaceId,
      });
      (state.panes as Array<Record<string, string>>).push({
        pane_id: watchPaneId,
        tab_id: watchTabId,
        workspace_id: data.team.workspaceId,
      });
      (state.processes as Record<string, unknown>)[watchPaneId] = {
        foreground_pgid: firstWatch.pid,
        foreground_processes: [{
          args: watchCommand.slice(1),
          command: "maestro-slp-watch",
          pid: firstWatch.pid,
        }],
      };
    });
    const on = envelope<{ teams: Array<{ watch: string }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout,
    );
    expect(on.teams[0]?.watch).toBe("on");

    firstWatch.kill();
    expect(await firstWatch.exited).toBe(0);
    await editFakeHerdrState(fake, (state) => {
      state.tabs = (state.tabs as Array<{ tab_id: string }>).filter(
        (tab) => tab.tab_id !== watchTabId,
      );
      state.panes = (state.panes as Array<{ pane_id: string }>).filter(
        (pane) => pane.pane_id !== watchPaneId,
      );
      delete (state.processes as Record<string, unknown>)[watchPaneId];
    });
    const off = envelope<{ teams: Array<{ watch: string }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout,
    );
    expect(off.teams[0]?.watch).toBe("off");
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const taken = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", data.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: lead.paneId },
    );
    expect(phaseFree(taken.stderr)).toBe("");
    expect(taken.exitCode).toBe(0);

    const reopened = Bun.spawn(watchCommand, {
      cwd: fixture.repo,
      env: watchEnvironment,
      stderr: "pipe",
      stdout: "ignore",
    });
    await waitForText(join(runtimeDirectory, "watch.lock"), String(reopened.pid));
    await waitForText(transcript, lead.name);
    reopened.kill();
    expect(await reopened.exited).toBe(0);

    const after = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      after.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity").get()?.count,
    ).toBe((activityBefore ?? 0) + 1);
    after.close();
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.agents).toHaveLength(3);
    const commands = await fakeHerdrCommands(fake);
    expect(commands.some((command) => command[0] === "agent" && command[1] === "prompt" && command[2]?.includes("watch")))
      .toBe(false);
  });
}, 20_000);

function failureEnvelope(stderr: string): { error: Record<string, unknown> } {
  const line = phaseFree(stderr).split("\n").findLast((candidate) => candidate.startsWith("{"));
  return JSON.parse(line ?? "{}") as { error: Record<string, unknown> };
}

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
    expect(reads.length).toBeGreaterThanOrEqual(5);
    expect(reads.every((command) => command.includes("visible"))).toBe(true);
    expect(started.stderr).toContain("waiting for acknowledgement (up to 30s)");
    expect(started.stderr).toMatch(/^supervisor-\S+: starting claude pane in slp-/m);
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
    expect(error.command).toEqual(["agent", "read", expect.stringMatching(/^supervisor-/)]);
    expect((error.paneTail as string[]).join("\n")).toContain("SLP_ROLE_READY");
    const reads = (await fakeHerdrCommands(fake)).filter(
      (command) => command[0] === "agent" && command[1] === "read",
    );
    expect(reads.length).toBeGreaterThanOrEqual(4);
    expect(reads.at(-1)).toContain("recent-unwrapped");
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

test("SLP v2 pushes one notice line to the counterpart after return, rework, and accept", async () => {
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
      ["team", "start", fixture.repo, "Push notices", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
    const notices = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("[from "),
      );
    const id = data.work.id;

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", id, "--json"], leadEnvironment)).exitCode,
    ).toBe(0);
    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", id, "result: first pass\nmore detail below", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(returned.stderr)).toBe("");
    expect(returned.exitCode).toBe(0);
    expect(await notices()).toEqual([
      ["agent", "prompt", supervisor.name, `[from lead][${id} RETURNED] result: first pass; read: maestro status ${id}`],
    ]);

    const rework = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "needs a test", "--rework", "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(rework.stderr)).toBe("");
    expect(rework.exitCode).toBe(0);
    expect((await notices()).at(-1)).toEqual([
      "agent",
      "prompt",
      lead.name,
      `[from team-supervisor][${id} RETURNED] rework granted: needs a test; read: maestro status ${id}`,
    ]);

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", id, "--json"], leadEnvironment)).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", id, "result: with test", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const accepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", id, "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(accepted.stderr)).toBe("");
    expect(accepted.exitCode).toBe(0);
    expect((await notices()).slice(-2)).toEqual([
      ["agent", "prompt", lead.name, `[from team-supervisor][${id} DONE] accepted; read: maestro status ${id}`],
      [
        "agent",
        "prompt",
        "supervisor",
        `[from team-supervisor][${id} DONE] accepted in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
      ],
    ]);

    const peerWork = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer task", "--to", "peer-notice", "--json"],
      leadEnvironment,
    );
    expect(peerWork.exitCode).toBe(0);
    const peerData = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(
      peerWork.stdout,
    );
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peerData.role.paneId };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", peerData.work.id, "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    await setFakeHerdrBehavior(fake, { prompts: false });
    const refused = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", peerData.work.id, "result: peer done", "--json"],
      peerEnvironment,
    );
    expect(refused.exitCode).toBe(0);
    expect(refused.stderr).toContain(
      `warning: could not notify ${lead.name} about ${peerData.work.id} RETURNED`,
    );
    expect(envelope<{ work: { state: string } }>(refused.stdout).work.state).toBe("RETURNED");
  });
}, 20_000);


test("SLP v2 work note --blocked flags the note and pushes one line to the seat above (d761)", async () => {
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
      ["team", "start", fixture.repo, "Blocked notes", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
    const notices = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("[from "),
      );
    const id = data.work.id;
    const noteFlag = (stdout: string) => envelope<{ note: { flag: string | null } }>(stdout).note.flag;

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", id, "--json"], leadEnvironment)).exitCode,
    ).toBe(0);
    const both = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "x", "--blocked", "--rework", "--json"],
      leadEnvironment,
    );
    expect(both.exitCode).toBe(1);
    expect(failureEnvelope(both.stderr).error.code).toBe("INVALID_OPTION");

    const blocked = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "need the API key name\nsecond line", "--blocked", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(blocked.stderr)).toBe("");
    expect(blocked.exitCode).toBe(0);
    expect(noteFlag(blocked.stdout)).toBe("blocked");
    expect(await notices()).toEqual([
      ["agent", "prompt", supervisor.name, `[from lead][${id} BLOCKED] need the API key name; read: maestro status ${id}`],
    ]);

    const shown = await runCliAt(fixture, fixture.repo, ["status", id], supervisorEnvironment);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`note [blocked] by ${lead.name}: need the API key name`);
    const shownJson = envelope<{ notes: Array<{ flag: string | null }> }>(
      (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], supervisorEnvironment)).stdout,
    );
    expect(shownJson.notes.map((note) => note.flag)).toEqual(["blocked"]);

    const escalated = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "owner must pick the vendor", "--blocked", "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(escalated.stderr)).toBe("");
    expect(escalated.exitCode).toBe(0);
    expect((await notices()).at(-1)).toEqual([
      "agent",
      "prompt",
      "supervisor",
      `[from team-supervisor][${id} BLOCKED] owner must pick the vendor in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
    ]);

    const plain = await runCliAt(fixture, fixture.repo, ["work", "note", id, "still going", "--json"], leadEnvironment);
    expect(plain.exitCode).toBe(0);
    expect(noteFlag(plain.stdout)).toBeNull();
    expect((await notices()).length).toBe(2);
  });
}, 30_000);


test("SLP v2 team start launches a Codex observer after the Lead, status admits it, and stop closes it (d762)", async () => {
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
      ["team", "start", fixture.repo, "Observer seat", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    expect(data.team.roles.map((role) => role.role)).toEqual(["team-supervisor", "lead", "observer"]);
    const observer = data.team.roles.find((role) => role.role === "observer")!;
    expect(observer.name).toBe(`observer-${data.team.teamId}`);
    const starts = (await fakeHerdrCommands(fake)).filter(
      (command) => command[0] === "agent" && command[1] === "start",
    );
    expect(starts.map((command) => command[2])).toEqual([
      `supervisor-${data.team.teamId}`,
      `lead-${data.team.teamId}`,
      observer.name,
    ]);
    expect(starts.at(-1)).toEqual([
      "agent",
      "start",
      observer.name,
      "--kind",
      "codex",
      "--pane",
      observer.paneId,
      "--timeout",
      "60000",
      "--",
      "--model",
      "gpt-5.6-luna",
    ]);

    const observerEnvironment = { ...fake.env, HERDR_PANE_ID: observer.paneId };
    const status = envelope<{ role: { role: string }; work: Array<{ id: string }> }>(
      (await runCliAt(fixture, fixture.repo, ["status", "--json"], observerEnvironment)).stdout,
    );
    expect(status.role.role).toBe("observer");
    expect(status.work.map((work) => work.id)).toEqual([data.work.id]);
    const shown = await runCliAt(fixture, fixture.repo, ["status", data.work.id], observerEnvironment);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`next: waiting on lead-${data.team.teamId}`);
    for (const args of [["work", "take", data.work.id], ["work", "note", data.work.id, "x"]]) {
      const refused = await runCliAt(fixture, fixture.repo, [...args, "--json"], observerEnvironment);
      expect(refused.exitCode).toBe(1);
      expect(failureEnvelope(refused.stderr).error.code).toBe("ROLE_FORBIDDEN");
    }

    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--emergency", "--reason", "observer test done", "--json"],
      fake.env,
    );
    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.agents).toEqual([]);
    expect(runtime.workspaces).toEqual([]);
  });
}, 30_000);

test("SLP v2 team start widens the role CHECK on stores created before the Observer seat (d762)", async () => {
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
    const legacyColumns = `
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('team-supervisor', 'lead', 'peer')),
      name TEXT NOT NULL,
      pane_id TEXT NOT NULL,
      workspace_id TEXT NOT NULL,
      created_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, name)`;
    const roomPath = join(room, ".maestro", "maestro.db");
    const roomDatabase = new Database(roomPath);
    roomDatabase.exec(
      `DROP TABLE IF EXISTS slp_team_roles;
       CREATE TABLE slp_team_roles (${legacyColumns},
         FOREIGN KEY(team_id, generation) REFERENCES slp_teams(team_id, generation));`,
    );
    roomDatabase.close();
    expect((await runCliAt(fixture, fixture.repo, ["status", "--json"])).exitCode).toBe(0);
    const projectPath = join(fixture.repo, ".maestro", "maestro.db");
    const projectDatabase = new Database(projectPath);
    projectDatabase.exec(`CREATE TABLE slp_local_roles (${legacyColumns});`);
    projectDatabase.close();

    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Legacy CHECK", "--json"],
      fake.env,
    );
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const data = envelope<{ team: { roles: Array<{ role: string }> } }>(started.stdout);
    expect(data.team.roles.map((role) => role.role)).toEqual(["team-supervisor", "lead", "observer"]);
    for (const [path, table] of [[roomPath, "slp_team_roles"], [projectPath, "slp_local_roles"]] as const) {
      const database = new Database(path, { readonly: true });
      expect(
        database
          .query<{ sql: string }, [string]>(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
          )
          .get(table)?.sql,
      ).toContain("'observer'");
      expect(
        database
          .query<{ count: number }, []>(`SELECT COUNT(*) AS count FROM ${table} WHERE role = 'observer'`)
          .get()?.count,
      ).toBe(1);
      expect(
        database
          .query<{ present: number }, [string]>("SELECT 1 AS present FROM sqlite_master WHERE name = ?")
          .get(`${table}_legacy`),
      ).toBeNull();
      database.close();
    }
  });
}, 30_000);

test("SLP v2 normal stop carries the Supervisor reason to the Hub ledger, status, and named Hub agent", async () => {
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
      ["team", "start", fixture.repo, "Stop with a report", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
    for (const args of [
      ["work", "take", data.work.id, "--json"],
      ["work", "return", data.work.id, "result: done", "--json"],
    ]) {
      expect((await runCliAt(fixture, fixture.repo, args, leadEnvironment)).exitCode).toBe(0);
    }
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "accept", data.work.id, "--json"],
          supervisorEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const hubRefused = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--reason", "not the Hub's call", "--json"],
      fake.env,
    );
    expect(hubRefused.exitCode).toBe(1);
    expect(hubRefused.stderr).toContain("INVALID_OPTION");

    const reason = "all green: w1 accepted, nothing left";
    const stopped = await runCliAt(
      fixture,
      fixture.repo,
      ["team", "stop", data.team.teamId, "--reason", reason, "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(stopped.stderr)).toBe("");
    expect(stopped.exitCode).toBe(0);
    expect(envelope<{ team: { state: string } }>(stopped.stdout).team.state).toBe("STOPPED");

    const hub = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      hub
        .query<{ actor: string; emergency: number; reason: string }, [string]>(
          `SELECT actor, emergency, reason FROM slp_lifecycle_operations
           WHERE operation = 'STOP' AND phase = 'COMMITTED' AND team_id = ?`,
        )
        .get(data.team.teamId),
    ).toEqual({ actor: supervisor.name, emergency: 0, reason });
    hub.close();

    const status = await runCliAt(fixture, room, ["status"], fake.env);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain(
      `${data.team.teamId} g${data.team.generation} STOPPED (supervisor): ${reason}`,
    );
    const statusJson = envelope<{ teams: Array<{ stop: unknown }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout,
    );
    expect(statusJson.teams[0]?.stop).toEqual({ actor: supervisor.name, emergency: false, reason });

    const hubNotice = (await fakeHerdrCommands(fake)).find(
      (command) =>
        command[0] === "agent" && command[1] === "prompt" && command[2] === "supervisor" &&
        (command[3] ?? "").includes("STOPPED"),
    );
    expect(hubNotice).toEqual([
      "agent",
      "prompt",
      "supervisor",
      `[from team-supervisor][${data.team.teamId} g${data.team.generation} STOPPED] ${reason}; read: maestro status`,
    ]);
  });
}, 30_000);

test("SLP v2 in-team status text is structured while JSON keeps its shape (d758)", async () => {
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
    const objective = "Structured status objective";
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, objective, "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
    const id = data.work.id;
    const header = (role: "lead" | "team-supervisor", who: { name: string; paneId: string }) =>
      `${data.team.teamId} g${data.team.generation} ${role} ${who.name} in ${who.paneId}`;
    const lines = async (args: string[], environment: Record<string, string>) => {
      const result = await runCliAt(fixture, fixture.repo, args, environment);
      expect(result.exitCode).toBe(0);
      return result.stdout.trim().split("\n");
    };
    const run = async (args: string[], environment: Record<string, string>) => {
      expect((await runCliAt(fixture, fixture.repo, args, environment)).exitCode).toBe(0);
    };

    expect(await lines(["status"], leadEnvironment)).toEqual([
      header("lead", lead),
      `* ${id} OPEN hub-supervisor -> ${lead.name}: ${objective}`,
      "decisions: none",
    ]);
    expect((await lines(["status"], supervisorEnvironment))[1]).toBe(
      `  ${id} OPEN hub-supervisor -> ${lead.name}: ${objective}`,
    );
    expect(await lines(["status", id], leadEnvironment)).toEqual([
      `${id} OPEN hub-supervisor -> ${lead.name}`,
      "revision: 0",
      `objective: ${objective}`,
      "entries: none",
      "decisions: none",
      `next: work take ${id}`,
    ]);

    await run(["work", "take", id, "--json"], leadEnvironment);
    expect((await lines(["status", id], leadEnvironment)).at(-1)).toBe(
      `next: work return ${id} "<result>"`,
    );
    expect((await lines(["status", id], supervisorEnvironment)).at(-1)).toBe(
      `next: waiting on ${lead.name}`,
    );
    await run(["work", "return", id, "result: first pass\nsecond line", "--json"], leadEnvironment);
    await run(
      ["decide", "Ship as is", "--why", "the first pass reads clean", "--work", id, "--json"],
      supervisorEnvironment,
    );
    expect(await lines(["status"], supervisorEnvironment)).toEqual([
      header("team-supervisor", supervisor),
      `* ${id} RETURNED hub-supervisor -> ${lead.name}: ${objective}`,
      `decisions: d1 (${id})`,
    ]);
    expect(await lines(["status", id], supervisorEnvironment)).toEqual([
      `${id} RETURNED hub-supervisor -> ${lead.name}`,
      "revision: 1",
      `objective: ${objective}`,
      `return by ${lead.name}: result: first pass`,
      "decisions: d1",
      `next: work accept ${id} | work note ${id} "<gap>" --rework | work accept ${id} --outcome cancelled`,
    ]);
    expect((await lines(["status", id], leadEnvironment)).at(-1)).toBe(
      `next: waiting on ${supervisor.name}`,
    );

    await run(["work", "note", id, "needs a test", "--rework", "--json"], supervisorEnvironment);
    const leadRework = await lines(["status", id], leadEnvironment);
    expect(leadRework[3]).toBe(`note by ${supervisor.name}: needs a test`);
    expect(leadRework.at(-1)).toBe(`next: work take ${id}`);
    expect((await lines(["status"], supervisorEnvironment))[1]).toBe(
      `  ${id} RETURNED hub-supervisor -> ${lead.name}: ${objective}`,
    );

    await run(["work", "take", id, "--json"], leadEnvironment);
    await run(["work", "return", id, "result: with test", "--json"], leadEnvironment);
    await run(["work", "accept", id, "--json"], supervisorEnvironment);
    expect(await lines(["status"], leadEnvironment)).toEqual([
      header("lead", lead),
      "1 DONE; --all to list",
      `decisions: d1 (${id})`,
    ]);
    expect(await lines(["status", "--all"], leadEnvironment)).toEqual([
      header("lead", lead),
      `  ${id} DONE hub-supervisor -> ${lead.name}: ${objective}`,
      `decisions: d1 (${id})`,
    ]);
    const leadDone = await lines(["status", id], leadEnvironment);
    expect(leadDone[3]).toBe(`acceptance by ${supervisor.name}: accepted`);
    expect(leadDone.at(-1)).toBe("next: none (accepted)");

    const teamJson = envelope<Record<string, unknown>>(
      (await runCliAt(fixture, fixture.repo, ["status", "--json"], leadEnvironment)).stdout,
    );
    expect(Object.keys(teamJson).sort()).toEqual([
      "generation",
      "missingPanes",
      "role",
      "roles",
      "runtime",
      "teamId",
      "watch",
      "work",
    ]);
    expect(teamJson.work).toEqual([]);
    const workJson = envelope<Record<string, unknown>>(
      (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], leadEnvironment)).stdout,
    );
    expect(Object.keys(workJson).sort()).toEqual([
      "acceptance",
      "decisions",
      "notes",
      "returns",
      "work",
    ]);
  });
}, 30_000);
