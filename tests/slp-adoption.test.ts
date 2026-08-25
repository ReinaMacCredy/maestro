import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  type Fixture,
  withFixture,
} from "./helpers.ts";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

async function addWork(fixture: Fixture, title: string, parent?: string): Promise<string> {
  const args = ["work", "add", title, "--atomic-reason", "slp fixture"];
  if (parent) args.push("--parent", parent);
  return idFrom(await runCli(fixture, args));
}

async function startWork(fixture: Fixture, id: string, holder: string): Promise<void> {
  const started = await runCli(fixture, ["work", "start", id], session(holder));
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

async function waitFor<T>(
  read: () => T | Promise<T>,
  accept: (value: T) => boolean,
  timeoutMs = 5_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let value = await read();
  while (!accept(value) && Date.now() < deadline) {
    await Bun.sleep(50);
    value = await read();
  }
  expect(accept(value)).toBe(true);
  return value;
}

test("133 install materializes the dispatch, handback, dependency, and episode contracts", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const skill = await Bun.file(
      join(fixture.home, ".agents", "skills", "maestro-work", "SKILL.md"),
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

    const verifyRoot = join(fixture.home, ".agents", "skills", "maestro-verify");
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

    const skillsRoot = join(fixture.home, ".agents", "skills");
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
        "intake: problem in one sentence; uncertainty -> lane (scout no-write | decision x2-3 | delivery | challenge); ROI 0-10 -> tier\n",
    );

    const prompt = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session,
    );
    expect(prompt.exitCode).toBe(0);
    expect(prompt.stdout).toBe(
      "held work: none\n" +
        "enabled policies: policy-breakdown, policy-lifecycle, policy-proof\n" +
        "0 pending messages\n" +
        "next: maestro ready\n" +
        "recipes: maestro recipe list; maestro recipe show <name>\n",
    );
  });
});

test("138 attention raises and routes a STALLED_LEASE packet to the parent holder", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "parent scope");
    await startWork(fixture, parent, "lead-session");
    const child = await addWork(fixture, "stalled child", parent);
    await startWork(fixture, child, "subject-session");
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
          target_session: string;
        }, []>("SELECT fingerprint, packet, target_session FROM attention")
        .get();
      const start = database
        .query<{ id: number }, [string]>(
          "SELECT id FROM event_log WHERE type = 'work.start' AND entity_id = ? ORDER BY id DESC LIMIT 1",
        )
        .get(child);
      expect(row?.fingerprint).toBe(`stalled:${child}:${start?.id}`);
      expect(row?.target_session).toBe("lead-session");
      expect(row?.packet).toContain("unknown:");
      const message = database
        .query<{ target_session: string; text: string }, []>(
          "SELECT target_session, text FROM messages ORDER BY id DESC LIMIT 1",
        )
        .get();
      expect(message?.target_session).toBe("lead-session");
      expect(message?.text).toBe(row?.packet);
    } finally {
      database.close();
    }
  });
});

test("139 attention broadcasts without a parent holder and falls back to the subject", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "unheld parent");
    const child = await addWork(fixture, "broadcast child", parent);
    await startWork(fixture, child, "subject-session");
    await recordSession(fixture, "peer-one");
    await recordSession(fixture, "peer-two");
    backdateSession(fixture, "subject-session", 45);

    expect(
      (await runCli(fixture, ["attention"], session("scanner-session"))).exitCode,
    ).toBe(0);
    const database = openDatabase(fixture);
    try {
      const targets = database
        .query<{ target_session: string }, []>(
          "SELECT target_session FROM messages ORDER BY target_session",
        )
        .all()
        .map((row) => row.target_session);
      expect(targets).toEqual(["peer-one", "peer-two"]);
      expect(database.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count)
        .toBe(1);
    } finally {
      database.close();
    }
  });

  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "unheld parent");
    const child = await addWork(fixture, "subject fallback", parent);
    await startWork(fixture, child, "only-subject");
    backdateSession(fixture, "only-subject", 45);

    expect(
      (await runCli(fixture, ["attention"], session("scanner-session"))).exitCode,
    ).toBe(0);
    const database = openDatabase(fixture);
    try {
      expect(
        database
          .query<{ target_session: string }, []>(
            "SELECT target_session FROM messages ORDER BY id DESC LIMIT 1",
          )
          .get()?.target_session,
      ).toBe("only-subject");
      expect(database.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count)
        .toBe(1);
    } finally {
      database.close();
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
      expect(database.query<{ count: number }, []>("SELECT count(*) AS count FROM messages").get()?.count)
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
    ["failed:first", "Failed: second", "ordinary failure"],
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

test("143 attention raises sorted SCOPE_COLLISION and routes it to the parent holder", async () => {
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
        .query<{ fingerprint: string; target_session: string }, []>(
          "SELECT fingerprint, target_session FROM attention WHERE kind = 'SCOPE_COLLISION'",
        )
        .get();
      expect(row?.fingerprint).toBe(`collision:${first}:${second}:holder-a:holder-z`);
      expect(row?.target_session).toBe("lead-session");
      expect(
        database.query<{ target_session: string }, []>(
          "SELECT target_session FROM messages ORDER BY id DESC LIMIT 1",
        ).get()?.target_session,
      ).toBe("lead-session");
    } finally {
      database.close();
    }
  });
});

test("144 attention --json works without a supervisor pid file", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "json failure");
    await startWork(fixture, work, "worker-session");
    for (const text of ["failed: first", "failed: second", "failed: third"]) {
      await runCli(fixture, ["work", "note", work, text], session("worker-session"));
    }
    expect(await Bun.file(join(fixture.repo, ".maestro", "supervisor.json")).exists()).toBe(false);

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
  });
});

test("145 supervisor start, status, and stop own one advancing pid file", async () => {
  await withFixture(async (fixture) => {
    const statePath = join(fixture.repo, ".maestro", "supervisor.json");
    const controller = session("daemon-controller");
    let daemonStarted = false;
    try {
      const started = await runCli(
        fixture,
        ["supervisor", "start", "--interval", "1"],
        controller,
      );
      daemonStarted = started.exitCode === 0;
      expect(started.exitCode).toBe(0);

      const first = await waitFor(
        async () =>
          (await Bun.file(statePath).exists())
            ? JSON.parse(await Bun.file(statePath).text()) as { lastTick: string | null; pid: number }
            : null,
        (state) => state !== null && typeof state.lastTick === "string",
      );
      expect(first?.pid).toBeGreaterThan(1);
      const second = await waitFor(
        async () => JSON.parse(await Bun.file(statePath).text()) as { lastTick: string | null },
        (state) => state.lastTick !== first?.lastTick,
      );
      expect(second.lastTick).not.toBe(first?.lastTick);

      const status = await runCli(fixture, ["supervisor", "status"], controller);
      expect(status.exitCode).toBe(0);
      expect(status.stdout).toContain("supervisor running");
      expect(status.stdout).toContain(`pid: ${first?.pid}`);
      expect(status.stdout).toContain("interval: 1s");
      expect(status.stdout).toContain("last tick:");
      expect(status.stdout).toContain("daemon commit: source");

      const refused = await runCli(
        fixture,
        ["supervisor", "start", "--interval", "1"],
        controller,
      );
      expect(refused.exitCode).not.toBe(0);
      expect(refused.stderr).toContain("SUPERVISOR_RUNNING");
      expect(refused.stderr).toContain(`pid: ${first?.pid}`);
    } finally {
      if (daemonStarted || (await Bun.file(statePath).exists())) {
        const stopped = await runCli(fixture, ["supervisor", "stop"], controller);
        expect(stopped.exitCode).toBe(0);
      }
    }

    expect(await Bun.file(statePath).exists()).toBe(false);
    const status = await runCli(fixture, ["supervisor", "status"], controller);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toBe("supervisor stopped\n");
  });
});

test("146 supervisor reports a killed daemon as stale and permits replacement", async () => {
  await withFixture(async (fixture) => {
    const statePath = join(fixture.repo, ".maestro", "supervisor.json");
    const controller = session("daemon-controller");
    let replacementStarted = false;
    try {
      expect(
        (await runCli(fixture, ["supervisor", "start", "--interval", "1"], controller)).exitCode,
      ).toBe(0);
      const state = await waitFor(
        async () =>
          (await Bun.file(statePath).exists())
            ? JSON.parse(await Bun.file(statePath).text()) as { pid: number }
            : null,
        (value) => value !== null,
      );
      process.kill(state?.pid as number, "SIGKILL");

      const stale = await waitFor(
        () => runCli(fixture, ["supervisor", "status"], controller),
        (result) => result.stdout.includes("supervisor stale"),
      );
      expect(stale.stdout).toContain(`pid: ${state?.pid}`);
      expect(stale.stdout).toContain("run: maestro supervisor stop");

      const replacement = await runCli(
        fixture,
        ["supervisor", "start", "--interval", "1"],
        controller,
      );
      expect(replacement.exitCode).toBe(0);
      replacementStarted = true;
      const replacementState = JSON.parse(await Bun.file(statePath).text()) as { pid: number };
      expect(replacementState.pid).not.toBe(state?.pid);
    } finally {
      if (replacementStarted || (await Bun.file(statePath).exists())) {
        await runCli(fixture, ["supervisor", "stop"], controller);
      }
    }
  });
});

test("147 daemon ticks deliver as supervisor without creating a daemon session", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "daemon parent");
    await startWork(fixture, parent, "lead-session");
    const child = await addWork(fixture, "daemon stalled child", parent);
    await startWork(fixture, child, "subject-session");
    backdateSession(fixture, "subject-session", 45);
    const controller = session("daemon-controller");

    try {
      const started = await runCli(
        fixture,
        ["supervisor", "start", "--interval", "1"],
        controller,
      );
      expect(started.exitCode).toBe(0);
      const message = await waitFor(
        () => {
          const database = openDatabase(fixture);
          try {
            return database
              .query<{ sender_session: string; target_session: string }, []>(
                "SELECT sender_session, target_session FROM messages ORDER BY id DESC LIMIT 1",
              )
              .get() ?? null;
          } finally {
            database.close();
          }
        },
        (value) => value?.sender_session === "supervisor",
      );
      expect(message?.target_session).toBe("lead-session");

      const database = openDatabase(fixture);
      try {
        const ids = database
          .query<{ id: string }, []>("SELECT id FROM sessions ORDER BY id")
          .all()
          .map((row) => row.id);
        expect(ids).not.toContain("supervisor");
        expect(ids).toEqual(["lead-session", "subject-session"]);
      } finally {
        database.close();
      }
    } finally {
      await runCli(fixture, ["supervisor", "stop"], controller);
    }
  });
});

test("148 install and uninstall manage idempotent PostToolUse wiring for both harnesses", async () => {
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
      expect(config.hooks.PostToolUse).toHaveLength(1);
      const group = config.hooks.PostToolUse?.[0];
      expect(group).not.toHaveProperty("matcher");
      expect(group?.hooks).toHaveLength(1);
      expect(group?.hooks[0]).not.toHaveProperty("statusMessage");
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

test("149 PostToolUse is a mailbox-only JSON fast path that refreshes last_seen", async () => {
  await withFixture(async (fixture) => {
    await recordSession(fixture, "post-session");
    backdateSession(fixture, "post-session", 10);
    const database = openDatabase(fixture);
    const eventCount = () =>
      database.query<{ count: number }, []>("SELECT count(*) AS count FROM event_log").get()?.count ?? 0;
    const lastSeen = () =>
      database.query<{ last_seen: string }, []>(
        "SELECT last_seen FROM sessions WHERE id = 'post-session'",
      ).get()?.last_seen ?? "";
    try {
      const beforeEmpty = eventCount();
      const oldLastSeen = lastSeen();
      const empty = await runCli(
        fixture,
        ["hook", "record", "--event", "PostToolUse"],
        session("post-session"),
      );
      expect(empty.exitCode).toBe(0);
      expect(empty.stdout).toBe("");
      expect(eventCount()).toBe(beforeEmpty);
      expect(Date.parse(lastSeen())).toBeGreaterThan(Date.parse(oldLastSeen));

      expect(
        (await runCli(fixture, ["msg", "send", "post-session", "pending attention"])).exitCode,
      ).toBe(0);
      const beforePending = eventCount();
      const delivered = await runCli(
        fixture,
        ["hook", "record", "--event", "PostToolUse"],
        session("post-session"),
      );
      expect(delivered.exitCode).toBe(0);
      const line = delivered.stdout.trimEnd();
      expect(delivered.stdout).toBe(`${line}\n`);
      expect(line).not.toContain("\n");
      const output = JSON.parse(line) as {
        hookSpecificOutput: { additionalContext: string; hookEventName: string };
      };
      expect(output.hookSpecificOutput.hookEventName).toBe("PostToolUse");
      expect(output.hookSpecificOutput.additionalContext).toContain("pending attention");
      expect(eventCount()).toBe(beforePending);
      expect(
        database.query<{ last_message_id: number }, []>(
          "SELECT last_message_id FROM message_cursors WHERE session_id = 'post-session'",
        ).get()?.last_message_id,
      ).toBeGreaterThan(0);

      const drained = await runCli(
        fixture,
        ["hook", "record", "--event", "PostToolUse"],
        session("post-session"),
      );
      expect(drained.stdout).toBe("");
      expect(eventCount()).toBe(beforePending);
    } finally {
      database.close();
    }
  });
});

test("150 install stays inert and attention plus a daemon tick preserve work and decisions", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const statePath = join(fixture.repo, ".maestro", "supervisor.json");
    expect(await Bun.file(statePath).exists()).toBe(false);
    expect((await runCli(fixture, ["supervisor", "status"])).stdout).toBe(
      "supervisor stopped\n",
    );

    const work = await addWork(fixture, "immutable supervisor subject");
    await startWork(fixture, work, "worker-session");
    for (const text of ["failed: first", "failed: second", "failed: third"]) {
      await runCli(fixture, ["work", "note", work, text], session("worker-session"));
    }
    await runCli(fixture, ["decision", "draft", "unchanged draft", "--work", work]);
    const database = openDatabase(fixture);
    const snapshot = () => ({
      work: JSON.stringify(database.query("SELECT * FROM work ORDER BY id").all()),
      decisions: JSON.stringify(database.query("SELECT * FROM decisions ORDER BY id").all()),
    });
    try {
      const before = snapshot();
      expect((await runCli(fixture, ["attention"], session("scanner-session"))).exitCode).toBe(0);
      const controller = session("daemon-controller");
      try {
        expect(
          (await runCli(fixture, ["supervisor", "start", "--interval", "1"], controller)).exitCode,
        ).toBe(0);
        await waitFor(
          async () =>
            (await Bun.file(statePath).exists())
              ? JSON.parse(await Bun.file(statePath).text()) as { lastTick: string | null }
              : null,
          (state) => typeof state?.lastTick === "string",
        );
      } finally {
        await runCli(fixture, ["supervisor", "stop"], controller);
      }
      expect(snapshot()).toEqual(before);
    } finally {
      database.close();
    }
  });
});

test("151 MAESTRO_SESSION_NONE records no session and sends messages as supervisor", async () => {
  await withFixture(async (fixture) => {
    await recordSession(fixture, "existing-session");
    const none = { ...session("ignored-session"), MAESTRO_SESSION_NONE: "1" };
    const status = await runCli(fixture, ["status"], none);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).not.toContain("supervisor");
    const sent = await runCli(
      fixture,
      ["msg", "send", "existing-session", "supervised delivery"],
      none,
    );
    expect(sent.exitCode).toBe(0);

    const database = openDatabase(fixture);
    try {
      expect(
        database.query<{ count: number }, []>(
          "SELECT count(*) AS count FROM sessions WHERE id = 'supervisor'",
        ).get()?.count,
      ).toBe(0);
      expect(
        database.query<{ sender_session: string }, []>(
          "SELECT sender_session FROM messages ORDER BY id DESC LIMIT 1",
        ).get()?.sender_session,
      ).toBe("supervisor");
    } finally {
      database.close();
    }
  });
});
