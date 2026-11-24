import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { readFile, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { buildSlpTeamPlan } from "../src/plugins/slp/runtime-plan.ts";
import { HerdrSlpRuntime } from "../src/plugins/slp/runtime.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./helpers-herdr.ts";
import { runCli, runCliAt, withFixture } from "./helpers.ts";
import { envelope, phaseFree, waitForText } from "./helpers-slp.ts";

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
    // Contract prompts only: the w696 wake-up for the still-OPEN initial item
    // rides on every start and is asserted in its own test.
    const prompts = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("slp team "),
      );
    expect(await prompts()).toHaveLength(1);
    let runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(runtime.agents).toHaveLength(1);

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
    expect(runtime.agents).toHaveLength(1);
    expect(runtime.agents.filter((agent: { name: string }) => agent.name.startsWith("lead-"))).toHaveLength(1);
    const repairPrompts = await prompts();
    expect(repairPrompts).toHaveLength(2);
    expect(repairPrompts[1]?.[2]).toBe(lead.name);
    const repairedData = envelope<{
      team: { roles: Array<{ paneId: string; readyChallenge: string; role: string }>; teamId: string };
    }>(repaired.stdout);
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
        [...args.slice(0, -1), "--peer-model", "gpt-5.6-sol", "--json"],
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
    expect(runtime.agents).toHaveLength(1);
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
    expect(runtime.agents).toHaveLength(1);
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
    const siblingPaneId = `${lead.paneId}:sibling`;
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
        foreground_processes: [{ command: "sleep" }],
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
    expect(runtime.agents).toHaveLength(1);
  });
});

test("SLP v2 lets a retry after a failed start change the objective and peer model", async () => {
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
    const failed = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "First objective", "--json"],
      fake.env,
    );
    expect(failed.exitCode).toBe(1);

    await setFakeHerdrBehavior(fake, { agents: true });
    const retried = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Second objective", "--peer-model", "gpt-5.6-sol", "--json"],
      fake.env,
    );
    expect(phaseFree(retried.stderr)).toBe("peer model: gpt-5.6-sol");
    expect(retried.exitCode).toBe(0);
    const data = envelope<{ team: { generation: number; teamId: string } }>(retried.stdout);
    expect(data.team.generation).toBe(1);
    const leadStart = (await fakeHerdrCommands(fake)).find(
      (command) => command[0] === "agent" && command[1] === "start" && command[2] === `lead-${data.team.teamId}`,
    );
    expect(leadStart?.slice(-3)).toEqual(["--", "--profile", "maestro-lead"]);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(
      JSON.parse(
        database
          .query<{ configuration_json: string }, []>(
            "SELECT configuration_json FROM slp_local_teams WHERE generation = 1",
          )
          .get()?.configuration_json ?? "{}",
      ),
    ).toMatchObject({ models: { peer: "gpt-5.6-sol" } });
    expect(
      database
        .query<{ objective: string; phase: string }, []>(
          "SELECT objective, phase FROM slp_lifecycle_operations WHERE operation = 'START'",
        )
        .get(),
    ).toEqual({ objective: "Second objective", phase: "COMMITTED" });
    database.close();
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
      [...originalArgs.slice(0, -1), "--peer-model", "gpt-5.6-sol", "--json"],
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
          profile: string;
          readyChallenge: string;
          role: "lead";
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
      lead: { harness: "codex" },
      projectPath: started.team.projectPath,
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
        profile: role.profile,
        readyChallenge: role.readyChallenge,
        role: role.role,
        workspaceId: "unused-by-inspect",
      })),
    )).rejects.toThrow("timed out after 50ms");
    expect(Date.now() - beganAt).toBeLessThan(1_000);
  });
}, 15_000);

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
      .toBe(false);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-slp-observe")).exists())
      .toBe(false);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-team-sensor")).exists())
      .toBe(false);
  });
}, 20_000);

// w696: team start creates the Lead's first item itself, so it owes the same
// d753 wake-up as work add; without it a fresh Lead pane holds only its
// acknowledgement and sits idle while the store says the item is OPEN.
test("SLP v2 team start pushes one [from hub-supervisor][<id> OPEN] line to the Lead pane, on a fresh start and on a repeat that leaves the item OPEN", async () => {
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
    const args = ["team", "start", fixture.repo, "Wake the Lead on start", "--json"];

    const started = await runCliAt(fixture, room, args, fake.env);

    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string; state: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const opens = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && / OPEN\] /.test(command[3] ?? ""),
      );
    expect(await opens()).toEqual([
      [
        "agent",
        "prompt",
        lead.name,
        `[from hub-supervisor][${data.work.id} OPEN] Wake the Lead on start; read: maestro status ${data.work.id}`,
      ],
    ]);

    // A repeat start leaves the acknowledged panes alone but re-wakes the Lead
    // while its first item is still OPEN.
    const repeated = await runCliAt(fixture, room, args, fake.env);

    expect(phaseFree(repeated.stderr)).toBe("");
    expect(repeated.exitCode).toBe(0);
    expect((await opens()).map((command) => command[2])).toEqual([lead.name, lead.name]);

    // Once the Lead holds the item, a repeat start pushes no stale OPEN line.
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", data.work.id, "--json"], leadEnvironment))
        .exitCode,
    ).toBe(0);
    expect((await runCliAt(fixture, room, args, fake.env)).exitCode).toBe(0);
    expect(await opens()).toHaveLength(2);
  });
}, 20_000);
