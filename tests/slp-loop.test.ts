import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, type Fixture, withFixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

async function waitFor<T>(
  read: () => T | Promise<T>,
  accept: (value: T) => boolean,
  timeoutMs = 5_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let value = await read();
  while (!accept(value) && Date.now() < deadline) {
    await Bun.sleep(50);
    value = await read();
  }
  expect(accept(value)).toBe(true);
  return value;
}

async function addWork(fixture: Fixture, title: string): Promise<string> {
  return idFrom(
    await runCli(fixture, ["work", "add", title, "--atomic-reason", "slp loop fixture"]),
  );
}

async function addFailedNotes(fixture: Fixture, workId: string): Promise<void> {
  for (const text of ["failed: first", "failed: second", "failed: third"]) {
    expect((await runCli(fixture, ["work", "note", workId, text])).exitCode).toBe(0);
  }
}

test("152 supervisor stop preserves live daemon state and reports failure honestly", async () => {
  await withFixture(async (fixture) => {
    const controller = session("stop-controller");
    const statePath = join(fixture.repo, ".maestro", "supervisor.json");
    expect(
      (await runCli(fixture, ["supervisor", "start", "--interval", "1"], controller)).exitCode,
    ).toBe(0);
    const state = await waitFor(
      async () => JSON.parse(await Bun.file(statePath).text()) as {
        lastTick: string | null;
        pid: number;
      },
      (value) => typeof value.lastTick === "string",
    );

    process.kill(state.pid, "SIGSTOP");
    try {
      const stopped = await runCli(fixture, ["supervisor", "stop"], controller);
      expect(stopped.exitCode).not.toBe(0);
      expect(stopped.stderr).toContain(
        `supervisor did not exit (pid ${state.pid}); run: kill -9 ${state.pid}`,
      );
      expect(await Bun.file(statePath).exists()).toBe(true);

      const status = await runCli(fixture, ["supervisor", "status"], controller);
      expect(status.exitCode).toBe(0);
      expect(status.stdout).toContain("supervisor running");
      expect(status.stdout).toContain(`pid: ${state.pid}`);
    } finally {
      process.kill(state.pid, "SIGCONT");
      const cleanup = await runCli(fixture, ["supervisor", "stop"], controller);
      expect(cleanup.exitCode).toBe(0);
    }

    expect(await Bun.file(statePath).exists()).toBe(false);
  });
});

test("153 repeated-failure attention skips terminal work and retains open and active work", async () => {
  await withFixture(async (fixture) => {
    const done = await addWork(fixture, "terminal done");
    const cancelled = await addWork(fixture, "terminal cancelled");
    const open = await addWork(fixture, "still open");
    const active = await addWork(fixture, "still active");

    expect((await runCli(fixture, ["work", "start", done], session("done-holder"))).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "start", cancelled], session("cancel-holder"))).exitCode,
    ).toBe(0);
    expect(
      (await runCli(fixture, ["work", "start", active], session("active-holder"))).exitCode,
    ).toBe(0);
    for (const workId of [done, cancelled, open, active]) await addFailedNotes(fixture, workId);

    expect(
      (
        await runCli(
          fixture,
          ["work", "done", done, "--claim", "terminal done", "--proof", "notes recorded"],
          session("done-holder"),
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          ["work", "cancel", cancelled, "--reason", "terminal cancelled"],
          session("cancel-holder"),
        )
      ).exitCode,
    ).toBe(0);

    const result = await runCli(fixture, ["attention", "--json"], session("scanner"));
    expect(result.exitCode).toBe(0);
    const output = JSON.parse(result.stdout) as {
      data: { detections: Array<{ kind: string; subjectWork: string }> };
    };
    expect(
      output.data.detections
        .filter((finding) => finding.kind === "REPEATED_FAILURE")
        .map((finding) => finding.subjectWork),
    ).toEqual([open, active]);
  });
});
