import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return a durable packet",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "handback is readable",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: CLI regression",
    "--pane",
    "w1:pA",
  ];
}

function idFromLine(result: { stdout: string }, prefix: string): string {
  const id = result.stdout.match(new RegExp(`^(${prefix}\\d+) `))?.[1];
  if (!id) throw new Error(`missing ${prefix} id in stdout: ${result.stdout}`);
  return id;
}

async function openDispatch(fixture: Fixture, work: string): Promise<string> {
  const opened = await runCli(fixture, dispatchOpenArgs(work));
  expect(opened.exitCode).toBe(0);
  return idFromLine(opened, "x");
}

async function fileHandback(
  fixture: Fixture,
  dispatch: string,
  claim: string,
  status = "DONE",
): Promise<string> {
  expect((await runCli(fixture, ["dispatch", "accept", dispatch])).exitCode).toBe(0);
  const filed = await runCli(fixture, [
    "handback",
    "file",
    dispatch,
    "--status",
    status,
    "--claim",
    claim,
    "--proof",
    "source: CLI regression",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ]);
  expect(filed.exitCode).toBe(0);
  return idFromLine(filed, "h");
}

function insertHandback(
  fixture: Fixture,
  input: { claim: string; dispatch: string; id: string; status: string },
): void {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
  database
    .query(
      `INSERT INTO handbacks
        (id, dispatch_id, status, claim, proof, assumptions, residual_risks,
         incidental_findings, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      input.id,
      input.dispatch,
      input.status,
      input.claim,
      "source: inserted fixture",
      "None",
      "None",
      "None",
      "2099-01-01T00:00:00.000Z",
    );
  database.close();
}

test("400 handback show accepts a dispatch id and resolves its latest handback", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "latest handback", "--atomic-reason", "fixture"]),
    );
    const dispatch = await openDispatch(fixture, work);
    const first = await fileHandback(fixture, dispatch, "first claim");
    insertHandback(fixture, {
      claim: "latest claim\nprivate detail",
      dispatch,
      id: "h2",
      status: "BLOCKED",
    });

    const byDispatch = await runCli(fixture, ["handback", "show", dispatch]);
    expect(byDispatch.exitCode).toBe(0);
    expect(byDispatch.stdout).toStartWith("h2 [BLOCKED]\n");
    expect(byDispatch.stdout).toContain("claim: latest claim\nprivate detail\n");

    const byHandback = await runCli(fixture, ["handback", "show", first]);
    expect(byHandback.exitCode).toBe(0);
    expect(byHandback.stdout).toStartWith(`${first} [DONE]\n`);
  });
});

test("401 dispatch show pins singular and plural handback lines", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "show handbacks", "--atomic-reason", "fixture"]),
    );
    const dispatch = await openDispatch(fixture, work);
    const handback = await fileHandback(fixture, dispatch, "shown claim");
    const expected = [
      `${dispatch} [returned]`,
      `work: ${work}`,
      "objective: Return a durable packet",
      "owned scope: src/plugins/dispatch.ts",
      "excluded scope: push",
      "mutation: write-bounded: src/plugins/dispatch.ts",
      "stop condition: handback is readable",
      "lane: delivery",
      "evidence required: source: CLI regression",
      "pane: w1:pA",
      "target session: none",
      "held by: none",
    ];

    const singular = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(singular).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: [...expected, `handback: ${handback}`].join("\n") + "\n",
    });

    insertHandback(fixture, {
      claim: "second shown claim",
      dispatch,
      id: "h2",
      status: "UNKNOWN",
    });
    const plural = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(plural).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: [...expected, `handbacks: ${handback}, h2`].join("\n") + "\n",
    });
  });
});

test("402 handback list scopes by dispatch or work and renders status with claim head", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "list handbacks", "--atomic-reason", "fixture"]),
    );
    const firstDispatch = await openDispatch(fixture, work);
    const secondDispatch = await openDispatch(fixture, work);
    const first = await fileHandback(fixture, firstDispatch, "first claim\nhidden detail");
    const second = await fileHandback(fixture, secondDispatch, "second claim", "BLOCKED");

    const dispatchList = await runCli(fixture, ["handback", "list", firstDispatch]);
    expect(dispatchList).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: `${first} [DONE] first claim\n`,
    });

    const workList = await runCli(fixture, ["handback", "list", work]);
    expect(workList).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: `${first} [DONE] first claim\n${second} [BLOCKED] second claim\n`,
    });

    const help = await runCli(fixture, ["help", "handback"]);
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("handback list <dispatch-or-work-id>");
  });
});
