import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchOpenArgs(work: string, targetSession: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return the delivery result",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, publish",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the handback is filed",
    "--lane",
    "delivery",
    "--evidence-required",
    "source and live",
    "--target-session",
    targetSession,
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

function handbackFileArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the delivery result is returned",
    "--proof",
    "source: focused test passes",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

async function acceptedDispatch(
  fixture: Fixture,
  work: string,
  holder: string,
): Promise<string> {
  const opened = await runCli(fixture, dispatchOpenArgs(work, holder));
  expect(opened.exitCode).toBe(0);
  const dispatch = dispatchId(opened);
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode)
    .toBe(0);
  return dispatch;
}

test("187 filing a handback releases the filing session's work lease without losing work", async () => {
  await withFixture(async (fixture) => {
    const holder = "filing-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "return delivery", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "note", work, "keep this note"], session(holder))).exitCode,
    ).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE work SET evidence = ? WHERE id = ?").run("source: existing proof", work);
    database.close();
    const dispatch = await acceptedDispatch(fixture, work, holder);

    const filed = await runCli(fixture, handbackFileArgs(dispatch), session(holder));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(shown.exitCode).toBe(0);
    const envelope = JSON.parse(shown.stdout) as {
      data: {
        notes: Array<{ text: string }>;
        work: { evidence: string | null; heldBy: string | null; state: string };
      };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({
        evidence: "source: existing proof",
        heldBy: null,
        state: "open",
      }),
    );
    expect(envelope.data.notes.map((note) => note.text)).toContain("keep this note");
  });
});

test("188 a non-holder handback leaves another session's work lease alone", async () => {
  await withFixture(async (fixture) => {
    const workHolder = "work-holder";
    const filingSession = "filing-non-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "preserve lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(workHolder))).exitCode).toBe(0);
    const dispatch = await acceptedDispatch(fixture, work, filingSession);

    const filed = await runCli(fixture, handbackFileArgs(dispatch), session(filingSession));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    const envelope = JSON.parse(shown.stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({ heldBy: workHolder, state: "active" }),
    );
  });
});

test("189 work release refuses a session that does not hold the lease", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "holder-only release", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("release-holder"))).exitCode)
      .toBe(0);

    const refused = await runCli(fixture, ["work", "release", work], session("other-session"));
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("LEASE_HELD");
    expect(refused.stderr).toContain("release-holder");
    const shown = JSON.parse((await runCli(fixture, ["work", "show", work, "--json"])).stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    };
    expect(shown.data.work).toEqual(
      expect.objectContaining({ heldBy: "release-holder", state: "active" }),
    );
  });
});

test("190 a holder releases open work without losing evidence and records the event", async () => {
  await withFixture(async (fixture) => {
    const holder = "release-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "voluntary release", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE work SET evidence = ? WHERE id = ?").run("source: retained", work);
    database.close();

    const released = await runCli(fixture, ["work", "release", work], session(holder));
    expect(released.exitCode).toBe(0);
    const shown = JSON.parse((await runCli(fixture, ["work", "show", work, "--json"])).stdout) as {
      data: { work: { evidence: string | null; heldBy: string | null; state: string } };
    };
    expect(shown.data.work).toEqual(
      expect.objectContaining({ evidence: "source: retained", heldBy: null, state: "open" }),
    );
    const trace = await runCli(fixture, ["trace", work]);
    expect(trace.stdout).toContain("work.release");
    expect(trace.stdout).toContain(`\"holder\":\"${holder}\"`);
    expect(trace.stdout).not.toContain("work.done");
  });
});

test("191 work reclaim refuses a missing or blank reason", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "reasoned reclaim", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("previous-holder"))).exitCode)
      .toBe(0);

    for (const args of [
      ["work", "reclaim", work],
      ["work", "reclaim", work, "--reason", "   "],
    ]) {
      const refused = await runCli(fixture, args, session("new-holder"));
      expect(refused.exitCode).not.toBe(0);
      expect(refused.stderr).toContain("--reason");
    }
  });
});

test("192 work reclaim records both sessions and the reason without completing work", async () => {
  await withFixture(async (fixture) => {
    const previousHolder = "previous-holder";
    const newHolder = "new-holder";
    const reason = "operator confirmed the prior lane stopped";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "take stopped lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(previousHolder))).exitCode)
      .toBe(0);

    const reclaimed = await runCli(
      fixture,
      ["work", "reclaim", work, "--reason", reason],
      session(newHolder),
    );
    expect(reclaimed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    const envelope = JSON.parse(shown.stdout) as {
      data: {
        work: {
          heldBy: string | null;
          reclaimReason: string | null;
          reclaimedBy: string | null;
          reclaimedFrom: string | null;
          state: string;
        };
      };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({
        heldBy: newHolder,
        reclaimReason: reason,
        reclaimedBy: newHolder,
        reclaimedFrom: previousHolder,
        state: "active",
      }),
    );
    expect((await runCli(fixture, ["work", "show", work])).stdout).toContain(
      `reclaim reason: ${reason}`,
    );
    const trace = await runCli(fixture, ["trace", work]);
    expect(trace.stdout).toContain("work.reclaim");
    expect(trace.stdout).toContain(`\"previousHolder\":\"${previousHolder}\"`);
    expect(trace.stdout).toContain(`\"newHolder\":\"${newHolder}\"`);
    expect(trace.stdout).toContain(`\"reason\":\"${reason}\"`);
    expect(trace.stdout).not.toContain("work.done");
  });
});
