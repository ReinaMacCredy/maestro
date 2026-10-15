import { expect, test } from "bun:test";
import { runCli, withFixture } from "./helpers.ts";

const grantLine = "delivery grant: policy-auto-pr enabled";

test("policy-auto-pr ships disabled and adds the delivery grant line to the SessionStart brief only while enabled", async () => {
  await withFixture(async (fixture) => {
    const listed = await runCli(fixture, ["plugin", "list"]);
    const defaultBrief = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);
    expect((await runCli(fixture, ["plugin", "enable", "policy-auto-pr"])).exitCode).toBe(0);
    const enabledBrief = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);
    expect((await runCli(fixture, ["plugin", "disable", "policy-auto-pr"])).exitCode).toBe(0);
    const disabledBrief = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);

    expect(listed.stdout).toContain("policy-auto-pr\tbuilt-in\tdisabled\t");
    expect(defaultBrief.exitCode).toBe(0);
    expect(defaultBrief.stdout).not.toContain("delivery grant:");
    expect(enabledBrief.exitCode).toBe(0);
    expect(enabledBrief.stdout).toContain(grantLine);
    expect(enabledBrief.stdout).toContain("merge, tag and release stay owner gates");
    expect(disabledBrief.stdout).not.toContain("delivery grant:");
  });
});
