/**
 * Integration test for the task feature's full daily loop.
 *
 * Spawns `bun run src/index.ts` against a tmpdir and runs the canonical
 * agent workflow: create a blocking task, create a dependent task, query
 * ready (expect only the blocker), close the blocker, query ready again
 * (expect only the dependent). This mirrors the brainstorm coordination
 * scenario end-to-end through the real commander surface.
 */
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "..", "..", "src", "index.ts"),
];

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

async function run(
  args: string[],
  cwd = process.cwd(),
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

async function initGitRepo(cwd: string): Promise<void> {
  const init = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await init.exited;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task CLI daily loop", () => {
  it("completes the full create -> ready -> close -> ready cycle", async () => {
    // Step 1: create an unblocked task using quick capture.
    const captured = await run(
      ["task", "q", "login endpoint", "--priority", "1"],
      tmpDir,
    );
    expect(captured.exitCode).toBe(0);
    const apiId = captured.stdout;
    expect(apiId).toMatch(/^tsk-[0-9a-f]{6}$/);

    // Step 2: create a dependent task that is blocked by step 1.
    const dependent = await run(
      [
        "task",
        "create",
        "JWT middleware",
        "--depends-on",
        apiId,
        "--priority",
        "1",
        "--json",
      ],
      tmpDir,
    );
    expect(dependent.exitCode).toBe(0);
    const mw: { id: string; dependsOn: string[] } = JSON.parse(dependent.stdout);
    expect(mw.id).toMatch(/^tsk-[0-9a-f]{6}$/);
    expect(mw.dependsOn).toEqual([apiId]);

    // Step 3: ready returns only the blocker (JSON mode for easy parsing).
    const readyBefore = await run(["task", "ready", "--json"], tmpDir);
    expect(readyBefore.exitCode).toBe(0);
    const beforeList: Array<{ id: string; title: string }> = JSON.parse(
      readyBefore.stdout,
    );
    expect(beforeList.length).toBe(1);
    expect(beforeList[0]?.id).toBe(apiId);

    // Step 4: close the blocker with a reason.
    const closed = await run(
      ["task", "close", apiId, "--reason", "shipped"],
      tmpDir,
    );
    expect(closed.exitCode).toBe(0);
    expect(closed.stdout).toContain("Task closed:");
    expect(closed.stdout).toContain("Reason: shipped");

    // Step 5: ready now returns the previously-dependent task.
    const readyAfter = await run(["task", "ready", "--json"], tmpDir);
    expect(readyAfter.exitCode).toBe(0);
    const afterList: Array<{ id: string; title: string }> = JSON.parse(
      readyAfter.stdout,
    );
    expect(afterList.length).toBe(1);
    expect(afterList[0]?.id).toBe(mw.id);

    // Step 6: show the closed task to verify persistence.
    const showClosed = await run(["task", "show", apiId, "--json"], tmpDir);
    expect(showClosed.exitCode).toBe(0);
    const closedTask: { id: string; status: string; closeReason: string } = JSON.parse(
      showClosed.stdout,
    );
    expect(closedTask.status).toBe("closed");
    expect(closedTask.closeReason).toBe("shipped");

    // Step 7: list --status open returns only the dependent.
    const listOpen = await run(
      ["task", "list", "--status", "open", "--json"],
      tmpDir,
    );
    expect(listOpen.exitCode).toBe(0);
    const openList: Array<{ id: string; status: string }> = JSON.parse(
      listOpen.stdout,
    );
    expect(openList.length).toBe(1);
    expect(openList[0]?.id).toBe(mw.id);
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects --status closed via update (must use close)", async () => {
    const create = await run(
      ["task", "q", "direct close attempt"],
      tmpDir,
    );
    expect(create.exitCode).toBe(0);
    const id = create.stdout;

    const bad = await run(
      ["task", "update", id, "--status", "closed"],
      tmpDir,
    );
    expect(bad.exitCode).not.toBe(0);
    expect(bad.stderr).toContain("Cannot set status to 'closed'");
  }, SLOW_CLI_TIMEOUT_MS);

  it("update --add-label and --remove-label", async () => {
    const created = await run(
      ["task", "create", "labeled", "--labels", "auth,ui", "--json"],
      tmpDir,
    );
    const id: string = JSON.parse(created.stdout).id;

    const added = await run(
      ["task", "update", id, "--add-label", "urgent", "--json"],
      tmpDir,
    );
    const updated: { labels: string[] } = JSON.parse(added.stdout);
    expect(updated.labels).toEqual(["auth", "ui", "urgent"]);

    const removed = await run(
      ["task", "update", id, "--remove-label", "auth", "--json"],
      tmpDir,
    );
    const after: { labels: string[] } = JSON.parse(removed.stdout);
    expect(after.labels).toEqual(["ui", "urgent"]);
  }, SLOW_CLI_TIMEOUT_MS);
});
