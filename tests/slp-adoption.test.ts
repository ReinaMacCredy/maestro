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

test("134 install materializes layered proof, failed traces, learning, and triage", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const verifyRoot = join(fixture.home, ".agents", "skills", "maestro-verify");
    const skill = await Bun.file(join(verifyRoot, "SKILL.md")).text();
    for (const required of [
      "source",
      "artifact",
      "installed",
      "live",
      "journey",
      "NOT TESTED",
      "Assumptions not verified",
      "Residual risks",
      '"failed: ',
    ]) {
      expect(skill).toContain(required);
    }

    const learning = await Bun.file(join(verifyRoot, "references", "learning.md")).text();
    expect(learning).toContain("canary");
    expect(learning).toContain("review/delete date");

    const triagePath = join(verifyRoot, "references", "triage.md");
    expect(await Bun.file(triagePath).exists()).toBe(true);
    const triage = await Bun.file(triagePath).text();
    for (const step of [
      "Problem",
      "Authority",
      "Topology",
      "Attention",
      "Capability",
      "State",
      "Evidence",
      "Owning layer",
      "Learning",
    ]) {
      expect(triage).toContain(step);
    }

    const audit = await Bun.file(join(verifyRoot, "references", "audit.md")).text();
    expect(audit).toContain("triage.md");
  });
});
