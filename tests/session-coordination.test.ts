import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture } from "./helpers.ts";

test("13 hook record persists a session event reflected by status", async () => {
  await withFixture(async (fixture) => {
    const recorded = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
      MAESTRO_SESSION_ID: "codex-13",
      MAESTRO_SESSION_PID: String(process.pid),
    });
    const status = await runCli(fixture, ["status"]);

    expect(recorded.exitCode).toBe(0);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain("codex-13");
    expect(status.stdout).toContain("SessionStart");
  });
});

test("14 SessionStart brief contains held work, enabled policies, and pending message count", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "held item", "--kind", "idea"]));
    expect(
      (
        await runCli(fixture, ["work", "start", id], {
          MAESTRO_SESSION_ID: "brief-session",
          MAESTRO_SESSION_PID: String(process.pid),
        })
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["msg", "send", "brief-session", "check this"])).exitCode).toBe(0);

    const brief = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
      MAESTRO_SESSION_ID: "brief-session",
      MAESTRO_SESSION_PID: String(process.pid),
    });

    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(id);
    expect(brief.stdout).toContain("policy-proof");
    expect(brief.stdout).toContain("policy-breakdown");
    expect(brief.stdout).toMatch(/1 pending message/);
  });
});

test("15 message reads advance a per-session cursor and return each message once", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["msg", "send", "target-session", "handoff context"])).exitCode).toBe(0);

    const first = await runCli(fixture, ["msg", "read"], {
      MAESTRO_SESSION_ID: "target-session",
    });
    const second = await runCli(fixture, ["msg", "read"], {
      MAESTRO_SESSION_ID: "target-session",
    });

    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain("handoff context");
    expect(second.exitCode).toBe(0);
    expect(second.stdout).not.toContain("handoff context");
  });
});
