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

test("14 SessionStart brief contains held work and enabled policies without mailbox state", async () => {
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
    const brief = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
      MAESTRO_SESSION_ID: "brief-session",
      MAESTRO_SESSION_PID: String(process.pid),
    });

    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(id);
    expect(brief.stdout).toContain("policy-proof");
    expect(brief.stdout).toContain("policy-breakdown");
    expect(brief.stdout).not.toContain("pending message");
  });
});

test("14b method map renders on SessionStart only, not on UserPromptSubmit", async () => {
  await withFixture(async (fixture) => {
    const start = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
      MAESTRO_SESSION_ID: "map-session",
      MAESTRO_SESSION_PID: String(process.pid),
    });
    const prompt = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"], {
      MAESTRO_SESSION_ID: "map-session",
      MAESTRO_SESSION_PID: String(process.pid),
    });

    expect(start.exitCode).toBe(0);
    expect(start.stdout).toContain("method:");
    expect(start.stdout).toContain("bundle open");
    expect(start.stdout).toContain("decision draft");
    expect(prompt.exitCode).toBe(0);
    expect(prompt.stdout).not.toContain("method:");
    expect(prompt.stdout).toContain("held work");
  });
});

test("14c UserPromptSubmit records the prompt into a listable, searchable corpus", async () => {
  await withFixture(async (fixture) => {
    const submit = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      { MAESTRO_SESSION_ID: "prompt-session", MAESTRO_SESSION_PID: String(process.pid) },
      JSON.stringify({
        hook_event_name: "UserPromptSubmit",
        prompt: "please fix the flaky login retry test",
      }),
    );
    const start = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      { MAESTRO_SESSION_ID: "prompt-session", MAESTRO_SESSION_PID: String(process.pid) },
      JSON.stringify({ hook_event_name: "SessionStart", prompt: "session start noise" }),
    );
    const listed = await runCli(fixture, ["prompt", "list", "--session", "prompt-session"]);
    const found = await runCli(fixture, ["search", "flaky login retry"]);

    expect(submit.exitCode).toBe(0);
    expect(start.exitCode).toBe(0);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("please fix the flaky login retry test");
    expect(listed.stdout).not.toContain("session start noise");
    expect(found.exitCode).toBe(0);
    expect(found.stdout).toContain("(prompt");
    expect(found.stdout).toContain("flaky login retry");
  });
});
