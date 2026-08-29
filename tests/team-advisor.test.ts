import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import {
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
} from "./fake-herdr.ts";
import { runCliAt, withFixture, type Fixture } from "./helpers.ts";

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

function authorityCounts(databasePath: string): Record<string, number> {
  const database = new Database(databasePath, { strict: true });
  const result = Object.fromEntries(
    ["work", "decisions", "dispatches"].map((table) => [
      table,
      database.query<{ count: number }, []>(`SELECT COUNT(*) AS count FROM ${table}`).get()?.count ?? 0,
    ]),
  );
  database.close();
  return result;
}

test("bounded Advisor returns one recommendation, touches no authority records, and stops", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture, {
      advisorRecommendation: "Keep review and health as independent axes.",
    });
    await openTeam(fixture, room, "mu", fake.env);
    const databasePath = join(room, ".maestro", "maestro.db");
    const beforeAuthority = authorityCounts(databasePath);
    const beforeRuntime = await fakeHerdrCommands(fake);

    const advised = await runCliAt(
      fixture,
      room,
      [
        "team",
        "advise",
        "mu",
        "--operation",
        "advise-mu-1",
        "--requested-by",
        "lead-repo",
        "--decision",
        "d19",
        "--question",
        "Should review clear health?",
        "--context",
        "w44",
        "--context",
        "p7",
        "--stop-condition",
        "return one recommendation",
        "--timeout-ms",
        "5000",
        "--json",
      ],
      fake.env,
    );

    expect(advised.exitCode).toBe(0);
    expect(envelope(advised.stdout).data).toMatchObject({
      consultation: {
        decisionRef: "d19",
        recommendation: "Keep review and health as independent axes.",
        requestedBy: "lead-repo",
        status: "COMPLETED",
      },
      team: { verdict: "OPERABLE" },
    });
    expect(authorityCounts(databasePath)).toEqual(beforeAuthority);
    const commands = (await fakeHerdrCommands(fake)).slice(beforeRuntime.length);
    for (const required of ["tab create", "agent start", "agent prompt", "agent read", "tab close"] ) {
      expect(commands.some((command) => command.slice(0, 2).join(" ") === required)).toBe(true);
    }
    const state = await readFakeHerdrState(fake);
    expect(state.agents.some((agent: { name: string }) => agent.name === "advisor-mu")).toBe(false);
  });
});

test("Advisor without a return marker is a failed consultation, never an empty recommendation", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture, { advisorRecommendation: null });
    await openTeam(fixture, room, "nu", fake.env);

    const advised = await runCliAt(
      fixture,
      room,
      [
        "team",
        "advise",
        "nu",
        "--operation",
        "advise-nu-1",
        "--requested-by",
        "supervisor-nu",
        "--decision",
        "d20",
        "--question",
        "What should we do?",
        "--stop-condition",
        "return one recommendation",
        "--timeout-ms",
        "5000",
        "--json",
      ],
      fake.env,
    );

    expect(advised.exitCode).toBe(1);
    const failure = envelope(advised.stderr).error;
    expect(failure.code).toBe("ADVISOR_FAILED");
    expect(failure.consultation.recommendation).toBeNull();
    expect(failure.consultation.status).toBe("FAILED");
    const state = await readFakeHerdrState(fake);
    expect(state.agents.some((agent: { name: string }) => agent.name === "advisor-nu")).toBe(false);
  });
});
