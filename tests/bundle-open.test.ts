import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { installFakeHerdr } from "./helpers-herdr.ts";
import { type Fixture, idFrom, initializeGitRepository, runCli, runCliAt, withFixture } from "./helpers.ts";

// Hub finding F4 (w734): bundle open --work checked the non-SLP work table
// only, so an id the project store holds as an slp_work row - every id an SLP
// generation issues - failed NOT_FOUND. An id in either table is accepted; the
// stored bundle_work link keeps its shape.

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

test("bundle open --work accepts an SLP work id and still refuses an unknown one (Hub finding F4)", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const room = await scaffoldRoom(fixture.home);
    expect(
      (await runCliAt(fixture, room, ["room", "mark"], { MAESTRO_ROOM_SCAFFOLD: "1", MAESTRO_SESSION_NONE: "1" }))
        .exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Bundle link", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const slpWorkId = envelope<{ work: { id: string } }>(started.stdout).work.id;

    const opened = await runCli(fixture, ["bundle", "open", "slp-linked", "--work", slpWorkId, "--json"]);
    expect(opened.stderr).toBe("");
    expect(opened.exitCode).toBe(0);
    expect(envelope<{ workIds: string[] }>(opened.stdout).workIds).toEqual([slpWorkId]);

    const missing = await runCli(fixture, ["bundle", "open", "absent-link", "--work", "w9999", "--json"]);
    expect(missing.exitCode).toBe(1);
    expect(failure(missing.stderr).code).toBe("NOT_FOUND");
  });
}, 30_000);

test("bundle open migrates a legacy bundle_work away from the work(id) foreign key, keeping its rows (Hub finding F4)", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const added = await runCli(fixture, ["work", "add", "plain item", "--json"]);
    expect(added.exitCode).toBe(0);
    const workId = envelope<{ work: { id: string } }>(added.stdout).work.id;
    expect((await runCli(fixture, ["bundle", "open", "legacy-link", "--work", workId])).exitCode).toBe(0);

    // Put the pre-F4 schema back, rows and all, so the migration has something
    // to rebuild the way a store written before this change holds it.
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(databasePath);
    database.exec("PRAGMA foreign_keys = OFF");
    database.exec("ALTER TABLE bundle_work RENAME TO bundle_work_old");
    database.exec(
      `CREATE TABLE bundle_work (
         bundle_id TEXT NOT NULL REFERENCES bundles(id),
         work_id TEXT NOT NULL REFERENCES work(id),
         PRIMARY KEY(bundle_id, work_id)
       )`,
    );
    database.exec("INSERT INTO bundle_work SELECT * FROM bundle_work_old");
    database.exec("DROP TABLE bundle_work_old");
    expect(
      database.query<{ sql: string }, []>("SELECT sql FROM sqlite_master WHERE name = 'bundle_work'").get()!.sql,
    ).toContain("REFERENCES work(id)");
    database.close();

    const shown = await runCli(fixture, ["bundle", "show", "legacy-link", "--json"]);
    expect(shown.exitCode).toBe(0);

    database = new Database(databasePath, { readonly: true });
    const schema = database
      .query<{ sql: string }, []>("SELECT sql FROM sqlite_master WHERE name = 'bundle_work'")
      .get()!.sql;
    const rows = database.query<{ bundle_id: string; work_id: string }, []>("SELECT * FROM bundle_work").all();
    database.close();
    expect(schema).not.toContain("REFERENCES work(id)");
    expect(schema).toContain("REFERENCES bundles(id)");
    expect(schema).toContain("PRIMARY KEY(bundle_id, work_id)");
    expect(rows).toEqual([{ bundle_id: "legacy-link", work_id: workId }]);
  });
}, 30_000);

const planSpec = `# SPEC plan

## Problem

## Units

### U1: Routing table

Depends on: none
Files: edit src/plugins/resources/WORKFLOW.md
Build: replace the Routing section with the task-type table
You will see: \`maestro recipe show bug-fix\` prints six steps

### U2: Drop the old skill

Depends on: U1
Files:
- delete src/plugins/skills/old/SKILL.md
- edit src/plugins/install.ts
Build: stop installing the old skill
You will see: install no longer links the old skill

## Derived artifacts
`;

const planVerify = `# VERIFY plan

| # | Scenario | unit | live | perf | Commit |
|---|----------|------|------|------|--------|
| 1 | U1 routing table | docs-contract 612 pass | verify-maestro run-1 transcript | n/a: text only | abc1234 |
| 2 | U2 old skill gone | install test pass | verify-maestro run-2 transcript | n/a: no hot path | def5678 |
| 3 | anti-goal no third coordinator | rg count 0 | n/a: grep is the check | n/a: text only | def5678 |
`;

async function openPlan(fixture: Fixture, id: string, spec?: string, verify?: string): Promise<string> {
  const opened = await runCli(fixture, ["bundle", "open", id, "--json"]);
  expect(opened.exitCode).toBe(0);
  const directory = envelope<{ bundle: { directory: string } }>(opened.stdout).bundle.directory;
  if (spec !== undefined) await writeFile(join(directory, "SPEC.md"), spec);
  if (verify !== undefined) await writeFile(join(directory, "VERIFY.md"), verify);
  return directory;
}

function problems(stderr: string): string[] {
  return (JSON.parse(stderr) as { error: { problems: string[] } }).error.problems;
}

test("bundle check fails only on a SPEC unit with no VERIFY row, matching the whole unit id (d903)", async () => {
  await withFixture(async (fixture) => {
    const looseSpec = planSpec.replace(/\nDepends on: none\n/, "\n").replace(/\nFiles: [^\n]*\n/, "\n");
    const looseVerify = `# VERIFY plan

| Scenario | Result |
|----------|--------|
| U1 routing table | n/a |
| U21 old skill gone | |
`;
    const directory = await openPlan(fixture, "plan", looseSpec, looseVerify);
    const checked = await runCli(fixture, ["bundle", "check", "plan"]);
    expect(checked.exitCode).toBe(1);
    expect(problems(checked.stderr)).toEqual([`${join(directory, "VERIFY.md")} U2 is missing a row`]);

    await writeFile(join(directory, "VERIFY.md"), looseVerify.replace("U21", "U2"));
    const passed = await runCli(fixture, ["bundle", "check", "plan"]);
    expect(passed.stderr).toBe("");
    expect(passed.stdout).toBe("bundle check plan: OK (2 units, 2 VERIFY rows)\n");
  });
}, 30_000);

test("bundle check --final also fails a VERIFY row with no result (d903)", async () => {
  await withFixture(async (fixture) => {
    const directory = await openPlan(
      fixture,
      "plan",
      planSpec,
      planVerify
        .replace("| verify-maestro run-1 transcript |", "|  |")
        .replace("| n/a: text only | def5678 |", "| n/a: text only |  |"),
    );
    const plan = await runCli(fixture, ["bundle", "check", "plan"]);
    expect(plan.stderr).toBe("");
    expect(plan.exitCode).toBe(0);
    const final = await runCli(fixture, ["bundle", "check", "plan", "--final"]);
    expect(final.exitCode).toBe(1);
    expect(problems(final.stderr)).toEqual([`${join(directory, "VERIFY.md")}:7 row has no result`]);
  });
}, 30_000);

test("bundle check prints one OK line for a clean plan, before and after verify (d866)", async () => {
  await withFixture(async (fixture) => {
    await openPlan(fixture, "plan", planSpec, planVerify);
    const plan = await runCli(fixture, ["bundle", "check", "plan"]);
    expect(plan.stderr).toBe("");
    expect(plan.exitCode).toBe(0);
    expect(plan.stdout).toBe("bundle check plan: OK (2 units, 3 VERIFY rows)\n");
    const final = await runCli(fixture, ["bundle", "check", "plan", "--final"]);
    expect(final.stderr).toBe("");
    expect(final.exitCode).toBe(0);
    expect(final.stdout).toBe("bundle check plan --final: OK (2 units, 3 VERIFY rows)\n");
  });
}, 30_000);

test("a bundle stored as paused still appears in bundle list and bundle show (d889)", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["bundle", "open", "slice"])).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      database.run("UPDATE bundles SET paused_at = '2026-01-01T00:00:00.000Z' WHERE id = 'slice'");
    } finally {
      database.close();
    }

    const listed = await runCli(fixture, ["bundle", "list"]);
    const shown = await runCli(fixture, ["bundle", "show", "slice"]);

    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("slice [paused]");
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("slice [paused]");
  });
});

test("work done on the last open linked card archives the bundle; a cancelled card does not count (d895)", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "feature", "--kind", "feature"]));
    const ids: string[] = [];
    for (const title of ["first", "second", "dropped"]) {
      const added = await runCli(fixture, ["work", "add", title, "--parent", parent]);
      expect(added.exitCode).toBe(0);
      ids.push(idFrom(added));
    }
    const [w1, w2, w3] = ids as [string, string, string];
    expect((await runCli(fixture, ["bundle", "open", "plan", "--work", w1, "--work", w2, "--work", w3])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "cancel", w3, "--reason", "not needed"])).exitCode).toBe(0);

    const one = await runCli(fixture, ["work", "done", w1, "--evidence", "first done"]);
    expect(one.exitCode).toBe(0);
    expect(one.stdout).toBe(`${w1} done\n`);
    expect((await runCli(fixture, ["bundle", "list"])).stdout).toContain("plan [active]");

    const last = await runCli(fixture, ["work", "done", w2, "--evidence", "second done"]);
    expect(last.stderr).toBe("");
    expect(last.stdout).toBe(`${w2} done\nbundle plan archived; 1 VERIFY row has no result\n`);
    const shown = await runCli(fixture, ["bundle", "show", "plan"]);
    const [heading, archive] = shown.stdout.split("\n");
    expect(heading).toStartWith("plan [archived] ");
    expect(archive).toBe(`archived when ${w2} was done; 1 VERIFY row had no result`);
    expect(shown.stdout).toContain("--- VERIFY.md\n# VERIFY");
  });
}, 30_000);

test("a filled VERIFY table archives without a count, and bundle close stays the manual path (d895)", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "feature", "--kind", "feature"]));
    const workId = idFrom(await runCli(fixture, ["work", "add", "only", "--parent", parent]));
    const opened = await runCli(fixture, ["bundle", "open", "plan", "--work", workId, "--json"]);
    const directory = envelope<{ bundle: { directory: string } }>(opened.stdout).bundle.directory;
    await writeFile(join(directory, "VERIFY.md"), planVerify);
    const done = await runCli(fixture, ["work", "done", workId, "--evidence", "done"]);
    expect(done.stdout).toBe(`${workId} done\nbundle plan archived\n`);
    expect((await runCli(fixture, ["bundle", "show", "plan"])).stdout).not.toContain("had no result");

    expect((await runCli(fixture, ["bundle", "open", "manual"])).exitCode).toBe(0);
    const closed = await runCli(fixture, ["bundle", "close", "manual"]);
    expect(closed.exitCode).toBe(0);
    expect(closed.stdout).toContain("manual [archived]");
  });
}, 30_000);

test("bundle list shows active bundles, most recently updated first, and counts the archived ones; --all lists every bundle (d895)", async () => {
  await withFixture(async (fixture) => {
    for (const id of ["older", "newer", "gone"]) {
      expect((await runCli(fixture, ["bundle", "open", id])).exitCode).toBe(0);
    }
    expect((await runCli(fixture, ["bundle", "close", "gone"])).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      database.run("UPDATE bundles SET updated_at = '2026-01-01T00:00:00.000Z' WHERE id = 'older'");
    } finally {
      database.close();
    }

    const listed = await runCli(fixture, ["bundle", "list"]);
    expect(listed.stdout.split("\n").map((line) => line.split(" ").slice(0, 2).join(" "))).toEqual([
      "newer [active]",
      "older [active]",
      "1 archived",
      "",
    ]);
    expect(listed.stdout).toContain("\n1 archived not shown; maestro bundle list --all\n");
    const all = await runCli(fixture, ["bundle", "list", "--all"]);
    expect(all.stdout).toContain("gone [archived]");
    expect(all.stdout).not.toContain("not shown");
    const json = envelope<{ bundles: Array<{ id: string }>; hidden: number }>(
      (await runCli(fixture, ["bundle", "list", "--json"])).stdout,
    );
    expect(json.bundles.map((bundle) => bundle.id)).toEqual(["newer", "older"]);
    expect(json.hidden).toBe(1);
  });
}, 30_000);
