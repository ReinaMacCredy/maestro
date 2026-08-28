import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  runTool,
  type Fixture,
  withFixture,
} from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) };
}

async function addWork(fixture: Fixture, title: string, parent?: string): Promise<string> {
  const args = ["work", "add", title, "--atomic-reason", "slp fixture"];
  if (parent) args.push("--parent", parent);
  return idFrom(await runCli(fixture, args));
}

async function openAttentionDispatch(
  fixture: Fixture,
  work: string,
  lane: "decision" | "delivery" | "scout" | "shadow",
  ownedScope: string,
  excludedScope: string,
  targetSession?: string,
  opener = "test-session",
): Promise<string> {
  const args = [
    "dispatch",
    "open",
    work,
    "--objective",
    "exercise dispatch scope attention",
    "--owned-scope",
    ownedScope,
    "--excluded-scope",
    excludedScope,
    "--mutation",
    lane === "delivery" ? "write-bounded to the owned scope" : "no-write",
    "--stop-condition",
    "attention observed",
    "--lane",
    lane,
    "--evidence-required",
    "source: attention packet",
    "--pane",
    `w1:p-${work}`,
  ];
  if (targetSession) args.push("--target-session", targetSession);
  const opened = await runCli(fixture, args, session(opener));
  expect(opened.exitCode).toBe(0);
  const dispatch = opened.stdout.match(/^(x\d+) \[open\]/)?.[1];
  expect(dispatch).toBeString();
  return dispatch as string;
}

async function startWork(
  fixture: Fixture,
  id: string,
  holder: string,
  pid = process.pid,
): Promise<void> {
  const started = await runCli(fixture, ["work", "start", id], session(holder, pid));
  expect(started.exitCode).toBe(0);
}

async function recordSession(fixture: Fixture, id: string): Promise<void> {
  const recorded = await runCli(
    fixture,
    ["hook", "record", "--event", "SessionStart"],
    session(id),
  );
  expect(recorded.exitCode).toBe(0);
}

function openDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"));
}

async function retiredSupervisorSnapshot(fixture: Fixture) {
  const processList = await runTool(["ps", "-axo", "pid=,command="], fixture.repo);
  expect(processList.exitCode).toBe(0);
  const processes = processList.stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => /\bmaestro(?:\.ts)?\s+supervisor(?:\s|$)/.test(line))
    .sort();
  const database = openDatabase(fixture);
  let tables: Array<{ name: string }>;
  try {
    tables = database
      .query<{ name: string }, []>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('messages', 'message_cursors') ORDER BY name",
      )
      .all();
  } finally {
    database.close();
  }
  return {
    files: {
      log: await Bun.file(join(fixture.repo, ".maestro", "supervisor.log")).exists(),
      state: await Bun.file(join(fixture.repo, ".maestro", "supervisor.json")).exists(),
    },
    processes,
    tables,
  };
}

function backdateSession(fixture: Fixture, id: string, minutes: number): void {
  const database = openDatabase(fixture);
  try {
    database
      .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - minutes * 60_000).toISOString(), id);
  } finally {
    database.close();
  }
}

test("133 install materializes the dispatch, handback, dependency, and episode contracts", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const skill = await Bun.file(
      join(fixture.home, "maestro", "skills", "maestro-work", "SKILL.md"),
    ).text();
    for (const required of [
      "## Dispatch",
      "## Handback",
      "Lane",
      "Excluded scope",
      "DONE",
      "BLOCKED",
      "UNTESTABLE",
      "UNKNOWN",
      "FAILED",
      "CHALLENGE",
      "REOPEN_REQUEST",
      "DEPENDENCY_REQUEST",
      "COUNCIL_REQUEST",
      "A+B+C",
      "Attempted",
      "Invariant assumed",
      "Exact failure",
      "What changed between attempts",
      "What did not change",
      "Smallest new information needed",
    ]) {
      expect(skill).toContain(required);
    }
  });
});

test("134 install materializes layered proof, failed traces, learning, and triage", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const verifyRoot = join(fixture.home, "maestro", "skills", "maestro-verify");
    const skill = await Bun.file(join(verifyRoot, "SKILL.md")).text();
    for (const required of [
      "source",
      "artifact",
      "installed",
      "live",
      "journey",
      "NOT TESTED",
      "Assumptions not verified",
      "Residual risks",
      '"failed: ',
    ]) {
      expect(skill).toContain(required);
    }

    const learning = await Bun.file(join(verifyRoot, "references", "learning.md")).text();
    expect(learning).toContain("canary");
    expect(learning).toContain("review/delete date");

    const triagePath = join(verifyRoot, "references", "triage.md");
    expect(await Bun.file(triagePath).exists()).toBe(true);
    const triage = await Bun.file(triagePath).text();
    for (const step of [
      "Problem",
      "Authority",
      "Topology",
      "Attention",
      "Capability",
      "State",
      "Evidence",
      "Owning layer",
      "Learning",
    ]) {
      expect(triage).toContain(step);
    }

    const audit = await Bun.file(join(verifyRoot, "references", "audit.md")).text();
    expect(audit).toContain("triage.md");
  });
});

test("135 install materializes intake, council reconcile, and handoff doctrine", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const skillsRoot = join(fixture.home, "maestro", "skills");
    const design = await Bun.file(join(skillsRoot, "maestro-design", "SKILL.md")).text();
    expect(design.indexOf("## Intake")).toBeGreaterThan(-1);
    expect(design.indexOf("## Intake")).toBeLessThan(design.indexOf("## Recall pass first"));
    for (const required of [
      "state unknown",
      "several architectures",
      "contract clear",
      "candidate needs breaking",
      "hard-to-reverse fork",
      "ROI",
      "## Council",
      "premise",
      "mechanism",
      "boundary",
      "failure",
      "reversibility",
      "evidence",
      "authority",
      "proof",
    ]) {
      expect(design).toContain(required);
    }

    const bundle = await Bun.file(join(skillsRoot, "maestro-bundle", "SKILL.md")).text();
    for (const required of [
      "break-before-make",
      "owner changes",
      "dependency becomes its own branch",
      "role changes",
      "context is full of false starts",
    ]) {
      expect(bundle).toContain(required);
    }
  });
});

test("136 bundle open scaffolds the NOTES handoff packet after Next Action", async () => {
  await withFixture(async (fixture) => {
    const opened = await runCli(fixture, ["bundle", "open", "slp-handoff"]);
    expect(opened.exitCode).toBe(0);

    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "slp-handoff", "NOTES.md"),
    ).text();
    const nextAction = notes.indexOf("## Next Action");
    const authority = notes.indexOf("## Authority");
    const failed = notes.indexOf("## Failed approaches");
    const doNotRepeat = notes.indexOf("## Do not repeat");
    expect(authority).toBeGreaterThan(nextAction);
    expect(failed).toBeGreaterThan(authority);
    expect(doNotRepeat).toBeGreaterThan(failed);
  });
});

test("137 SessionStart adds only the intake line and UserPromptSubmit stays byte-identical", async () => {
  await withFixture(async (fixture) => {
    const session = {
      MAESTRO_SESSION_ID: "slp-brief",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const start = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    expect(start.exitCode).toBe(0);
    expect(start.stdout).toContain(
      '  close: maestro bundle close <id> after VERIFY passes; recall with maestro search "<term>"\n' +
        "intake: problem in one sentence; uncertainty -> lane (scout no-write | decision x2-3 | delivery | challenge | shadow no-write); ROI 0-10 -> tier; say the route and the one not taken\n",
    );

    const prompt = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session,
    );
    expect(prompt.exitCode).toBe(0);
    expect(prompt.stdout).toBe(
      "held work: none\n" +
        "enabled policies: policy-breakdown, policy-dispatch, policy-lifecycle, policy-proof\n" +
        "next: maestro ready\n" +
        "recipes: maestro recipe list; maestro recipe show <name>\n",
    );
  });
});

test("138 attention raises and records a STALLED_LEASE packet at read time", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "parent scope");
    await startWork(fixture, parent, "lead-session");
    const child = await addWork(fixture, "stalled child", parent);
    await startWork(fixture, child, "subject-session", 1);
    backdateSession(fixture, "subject-session", 45);

    const attention = await runCli(
      fixture,
      ["attention", "--stale", "30"],
      session("scanner-session"),
    );
    expect(attention.exitCode).toBe(0);
    for (const required of [
      `attention STALLED_LEASE work ${child}`,
      "  observed:",
      "  evidence:",
      "  unknown:",
      "  question:",
      `  smallest action: maestro work show ${child}`,
      "  human decision needed: no",
    ]) {
      expect(attention.stdout).toContain(required);
    }

    const database = openDatabase(fixture);
    try {
      const row = database
        .query<{
          fingerprint: string;
          packet: string;
        }, []>("SELECT fingerprint, packet FROM attention")
        .get();
      const start = database
        .query<{ id: number }, [string]>(
          "SELECT id FROM event_log WHERE type = 'work.start' AND entity_id = ? ORDER BY id DESC LIMIT 1",
        )
        .get(child);
      expect(row?.fingerprint).toBe(`stalled:${child}:${start?.id}`);
      expect(row?.packet).toContain("unknown:");
    } finally {
      database.close();
    }
  });
});

test("139 attention records findings without delivery targets or mailbox tables", async () => {
  await withFixture(async (fixture) => {
    const retired = openDatabase(fixture);
    retired.exec(`
      CREATE TABLE messages (id TEXT);
      CREATE TABLE message_cursors (id TEXT);
    `);
    retired.close();

    const work = await addWork(fixture, "decision without recipient");
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "stale without recipient", "--work", work]),
    );
    const database = openDatabase(fixture);
    try {
      database
        .query("UPDATE decisions SET created_at = ? WHERE id = ?")
        .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    } finally {
      database.close();
    }

    const attention = await runCli(
      fixture,
      ["attention", "--json", "--decision-stale", "24"],
      session("scanner-session"),
    );
    expect(attention.exitCode).toBe(0);
    const detection = (JSON.parse(attention.stdout) as {
      data: { detections: Array<Record<string, unknown>> };
    }).data.detections[0];
    expect(detection).not.toHaveProperty("targets");
    const recorded = openDatabase(fixture);
    try {
      expect(recorded.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count)
        .toBe(1);
      expect(
        recorded
          .query<{ name: string }, []>(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('messages', 'message_cursors') ORDER BY name",
          )
          .all(),
      ).toEqual([]);
    } finally {
      recorded.close();
    }
  });
});

test("140 attention fingerprints are one-shot and existing raises stay listed", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "deduplicated lease");
    await startWork(fixture, work, "subject-session");
    backdateSession(fixture, "subject-session", 45);

    const first = await runCli(fixture, ["attention"], session("scanner-session"));
    const second = await runCli(fixture, ["attention"], session("scanner-session"));
    expect(first.exitCode).toBe(0);
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toMatch(/raised \d{4}-\d{2}-\d{2}T/);

    const database = openDatabase(fixture);
    try {
      expect(database.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count)
        .toBe(1);
    } finally {
      database.close();
    }
  });
});

test("141 attention raises REPEATED_FAILURE only for the third failed note since start", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "three failed passes");
    await startWork(fixture, work, "worker-session");
    for (const text of ["failed: first", "failed: second", "failed: third", "failed: fourth"]) {
      expect((await runCli(fixture, ["work", "note", work, text], session("worker-session"))).exitCode)
        .toBe(0);
    }
    const attention = await runCli(fixture, ["attention"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain(`attention REPEATED_FAILURE work ${work}`);

    const database = openDatabase(fixture);
    try {
      const third = database
        .query<{ id: number }, [string]>(
          "SELECT id FROM work_notes WHERE work_id = ? AND text LIKE 'failed: %' ORDER BY id LIMIT 1 OFFSET 2",
        )
        .get(work);
      expect(
        database.query<{ fingerprint: string }, []>(
          "SELECT fingerprint FROM attention WHERE kind = 'REPEATED_FAILURE'",
        ).get()?.fingerprint,
      ).toBe(`repeat:${work}:${third?.id}`);
    } finally {
      database.close();
    }
  });

  for (const notes of [
    ["failed: first", "failed: second"],
    ["failed:first", "failed:second", "failed:third"],
    ["Failed: first", "Failed: second", "Failed: third"],
    ["failed first", "failed second", "ordinary failure"],
  ]) {
    await withFixture(async (fixture) => {
      const work = await addWork(fixture, "non-qualifying failures");
      await startWork(fixture, work, "worker-session");
      for (const text of notes) {
        await runCli(fixture, ["work", "note", work, text], session("worker-session"));
      }
      const attention = await runCli(fixture, ["attention"], session("scanner-session"));
      expect(attention.exitCode).toBe(0);
      expect(attention.stdout).not.toContain("REPEATED_FAILURE");
    });
  }
});

test("142 attention raises DECISION_STALE only for old drafts linked to open work", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "open decision work");
    const drafted = await runCli(fixture, ["decision", "draft", "old fork", "--work", work]);
    const decision = idFrom(drafted);
    const database = openDatabase(fixture);
    try {
      database
        .query("UPDATE decisions SET created_at = ? WHERE id = ?")
        .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    } finally {
      database.close();
    }
    const attention = await runCli(
      fixture,
      ["attention", "--decision-stale", "24"],
      session("scanner-session"),
    );
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain(`attention DECISION_STALE decision ${decision}`);
  });

  for (const terminal of ["locked", "done"] as const) {
    await withFixture(async (fixture) => {
      const work = await addWork(fixture, `${terminal} decision work`);
      const drafted = await runCli(fixture, ["decision", "draft", "old fork", "--work", work]);
      const decision = idFrom(drafted);
      if (terminal === "locked") {
        await runCli(fixture, ["decision", "lock", decision]);
      }
      const database = openDatabase(fixture);
      try {
        database
          .query("UPDATE decisions SET created_at = ? WHERE id = ?")
          .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
        if (terminal === "done") {
          database.query("UPDATE work SET state = 'done', held_by = NULL WHERE id = ?").run(work);
        }
      } finally {
        database.close();
      }
      const attention = await runCli(
        fixture,
        ["attention", "--decision-stale", "24"],
        session("scanner-session"),
      );
      expect(attention.exitCode).toBe(0);
      expect(attention.stdout).not.toContain("DECISION_STALE");
    });
  }
});

test("416 attention raises HUMAN_DECISION_REQUIRED immediately only for marked drafts", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "owner decision needed");
    const ordinary = idFrom(
      await runCli(fixture, ["decision", "draft", "ordinary fork", "--work", work]),
    );
    const marked = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "owner must choose",
        "--needs-owner",
        "--work",
        work,
      ]),
    );

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: {
        detections: Array<{
          fingerprint: string;
          kind: string;
          packet: string;
          subjectWork: string | null;
        }>;
      };
    }).data.detections.filter((detection) => detection.kind === "HUMAN_DECISION_REQUIRED");
    expect(detections).toHaveLength(1);
    expect(detections[0]).toEqual(expect.objectContaining({
      fingerprint: `human-decision:${marked}`,
      subjectWork: work,
    }));
    for (const required of [
      `attention HUMAN_DECISION_REQUIRED decision ${marked}`,
      "  observed:",
      "  evidence:",
      "  unknown:",
      "  question:",
      `  smallest action: maestro decision show ${marked}`,
      "  human decision needed: yes",
    ]) {
      expect(detections[0]?.packet).toContain(required);
    }
    expect(detections[0]?.packet).not.toContain(ordinary);
    expect(attention.stdout).not.toContain("DECISION_STALE");

    const hook = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session("scanner-session"),
    );
    expect(hook.exitCode).toBe(0);
    expect(hook.stdout).toContain(`attention HUMAN_DECISION_REQUIRED decision ${marked}`);
  });
});

test("453 withdrawn drafts raise neither stale nor human-decision attention", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "withdrawn owner decision");
    const decision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "owner choice that lost",
        "--needs-owner",
        "--work",
        work,
      ]),
    );
    const database = openDatabase(fixture);
    try {
      database
        .query("UPDATE decisions SET created_at = ? WHERE id = ?")
        .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    } finally {
      database.close();
    }
    expect((await runCli(fixture, [
      "decision",
      "withdraw",
      decision,
      "--reason",
      "a different option was locked",
    ])).exitCode).toBe(0);

    const attention = await runCli(
      fixture,
      ["attention", "--json", "--decision-stale", "24"],
      session("scanner-session"),
    );
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: { detections: Array<{ kind: string; packet: string }> };
    }).data.detections;
    for (const kind of ["DECISION_STALE", "HUMAN_DECISION_REQUIRED"]) {
      expect(
        detections.filter((detection) => detection.kind === kind).map((detection) => detection.packet),
      ).not.toContainEqual(expect.stringContaining(`decision ${decision}`));
    }
  });
});

test("430 attention raises DECISION_REVIEW_DUE only for due locked unsuperseded decisions", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "reviewable decision work");
    const past = new Date(Date.now() - 60_000).toISOString();
    const future = new Date(Date.now() + 60_000).toISOString();
    const due = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "due review",
        "--review-at",
        past,
        "--work",
        work,
      ]),
    );
    expect((await runCli(fixture, ["decision", "lock", due])).exitCode).toBe(0);

    const futureDecision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "future review",
        "--review-at",
        future,
        "--work",
        work,
      ]),
    );
    expect((await runCli(fixture, ["decision", "lock", futureDecision])).exitCode).toBe(0);

    const draft = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "draft review",
        "--review-at",
        past,
        "--work",
        work,
      ]),
    );
    const superseded = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "superseded review",
        "--review-at",
        past,
        "--work",
        work,
      ]),
    );
    expect((await runCli(fixture, ["decision", "lock", superseded])).exitCode).toBe(0);
    const replacement = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "replacement decision",
        "--supersedes",
        superseded,
        "--work",
        work,
      ]),
    );
    expect((await runCli(fixture, ["decision", "lock", replacement])).exitCode).toBe(0);

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: {
        detections: Array<{
          fingerprint: string;
          kind: string;
          packet: string;
          subjectWork: string | null;
        }>;
      };
    }).data.detections.filter((detection) => detection.kind === "DECISION_REVIEW_DUE");
    expect(detections).toHaveLength(1);
    expect(detections[0]).toEqual(expect.objectContaining({
      fingerprint: `review:${due}:${past}`,
      subjectWork: work,
    }));
    for (const required of [
      `attention DECISION_REVIEW_DUE decision ${due}`,
      "  observed:",
      "  evidence:",
      "  unknown:",
      "  question:",
      `  smallest action: maestro decision show ${due}`,
      "  human decision needed: no",
    ]) {
      expect(detections[0]?.packet).toContain(required);
    }
    expect(attention.stdout).not.toContain(`DECISION_REVIEW_DUE decision ${futureDecision}`);
    expect(attention.stdout).not.toContain(`DECISION_REVIEW_DUE decision ${draft}`);
    expect(attention.stdout).not.toContain(`DECISION_REVIEW_DUE decision ${superseded}`);
  });
});

test("143 attention raises and records sorted SCOPE_COLLISION", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "shared scope");
    await startWork(fixture, parent, "lead-session");
    const first = await addWork(fixture, "first sibling", parent);
    const second = await addWork(fixture, "second sibling", parent);
    await startWork(fixture, first, "holder-z");
    await startWork(fixture, second, "holder-a");

    const attention = await runCli(fixture, ["attention"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain(`attention SCOPE_COLLISION work ${first},${second}`);

    const database = openDatabase(fixture);
    try {
      const row = database
        .query<{ fingerprint: string }, []>(
          "SELECT fingerprint FROM attention WHERE kind = 'SCOPE_COLLISION'",
        )
        .get();
      expect(row?.fingerprint).toBe(`collision:${first}:${second}:holder-a:holder-z`);
    } finally {
      database.close();
    }
  });
});

test("432 attention raises SCOPE_COLLISION for delivery dispatch paths across work items", async () => {
  await withFixture(async (fixture) => {
    const firstWork = await addWork(fixture, "first unrelated dispatch work");
    const secondWork = await addWork(fixture, "second unrelated dispatch work");
    const first = await openAttentionDispatch(
      fixture,
      firstWork,
      "delivery",
      "src/plugins/dispatch.ts",
      "tests only",
    );
    const second = await openAttentionDispatch(
      fixture,
      secondWork,
      "delivery",
      "src/plugins/dispatch.ts",
      "docs only",
    );

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: { detections: Array<{ fingerprint: string; kind: string; packet: string }> };
    }).data.detections.filter((detection) => detection.kind === "SCOPE_COLLISION");
    expect(detections).toHaveLength(1);
    expect(detections[0]?.fingerprint).toBe(`collision:dispatch:${first}:${second}`);
    expect(detections[0]?.packet).toContain(`attention SCOPE_COLLISION work ${first},${second}`);
    expect(detections[0]?.packet).toContain("evidence: owned_scope tokens src/plugins/dispatch.ts");
    expect(detections[0]?.packet).toContain(`smallest action: maestro dispatch show ${first}`);
  });
});

test("433 dispatch scope collision ignores a path excluded by the other lane", async () => {
  await withFixture(async (fixture) => {
    const firstWork = await addWork(fixture, "first excluded dispatch work");
    const secondWork = await addWork(fixture, "second excluded dispatch work");
    const first = await openAttentionDispatch(
      fixture,
      firstWork,
      "delivery",
      "src/plugins/dispatch.ts",
      "tests only",
    );
    const second = await openAttentionDispatch(
      fixture,
      secondWork,
      "delivery",
      "src/plugins/dispatch.ts",
      "src/plugins/dispatch.ts",
    );

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).not.toContain(`collision:dispatch:${first}:${second}`);
  });
});

test("434 dispatch scope collision ignores overlap between two no-write lanes", async () => {
  await withFixture(async (fixture) => {
    const firstWork = await addWork(fixture, "first no-write dispatch work");
    const secondWork = await addWork(fixture, "second no-write dispatch work");
    const first = await openAttentionDispatch(
      fixture,
      firstWork,
      "decision",
      "src/plugins/dispatch.ts",
      "product writes",
    );
    const second = await openAttentionDispatch(
      fixture,
      secondWork,
      "scout",
      "src/plugins/dispatch.ts",
      "product writes",
    );

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).not.toContain(`collision:dispatch:${first}:${second}`);
  });
});

test("459 dispatch scope collision ignores delivery overlap with a shadow lane", async () => {
  await withFixture(async (fixture) => {
    const deliveryWork = await addWork(fixture, "delivery dispatch work");
    const shadowWork = await addWork(fixture, "shadow dispatch work");
    const delivery = await openAttentionDispatch(
      fixture,
      deliveryWork,
      "delivery",
      "src/plugins/dispatch.ts",
      "tests only",
    );
    const shadow = await openAttentionDispatch(
      fixture,
      shadowWork,
      "shadow",
      "src/plugins/dispatch.ts",
      "product writes",
    );

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).not.toContain(`collision:dispatch:${delivery}:${shadow}`);
  });
});

test("435 dispatch scope collision matches a directory token to its descendant path", async () => {
  await withFixture(async (fixture) => {
    const firstWork = await addWork(fixture, "directory dispatch work");
    const secondWork = await addWork(fixture, "descendant dispatch work");
    const first = await openAttentionDispatch(
      fixture,
      firstWork,
      "delivery",
      "src/plugins",
      "tests only",
    );
    const second = await openAttentionDispatch(
      fixture,
      secondWork,
      "delivery",
      "src/plugins/dispatch.ts",
      "docs only",
    );

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain(`collision:dispatch:${first}:${second}`);
    expect(attention.stdout).toContain("owned_scope tokens src/plugins, src/plugins/dispatch.ts");
  });
});

test("414 attention raises LEAD_COLLISION without treating a delivery Peer as a Lead", async () => {
  await withFixture(async (fixture) => {
    const first = await addWork(fixture, "first Lead scope");
    const second = await addWork(fixture, "second Lead scope");
    await startWork(fixture, first, "lead-z");
    await startWork(fixture, second, "lead-a");

    const peerWork = await addWork(fixture, "delivery Peer scope");
    const opened = await runCli(fixture, [
      "dispatch",
      "open",
      peerWork,
      "--objective",
      "implement the accepted change",
      "--owned-scope",
      "product source",
      "--excluded-scope",
      "push, tag, publish",
      "--mutation",
      "write-bounded: product source",
      "--stop-condition",
      "verified handback filed",
      "--lane",
      "delivery",
      "--evidence-required",
      "source: focused regression",
      "--pane",
      "w1:pPeer",
    ]);
    const dispatch = opened.stdout.match(/^(x\d+) \[open\]/)?.[1];
    expect(opened.exitCode).toBe(0);
    expect(dispatch).toBeString();
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch as string], session("peer-session")))
        .exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "confirm",
          dispatch as string,
          "--session",
          "peer-session",
        ])
      ).exitCode,
    ).toBe(0);
    await startWork(fixture, peerWork, "peer-session");

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: {
        detections: Array<{
          fingerprint: string;
          kind: string;
          packet: string;
          subjectSession: string | null;
          subjectWork: string | null;
        }>;
      };
    }).data.detections.filter((detection) => detection.kind === "LEAD_COLLISION");
    expect(detections).toHaveLength(1);
    expect(detections[0]).toEqual(expect.objectContaining({
      fingerprint: `lead-collision:${first}:${second}:lead-a:lead-z`,
      subjectSession: "lead-a,lead-z",
      subjectWork: first,
    }));
    for (const required of [
      `attention LEAD_COLLISION work ${first},${second}`,
      "  observed:",
      "  evidence:",
      "  unknown:",
      "  question:",
      "  smallest action: maestro status --live",
      "  human decision needed: no",
    ]) {
      expect(detections[0]?.packet).toContain(required);
    }
    expect(detections[0]?.packet).not.toContain(peerWork);
    expect(detections[0]?.packet).not.toContain("peer-session");

    const hook = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session("scanner-session"),
    );
    expect(hook.exitCode).toBe(0);
    expect(hook.stdout).toContain(`attention LEAD_COLLISION work ${first},${second}`);
  });
});

test("458 delivery acceptance after work start does not hide a Lead collision", async () => {
  await withFixture(async (fixture) => {
    const first = await addWork(fixture, "first established Lead");
    const second = await addWork(fixture, "second established Lead");
    await startWork(fixture, first, "lead-a");
    await startWork(fixture, second, "lead-b");

    const dispatch = await openAttentionDispatch(
      fixture,
      second,
      "delivery",
      "src/plugins/attention.ts",
      "tests only",
    );
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch], session("lead-b"))).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(fixture, ["dispatch", "confirm", dispatch, "--session", "lead-b"])
      ).exitCode,
    ).toBe(0);

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain(
      `lead-collision:${first}:${second}:lead-a:lead-b`,
    );
  });
});

test("144 attention --json computes findings without background state", async () => {
  await withFixture(async (fixture) => {
    const before = await retiredSupervisorSnapshot(fixture);
    const work = await addWork(fixture, "json failure");
    await startWork(fixture, work, "worker-session");
    for (const text of ["failed: first", "failed: second", "failed: third"]) {
      await runCli(fixture, ["work", "note", work, text], session("worker-session"));
    }
    const attention = await runCli(
      fixture,
      ["attention", "--json"],
      session("scanner-session"),
    );
    expect(attention.exitCode).toBe(0);
    const envelope = JSON.parse(attention.stdout) as {
      data: { detections: Array<{ kind: string; raised: boolean }> };
      ok: boolean;
    };
    expect(envelope.ok).toBe(true);
    expect(envelope.data.detections[0]?.kind).toBe("REPEATED_FAILURE");
    expect(envelope.data.detections[0]?.raised).toBe(true);
    expect(await retiredSupervisorSnapshot(fixture)).toEqual(before);
  });
});

test("148 install and uninstall manage Claude PreToolUse without changing Codex hooks", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const first = await runCli(fixture, ["install"], { PATH: path });
    expect(first.exitCode).toBe(0);

    type HookConfig = {
      hooks: Record<
        string,
        Array<{ hooks: Array<{ command: string; type: string }>; matcher?: string }>
      >;
    };
    const claudePath = join(fixture.repo, ".claude", "settings.json");
    const codexPath = join(fixture.repo, ".codex", "hooks.json");
    const firstClaude = JSON.parse(await Bun.file(claudePath).text()) as HookConfig;
    const firstCodex = JSON.parse(await Bun.file(codexPath).text()) as HookConfig;
    for (const config of [firstClaude, firstCodex]) {
      expect(config.hooks.SessionStart).toBeArray();
      expect(config.hooks.UserPromptSubmit).toBeArray();
      expect(config.hooks.PostToolUse).toBeUndefined();
    }
    const managedPreToolUse = {
      matcher: "Agent|Task",
      hooks: [{ type: "command", command: "bun .claude/hooks/maestro-record.ts" }],
    };
    const firstClaudePreToolUse = firstClaude.hooks.PreToolUse ?? [];
    expect(firstClaudePreToolUse).toEqual([managedPreToolUse]);
    expect(firstCodex.hooks.PreToolUse).toBeUndefined();

    const foreignPreToolUse = {
      matcher: "Write",
      hooks: [{ type: "command", command: "foreign-pre-tool-hook" }],
    };
    firstClaudePreToolUse.unshift(foreignPreToolUse);
    await Bun.write(claudePath, `${JSON.stringify(firstClaude, null, 2)}\n`);

    const second = await runCli(fixture, ["install"], { PATH: path });
    expect(second.exitCode).toBe(0);
    const secondClaudeText = await Bun.file(claudePath).text();
    const secondClaude = JSON.parse(secondClaudeText) as HookConfig;
    expect(secondClaude.hooks.PreToolUse).toEqual([foreignPreToolUse, managedPreToolUse]);
    expect((JSON.parse(await Bun.file(codexPath).text()) as HookConfig).hooks.PreToolUse)
      .toBeUndefined();
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(await Bun.file(claudePath).text()).toBe(secondClaudeText);

    const uninstalled = await runCli(fixture, ["uninstall"], { PATH: path });
    expect(uninstalled.exitCode).toBe(0);
    const remainingClaude = JSON.parse(await Bun.file(claudePath).text()) as HookConfig;
    expect(remainingClaude.hooks.PreToolUse).toEqual([foreignPreToolUse]);
    expect(remainingClaude.hooks.SessionStart).toBeUndefined();
    expect(remainingClaude.hooks.UserPromptSubmit).toBeUndefined();
    expect(remainingClaude.hooks.PostToolUse).toBeUndefined();
    expect(await Bun.file(codexPath).exists()).toBe(false);
  });
});

test("447 PreToolUse denies returned holders and targets but stays silent for the opener", async () => {
  await withFixture(async (fixture) => {
    const opener = "peer-hook-opener";
    const holder = "peer-hook-holder";
    const target = "peer-hook-target";
    await recordSession(fixture, opener);
    await recordSession(fixture, holder);
    await recordSession(fixture, target);
    const work = await addWork(fixture, "deny peer sub-topology");
    const dispatch = await openAttentionDispatch(
      fixture,
      work,
      "delivery",
      "src/plugins/coordination.ts",
      "all other files",
      undefined,
      opener,
    );
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "confirm",
          dispatch,
          "--session",
          holder,
        ], session(opener))
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          [
            "handback",
            "file",
            dispatch,
            "--status",
            "DONE",
            "--candidate",
            "fixture-candidate",
            "--claim",
            "returned holders remain Peers",
            "--proof",
            "source: hook stdout",
            "--assumptions",
            "None",
            "--residual-risks",
            "None",
            "--incidental-findings",
            "None",
          ],
          session(holder),
        )
      ).exitCode,
    ).toBe(0);

    const targetWork = await addWork(fixture, "deny targeted peer sub-topology");
    const targetedDispatch = await openAttentionDispatch(
      fixture,
      targetWork,
      "scout",
      "tests/slp-adoption.test.ts",
      "product source",
      target,
      opener,
    );

    for (const [peer, peerDispatch] of [
      [holder, dispatch],
      [target, targetedDispatch],
    ] as const) {
      for (const toolName of ["Agent", "Task"]) {
        const denied = await runCli(
          fixture,
          ["hook", "record", "--event", "PreToolUse", "--harness", "claude"],
          session(peer),
          JSON.stringify({ tool_name: toolName }),
        );

        expect(denied.exitCode).toBe(0);
        expect(denied.stderr).toBe("");
        expect(JSON.parse(denied.stdout)).toEqual({
          hookSpecificOutput: {
            hookEventName: "PreToolUse",
            permissionDecision: "deny",
            permissionDecisionReason:
              `${peerDispatch}: a Peer does not create sub-topology (SLP invariant 4)`,
          },
        });
      }
    }
    for (const toolName of ["Agent", "Task"]) {
      const allowed = await runCli(
        fixture,
        ["hook", "record", "--event", "PreToolUse", "--harness", "claude"],
        session(opener),
        JSON.stringify({ tool_name: toolName }),
      );
      expect(allowed.exitCode).toBe(0);
      expect(allowed.stdout).toBe("");
      expect(allowed.stderr).toBe("");
    }
    const status = await runCli(fixture, ["status", "--json"], session(holder));
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ id: string; lastEvent: string }> };
    }).data.sessions;
    expect(sessions.find((candidate) => candidate.id === holder)?.lastEvent).toBe(
      "SessionStart",
    );
  });
});

test("448 PreToolUse stays silent for a Lead invoking Agent or Task", async () => {
  await withFixture(async (fixture) => {
    const lead = "lead-with-unaccepted-dispatch";
    await recordSession(fixture, lead);
    const work = await addWork(fixture, "keep the Lead outside Peer enforcement");
    const opened = await runCli(
      fixture,
      [
        "dispatch",
        "open",
        work,
        "--objective",
        "leave the dispatch unaccepted",
        "--owned-scope",
        "tests/slp-adoption.test.ts",
        "--excluded-scope",
        "product source",
        "--mutation",
        "no-write",
        "--stop-condition",
        "PreToolUse remains silent",
        "--lane",
        "scout",
        "--evidence-required",
        "source: hook stdout",
        "--pane",
        "w1:p-lead-hook",
      ],
      session(lead),
    );
    expect(opened.exitCode).toBe(0);

    for (const toolName of ["Agent", "Task"]) {
      const allowed = await runCli(
        fixture,
        ["hook", "record", "--event", "PreToolUse", "--harness", "claude"],
        session(lead),
        JSON.stringify({ tool_name: toolName }),
      );

      expect(allowed.exitCode).toBe(0);
      expect(allowed.stdout).toBe("");
      expect(allowed.stderr).toBe("");
    }
    const status = await runCli(fixture, ["status", "--json"], session(lead));
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ id: string; lastEvent: string }> };
    }).data.sessions;
    expect(sessions.find((candidate) => candidate.id === lead)?.lastEvent).toBe(
      "SessionStart",
    );
  });
});

test("150 install stays inert and read-time attention preserves work and decisions", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const work = await addWork(fixture, "immutable attention subject");
    await startWork(fixture, work, "worker-session");
    for (const text of ["failed: first", "failed: second", "failed: third"]) {
      await runCli(fixture, ["work", "note", work, text], session("worker-session"));
    }
    await runCli(fixture, ["decision", "draft", "unchanged draft", "--work", work]);
    const snapshot = () => {
      const database = openDatabase(fixture);
      try {
        return {
          work: JSON.stringify(database.query("SELECT * FROM work ORDER BY id").all()),
          decisions: JSON.stringify(database.query("SELECT * FROM decisions ORDER BY id").all()),
        };
      } finally {
        database.close();
      }
    };

    const beforeInstall = snapshot();
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(snapshot()).toEqual(beforeInstall);

    const beforeAttention = snapshot();
    expect((await runCli(fixture, ["attention"], session("scanner-session"))).exitCode).toBe(0);
    expect(snapshot()).toEqual(beforeAttention);
  });
});

test("151 MAESTRO_SESSION_NONE records no session while status remains observable", async () => {
  await withFixture(async (fixture) => {
    await recordSession(fixture, "existing-session");
    const none = { ...session("ignored-session"), MAESTRO_SESSION_NONE: "1" };
    const status = await runCli(fixture, ["status"], none);
    expect(status.exitCode).toBe(0);
    const database = openDatabase(fixture);
    try {
      expect(
        database.query<{ count: number }, []>(
          "SELECT count(*) AS count FROM sessions WHERE id = 'ignored-session'",
        ).get()?.count,
      ).toBe(0);
    } finally {
      database.close();
    }
  });
});
