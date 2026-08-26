import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchOpenArgs(work: string, targetSession: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return the delivery result",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, publish",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the handback is filed",
    "--lane",
    "delivery",
    "--evidence-required",
    "source and live",
    "--target-session",
    targetSession,
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

function handbackFileArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the delivery result is returned",
    "--proof",
    "source: focused test passes",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

async function acceptedDispatch(
  fixture: Fixture,
  work: string,
  holder: string,
): Promise<string> {
  const opened = await runCli(fixture, dispatchOpenArgs(work, holder));
  expect(opened.exitCode).toBe(0);
  const dispatch = dispatchId(opened);
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode)
    .toBe(0);
  return dispatch;
}

test("187 filing a handback releases the filing session's work lease without losing work", async () => {
  await withFixture(async (fixture) => {
    const holder = "filing-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "return delivery", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "note", work, "keep this note"], session(holder))).exitCode,
    ).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE work SET evidence = ? WHERE id = ?").run("source: existing proof", work);
    database.close();
    const dispatch = await acceptedDispatch(fixture, work, holder);

    const filed = await runCli(fixture, handbackFileArgs(dispatch), session(holder));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(shown.exitCode).toBe(0);
    const envelope = JSON.parse(shown.stdout) as {
      data: {
        notes: Array<{ text: string }>;
        work: { evidence: string | null; heldBy: string | null; state: string };
      };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({
        evidence: "source: existing proof",
        heldBy: null,
        state: "open",
      }),
    );
    expect(envelope.data.notes.map((note) => note.text)).toContain("keep this note");
  });
});

test("188 a non-holder handback leaves another session's work lease alone", async () => {
  await withFixture(async (fixture) => {
    const workHolder = "work-holder";
    const filingSession = "filing-non-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "preserve lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(workHolder))).exitCode).toBe(0);
    const dispatch = await acceptedDispatch(fixture, work, filingSession);

    const filed = await runCli(fixture, handbackFileArgs(dispatch), session(filingSession));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    const envelope = JSON.parse(shown.stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({ heldBy: workHolder, state: "active" }),
    );
  });
});
