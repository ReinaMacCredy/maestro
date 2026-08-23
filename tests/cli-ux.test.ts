import { expect, test } from "bun:test";
import { runCli, withFixture } from "./helpers.ts";

test("25 bare and help invocations list verbs while unknown verbs suggest the nearest verb", async () => {
  await withFixture(async (fixture) => {
    const bare = await runCli(fixture, []);
    const help = await runCli(fixture, ["help"]);
    const flagHelp = await runCli(fixture, ["--help"]);
    const unknown = await runCli(fixture, ["wrok", "discarded-tail"]);
    const error = JSON.parse(unknown.stderr.trim());

    for (const result of [bare, help, flagHelp]) {
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("work");
      expect(result.stdout).toContain("plugin");
      expect(result.stdout).toContain("install");
    }
    expect(unknown.exitCode).not.toBe(0);
    expect(error.error.code).toBe("UNKNOWN_VERB");
    expect(error.error.message).toContain("work");
    expect(error.error.message).not.toContain("discarded-tail");
  });
});
