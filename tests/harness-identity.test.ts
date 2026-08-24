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
          sessionEnvironment("unknown-session"),
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

test("B3.8 claude peers receive a native SendMessage tip and JSON delivery signal only while live", async () => {
  await withFixture(async (fixture) => {
    for (const [id, harness, pid] of [
      ["claude-sender", "claude", process.pid],
      ["claude-target", "claude", process.pid],
      ["codex-target", "codex", process.pid],
      ["dead-claude-target", "claude", 99_999_999],
      ["codex-sender", "codex", process.pid],
    ] as const) {
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart", "--harness", harness],
            sessionEnvironment(id, pid),
          )
        ).exitCode,
      ).toBe(0);
    }

    const human = await runCli(
      fixture,
      ["msg", "send", "claude-target", "urgent"],
      sessionEnvironment("claude-sender"),
    );
    expect(human.stdout).toContain("native SendMessage");
    expect(human.stdout).toContain("claude-target");

    const codexHuman = await runCli(
      fixture,
      ["msg", "send", "codex-target", "codex human"],
      sessionEnvironment("claude-sender"),
    );
    const deadHuman = await runCli(
      fixture,
      ["msg", "send", "dead-claude-target", "dead human"],
      sessionEnvironment("claude-sender"),
    );
    expect(codexHuman.stdout).not.toContain("[native-delivery]");
    expect(deadHuman.stdout).not.toContain("[native-delivery]");

    const codexSenderHuman = await runCli(
      fixture,
      ["msg", "send", "claude-target", "codex sender human"],
      sessionEnvironment("codex-sender"),
    );
    const codexSenderJson = await runCli(
      fixture,
      ["msg", "send", "claude-target", "codex sender json", "--json"],
      sessionEnvironment("codex-sender"),
    );
    expect(codexSenderHuman.stdout).not.toContain("[native-delivery]");
    expect(
      (JSON.parse(codexSenderJson.stdout) as { data: { nativeDelivery: boolean } }).data
        .nativeDelivery,
    ).toBeFalse();

    const nativeJson = await runCli(
      fixture,
      ["msg", "send", "claude-target", "urgent json", "--json"],
      sessionEnvironment("claude-sender"),
    );
    const codexJson = await runCli(
      fixture,
      ["msg", "send", "codex-target", "codex", "--json"],
      sessionEnvironment("claude-sender"),
    );
    const deadJson = await runCli(
      fixture,
      ["msg", "send", "dead-claude-target", "dead", "--json"],
      sessionEnvironment("claude-sender"),
    );
    expect((JSON.parse(nativeJson.stdout) as { data: { nativeDelivery: boolean } }).data.nativeDelivery).toBeTrue();
    expect((JSON.parse(codexJson.stdout) as { data: { nativeDelivery: boolean } }).data.nativeDelivery).toBeFalse();
    expect((JSON.parse(deadJson.stdout) as { data: { nativeDelivery: boolean } }).data.nativeDelivery).toBeFalse();
    expect(codexJson.stdout).not.toContain("SendMessage");
    expect(deadJson.stdout).not.toContain("SendMessage");
  });
});
