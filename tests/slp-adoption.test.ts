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
        "intake: problem in one sentence; uncertainty -> lane (scout no-write | decision x2-3 | delivery | challenge | shadow no-write); ROI 0-10 -> tier\n",
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
      `attention STALLED_LEASE ${child}`,
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
    expect(attention.stdout).toContain(`attention REPEATED_FAILURE ${work}`);

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
    expect(attention.stdout).toContain(`attention DECISION_STALE ${decision}`);
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
    expect(attention.stdout).toContain(`attention SCOPE_COLLISION ${first},${second}`);

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

test("148 install and uninstall manage only session and prompt hook wiring", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const first = await runCli(fixture, ["install"], { PATH: path });
    expect(first.exitCode).toBe(0);

    const hookPaths = [
      join(fixture.repo, ".claude", "settings.json"),
      join(fixture.repo, ".codex", "hooks.json"),
    ];
    const firstTexts = await Promise.all(hookPaths.map((hookPath) => Bun.file(hookPath).text()));
    for (const text of firstTexts) {
      const config = JSON.parse(text) as {
        hooks: Record<string, Array<{ hooks: Array<Record<string, unknown>>; matcher?: unknown }>>;
      };
      expect(config.hooks.SessionStart).toBeArray();
      expect(config.hooks.UserPromptSubmit).toBeArray();
      expect(config.hooks.PostToolUse).toBeUndefined();
    }

    const second = await runCli(fixture, ["install"], { PATH: path });
    expect(second.exitCode).toBe(0);
    expect(await Promise.all(hookPaths.map((hookPath) => Bun.file(hookPath).text()))).toEqual(
      firstTexts,
    );

    const uninstalled = await runCli(fixture, ["uninstall"], { PATH: path });
    expect(uninstalled.exitCode).toBe(0);
    for (const hookPath of hookPaths) {
      if (!(await Bun.file(hookPath).exists())) continue;
      const config = JSON.parse(await Bun.file(hookPath).text()) as {
        hooks?: Record<string, unknown>;
      };
      expect(config.hooks?.PostToolUse).toBeUndefined();
    }
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
