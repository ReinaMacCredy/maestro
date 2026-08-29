import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  setFakeHerdrBehavior,
} from "./fake-herdr.ts";
import { runCli, runCliAt, withFixture } from "./helpers.ts";

function errorEnvelope(stderr: string): {
  error: { code: string; receipt?: { missing: Array<{ code: string }>; result: string } };
} {
  return JSON.parse(stderr) as {
    error: { code: string; receipt?: { missing: Array<{ code: string }>; result: string } };
  };
}

function successEnvelope(stdout: string): { data: { team: Record<string, unknown> } } {
  return JSON.parse(stdout) as { data: { team: Record<string, unknown> } };
}

test("team lifecycle CLI exposes the Supervisor-facing contract", async () => {
  await withFixture(async (fixture) => {
    const help = await runCli(fixture, ["help", "team"]);

    expect(help.exitCode).toBe(0);
    for (const subverb of [
      "open",
      "status",
      "health",
      "await-ready",
      "reconcile",
      "review",
      "advise",
      "stop",
    ]) {
      expect(help.stdout).toMatch(new RegExp(`^  ${subverb} {2,}\\S`, "m"));
    }
  });
});

test("team open stays CLOSED/STARTING until live role and sensor postconditions pass", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture, {
      agents: false,
      prompts: false,
      roleProcesses: false,
      sensor: false,
      sensorDelivery: false,
    });

    const opened = await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "alpha",
        "--repo",
        fixture.repo,
        "--operation",
        "open-alpha-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );

    expect(opened.exitCode).toBe(1);
    const failure = errorEnvelope(opened.stderr);
    expect(failure.error.code).toBe("TEAM_STARTING");
    expect(failure.error.receipt?.result).toBe("STARTING");
    expect(failure.error.receipt?.missing.map((entry) => entry.code)).toContain("role.missing");
    expect(failure.error.receipt?.missing.map((entry) => entry.code)).toContain("sensor.process");

    const commandsBeforeRetry = await fakeHerdrCommands(fake);
    const retried = await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "alpha",
        "--repo",
        fixture.repo,
        "--operation",
        "open-alpha-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(errorEnvelope(retried.stderr).error.receipt).toEqual(failure.error.receipt);
    expect(await fakeHerdrCommands(fake)).toEqual(commandsBeforeRetry);

    const commandsBeforeStatus = await fakeHerdrCommands(fake);
    const status = await runCliAt(
      fixture,
      room,
      ["team", "status", "alpha", "--json"],
      fake.env,
    );
    expect(status.exitCode).toBe(0);
    expect(successEnvelope(status.stdout).data.team).toMatchObject({
      generation: 1,
      health: null,
      review: "CLEAR",
      stage: "STARTING",
      verdict: "CLOSED",
    });
    expect(await fakeHerdrCommands(fake)).toEqual(commandsBeforeStatus);
  });
});

test("team open exposes OPERABLE only after the complete fake runtime is live", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);

    const opened = await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "beta",
        "--repo",
        fixture.repo,
        "--operation",
        "open-beta-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );

    expect(opened.exitCode).toBe(0);
    expect(successEnvelope(opened.stdout).data.team).toMatchObject({
      generation: 1,
      health: "READY",
      review: "CLEAR",
      stage: "ACTIVE",
      verdict: "OPERABLE",
    });
  });
});

test("team health alone can prove a STARTING team ready without recreating resources", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture, { sensorDelivery: false });

    const opened = await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "gamma",
        "--repo",
        fixture.repo,
        "--operation",
        "open-gamma-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(errorEnvelope(opened.stderr).error.code).toBe("TEAM_STARTING");
    await setFakeHerdrBehavior(fake, { sensorDelivery: true });

    const beforeHealth = await fakeHerdrCommands(fake);
    const health = await runCliAt(
      fixture,
      room,
      [
        "team",
        "health",
        "gamma",
        "--operation",
        "health-gamma-1",
        "--expected-revision",
        "1",
        "--json",
      ],
      fake.env,
    );

    expect(health.exitCode).toBe(0);
    expect(successEnvelope(health.stdout).data.team).toMatchObject({
      health: "READY",
      revision: 2,
      stage: "ACTIVE",
      verdict: "OPERABLE",
    });
    const healthCommands = (await fakeHerdrCommands(fake)).slice(beforeHealth.length);
    expect(healthCommands.some((command) => command.slice(0, 2).join(" ") === "agent start")).toBe(false);
    expect(healthCommands.some((command) => command.slice(0, 2).join(" ") === "tab create")).toBe(false);
    expect(healthCommands.some((command) => command.slice(0, 2).join(" ") === "pane split")).toBe(false);
  });
});

test("team await-ready finalizes readiness and terminal retries reuse the same receipt", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture, { sensorDelivery: false });
    await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "delta",
        "--repo",
        fixture.repo,
        "--operation",
        "open-delta-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    await setFakeHerdrBehavior(fake, { sensorDelivery: true });

    const ready = await runCliAt(
      fixture,
      room,
      [
        "team",
        "await-ready",
        "delta",
        "--operation",
        "ready-delta-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(ready.exitCode).toBe(0);
    const first = JSON.parse(ready.stdout) as {
      data: { receipt: Record<string, unknown>; team: Record<string, unknown> };
    };

    const beforeRetry = await fakeHerdrCommands(fake);
    const retried = await runCliAt(
      fixture,
      room,
      [
        "team",
        "await-ready",
        "delta",
        "--operation",
        "ready-delta-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(retried.exitCode).toBe(0);
    const second = JSON.parse(retried.stdout) as {
      data: { receipt: Record<string, unknown>; team: Record<string, unknown> };
    };
    expect(second.data.receipt).toEqual(first.data.receipt);
    expect(await fakeHerdrCommands(fake)).toEqual(beforeRetry);
  });
});

test("stale expected revisions fail closed before touching TeamRuntime", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "epsilon",
        "--repo",
        fixture.repo,
        "--operation",
        "open-epsilon-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    const before = await fakeHerdrCommands(fake);

    const stale = await runCliAt(
      fixture,
      room,
      [
        "team",
        "health",
        "epsilon",
        "--operation",
        "health-epsilon-stale",
        "--expected-revision",
        "0",
        "--json",
      ],
      fake.env,
    );

    expect(stale.exitCode).toBe(1);
    expect(errorEnvelope(stale.stderr).error.code).toBe("STALE_REVISION");
    expect(await fakeHerdrCommands(fake)).toEqual(before);
  });
});

test("an interrupted open adopts deterministic resources without duplicating them", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    const args = [
      "team",
      "open",
      "zeta",
      "--repo",
      fixture.repo,
      "--operation",
      "open-zeta-1",
      "--wait-ms",
      "0",
      "--json",
    ];
    const first = await runCliAt(fixture, room, args, fake.env);
    expect(first.exitCode).toBe(0);

    const database = new Database(join(room, ".maestro", "maestro.db"), { strict: true });
    database.query("DELETE FROM team_operation_effects WHERE operation_id = ?").run("open-zeta-1");
    database.query("DELETE FROM team_lifecycle WHERE team_id = ?").run("zeta");
    database.query(
      `UPDATE team_receipts
       SET status = 'ATTEMPTED', completed_at = NULL, observed_at = NULL,
           observed_runtime_revision = NULL, actual_json = NULL, result = NULL,
           after_json = NULL, missing_json = NULL
       WHERE operation_id = ?`,
    ).run("open-zeta-1");
    database.close();

    const beforeRetry = await fakeHerdrCommands(fake);
    const retried = await runCliAt(fixture, room, args, fake.env);
    expect(retried.exitCode).toBe(0);
    const retryCommands = (await fakeHerdrCommands(fake)).slice(beforeRetry.length);
    for (const forbidden of [
      "workspace create",
      "tab create",
      "agent start",
      "pane split",
      "pane run",
    ]) {
      expect(
        retryCommands.some((command) => command.slice(0, 2).join(" ") === forbidden),
      ).toBe(false);
    }
  });
});

test("fresh health marks duplicate required roles DEGRADED without automatic repair", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await runCliAt(
      fixture,
      room,
      [
        "team",
        "open",
        "eta",
        "--repo",
        fixture.repo,
        "--operation",
        "open-eta-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<Record<string, unknown>>;
      const observer = agents.find((agent) => agent.name === "observer-eta");
      if (!observer) throw new Error("fake observer missing");
      agents.push({ ...observer });
    });

    const beforeHealth = await fakeHerdrCommands(fake);
    const health = await runCliAt(
      fixture,
      room,
      [
        "team",
        "health",
        "eta",
        "--operation",
        "health-eta-1",
        "--json",
      ],
      fake.env,
    );
    expect(health.exitCode).toBe(0);
    const envelope = JSON.parse(health.stdout) as {
      data: {
        receipt: { missing: Array<{ code: string }> };
        team: Record<string, unknown>;
      };
    };
    expect(envelope.data.team).toMatchObject({
      health: "DEGRADED",
      stage: "ACTIVE",
      verdict: "DRAINING",
    });
    expect(envelope.data.receipt.missing.map((entry) => entry.code)).toContain("role.duplicate");
    const healthCommands = (await fakeHerdrCommands(fake)).slice(beforeHealth.length);
    for (const forbidden of ["agent start", "tab create", "pane split", "pane run"]) {
      expect(
        healthCommands.some((command) => command.slice(0, 2).join(" ") === forbidden),
      ).toBe(false);
    }
  });
});
