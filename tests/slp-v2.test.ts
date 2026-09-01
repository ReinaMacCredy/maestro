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

    expect(started.stderr).toBe("");
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
        startData.team.roles.find((role) => role.role === "team-supervisor")?.name,
      ].sort(),
    );
    const commands = await fakeHerdrCommands(fake);
    const prompts = commands.filter(
      (command) => command[0] === "agent" && command[1] === "prompt",
    );
    expect(prompts).toHaveLength(2);
    expect(prompts.every((command) => command.includes("--wait") && command.includes("--timeout")))
      .toBe(true);
    expect(
      prompts.every((command) => command[3]?.includes("SLP_ROLE_READY") === true),
    ).toBe(true);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "read"),
    ).toHaveLength(2);
  });
});

test("SLP v2 rejects an incorrect role acknowledgement before granting authority", async () => {
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
    const fake = await installFakeHerdr(fixture, { invalidAcknowledgements: true });

    const failed = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Reject the wrong role acknowledgement", "--json"],
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
});

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

    expect(started.stderr).toBe("");
    expect(started.exitCode).toBe(0);
    expect(
      (await fakeHerdrCommands(fake)).filter(
        (command) => command[0] === "agent" && command[1] === "start",
      ),
    ).toHaveLength(4);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(2);
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

    expect(started.stderr).toBe("");
    expect(started.exitCode).toBe(0);
    const commands = await fakeHerdrCommands(fake);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "start"),
    ).toHaveLength(2);
    expect(
      commands.filter((command) => command[0] === "agent" && command[1] === "prompt"),
    ).toHaveLength(4);
    expect((await readFakeHerdrState(fake)).agents).toHaveLength(2);
  });
});

test("SLP v2 gives Herdr agent startup its own bounded readiness window", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { agentStartDelayMs: 1_500 });
    const plan = buildSlpTeamPlan({
      generation: 1,
      leadModel: "default",
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
});

test("SLP v2 waits for workspace-close visibility before declaring runtime absence", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture);
    const plan = buildSlpTeamPlan({
      generation: 1,
      leadModel: "default",
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

    expect(nestedStart.stderr).toBe("");
    expect(rootStart.stderr).toBe("");
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

test("SLP v2 isolates generations and role lookup between linked worktree checkouts", async () => {
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
    const first = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
    }>((await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Main checkout team", "--json"],
      fake.env,
    )).stdout);
    const second = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
    }>((await runCliAt(
      fixture,
      room,
      ["team", "start", linked, "Linked checkout team", "--json"],
      fake.env,
    )).stdout);
    expect(first.team.teamId).not.toBe(second.team.teamId);
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
    expect(repeated.stderr).toBe("");
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
    expect(repeatedData.team.roles.map((role) => role.readyChallenge)).not.toEqual(
      firstData.team.roles.map((role) => role.readyChallenge),
    );
    let runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(2);

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

    expect(repaired.stderr).toBe("");
    expect(repaired.exitCode).toBe(0);
    expect(
      envelope<{ team: { generation: number }; work: { id: string } }>(repaired.stdout),
    ).toMatchObject({
      team: { generation: firstData.team.generation },
      work: { id: firstData.work.id },
    });
    runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(2);
    expect(runtime.agents.filter((agent: { name: string }) => agent.name.startsWith("lead-"))).toHaveLength(1);
  });
});

test("SLP v2 serializes concurrent identical starts into one generation", async () => {
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

    const [first, second] = await Promise.all([
      runCliAt(fixture, room, args, fake.env),
      runCliAt(fixture, room, args, fake.env),
    ]);

    expect(first.stderr).toBe("");
    expect(second.stderr).toBe("");
    const firstData = envelope<{ team: { generation: number; teamId: string }; work: { id: string } }>(
      first.stdout,
    );
    const secondData = envelope<{ team: { generation: number; teamId: string }; work: { id: string } }>(
      second.stdout,
    );
    expect(secondData).toMatchObject({
      team: { generation: firstData.team.generation, teamId: firstData.team.teamId },
      work: { id: firstData.work.id },
    });
    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(2);
    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(roomDatabase.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_teams").get()?.count)
      .toBe(1);
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
    expect(started.stderr).toBe("");
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
      BEFORE INSERT ON slp_teams
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
    projectAfterFailure.close();

    const repairDatabase = new Database(roomDatabasePath);
    repairDatabase.exec("DROP TRIGGER reject_start_finalization");
    repairDatabase.close();
    await setFakeHerdrBehavior(fake, { processInfoDelayMs: 0 });
    const commandCount = (await fakeHerdrCommands(fake)).length;
    const retried = await runCliAt(fixture, room, args, fake.env);

    expect(retried.stderr).toBe("");
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

    expect(first.stderr).toBe("");
    expect(repeated.stderr).toBe("");
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
    expect(runtime.agents).toHaveLength(2);
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

    expect(first.stderr).toBe("");
    expect(second.stderr).toBe("");
    const firstTeam = envelope<{
      team: { roles: Array<{ name: string }>; teamId: string };
    }>(first.stdout).team;
    const secondTeam = envelope<{
      team: { roles: Array<{ name: string }>; teamId: string };
    }>(second.stdout).team;
    expect(firstTeam.teamId).not.toBe(secondTeam.teamId);
    expect(new Set([...firstTeam.roles, ...secondTeam.roles].map((role) => role.name)).size).toBe(4);
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

    expect(retried.stderr).toBe("");
    expect(retried.exitCode).toBe(0);
    expect(envelope<{ team: { generation: number } }>(retried.stdout).team.generation).toBe(1);
    runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(2);
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
    expect(taken.stderr).toBe("");
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
    expect(returned.stderr).toBe("");
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
    expect(accepted.stderr).toBe("");
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
    expect(lead).toBeDefined();
    const environment = { ...fake.env, HERDR_PANE_ID: lead?.paneId };

    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Design independently", "--to", "peer-design", "--json"],
      environment,
    );
    expect(first.stderr).toBe("");
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; id: string; state: string };
    }>(first.stdout);
    expect(firstData.role).toMatchObject({ name: expect.stringMatching(/^peer-design-/), role: "peer" });
    expect(firstData.work).toMatchObject({ assignedTo: firstData.role.name, state: "OPEN" });

    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Implement independently", "--to", "peer-design", "--json"],
      environment,
    );
    expect(second.stderr).toBe("");
    expect(second.exitCode).toBe(0);
    const secondData = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; id: string; state: string };
    }>(second.stdout);
    expect(secondData.role).toEqual(firstData.role);
    expect(secondData.work).toMatchObject({ assignedTo: firstData.role.name, state: "OPEN" });
    expect(secondData.work.id).not.toBe(firstData.work.id);

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

    const leadAccepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", addedData.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(leadAccepted.stderr).toBe("");
    expect(leadAccepted.exitCode).toBe(0);
    expect(envelope<{ work: { acceptedBy: string; state: string } }>(leadAccepted.stdout).work)
      .toMatchObject({ acceptedBy: lead?.name, state: "DONE" });
  });
}, 15_000);

test("SLP v2 rework is one reviewer note followed by the assignee taking RETURNED work", async () => {
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

    const noted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", startData.work.id, "rework: add the missing negative case", "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisorPane },
    );
    expect(noted.stderr).toBe("");
    expect(noted.exitCode).toBe(0);
    expect(envelope<{ work: { state: string } }>(noted.stdout).work.state).toBe("RETURNED");

    const retaken = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(retaken.stderr).toBe("");
    expect(envelope<{ work: { currentReturn: string | null; state: string } }>(retaken.stdout).work)
      .toMatchObject({ currentReturn: null, state: "ACTIVE" });
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
          ["work", "note", startData.work.id, "resolved: contract supplied", "--json"],
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
    expect(openCancelled.stderr).toBe("");
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
    expect(returnedCancelled.stderr).toBe("");
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
    expect(original.stderr).toBe("");
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
    expect(replacement.stderr).toBe("");
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

test("SLP v2 serializes concurrent decision ids without dropping a decision", async () => {
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
      Array.from({ length: 8 }, (_, index) =>
        runCliAt(fixture, room, [
          "decide",
          `Concurrent choice ${index}`,
          "--why",
          `Concurrent reason ${index}`,
          "--json",
        ])
      ),
    );

    expect(results.map((result) => result.exitCode)).toEqual(Array(8).fill(0));
    const ids = results.map(
      (result) => envelope<{ decision: { id: string } }>(result.stdout).decision.id,
    );
    expect(new Set(ids).size).toBe(8);
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
    expect(hubStatus.stderr).toBe("");
    expect(envelope<{ teams: Array<{ packDigest: string; state: string }> }>(hubStatus.stdout).teams)
      .toEqual([expect.objectContaining({ packDigest: expect.any(String), state: "RUNNING" })]);

    const teamStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      supervisorEnvironment,
    );
    expect(teamStatus.stderr).toBe("");
    const teamData = envelope<{
      missingPanes: string[];
      runtime: string;
      roles: Array<{ role: string }>;
      watch: string;
      work: Array<{ id: string; state: string }>;
    }>(teamStatus.stdout);
    expect(teamData.roles.map((role) => role.role).sort()).toEqual(["lead", "team-supervisor"]);
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
    expect(workStatus.stderr).toBe("");
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
    expect(stopped.stderr).toBe("");
    expect(stopped.exitCode).toBe(0);

    const second = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Run generation two", "--json"],
      fake.env,
    );
    expect(second.stderr).toBe("");
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
    expect(stopped.stderr).toBe("");
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
    expect(retried.stderr).toBe("");
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
    expect(recovered.stderr).toBe("");
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
    expect(stopped.stderr).toBe("");
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
      BEFORE UPDATE OF state ON slp_teams
      WHEN NEW.state = 'STOPPED'
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
    projectAfterFailure.close();
    expect((await readFakeHerdrState(fake)).workspaces).toEqual([]);

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

    expect(retried.stderr).toBe("");
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

test("SLP v2 stop excludes a late work mutation before committing STOPPED", async () => {
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
    const lateNote = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", started.work.id, "late mutation", "--json"],
      supervisorEnvironment,
    );
    const stopped = await stopping;

    expect(stopped.stderr).toBe("");
    expect(stopped.exitCode).toBe(0);
    expect(lateNote.exitCode).toBe(1);
    expect((JSON.parse(lateNote.stderr) as { error: { code: string } }).error.code)
      .toBe("TEAM_STOP_IN_PROGRESS");
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
    project.close();
  });
}, 20_000);

test("SLP v2 Hub emergency stop preserves active work and appends one emergency activity", async () => {
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
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", data.work.id, "--json"],
          { ...fake.env, HERDR_PANE_ID: leadPane },
        )
      ).exitCode,
    ).toBe(0);

    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--emergency", "--json"],
      fake.env,
    );
    expect(stopped.stderr).toBe("");
    expect(stopped.exitCode).toBe(0);
    expect(envelope<{ emergency: boolean; team: { state: string } }>(stopped.stdout)).toMatchObject({
      emergency: true,
      team: { state: "STOPPED" },
    });
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(project.query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?").get(data.work.id)?.state)
      .toBe("ACTIVE");
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
      expect(result.stderr).toBe("");
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
    expect(linked.stderr).toBe("");
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
    expect(qualified.stderr).toBe("");
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

test("SLP v2 install removes only old managed block bytes and replaces sensor with Watch shim", async () => {
  await withFixture(async (fixture) => {
    const managed = "<!-- maestro:begin -->\nold managed SLP\n<!-- maestro:end -->";
    const agentsPrefix = "alpha\r\nuser spacing\r\n";
    const agentsSuffix = "\r\nomega\r\n";
    const claudePrefix = "custom-one\n\n\n";
    const claudeSuffix = "\n\ncustom-two";
    await writeFile(join(fixture.repo, "AGENTS.md"), `${agentsPrefix}${managed}${agentsSuffix}`);
    await writeFile(join(fixture.repo, "CLAUDE.md"), `${claudePrefix}${managed}${claudeSuffix}`);
    const cleanPath = [join(import.meta.dir, "..", "node_modules", ".bin"), process.execPath, "/usr/bin", "/bin"]
      .map((entry) => entry.endsWith("/bun") ? join(entry, "..") : entry)
      .join(":");

    const installed = await runCli(fixture, ["install"], {
      PATH: cleanPath,
      SHELL: "/bin/zsh",
    });
    expect(installed.stderr).toBe("");
    expect(installed.exitCode).toBe(0);
    expect(await readFile(join(fixture.repo, "AGENTS.md"), "utf8"))
      .toBe(`${agentsPrefix}${agentsSuffix}`);
    expect(await readFile(join(fixture.repo, "CLAUDE.md"), "utf8"))
      .toBe(`${claudePrefix}${claudeSuffix}`);
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
    expect(taken.stderr).toBe("");
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
    expect(runtime.agents).toHaveLength(2);
    const commands = await fakeHerdrCommands(fake);
    expect(commands.some((command) => command[0] === "agent" && command[1] === "prompt" && command[2]?.includes("watch")))
      .toBe(false);
  });
}, 20_000);
