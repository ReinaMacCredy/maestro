import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, prepareInstallFixture, runCli, type Fixture, withFixture } from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) };
}

function probeDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"));
}

async function stalledLease(fixture: Fixture): Promise<void> {
  const parent = idFrom(
    await runCli(fixture, ["work", "add", "parent", "--atomic-reason", "probe fixture"]),
  );
  expect((await runCli(fixture, ["work", "start", parent], session("lead"))).exitCode).toBe(0);
  const child = idFrom(
    await runCli(fixture, ["work", "add", "child", "--parent", parent, "--kind", "task"]),
  );
  expect((await runCli(fixture, ["work", "start", child], session("subject", 1))).exitCode).toBe(0);
  const database = probeDatabase(fixture);
  try {
    database
      .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 45 * 60_000).toISOString(), "subject");
  } finally {
    database.close();
  }
}

test("220 no session may be recorded under the reserved system author id", async () => {
  await withFixture(async (fixture) => {
    const squatted = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("supervisor"),
    );
    expect(squatted.exitCode).not.toBe(0);
    expect(squatted.stderr).toContain("RESERVED_SESSION");

    const database = probeDatabase(fixture);
    try {
      expect(
        database
          .query<{ count: number }, [string]>("SELECT count(*) AS count FROM sessions WHERE id = ?")
          .get("supervisor")?.count,
      ).toBe(0);
    } finally {
      database.close();
    }
  });
});

test("221 mail addressed to the supervisor says it receives none", async () => {
  await withFixture(async (fixture) => {
    const sent = await runCli(fixture, ["msg", "send", "supervisor", "who are you?"], session("lead"));
    expect(sent.exitCode).not.toBe(0);
    expect(sent.stderr).toContain("SYSTEM_AUTHOR");
    expect(sent.stderr).toContain("does not receive mail");
    expect(sent.stderr).not.toContain("maestro status");
  });
});

test("222 the inbox marks a packet the supervisor authored", async () => {
  await withFixture(async (fixture) => {
    await stalledLease(fixture);
    expect((await runCli(fixture, ["attention"], session("scanner"))).exitCode).toBe(0);
    const database = probeDatabase(fixture);
    try {
      database.query("UPDATE messages SET sender_session = 'supervisor'").run();
    } finally {
      database.close();
    }

    const inbox = await runCli(fixture, ["msg", "read"], session("lead"));
    expect(inbox.exitCode).toBe(0);
    expect(inbox.stdout).toContain("from supervisor (system)");
  });
});

const legacySchema = `
  CREATE TABLE schema_version (version INTEGER NOT NULL);
  INSERT INTO schema_version (version) VALUES (1);
  CREATE TABLE cards (
    id TEXT PRIMARY KEY NOT NULL, card_type TEXT NOT NULL, parent TEXT, status TEXT NOT NULL,
    title TEXT NOT NULL, record_file TEXT NOT NULL, card_yaml TEXT NOT NULL,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, imported_at TEXT NOT NULL
  );
  INSERT INTO cards VALUES ('old-card', 'feature', NULL, 'shipped', 'Old card', 'card.yaml',
    'id: old-card', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
  CREATE TABLE card_files (
    card_id TEXT NOT NULL, path TEXT NOT NULL, mode INTEGER NOT NULL, contents BLOB NOT NULL,
    sha256 TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY (card_id, path)
  );
`;

function writeLegacyStore(fixture: Fixture): void {
  const legacy = new Database(join(fixture.repo, ".maestro", "store.sqlite"), { create: true });
  try {
    legacy.exec(legacySchema);
  } finally {
    legacy.close();
  }
}

test("223 doctor names the old store a migrating repo still carries", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const clean = await runCli(fixture, ["doctor"], { PATH: path });
    expect(clean.exitCode).toBe(0);
    expect(clean.stdout).not.toContain("legacy store");

    writeLegacyStore(fixture);
    const carrying = await runCli(fixture, ["doctor"], { PATH: path });
    expect(carrying.exitCode).toBe(0);
    expect(carrying.stdout).toContain("legacy store: 1 card(s) not imported");
    expect(carrying.stdout).toContain("maestro import rust");

    expect((await runCli(fixture, ["import", "rust"])).exitCode).toBe(0);
    const imported = await runCli(fixture, ["doctor"], { PATH: path });
    expect(imported.stdout).toContain("legacy store: imported");
    expect(imported.stdout).not.toContain("not imported");
  });
});

test("224 the import says how to read what it just imported", async () => {
  await withFixture(async (fixture) => {
    writeLegacyStore(fixture);
    const imported = await runCli(fixture, ["import", "rust"]);
    expect(imported.exitCode).toBe(0);
    expect(imported.stdout).toContain("read them: maestro legacy show old-card");
  });
});

test("225 the codex check asks about the hook that carries delivery", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const hooksPath = join(fixture.repo, ".codex", "hooks.json");
    const installed = JSON.parse(await Bun.file(hooksPath).text()) as
      { hooks: Record<string, unknown> };
    expect(Object.keys(installed.hooks)).toContain("PostToolUse");

    await mkdir(join(fixture.home, ".codex"), { recursive: true });
    // Trusting one unrelated event is what the owner's /hooks pass recorded.
    await writeFile(
      join(fixture.home, ".codex", "config.toml"),
      `[hooks.state."${hooksPath}:session_start:0:0"]\ntrusted_hash = "sha256:deadbeef"\n`,
    );
    const partial = await runCli(fixture, ["doctor"], { PATH: path });
    expect(partial.stdout).toContain("codex hooks: not trusted");

    await writeFile(
      join(fixture.home, ".codex", "config.toml"),
      `[hooks.state."${hooksPath}:session_start:0:0"]\ntrusted_hash = "sha256:deadbeef"\n` +
        `[hooks.state."${hooksPath}:post_tool_use:0:0"]\ntrusted_hash = "sha256:deadbeef"\n`,
    );
    const trusted = await runCli(fixture, ["doctor"], { PATH: path });
    expect(trusted.stdout).toContain("codex hooks: trusted");

    // A checkout wired by an older runtime declares no PostToolUse at all.
    delete installed.hooks.PostToolUse;
    await writeFile(hooksPath, JSON.stringify(installed, null, 2));
    const stale = await runCli(fixture, ["doctor"], { PATH: path });
    expect(stale.stdout).toContain("codex hooks: stale");
    expect(stale.stdout).toContain("maestro install");
  });
});

test("226 dead-session mail is discarded by advancing its cursor with an audit record", async () => {
  await withFixture(async (fixture) => {
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "SessionStart"],
          session("dead-target", 99999999),
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["msg", "send", "dead-target", "one"], session("lead"))).exitCode)
      .toBe(0);
    expect((await runCli(fixture, ["msg", "send", "dead-target", "two"], session("lead"))).exitCode)
      .toBe(0);

    const discarded = await runCli(
      fixture,
      ["msg", "discard", "dead-target", "--reason", "session was abandoned"],
      session("operator"),
    );
    expect(discarded.exitCode).toBe(0);
    expect(discarded.stdout).toContain("discarded 2 message(s) for dead-target");
    expect(discarded.stdout).toContain("reason: session was abandoned");

    const database = probeDatabase(fixture);
    try {
      expect(
        database
          .query<{ count: number }, []>("SELECT count(*) AS count FROM messages")
          .get()?.count,
      ).toBe(2);
      expect(
        database
          .query<{ last_message_id: number }, [string]>(
            "SELECT last_message_id FROM message_cursors WHERE session_id = ?",
          )
          .get("dead-target")?.last_message_id,
      ).toBe(2);
      const event = database
        .query<{ payload: string; session_id: string }, []>(
          "SELECT session_id, payload FROM event_log WHERE type = 'msg.discard' ORDER BY id DESC LIMIT 1",
        )
        .get();
      expect(event?.session_id).toBe("operator");
      expect(JSON.parse(event?.payload ?? "{}"))
        .toEqual({ count: 2, reason: "session was abandoned", throughMessageId: 2 });
    } finally {
      database.close();
    }

    expect((await runCli(fixture, ["msg", "read"], session("dead-target"))).stdout)
      .toContain("no new messages");
  });
});

test("227 msg discard refuses a live target and a blank reason", async () => {
  await withFixture(async (fixture) => {
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "SessionStart"],
          session("live-target"),
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["msg", "send", "live-target", "keep"], session("lead"))).exitCode)
      .toBe(0);

    const live = await runCli(
      fixture,
      ["msg", "discard", "live-target", "--reason", "not allowed"],
      session("operator"),
    );
    expect(live.exitCode).not.toBe(0);
    expect(live.stderr).toContain("SESSION_LIVE");

    const blank = await runCli(
      fixture,
      ["msg", "discard", "live-target", "--reason", "   "],
      session("operator"),
    );
    expect(blank.exitCode).not.toBe(0);
    expect(blank.stderr).toContain("MISSING_ARGUMENT");
  });
});

test("228 doctor names each dead mailbox target and the audited discard command", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "SessionStart"],
          session("doctor-dead", 99999999),
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["msg", "send", "doctor-dead", "one"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["msg", "send", "doctor-dead", "two"])).exitCode).toBe(0);

    const queued = await runCli(fixture, ["doctor"], { PATH: path });
    expect(queued.exitCode).toBe(0);
    expect(queued.stdout).toContain("mailbox: 2 message(s) queued for dead sessions");
    expect(queued.stdout).toContain("doctor-dead: 2");
    expect(queued.stdout).toContain("maestro msg discard <session> --reason <text>");
  });
});
