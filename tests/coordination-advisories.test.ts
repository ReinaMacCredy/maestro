import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture } from "./helpers.ts";

function sessionEnvironment(id: string, pid = process.pid): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(pid),
  };
}

test("B3.5 sibling overlap warns for live holders but stays silent for dead or unrelated holders", async () => {
  await withFixture(async (fixture) => {
    const firstParent = idFrom(await runCli(fixture, ["work", "add", "first parent"]));
    const held = idFrom(
      await runCli(fixture, ["work", "add", "held sibling", "--parent", firstParent]),
    );
    const claimed = idFrom(
      await runCli(fixture, ["work", "add", "claimed sibling", "--parent", firstParent]),
    );
    expect((await runCli(fixture, ["work", "start", held], sessionEnvironment("live-a"))).exitCode).toBe(0);

    const overlap = await runCli(
      fixture,
      ["work", "start", claimed],
      sessionEnvironment("live-b"),
    );
    expect(overlap.exitCode).toBe(0);
    expect(overlap.stderr).toContain("[overlap]");
    expect(overlap.stderr).toContain("live-a");
    expect(overlap.stderr).toContain(held);

    const unrelatedParent = idFrom(await runCli(fixture, ["work", "add", "other parent"]));
    const unrelated = idFrom(
      await runCli(fixture, ["work", "add", "unrelated child", "--parent", unrelatedParent]),
    );
    const unrelatedStart = await runCli(
      fixture,
      ["work", "start", unrelated],
      sessionEnvironment("live-c"),
    );
    expect(unrelatedStart.exitCode).toBe(0);
    expect(unrelatedStart.stderr).not.toContain("[overlap]");

    const deadParent = idFrom(await runCli(fixture, ["work", "add", "dead parent"]));
    const deadHeld = idFrom(
      await runCli(fixture, ["work", "add", "dead held", "--parent", deadParent]),
    );
    const deadSibling = idFrom(
      await runCli(fixture, ["work", "add", "dead sibling", "--parent", deadParent]),
    );
    expect(
      (await runCli(fixture, ["work", "start", deadHeld], sessionEnvironment("dead-a", 99_999_999)))
        .exitCode,
    ).toBe(0);
    const deadStart = await runCli(
      fixture,
      ["work", "start", deadSibling],
      sessionEnvironment("live-d"),
    );
    expect(deadStart.exitCode).toBe(0);
    expect(deadStart.stderr).not.toContain("[overlap]");
  });
});

test("B3.6 status and hook briefs show live peers with held work and stay silent when alone", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "peer parent"]));
    const held = idFrom(
      await runCli(fixture, ["work", "add", "peer-held work", "--parent", parent]),
    );
    expect((await runCli(fixture, ["work", "start", held], sessionEnvironment("peer-a"))).exitCode).toBe(0);
    expect(
      (await runCli(
        fixture,
        // The literal two-character \\n an agent types in a shell, not a newline.
        ["work", "note", held, "checkpoint:\\nstate: half done\\nnext: finish the seam"],
        sessionEnvironment("peer-a"),
      )).exitCode,
    ).toBe(0);
    const holderBrief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      sessionEnvironment("peer-a"),
    );
    expect(holderBrief.stdout).toContain(
      `checkpoint ${held}: state: half done | next: finish the seam | avoid: none`,
    );
    const holderPrompt = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      sessionEnvironment("peer-a"),
    );
    expect(holderPrompt.stdout).toContain(`held work: ${held}`);
    expect(holderPrompt.stdout).not.toContain("checkpoint ");

    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      sessionEnvironment("peer-b"),
    );
    expect(brief.stdout).toContain("live peers:");
    expect(brief.stdout).toContain("peer-a");
    expect(brief.stdout).toContain(held);
    expect(brief.stdout).not.toContain("checkpoint ");
  });

  await withFixture(async (fixture) => {
    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      sessionEnvironment("solo"),
    );
    expect(brief.stdout).not.toContain("live peers:");
  });
});
