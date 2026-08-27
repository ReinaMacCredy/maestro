import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type CliResult, type Fixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

function dispatchId(result: CliResult): string {
  const id = result.stdout.match(/^(x\d+) \[open\]/)?.[1];
  if (!id) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return id;
}

function dispatchOpenArgs(work: string, lane: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "produce comparison evidence",
    "--owned-scope",
    "read-only investigation",
    "--excluded-scope",
    "candidate selection and writes",
    "--mutation",
    "no-write",
    "--stop-condition",
    "evidence returned",
    "--lane",
    lane,
    "--evidence-required",
    "source: CLI readback",
    "--pane",
    "w1:p-shadow",
  ];
}

async function addWork(fixture: Fixture, title: string): Promise<string> {
  return idFrom(
    await runCli(fixture, ["work", "add", title, "--atomic-reason", "paseo slice fixture"]),
  );
}

function handbackFileArgs(dispatch: string, status: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    status,
    "--claim",
    status === "COUNCIL_REQUEST" ? "the assignment needs a council" : "the lane is complete",
    "--proof",
    "source: CLI readback",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

async function openAcceptedDispatch(
  fixture: Fixture,
  title: string,
  holder: string,
): Promise<{ dispatch: string; work: string }> {
  const work = await addWork(fixture, title);
  const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work, "delivery")));
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode).toBe(
    0,
  );
  return { dispatch, work };
}

test("380 dispatch accepts and renders the shadow lane in open, show, list, and the brief", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "shadow lane readback");
    const opened = await runCli(fixture, dispatchOpenArgs(work, "shadow"));
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain("lane: shadow");
    const dispatch = dispatchId(opened);

    const shown = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("lane: shadow");
    const listed = await runCli(fixture, ["dispatch", "list", work]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain(`| ${dispatch} | shadow |`);

    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("shadow-brief"),
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      "lane (scout no-write | decision x2-3 | delivery | challenge | shadow no-write)",
    );
  });
});

test("381 dispatch rejects an unknown lane with all five valid names", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "unknown lane rejection");
    const rejected = await runCli(fixture, dispatchOpenArgs(work, "observer"));
    expect(rejected.exitCode).not.toBe(0);
    expect(rejected.stderr).toContain("INVALID_LANE");
    expect(rejected.stderr).toContain(
      "expected one of: scout, decision, delivery, challenge, shadow",
    );
  });
});

test("382 accepting a shadow dispatch never takes the work write lease", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "shadow lane lease boundary");
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work, "shadow")));
    const accepted = await runCli(
      fixture,
      ["dispatch", "accept", dispatch],
      session("shadow-holder"),
    );
    expect(accepted.exitCode).toBe(0);
    expect(accepted.stdout).toContain("held by: shadow-holder");

    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(shown.exitCode).toBe(0);
    const envelope = JSON.parse(shown.stdout) as { data: { work: { heldBy: string | null } } };
    expect(envelope.data.work.heldBy).toBeNull();
  });
});

test("383 SLP maps all five maestro lanes to Paseo dispositions and shadow evidence semantics", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const peer = recipe.match(/### Peer\n([\s\S]*?)\n## Topology invariants/)?.[1] ?? "";
  for (const row of [
    "| scout | Scout | no |",
    "| decision | Architect | no |",
    "| delivery | Engineer/Owner | yes, one owner per scope |",
    "| challenge | Reviewer | no |",
    "| shadow | Shadow | no, evidence only |",
  ]) {
    expect(peer).toContain(row);
  }
  expect(peer).toMatch(/shadow.*comparison evidence/i);
  expect(peer).toMatch(/never.*candidate/i);
});

test("384 lane.md step 5 and the SLP role binding list all five lane types", async () => {
  const room = (
    await readFile(join(import.meta.dir, "..", "src", "plugins", "room.ts"), "utf8")
  ).replace(/\\`/g, "`");
  const stepFive = room.match(/^5\. .*$/m)?.[0] ?? "";
  for (const lane of ["scout", "decision", "delivery", "challenge", "shadow"]) {
    expect(stepFive).toContain(lane);
  }

  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const binding = recipe.match(/\| a pane the Lead opened with a dispatch \|.*$/m)?.[0] ?? "";
  for (const lane of ["scout", "decision", "delivery", "challenge", "shadow"]) {
    expect(binding).toContain(lane);
  }
});

test("385 handback files and shows COUNCIL_REQUEST", async () => {
  await withFixture(async (fixture) => {
    const holder = "council-request-peer";
    const { dispatch } = await openAcceptedDispatch(fixture, "council request readback", holder);
    const filed = await runCli(
      fixture,
      handbackFileArgs(dispatch, "COUNCIL_REQUEST"),
      session(holder),
    );
    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain("[COUNCIL_REQUEST]");
    const handback = filed.stdout.match(/^(h\d+) \[COUNCIL_REQUEST\]/)?.[1];
    expect(handback).toBeDefined();

    const shown = await runCli(fixture, ["handback", "show", handback as string]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("[COUNCIL_REQUEST]");
    expect(shown.stdout).toContain("claim: the assignment needs a council");
  });
});

test("386 handback rejects an unknown status with all nine valid names", async () => {
  await withFixture(async (fixture) => {
    const holder = "unknown-status-peer";
    const { dispatch } = await openAcceptedDispatch(fixture, "unknown status rejection", holder);
    const rejected = await runCli(fixture, handbackFileArgs(dispatch, "PASS"), session(holder));
    expect(rejected.exitCode).not.toBe(0);
    expect(rejected.stderr).toContain("INVALID_STATUS");
    expect(rejected.stderr).toContain(
      "expected one of: DONE, BLOCKED, UNTESTABLE, UNKNOWN, FAILED, CHALLENGE, REOPEN_REQUEST, DEPENDENCY_REQUEST, COUNCIL_REQUEST",
    );
  });
});

test("387 an eight-status store migrates in place and preserves prior handbacks", async () => {
  await withFixture(async (fixture) => {
    const firstHolder = "legacy-done-peer";
    const first = await openAcceptedDispatch(fixture, "preserved legacy handback", firstHolder);
    const legacyFiled = await runCli(
      fixture,
      handbackFileArgs(first.dispatch, "DONE"),
      session(firstHolder),
    );
    expect(legacyFiled.exitCode).toBe(0);
    const legacyHandback = legacyFiled.stdout.match(/^(h\d+) \[DONE\]/)?.[1] as string;

    const secondHolder = "migrated-council-peer";
    const second = await openAcceptedDispatch(fixture, "migrated council request", secondHolder);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.exec(`
      ALTER TABLE handbacks RENAME TO handbacks_current_vocabulary;
      CREATE TABLE handbacks (
        id TEXT PRIMARY KEY,
        dispatch_id TEXT NOT NULL REFERENCES dispatches(id),
        status TEXT NOT NULL CHECK(status IN (
          'DONE', 'BLOCKED', 'UNTESTABLE', 'UNKNOWN', 'FAILED', 'CHALLENGE',
          'REOPEN_REQUEST', 'DEPENDENCY_REQUEST'
        )),
        claim TEXT NOT NULL,
        proof TEXT NOT NULL,
        assumptions TEXT NOT NULL,
        residual_risks TEXT NOT NULL,
        incidental_findings TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      INSERT INTO handbacks SELECT * FROM handbacks_current_vocabulary;
      DROP TABLE handbacks_current_vocabulary;
      CREATE INDEX handbacks_dispatch_id ON handbacks(dispatch_id);
    `);
    const legacySchema = database
      .query<{ sql: string }, []>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'handbacks'",
      )
      .get()?.sql;
    expect(legacySchema).not.toContain("COUNCIL_REQUEST");
    database.close();

    const filed = await runCli(
      fixture,
      handbackFileArgs(second.dispatch, "COUNCIL_REQUEST"),
      session(secondHolder),
    );
    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain("[COUNCIL_REQUEST]");
    const preserved = await runCli(fixture, ["handback", "show", legacyHandback]);
    expect(preserved.exitCode).toBe(0);
    expect(preserved.stdout).toContain("[DONE]");
  });
});

test("388 COUNCIL_REQUEST raises HANDBACK_UNREVIEWED and names the status", async () => {
  await withFixture(async (fixture) => {
    const holder = "attention-council-peer";
    const { dispatch } = await openAcceptedDispatch(fixture, "unreviewed council request", holder);
    const filed = await runCli(
      fixture,
      handbackFileArgs(dispatch, "COUNCIL_REQUEST"),
      session(holder),
    );
    expect(filed.exitCode).toBe(0);
    const handback = filed.stdout.match(/^(h\d+) \[COUNCIL_REQUEST\]/)?.[1] as string;

    const attention = await runCli(fixture, ["attention", "--json"], session("lead-session"));
    expect(attention.exitCode).toBe(0);
    const envelope = JSON.parse(attention.stdout) as {
      data: { detections: Array<{ kind: string; packet: string }> };
    };
    const unreviewed = envelope.data.detections.filter(
      (detection) => detection.kind === "HANDBACK_UNREVIEWED",
    );
    expect(unreviewed).toHaveLength(1);
    expect(unreviewed[0]?.packet).toContain(
      `${dispatch} returned COUNCIL_REQUEST (${handback})`,
    );
  });
});

test("389 policy-dispatch accepts COUNCIL_REQUEST and SLP documents the Lead response", async () => {
  await withFixture(async (fixture) => {
    const holder = "policy-council-peer";
    const { dispatch, work } = await openAcceptedDispatch(
      fixture,
      "policy council request",
      holder,
    );
    expect(
      (
        await runCli(
          fixture,
          handbackFileArgs(dispatch, "COUNCIL_REQUEST"),
          session(holder),
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "start", work], session("lead-session"))).exitCode).toBe(
      0,
    );
  });

  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const cross = recipe.split("## Cross-examination")[1]?.split("\n## ")[0] ?? "";
  expect(cross).toContain("COUNCIL_REQUEST");
  expect(cross).toContain("d688");
  expect(cross).toMatch(/second generation/i);
  expect(cross).toMatch(/declin.*work note/i);
});
