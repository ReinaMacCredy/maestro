import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  addLinkedWorktree,
  idFrom,
  initializeGitRepository,
  runCli,
  runCliAt,
  withFixture,
} from "./helpers.ts";

function sessionEnvironment(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

test("B3.10 concurrent shared-store startup and work creation avoid locks and duplicate IDs", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "concurrent-worktree");
    await addLinkedWorktree(fixture.repo, worktree);

    const statuses = await Promise.all(
      Array.from({ length: 12 }, (_, index) =>
        runCliAt(
          fixture,
          index % 2 === 0 ? fixture.repo : worktree,
          ["status"],
          sessionEnvironment(`status-${index}`),
        ),
      ),
    );
    expect(
      statuses
        .filter((result) => result.exitCode !== 0)
        .map((result) => result.stderr.trim()),
    ).toEqual([]);

    const additions = await Promise.all(
      Array.from({ length: 20 }, (_, index) =>
        runCliAt(
          fixture,
          index % 2 === 0 ? fixture.repo : worktree,
          ["work", "add", `concurrent item ${index}`],
          sessionEnvironment(`add-${index}`),
        ),
      ),
    );
    expect(
      additions
        .filter((result) => result.exitCode !== 0)
        .map((result) => result.stderr.trim()),
    ).toEqual([]);
    const ids = additions.map(idFrom);
    expect(new Set(ids).size).toBe(additions.length);
  });
});

test("B3.11 concurrent starts produce exactly one lease winner", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "lease-worktree");
    await addLinkedWorktree(fixture.repo, worktree);
    const parent = idFrom(await runCli(fixture, ["work", "add", "lease parent"]));
    const target = idFrom(
      await runCli(fixture, ["work", "add", "lease target", "--parent", parent]),
    );
    const delayPlugin = `
export default {
  name: "start-delay",
  apply(context) {
    context.effect(() => context.events.on("work.start", async (input, next) => {
      await Bun.sleep(300);
      return next(input);
    }));
  },
};
`;
    for (const checkout of [fixture.repo, worktree]) {
      const plugins = join(checkout, ".maestro", "plugins");
      await mkdir(plugins, { recursive: true });
      await writeFile(join(plugins, "start-delay.ts"), delayPlugin);
    }

    const results = await Promise.all([
      runCli(fixture, ["work", "start", target], sessionEnvironment("lease-a")),
      runCliAt(
        fixture,
        worktree,
        ["work", "start", target],
        sessionEnvironment("lease-b"),
      ),
    ]);
    const successes = results.filter((result) => result.exitCode === 0);
    const failures = results.filter((result) => result.exitCode !== 0);
    expect(successes).toHaveLength(1);
    expect(failures).toHaveLength(1);
    expect(failures[0]?.stderr).toContain('"code":"LEASE_HELD"');
  });
});
