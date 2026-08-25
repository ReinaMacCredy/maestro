import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Settle the storage boundary",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, publish",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the contract is stored",
    "--lane",
    "delivery",
    "--evidence-required",
    "source and live",
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

test("173 dispatch open refuses every missing or blank envelope field", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "dispatch contract", "--atomic-reason", "fixture"]),
    );
    const fields = [
      "--objective",
      "--owned-scope",
      "--excluded-scope",
      "--mutation",
      "--stop-condition",
      "--lane",
      "--evidence-required",
    ];

    for (const field of fields) {
      const args = dispatchOpenArgs(work);
      const index = args.indexOf(field);
      args.splice(index, 2);
      const missing = await runCli(fixture, args);
      expect(missing.exitCode).not.toBe(0);
      expect(missing.stderr).toContain(field);

      const blankArgs = dispatchOpenArgs(work);
      blankArgs[blankArgs.indexOf(field) + 1] = "   ";
      const blank = await runCli(fixture, blankArgs);
      expect(blank.exitCode).not.toBe(0);
      expect(blank.stderr).toContain(field);
    }
  });
});

test("174 dispatch show and list render the complete stored contract and identities", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "render dispatch", "--atomic-reason", "fixture"]),
    );
    const opened = await runCli(
      fixture,
      [...dispatchOpenArgs(work), "--target-session", "lane-one"],
    );
    expect(opened.exitCode).toBe(0);
    const id = dispatchId(opened);

    for (const command of [["dispatch", "show", id], ["dispatch", "list"]]) {
      const rendered = await runCli(fixture, command);
      expect(rendered.exitCode).toBe(0);
      for (const line of [
        `work: ${work}`,
        "objective: Settle the storage boundary",
        "owned scope: src/plugins/dispatch.ts",
        "excluded scope: push, tag, publish",
        "mutation: write-bounded: src/plugins/dispatch.ts",
        "stop condition: the contract is stored",
        "lane: delivery",
        "evidence required: source and live",
        "target session: lane-one",
        "held by: none",
      ]) {
        expect(rendered.stdout).toContain(line);
      }
    }
  });
});

test("175 accepting dispatches never changes the work write lease", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "three lanes", "--atomic-reason", "fixture"]),
    );
    const holders = ["lane-one", "lane-two", "lane-three"];
    const dispatches: string[] = [];
    for (const holder of holders) {
      const opened = await runCli(
        fixture,
        [...dispatchOpenArgs(work), "--target-session", holder],
      );
      expect(opened.exitCode).toBe(0);
      const id = dispatchId(opened);
      dispatches.push(id);
      expect((await runCli(fixture, ["dispatch", "accept", id], session(holder))).exitCode).toBe(0);
    }

    const listed = await runCli(fixture, ["dispatch", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const listEnvelope = JSON.parse(listed.stdout) as {
      data: { dispatches: Array<{ heldBy: string | null; id: string }> };
    };
    expect(
      listEnvelope.data.dispatches
        .filter((dispatch) => dispatches.includes(dispatch.id))
        .map((dispatch) => dispatch.heldBy),
    ).toEqual(holders);

    const shown = await runCli(fixture, ["work", "show", work]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).not.toContain("held by:");
  });
});
