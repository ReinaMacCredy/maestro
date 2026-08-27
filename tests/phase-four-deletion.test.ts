import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  runInstalledCliAt,
  type CliResult,
  type Fixture,
  withFixture,
} from "./helpers.ts";

interface HookConfig {
  hooks: Record<string, Array<{
    hooks: Array<{ command: string; type: "command" }>;
    matcher?: string;
  }>>;
}

const attentionKinds = [
  "DECISION_STALE",
  "DISPATCH_UNRETURNED",
  "REPEATED_FAILURE",
  "SCOPE_COLLISION",
  "STALLED_LEASE",
] as const;

function errorFrom(result: CliResult): { code: string; message: string; verb?: string } {
  return (
    JSON.parse(result.stderr) as {
      error: { code: string; message: string; verb?: string };
    }
  ).error;
}

function openDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"));
}

async function addWork(fixture: Fixture, title: string, parent?: string): Promise<string> {
  return idFrom(
    await runCli(fixture, [
      "work",
      "add",
      title,
      "--atomic-reason",
      "phase four fixture",
      ...(parent ? ["--parent", parent] : []),
    ]),
  );
}

async function startWork(fixture: Fixture, work: string, holder: string): Promise<void> {
  const started = await runCli(fixture, ["work", "start", work], {
    MAESTRO_SESSION_ID: holder,
    MAESTRO_SESSION_PID: String(process.pid),
  });
  expect(started.exitCode).toBe(0);
}

async function seedRetiredHook(path: string, command: string): Promise<void> {
  const config = JSON.parse(await readFile(path, "utf8")) as HookConfig;
  config.hooks.PostToolUse = [
    {
      matcher: "foreign",
      hooks: [{ type: "command", command: "foreign-post-tool-hook" }],
    },
    { hooks: [{ type: "command", command }] },
  ];
  await writeFile(path, `${JSON.stringify(config, null, 2)}\n`);
}

test("256 msg send is absent and returns the ordinary UNKNOWN_VERB error", async () => {
  await withFixture(async (fixture) => {
    const legacy = openDatabase(fixture);
    legacy.exec(`
      CREATE TABLE messages (id TEXT);
      CREATE TABLE message_cursors (id TEXT);
    `);
    legacy.close();

    const sent = await runCli(fixture, ["msg", "send", "former-peer", "handoff context"]);
    const error = errorFrom(sent);

    expect(sent.exitCode).toBe(2);
    expect(error.code).toBe("UNKNOWN_VERB");
    expect(error.verb).toBe("msg");
    expect(error.message).toContain("unknown verb: msg");

    const database = openDatabase(fixture);
    try {
      const retiredTables = database
        .query<{ name: string }, []>(
          "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('messages', 'message_cursors') ORDER BY name",
        )
        .all();
      expect(retiredTables).toEqual([]);
    } finally {
      database.close();
    }
  });
});

test("257 agents outside Herdr still record SessionStart and UserPromptSubmit without a mailbox brief", async () => {
  await withFixture(async (fixture) => {
    const environment = {
      HERDR_ENV: undefined,
      MAESTRO_SESSION_ID: "outside-pane-codex",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const started = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
      environment,
    );
    const prompted = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit", "--harness", "codex"],
      environment,
      JSON.stringify({ prompt: "record this prompt outside a pane" }),
    );
    const status = await runCli(fixture, ["status", "--json"]);
    const listed = await runCli(fixture, ["prompt", "list", "--session", "outside-pane-codex"]);
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ harness: string | null; id: string; lastEvent: string }> };
    }).data.sessions;

    expect(started.exitCode).toBe(0);
    expect(started.stdout).not.toContain("pending message");
    expect(prompted.exitCode).toBe(0);
    expect(sessions).toContainEqual(
      expect.objectContaining({
        harness: "codex",
        id: "outside-pane-codex",
        lastEvent: "UserPromptSubmit",
      }),
    );
    expect(listed.stdout).toContain("record this prompt outside a pane");
  });
});

test("258 install omits PostToolUse and re-install removes only its retired managed hooks", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const first = await runCli(fixture, ["install"], { PATH: path });
    const claudePath = join(fixture.repo, ".claude", "settings.json");
    const codexPath = join(fixture.repo, ".codex", "hooks.json");
    const firstClaude = JSON.parse(await readFile(claudePath, "utf8")) as HookConfig;
    const firstCodex = JSON.parse(await readFile(codexPath, "utf8")) as HookConfig;

    await seedRetiredHook(claudePath, "bun .claude/hooks/maestro-record.ts");
    await seedRetiredHook(codexPath, "bun .codex/hooks/maestro-record.ts");
    const repeated = await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path });
    const repeatedClaude = JSON.parse(await readFile(claudePath, "utf8")) as HookConfig;
    const repeatedCodex = JSON.parse(await readFile(codexPath, "utf8")) as HookConfig;
    const workSkillRoot = join(fixture.home, "maestro", "skills", "maestro-work");
    const workSkill = await readFile(join(workSkillRoot, "SKILL.md"), "utf8");
    const conflictHandoff = await readFile(
      join(workSkillRoot, "references", "conflict-handoff.md"),
      "utf8",
    );
    const worktree = await readFile(join(workSkillRoot, "references", "worktree.md"), "utf8");

    expect(first.exitCode).toBe(0);
    expect(repeated.exitCode).toBe(0);
    expect(firstClaude.hooks.PostToolUse).toBeUndefined();
    expect(firstCodex.hooks.PostToolUse).toBeUndefined();
    expect(workSkill).toMatch(/<!-- maestro-skill-version: [0-9a-f]{40} -->/);
    for (const reference of [conflictHandoff, worktree]) {
      expect(reference).toContain("herdr pane send-text");
      expect(reference).toContain("maestro dispatch");
      expect(reference).toContain("maestro handback");
      expect(reference).not.toContain("maestro msg");
    }
    for (const config of [repeatedClaude, repeatedCodex]) {
      expect(config.hooks.SessionStart).toBeArray();
      expect(config.hooks.UserPromptSubmit).toBeArray();
      expect(config.hooks.PostToolUse).toEqual([
        {
          matcher: "foreign",
          hooks: [{ type: "command", command: "foreign-post-tool-hook" }],
        },
      ]);
    }
  });
});

test("259 supervisor start is absent and returns the ordinary UNKNOWN_VERB error", async () => {
  await withFixture(async (fixture) => {
    const started = await runCli(fixture, ["supervisor", "start"]);

    expect(started.exitCode).toBe(2);
    const error = errorFrom(started);
    expect(error.code).toBe("UNKNOWN_VERB");
    expect(error.verb).toBe("supervisor");
    expect(error.message).toContain("unknown verb: supervisor");
  });
});

test("260 all five attention kinds are computed at read time without mailbox state", async () => {
  await withFixture(async (fixture) => {
    const stalled = await addWork(fixture, "stalled lane");
    await startWork(fixture, stalled, "stalled-holder");

    const repeated = await addWork(fixture, "repeatedly failing lane");
    await startWork(fixture, repeated, "failure-holder");
    for (const note of ["failed: first", "failed: second", "failed: third"]) {
      expect(
        (
          await runCli(fixture, ["work", "note", repeated, note], {
            MAESTRO_SESSION_ID: "failure-holder",
            MAESTRO_SESSION_PID: String(process.pid),
          })
        ).exitCode,
      ).toBe(0);
    }

    const decisionWork = await addWork(fixture, "owner decision needed");
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "choose the boundary", "--work", decisionWork]),
    );

    const collisionParent = await addWork(fixture, "shared mutation scope");
    await startWork(fixture, collisionParent, "lead-holder");
    const collisionA = await addWork(fixture, "first colliding lane", collisionParent);
    const collisionB = await addWork(fixture, "second colliding lane", collisionParent);
    await startWork(fixture, collisionA, "collision-a");
    await startWork(fixture, collisionB, "collision-b");

    const dispatchWork = await addWork(fixture, "unreturned lane");
    const opened = await runCli(fixture, [
        "dispatch",
        "open",
        dispatchWork,
        "--objective",
        "return the result",
        "--owned-scope",
        "scratch",
        "--excluded-scope",
        "product source",
        "--mutation",
        "no-write",
        "--stop-condition",
        "handback filed",
        "--lane",
        "delivery",
        "--evidence-required",
        "source",
        "--pane",
        "w1:pZ",
      ]);
    expect(opened.exitCode).toBe(0);
    const dispatch = opened.stdout.match(/\b(x\d+)\b/)?.[1];
    expect(dispatch).toBeDefined();

    const database = openDatabase(fixture);
    try {
      database
        .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
        .run(new Date(Date.now() - 31 * 60_000).toISOString(), "stalled-holder");
      database
        .query("UPDATE decisions SET created_at = ? WHERE id = ?")
        .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
      database
        .query("UPDATE dispatches SET created_at = ? WHERE id = ?")
        .run(new Date(Date.now() - 3 * 60 * 60_000).toISOString(), dispatch as string);
    } finally {
      database.close();
    }

    const attention = await runCli(
      fixture,
      ["attention", "--json"],
      { MAESTRO_READ_ONLY: "1" },
    );
    const detections = (JSON.parse(attention.stdout) as {
      data: {
        detections: Array<{
          kind: string;
          raised: boolean;
          raisedAt: string;
          targets?: string[];
        }>;
      };
    }).data.detections;
    const observedKinds = [...new Set(detections.map((detection) => detection.kind))].sort();
    const after = openDatabase(fixture);
    try {
      const attentionRows = after
        .query<{ count: number }, []>("SELECT count(*) AS count FROM attention")
        .get()?.count;

      expect(attention.exitCode).toBe(0);
      expect(observedKinds).toEqual([...attentionKinds].sort());
      expect(detections.every((detection) => detection.raised === false)).toBe(true);
      expect(detections.every((detection) => detection.raisedAt === "not recorded (read-only)"))
        .toBe(true);
      expect(detections.every((detection) => detection.targets === undefined)).toBe(true);
      expect(attentionRows).toBe(0);
    } finally {
      after.close();
    }
  });
});

test("261 watch is absent and returns the ordinary UNKNOWN_VERB error", async () => {
  await withFixture(async (fixture) => {
    const watched = await runCli(fixture, ["watch", "--once"]);

    expect(watched.exitCode).toBe(2);
    const error = errorFrom(watched);
    expect(error.code).toBe("UNKNOWN_VERB");
    expect(error.verb).toBe("watch");
    expect(error.message).toContain("unknown verb: watch");
  });
});

test("262 status --live filters dead sessions while bare status remains unchanged", async () => {
  await withFixture(async (fixture) => {
    for (const [id, pid] of [
      ["live-status", process.pid],
      ["dead-status", 2_147_483_647],
    ] as const) {
      const recorded = await runCli(
        fixture,
        ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
        { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) },
      );
      expect(recorded.exitCode).toBe(0);
    }

    const all = await runCli(fixture, ["status"]);
    const live = await runCli(fixture, ["status", "--live"]);
    const liveJson = await runCli(fixture, ["status", "--live", "--json"]);

    expect(all.exitCode).toBe(0);
    expect(all.stdout).toContain("live-status [live]");
    expect(all.stdout).toContain("dead-status [dead]");
    expect(live.exitCode).toBe(0);
    expect(live.stdout).toContain("live-status [live]");
    expect(live.stdout).not.toContain("dead-status");
    expect(liveJson.exitCode).toBe(0);
    const sessions = (JSON.parse(liveJson.stdout) as {
      data: { sessions: Array<{ id: string; live: boolean }> };
    }).data.sessions;
    expect(sessions.some((session) => session.id === "live-status" && session.live)).toBe(true);
    expect(sessions.some((session) => session.id === "dead-status")).toBe(false);
  });
});
