import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./fake-herdr.ts";
import { idFrom, runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

function envelope(value: string): Record<string, any> {
  return JSON.parse(value) as Record<string, any>;
}

async function openTeam(
  fixture: Fixture,
  room: string,
  teamId: string,
  env: Record<string, string>,
): Promise<void> {
  const opened = await runCliAt(
    fixture,
    room,
    [
      "team",
      "open",
      teamId,
      "--repo",
      fixture.repo,
      "--operation",
      `open-${teamId}-1`,
      "--wait-ms",
      "0",
      "--json",
    ],
    env,
  );
  expect(opened.exitCode).toBe(0);
}

async function raiseReview(
  fixture: Fixture,
  room: string,
  teamId: string,
  env: Record<string, string>,
): Promise<void> {
  const triggered = await runCliAt(
    fixture,
    room,
    [
      "team",
      "review",
      "trigger",
      teamId,
      "--operation",
      `trigger-${teamId}-reconcile`,
      "--rule",
      "semantic.role-boundary",
      "--actor",
      "lead-repo",
      "--evidence",
      "wrong owner answered",
      "--json",
    ],
    env,
  );
  const packet = envelope(triggered.stdout).data.packet;
  const raised = await runCliAt(
    fixture,
    room,
    [
      "team",
      "review",
      "raise",
      teamId,
      "--operation",
      `raise-${teamId}-reconcile`,
      "--packet",
      packet.id,
      "--capability",
      packet.capability,
      "--finding",
      "role boundary crossed",
      "--json",
    ],
    env,
  );
  expect(raised.exitCode).toBe(0);
}

async function openDispatch(fixture: Fixture, workId: string, env: Record<string, string>): Promise<string> {
  const opened = await runCli(
    fixture,
    [
      "dispatch",
      "open",
      workId,
      "--objective",
      "finish the bounded stop",
      "--owned-scope",
      "fixture",
      "--excluded-scope",
      "everything else",
      "--mutation",
      "write-bounded: fixture",
      "--stop-condition",
      "handback filed",
      "--lane",
      "delivery",
      "--evidence-required",
      "source: fixture",
      "--pane",
      `peer-${workId}`,
    ],
    env,
  );
  expect(opened.exitCode, opened.stderr).toBe(0);
  const dispatchId = opened.stdout.match(/\bx\d+\b/)?.[0];
  if (!dispatchId) throw new Error(`missing dispatch id: ${opened.stdout}`);
  return dispatchId;
}

function closeIndex(commands: string[][], command: string, id: string): number {
  return commands.findIndex(
    (candidate) => candidate.slice(0, 2).join(" ") === command && candidate[2] === id,
  );
}

test("explicit reconcile repairs only selected resources and never clears review", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "chi", fake.env);
    await raiseReview(fixture, room, "chi", fake.env);
    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<Record<string, any>>;
      const observer = agents.find((agent) => agent.name === "observer-chi");
      if (!observer) throw new Error("fake observer missing");
      state.agents = agents.filter((agent) => agent.name !== "observer-chi");
      const processes = state.processes as Record<string, unknown>;
      delete processes[String(observer.pane_id)];
      const sensor = (state.panes as Array<Record<string, any>>).find((pane) => {
        const info = processes[String(pane.pane_id)] as Record<string, unknown> | undefined;
        return JSON.stringify(info ?? {}).includes("maestro-team-sensor");
      });
      if (!sensor) throw new Error("fake sensor missing");
      delete processes[String(sensor.pane_id)];
    });
    await runCliAt(
      fixture,
      room,
      ["team", "health", "chi", "--operation", "health-chi-broken", "--json"],
      fake.env,
    );

    const beforeObserver = await fakeHerdrCommands(fake);
    const observerOnly = await runCliAt(
      fixture,
      room,
      [
        "team",
        "reconcile",
        "chi",
        "--operation",
        "reconcile-chi-observer",
        "--requested-by",
        "supervisor-chi",
        "--resource",
        "observer",
        "--json",
      ],
      fake.env,
    );
    expect(observerOnly.exitCode).toBe(1);
    expect(envelope(observerOnly.stderr).error.code).toBe("TEAM_RECONCILE_INCOMPLETE");
    expect(envelope(observerOnly.stderr).error.team).toMatchObject({
      health: "DEGRADED",
      review: "REVIEW_REQUIRED",
      verdict: "DRAINING",
    });
    const observerCommands = (await fakeHerdrCommands(fake)).slice(beforeObserver.length);
    expect(observerCommands.some((command) => command.slice(0, 2).join(" ") === "agent start")).toBe(true);
    expect(observerCommands.some((command) => command.slice(0, 2).join(" ") === "pane run")).toBe(false);

    const beforeSensor = await fakeHerdrCommands(fake);
    const sensorOnly = await runCliAt(
      fixture,
      room,
      [
        "team",
        "reconcile",
        "chi",
        "--operation",
        "reconcile-chi-sensor",
        "--requested-by",
        "supervisor-chi",
        "--resource",
        "sensor",
        "--json",
      ],
      fake.env,
    );
    expect(sensorOnly.exitCode).toBe(0);
    expect(envelope(sensorOnly.stdout).data.team).toMatchObject({
      health: "READY",
      review: "REVIEW_REQUIRED",
      verdict: "REVIEW_HOLD",
    });
    const sensorCommands = (await fakeHerdrCommands(fake)).slice(beforeSensor.length);
    expect(sensorCommands.some((command) => command.slice(0, 2).join(" ") === "pane run")).toBe(true);
    expect(sensorCommands.some((command) => command.slice(0, 2).join(" ") === "agent start")).toBe(false);
  });
});

test("only the exact team Supervisor can reconcile or stop the generation", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "rho", fake.env);
    const before = await fakeHerdrCommands(fake);

    const reconcile = await runCliAt(
      fixture,
      room,
      [
        "team",
        "reconcile",
        "rho",
        "--operation",
        "reconcile-rho-wrong-actor",
        "--requested-by",
        "lead-repo",
        "--resource",
        "sensor",
        "--json",
      ],
      fake.env,
    );
    expect(reconcile.exitCode).toBe(1);
    expect(envelope(reconcile.stderr).error.code).toBe("TEAM_AUTHORITY_REQUIRED");

    const stop = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "rho",
        "--operation",
        "stop-rho-wrong-actor",
        "--requested-by",
        "lead-repo",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(stop.exitCode).toBe(1);
    expect(envelope(stop.stderr).error.code).toBe("TEAM_AUTHORITY_REQUIRED");
    expect(await fakeHerdrCommands(fake)).toEqual(before);
    const status = await runCliAt(fixture, room, ["team", "status", "rho", "--json"], fake.env);
    expect(envelope(status.stdout).data.team).toMatchObject({ stage: "ACTIVE", verdict: "OPERABLE" });
  });
});

test("normal stop closes work seats before sensor and Observer, Supervisor last, then proves absence", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "sigma", fake.env);
    const beforeState = await readFakeHerdrState(fake);
    const workspace = beforeState.workspaces.find(
      (candidate: Record<string, unknown>) => candidate.label === "team-sigma-g1",
    );
    const tabs = new Map(
      beforeState.tabs.map((tab: Record<string, string>) => [tab.label, tab.tab_id]),
    );
    const sensorPane = beforeState.panes.find((pane: Record<string, string>) =>
      typeof pane.pane_id === "string" &&
      JSON.stringify(beforeState.processes[pane.pane_id] ?? {}).includes("maestro-team-sensor")
    );
    expect(workspace?.workspace_id).toBeDefined();
    expect(sensorPane?.pane_id).toBeDefined();

    const beforeCommands = await fakeHerdrCommands(fake);
    const stopped = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "sigma",
        "--operation",
        "stop-sigma-1",
        "--requested-by",
        "supervisor-sigma",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(stopped.exitCode, stopped.stderr).toBe(0);
    const result = envelope(stopped.stdout).data;
    expect(result.team).toMatchObject({
      generation: 1,
      health: null,
      stage: "STOPPED",
      verdict: "CLOSED",
    });
    expect(result.receipt).toMatchObject({
      executedBy: "test-session",
      forced: false,
      generation: 1,
      missing: [],
      requestedBy: "supervisor-sigma",
      result: "STOPPED",
    });

    const commands = (await fakeHerdrCommands(fake)).slice(beforeCommands.length);
    const lead = closeIndex(commands, "tab close", tabs.get("team:sigma:g1:lead") as string);
    const sensor = closeIndex(commands, "pane close", sensorPane.pane_id);
    const observer = closeIndex(commands, "tab close", tabs.get("team:sigma:g1:observer") as string);
    const supervisor = closeIndex(commands, "tab close", tabs.get("team:sigma:g1:supervisor") as string);
    const workspaceClose = closeIndex(commands, "workspace close", workspace.workspace_id);
    expect(lead).toBeGreaterThanOrEqual(0);
    expect(sensor).toBeGreaterThan(lead);
    expect(observer).toBeGreaterThan(sensor);
    expect(supervisor).toBeGreaterThan(observer);
    expect(workspaceClose).toBeGreaterThan(supervisor);

    const afterState = await readFakeHerdrState(fake);
    expect(afterState.workspaces).toEqual([]);
    expect(afterState.tabs).toEqual([]);
    expect(afterState.panes).toEqual([]);
    expect(afterState.agents).toEqual([]);
    expect(afterState.processes).toEqual({});

    const reopened = await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "sigma",
        "--repo",
        fixture.repo,
        "--operation",
        "open-sigma-2",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(reopened.exitCode, reopened.stderr).toBe(0);
    expect(envelope(reopened.stdout).data.team.generation).toBe(2);
    const beforeRetry = await fakeHerdrCommands(fake);
    const retry = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "sigma",
        "--operation",
        "stop-sigma-1",
        "--requested-by",
        "supervisor-sigma",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(retry.exitCode, retry.stderr).toBe(0);
    expect(envelope(retry.stdout).data).toMatchObject({
      receipt: { generation: 1, result: "STOPPED" },
      team: { generation: 2, stage: "ACTIVE" },
    });
    expect(await fakeHerdrCommands(fake)).toEqual(beforeRetry);
  });
});

test("stop timeout persists STOPPING before drain and names live roles, processes, handback, and lease", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "tau", fake.env);
    const added = await runCli(
      fixture,
      ["work", "add", "unfinished team work", "--atomic-reason", "fixture"],
      fake.env,
    );
    const workId = idFrom(added);
    expect((await runCli(fixture, ["work", "start", workId], fake.env)).exitCode).toBe(0);
    const dispatchId = await openDispatch(fixture, workId, fake.env);

    const beforeCommands = await fakeHerdrCommands(fake);
    const stopped = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "tau",
        "--operation",
        "stop-tau-timeout",
        "--requested-by",
        "supervisor-tau",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(stopped.exitCode).toBe(1);
    const failure = envelope(stopped.stderr).error;
    expect(failure.code).toBe("TEAM_STOPPING");
    expect(failure.team).toMatchObject({ stage: "STOPPING", verdict: "CLOSED" });
    expect(failure.receipt.result).toBe("STOP_TIMEOUT");
    const missing = failure.receipt.missing as Array<{ code: string; resource: string }>;
    expect(missing.some((entry) => entry.code === "shutdown.lease" && entry.resource === `work:${workId}`)).toBe(true);
    expect(
      missing.some((entry) => entry.code === "shutdown.handback" && entry.resource === `dispatch:${dispatchId}`),
      JSON.stringify(missing),
    ).toBe(true);
    expect(missing.some((entry) => entry.code === "shutdown.role")).toBe(true);
    expect(missing.some((entry) => entry.code === "shutdown.process")).toBe(true);

    const commands = (await fakeHerdrCommands(fake)).slice(beforeCommands.length);
    expect(commands.some((command) => command[0] === "agent" && command[1] === "prompt" && command[2] === "lead-repo")).toBe(true);
    expect(commands.some((command) => command[1] === "close")).toBe(false);
    const state = await readFakeHerdrState(fake);
    expect(state.agents.some((agent: Record<string, string>) => agent.name === "observer-tau")).toBe(true);
    expect(Object.values(state.processes).some((process) => JSON.stringify(process).includes("maestro-team-sensor"))).toBe(true);

    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { strict: true });
    const receipts = roomDatabase
      .query<{ attempted_at: string; completed_at: string; operation_id: string; status: string }, []>(
        `SELECT operation_id, status, attempted_at, completed_at
         FROM team_receipts
         WHERE operation_id IN ('stop-tau-timeout:stage', 'stop-tau-timeout')
         ORDER BY attempted_at`,
      )
      .all();
    roomDatabase.close();
    expect(receipts.map((receipt) => receipt.operation_id)).toEqual([
      "stop-tau-timeout:stage",
      "stop-tau-timeout",
    ]);
    expect(receipts.every((receipt) => receipt.status === "FINALIZED")).toBe(true);
    const stageCompletedAt = receipts[0]?.completed_at;
    const stopAttemptedAt = receipts[1]?.attempted_at;
    expect(Boolean(stageCompletedAt && stopAttemptedAt && stageCompletedAt <= stopAttemptedAt)).toBe(true);

    const blockedAdd = await runCli(
      fixture,
      ["work", "add", "must not start while stopping", "--atomic-reason", "fixture"],
      fake.env,
    );
    expect(blockedAdd.exitCode).toBe(1);
    expect(envelope(blockedAdd.stderr).error).toMatchObject({ code: "GATE_BLOCKED" });
    expect((await runCli(fixture, ["work", "note", workId, "drain evidence recorded"], fake.env)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "release", workId], fake.env)).exitCode).toBe(0);
  });
});

test("force stop requires explicit reason and evidence, records possible loss, and cannot claim graceful completion", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "upsilon", fake.env);
    const added = await runCli(
      fixture,
      ["work", "add", "possibly lost work", "--atomic-reason", "fixture"],
      fake.env,
    );
    const workId = idFrom(added);
    expect((await runCli(fixture, ["work", "start", workId], fake.env)).exitCode).toBe(0);
    const dispatchId = await openDispatch(fixture, workId, fake.env);

    const timedOut = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "upsilon",
        "--operation",
        "stop-upsilon-timeout",
        "--requested-by",
        "supervisor-upsilon",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(timedOut.exitCode).toBe(1);

    const beforeRejected = await fakeHerdrCommands(fake);
    const rejected = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "upsilon",
        "--operation",
        "stop-upsilon-force-invalid",
        "--requested-by",
        "supervisor-upsilon",
        "--force",
        "--json",
      ],
      fake.env,
    );
    expect(rejected.exitCode).toBe(1);
    expect(envelope(rejected.stderr).error.code).toBe("MISSING_ARGUMENT");
    expect(await fakeHerdrCommands(fake)).toEqual(beforeRejected);

    const beforeForced = await fakeHerdrCommands(fake);
    const forced = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "upsilon",
        "--operation",
        "stop-upsilon-force",
        "--requested-by",
        "supervisor-upsilon",
        "--force",
        "--reason",
        "owner accepted bounded loss",
        "--evidence",
        "incident: fixture-1",
        "--json",
      ],
      fake.env,
    );
    expect(forced.exitCode, forced.stderr).toBe(0);
    const result = envelope(forced.stdout).data;
    expect(result.team).toMatchObject({ stage: "STOPPED", verdict: "CLOSED" });
    expect(result.receipt).toMatchObject({
      forced: true,
      missing: [],
      result: "FORCED_STOPPED",
    });
    expect(result.receipt.actual).toMatchObject({
      evidence: "incident: fixture-1",
      forceReason: "owner accepted bounded loss",
    });
    expect(result.receipt.actual.possibleLoss).toEqual(expect.arrayContaining([
      expect.objectContaining({ resource: `work:${workId}` }),
      expect.objectContaining({ resource: `dispatch:${dispatchId}` }),
    ]));
    const commands = (await fakeHerdrCommands(fake)).slice(beforeForced.length);
    expect(commands.some((command) => command[0] === "agent" && command[1] === "prompt")).toBe(false);
    expect(commands.some((command) => command.slice(0, 2).join(" ") === "workspace close")).toBe(true);
  });
});

test("a failed workspace close stays STOPPING until a later operation proves absence", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture, { closeResources: false });
    await openTeam(fixture, room, "phi", fake.env);

    const incomplete = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "phi",
        "--operation",
        "stop-phi-incomplete",
        "--requested-by",
        "supervisor-phi",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(incomplete.exitCode).toBe(1);
    const failure = envelope(incomplete.stderr).error;
    expect(failure.code).toBe("TEAM_STOPPING");
    expect(failure.team).toMatchObject({ stage: "STOPPING", verdict: "CLOSED" });
    expect(failure.receipt).toMatchObject({ forced: false, result: "STOP_INCOMPLETE" });
    expect(
      failure.receipt.missing.some((entry: { code: string }) => entry.code === "shutdown.workspace"),
    ).toBe(true);
    expect((await readFakeHerdrState(fake)).workspaces).toHaveLength(1);

    await setFakeHerdrBehavior(fake, { closeResources: true });
    const completed = await runCliAt(
      fixture,
      room,
      [
        "team",
        "stop",
        "phi",
        "--operation",
        "stop-phi-retry",
        "--requested-by",
        "supervisor-phi",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(completed.exitCode, completed.stderr).toBe(0);
    expect(envelope(completed.stdout).data.team).toMatchObject({
      stage: "STOPPED",
      verdict: "CLOSED",
    });
  });
});
