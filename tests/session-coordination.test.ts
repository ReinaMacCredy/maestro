import { expect, test } from "bun:test";
import { idFrom, recordedPrompts, runCli, storedSessions, withFixture } from "./helpers.ts";

test("13 hook record persists a session event reflected by status", async () => {
  await withFixture(async (fixture) => {
    const recorded = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
      MAESTRO_SESSION_ID: "codex-13",
      MAESTRO_SESSION_PID: String(process.pid),
    });
    const session = storedSessions(fixture).find((row) => row.id === "codex-13");

    expect(recorded.exitCode).toBe(0);
    expect(session?.last_event).toBe("SessionStart");
  });
});

test("14 SessionStart brief contains held work and no enabled-policies or next line (d911)", async () => {
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
    expect(brief.stdout).not.toContain("enabled policies");
    expect(brief.stdout).not.toContain("next:");
    expect(brief.stdout).not.toContain("attention");
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
    expect(start.stdout).toContain('maestro work done <id> --check "<command>"');
    expect(start.stdout).toContain("maestro topic path <slug>");
    expect(start.stdout).toContain("maestro bundle open <id> --work <workId>");
    expect(start.stdout).toContain("rules: ~/maestro/WORKFLOW.md");
    expect(start.stdout).not.toContain("copy its numbered steps");
    expect(prompt.exitCode).toBe(0);
    expect(prompt.stdout).not.toContain("tracked work");
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
    const found = await runCli(fixture, ["search", "flaky login retry"]);

    expect(submit.exitCode).toBe(0);
    expect(start.exitCode).toBe(0);
    const prompts = recordedPrompts(fixture, "prompt-session");
    expect(prompts).toContain("please fix the flaky login retry test");
    expect(prompts).not.toContain("session start noise");
    expect(found.exitCode).toBe(0);
    expect(found.stdout).toContain("(prompt");
    expect(found.stdout).toContain("flaky login retry");
  });
});
