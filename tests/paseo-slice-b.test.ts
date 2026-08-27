import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  idFrom,
  runCli,
  type Fixture,
  withFixture,
} from "./helpers.ts";

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
    "Route repeated failure attention",
    "--owned-scope",
    "fixture",
    "--excluded-scope",
    "product source",
    "--mutation",
    "no-write",
    "--stop-condition",
    "routing is observable",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: CLI output",
    "--pane",
    "w1:p390",
  ];
}

function dispatchId(stdout: string): string {
  const match = stdout.match(/^(x\d+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${stdout}`);
  return match[1];
}

async function addFailedNotes(
  fixture: Fixture,
  work: string,
  environment: Record<string, string>,
): Promise<void> {
  for (const note of ["failed: first", "failed: second", "failed: third"]) {
    expect((await runCli(fixture, ["work", "note", work, note], environment)).exitCode).toBe(0);
  }
}

test("390 REPEATED_FAILURE routes by the current holder role", async () => {
  await withFixture(async (fixture) => {
    const peerWork = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "peer failure episode",
        "--atomic-reason",
        "routing fixture",
      ]),
    );
    const leadWork = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "lead failure episode",
        "--atomic-reason",
        "routing fixture",
      ]),
    );
    const peer = session("peer-holder");
    const lead = session("lead-holder");
    const opened = await runCli(fixture, dispatchOpenArgs(peerWork));
    expect(opened.exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatchId(opened.stdout)], peer)).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "start", peerWork], peer)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "start", leadWork], lead)).exitCode).toBe(0);
    await addFailedNotes(fixture, peerWork, peer);
    await addFailedNotes(fixture, leadWork, lead);

    const hook = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("lead-viewer"),
    );
    expect(hook.exitCode).toBe(0);
    expect(hook.stdout).toContain(`attention REPEATED_FAILURE ${peerWork}`);
    expect(hook.stdout).not.toContain(`attention REPEATED_FAILURE ${leadWork}`);

    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const room = await runCli(fixture, ["brief"], { MAESTRO_READ_ONLY: "1" });
    expect(room.exitCode).toBe(0);
    expect(room.stdout).not.toContain(`attention REPEATED_FAILURE ${peerWork}`);
    expect(room.stdout).toContain(`attention REPEATED_FAILURE ${leadWork}`);

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner"));
    expect(attention.exitCode).toBe(0);
    const findings = (JSON.parse(attention.stdout) as {
      data: {
        detections: Array<{
          holderRole?: string;
          kind: string;
          packet: string;
          route?: string;
          subjectWork: string | null;
        }>;
      };
    }).data.detections.filter((finding) => finding.kind === "REPEATED_FAILURE");
    expect(findings).toContainEqual(
      expect.objectContaining({
        holderRole: "peer",
        route: "lead",
        subjectWork: peerWork,
        packet: expect.stringContaining("holder role: peer"),
      }),
    );
    expect(findings).toContainEqual(
      expect.objectContaining({
        holderRole: "lead",
        route: "supervisor",
        subjectWork: leadWork,
        packet: expect.stringContaining("holder role: lead"),
      }),
    );
  });
});
