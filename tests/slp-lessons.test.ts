import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  runCliAt,
  withFixture,
  type Fixture,
} from "./helpers.ts";

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

async function makeStore(path: string): Promise<void> {
  await mkdir(join(path, ".maestro", "plugins"), { recursive: true });
  await writeFile(join(path, ".maestro", "config"), `${JSON.stringify({ plugins: [] })}\n`);
}

test("551 the room renders one project view from its own store and the repository's (w552/d725)", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.home, "maestro");
    await makeStore(room);
    await writeFile(join(room, "registry"), `${fixture.repo}\n${join(fixture.root, "gone")}\n`);

    expect((await runCli(fixture, fileArgs("the Lead closed a relay in two commands"))).exitCode)
      .toBe(0);
    const roomFiled = await runCliAt(
      fixture,
      room,
      fileArgs("the room relayed intent without naming the report target", [
        "--project",
        "repo",
      ]),
    );
    expect(roomFiled.exitCode).toBe(0);

    const rendered = await runCliAt(fixture, room, ["lesson", "render"]);
    expect(rendered.exitCode).toBe(0);
    expect(rendered.stdout).toContain("PROJECT/repo.md");

    const view = await readFile(join(room, "PROJECT", "repo.md"), "utf8");
    // Both stores reach one view: the repository files its own corrections and
    // the room files the ones it makes about that project.
    expect(view).toContain("the Lead closed a relay in two commands");
    expect(view).toContain("the room relayed intent without naming the report target");
    expect(view).toContain("a relay card closes in one command");
    // A rendered view says so: an edit here is lost on the next render.
    expect(view).toContain("maestro lesson render");
    expect(view).toContain("never hand-edited");
    // A registry entry that is not there is skipped, not an error.
    expect(rendered.stdout).toContain("skipped");
  });
});

test("552 a new team inherits processed lessons as well as pending ones (w552/d42)", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.home, "maestro");
    await makeStore(room);
    await writeFile(join(room, "registry"), `${fixture.repo}\n`);

    expect((await runCli(fixture, fileArgs("the correction already answered"))).exitCode).toBe(0);
    expect((await runCli(fixture, fileArgs("the correction still open"))).exitCode).toBe(0);
    expect((await runCli(fixture, ["lesson", "process", "l1", "--commit", "25f6cd03"])).exitCode)
      .toBe(0);

    expect((await runCliAt(fixture, room, ["lesson", "render"])).exitCode).toBe(0);
    const view = await readFile(join(room, "PROJECT", "repo.md"), "utf8");

    expect(view).toContain("the correction still open");
    expect(view).toContain("the correction already answered");
    expect(view).toContain("25f6cd03");
  });
});

test("554 install ships maestro-improve beside the other method skills (w553/d42)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const skill = join(fixture.home, "maestro", "skills", "maestro-improve", "SKILL.md");
    expect(await readFile(skill, "utf8")).toMatch(/<!-- maestro-skill-version: [0-9a-f]{40} -->/);
    expect(await realpath(join(fixture.home, ".claude", "skills", "maestro-improve"))).toBe(
      await realpath(join(fixture.home, "maestro", "skills", "maestro-improve")),
    );
  });
});

test("555 [lint] the improver skill is one lane parameterised by target (w553/d42, d44)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-improve");
  const skill = await readFile(join(root, "SKILL.md"), "utf8");

  // One skill, one parameter: the target is what the lane is pointed at.
  expect(skill).toContain("maestro lesson list");
  expect(skill).toContain("target");
  // The smallest edit per group, and the evidence ids travel into the commit.
  expect(skill).toContain("smallest edit");
  expect(skill).toContain("evidence ids");
  // A lesson is closed by pointing at the commit, or answered when it is wrong.
  expect(skill).toContain("maestro lesson process");
  expect(skill).toContain("--answer");
  expect(skill).toContain("never deletes");
  // Progressive disclosure: the target catalogue is a reference, not the skill.
  expect(skill).toContain("references/targets.md");
  expect(await readFile(join(root, "references", "targets.md"), "utf8")).toContain(
    "Workspace Protocol",
  );
});

test("556 [lint] slp.md carries the improver loop from threshold to challenge (w553/d42, d43, d44)", async () => {
  const slp = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );

  // The trigger is a threshold the room reads, never a correction firing a run.
  expect(slp).toContain("LESSONS_PENDING");
  expect(slp).toContain("never per correction");
  // One delivery lane on the strong rung, then a challenge on the diverse one.
  expect(slp).toContain("`maestro-improve`");
  expect(slp).toContain("strong rung");
  expect(slp).toContain("diverse rung");
  // d43: the scenario harness gates the first run.
  expect(slp).toContain("golden");
  // d44: sources, and a rejection that stays as data.
  expect(slp).toContain("through its handback");
  expect(slp).toContain("never deleted");
});

test("560 lesson render warns before it summarises and names the command that shows why (w558)", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.home, "maestro");
    await makeStore(room);
    const broken = join(fixture.root, "broken");
    await mkdir(join(broken, ".maestro"), { recursive: true });
    await writeFile(join(broken, ".maestro", "config"), "not json\n");
    await writeFile(join(room, "registry"), `${fixture.repo}\n${broken}\n`);

    expect((await runCli(fixture, fileArgs("a correction worth rendering"))).exitCode).toBe(0);

    const rendered = await runCliAt(fixture, room, ["lesson", "render"]);
    expect(rendered.exitCode).toBe(0);

    const lines = rendered.stdout.trim().split("\n");
    const warning = lines.findIndex((line) => line.startsWith("Unreadable repository:"));
    const summary = lines.findIndex((line) => line.startsWith("PROJECT/"));
    // A store the render could not read is why the view below it is
    // incomplete, so it is read before the view it qualifies, not after.
    expect(warning).toBeGreaterThanOrEqual(0);
    expect(summary).toBeGreaterThanOrEqual(0);
    expect(warning).toBeLessThan(summary);
    // render discards the child's stderr, so the line names the command that
    // shows what the child said.
    expect(rendered.stdout).toContain(`cd ${broken} && maestro lesson list --all`);
  });
});

function improverSession(): Record<string, string> {
  return { MAESTRO_SESSION_ID: "improver-session", MAESTRO_SESSION_PID: String(process.pid) };
}

async function acceptedDispatch(fixture: Fixture): Promise<string> {
  const work = idFrom(
    await runCli(fixture, [
      "work",
      "add",
      "answer the pending lessons",
      "--atomic-reason",
      "one improver pass",
    ]),
  );
  const opened = await runCli(fixture, [
    "dispatch",
    "open",
    work,
    "--objective",
    "process the pending lessons",
    "--owned-scope",
    "doctrine files",
    "--excluded-scope",
    "product source",
    "--mutation",
    "write-bounded: doctrine files",
    "--stop-condition",
    "the lessons are answered",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: the replay",
    "--pane",
    "w1:pA",
    "--target-session",
    "improver-session",
  ]);
  const dispatch = opened.stdout.match(/^(x\d+) \[open\]/)?.[1];
  if (!dispatch) throw new Error(`missing dispatch id: ${opened.stdout}${opened.stderr}`);
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], improverSession())).exitCode)
    .toBe(0);
  return dispatch;
}

function handbackArgs(dispatch: string, extra: string[]): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the pending lessons are answered by one commit",
    "--proof",
    "source: the doctrine diff",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
    ...extra,
  ];
}

test("563 a handback names the lessons it answers, repeatable and store-qualified (w561/d729)", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.home, "maestro");
    await makeStore(room);
    // A store path can carry a comma, which is why the option repeats instead
    // of splitting one value: a comma-split fails at resolution, not at parse.
    const team = join(fixture.root, "team,one");
    await makeStore(team);
    expect((await runCliAt(fixture, room, fileArgs("the room retyped four commit shas"))).exitCode)
      .toBe(0);
    expect((await runCliAt(fixture, team, fileArgs("the team cited a decision it cannot read")))
      .exitCode).toBe(0);

    const dispatch = await acceptedDispatch(fixture);
    const filed = await runCli(
      fixture,
      handbackArgs(dispatch, ["--lessons", `l1@${room}`, "--lessons", `l1@${team}`]),
      improverSession(),
    );

    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain(`lessons: l1@${room}, l1@${team}`);

    // The room reads the return in this store read-only and then runs
    // lesson process in its own cwd, so the field is what it parses.
    const shown = await runCli(fixture, ["handback", "show", dispatch], improverSession());
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`lessons: l1@${room}, l1@${team}`);
    const listed = await runCli(fixture, ["handback", "list", dispatch, "--json"]);
    expect(listed.exitCode).toBe(0);
    const envelope = JSON.parse(listed.stdout) as { data: { handbacks: { lessons: string[] }[] } };
    expect(envelope.data.handbacks[0]?.lessons).toEqual([`l1@${room}`, `l1@${team}`]);
  });
});

test("564 an absent lesson id refuses and an unreadable store warns and files (w561/d729)", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.home, "maestro");
    await makeStore(room);
    expect((await runCliAt(fixture, room, fileArgs("the room retyped four commit shas"))).exitCode)
      .toBe(0);
    const broken = join(fixture.root, "broken");
    await mkdir(join(broken, ".maestro"), { recursive: true });
    await writeFile(join(broken, ".maestro", "config"), "not json\n");

    const dispatch = await acceptedDispatch(fixture);
    const absent = await runCli(
      fixture,
      handbackArgs(dispatch, ["--lessons", `l9@${room}`]),
      improverSession(),
    );

    // A readable store with an absent id is the lane's own mistake, cheap to
    // fix at the keyboard and expensive as a NOT_FOUND in the room later.
    expect(absent.exitCode).not.toBe(0);
    expect(absent.stderr).toContain("LESSON_NOT_FOUND");
    expect(absent.stderr).toContain("l9");
    expect(absent.stderr).toContain(room);

    const filed = await runCli(
      fixture,
      handbackArgs(dispatch, ["--lessons", `l1@${room}`, "--lessons", `l1@${broken}`]),
      improverSession(),
    );
    // An unreadable store is a fact the room needs told, not a reason to block
    // a return, and the refusal above left the dispatch open to file this one.
    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain(`lessons: l1@${room}, l1@${broken}`);
    expect(filed.stderr).toContain(broken);
    expect(filed.stderr).toContain("l1");
    expect(filed.stderr).toContain(`cd ${broken} && maestro lesson list --all`);
  });
});

test("565 --lessons takes a lesson id and an absolute store path or it refuses (w561/d729)", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await acceptedDispatch(fixture);
    for (const value of ["l1", "l1@team/one", `12@${join(fixture.root, "room")}`]) {
      const refused = await runCli(
        fixture,
        handbackArgs(dispatch, ["--lessons", value]),
        improverSession(),
      );
      expect(refused.exitCode).not.toBe(0);
      expect(refused.stderr).toContain("<lesson id>@<store path>");
      expect(refused.stderr).toContain(value);
    }
  });
});

test("566 [lint] the doctrine that answers a lesson names the field that carries it (w561/d729)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins");
  const slp = await readFile(join(root, "recipes", "slp.md"), "utf8");
  const skill = await readFile(join(root, "skills", "maestro-improve", "SKILL.md"), "utf8");

  for (const text of [slp, skill]) {
    expect(text).toContain("--lessons");
    // The store is half the id: lesson process writes only where it runs.
    expect(text).toContain("<lesson id>@<store path>");
  }
});
