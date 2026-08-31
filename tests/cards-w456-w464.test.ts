import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
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

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function packetHeads(result: { stdout: string }): Map<string, string> {
  const detections = (JSON.parse(result.stdout) as {
    data: { detections: Array<{ kind: string; packet: string }> };
  }).data.detections;
  return new Map(
    detections.map(({ kind, packet }) => [kind, packet.split("\n")[0] ?? ""]),
  );
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
  expect(
    (
      await runCli(fixture, [
        "dispatch",
        "confirm",
        dispatch,
        "--session",
        "test-session",
      ])
    ).exitCode,
  ).toBe(0);
  const filed = await runCli(fixture, [
    "handback",
    "file",
    dispatch,
    "--status",
    status,
    ...(status === "BLOCKED" ? ["--request", "the blocker is resolved"] : []),
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

test("400 handback show accepts a dispatch id and resolves its one handback", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "latest handback", "--atomic-reason", "fixture"]),
    );
    const dispatch = await openDispatch(fixture, work);
    const first = await fileHandback(fixture, dispatch, "the claim\nprivate detail");

    const byDispatch = await runCli(fixture, ["handback", "show", dispatch]);
    expect(byDispatch.exitCode).toBe(0);
    expect(byDispatch.stdout).toStartWith(`${first} [DONE]\n`);
    expect(byDispatch.stdout).toContain("claim: the claim\nprivate detail\n");

    const byHandback = await runCli(fixture, ["handback", "show", first]);
    expect(byHandback.exitCode).toBe(0);
    expect(byHandback.stdout).toStartWith(`${first} [DONE]\n`);
  });
});

test("401 dispatch show pins its handback line", async () => {
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
      "opened by: test-session",
      "claimed by: none",
      "held by: none",
    ];

    const singular = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(singular).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: [...expected, `handback: ${handback}`].join("\n") + "\n",
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

test("403 uninstall removes only the repository's absolute registry line", async () => {
  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"]);
    expect(installed.exitCode).toBe(0);
    const registry = join(fixture.home, "maestro", "registry");
    const otherRepo = join(fixture.root, "other-repo");
    await writeFile(registry, `${fixture.repo}\n${otherRepo}\n`);

    const uninstalled = await runCli(fixture, ["uninstall"]);
    expect(uninstalled.exitCode).toBe(0);
    expect(await readFile(registry, "utf8")).toBe(`${otherRepo}\n`);
  });
});

test("404 brief skips registered paths that are absent or have no .maestro", async () => {
  await withFixture(async (fixture) => {
    const missingRepo = join(fixture.root, "missing-repo");
    const bareRepo = join(fixture.root, "bare-repo");
    await mkdir(bareRepo, { recursive: true });
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "registry"),
      `${missingRepo}\n${bareRepo}\n`,
    );

    const brief = await runCli(fixture, ["brief"]);
    expect(brief).toEqual({
      exitCode: 0,
      stderr: "",
      stdout:
        `Needs attention:\nskipped: ${missingRepo} (missing)\n` +
        `skipped: ${bareRepo} (missing)\n`,
    });
  });
});

test("405 room forget removes a registry line without uninstalling repository wiring", async () => {
  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"]);
    expect(installed.exitCode).toBe(0);
    const hook = join(fixture.repo, ".codex", "hooks", "maestro-record.ts");
    expect(existsSync(hook)).toBe(true);

    const forgotten = await runCli(fixture, ["room", "forget", fixture.repo]);
    expect(forgotten).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: `forgot: ${fixture.repo}\n`,
    });
    expect(await readFile(join(fixture.home, "maestro", "registry"), "utf8")).toBe("");
    expect(existsSync(hook)).toBe(true);

    const help = await runCli(fixture, ["help", "room"]);
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("room forget <path>");
  });
});

test("407 attention and briefs prefix work subjects with their kind", async () => {
  await withFixture(async (fixture) => {
    const stalled = idFrom(
      await runCli(fixture, ["work", "add", "stalled work", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", stalled], session("stalled-holder"))).exitCode)
      .toBe(0);

    const repeated = idFrom(
      await runCli(fixture, ["work", "add", "repeated work", "--atomic-reason", "fixture"]),
    );
    expect(
      (await runCli(fixture, ["work", "start", repeated], session("repeated-holder"))).exitCode,
    ).toBe(0);
    for (const note of ["failed: first", "failed: second", "failed: third"]) {
      expect(
        (await runCli(fixture, ["work", "note", repeated, note], session("repeated-holder")))
          .exitCode,
      ).toBe(0);
    }

    const parent = idFrom(
      await runCli(fixture, ["work", "add", "collision parent", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", parent], session("parent-holder"))).exitCode)
      .toBe(0);
    const first = idFrom(
      await runCli(fixture, ["work", "add", "first collision", "--parent", parent]),
    );
    const second = idFrom(
      await runCli(fixture, ["work", "add", "second collision", "--parent", parent]),
    );
    expect((await runCli(fixture, ["work", "start", first], session("first-holder"))).exitCode)
      .toBe(0);
    expect((await runCli(fixture, ["work", "start", second], session("second-holder"))).exitCode)
      .toBe(0);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 31 * 60_000).toISOString(), "stalled-holder");
    database.close();

    const attention = await runCli(fixture, ["attention", "--json"]);
    expect(attention.exitCode).toBe(0);
    const heads = packetHeads(attention);
    expect(heads.get("STALLED_LEASE")).toBe(`attention STALLED_LEASE work ${stalled}`);
    expect(heads.get("REPEATED_FAILURE")).toBe(`attention REPEATED_FAILURE work ${repeated}`);
    expect(heads.get("SCOPE_COLLISION")).toBe(
      `attention SCOPE_COLLISION work ${first},${second}`,
    );

    const hook = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"]);
    expect(hook.stdout).toContain(`attention STALLED_LEASE work ${stalled}`);
    expect(hook.stdout).toContain(`attention SCOPE_COLLISION work ${first},${second}`);

    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const brief = await runCli(fixture, ["brief"], { MAESTRO_READ_ONLY: "1" });
    expect(brief.stdout).toContain(`attention REPEATED_FAILURE work ${repeated}`);
  });
});

test("408 attention and hook briefs prefix dispatch subjects with their kind", async () => {
  await withFixture(async (fixture) => {
    const unacceptedWork = idFrom(
      await runCli(fixture, ["work", "add", "unaccepted", "--atomic-reason", "fixture"]),
    );
    const unaccepted = await openDispatch(fixture, unacceptedWork);
    const unreturnedWork = idFrom(
      await runCli(fixture, ["work", "add", "unreturned", "--atomic-reason", "fixture"]),
    );
    const unreturned = await openDispatch(fixture, unreturnedWork);
    expect((await runCli(fixture, ["dispatch", "accept", unreturned])).exitCode).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "confirm",
          unreturned,
          "--session",
          "test-session",
        ])
      ).exitCode,
    ).toBe(0);
    const returnedWork = idFrom(
      await runCli(fixture, ["work", "add", "returned", "--atomic-reason", "fixture"]),
    );
    const returned = await openDispatch(fixture, returnedWork);
    await fileHandback(fixture, returned, "returned claim");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 11 * 60_000).toISOString(), unaccepted);
    database.query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 3 * 60 * 60_000).toISOString(), unreturned);
    database.close();

    const attention = await runCli(fixture, ["attention", "--json"]);
    expect(attention.exitCode).toBe(0);
    const heads = packetHeads(attention);
    expect(heads.get("DISPATCH_UNACCEPTED")).toBe(
      `attention DISPATCH_UNACCEPTED dispatch ${unaccepted}`,
    );
    expect(heads.get("DISPATCH_UNRETURNED")).toBe(
      `attention DISPATCH_UNRETURNED dispatch ${unreturned}`,
    );
    expect(heads.get("HANDBACK_UNREVIEWED")).toBe(
      `attention HANDBACK_UNREVIEWED dispatch ${returned}`,
    );

    const hook = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"]);
    for (const line of [
      `attention DISPATCH_UNACCEPTED dispatch ${unaccepted}`,
      `attention DISPATCH_UNRETURNED dispatch ${unreturned}`,
      `attention HANDBACK_UNREVIEWED dispatch ${returned}`,
    ]) {
      expect(hook.stdout).toContain(line);
    }
  });
});

test("409 attention and hook briefs prefix decision subjects with their kind", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "stale decision", "--atomic-reason", "fixture"]),
    );
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "choose the boundary", "--work", work]),
    );
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE decisions SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    database.close();

    const attention = await runCli(fixture, ["attention", "--json"]);
    expect(attention.exitCode).toBe(0);
    expect(packetHeads(attention).get("DECISION_STALE")).toBe(
      `attention DECISION_STALE decision ${decision}`,
    );

    const hook = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"]);
    expect(hook.stdout).toContain(`attention DECISION_STALE decision ${decision}`);
  });
});

test("476 the store refuses a second handback on one dispatch, not just the command (w475, d706)", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "one return", "--atomic-reason", "fixture"]),
    );
    const dispatch = await openDispatch(fixture, work);
    await fileHandback(fixture, dispatch, "the only view");

    // The command guard is not the invariant; the store is. Bypass the command.
    expect(() =>
      insertHandback(fixture, {
        claim: "a second view",
        dispatch,
        id: "h2",
        status: "DONE",
      })
    ).toThrow(/UNIQUE|constraint/i);

    const listed = await runCli(fixture, ["handback", "list", dispatch]);
    expect(listed.stdout).not.toContain("h2");
  });
});

test("477 a store that already holds duplicate handbacks keeps the first and migrates (w475, d706)", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "legacy duplicates", "--atomic-reason", "fixture"]),
    );
    const dispatch = await openDispatch(fixture, work);
    const first = await fileHandback(fixture, dispatch, "the first view");

    const path = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(path);
    database.exec("DROP INDEX IF EXISTS handbacks_dispatch_id");
    database.exec("CREATE INDEX handbacks_dispatch_id ON handbacks(dispatch_id)");
    database
      .query(
        `INSERT INTO handbacks
          (id, dispatch_id, status, claim, proof, assumptions, residual_risks,
           incidental_findings, created_at)
         VALUES ('h9', ?, 'DONE', 'a later duplicate', 'p', 'None', 'None', 'None', ?)`,
      )
      .run(dispatch, new Date(Date.now() + 60_000).toISOString());
    database.close();

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);

    database = new Database(path, { readonly: true });
    const survivors = database
      .query<{ id: string }, [string]>("SELECT id FROM handbacks WHERE dispatch_id = ?")
      .all(dispatch)
      .map((row) => row.id);
    const unique = database
      .query<{ sql: string | null }, []>(
        "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'handbacks_dispatch_id'",
      )
      .get()?.sql ?? "";
    database.close();
    expect(survivors).toEqual([first]);
    expect(unique).toMatch(/UNIQUE/i);
  });
});
