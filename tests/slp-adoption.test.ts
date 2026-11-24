import { Database } from "bun:sqlite";
import { expect, setDefaultTimeout, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  runTool,
  type Fixture,
  withFixture,
} from "./helpers.ts";

setDefaultTimeout(15_000);

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
      "`maestro-council`",
    ]) {
      expect(design).toContain(required);
    }
    // w625: the eight reconcile axes moved from maestro-design's Council
    // section, now a pointer, into maestro-council's Lead draft.
    const council = await Bun.file(join(skillsRoot, "maestro-council", "SKILL.md")).text();
    const leadDraft = council.slice(council.indexOf("7. Draft the verdict"), council.indexOf("8. Audit the draft"));
    for (const axis of [
      "premise",
      "mechanism",
      "boundary",
      "failure",
      "reversibility",
      "evidence",
      "authority",
      "proof",
    ]) {
      expect(leadDraft).toContain(`\`${axis}\``);
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

test("137 SessionStart adds only the intake line and UserPromptSubmit carries session state alone", async () => {
  await withFixture(async (fixture) => {
    const session = {
      MAESTRO_SESSION_ID: "slp-brief",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const start = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    expect(start.exitCode).toBe(0);
    expect(start.stdout).toContain(
      "intake: problem in one sentence; route by ~/maestro/WORKFLOW.md; optional lane (scout no-write | decision x2-3 | delivery | challenge | shadow no-write); a finding is not work: doctrine correction -> maestro lesson file, code finding -> review doc or handback, work add only for the next thing being built\n",
    );

    const prompt = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session,
    );
    expect(prompt.exitCode).toBe(0);
    // The policy list, the next verb and the recipe pointer are stated once on
    // SessionStart; a prompt-time brief carries only what changes between turns.
    expect(prompt.stdout).toBe("held work: none\n");
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
    expect(
      (await runCli(fixture, ["decision", "draft", "owner fork without recipient", "--needs-owner", "--work", work]))
        .exitCode,
    ).toBe(0);

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner-session"));
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
    const work = await addWork(fixture, "deduplicated failure");
    await startWork(fixture, work, "subject-session");
    for (const text of ["failed: first", "failed: second", "failed: third"]) {
      expect((await runCli(fixture, ["work", "note", work, text], session("subject-session"))).exitCode)
        .toBe(0);
    }

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

    const hook = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session("scanner-session"),
    );
    expect(hook.exitCode).toBe(0);
    expect(hook.stdout).toContain(`attention HUMAN_DECISION_REQUIRED decision ${marked}`);
  });
});

test("453 withdrawn drafts raise no human-decision attention", async () => {
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
    expect((await runCli(fixture, [
      "decision",
      "withdraw",
      decision,
      "--reason",
      "a different option was locked",
    ])).exitCode).toBe(0);

    const attention = await runCli(
      fixture,
      ["attention", "--json"],
      session("scanner-session"),
    );
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: { detections: Array<{ kind: string; packet: string }> };
    }).data.detections;
    expect(
      detections
        .filter((detection) => detection.kind === "HUMAN_DECISION_REQUIRED")
        .map((detection) => detection.packet),
    ).not.toContainEqual(expect.stringContaining(`decision ${decision}`));
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

test("148 install retires maestro's Claude PreToolUse group and keeps foreign ones (d875)", async () => {
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
      expect(config.hooks.PreToolUse).toBeUndefined();
      expect(config.hooks.PostToolUse).toBeUndefined();
    }

    const staleManaged = {
      matcher: "Agent|Task",
      hooks: [{ type: "command", command: "bun \"${CLAUDE_PROJECT_DIR:-.}/.claude/hooks/maestro-record.ts\"" }],
    };
    const foreignPreToolUse = {
      matcher: "Write",
      hooks: [{ type: "command", command: "foreign-pre-tool-hook" }],
    };
    firstClaude.hooks.PreToolUse = [foreignPreToolUse, staleManaged];
    await Bun.write(claudePath, `${JSON.stringify(firstClaude, null, 2)}\n`);

    const second = await runCli(fixture, ["install"], { PATH: path });
    expect(second.exitCode).toBe(0);
    const secondClaudeText = await Bun.file(claudePath).text();
    const secondClaude = JSON.parse(secondClaudeText) as HookConfig;
    expect(secondClaude.hooks.PreToolUse).toEqual([foreignPreToolUse]);
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

test("pstack-port: a stale PreToolUse hook from an older install records nothing and prints nothing (d875)", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "held while a sub-agent spawns");
    await startWork(fixture, work, "stale-hook-holder");
    const lastEvent = async (): Promise<string | undefined> => {
      const status = await runCli(fixture, ["status", "--json"], session("stale-hook-holder"));
      const sessions = (JSON.parse(status.stdout) as {
        data: { sessions: Array<{ id: string; lastEvent: string }> };
      }).data.sessions;
      return sessions.find((candidate) => candidate.id === "stale-hook-holder")?.lastEvent;
    };
    const before = await lastEvent();
    const fired = await runCli(
      fixture,
      ["hook", "record", "--event", "PreToolUse", "--harness", "claude"],
      session("stale-hook-holder"),
      JSON.stringify({ tool_name: "Agent" }),
    );
    expect(fired.exitCode).toBe(0);
    expect(fired.stdout).toBe("");
    expect(await lastEvent()).toBe(before);
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
