import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

interface LessonRow {
  answer: string | null;
  commit_sha: string | null;
  evidence: string;
  expected: string;
  happened: string;
  id: string;
  project: string;
  state: string;
  target: string;
  why: string;
}

function lessons(fixture: Fixture): LessonRow[] {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    return database.query<LessonRow, []>("SELECT * FROM lessons ORDER BY id").all();
  } finally {
    database.close();
  }
}

function fileArgs(happened: string, extra: string[] = []): string[] {
  return [
    "lesson",
    "file",
    happened,
    "--target",
    "recipe work: the start-before-work rule",
    "--expected",
    "a relay card closes in one command",
    "--why",
    "a relay card records a handoff and is never worked in the delivery sense",
    "--evidence",
    "w7",
    ...extra,
  ];
}

test("543 a correction becomes a lesson record in one command (w550/d40)", async () => {
  await withFixture(async (fixture) => {
    const filed = await runCli(
      fixture,
      fileArgs("the room ran work start then work done seconds apart", ["--evidence", "d720"]),
    );

    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain("l1 [pending]");

    const shown = await runCli(fixture, ["lesson", "show", "l1"]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("target: recipe work: the start-before-work rule");
    expect(shown.stdout).toContain("expected: a relay card closes in one command");
    expect(shown.stdout).toContain("why: a relay card records a handoff");
    expect(shown.stdout).toContain("evidence: w7, d720");
    // The project tag defaults to the store scope, which is the registry name
    // the room renders its per-project view from (d723).
    expect(shown.stdout).toContain("project: repo");

    const [row] = lessons(fixture);
    expect(row?.state).toBe("pending");
    expect(JSON.parse(row?.evidence ?? "[]")).toEqual(["w7", "d720"]);
  });
});

test("544 a lesson without the gap it names is refused, not half-filed (w550/d40)", async () => {
  await withFixture(async (fixture) => {
    const missing = await runCli(fixture, [
      "lesson",
      "file",
      "the room ran two commands",
      "--target",
      "recipe work",
      "--why",
      "the card is a handoff",
      "--evidence",
      "w7",
    ]);

    expect(missing.exitCode).not.toBe(0);
    expect(missing.stderr).toContain("MISSING_ARGUMENT");
    expect(missing.stderr).toContain("--expected");
    expect(lessons(fixture)).toHaveLength(0);
  });
});

test("545 processing points a lesson at the commit that answers it and keeps it (w550/d42)", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, fileArgs("done needed two commands"))).exitCode).toBe(0);

    const processed = await runCli(fixture, ["lesson", "process", "l1", "--commit", "47524c9c"]);
    expect(processed.exitCode).toBe(0);
    expect(processed.stdout).toContain("l1 [processed]");
    expect(processed.stdout).toContain("commit: 47524c9c");

    // The improver reads what is still pending; nothing is ever deleted.
    const pending = await runCli(fixture, ["lesson", "list"]);
    expect(pending.exitCode).toBe(0);
    expect(pending.stdout).not.toContain("l1");
    const all = await runCli(fixture, ["lesson", "list", "--all"]);
    expect(all.stdout).toContain("l1 [processed]");
    expect(lessons(fixture)).toHaveLength(1);

    const again = await runCli(fixture, ["lesson", "process", "l1", "--commit", "deadbeef"]);
    expect(again.exitCode).not.toBe(0);
    expect(again.stderr).toContain("INVALID_STATE");
    expect(lessons(fixture)[0]?.commit_sha).toBe("47524c9c");
  });
});

test("546 a rejected lesson is answered with the reason and stays as data (w550/d44)", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, fileArgs("the improver disagreed"))).exitCode).toBe(0);

    const bare = await runCli(fixture, ["lesson", "process", "l1"]);
    expect(bare.exitCode).not.toBe(0);
    expect(bare.stderr).toContain("MISSING_ARGUMENT");
    expect(bare.stderr).toContain("--answer");

    const rejected = await runCli(fixture, [
      "lesson",
      "process",
      "l1",
      "--answer",
      "the rule already says this; the correction misread it",
    ]);
    expect(rejected.exitCode).toBe(0);
    expect(rejected.stdout).toContain("l1 [processed]");
    expect(rejected.stdout).toContain("answer: the rule already says this");

    const [row] = lessons(fixture);
    expect(row?.commit_sha).toBeNull();
    expect(row?.answer).toContain("the correction misread it");
  });
});

test("547 a lesson carries evidence and a project from another store as written (w550/d723)", async () => {
  await withFixture(async (fixture) => {
    // A Lead files in its repository store about a room decision: neither id
    // exists here, and refusing them would refuse the evidence d40 asks for.
    const filed = await runCli(fixture, [
      "lesson",
      "file",
      "the relay card needed two commands to close",
      "--target",
      "lead.md: reporting a relay",
      "--expected",
      "one command",
      "--why",
      "the room never works a relay card",
      "--evidence",
      "w9",
      "--evidence",
      "d40",
      "--project",
      "cmux",
    ]);
    expect(filed.exitCode).toBe(0);

    expect((await runCli(fixture, fileArgs("a maestro-side correction"))).exitCode).toBe(0);

    const scoped = await runCli(fixture, ["lesson", "list", "--project", "cmux"]);
    expect(scoped.exitCode).toBe(0);
    expect(scoped.stdout).toContain("l1");
    expect(scoped.stdout).toContain("cmux");
    expect(scoped.stdout).not.toContain("l2");
  });
});

function backdate(fixture: Fixture, id: string, days: number): void {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
  try {
    const when = new Date(Date.now() - days * 24 * 60 * 60_000).toISOString();
    database.query("UPDATE lessons SET created_at = ? WHERE id = ?").run(when, id);
  } finally {
    database.close();
  }
}

test("548 five pending lessons raise LESSONS_PENDING for their project (w551/d42)", async () => {
  await withFixture(async (fixture) => {
    for (let index = 0; index < 4; index += 1) {
      expect((await runCli(fixture, fileArgs(`correction ${index}`))).exitCode).toBe(0);
    }

    const quiet = await runCli(fixture, ["attention"]);
    expect(quiet.exitCode).toBe(0);
    expect(quiet.stdout).not.toContain("LESSONS_PENDING");

    expect((await runCli(fixture, fileArgs("correction 4"))).exitCode).toBe(0);

    const raised = await runCli(fixture, ["attention"]);
    expect(raised.exitCode).toBe(0);
    expect(raised.stdout).toContain("attention LESSONS_PENDING lesson l1");
    expect(raised.stdout).toContain("5 lessons pending for repo");
    expect(raised.stdout).toContain("maestro lesson list --project repo");
  });
});

test("549 seven days without an improver run raise it before the count does (w551/d724)", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, fileArgs("the only correction"))).exitCode).toBe(0);

    const fresh = await runCli(fixture, ["attention"]);
    expect(fresh.stdout).not.toContain("LESSONS_PENDING");

    backdate(fixture, "l1", 8);
    const aged = await runCli(fixture, ["attention"]);
    expect(aged.stdout).toContain("attention LESSONS_PENDING lesson l1");
    expect(aged.stdout).toContain("no improver run");

    // Processing it is the improver run: the clock restarts from there, so a
    // lesson filed today does not re-raise.
    expect((await runCli(fixture, ["lesson", "process", "l1", "--commit", "806ba20e"])).exitCode)
      .toBe(0);
    expect((await runCli(fixture, fileArgs("a fresh correction"))).exitCode).toBe(0);
    const quiet = await runCli(fixture, ["attention"]);
    expect(quiet.stdout).not.toContain("LESSONS_PENDING");
  });
});

test("550 brief carries LESSONS_PENDING from a registered project (w551/d42)", async () => {
  await withFixture(async (fixture) => {
    for (let index = 0; index < 5; index += 1) {
      expect((await runCli(fixture, fileArgs(`correction ${index}`))).exitCode).toBe(0);
    }
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);

    const brief = await runCli(fixture, ["brief"], { MAESTRO_READ_ONLY: "1" });

    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain("LESSONS_PENDING");
    expect(brief.stdout).toContain(fixture.repo);
  });
});
