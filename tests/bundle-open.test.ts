import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { installFakeHerdr } from "./helpers-herdr.ts";
import { type Fixture, initializeGitRepository, runCli, runCliAt, withFixture } from "./helpers.ts";

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
    const added = await runCli(fixture, ["work", "add", "plain item", "--atomic-reason", "fixture", "--json"]);
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

test("bundle check names a scaffolded unit missing its You will see line (d866)", async () => {
  await withFixture(async (fixture) => {
    const directory = await openPlan(fixture, "fresh-plan");
    const spec = await Bun.file(join(directory, "SPEC.md")).text();
    const verify = await Bun.file(join(directory, "VERIFY.md")).text();
    expect(spec.indexOf("## Decisions")).toBeLessThan(spec.indexOf("## Units"));
    expect(spec.indexOf("## Units")).toBeLessThan(spec.indexOf("## Derived artifacts"));
    const specLines = spec.split("\n");
    for (const line of ["### U1: <title>", "Depends on:", "Files:", "Build:", "You will see:"]) {
      expect(specLines).toContain(line);
    }
    expect(verify.split("\n")).toContain("| # | Scenario | unit | live | perf | Commit |");
    expect(verify).toContain("n/a: <reason>");

    await writeFile(
      join(directory, "SPEC.md"),
      spec
        .replace("\nDepends on:\n", "\nDepends on: none\n")
        .replace("\nFiles:\n", "\nFiles: edit src/plugins/bundle.ts\n")
        .replace("\nBuild:\n", "\nBuild: add the check verb\n"),
    );
    const youWillSee = specLines.indexOf("You will see:") + 1;
    const checked = await runCli(fixture, ["bundle", "check", "fresh-plan"]);
    expect(checked.exitCode).toBe(1);
    expect(problems(checked.stderr)).toEqual([
      `${join(directory, "SPEC.md")}:${youWillSee} U1 is missing "You will see:"`,
    ]);
  });
}, 30_000);

test("bundle check names a unit with no VERIFY row, matching the whole unit id (d866)", async () => {
  await withFixture(async (fixture) => {
    const directory = await openPlan(
      fixture,
      "plan",
      planSpec,
      planVerify.replace("| U2 old skill gone |", "| U21 old skill gone |"),
    );
    const checked = await runCli(fixture, ["bundle", "check", "plan"]);
    expect(checked.exitCode).toBe(1);
    expect(problems(checked.stderr)).toEqual([`${join(directory, "VERIFY.md")}:3 U2 is missing a row`]);
  });
}, 30_000);

test("bundle check names a bare n/a cell (d866)", async () => {
  await withFixture(async (fixture) => {
    const directory = await openPlan(
      fixture,
      "plan",
      planSpec,
      planVerify.replace("| n/a: no hot path |", "| n/a |"),
    );
    const checked = await runCli(fixture, ["bundle", "check", "plan"]);
    expect(checked.exitCode).toBe(1);
    expect(problems(checked.stderr)).toEqual([
      `${join(directory, "VERIFY.md")}:6 perf cell n/a is missing its reason`,
    ]);
  });
}, 30_000);

test("bundle check --final names empty evidence and Commit cells the default mode accepts (d866)", async () => {
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
    expect(problems(final.stderr)).toEqual([
      `${join(directory, "VERIFY.md")}:5 live cell is missing evidence or n/a: <reason>`,
      `${join(directory, "VERIFY.md")}:7 Commit cell is missing a commit`,
    ]);
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

test("bundle check passes a bundle with no Units section, active or archived, and --final names it (d866)", async () => {
  await withFixture(async (fixture) => {
    const oldSpec = "# SPEC old\n\n## Problem\n\n## Decisions\n";
    const oldVerify = "# VERIFY old\n\n| # | Scenario | Work / acceptance | Result |\n|---|----------|-------------------|--------|\n";
    const directory = await openPlan(fixture, "old-plan", oldSpec, oldVerify);
    const plan = await runCli(fixture, ["bundle", "check", "old-plan"]);
    expect(plan.stderr).toBe("");
    expect(plan.exitCode).toBe(0);
    expect(plan.stdout).toBe("bundle check old-plan: SPEC.md has no ## Units section; nothing to check\n");
    const final = await runCli(fixture, ["bundle", "check", "old-plan", "--final"]);
    expect(final.exitCode).toBe(1);
    expect(problems(final.stderr)).toEqual([`${join(directory, "SPEC.md")}:1 ## Units section is missing`]);

    const foreign = join(fixture.repo, "old-bundles", "archived-plan");
    await mkdir(foreign, { recursive: true });
    await writeFile(join(foreign, "SPEC.md"), oldSpec);
    await writeFile(join(foreign, "VERIFY.md"), oldVerify);
    expect((await runCli(fixture, ["bundle", "save", foreign])).exitCode).toBe(0);
    const archived = await runCli(fixture, ["bundle", "check", "archived-plan"]);
    expect(archived.stderr).toBe("");
    expect(archived.exitCode).toBe(0);
    expect(archived.stdout).toBe("bundle check archived-plan: SPEC.md has no ## Units section; nothing to check\n");
  });
}, 30_000);
