import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
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
      `trigger-${teamId}-gate`,
      "--rule",
      "semantic.status-contradiction",
      "--actor",
      "lead-repo",
      "--evidence",
      "claim contradicts active work",
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
      `raise-${teamId}-gate`,
      "--packet",
      packet.id,
      "--capability",
      packet.capability,
      "--finding",
      "contradiction requires Supervisor review",
      "--json",
    ],
    env,
  );
  expect(raised.exitCode).toBe(0);
}

test("project binding stores no READY copy and fresh TeamControl gates REVIEW_HOLD/DRAINING", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "omicron", fake.env);

    const first = await runCli(
      fixture,
      ["work", "add", "bound work", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    expect(first.exitCode, first.stderr).toBe(0);
    const firstId = idFrom(first);
    const beforeStart = await fakeHerdrCommands(fake);
    const started = await runCli(fixture, ["work", "start", firstId], fake.env);
    expect(started.exitCode).toBe(0);
    expect((await fakeHerdrCommands(fake)).length).toBeGreaterThan(beforeStart.length);
    const bundle = await runCli(
      fixture,
      ["bundle", "open", "gate-bundle", "--work", firstId],
      fake.env,
    );
    expect(bundle.exitCode, bundle.stderr).toBe(0);

    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), { strict: true });
    expect(
      projectDatabase.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM team_lifecycle").get()?.count,
    ).toBe(0);
    const bindingColumns = projectDatabase
      .query<{ name: string }, []>("PRAGMA table_info(team_local_bindings)")
      .all()
      .map((column) => column.name);
    expect(bindingColumns).not.toContain("health");
    expect(bindingColumns).not.toContain("review");
    expect(bindingColumns).not.toContain("verdict");
    projectDatabase.close();

    await raiseReview(fixture, room, "omicron", fake.env);
    const bundleClose = await runCli(fixture, ["bundle", "close", "gate-bundle"], fake.env);
    expect(bundleClose.exitCode).toBe(1);
    expect(envelope(bundleClose.stderr).error).toMatchObject({
      code: "GATE_BLOCKED",
      origin: "team-control",
    });
    expect(envelope(bundleClose.stderr).error.message).toContain("REVIEW_HOLD");
    const completion = await runCli(
      fixture,
      ["work", "done", firstId, "--evidence", "bounded work complete"],
      fake.env,
    );
    expect(completion.exitCode).toBe(1);
    expect(envelope(completion.stderr).error).toMatchObject({
      code: "GATE_BLOCKED",
      origin: "team-control",
    });
    expect(envelope(completion.stderr).error.message).toContain("REVIEW_HOLD");
    expect((await runCli(fixture, ["work", "note", firstId, "evidence retained"], fake.env)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "release", firstId], fake.env)).exitCode).toBe(0);

    const next = await runCli(
      fixture,
      ["work", "add", "next work", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    const nextId = idFrom(next);
    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<Record<string, unknown>>;
      const observer = agents.find((agent) => agent.name === "observer-omicron");
      if (!observer) throw new Error("fake observer missing");
      agents.push({ ...observer });
    });
    const degraded = await runCliAt(
      fixture,
      room,
      ["team", "health", "omicron", "--operation", "health-omicron-degraded", "--json"],
      fake.env,
    );
    expect(envelope(degraded.stdout).data.team.verdict).toBe("DRAINING");

    const blockedStart = await runCli(fixture, ["work", "start", nextId], fake.env);
    expect(blockedStart.exitCode).toBe(1);
    expect(envelope(blockedStart.stderr).error.message).toContain("DRAINING");

    const blockedAdd = await runCli(
      fixture,
      ["work", "add", "forbidden new work", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    expect(blockedAdd.exitCode).toBe(1);
    expect(envelope(blockedAdd.stderr).error.message).toContain("DRAINING");

    const blockedDispatch = await runCli(
      fixture,
      [
        "dispatch",
        "open",
        nextId,
        "--objective",
        "should not open",
        "--owned-scope",
        "none",
        "--excluded-scope",
        "all",
        "--mutation",
        "no-write",
        "--stop-condition",
        "blocked",
        "--lane",
        "scout",
        "--evidence-required",
        "source: none",
        "--pane",
        "fake:pane",
      ],
      fake.env,
    );
    expect(blockedDispatch.exitCode).toBe(1);
    expect(envelope(blockedDispatch.stderr).error.message).toContain("DRAINING");

    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { strict: true });
    expect(
      roomDatabase
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM team_receipts WHERE kind LIKE 'team.control.%'",
        )
        .get()?.count,
    ).toBeGreaterThan(0);
    roomDatabase.close();
  });
});

test("invalid project binding fails closed before runtime inspection", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "pi", fake.env);
    const work = await runCli(
      fixture,
      ["work", "add", "binding failure", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    expect(work.exitCode, work.stderr).toBe(0);
    const workId = idFrom(work);
    const projectDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"), { strict: true });
    projectDatabase.query("UPDATE team_local_bindings SET token = 'forged'").run();
    projectDatabase.close();
    const before = await fakeHerdrCommands(fake);

    const blocked = await runCli(fixture, ["work", "start", workId], fake.env);

    expect(blocked.exitCode).toBe(1);
    expect(envelope(blocked.stderr).error).toMatchObject({
      code: "GATE_BLOCKED",
      origin: "team-control",
    });
    expect(envelope(blocked.stderr).error.message).toContain("binding");
    expect(await fakeHerdrCommands(fake)).toEqual(before);
  });
});

test("a STARTING team binds its project but every normal work gate remains CLOSED", async () => {
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
        "rho",
        "--repo",
        fixture.repo,
        "--operation",
        "open-rho-1",
        "--wait-ms",
        "0",
        "--json",
      ],
      fake.env,
    );
    expect(envelope(opened.stderr).error.code).toBe("TEAM_STARTING");

    const added = await runCli(
      fixture,
      ["work", "add", "must remain closed", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    expect(added.exitCode).toBe(1);
    expect(envelope(added.stderr).error).toMatchObject({
      code: "GATE_BLOCKED",
      origin: "team-control",
    });
    expect(envelope(added.stderr).error.message).toContain("CLOSED");
  });
});

test("multiple projects consume one Room verdict without copying lifecycle state", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "tau", fake.env);
    const second = join(fixture.root, "second-project");
    await mkdir(join(second, ".maestro", "plugins"), { recursive: true });
    await writeFile(join(second, ".maestro", "config"), `${JSON.stringify({ plugins: [] })}\n`);

    const bound = await runCliAt(
      fixture,
      room,
      [
        "team",
        "bind",
        "tau",
        "--operation",
        "bind-tau-second",
        "--requested-by",
        "supervisor-tau",
        "--repo",
        second,
        "--json",
      ],
      fake.env,
    );
    expect(bound.exitCode, bound.stderr).toBe(0);

    const firstWork = await runCli(
      fixture,
      ["work", "add", "first project work", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    const firstId = idFrom(firstWork);
    expect((await runCli(fixture, ["work", "start", firstId], fake.env)).exitCode).toBe(0);
    const secondWork = await runCliAt(
      fixture,
      second,
      ["work", "add", "second project work", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    const secondId = idFrom(secondWork);
    expect((await runCliAt(fixture, second, ["work", "start", secondId], fake.env)).exitCode).toBe(0);

    await raiseReview(fixture, room, "tau", fake.env);
    const firstDone = await runCli(
      fixture,
      ["work", "done", firstId, "--evidence", "done"],
      fake.env,
    );
    const secondDone = await runCliAt(
      fixture,
      second,
      ["work", "done", secondId, "--evidence", "done"],
      fake.env,
    );
    expect(envelope(firstDone.stderr).error.message).toContain("REVIEW_HOLD");
    expect(envelope(secondDone.stderr).error.message).toContain("REVIEW_HOLD");

    for (const databasePath of [
      join(fixture.repo, ".maestro", "maestro.db"),
      join(second, ".maestro", "maestro.db"),
    ]) {
      const database = new Database(databasePath, { strict: true });
      expect(
        database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM team_lifecycle").get()?.count,
      ).toBe(0);
      database.close();
    }
    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { strict: true });
    expect(
      roomDatabase
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM team_project_bindings WHERE team_id = 'tau' AND status = 'ACTIVE'",
        )
        .get()?.count,
    ).toBe(2);
    roomDatabase.close();
  });
});

test("REVIEW_HOLD denies final handback acceptance after allowing the bounded return", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "upsilon", fake.env);
    const work = await runCli(
      fixture,
      ["work", "add", "return under review", "--atomic-reason", "gate fixture"],
      fake.env,
    );
    const workId = idFrom(work);
    const opened = await runCli(
      fixture,
      [
        "dispatch",
        "open",
        workId,
        "--objective",
        "return bounded evidence",
        "--owned-scope",
        "fixture",
        "--excluded-scope",
        "external effects",
        "--mutation",
        "no-write",
        "--stop-condition",
        "handback filed",
        "--lane",
        "delivery",
        "--evidence-required",
        "source: fixture",
        "--pane",
        "fake:pane",
      ],
      fake.env,
    );
    const dispatchId = opened.stdout.match(/^(\S+) \[open\]/)?.[1];
    if (!dispatchId) throw new Error(`dispatch id missing: ${opened.stdout}`);
    expect((await runCli(fixture, ["dispatch", "accept", dispatchId], fake.env)).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["dispatch", "confirm", dispatchId, "--session", "test-session"], fake.env))
        .exitCode,
    ).toBe(0);
    const filed = await runCli(
      fixture,
      [
        "handback",
        "file",
        dispatchId,
        "--status",
        "DONE",
        "--claim",
        "bounded return filed",
        "--proof",
        "source: fixture",
        "--assumptions",
        "None",
        "--residual-risks",
        "None",
        "--incidental-findings",
        "None",
      ],
      fake.env,
    );
    expect(filed.exitCode, filed.stderr).toBe(0);
    const handbackId = filed.stdout.match(/^(\S+) \[[A-Z_]+\]/)?.[1];
    if (!handbackId) throw new Error(`handback id missing: ${filed.stdout}`);

    await raiseReview(fixture, room, "upsilon", fake.env);
    const reviewed = await runCli(
      fixture,
      ["handback", "review", handbackId, "--note", "candidate accepted"],
      fake.env,
    );
    expect(reviewed.exitCode).toBe(1);
    expect(envelope(reviewed.stderr).error).toMatchObject({
      code: "GATE_BLOCKED",
      origin: "team-control",
    });
    expect(envelope(reviewed.stderr).error.message).toContain("REVIEW_HOLD");
  });
});
