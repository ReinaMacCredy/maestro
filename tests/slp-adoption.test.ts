import { expect, test } from "bun:test";
import { join } from "node:path";
import { prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

test("133 install materializes the dispatch, handback, dependency, and episode contracts", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const skill = await Bun.file(
      join(fixture.home, ".agents", "skills", "maestro-work", "SKILL.md"),
    ).text();
    for (const required of [
      "## Dispatch",
      "## Handback",
      "Lane",
      "Excluded scope",
      "DONE",
      "BLOCKED",
      "UNTESTABLE",
      "UNKNOWN",
      "FAILED",
      "CHALLENGE",
      "REOPEN_REQUEST",
      "DEPENDENCY_REQUEST",
      "A+B+C",
      "Attempted",
      "Invariant assumed",
      "Exact failure",
      "What changed between attempts",
      "What did not change",
      "Smallest new information needed",
    ]) {
      expect(skill).toContain(required);
    }
  });
});
