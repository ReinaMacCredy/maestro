import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./helpers-herdr.ts";
import { runCliAt, withFixture } from "./helpers.ts";
import { envelope, herdrPaneIdShape, phaseFree, watchRuntimeDirectory } from "./helpers-slp.ts";

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
          room,
          ["work", "accept", firstData.work.id, "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);
    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", firstData.team.teamId, "--json"],
      fake.env,
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
      room,
      ["team", "stop", data.team.teamId, "--json"],
      fake.env,
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

test("SLP v2 normal stop closes Peer Lead Watch transcript then workspace", async () => {
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
        runtimePaneId: string;
        teamId: string;
        workspaceId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
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
          room,
          ["work", "accept", data.work.id, "--json"],
          fake.env,
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

    const runtimePaneId = data.team.runtimePaneId;
    expect(runtimePaneId).toMatch(herdrPaneIdShape);
    // w702's other half: the fake mints base-36 workspace ids, so the shape
    // above is exercised against an id carrying a letter, the way a live
    // daemon issues them, instead of the digit-only ids that let an
    // impossible assertion pass.
    expect(runtimePaneId).toMatch(/^w[0-9]*[A-Z][0-9A-Z]*:p\d+$/);
    for (const malformed of ["", "w2G", "p3", "w2G:p", "w2G:t2", "workspace:pane"]) {
      expect(malformed).not.toMatch(herdrPaneIdShape);
    }
    const runtimeDirectory = watchRuntimeDirectory(
      fixture.repo,
      data.team.teamId,
      data.team.generation,
    );
    const transcript = join(runtimeDirectory, "state.json");
    await mkdir(runtimeDirectory, { recursive: true });
    await writeFile(transcript, "{}\n");
    await setFakeHerdrBehavior(fake, {
      closeWorkspaceWithLastTab: true,
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
      room,
      ["team", "stop", data.team.teamId, "--json"],
      fake.env,
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
    const peerClosed = closeIndex("tab", peerTab);
    const leadClosed = closeIndex("tab", leadTab);
    const watchClosed = commands.findIndex(
      (command) => command[0] === "pane" && command[1] === "close" && command[2] === runtimePaneId,
    );
    const workspaceClosed = closeIndex("workspace", data.team.workspaceId);
    expect(peerClosed).toBeGreaterThanOrEqual(0);
    expect(leadClosed).toBeGreaterThan(peerClosed);
    expect(watchClosed).toBeGreaterThan(leadClosed);
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
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
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
          room,
          ["work", "accept", data.work.id, "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);

    await setFakeHerdrBehavior(fake, { closeResources: false });
    const interrupted = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--json"],
      fake.env,
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
      room,
      ["team", "stop", data.team.teamId, "--json"],
      fake.env,
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

test("SLP v2 final workspace close failure leaves RUNNING and the Hub retries after a restore", async () => {
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
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
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
          room,
          ["work", "accept", started.work.id, "--json"],
          fake.env,
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
      room,
      ["team", "stop", started.team.teamId, "--json"],
      fake.env,
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
      team: { generation: number };
    }>((await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Commit only after final close", "--json"],
      fake.env,
    )).stdout);
    expect(restored.team.generation).toBe(1);
    const recovered = await runCliAt(
      fixture,
      room,
      ["team", "stop", started.team.teamId, "--json"],
      fake.env,
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
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
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
          room,
          ["work", "accept", started.work.id, "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);

    const commandCount = (await fakeHerdrCommands(fake)).length;
    await setFakeHerdrBehavior(fake, { workspaceListDelayMs: 400 });
    const stopping = runCliAt(
      fixture,
      room,
      ["team", "stop", started.team.teamId, "--json"],
      fake.env,
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
      leadEnvironment,
    );
    const repeating = runCliAt(
      fixture,
      room,
      ["team", "stop", started.team.teamId, "--json"],
      fake.env,
    );
    const competing = await Promise.all([
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "add", "late work", "--json"],
        leadEnvironment,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["work", "note", started.work.id, "late note", "--json"],
        leadEnvironment,
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
        room,
        ["work", "accept", started.work.id, "--json"],
        fake.env,
      ),
      runCliAt(
        fixture,
        fixture.repo,
        ["decide", "Late decision", "--why", "stop already fenced writes", "--json"],
        leadEnvironment,
      ),
      runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "Serialize stop and work", "--json"],
        fake.env,
      ),
    ]);
    const repeated = await repeating;
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
      Array.from({ length: 7 }, () => ({ code: "TEAM_STOP_IN_PROGRESS", exitCode: 1 })),
    );
    expect(phaseFree(repeated.stderr)).toBe("");
    expect(repeated.exitCode).toBe(0);
    expect(envelope<{ team: { state: string } }>(repeated.stdout).team.state).toBe("STOPPED");
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    project.exec("PRAGMA busy_timeout = 5000");
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
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
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
          leadEnvironment,
        )
      ).stdout,
    ).work;
    const returnedWork = envelope<{ work: { id: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "add", "Return before emergency", "--json"],
          leadEnvironment,
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

test("SLP v2 Hub normal stop carries its reason to the Hub ledger and status", async () => {
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
      { ...fake.env, HERDR_PANE_ID: "hub:p0" },
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
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
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
          room,
          ["work", "accept", data.work.id, "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);

    const reason = "all green: w1 accepted, nothing left";
    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--reason", reason, "--json"],
      fake.env,
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
    ).toEqual({ actor: "hub-supervisor", emergency: 0, reason });
    hub.close();

    const status = await runCliAt(fixture, room, ["status"], fake.env);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain(
      `${data.team.teamId} g${data.team.generation} STOPPED (supervisor): ${reason}`,
    );
    const statusJson = envelope<{ teams: Array<{ stop: unknown }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout,
    );
    expect(statusJson.teams[0]?.stop).toEqual({ actor: "hub-supervisor", emergency: false, reason });
  });
}, 30_000);
