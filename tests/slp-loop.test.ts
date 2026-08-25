import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

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
