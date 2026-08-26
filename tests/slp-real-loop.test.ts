import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return the detector result",
    "--owned-scope",
    "scratch",
    "--excluded-scope",
    "product source",
    "--mutation",
    "no-write",
    "--stop-condition",
    "the lane reports",
    "--lane",
    "delivery",
    "--evidence-required",
    "journey",
    "--target-session",
    "worker-session",
  ];
}

async function terminalWork(fixture: Fixture, terminal: "cancelled" | "done"): Promise<string> {
  const work = idFrom(
    await runCli(fixture, [
      "work",
      "add",
      `${terminal} dispatch subject`,
      "--atomic-reason",
      "real loop fixture",
    ]),
  );
  if (terminal === "done") {
    expect((await runCli(fixture, ["work", "start", work])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", work, "--evidence", "source: fixture"]))
        .exitCode,
    ).toBe(0);
  } else {
    expect(
      (await runCli(fixture, ["work", "cancel", work, "--reason", "fixture terminal state"]))
        .exitCode,
    ).toBe(0);
  }
  return work;
}

test("198 unreturned dispatch attention ignores done and cancelled work", async () => {
  await withFixture(async (fixture) => {
    const dispatches: string[] = [];
    for (const terminal of ["done", "cancelled"] as const) {
      const work = await terminalWork(fixture, terminal);
      const opened = await runCli(fixture, dispatchOpenArgs(work));
      expect(opened.exitCode).toBe(0);
      dispatches.push(opened.stdout.trim().split(/\s+/)[0] as string);
    }

    const scanned = await runCli(fixture, [
      "attention",
      "--json",
      "--dispatch-stale",
      "0.000001",
    ]);
    expect(scanned.exitCode).toBe(0);
    const envelope = JSON.parse(scanned.stdout) as {
      data: { detections: Array<{ fingerprint: string; kind: string }> };
    };
    expect(
      envelope.data.detections.filter(
        (finding) =>
          finding.kind === "DISPATCH_UNRETURNED" &&
          dispatches.some((dispatch) => finding.fingerprint.includes(dispatch)),
      ),
    ).toEqual([]);
  });
});
