import { expect, test } from "bun:test";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

test("322 help --help prints top-level help without changing existing help forms", async () => {
  await withFixture(async (fixture) => {
    const topLevel = await runCli(fixture, ["help"]);
    const flagged = await runCli(fixture, ["help", "--help"]);
    const perVerb = await runCli(fixture, ["help", "work"]);

    expect(topLevel.exitCode).toBe(0);
    expect(flagged.exitCode).toBe(0);
    expect(flagged.stdout).toBe(topLevel.stdout);
    expect(flagged.stdout).toContain("verbs:");
    expect(perVerb.exitCode).toBe(0);
    expect(perVerb.stdout).toContain("usage: maestro work");
  });
});
