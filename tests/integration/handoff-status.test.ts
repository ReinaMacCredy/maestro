import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;

async function run(
  args: string[],
  cwd: string,
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

describe("handoff status integration", () => {
  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-handoff-status-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("status --json reports pending UKI handoffs", async () => {
    const create = await run([
      "handoff",
      "create",
      "--session-core", "status_test",
      "--summary", "Status_test",
      "--next-action", "inspect_status",
      "--artifact", "branch_main",
      "--confidence-work", "0.9",
      "--json",
    ], tmpDir);
    expect(create.exitCode).toBe(0);

    const status = await run(["status", "--json"], tmpDir);
    expect(status.exitCode).toBe(0);
    const parsed = JSON.parse(status.stdout);
    expect(parsed.pendingHandoffs).toHaveLength(1);
  });
});
