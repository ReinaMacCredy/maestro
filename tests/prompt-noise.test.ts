import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

test("518 UserPromptSubmit ignores task-notification harness envelopes", async () => {
  await withFixture(async (fixture) => {
    const marker = "harnesspromptmarker";
    const submitted = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit", "--harness", "codex"],
      {},
      JSON.stringify({ prompt: `  <task-notification>\n<task-id>${marker}</task-id>` }),
    );
    const listed = await runCli(fixture, ["prompt", "list"]);
    const searched = await runCli(fixture, ["search", marker]);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    const promptCount = database
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM prompts")
      .get()?.count;
    database.close();

    expect(submitted.exitCode).toBe(0);
    expect(listed.stdout).toBe("no prompts recorded\n");
    expect(searched.stdout).toBe("");
    expect(promptCount).toBe(0);
  });
});

test("519 prompt migration drops task-notification rows and their search entries once", async () => {
  await withFixture(async (fixture) => {
    const realPrompt = "keep genuinepromptmarker for the owner";
    const noisePrompt =
      "<task-notification>\n<task-id>harnesspromptmarker</task-id>\n</task-notification>";
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "UserPromptSubmit", "--harness", "codex"],
          {},
          JSON.stringify({ prompt: realPrompt }),
        )
      ).exitCode,
    ).toBe(0);

    const path = join(fixture.repo, ".maestro", "maestro.db");
    const seeded = new Database(path);
    seeded
      .query("INSERT INTO prompts (session_id, text, created_at) VALUES (?, ?, ?)")
      .run("harness-session", noisePrompt, new Date().toISOString());
    seeded.close();

    const listed = await runCli(fixture, ["prompt", "list"]);
    const noiseSearch = await runCli(fixture, ["search", "harnesspromptmarker"]);
    const realSearch = await runCli(fixture, ["search", "genuinepromptmarker"]);
    const migrated = new Database(path);
    const prompts = migrated
      .query<{ text: string }, []>("SELECT text FROM prompts ORDER BY id")
      .all()
      .map(({ text }) => text);
    const searchNoise = migrated
      .query<{ count: number }, []>(
        "SELECT COUNT(*) AS count FROM search_index WHERE surface = 'prompt' AND text LIKE '<task-notification>%'",
      )
      .get()?.count;
    const events = migrated
      .query<{ payload: string }, []>(
        "SELECT payload FROM event_log WHERE type = 'prompt.harness-noise-dropped' ORDER BY id",
      )
      .all();
    migrated.close();

    expect(listed.stdout).toContain(realPrompt);
    expect(listed.stdout).not.toContain("<task-notification>");
    expect(noiseSearch.stdout).toBe("");
    expect(realSearch.stdout).toContain(realPrompt);
    expect(prompts).toEqual([realPrompt]);
    expect(searchNoise).toBe(0);
    expect(events).toHaveLength(1);
    expect(JSON.parse(events[0]!.payload)).toEqual({ count: 1 });

    expect((await runCli(fixture, ["status"])).toMatchObject({ exitCode: 0 }));
    const reopened = new Database(path);
    const eventCount = reopened
      .query<{ count: number }, []>(
        "SELECT COUNT(*) AS count FROM event_log WHERE type = 'prompt.harness-noise-dropped'",
      )
      .get()?.count;
    reopened.close();
    expect(eventCount).toBe(1);
  });
});
