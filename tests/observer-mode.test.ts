import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

const withoutSession = { MAESTRO_SESSION_NONE: "1" };

test("208 sessionless work start refuses a lease that cannot survive one read", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "trap check", "--atomic-reason", "fixture"]),
    );

    const started = await runCli(fixture, ["work", "start", work], withoutSession);
    expect(started.exitCode).toBe(1);
    expect(started.stderr).toContain('"code":"SESSION_REQUIRED"');
    expect(started.stderr).toContain("remove MAESTRO_SESSION_NONE");

    const shown = await runCli(fixture, ["work", "show", work], withoutSession);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`${work} [open]`);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      expect(
        database.query<{ held_by: string | null }, [string]>(
          "SELECT held_by FROM work WHERE id = ?",
        ).get(work)?.held_by,
      ).toBeNull();
      expect(
        database.query<{ count: number }, []>(
          "SELECT count(*) AS count FROM sessions WHERE id = 'supervisor'",
        ).get()?.count,
      ).toBe(0);
    } finally {
      database.close();
    }
  });
});

test("209 every session-attributed write door shares the SESSION_REQUIRED gate", async () => {
  await withFixture(async (fixture) => {
    const commands = [
      ["work", "add"],
      ["work", "start"],
      ["work", "release"],
      ["work", "reclaim"],
      ["work", "note"],
      ["work", "done"],
      ["work", "cancel"],
      ["decision", "draft"],
      ["decision", "lock"],
      ["bundle", "open"],
      ["bundle", "close"],
      ["handoff"],
      ["bundle", "save"],
      ["dispatch", "open"],
      ["dispatch", "accept"],
      ["dispatch", "cancel"],
      ["dispatch", "unseal"],
      ["handback", "file"],
      ["msg", "send"],
      ["msg", "read"],
      ["hook", "record"],
      ["plugin", "enable"],
      ["plugin", "disable"],
      ["plugin", "new"],
      ["plugin", "add"],
      ["plugin", "remove"],
      ["install"],
    ];

    for (const command of commands) {
      const result = await runCli(fixture, command, withoutSession);
      expect(result.exitCode, command.join(" ")).toBe(1);
      expect(result.stderr, command.join(" ")).toContain('"code":"SESSION_REQUIRED"');
      expect(result.stderr, command.join(" ")).toContain("remove MAESTRO_SESSION_NONE");
    }
  });
});
