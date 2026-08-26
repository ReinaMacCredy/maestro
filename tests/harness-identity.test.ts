import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

function sessionEnvironment(id: string, pid = process.pid): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(pid),
  };
}

const withoutHarnessEnvironment = Object.fromEntries(
  Object.keys(process.env)
    .filter((name) => name.startsWith("CLAUDE_") || name.startsWith("CODEX_"))
    .map((name) => [name, undefined]),
) as Record<string, undefined>;

test("B3.7 hook-recorded harness identity is shown while an absent harness remains null", async () => {
  await withFixture(async (fixture) => {
    const legacy = new Database(join(fixture.repo, ".maestro", "maestro.db"), { create: true });
    legacy.exec(`
      CREATE TABLE sessions (
        id TEXT PRIMARY KEY,
        pid INTEGER NOT NULL,
        last_event TEXT NOT NULL,
        last_seen TEXT NOT NULL
      );
      INSERT INTO sessions (id, pid, last_event, last_seen)
      VALUES ('legacy-session', ${process.pid}, 'SessionStart', '2026-08-23T00:00:00.000Z');
    `);
    legacy.close();
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "SessionStart", "--harness", "claude"],
          sessionEnvironment("claude-session"),
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "SessionStart"],
          { ...withoutHarnessEnvironment, ...sessionEnvironment("unknown-session") },
        )
      ).exitCode,
    ).toBe(0);

    const human = await runCli(fixture, ["status"]);
    const json = await runCli(fixture, ["status", "--json"]);
    const envelope = JSON.parse(json.stdout) as {
      data: { sessions: Array<{ harness: string | null; id: string }> };
    };
    expect(human.stdout).toContain("claude-session");
    expect(human.stdout).toContain("harness=claude");
    expect(envelope.data.sessions.find((session) => session.id === "claude-session")?.harness).toBe(
      "claude",
    );
    expect(envelope.data.sessions.find((session) => session.id === "unknown-session")?.harness).toBeNull();
    expect(envelope.data.sessions.find((session) => session.id === "legacy-session")?.harness).toBeNull();
  });
});

test("68 bare SessionStart preserves the Claude harness guessed from the environment", async () => {
  await withFixture(async (fixture) => {
    const sessionId = "bare-claude-session";
    const recorded = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      {
        ...withoutHarnessEnvironment,
        CLAUDE_CODE_ENTRYPOINT: "cli",
        MAESTRO_SESSION_ID: sessionId,
        MAESTRO_SESSION_PID: String(process.pid),
      },
    );
    const status = await runCli(fixture, ["status", "--json"]);
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ harness: string | null; id: string }> };
    }).data.sessions;

    expect(recorded.exitCode).toBe(0);
    expect(sessions.find((session) => session.id === sessionId)?.harness).toBe("claude");
  });
});
